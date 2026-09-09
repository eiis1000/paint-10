//! Reversible image adjustments and source-pixel cropping.

use super::*;
use crate::document::{ImageEdits, ImageSampling};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum CropAspect {
    #[default]
    Free,
    Original,
    Square,
    Photo,
    Standard,
    Wide,
    Portrait,
}

impl CropAspect {
    const ALL: [Self; 7] = [
        Self::Free,
        Self::Original,
        Self::Square,
        Self::Photo,
        Self::Standard,
        Self::Wide,
        Self::Portrait,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Original => "Original ratio",
            Self::Square => "Square (1:1)",
            Self::Photo => "Photo (3:2)",
            Self::Standard => "Standard (4:3)",
            Self::Wide => "Widescreen (16:9)",
            Self::Portrait => "Portrait (9:16)",
        }
    }

    fn ratio(self, dimensions: (u32, u32)) -> Option<f64> {
        match self {
            Self::Free => None,
            Self::Original => Some(dimensions.0 as f64 / dimensions.1 as f64),
            Self::Square => Some(1.0),
            Self::Photo => Some(1.5),
            Self::Standard => Some(4.0 / 3.0),
            Self::Wide => Some(16.0 / 9.0),
            Self::Portrait => Some(9.0 / 16.0),
        }
    }
}

#[derive(Clone, Copy)]
enum CropDrag {
    Corner((u32, u32)),
    Move { start: (u32, u32), region: Region },
}

#[derive(Clone)]
struct ImageDialog {
    source: std::sync::Arc<Object>,
    original: ImageEdits,
    edits: ImageEdits,
    dimensions: (u32, u32),
    aspect: CropAspect,
    crop_drag: Option<CropDrag>,
    show_before: bool,
    preview: Option<TextureHandle>,
    preview_edits: Option<ImageEdits>,
    error: Option<String>,
}

fn state_key() -> Id {
    Id::new("paint10-image-dialog")
}

pub(super) fn clear(ctx: &Context) {
    ctx.data_mut(|data| data.remove::<ImageDialog>(state_key()));
}

impl ImageDialog {
    fn load(app: &PaintApp, ctx: &Context) -> Option<Self> {
        if let Some(state) = ctx.data(|data| data.get_temp::<Self>(state_key())) {
            return Some(state);
        }
        let (edits, dimensions) = app.image_edit_target()?;
        Some(Self {
            source: std::sync::Arc::new(app.image_edit_object().ok()?),
            original: edits,
            edits,
            dimensions,
            aspect: CropAspect::Free,
            crop_drag: None,
            show_before: false,
            preview: None,
            preview_edits: None,
            error: None,
        })
    }

    fn crop(&self) -> Region {
        self.edits.crop.unwrap_or(Region {
            x: 0,
            y: 0,
            w: self.dimensions.0,
            h: self.dimensions.1,
        })
    }

    fn refresh_preview(&mut self, ctx: &Context, crop: bool) {
        let mut edits = if self.show_before {
            self.original
        } else {
            self.edits
        };
        if crop {
            edits.crop = None;
        }
        if self.preview_edits == Some(edits) {
            return;
        }
        self.preview_edits = Some(edits);
        match self.source.preview_image_edits(edits, 360) {
            Ok(image) => {
                let image = crate::display::image(
                    [image.width() as usize, image.height() as usize],
                    image.as_raw(),
                );
                let filtering = if edits.sampling == ImageSampling::Nearest {
                    TextureOptions::NEAREST
                } else {
                    TextureOptions::LINEAR
                };
                if let Some(texture) = &mut self.preview {
                    texture.set(image, filtering);
                } else {
                    self.preview = Some(ctx.load_texture("Image preview", image, filtering));
                }
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn preview(&mut self, ui: &mut Ui, crop: bool, height: f32) {
        self.refresh_preview(ui.ctx(), crop);
        let available = vec2(ui.available_width(), height);
        let (panel, _) = ui.allocate_exact_size(available, Sense::hover());
        ui.painter()
            .rect_filled(panel, 0.0, Color32::from_gray(234));
        let Some(texture) = &self.preview else {
            return;
        };
        let dimensions = texture.size_vec2();
        let size = dimensions
            * ((available.x - 16.0) / dimensions.x).min((available.y - 16.0) / dimensions.y);
        let rect = Rect::from_center_size(panel.center(), size);
        let painter = ui.painter();
        for row in 0..(size.y / 10.0).ceil() as i32 {
            for column in 0..(size.x / 10.0).ceil() as i32 {
                let square = Rect::from_min_size(
                    rect.min + vec2(column as f32 * 10.0, row as f32 * 10.0),
                    vec2(10.0, 10.0),
                )
                .intersect(rect);
                painter.rect_filled(
                    square,
                    0.0,
                    Color32::from_gray(if (row + column) % 2 == 0 { 250 } else { 218 }),
                );
            }
        }
        painter.image(
            texture.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        if crop {
            self.crop_preview(ui, rect);
        }
        ui.painter().rect_stroke(
            panel,
            0.0,
            Stroke::new(1.0_f32, Color32::from_gray(190)),
            StrokeKind::Inside,
        );
    }

    fn crop_preview(&mut self, ui: &mut Ui, rect: Rect) {
        let crop = self.crop();
        let position = |x: u32, y: u32| {
            rect.min
                + vec2(
                    x as f32 / self.dimensions.0 as f32 * rect.width(),
                    y as f32 / self.dimensions.1 as f32 * rect.height(),
                )
        };
        let bounds = Rect::from_min_max(
            position(crop.x, crop.y),
            position(crop.x + crop.w, crop.y + crop.h),
        );
        let response = ui.interact(
            rect.expand(7.0).intersect(ui.clip_rect()),
            ui.id().with("source_crop"),
            Sense::drag(),
        );
        let to_source = |point: Pos2| {
            let fraction = (point - rect.min) / rect.size();
            (
                (fraction.x.clamp(0.0, 1.0) * self.dimensions.0 as f32).round() as u32,
                (fraction.y.clamp(0.0, 1.0) * self.dimensions.1 as f32).round() as u32,
            )
        };
        if response.drag_started() {
            if let Some(pointer) = ui.input(|input| input.pointer.press_origin()) {
                let handles = [
                    (bounds.left_top(), (crop.x + crop.w, crop.y + crop.h)),
                    (bounds.right_top(), (crop.x, crop.y + crop.h)),
                    (bounds.left_bottom(), (crop.x + crop.w, crop.y)),
                    (bounds.right_bottom(), (crop.x, crop.y)),
                ];
                self.crop_drag = Some(
                    if let Some((_, anchor)) = handles
                        .iter()
                        .find(|(corner, _)| corner.distance(pointer) <= 9.0)
                    {
                        CropDrag::Corner(*anchor)
                    } else if bounds.contains(pointer) {
                        CropDrag::Move {
                            start: to_source(pointer),
                            region: crop,
                        }
                    } else {
                        CropDrag::Corner(to_source(pointer))
                    },
                );
            }
        }
        if response.dragged() {
            if let (Some(drag), Some(pointer)) = (self.crop_drag, response.interact_pointer_pos()) {
                let point = to_source(pointer);
                self.edits.crop = Some(match drag {
                    CropDrag::Corner(anchor) => crop_between(
                        anchor,
                        point,
                        self.dimensions,
                        self.aspect.ratio(self.dimensions),
                    ),
                    CropDrag::Move { start, mut region } => {
                        region.x = (region.x as i64 + point.0 as i64 - start.0 as i64)
                            .clamp(0, (self.dimensions.0 - region.w) as i64)
                            as u32;
                        region.y = (region.y as i64 + point.1 as i64 - start.1 as i64)
                            .clamp(0, (self.dimensions.1 - region.h) as i64)
                            as u32;
                        region
                    }
                });
            }
        }
        if response.drag_stopped() {
            self.crop_drag = None;
        }
        response.on_hover_cursor(CursorIcon::Crosshair);
        let painter = ui.painter();
        for outside in [
            Rect::from_min_max(rect.min, pos2(rect.right(), bounds.top())),
            Rect::from_min_max(pos2(rect.left(), bounds.bottom()), rect.max),
            Rect::from_min_max(pos2(rect.left(), bounds.top()), bounds.left_bottom()),
            Rect::from_min_max(bounds.right_top(), pos2(rect.right(), bounds.bottom())),
        ] {
            painter.rect_filled(outside, 0.0, Color32::from_black_alpha(120));
        }
        painter.rect_stroke(bounds, 0.0, Stroke::new(1.5_f32, BLUE), StrokeKind::Inside);
        for fraction in [1.0 / 3.0, 2.0 / 3.0] {
            painter.vline(
                bounds.left() + bounds.width() * fraction,
                bounds.y_range(),
                Stroke::new(1.0_f32, Color32::from_black_alpha(95)),
            );
            painter.hline(
                bounds.x_range(),
                bounds.top() + bounds.height() * fraction,
                Stroke::new(1.0_f32, Color32::from_black_alpha(95)),
            );
        }
        for corner in [
            bounds.left_top(),
            bounds.right_top(),
            bounds.left_bottom(),
            bounds.right_bottom(),
        ] {
            painter.rect(
                Rect::from_center_size(corner, Vec2::splat(7.0)),
                0.0,
                Color32::WHITE,
                Stroke::new(1.0_f32, BLUE),
                StrokeKind::Inside,
            );
        }
    }
}

fn crop_between(
    a: (u32, u32),
    b: (u32, u32),
    dimensions: (u32, u32),
    ratio: Option<f64>,
) -> Region {
    let x = a.0.min(b.0).min(dimensions.0 - 1);
    let y = a.1.min(b.1).min(dimensions.1 - 1);
    let mut width = a.0.abs_diff(b.0).max(1).min(dimensions.0 - x);
    let mut height = a.1.abs_diff(b.1).max(1).min(dimensions.1 - y);
    if let Some(ratio) = ratio {
        if width as f64 / height as f64 > ratio {
            width = (height as f64 * ratio).round().max(1.0) as u32;
        } else {
            height = (width as f64 / ratio).round().max(1.0) as u32;
        }
    }
    Region {
        x: if b.0 < a.0 {
            a.0.min(dimensions.0).saturating_sub(width)
        } else {
            x
        },
        y: if b.1 < a.1 {
            a.1.min(dimensions.1).saturating_sub(height)
        } else {
            y
        },
        w: width,
        h: height,
    }
}

fn adjustment(
    ui: &mut Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    speed: f64,
) {
    let label = ui
        .allocate_ui_with_layout(
            vec2(72.0, 22.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_width(72.0);
                ui.label(label)
            },
        )
        .inner;
    ui.add(Slider::new(value, range.clone()).show_value(false))
        .labelled_by(label.id);
    let number = ui
        .allocate_ui_with_layout(
            vec2(72.0, 22.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_width(72.0);
                ui.spacing_mut().interact_size = vec2(56.0, 22.0);
                numeric_input(
                    ui,
                    DragValue::new(value)
                        .range(range)
                        .speed(speed)
                        .max_decimals(if speed < 1.0 { 2 } else { 0 }),
                )
            },
        )
        .inner;
    number.clone().labelled_by(label.id);
    initial_focus(ui, &number);
    ui.end_row();
}

impl PaintApp {
    pub(super) fn image_adjustments_dialog(&mut self, ui: &mut Ui) -> bool {
        let Some(mut state) = ImageDialog::load(self, ui.ctx()) else {
            ui.label("Select an image or a part of the picture first.");
            return default_button(ui, "Close", true);
        };
        let width = 620.0_f32.min((ui.ctx().screen_rect().width() - 64.0).max(320.0));
        ui.set_width(width);
        let controls = |ui: &mut Ui, state: &mut ImageDialog| {
            ui.strong("Light and color");
            Grid::new("image_adjustments")
                .num_columns(3)
                .spacing(vec2(10.0, 7.0))
                .show(ui, |ui| {
                    ui.spacing_mut().slider_width = 125.0;
                    adjustment(
                        ui,
                        "Brightness",
                        &mut state.edits.brightness,
                        -100.0..=100.0,
                        1.0,
                    );
                    adjustment(
                        ui,
                        "Contrast",
                        &mut state.edits.contrast,
                        -100.0..=100.0,
                        1.0,
                    );
                    adjustment(
                        ui,
                        "Saturation",
                        &mut state.edits.saturation,
                        -100.0..=100.0,
                        1.0,
                    );
                    adjustment(
                        ui,
                        "Warmth",
                        &mut state.edits.temperature,
                        -100.0..=100.0,
                        1.0,
                    );
                    adjustment(ui, "Hue", &mut state.edits.hue, -180.0..=180.0, 1.0);
                    adjustment(ui, "Gamma", &mut state.edits.gamma, 0.1..=5.0, 0.01);
                    adjustment(ui, "Opacity", &mut state.edits.opacity, 0.0..=100.0, 1.0);
                });
            ui.horizontal(|ui| {
                ui.checkbox(&mut state.edits.grayscale, "Grayscale");
                ui.checkbox(&mut state.edits.invert, "Invert colors");
            });
            ui.add_space(8.0);
            ui.strong("Detail");
            Grid::new("image_detail")
                .num_columns(3)
                .spacing(vec2(10.0, 7.0))
                .show(ui, |ui| {
                    ui.spacing_mut().slider_width = 125.0;
                    adjustment(ui, "Blur", &mut state.edits.blur, 0.0..=10.0, 0.1);
                    adjustment(ui, "Sharpen", &mut state.edits.sharpen, 0.0..=5.0, 0.1);
                });
        };
        if width >= 580.0 {
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    vec2(300.0, 320.0),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_width(300.0);
                        controls(ui, &mut state);
                    },
                );
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.set_width(width - 322.0);
                    ui.strong("Preview");
                    state.preview(ui, false, 240.0);
                    ui.checkbox(&mut state.show_before, "Show before changes");
                    ui.label(
                        RichText::new(format!(
                            "Original: {} × {} px",
                            state.dimensions.0, state.dimensions.1
                        ))
                        .small()
                        .weak(),
                    );
                });
            });
        } else {
            state.preview(ui, false, 150.0);
            ui.checkbox(&mut state.show_before, "Show before changes");
            ui.add_space(8.0);
            controls(ui, &mut state);
        }
        ui.add_space(8.0);
        if dialog_button(ui, "Reset adjustments") {
            state.edits = ImageEdits {
                crop: state.edits.crop,
                sampling: state.edits.sampling,
                matte: state.edits.matte,
                ..Default::default()
            };
        }
        self.finish_image_dialog(ui, state)
    }

    pub(super) fn image_crop_dialog(&mut self, ui: &mut Ui) -> bool {
        let Some(mut state) = ImageDialog::load(self, ui.ctx()) else {
            ui.label("Select an image or a part of the picture first.");
            return default_button(ui, "Close", true);
        };
        ui.set_width(430.0_f32.min((ui.ctx().screen_rect().width() - 64.0).max(320.0)));
        state.preview(
            ui,
            true,
            if ui.ctx().screen_rect().height() < 600.0 {
                160.0
            } else {
                250.0
            },
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Aspect ratio:");
            let previous = state.aspect;
            let combo = ComboBox::from_id_salt("crop_aspect")
                .selected_text(state.aspect.name())
                .show_ui(ui, |ui| {
                    theme::menu(ui);
                    for aspect in CropAspect::ALL {
                        if ui
                            .add(
                                theme::MenuItem::new(aspect.name())
                                    .selected(state.aspect == aspect),
                            )
                            .clicked()
                        {
                            state.aspect = aspect;
                        }
                    }
                });
            register_button(ui, &combo.response);
            if previous != state.aspect {
                let current = state.crop();
                let mut crop = crop_between(
                    (current.x, current.y),
                    (current.x + current.w, current.y + current.h),
                    state.dimensions,
                    state.aspect.ratio(state.dimensions),
                );
                crop.x += (current.w - crop.w) / 2;
                crop.y += (current.h - crop.h) / 2;
                state.edits.crop = Some(crop);
            }
        });
        let mut crop = state.crop();
        Grid::new("crop_coordinates")
            .num_columns(4)
            .spacing(vec2(14.0, 8.0))
            .show(ui, |ui| {
                ui.label("Left:");
                let x = numeric_input(
                    ui,
                    DragValue::new(&mut crop.x)
                        .range(0..=state.dimensions.0 - 1)
                        .suffix(" px"),
                );
                initial_focus(ui, &x);
                ui.label("Top:");
                let y = numeric_input(
                    ui,
                    DragValue::new(&mut crop.y)
                        .range(0..=state.dimensions.1 - 1)
                        .suffix(" px"),
                );
                if x.changed() || y.changed() {
                    // Moving the origin is useful even before the full-image
                    // crop has been narrowed. Fit its size to the space left.
                    crop = crop_between(
                        (crop.x, crop.y),
                        (crop.x + crop.w, crop.y + crop.h),
                        state.dimensions,
                        state.aspect.ratio(state.dimensions),
                    );
                }
                ui.end_row();
                ui.label("Width:");
                if numeric_input(
                    ui,
                    DragValue::new(&mut crop.w)
                        .range(1..=state.dimensions.0 - crop.x)
                        .suffix(" px"),
                )
                .changed()
                {
                    if let Some(ratio) = state.aspect.ratio(state.dimensions) {
                        crop.h = (crop.w as f64 / ratio)
                            .round()
                            .clamp(1.0, (state.dimensions.1 - crop.y) as f64)
                            as u32;
                        crop.w = (crop.h as f64 * ratio)
                            .round()
                            .clamp(1.0, (state.dimensions.0 - crop.x) as f64)
                            as u32;
                    }
                }
                ui.label("Height:");
                if numeric_input(
                    ui,
                    DragValue::new(&mut crop.h)
                        .range(1..=state.dimensions.1 - crop.y)
                        .suffix(" px"),
                )
                .changed()
                {
                    if let Some(ratio) = state.aspect.ratio(state.dimensions) {
                        crop.w = (crop.h as f64 * ratio)
                            .round()
                            .clamp(1.0, (state.dimensions.0 - crop.x) as f64)
                            as u32;
                        crop.h = (crop.w as f64 / ratio)
                            .round()
                            .clamp(1.0, (state.dimensions.1 - crop.y) as f64)
                            as u32;
                    }
                }
                ui.end_row();
            });
        if crop != state.crop() {
            state.edits.crop = Some(crop);
        }
        ui.label(
            RichText::new(format!(
                "Original: {} × {} pixels",
                state.dimensions.0, state.dimensions.1
            ))
            .small()
            .weak(),
        );
        if dialog_button(ui, "Restore full image") {
            state.edits.crop = None;
            state.aspect = CropAspect::Free;
        }
        self.finish_image_dialog(ui, state)
    }

    fn finish_image_dialog(&mut self, ui: &mut Ui, state: ImageDialog) -> bool {
        let changed = ui.ctx().data(|data| {
            data.get_temp::<ImageDialog>(state_key())
                .is_none_or(|previous| {
                    previous.edits != state.edits || previous.show_before != state.show_before
                })
        });
        if changed {
            ui.ctx().request_repaint();
        }
        if let Some(error) = &state.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        let mut close = false;
        dialog_actions(ui, &["OK", "Cancel"], |ui| {
            if default_button(ui, "OK", state.error.is_none()) {
                match self.apply_image_edits(state.edits) {
                    Ok(()) => close = true,
                    Err(error) => self.dialog_error = Some(error),
                }
            }
            close |= dialog_button(ui, "Cancel");
        });
        ui.ctx()
            .data_mut(|data| data.insert_temp(state_key(), state));
        close
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(app: &mut PaintApp, ctx: &Context, events: Vec<Event>) -> FullOutput {
        let mut input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 760.0))),
            time: Some(ctx.cumulative_pass_nr() as f64 / 30.0),
            events,
            ..Default::default()
        };
        eframe::App::raw_input_hook(app, ctx, &mut input);
        ctx.run(input, |ctx| {
            CentralPanel::default().show(ctx, |_| {});
            app.dialogs(ctx);
        })
    }

    fn key(key: Key) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn print_dialog_action_row_settles_without_expanding_each_frame() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.dialog = Some(Dialog::Print);
        let mut positions = Vec::new();
        for pass in 0..24 {
            let output = frame(&mut app, &ctx, Vec::new());
            if pass >= 16 {
                let position = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.job.text == "Close" => {
                            Some(text.pos)
                        }
                        _ => None,
                    })
                    .expect("print footer remains visible");
                positions.push(position);
            }
        }
        assert!(
            positions
                .windows(2)
                .all(|pair| pair[0].distance(pair[1]) < 0.1),
            "footer moved after settling: {positions:?}"
        );
    }

    #[test]
    fn normal_height_image_dialogs_show_their_footer_without_scrolling() {
        for (dialog, name) in [
            (Dialog::ImageAdjustments, "Edit Image"),
            (Dialog::ImageCrop, "Crop Image"),
        ] {
            let ctx = Context::default();
            let mut app = PaintApp::new_with_context(&ctx, false);
            app.dialog = Some(dialog);
            let mut output = FullOutput::default();
            for _ in 0..12 {
                output = frame(&mut app, &ctx, Vec::new());
            }
            for label in ["OK", "Cancel"] {
                let visible = output.shapes.iter().any(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == label => shape
                        .clip_rect
                        .contains_rect(Rect::from_min_size(text.pos, text.galley.size())),
                    _ => false,
                });
                assert!(visible, "{name} clips its {label} action");
            }
        }
    }

    #[test]
    fn adjustments_preview_cancel_and_apply_keep_the_original_and_undo() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.doc = Document::new(80, 40);
        let source = RgbaImage::from_pixel(32, 16, Rgba([80, 120, 160, 255]));
        let index = app
            .doc
            .add_object(Object::new(ObjectKind::Image(source.clone()), (4, 3)));
        app.select_object(index);
        let original = app.doc.objects[index].clone();

        app.dialog = Some(Dialog::ImageAdjustments);
        for _ in 0..6 {
            frame(&mut app, &ctx, Vec::new());
        }
        frame(&mut app, &ctx, vec![Event::Text("40".into())]);
        let state = ctx
            .data(|data| data.get_temp::<ImageDialog>(state_key()))
            .unwrap();
        assert_eq!(state.edits.brightness, 40.0);
        assert!(
            app.doc.objects[index] == original,
            "preview must not change the document"
        );
        frame(&mut app, &ctx, vec![key(Key::Escape)]);
        assert!(app.dialog.is_none());
        assert!(app.doc.objects[index] == original);
        assert!(ctx
            .data(|data| data.get_temp::<ImageDialog>(state_key()))
            .is_none());

        app.dialog = Some(Dialog::ImageAdjustments);
        for _ in 0..6 {
            frame(&mut app, &ctx, Vec::new());
        }
        frame(&mut app, &ctx, vec![Event::Text("30".into())]);
        frame(&mut app, &ctx, vec![key(Key::Enter)]);
        assert!(app.dialog.is_none());
        assert_eq!(app.doc.objects[index].image_edits.brightness, 30.0);
        assert!(app.doc.objects[index].kind == ObjectKind::Image(source));
        app.doc.undo();
        assert!(app.doc.objects[index] == original);
    }

    #[test]
    fn opening_crop_accepts_left_and_top_before_changing_its_size() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.doc = Document::new(900, 600);
        let original = app.doc.composite();
        app.dialog = Some(Dialog::ImageCrop);

        frame(
            &mut app,
            &ctx,
            vec![
                Event::Text("90".into()),
                key(Key::Tab),
                Event::Text("40".into()),
            ],
        );
        for _ in 0..12 {
            frame(&mut app, &ctx, Vec::new());
        }

        let crop = ctx
            .data(|data| data.get_temp::<ImageDialog>(state_key()))
            .unwrap()
            .crop();
        assert_eq!(
            crop,
            Region {
                x: 90,
                y: 40,
                w: 810,
                h: 560
            }
        );
        assert_eq!(
            app.doc.composite(),
            original,
            "draft leaves the picture intact"
        );

        frame(&mut app, &ctx, vec![key(Key::Enter)]);
        assert!(app.dialog.is_none());
        assert_eq!(app.doc.objects[0].image_edits.crop, Some(crop));
        assert!(app.doc.objects[0].kind == ObjectKind::Image(original));
    }

    #[test]
    fn moving_crop_origin_shrinks_both_axes_to_keep_the_selected_ratio() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.doc = Document::new(900, 600);
        app.dialog = Some(Dialog::ImageCrop);
        for _ in 0..6 {
            frame(&mut app, &ctx, Vec::new());
        }
        ctx.data_mut(|data| {
            let mut state = data.get_temp::<ImageDialog>(state_key()).unwrap();
            state.aspect = CropAspect::Wide;
            state.edits.crop = Some(Region {
                x: 0,
                y: 0,
                w: 900,
                h: 506,
            });
            data.insert_temp(state_key(), state);
        });

        frame(
            &mut app,
            &ctx,
            vec![
                Event::Text("500".into()),
                key(Key::Tab),
                Event::Text("400".into()),
            ],
        );
        for _ in 0..8 {
            frame(&mut app, &ctx, Vec::new());
        }

        let crop = ctx
            .data(|data| data.get_temp::<ImageDialog>(state_key()))
            .unwrap()
            .crop();
        assert_eq!((crop.x, crop.y), (500, 400));
        assert_eq!((crop.w, crop.h), (356, 200));
        assert!(crop.x + crop.w <= 900 && crop.y + crop.h <= 600);
    }

    #[test]
    fn crop_gestures_stay_within_source_pixels_and_keep_requested_ratios() {
        for (a, b) in [
            ((0, 0), (120, 80)),
            ((120, 80), (0, 0)),
            ((120, 80), (120, 80)),
            ((22, 18), (77, 54)),
        ] {
            for aspect in CropAspect::ALL {
                let ratio = aspect.ratio((120, 80));
                let crop = crop_between(a, b, (120, 80), ratio);
                assert!(crop.w > 0 && crop.h > 0);
                assert!(crop.x + crop.w <= 120 && crop.y + crop.h <= 80);
                if let Some(ratio) = ratio {
                    assert!((crop.w as f64 - crop.h as f64 * ratio).abs() <= ratio.max(1.0));
                }
            }
        }
    }
}
