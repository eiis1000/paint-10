//! A view-only ruler. Its endpoints never enter the document or undo history.

use super::*;

const HELP: &str = "Drag between pixel centers to measure. Drag A or B to adjust an endpoint; arrow keys move the active endpoint by one pixel (Shift: ten). Delete resets the ruler; Esc leaves Measure. Angles increase clockwise from the right. Measurements are not saved in the picture.";

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Unit {
    #[default]
    Pixels,
    Millimeters,
    Centimeters,
    Inches,
}

impl Unit {
    fn label(self) -> &'static str {
        match self {
            Self::Pixels => "Pixels",
            Self::Millimeters => "Millimeters",
            Self::Centimeters => "Centimeters",
            Self::Inches => "Inches",
        }
    }

    fn physical(self) -> Option<(f64, &'static str)> {
        match self {
            Self::Pixels => None,
            Self::Millimeters => Some((25.4, "mm")),
            Self::Centimeters => Some((2.54, "cm")),
            Self::Inches => Some((1.0, "in")),
        }
    }
}

#[derive(Default)]
pub(super) struct Measurement {
    pub(super) enabled: bool,
    points: Option<[Point; 2]>,
    dragging: Option<usize>,
    active: usize,
    unit: Unit,
}

#[derive(Debug)]
struct Reading {
    dx: i64,
    dy: i64,
    pixels: f64,
    angle: f64,
}

impl Reading {
    fn between([a, b]: [Point; 2]) -> Self {
        let dx = i64::from(b.0) - i64::from(a.0);
        let dy = i64::from(b.1) - i64::from(a.1);
        Self {
            dx,
            dy,
            pixels: (dx as f64).hypot(dy as f64),
            angle: (dy as f64).atan2(dx as f64).to_degrees(),
        }
    }

    fn physical_distance(&self, dpi_x: f32, dpi_y: f32, unit: Unit) -> Option<(f64, &'static str)> {
        if [dpi_x, dpi_y]
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return None;
        }
        let (factor, label) = unit.physical()?;
        Some((
            (self.dx as f64 / dpi_x as f64).hypot(self.dy as f64 / dpi_y as f64) * factor,
            label,
        ))
    }
}

impl Measurement {
    fn reset(&mut self) {
        self.points = None;
        self.dragging = None;
    }

    fn clamp(point: Point, dimensions: (u32, u32)) -> Point {
        (
            point.0.clamp(0, dimensions.0.saturating_sub(1) as i32),
            point.1.clamp(0, dimensions.1.saturating_sub(1) as i32),
        )
    }
}

impl PaintApp {
    pub(in crate::app) fn set_measure_enabled(&mut self, enabled: bool, ctx: &Context) {
        if self.gesture.is_some() || self.dialog.is_some() || self.pending.is_some() {
            return;
        }
        self.measure.enabled = enabled;
        self.measure.dragging = None;
        ctx.request_repaint();
        // Pause a pending text editor without committing its text or format.
        ctx.memory_mut(|memory| {
            memory.request_focus(if !enabled && self.text_edit.is_some() {
                Id::new("text_input")
            } else {
                Id::new("canvas")
            });
        });
    }

    pub(in crate::app) fn measure_group(&mut self, ui: &mut Ui, origin: Pos2, ctx: &Context) {
        Self::group(ui, origin, 0.0, 189.0, "Measure");
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(
                origin + vec2(10.0, 7.0),
                vec2(168.0, 80.0),
            )),
            |ui| self.measure_controls(ui, ctx),
        );
    }

    fn measure_controls(&mut self, ui: &mut Ui, ctx: &Context) {
        let mut enabled = self.measure.enabled;
        let toggle = ui.add_enabled(
            self.gesture.is_none(),
            Checkbox::new(&mut enabled, "Measure distance"),
        );
        ribbon_controls::register(ui, &toggle, "M", keytips::Kind::Button);
        if toggle.changed() {
            self.set_measure_enabled(enabled, ctx);
            ui.close_menu();
        }
        toggle.on_hover_text(HELP);
        let reset = ui.add_enabled(
            self.measure.points.is_some(),
            Button::new("Reset measurement"),
        );
        ribbon_controls::register(ui, &reset, "C", keytips::Kind::Button);
        if reset.clicked() {
            self.measure.reset();
        }
        let units = ComboBox::from_id_salt("measure_units")
            .selected_text(self.measure.unit.label())
            .width(150.0)
            .show_ui(ui, |ui| {
                theme::menu(ui);
                for (unit, key) in [
                    (Unit::Pixels, "P"),
                    (Unit::Millimeters, "M"),
                    (Unit::Centimeters, "C"),
                    (Unit::Inches, "I"),
                ] {
                    let response = ui.add(
                        theme::MenuItem::new(unit.label())
                            .selected(self.measure.unit == unit)
                            .width(170.0),
                    );
                    if response.clicked() {
                        self.measure.unit = unit;
                        ui.close_menu();
                    }
                    keytips::register(
                        ui,
                        &response,
                        "measure_units",
                        "Units",
                        key,
                        keytips::Kind::Button,
                    );
                }
            });
        ribbon_controls::register(
            ui,
            &units.response,
            "U",
            keytips::Kind::Menu {
                scope: "measure_units",
            },
        );
        units.response.widget_info(|| {
            WidgetInfo::labeled(
                WidgetType::ComboBox,
                units.response.enabled(),
                "Measurement units",
            )
        });
        units.response.on_hover_text("Physical distance uses the picture's horizontal and vertical DPI. Pixel distance is always shown.");
    }

    pub(in crate::app) fn measure_readout(&self, ctx: &Context) {
        if !self.measure.enabled {
            return;
        }
        TopBottomPanel::top("measurement_readout")
            .frame(
                Frame::NONE
                    .fill(Color32::from_rgb(232, 243, 252))
                    .inner_margin(Margin::symmetric(10, 5)),
            )
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| self.measure_values(ui));
            });
    }

    fn measure_values(&self, ui: &mut Ui) {
        ui.strong("Measure").on_hover_text(HELP);
        let Some(points) = self.measure.points else {
            ui.label("Drag between two pixels. Drag A or B to adjust.");
            return;
        };
        let reading = Reading::between(points);
        ui.label(format!("{:.2} px", reading.pixels));
        ui.separator();
        ui.label(format!("Δx: {} px   Δy: {} px", reading.dx, reading.dy));
        ui.label(format!("Angle: {:.2}°", reading.angle))
            .on_hover_text(
                "Angle in pixel coordinates: zero points right; positive angles turn clockwise.",
            );
        if let Some((distance, unit)) = reading.physical_distance(
            self.doc.resolution.x,
            self.doc.resolution.y,
            self.measure.unit,
        ) {
            ui.label(format!("{distance:.3} {unit}"))
                .on_hover_text(format!(
                    "Using {:.3} × {:.3} DPI",
                    self.doc.resolution.x, self.doc.resolution.y
                ));
        }
    }

    pub(in crate::app) fn measure_canvas(
        &mut self,
        ui: &mut Ui,
        response: &Response,
        canvas: Rect,
        ctx: &Context,
    ) {
        let previous = self.measure.points;
        ctx.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                Id::new("canvas"),
                EventFilter {
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    ..Default::default()
                },
            );
        });
        if let Some(text) = &self.text_edit {
            let mut preview = ui.new_child(UiBuilder::new());
            preview.set_clip_rect(canvas.intersect(ui.clip_rect()));
            let slot = preview.painter().add(egui::Shape::Noop);
            text_preview::paint(
                &preview,
                slot,
                canvas.min + vec2(text.origin.0 as f32, text.origin.1 as f32) * self.zoom,
                &text.text,
                &text.format,
                self.zoom,
            );
        }
        let blocked = self.dialog.is_some()
            || self.pending.is_some()
            || keytips::popup_open(ctx);
        if blocked {
            self.measure.dragging = None;
        } else {
            let events = ctx.input(|input| input.events.clone());
            for event in events {
                match event {
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: true,
                        ..
                    } if canvas.contains(pos)
                        && ui.clip_rect().contains(pos)
                        && ctx.layer_id_at(pos) == Some(ui.layer_id()) =>
                    {
                        let point = Measurement::clamp(
                            self.point(pos, canvas),
                            self.doc.image.dimensions(),
                        );
                        let endpoint = self.measure.points.and_then(|points| {
                            [1, 0].into_iter().find(|&index| {
                                let marker = self.measure_position(points[index], canvas);
                                marker.distance(pos) <= 8.0
                            })
                        });
                        let active = if let Some(index) = endpoint {
                            index
                        } else {
                            self.measure.points = Some([point, point]);
                            1
                        };
                        self.measure.active = active;
                        self.measure.dragging = Some(active);
                        response.request_focus();
                    }
                    Event::PointerMoved(pos) => self.move_measure_endpoint(pos, canvas),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: false,
                        ..
                    } => {
                        self.move_measure_endpoint(pos, canvas);
                        self.measure.dragging = None;
                    }
                    Event::PointerGone => self.measure.dragging = None,
                    _ => {}
                }
            }
        }
        self.cursor = ctx
            .input(|input| input.pointer.hover_pos())
            .filter(|pos| response.hovered() && canvas.contains(*pos))
            .map(|pos| Measurement::clamp(self.point(pos, canvas), self.doc.image.dimensions()));
        if let Some(points) = self.measure.points {
            let painter = ui
                .painter()
                .with_clip_rect(canvas.intersect(ui.clip_rect()));
            let positions = points.map(|point| self.measure_position(point, canvas));
            painter.line_segment(positions, Stroke::new(3.0_f32, Color32::WHITE));
            painter.line_segment(positions, Stroke::new(1.0_f32, BLUE));
            for (index, position) in positions.into_iter().enumerate() {
                painter.circle_filled(position, 5.0, Color32::WHITE);
                painter.circle_stroke(
                    position,
                    5.0,
                    Stroke::new(
                        if index == self.measure.active {
                            2.0_f32
                        } else {
                            1.0_f32
                        },
                        BLUE,
                    ),
                );
                painter.line_segment(
                    [position - vec2(7.0, 0.0), position + vec2(7.0, 0.0)],
                    Stroke::new(1.0_f32, BLUE),
                );
                painter.line_segment(
                    [position - vec2(0.0, 7.0), position + vec2(0.0, 7.0)],
                    Stroke::new(1.0_f32, BLUE),
                );
                let label = format!(
                    "{} ({}, {})",
                    if index == 0 { "A" } else { "B" },
                    points[index].0,
                    points[index].1
                );
                let galley =
                    painter.layout_no_wrap(label, FontId::proportional(11.0), Color32::BLACK);
                let label_size = galley.size() + vec2(6.0, 4.0);
                let offset = if index == 0 {
                    vec2(9.0, -label_size.y - 7.0)
                } else {
                    vec2(9.0, 7.0)
                };
                let clip = ui.clip_rect();
                let preferred = position + offset;
                let origin = pos2(
                    preferred
                        .x
                        .clamp(clip.left(), (clip.right() - label_size.x).max(clip.left())),
                    preferred
                        .y
                        .clamp(clip.top(), (clip.bottom() - label_size.y).max(clip.top())),
                );
                let label_rect = Rect::from_min_size(origin, label_size);
                // Labels may extend past a tiny canvas so pixel-art measurements
                // remain readable at 100%; they still stay inside its viewport.
                ui.painter()
                    .rect_filled(label_rect, 2.0, Color32::from_white_alpha(235));
                ui.painter()
                    .galley(label_rect.min + vec2(3.0, 2.0), galley, Color32::BLACK);
            }
        }
        if self.measure.points != previous {
            ctx.request_repaint();
        }
    }

    fn measure_position(&self, point: Point, canvas: Rect) -> Pos2 {
        canvas.min + vec2(point.0 as f32 + 0.5, point.1 as f32 + 0.5) * self.zoom
    }

    fn move_measure_endpoint(&mut self, position: Pos2, canvas: Rect) {
        let Some(index) = self.measure.dragging else {
            return;
        };
        let point = Measurement::clamp(self.point(position, canvas), self.doc.image.dimensions());
        if let Some(points) = &mut self.measure.points {
            points[index] = point;
        }
    }

    pub(in crate::app) fn measure_shortcuts(&mut self, ctx: &Context) -> bool {
        use super::shortcuts::consume_shortcut;
        if !self.measure.enabled {
            return false;
        }
        if ctx.input_mut(|input| consume_shortcut(input, Modifiers::NONE, Key::Escape)) {
            self.set_measure_enabled(false, ctx);
            return true;
        }
        let canvas_focus = ctx.memory(|memory| {
            memory.focused().is_none()
                || memory.has_focus(Id::new("canvas"))
                || memory.has_focus(Id::new("text_input"))
        });
        if !canvas_focus {
            return false;
        }
        if ctx.input_mut(|input| consume_shortcut(input, Modifiers::NONE, Key::Delete)) {
            self.measure.reset();
            return true;
        }
        for (key, delta) in [
            (Key::ArrowLeft, (-1, 0)),
            (Key::ArrowRight, (1, 0)),
            (Key::ArrowUp, (0, -1)),
            (Key::ArrowDown, (0, 1)),
        ] {
            for (modifiers, step) in [(Modifiers::NONE, 1), (Modifiers::SHIFT, 10)] {
                if ctx.input_mut(|input| consume_shortcut(input, modifiers, key)) {
                    if let Some(points) = &mut self.measure.points {
                        let point = &mut points[self.measure.active];
                        *point = Measurement::clamp(
                            (
                                point.0.saturating_add(delta.0 * step),
                                point.1.saturating_add(delta.1 * step),
                            ),
                            self.doc.image.dimensions(),
                        );
                    }
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(app: &mut PaintApp, ctx: &Context, events: Vec<Event>) -> FullOutput {
        let mut input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(640.0, 480.0))),
            events,
            time: Some(ctx.cumulative_pass_nr() as f64 / 30.0),
            ..Default::default()
        };
        eframe::App::raw_input_hook(app, ctx, &mut input);
        ctx.run(input, |ctx| {
            app.shortcut(ctx);
            app.status(ctx);
            app.canvas(ctx);
            app.dialogs(ctx);
        })
    }

    fn button(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    fn key(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn drag(app: &mut PaintApp, ctx: &Context, start: Point, end: Point) {
        let a = app.measure_position(start, app.canvas_rect);
        let b = app.measure_position(end, app.canvas_rect);
        frame(
            app,
            ctx,
            vec![
                Event::PointerMoved(a),
                button(a, true),
                Event::PointerMoved(b),
                button(b, false),
            ],
        );
        frame(app, ctx, vec![]);
    }

    #[test]
    fn measurement_math_uses_both_dpi_axes_and_avoids_integer_overflow() {
        let reading = Reading::between([(1, 2), (4, 6)]);
        assert_eq!((reading.dx, reading.dy, reading.pixels), (3, 4, 5.0));
        assert!((reading.angle - 53.130102).abs() < 0.000001);
        let reading = Reading::between([(0, 0), (300, 300)]);
        let inches = reading
            .physical_distance(300.0, 150.0, Unit::Inches)
            .unwrap();
        assert!((inches.0 - 5.0_f64.sqrt()).abs() < 1e-10);
        let millimeters = reading
            .physical_distance(300.0, 150.0, Unit::Millimeters)
            .unwrap();
        assert!((millimeters.0 - inches.0 * 25.4).abs() < 1e-10);
        assert!(reading.physical_distance(0.0, 96.0, Unit::Inches).is_none());
        assert!(reading
            .physical_distance(96.0, 96.0, Unit::Pixels)
            .is_none());
        let extreme = Reading::between([(i32::MIN, i32::MIN), (i32::MAX, i32::MAX)]);
        assert!(extreme.pixels.is_finite());
        assert!(extreme.pixels > u32::MAX as f64);
        assert_eq!(Reading::between([(0, 0), (0, -5)]).angle, -90.0);
    }

    #[test]
    fn measurement_uses_pixel_centers_at_every_zoom_and_ignores_hover_after_release() {
        for zoom in [1.0, 8.0, 32.0] {
            let ctx = Context::default();
            let mut app = PaintApp::new_with_context(&ctx, false);
            app.doc = Document::new(16, 16);
            app.zoom = zoom;
            app.set_measure_enabled(true, &ctx);
            frame(&mut app, &ctx, vec![]);
            frame(&mut app, &ctx, vec![]);
            drag(&mut app, &ctx, (1, 2), (4, 6));
            let hover = app.measure_position((9, 10), app.canvas_rect);
            frame(&mut app, &ctx, vec![Event::PointerMoved(hover)]);
            assert_eq!(app.measure.points, Some([(1, 2), (4, 6)]), "zoom {zoom}");
            assert_eq!(Reading::between(app.measure.points.unwrap()).pixels, 5.0);
            assert!(app.measure.dragging.is_none());
            assert!(!app.doc.dirty());
            assert!(!app.doc.can_undo());
        }
    }

    #[test]
    fn measurement_adjust_reset_and_exit_preserve_picture_selection_exports_and_history() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.doc = Document::new(20, 20);
        app.doc.begin();
        app.doc.image.put_pixel(0, 0, Rgba(BLACK));
        app.doc.commit();
        app.doc.mark_saved();
        app.doc.begin();
        app.doc.image.put_pixel(1, 0, Rgba(BLACK));
        app.doc.commit();
        app.doc.undo();
        app.selection = Some(Region {
            x: 1,
            y: 2,
            w: 12,
            h: 13,
        });
        let before = crate::project::encode(&app.doc).unwrap();
        app.zoom = 16.0;
        app.set_measure_enabled(true, &ctx);
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        drag(&mut app, &ctx, (1, 2), (4, 6));
        drag(&mut app, &ctx, (1, 2), (2, 3));
        assert_eq!(app.measure.points, Some([(2, 3), (4, 6)]));
        frame(&mut app, &ctx, vec![key(Key::ArrowRight, Modifiers::SHIFT)]);
        assert_eq!(app.measure.points, Some([(12, 3), (4, 6)]));
        frame(&mut app, &ctx, vec![key(Key::Delete, Modifiers::NONE)]);
        assert!(app.measure.points.is_none());
        frame(&mut app, &ctx, vec![key(Key::Escape, Modifiers::NONE)]);
        assert!(!app.measure.enabled);
        assert_eq!(
            app.selection,
            Some(Region {
                x: 1,
                y: 2,
                w: 12,
                h: 13
            })
        );
        assert_eq!(crate::project::encode(&app.doc).unwrap(), before);
        assert!(!app.doc.dirty());
        assert!(app.doc.can_undo() && app.doc.can_redo());
    }

    #[test]
    fn measurement_preserves_a_pending_text_editor_and_is_blocked_by_dialogs() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.doc = Document::new(40, 40);
        app.zoom = 8.0;
        app.text_edit = Some(TextEditState {
            index: None,
            origin: (0, 0),
            text: "Still editable".into(),
            format: crate::text::TextFormat::default(),
            focus: false,
            selection: 0..5,
            insertion_style: None,
            history: Default::default(),
            palette_colors: app.colors,
        });
        app.set_measure_enabled(true, &ctx);
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        drag(&mut app, &ctx, (1, 2), (4, 6));
        assert_eq!(app.measure.points, Some([(1, 2), (4, 6)]));
        assert_eq!(app.text_edit.as_ref().unwrap().text, "Still editable");
        assert_eq!(app.text_edit.as_ref().unwrap().selection, 0..5);
        assert!(app.doc.objects.is_empty() && !app.doc.dirty());
        app.dialog = Some(Dialog::About);
        drag(&mut app, &ctx, (20, 20), (25, 25));
        assert_eq!(app.measure.points, Some([(1, 2), (4, 6)]));
        assert!(app.doc.objects.is_empty() && !app.doc.dirty());
    }

    fn narrow_frame(app: &mut PaintApp, ctx: &Context, events: Vec<Event>) -> FullOutput {
        ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(500.0, 480.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                if !app.ribbon_keyboard(ctx) {
                    app.shortcut(ctx);
                }
                app.titlebar(ctx);
                app.ribbon(ctx);
                app.status(ctx);
                app.canvas(ctx);
                app.keyboard_menu(ctx);
            },
        )
    }

    #[test]
    fn measurement_controls_work_with_keytips_and_wrap_readout_at_500_pixels() {
        let ctx = Context::default();
        ctx.enable_accesskit();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.view_tab = true;
        for _ in 0..3 {
            narrow_frame(&mut app, &ctx, vec![]);
        }
        for code in [Key::F10, Key::V, Key::Z, Key::M, Key::M] {
            narrow_frame(&mut app, &ctx, vec![key(code, Modifiers::NONE)]);
        }
        assert!(app.measure.enabled);
        // The closing frame intentionally retains the popup input guard.
        narrow_frame(&mut app, &ctx, vec![]);
        assert!(!keytips::popup_open(&ctx));
        for code in [Key::F10, Key::V, Key::Z, Key::M, Key::U, Key::I] {
            narrow_frame(&mut app, &ctx, vec![key(code, Modifiers::NONE)]);
        }
        assert_eq!(app.measure.unit, Unit::Inches);
        app.measure.points = Some([(0, 0), (899, 599)]);
        app.selection = Some(Region {
            x: 0,
            y: 0,
            w: 12,
            h: 13,
        });
        let output = narrow_frame(&mut app, &ctx, vec![]);
        let nodes = &output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes;
        let reading = Reading::between(app.measure.points.unwrap());
        let labels = [
            format!("{:.2} px", reading.pixels),
            "Δx: 899 px   Δy: 599 px".into(),
            format!("Angle: {:.2}°", reading.angle),
            format!("{:.3} in", reading.pixels / 96.0),
        ];
        let bounds: Vec<_> = labels
            .iter()
            .map(|label| {
                nodes
                    .iter()
                    .find_map(|(_, node)| {
                        (node.value() == Some(label.as_str())
                            || node.label() == Some(label.as_str()))
                        .then(|| node.bounds())
                        .flatten()
                    })
                    .unwrap_or_else(|| panic!("missing measurement label {label}"))
            })
            .collect();
        for (index, rect) in bounds.iter().enumerate() {
            assert!(rect.x0 >= 0.0 && rect.x1 <= 500.0, "{rect:?}");
            assert!(rect.y1 <= app.canvas_rect.top() as f64, "{rect:?}");
            for other in &bounds[..index] {
                assert!(
                    rect.x1 <= other.x0
                        || rect.x0 >= other.x1
                        || rect.y1 <= other.y0
                        || rect.y0 >= other.y1,
                    "overlap: {rect:?}, {other:?}"
                );
            }
        }
        assert!(nodes.iter().any(|(_, node)| node
            .value()
            .is_some_and(|value| value.contains("12 × 13 px selected"))));
    }
}
