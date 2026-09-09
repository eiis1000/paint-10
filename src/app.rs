mod canvas;
mod chrome;
mod color_editor;
mod commands;
mod dialogs;
mod file_menu;
#[cfg_attr(target_arch = "wasm32", path = "app/web_files.rs")]
mod files;
mod gestures;
mod image_ribbon;
mod jobs;
mod keyboard;
mod keytips;
mod latex_editing;
#[cfg(test)]
mod layer_workflow_tests;
mod layers;
mod measure;
mod properties;
mod ribbon;
mod ribbon_controls;
mod ribbon_layout;
mod selection;
mod shapes;
mod shortcuts;
mod text_editing;
mod text_preview;
mod theme;
mod thumbnail;
mod transforms;

use canvas::dashed_rect;
use gestures::pointer_press_in;
use shapes::{CurveBend, ShapeDraft, ShapeGeometry};

use crate::display;
use crate::document::{
    self as d, Brush, Color, Document, Gradient, Object, ObjectKind, PaintStyle, Point, Region,
    ShapeFill, Tool, BLACK, WHITE,
};
use crate::icons::{self, Icon};
use eframe::egui::{self, *};
use image::{imageops, Rgba, RgbaImage};
#[cfg(not(target_arch = "wasm32"))]
use std::borrow::Cow;
use std::path::PathBuf;

const RIBBON: Color32 = Color32::from_rgb(245, 246, 247);
const BLUE: Color32 = Color32::from_rgb(25, 121, 202);
const MIN_ZOOM: f32 = 0.125;
const MAX_ZOOM: f32 = 32.0;

#[derive(Clone, Copy)]
enum Action {
    New,
    Open,
    Save,
    SaveAs,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    PasteFrom,
    Crop,
    Resize,
    Rotate(f32),
    Flip(bool),
    Properties,
    Clear,
    ClearPicture,
    SelectAll,
    Invert,
    Print,
    Close,
}
#[derive(Clone, Copy, PartialEq)]
enum Dialog {
    Resize,
    Rotate,
    Colors,
    ImageAdjustments,
    ImageCrop,
    Latex,
    Properties,
    About,
    Print,
    Import,
    Wallpaper,
}
enum JobResult {
    Devices(Result<crate::integration::DeviceList, String>),
    Image(Result<RgbaImage, String>),
    Status(Result<String, String>),
}
struct TextEditState {
    index: Option<usize>,
    origin: Point,
    text: String,
    format: crate::text::TextFormat,
    focus: bool,
    selection: std::ops::Range<usize>,
    insertion_style: Option<crate::text::TextStyle>,
    history: text_editing::TextHistory,
    palette_colors: [Color; 2],
}
enum Gesture {
    Paint {
        start: Point,
        last: Point,
        color: Color,
        erase_target: Option<Color>,
        first: bool,
    },
    Select {
        start: Point,
        points: Vec<Point>,
        click_candidate: Option<usize>,
    },
    Move {
        start: Point,
        origin: Point,
        index: usize,
        last_stamp: Point,
    },
    CanvasSize {
        start: Point,
        original: (u32, u32),
        axis: u8,
    },
    TextBox {
        start: Point,
    },
    MoveText {
        start: Point,
        origin: Point,
    },
    ResizeText {
        start: Point,
        origin: Point,
        width: u32,
        height: u32,
        minimum_height: u32,
        handle: usize,
    },
    ResizeObject {
        index: Option<usize>,
        original: Region,
        start: Point,
        base: Option<Box<Object>>,
        handle: usize,
    },
    MoveShape {
        start: Point,
        original: ShapeDraft,
    },
    ResizeShape {
        bounds: Region,
        original: ShapeDraft,
        start: Point,
        handle: usize,
    },
    LineEndpoint {
        start: Point,
        original: ShapeDraft,
        endpoint: usize,
    },
}

pub struct PaintApp {
    doc: Document,
    texture: Option<TextureHandle>,
    refresh: bool,
    render_revision: u64,
    rendered: RgbaImage,
    tool: Tool,
    brush: Brush,
    size: u32,
    tool_sizes: [u32; 4],
    colors: [Color; 2],
    active_color: usize,
    custom_colors: Vec<Color>,
    recent_custom_colors: Vec<Color>,
    persist_preferences: bool,
    outline: PaintStyle,
    fill: PaintStyle,
    fill_gradient: Option<Gradient>,
    spray_seed: u32,
    zoom: f32,
    grid: bool,
    rulers: bool,
    measure: measure::Measurement,
    status_bar: bool,
    layer_ui: layers::LayersState,
    view_tab: bool,
    image_tab: bool,
    text_tab: bool,
    collapsed: bool,
    selection: Option<Region>,
    free_select: bool,
    free_points: Vec<Point>,
    transparent: bool,
    object: Option<usize>,
    gesture: Option<Gesture>,
    curve: Option<CurveBend>,
    shape_draft: Option<ShapeDraft>,
    polygon: Vec<Point>,
    polygon_color: Color,
    polygon_color_slot: usize,
    file: Option<PathBuf>,
    message: String,
    cursor: Option<Point>,
    #[cfg(not(target_arch = "wasm32"))]
    clipboard: Option<arboard::Clipboard>,
    #[cfg(target_arch = "wasm32")]
    web: files::BrowserState,
    copied: Option<RgbaImage>,
    mask: Option<image::GrayImage>,
    recent: Vec<PathBuf>,
    quick_access: crate::preferences::QuickAccess,
    dialog: Option<Dialog>,
    dialog_error: Option<String>,
    pending: Option<Action>,
    pending_path: Option<PathBuf>,
    allow_close: bool,
    text_edit: Option<TextEditState>,
    latex_edit: Option<latex_editing::LatexDraft>,
    resize_w: u32,
    resize_h: u32,
    percent: bool,
    aspect: bool,
    pixel_resize: bool,
    skew_x: f32,
    skew_y: f32,
    angle: f32,
    hex: String,
    unit: u8,
    prop_mono: bool,
    canvas_rect: Rect,
    canvas_alpha: bool,
    fullscreen: bool,
    keyboard_context_menu: bool,
    preview: bool,
    page: crate::printing::PageSettings,
    font_db: fontdb::Database,
    font_names: Vec<(String, fontdb::ID)>,
    text_format: crate::text::TextFormat,
    previous_tool: Tool,
    print_preview: Option<crate::print_preview::PrintPreview>,
    job: Option<std::sync::mpsc::Receiver<JobResult>>,
    job_cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    devices: crate::integration::DeviceList,
    device_index: usize,
    capture_settings: crate::integration::CaptureSettings,
    wallpaper_style: crate::integration::WallpaperStyle,
    wallpaper_size: (u32, u32),
}

impl PaintApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::new_with_context(&cc.egui_ctx, true)
    }

    fn new_with_context(ctx: &Context, load_environment: bool) -> Self {
        theme::install(ctx);
        let doc = Document::new(900, 600);
        let mut font_db = fontdb::Database::new();
        #[cfg(not(target_arch = "wasm32"))]
        if load_environment {
            font_db.load_system_fonts();
        }
        font_db.load_font_data(crate::text::DEFAULT_FONT.to_vec());
        #[cfg(target_arch = "wasm32")]
        {
            font_db.load_font_data(epaint_default_fonts::UBUNTU_LIGHT.to_vec());
            font_db.load_font_data(epaint_default_fonts::HACK_REGULAR.to_vec());
            font_db.load_font_data(epaint_default_fonts::NOTO_EMOJI_REGULAR.to_vec());
        }
        let font_names = text_editing::font_families(&font_db);
        let rendered = doc.image.clone();
        let (custom_colors, recent_custom_colors) = if load_environment {
            crate::preferences::custom_palette()
        } else {
            (
                vec![WHITE; crate::preferences::CUSTOM_COLOR_COUNT],
                Vec::new(),
            )
        };
        let app = Self {
            doc,
            rendered,
            texture: None,
            refresh: true,
            render_revision: 0,
            tool: Tool::Brush,
            brush: Brush::Round,
            size: 3,
            tool_sizes: [3, 1, 8, 3],
            colors: [BLACK, WHITE],
            active_color: 0,
            custom_colors,
            recent_custom_colors,
            persist_preferences: load_environment,
            outline: PaintStyle::Solid,
            fill: PaintStyle::None,
            fill_gradient: None,
            spray_seed: 0,
            zoom: 1.,
            grid: false,
            rulers: false,
            measure: Default::default(),
            status_bar: true,
            layer_ui: Default::default(),
            view_tab: false,
            image_tab: false,
            text_tab: false,
            collapsed: false,
            selection: None,
            free_select: false,
            free_points: vec![],
            transparent: false,
            object: None,
            gesture: None,
            curve: None,
            shape_draft: None,
            polygon: vec![],
            polygon_color: BLACK,
            polygon_color_slot: 0,
            file: None,
            message: String::new(),
            cursor: None,
            #[cfg(target_arch = "wasm32")]
            web: files::BrowserState::new(ctx.clone()),
            #[cfg(not(target_arch = "wasm32"))]
            clipboard: if load_environment {
                arboard::Clipboard::new().ok()
            } else {
                None
            },
            copied: None,
            mask: None,
            recent: if load_environment {
                crate::preferences::recent_files()
            } else {
                Vec::new()
            },
            dialog: None,
            quick_access: if load_environment {
                crate::preferences::quick_access()
            } else {
                Default::default()
            },
            dialog_error: None,
            pending: None,
            pending_path: None,
            allow_close: false,
            text_edit: None,
            latex_edit: None,
            resize_w: 900,
            resize_h: 600,
            percent: false,
            aspect: true,
            pixel_resize: false,
            skew_x: 0.,
            skew_y: 0.,
            angle: 0.,
            hex: "000000".into(),
            unit: 0,
            prop_mono: false,
            canvas_rect: Rect::NOTHING,
            canvas_alpha: false,
            fullscreen: false,
            keyboard_context_menu: false,
            preview: false,
            page: Default::default(),
            font_db,
            font_names,
            text_format: Default::default(),
            previous_tool: Tool::Brush,
            print_preview: None,
            job: None,
            job_cancel: Default::default(),
            devices: Default::default(),
            device_index: 0,
            capture_settings: Default::default(),
            wallpaper_style: Default::default(),
            wallpaper_size: (1920, 1080),
        };
        #[cfg(not(target_arch = "wasm32"))]
        let mut app = app;
        #[cfg(not(target_arch = "wasm32"))]
        if load_environment {
            if let Some(path) = std::env::args_os().nth(1) {
                app.load(PathBuf::from(path));
            }
        }
        app
    }
}

impl eframe::App for PaintApp {
    fn raw_input_hook(&mut self, ctx: &Context, raw_input: &mut RawInput) {
        self.canvas_raw_input(ctx, raw_input);
        self.text_raw_input(ctx, raw_input);
        self.modal_raw_input(ctx, raw_input);
        if self.dialog.is_none()
            && self.pending.is_none()
            && self.print_preview.is_none()
            && !self.preview
            && !self.browser_dialog_open()
        {
            keytips::raw_input(ctx, raw_input);
        }
    }

    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        #[cfg(target_arch = "wasm32")]
        self.poll_browser(ctx);
        self.poll_job();
        self.page
            .set_resolution(self.doc.resolution.x, self.doc.resolution.y);
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            if self.latex_edit.is_some() {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                self.dialog_error =
                    Some("Apply or cancel the equation before closing Paint 10.".into());
            } else {
                self.commit_text();
                self.finish_polygon();
                self.commit_shape();
                if self.curve.take().is_some() {
                    self.doc.commit();
                }
                if self.doc.dirty() {
                    ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                    self.pending = Some(Action::Close);
                }
            }
        }
        if self.print_preview.is_none() && !self.preview {
            if !self.browser_dialog_open() && !self.ribbon_keyboard(ctx) {
                self.shortcut(ctx);
            }
            if self.dialog.is_none() && self.pending.is_none() {
                for file in ctx.input(|i| i.raw.dropped_files.clone()) {
                    #[cfg(target_arch = "wasm32")]
                    if let Some(bytes) = file.bytes {
                        self.import_browser_bytes(&file.name, &bytes, false);
                    }
                    if let Some(path) = file.path {
                        match Self::read_image(&path) {
                            Ok(img) => self.insert_image(img),
                            Err(e) => self.message = e,
                        }
                    }
                }
            }
        } else if self.dialog.is_none() && self.pending.is_none() {
            if ctx.input_mut(|input| input.consume_key(Modifiers::CTRL, Key::P)) {
                self.dialog = Some(Dialog::Print);
            }
            if self.preview
                && ctx.input_mut(|input| {
                    input.consume_key(Modifiers::NONE, Key::Escape)
                        || input.consume_key(Modifiers::NONE, Key::F11)
                })
            {
                self.hide_picture(ctx);
            }
        }
        let preview_interactive = self.dialog.is_none() && self.pending.is_none();
        if let Some(preview) = &mut self.print_preview {
            let mut action = None;
            CentralPanel::default()
                .frame(Frame::NONE.fill(RIBBON))
                .show(ctx, |ui| {
                    ui.add_enabled_ui(preview_interactive, |ui| {
                        action = preview.show(ui, &self.page);
                    });
                });
            match action {
                Some(crate::print_preview::PreviewAction::Close) => self.print_preview = None,
                Some(
                    crate::print_preview::PreviewAction::PageSetup
                    | crate::print_preview::PreviewAction::Print,
                ) => self.dialog = Some(Dialog::Print),
                None => {}
            }
        } else if self.preview {
            self.refresh_texture(ctx);
            CentralPanel::default()
                .frame(Frame::NONE.fill(Color32::from_gray(45)))
                .show(ctx, |ui| {
                    if ui.button("Back to Paint 10 (Esc)").clicked() {
                        self.hide_picture(ctx);
                    }
                    let available = ui.available_size();
                    let image_size = vec2(
                        self.doc.image.width() as f32,
                        self.doc.image.height() as f32,
                    );
                    let scale = (available.x / image_size.x)
                        .min(available.y / image_size.y)
                        .min(1.);
                    ui.centered_and_justified(|ui| {
                        ui.image((self.texture.as_ref().unwrap().id(), image_size * scale));
                    });
                });
        } else {
            self.titlebar(ctx);
            self.ribbon(ctx);
            self.quick_access_below(ctx);
            self.status(ctx);
            self.layers_panel(ctx);
            self.canvas(ctx);
            self.thumbnail(ctx);
        }
        self.keyboard_menu(ctx);
        self.dialogs(ctx);
        #[cfg(target_arch = "wasm32")]
        {
            self.browser_save_dialog(ctx);
            self.sync_browser_unsaved();
        }
        if self.refresh && self.print_preview.is_none() {
            ctx.request_repaint();
        }
    }
}

impl PaintApp {
    fn browser_dialog_open(&self) -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            self.web.save.is_some()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            false
        }
    }
}
