//! Browser entry point and small adapters for browser-owned resources.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
const pendingImages = [];
let notifyPaint = () => {};
let unsaved = false;

export function installBrowserEvents(notify, canvas) {
    notifyPaint = notify;

    // eframe forwards Ctrl/Cmd+R to Paint but deliberately leaves the browser's
    // Reload default enabled. Keep the ruler shortcut inside the focused canvas;
    // F5 and the browser Reload button still provide normal page reload.
    canvas.addEventListener('keydown', event => {
        if (
            (event.ctrlKey || event.metaKey)
            && !event.shiftKey
            && !event.altKey
            && event.key.toLowerCase() === 'r'
        ) {
            event.preventDefault();
        }
    }, { capture: true });

    window.addEventListener('beforeunload', event => {
        if (unsaved) {
            event.preventDefault();
            event.returnValue = '';
        }
    });

    // eframe handles text at document level and stops the event from bubbling.
    // Capture image data before that listener, leaving text-only paste to it.
    window.addEventListener('paste', async event => {
        const target = event.target;
        const textTarget = target?.tagName === 'INPUT'
            || target?.tagName === 'TEXTAREA'
            || target?.isContentEditable;
        if (textTarget && event.clipboardData?.getData('text/plain')) {
            return;
        }

        const item = [...(event.clipboardData?.items || [])]
            .find(item => item.type.startsWith('image/'));
        if (!item) {
            return;
        }

        const file = item.getAsFile();
        if (!file || file.size > 268435456) {
            return;
        }

        event.preventDefault();
        event.stopPropagation();
        pendingImages.push(new Uint8Array(await file.arrayBuffer()));
        notifyPaint();
    }, { capture: true });
}

export function takePastedImages() {
    return pendingImages.splice(0);
}

export function setUnsaved(value) {
    unsaved = value;
}

export function downloadBytes(name, mime, bytes) {
    const url = URL.createObjectURL(new Blob([bytes], { type: mime }));
    const link = document.createElement('a');
    link.href = url;
    link.download = name;
    document.body.appendChild(link);
    link.click();
    link.remove();
    setTimeout(() => URL.revokeObjectURL(url), 60000);
}

export function chooseFile(accept) {
    return new Promise((resolve, reject) => {
        const input = document.createElement('input');
        input.type = 'file';
        input.accept = accept;
        input.hidden = true;
        document.body.appendChild(input);

        input.addEventListener('cancel', () => {
            input.remove();
            resolve(null);
        }, { once: true });

        input.addEventListener('change', async () => {
            const file = input.files[0];
            input.remove();

            if (!file) {
                resolve(null);
                return;
            }
            if (file.size > 268435456) {
                reject(new Error('Files must be no larger than 256 MiB.'));
                return;
            }

            try {
                resolve({
                    name: file.name,
                    bytes: new Uint8Array(await file.arrayBuffer()),
                });
            } catch (error) {
                reject(error);
            }
        }, { once: true });

        input.click();
    });
}

async function clipboardText(item) {
    const text = await (await item.getType('text/plain')).text();
    // Match eframe's keyboard-paste line endings.
    return { text: text.replace(/\r\n/g, '\n') };
}

export async function readClipboard(preferText = false) {
    if (!navigator.clipboard?.read) {
        throw new Error('Use Ctrl+V / Command+V, or Paste from, to grant clipboard access.');
    }

    const items = await navigator.clipboard.read();
    const textItem = preferText && items.find(item => item.types.includes('text/plain'));
    if (textItem) {
        return clipboardText(textItem);
    }

    for (const item of items) {
        const type = item.types.find(type => type.startsWith('image/'));
        if (type) {
            const blob = await item.getType(type);
            if (blob.size > 268435456) {
                throw new Error('Clipboard image exceeds 256 MiB.');
            }
            return { bytes: new Uint8Array(await blob.arrayBuffer()) };
        }

        if (item.types.includes('text/plain')) {
            return clipboardText(item);
        }
    }

    throw new Error('No picture or text is available on the clipboard.');
}

export async function writeClipboard(bytes) {
    if (!navigator.clipboard?.write || !window.ClipboardItem) {
        throw new Error('Image clipboard access is unavailable.');
    }

    await navigator.clipboard.write([
        new ClipboardItem({
            'image/png': new Blob([bytes], { type: 'image/png' }),
        }),
    ]);
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = installBrowserEvents)]
    fn install_browser_events(notify: &js_sys::Function, canvas: &web_sys::HtmlCanvasElement);
    #[wasm_bindgen(js_name = takePastedImages)]
    pub(crate) fn take_pasted_images() -> js_sys::Array;
    #[wasm_bindgen(js_name = setUnsaved)]
    pub(crate) fn set_unsaved(value: bool);
    #[wasm_bindgen(catch, js_name = downloadBytes)]
    fn download_bytes(name: &str, mime: &str, bytes: &[u8]) -> Result<(), JsValue>;
    #[wasm_bindgen(catch, js_name = chooseFile)]
    pub(crate) async fn choose_file(accept: &str) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = readClipboard)]
    pub(crate) async fn read_clipboard(prefer_text: bool) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = writeClipboard)]
    pub(crate) async fn write_clipboard(bytes: &[u8]) -> Result<(), JsValue>;
}

pub fn download(filename: &str, mime: &str, bytes: &[u8]) -> Result<(), String> {
    download_bytes(filename, mime, bytes).map_err(error_message)
}

pub(crate) fn error_message(error: JsValue) -> String {
    error
        .as_string()
        .or_else(|| {
            js_sys::Reflect::get(&error, &"message".into())
                .ok()?
                .as_string()
        })
        .unwrap_or_else(|| "The browser could not complete this operation.".into())
}

pub(crate) fn install(ctx: &eframe::egui::Context, canvas: &web_sys::HtmlCanvasElement) {
    let ctx = ctx.clone();
    let notify = Closure::<dyn Fn()>::new(move || ctx.request_repaint());
    install_browser_events(notify.as_ref().unchecked_ref(), canvas);
    notify.forget();
}

#[wasm_bindgen]
pub struct WebHandle {
    runner: eframe::WebRunner,
}

#[wasm_bindgen]
impl WebHandle {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            runner: eframe::WebRunner::new(),
        }
    }

    pub async fn start(&self, canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue> {
        let event_canvas = canvas.clone();
        self.runner
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(move |cc| {
                    install(&cc.egui_ctx, &event_canvas);
                    Ok(Box::new(crate::app::PaintApp::new(cc)))
                }),
            )
            .await
    }

    pub fn has_panicked(&self) -> bool {
        self.runner.has_panicked()
    }
    pub fn panic_message(&self) -> Option<String> {
        self.runner.panic_summary().map(|summary| summary.message())
    }
    pub fn destroy(&self) {
        self.runner.destroy();
    }
}

impl Default for WebHandle {
    fn default() -> Self {
        Self::new()
    }
}
