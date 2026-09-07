mod canvas;
mod chrome;
mod commands;
mod dialogs;
mod file_menu;
mod files;
mod gestures;
mod jobs;
mod keyboard;
mod keytips;
mod properties;
mod ribbon;
mod ribbon_controls;
mod ribbon_layout;
mod selection;
mod shapes;
mod shortcuts;
mod text_editing;
mod text_preview;
mod thumbnail;
mod transforms;

use canvas::dashed_rect;
use gestures::pointer_press_in;
use shapes::{CurveBend, ShapeDraft, ShapeGeometry};

use crate::document::{
    self as d, Brush, Color, Document, Object, ObjectKind, PaintStyle, Point, Region, Tool, BLACK,
    WHITE,
};
use crate::icons::{self, Icon};
use eframe::egui::{self, *};
use image::{imageops, Rgba, RgbaImage};
use std::{borrow::Cow, path::PathBuf};

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
        base: Option<Object>,
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
    rendered: RgbaImage,
    tool: Tool,
    brush: Brush,
    size: u32,
    tool_sizes: [u32; 4],
    colors: [Color; 2],
    active_color: usize,
    custom_colors: Vec<Color>,
    outline: PaintStyle,
    fill: PaintStyle,
    spray_seed: u32,
    zoom: f32,
    grid: bool,
    rulers: bool,
    status_bar: bool,
    view_tab: bool,
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
    clipboard: Option<arboard::Clipboard>,
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
        ctx.set_visuals(Visuals::light());
        let mut style = (*ctx.style()).clone();
        style
            .text_styles
            .insert(TextStyle::Body, FontId::proportional(13.));
        style
            .text_styles
            .insert(TextStyle::Button, FontId::proportional(13.));
        style
            .text_styles
            .insert(TextStyle::Small, FontId::proportional(11.));
        style.spacing.item_spacing = vec2(6., 4.);
        style.spacing.button_padding = vec2(8., 4.);
        style.visuals.widgets.inactive.corner_radius = CornerRadius::ZERO;
        style.visuals.widgets.hovered.corner_radius = CornerRadius::ZERO;
        style.visuals.widgets.active.corner_radius = CornerRadius::ZERO;
        style.visuals.window_corner_radius = CornerRadius::ZERO;
        style.visuals.selection.bg_fill = Color32::from_rgb(204, 232, 255);
        style.visuals.selection.stroke = Stroke::new(1.0_f32, BLUE);
        ctx.set_style(style);
        let doc = Document::new(900, 600);
        let mut font_db = fontdb::Database::new();
        if load_environment {
            font_db.load_system_fonts();
        }
        let mut font_names: Vec<_> = font_db
            .faces()
            .filter(|f| f.style == fontdb::Style::Normal && f.weight == fontdb::Weight::NORMAL)
            .filter_map(|f| f.families.first().map(|n| (n.0.clone(), f.id)))
            .collect();
        font_names.sort_by(|a, b| a.0.cmp(&b.0));
        font_names.dedup_by(|a, b| a.0 == b.0);
        let rendered = doc.image.clone();
        let mut app = Self {
            doc,
            rendered,
            texture: None,
            refresh: true,
            tool: Tool::Brush,
            brush: Brush::Round,
            size: 3,
            tool_sizes: [3, 1, 8, 3],
            colors: [BLACK, WHITE],
            active_color: 0,
            custom_colors: if load_environment {
                crate::preferences::custom_colors()
            } else {
                Vec::new()
            },
            outline: PaintStyle::Solid,
            fill: PaintStyle::None,
            spray_seed: 0,
            zoom: 1.,
            grid: false,
            rulers: false,
            status_bar: true,
            view_tab: false,
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
            message: "For Help, click ? or press F1".into(),
            cursor: None,
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
        self.modal_raw_input(ctx, raw_input);
    }

    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        self.poll_job();
        self.page
            .set_resolution(self.doc.resolution.x, self.doc.resolution.y);
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
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
        if self.print_preview.is_none() && !self.preview {
            if !self.ribbon_keyboard(ctx) {
                self.shortcut(ctx);
            }
            if self.dialog.is_none() && self.pending.is_none() {
                for file in ctx.input(|i| i.raw.dropped_files.clone()) {
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
            self.canvas(ctx);
            self.thumbnail(ctx);
        }
        self.keyboard_menu(ctx);
        self.dialogs(ctx);
        if self.refresh && self.print_preview.is_none() {
            ctx.request_repaint();
        }
    }
}
