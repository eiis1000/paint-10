//! Browser file access preserves the native document and codec contracts.

use super::*;
use crate::raster_io::RasterFormat;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::JsValue;

enum BrowserResult {
    File {
        name: String,
        bytes: Vec<u8>,
        open: bool,
    },
    Clipboard(JsValue),
    Font {
        name: String,
        bytes: Vec<u8>,
    },
    Message(String),
}

pub(super) struct DownloadDialog {
    name: String,
    format: RasterFormat,
    selection: Option<RgbaImage>,
    copy: bool,
    resume: Option<Action>,
    error: Option<String>,
    initial_focus: bool,
}

pub(super) struct BrowserState {
    ctx: Context,
    events: Rc<RefCell<Vec<BrowserResult>>>,
    pub(super) save: Option<DownloadDialog>,
    format: RasterFormat,
}

impl BrowserState {
    pub(super) fn new(ctx: Context) -> Self {
        Self {
            ctx,
            events: Default::default(),
            save: None,
            format: RasterFormat::Png,
        }
    }
}

impl PaintApp {
    pub(in crate::app) fn choose_browser_font(&mut self) {
        let events = self.web.events.clone();
        let ctx = self.web.ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = match crate::web::choose_file(".ttf,.otf,.ttc").await {
                Ok(value) if value.is_null() || value.is_undefined() => return,
                Ok(value) => BrowserResult::Font {
                    name: property(&value, "name")
                        .as_string()
                        .unwrap_or_else(|| "font".into()),
                    bytes: js_sys::Uint8Array::new(&property(&value, "bytes")).to_vec(),
                },
                Err(error) => BrowserResult::Message(crate::web::error_message(error)),
            };
            events.borrow_mut().push(result);
            ctx.request_repaint();
        });
    }

    fn import_browser_font(&mut self, name: &str, bytes: Vec<u8>) {
        if bytes.len() > crate::text::MAX_FONT_BYTES {
            self.message = "Font files must be no larger than 32 MiB.".into();
            return;
        }
        let mut candidate = fontdb::Database::new();
        candidate.load_font_data(bytes.clone());
        let usable = candidate.faces().any(|face| {
            candidate
                .with_face_data(face.id, |data, index| {
                    ab_glyph::FontRef::try_from_slice_and_index(data, index).is_ok()
                })
                .unwrap_or(false)
        });
        if !usable {
            self.message = "This file has no supported TrueType/OpenType font faces.".into();
            return;
        }
        self.font_db.load_font_data(bytes);
        self.font_names = text_editing::font_families(&self.font_db);
        self.message = format!("Loaded {name}. Choose its family in the Font list. Used fonts are embedded when you save a Paint 10 project.");
    }

    pub(in crate::app) fn read_image(_path: &std::path::Path) -> Result<RgbaImage, String> {
        Err("Use Open or Paste from to choose a file in the browser.".into())
    }

    pub(in crate::app) fn choose_browser_file(&mut self, open: bool) {
        let events = self.web.events.clone();
        let ctx = self.web.ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = match crate::web::choose_file(
                ".p10,.png,.jpg,.jpeg,.jpe,.jfif,.bmp,.dib,.gif,.tif,.tiff,.webp,.ico",
            )
            .await
            {
                Ok(value) if value.is_null() || value.is_undefined() => return,
                Ok(value) => BrowserResult::File {
                    name: property(&value, "name")
                        .as_string()
                        .unwrap_or_else(|| "Untitled".into()),
                    bytes: js_sys::Uint8Array::new(&property(&value, "bytes")).to_vec(),
                    open,
                },
                Err(error) => BrowserResult::Message(crate::web::error_message(error)),
            };
            events.borrow_mut().push(result);
            ctx.request_repaint();
        });
    }

    pub(in crate::app) fn import_browser_bytes(&mut self, name: &str, bytes: &[u8], open: bool) {
        let path = PathBuf::from(name);
        let format = crate::raster_io::detect_format_bytes(bytes, &path);
        let result = if format == Some(RasterFormat::Project) {
            crate::project::decode(bytes)
        } else {
            crate::raster_io::decode_bytes_with_resolution(bytes).map(|(image, resolution)| {
                let mut doc = Document::from_image(image);
                doc.resolution = resolution;
                doc.mono = format == Some(RasterFormat::BmpMono);
                doc
            })
        };
        match result {
            Ok(document) if open => {
                self.doc = document;
                self.measure.reset();
                self.register_document_fonts();
                self.file = Some(path);
                self.web.format = format.unwrap_or_default();
                self.clear_selection();
                self.refresh = true;
                self.message =
                    "Opened in the browser. Save downloads a copy to your device.".into();
            }
            Ok(document) => self.insert_image(document.composite()),
            Err(error) => self.message = error,
        }
    }

    pub(in crate::app) fn save(&mut self, save_as: bool) -> bool {
        self.finish_editing();
        if let Some(path) = self.file.as_ref().filter(|_| !save_as) {
            return match self.download_document(
                self.web.format,
                None,
                path.to_string_lossy().as_ref(),
            ) {
                Ok(name) => {
                    self.message = format!("Download started: {name}. Confirm the file was saved; browsers cannot report download completion.");
                    false
                }
                Err(error) => {
                    self.message = format!("Could not save: {error}");
                    false
                }
            };
        }
        self.open_download_dialog(None, false);
        false
    }

    pub(in crate::app) fn save_as_format(&mut self, format: RasterFormat) -> bool {
        self.finish_editing();
        self.open_download_dialog(None, false);
        if let Some(dialog) = &mut self.web.save {
            dialog.format = format;
        }
        false
    }

    pub(in crate::app) fn export_copy(&mut self, _ctx: &Context, selection_only: bool) {
        let shape = self.finish_editing();
        let selection = if selection_only {
            match self
                .selected_image()
                .or_else(|| shape.map(|bounds| bounds.extract(&self.doc.composite())))
            {
                Some(image) => Some(image),
                None => {
                    self.message = "Select part of the picture before saving a selection.".into();
                    return;
                }
            }
        } else {
            None
        };
        self.open_download_dialog(selection, true);
    }

    fn open_download_dialog(&mut self, selection: Option<RgbaImage>, copy: bool) {
        let stem = self
            .file
            .as_deref()
            .and_then(|path| path.file_stem())
            .unwrap_or_default()
            .to_string_lossy();
        let stem = if stem.is_empty() { "Untitled" } else { &stem };
        let suffix = if selection.is_some() {
            " - selection"
        } else if copy {
            " - copy"
        } else {
            ""
        };
        self.web.save = Some(DownloadDialog {
            name: format!("{stem}{suffix}"),
            format: if copy {
                RasterFormat::Png
            } else if self.doc.mono {
                RasterFormat::BmpMono
            } else if self.file.is_none() {
                RasterFormat::Png
            } else {
                self.web.format
            },
            selection,
            copy,
            resume: self.pending.take(),
            error: None,
            initial_focus: true,
        });
    }

    fn download_document(
        &self,
        format: RasterFormat,
        selection: Option<&RgbaImage>,
        name: &str,
    ) -> Result<String, String> {
        let name = std::path::Path::new(name.trim())
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .ok_or("Enter a filename.")?;
        let mut path = PathBuf::from(name);
        if !format.matches_path(&path) {
            path.set_extension(format.extensions()[0]);
        }
        let bytes = if format == RasterFormat::Project {
            if let Some(image) = selection {
                let mut document = Document::from_image(image.clone());
                document.resolution = self.doc.resolution;
                document.mono = self.doc.mono;
                crate::project::encode(&document)?
            } else {
                crate::project::encode(&self.doc)?
            }
        } else {
            let composite;
            let image = if let Some(image) = selection {
                image
            } else {
                composite = self.doc.composite();
                &composite
            };
            crate::raster_io::encode_with_resolution(image, format, self.doc.resolution)?
        };
        let filename = path.to_string_lossy().into_owned();
        crate::web::download(&filename, "application/octet-stream", &bytes)?;
        Ok(filename)
    }

    pub(in crate::app) fn browser_save_dialog(&mut self, ctx: &Context) {
        let Some(mut dialog) = self.web.save.take() else {
            return;
        };
        let action = dialogs::browser_download_controls(
            ctx,
            &mut dialog.name,
            &mut dialog.format,
            dialog.copy,
            dialog.error.as_deref(),
            &mut dialog.initial_focus,
        );
        if action == dialogs::DownloadAction::Download {
            match self.download_document(dialog.format, dialog.selection.as_ref(), &dialog.name) {
                Ok(name) => {
                    if !dialog.copy {
                        self.file = Some(PathBuf::from(&name));
                        self.web.format = dialog.format;
                    }
                    self.message = format!("Download started: {name}. Confirm the file was saved; browsers cannot report download completion.");
                    if let Some(action) = dialog.resume {
                        self.pending = Some(action);
                        self.message
                            .push_str(" After confirming it, choose Don't save to continue.");
                    }
                    return;
                }
                Err(error) => dialog.error = Some(error),
            }
        }
        if action == dialogs::DownloadAction::Cancel {
            self.pending = dialog.resume;
        } else {
            self.web.save = Some(dialog);
        }
    }

    pub(in crate::app) fn paste_browser_clipboard(&mut self) {
        let events = self.web.events.clone();
        let ctx = self.web.ctx.clone();
        let prefer_text = self.text_edit.is_some();
        wasm_bindgen_futures::spawn_local(async move {
            let result = match crate::web::read_clipboard(prefer_text).await {
                Ok(value) => BrowserResult::Clipboard(value),
                Err(error) => BrowserResult::Message(format!(
                    "Clipboard access: {} Use Ctrl+V / Command+V, Paste from, or Paste options → Paste copied selection.",
                    crate::web::error_message(error)
                )),
            };
            events.borrow_mut().push(result);
            ctx.request_repaint();
        });
    }

    pub(in crate::app) fn copy_browser_image(&self, image: &RgbaImage) {
        let events = self.web.events.clone();
        let ctx = self.web.ctx.clone();
        let bytes = match crate::raster_io::encode(image, RasterFormat::Png) {
            Ok(bytes) => bytes,
            Err(_) => return,
        };
        wasm_bindgen_futures::spawn_local(async move {
            let message = match crate::web::write_clipboard(&bytes).await {
                Ok(()) => "Selection copied".into(),
                Err(_) => "Selection copied within Paint 10. Browser permission is required to paste into other apps.".into(),
            };
            events.borrow_mut().push(BrowserResult::Message(message));
            ctx.request_repaint();
        });
    }

    pub(in crate::app) fn poll_browser(&mut self, ctx: &Context) {
        for bytes in crate::web::take_pasted_images() {
            self.import_browser_bytes(
                "clipboard.png",
                &js_sys::Uint8Array::new(&bytes).to_vec(),
                false,
            );
        }
        let events = std::mem::take(&mut *self.web.events.borrow_mut());
        for event in events {
            match event {
                BrowserResult::Font { name, bytes } => self.import_browser_font(&name, bytes),
                BrowserResult::File { name, bytes, open } => {
                    self.import_browser_bytes(&name, &bytes, open)
                }
                BrowserResult::Message(message) => self.message = message,
                BrowserResult::Clipboard(value) => {
                    if let Some(text) = property(&value, "text").as_string() {
                        if let Some(state) = self.text_edit.as_mut() {
                            if let Err(error) = text_editing::apply_text_clipboard(
                                state,
                                Action::Paste,
                                Some(&text),
                                ctx,
                            ) {
                                self.message = error;
                            }
                        } else {
                            self.message =
                                "Clipboard contains text. Create or edit a text box to paste it."
                                    .into();
                        }
                    } else {
                        self.import_browser_bytes(
                            "clipboard.png",
                            &js_sys::Uint8Array::new(&property(&value, "bytes")).to_vec(),
                            false,
                        );
                    }
                }
            }
        }
    }

    pub(in crate::app) fn sync_browser_unsaved(&self) {
        crate::web::set_unsaved(
            self.doc.dirty()
                || self
                    .text_edit
                    .as_ref()
                    .is_some_and(|edit| !edit.text.is_empty())
                || self.shape_draft.is_some()
                || self.curve.is_some()
                || !self.polygon.is_empty()
                || self.gesture.is_some(),
        );
    }
}

fn property(value: &JsValue, name: &str) -> JsValue {
    js_sys::Reflect::get(value, &name.into()).unwrap_or(JsValue::UNDEFINED)
}
