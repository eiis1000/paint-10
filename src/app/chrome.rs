use super::*;
use crate::preferences::QuickCommand;

impl PaintApp {
    pub(in crate::app) fn quick_action(&mut self, command: QuickCommand, ctx: &Context) {
        match command {
            QuickCommand::New => self.action(Action::New, ctx),
            QuickCommand::Open => self.action(Action::Open, ctx),
            QuickCommand::Save => self.action(Action::Save, ctx),
            QuickCommand::Undo => self.action(Action::Undo, ctx),
            QuickCommand::Redo => self.action(Action::Redo, ctx),
            QuickCommand::Print => self.action(Action::Print, ctx),
            QuickCommand::PrintPreview => {
                self.finish_editing();
                self.print_preview = Some(crate::print_preview::PrintPreview::new(
                    self.doc.composite(),
                ));
            }
            QuickCommand::Email => {
                self.finish_editing();
                let image = self.doc.composite();
                self.start_job(ctx, move || {
                    JobResult::Status(
                        crate::integration::compose_email(&image)
                            .map(|_| "Email draft opened".into()),
                    )
                });
            }
        }
    }

    fn quick_access_controls(&mut self, ui: &mut Ui, origin: Pos2) -> f32 {
        let before = self.quick_access.clone();
        for (index, command) in before.commands.iter().copied().enumerate() {
            let icon = match command {
                QuickCommand::New => Icon::New,
                QuickCommand::Open => Icon::Open,
                QuickCommand::Save => Icon::Save,
                QuickCommand::Undo => Icon::Undo,
                QuickCommand::Redo => Icon::Redo,
                QuickCommand::PrintPreview => Icon::PrintPreview,
                QuickCommand::Print => Icon::Print,
                QuickCommand::Email => Icon::Email,
            };
            let enabled = self.dialog.is_none()
                && self.pending.is_none()
                && match command {
                    QuickCommand::Undo if self.text_edit.is_some() => self.text_can_undo(),
                    QuickCommand::Undo => {
                        self.doc.can_undo()
                            || self.shape_draft.is_some()
                            || self.curve.is_some()
                            || !self.polygon.is_empty()
                    }
                    QuickCommand::Redo if self.text_edit.is_some() => self.text_can_redo(),
                    QuickCommand::Redo => self.doc.can_redo(),
                    QuickCommand::Email => self.job.is_none(),
                    _ => true,
                };
            let response = icons::button(
                ui,
                ("quick", index),
                Rect::from_min_size(origin + vec2(index as f32 * 27.0, 0.0), vec2(25.0, 25.0)),
                icon,
                "",
                false,
                enabled,
            )
            .on_hover_text(command.name());
            if response.clicked() {
                self.quick_action(command, ui.ctx());
            }
            response.context_menu(|ui| {
                if ui.button("Remove from Quick Access Toolbar").clicked() {
                    self.quick_access.commands.retain(|item| *item != command);
                    ui.close_menu();
                }
                if ui
                    .button(if self.quick_access.below_ribbon {
                        "Show above the ribbon"
                    } else {
                        "Show below the ribbon"
                    })
                    .clicked()
                {
                    self.quick_access.below_ribbon = !self.quick_access.below_ribbon;
                    ui.close_menu();
                }
            });
        }
        let offset = before.commands.len() as f32 * 27.0;
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(
                origin + vec2(offset, 3.0),
                vec2(24.0, 24.0),
            )),
            |ui| {
                let menu = ui.menu_button("", |ui| {
                    ui.strong("Customize Quick Access Toolbar");
                    for command in QuickCommand::ALL {
                        let mut selected = self.quick_access.commands.contains(&command);
                        if ui.checkbox(&mut selected, command.name()).changed() {
                            if selected {
                                self.quick_access.commands.push(command);
                            } else {
                                self.quick_access.commands.retain(|item| *item != command);
                            }
                        }
                    }
                    ui.separator();
                    ui.checkbox(&mut self.quick_access.below_ribbon, "Show below the ribbon");
                });
                icons::draw(
                    ui.painter(),
                    menu.response.rect.shrink(3.0),
                    Icon::ChevronDown,
                );
                menu.response.widget_info(|| {
                    WidgetInfo::labeled(WidgetType::Button, true, "Customize Quick Access Toolbar")
                });
                menu.response
                    .on_hover_text("Customize Quick Access Toolbar");
            },
        );
        if self.quick_access != before {
            if let Err(error) = crate::preferences::save_quick_access(&self.quick_access) {
                self.message = format!("Could not save toolbar settings: {error}");
            }
        }
        offset + 24.0
    }

    pub(in crate::app) fn quick_access_below(&mut self, ctx: &Context) {
        if self.quick_access.below_ribbon {
            TopBottomPanel::top("quick_access_below")
                .exact_height(29.0)
                .frame(Frame::NONE.fill(RIBBON))
                .show(ctx, |ui| {
                    self.quick_access_controls(ui, ui.max_rect().min + vec2(5.0, 2.0));
                });
        }
    }

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
                let quick_width = if self.quick_access.below_ribbon {
                    0.0
                } else {
                    self.quick_access_controls(ui, r.min + vec2(38.0, 3.0))
                };
                let title_x = 44.0 + quick_width;
                ui.painter().line_segment(
                    [r.min + vec2(title_x, 8.), r.min + vec2(title_x, 23.)],
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
                let title_rect = Rect::from_min_max(
                    r.min + vec2(title_x + 10.0, 0.0),
                    r.right_top() + vec2(-140.0, 31.0),
                );
                ui.painter().with_clip_rect(title_rect).text(
                    r.min + vec2(title_x + 10.0, 15.),
                    Align2::LEFT_CENTER,
                    &title,
                    FontId::proportional(13.),
                    Color32::from_gray(25),
                );
                ctx.send_viewport_cmd(ViewportCommand::Title(title));
                let drag = ui.interact(
                    title_rect,
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
