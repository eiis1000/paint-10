use super::*;

impl PaintApp {
    pub(in crate::app) fn titlebar(&mut self, ctx: &Context) {
        TopBottomPanel::top("title")
            .exact_height(31.)
            .frame(Frame::NONE.fill(Color32::WHITE))
            .show(ctx, |ui| {
                let r = ui.max_rect();
                icons::draw(
                    ui.painter(),
                    Rect::from_min_size(r.min + vec2(8., 5.), vec2(21., 21.)),
                    Icon::Colors,
                );
                for (j, icon, tip, act, enabled) in [
                    (0, Icon::Save, "Save (Ctrl+S)", Action::Save, true),
                    (
                        1,
                        Icon::Undo,
                        "Undo (Ctrl+Z)",
                        Action::Undo,
                        self.doc.can_undo()
                            || self.shape_draft.is_some()
                            || self.curve.is_some()
                            || !self.polygon.is_empty(),
                    ),
                    (
                        2,
                        Icon::Redo,
                        "Redo (Ctrl+Y)",
                        Action::Redo,
                        self.doc.can_redo(),
                    ),
                ] {
                    if icons::button(
                        ui,
                        ("quick", j),
                        Rect::from_min_size(r.min + vec2(38. + j as f32 * 27., 3.), vec2(25., 25.)),
                        icon,
                        "",
                        false,
                        enabled,
                    )
                    .on_hover_text(tip)
                    .clicked()
                    {
                        self.action(act, ctx);
                    }
                }
                ui.painter().line_segment(
                    [r.min + vec2(125., 8.), r.min + vec2(125., 23.)],
                    Stroke::new(1.0_f32, Color32::from_gray(217)),
                );
                let name = self
                    .file
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or("Untitled".into());
                let title = format!(
                    "{}{} - Paint 10",
                    if self.doc.dirty() { "*" } else { "" },
                    name
                );
                ui.painter().text(
                    r.min + vec2(138., 15.),
                    Align2::LEFT_CENTER,
                    &title,
                    FontId::proportional(13.),
                    Color32::from_gray(25),
                );
                ctx.send_viewport_cmd(ViewportCommand::Title(title));
                let drag = ui.interact(
                    Rect::from_min_max(r.min + vec2(130., 0.), r.right_top() + vec2(-138., 31.)),
                    Id::new("title_drag"),
                    Sense::click_and_drag().difference(Sense::FOCUSABLE),
                );
                if drag.drag_started() {
                    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                }
                if drag.double_clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Maximized(
                        !ctx.input(|i| i.viewport().maximized.unwrap_or(false)),
                    ));
                }
                for i in 0..3 {
                    let rr = Rect::from_min_size(
                        pos2(r.right() - 138. + i as f32 * 46., r.top()),
                        vec2(46., 31.),
                    );
                    let response = ui.interact(rr, Id::new(("caption", i)), Sense::click());
                    let label = match i {
                        0 => "Minimize",
                        1 if ctx.input(|input| input.viewport().maximized.unwrap_or(false)) => {
                            "Restore window"
                        }
                        1 => "Maximize",
                        _ => "Close Paint 10",
                    };
                    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
                    if response.hovered() {
                        ui.painter().rect_filled(
                            rr,
                            0.,
                            if i == 2 {
                                Color32::from_rgb(232, 17, 35)
                            } else {
                                Color32::from_gray(231)
                            },
                        );
                    }
                    let c = if i == 2 && response.hovered() {
                        Color32::WHITE
                    } else {
                        Color32::from_gray(30)
                    };
                    let center = rr.center();
                    if i == 0 {
                        ui.painter().line_segment(
                            [center + vec2(-5., 0.), center + vec2(5., 0.)],
                            Stroke::new(1.0_f32, c),
                        );
                    } else if i == 1 {
                        ui.painter().rect_stroke(
                            Rect::from_center_size(center, vec2(10., 10.)),
                            0.,
                            Stroke::new(1.0_f32, c),
                            StrokeKind::Inside,
                        );
                    } else {
                        ui.painter().line_segment(
                            [center + vec2(-5., -5.), center + vec2(5., 5.)],
                            Stroke::new(1.0_f32, c),
                        );
                        ui.painter().line_segment(
                            [center + vec2(5., -5.), center + vec2(-5., 5.)],
                            Stroke::new(1.0_f32, c),
                        );
                    }
                    if response.clicked() {
                        match i {
                            0 => ctx.send_viewport_cmd(ViewportCommand::Minimized(true)),
                            1 => ctx.send_viewport_cmd(ViewportCommand::Maximized(
                                !ctx.input(|i| i.viewport().maximized.unwrap_or(false)),
                            )),
                            _ => self.action(Action::Close, ctx),
                        }
                    }
                    if response.has_focus() {
                        ui.painter().rect_stroke(
                            rr.shrink(3.0),
                            0.0,
                            Stroke::new(2.0_f32, Color32::from_rgb(0, 80, 160)),
                            StrokeKind::Inside,
                        );
                    }
                    response.on_hover_text(label);
                }
            });
    }

    pub(in crate::app) fn status(&mut self, ctx: &Context) {
        if !self.status_bar {
            return;
        }
        TopBottomPanel::bottom("status")
            .exact_height(26.)
            .frame(
                Frame::NONE
                    .fill(Color32::from_rgb(240, 240, 240))
                    .inner_margin(Margin::symmetric(8, 3)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [115., 18.],
                        Label::new(
                            self.cursor
                                .map(|p| format!("⌖  {}, {} px", p.0, p.1))
                                .unwrap_or_default(),
                        ),
                    );
                    ui.separator();
                    ui.add_sized(
                        [125., 18.],
                        Label::new(
                            self.selected_region()
                                .map(|r| format!("  {} × {} px", r.w, r.h))
                                .unwrap_or_default(),
                        ),
                    );
                    ui.separator();
                    ui.label(format!(
                        "  {} × {} px",
                        self.doc.image.width(),
                        self.doc.image.height()
                    ));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let zoom_in = ui.small_button("+").on_hover_text("Zoom in");
                        zoom_in.widget_info(|| {
                            WidgetInfo::labeled(WidgetType::Button, true, "Zoom in")
                        });
                        if zoom_in.clicked() {
                            self.zoom = (self.zoom * 2.).min(8.);
                        }
                        let mut zoom = self.zoom * 100.;
                        let zoom_slider = ui.add(
                            Slider::new(&mut zoom, 12.5..=800.)
                                .logarithmic(true)
                                .show_value(false),
                        );
                        zoom_slider.widget_info(|| {
                            WidgetInfo::slider(true, zoom as f64, "Zoom percentage")
                        });
                        self.zoom = zoom / 100.;
                        let zoom_out = ui.small_button("−").on_hover_text("Zoom out");
                        zoom_out.widget_info(|| {
                            WidgetInfo::labeled(WidgetType::Button, true, "Zoom out")
                        });
                        if zoom_out.clicked() {
                            self.zoom = (self.zoom / 2.).max(0.125);
                        }
                        if ui
                            .button(format!("{:.0}%", self.zoom * 100.))
                            .on_hover_text("Reset to 100%")
                            .clicked()
                        {
                            self.zoom = 1.;
                        }
                        if ui.available_width() > 200. {
                            ui.add(Label::new(&self.message).truncate());
                        }
                    });
                });
            });
    }
}
