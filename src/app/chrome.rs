use super::*;
use crate::preferences::QuickCommand;

fn app_icon(ctx: &Context) -> TextureHandle {
    let size = (21.0 * ctx.pixels_per_point()).round().clamp(16.0, 256.0) as u32;
    let cache_key = Id::new("paint10-app-icon");
    let cached = ctx.data(|data| data.get_temp::<(u32, TextureHandle)>(cache_key));
    if let Some((_, texture)) = cached.filter(|(cached_size, _)| *cached_size == size) {
        return texture;
    }

    // Resize once at the current display scale so the small mark stays crisp.
    let pixels = image::load_from_memory(include_bytes!("../../assets/paint-10.png"))
        .expect("The bundled Paint 10 icon must be a valid PNG")
        .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
        .into_rgba8();
    let texture = ctx.load_texture(
        "paint10-app-icon",
        display::image([size as usize, size as usize], pixels.as_raw()),
        TextureOptions::LINEAR,
    );
    ctx.data_mut(|data| data.insert_temp(cache_key, (size, texture.clone())));
    texture
}

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
                    QuickCommand::Email => self.job.is_none() && !cfg!(target_arch = "wasm32"),
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
            keytips::register(
                ui,
                &response,
                "tabs",
                "Quick Access",
                (index + 1).to_string(),
                keytips::Kind::Button,
            );
            if response.clicked() {
                self.quick_action(command, ui.ctx());
            }
            response.context_menu(|ui| {
                let remove = ui.button("Remove from Quick Access Toolbar");
                keytips::register(
                    ui,
                    &remove,
                    "quick_access_context",
                    "Quick Access",
                    "R",
                    keytips::Kind::Button,
                );
                if remove.clicked() {
                    self.quick_access.commands.retain(|item| *item != command);
                    ui.close_menu();
                }
                let position = ui.button(if self.quick_access.below_ribbon {
                    "Show above the ribbon"
                } else {
                    "Show below the ribbon"
                });
                keytips::register(
                    ui,
                    &position,
                    "quick_access_context",
                    "Quick Access",
                    "B",
                    keytips::Kind::Button,
                );
                if position.clicked() {
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
                let menu = egui::menu::menu_custom_button(
                    ui,
                    Button::new("").min_size(vec2(21.0, 20.0)),
                    |ui| {
                        theme::menu(ui);
                        theme::menu_heading(ui, "Customize Quick Access Toolbar", 265.0);
                        for (index, command) in QuickCommand::ALL.into_iter().enumerate() {
                            let selected = self.quick_access.commands.contains(&command);
                            let choice = ui.add(
                                theme::MenuItem::new(command.name())
                                    .selected(selected)
                                    .width(265.0),
                            );
                            choice.widget_info(|| {
                                WidgetInfo::selected(
                                    WidgetType::Checkbox,
                                    true,
                                    selected,
                                    command.name(),
                                )
                            });
                            keytips::register(
                                ui,
                                &choice,
                                "quick_access",
                                "Quick Access",
                                (index + 1).to_string(),
                                keytips::Kind::Button,
                            );
                            if choice.clicked() {
                                if !selected {
                                    self.quick_access.commands.push(command);
                                } else {
                                    self.quick_access.commands.retain(|item| *item != command);
                                }
                            }
                        }
                        ui.separator();
                        let position = ui.add(
                            theme::MenuItem::new("Show below the ribbon")
                                .selected(self.quick_access.below_ribbon)
                                .width(265.0),
                        );
                        position.widget_info(|| {
                            WidgetInfo::selected(
                                WidgetType::Checkbox,
                                true,
                                self.quick_access.below_ribbon,
                                "Show below the ribbon",
                            )
                        });
                        if position.clicked() {
                            self.quick_access.below_ribbon = !self.quick_access.below_ribbon;
                        }
                        keytips::register(
                            ui,
                            &position,
                            "quick_access",
                            "Quick Access",
                            "B",
                            keytips::Kind::Button,
                        );
                    },
                );
                icons::draw(
                    ui.painter(),
                    menu.response.rect.shrink(3.0),
                    Icon::ChevronDown,
                );
                menu.response.widget_info(|| {
                    WidgetInfo::labeled(WidgetType::Button, true, "Customize Quick Access Toolbar")
                });
                keytips::register(
                    ui,
                    &menu.response,
                    "tabs",
                    "Quick Access",
                    "0",
                    keytips::Kind::Menu {
                        scope: "quick_access",
                    },
                );
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
                ui.painter().image(
                    app_icon(ctx).id(),
                    Rect::from_min_size(r.min + vec2(8., 5.), vec2(21., 21.)),
                    Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                    Color32::WHITE,
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
                // Viewport commands request another repaint. Send the title
                // only when it changes, so an idle picture can remain idle.
                let title_key = Id::new("paint10-last-window-title");
                let title_changed = ctx.data_mut(|data| {
                    if data.get_temp::<String>(title_key).as_ref() == Some(&title) {
                        false
                    } else {
                        data.insert_temp(title_key, title.clone());
                        true
                    }
                });
                if title_changed {
                    ctx.send_viewport_cmd(ViewportCommand::Title(title));
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
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
                        response
                            .widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
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
                }
                #[cfg(target_arch = "wasm32")]
                ui.painter().text(
                    r.right_top() + vec2(-12.0, 15.0),
                    Align2::RIGHT_CENTER,
                    "Browser edition",
                    FontId::proportional(12.0),
                    Color32::from_gray(90),
                );
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
                let bounds = ui.max_rect();
                let compact = bounds.width() < 700.0;
                let zoom_width = if compact { 188.0 } else { 212.0 };
                let zoom_rect =
                    Rect::from_min_max(pos2(bounds.right() - zoom_width, bounds.top()), bounds.max);
                let information_rect =
                    Rect::from_min_max(bounds.min, pos2(zoom_rect.left() - 8.0, bounds.bottom()));
                let mut information = format!(
                    "{} × {} px",
                    self.doc.image.width(),
                    self.doc.image.height()
                );
                if let Some(region) = self.selected_region() {
                    information.push_str(&format!("  ·  {} × {} px selected", region.w, region.h));
                }
                if let Some((x, y)) = self.cursor {
                    information.push_str(&format!("  ·  {x}, {y} px"));
                }
                if bounds.width() >= 900.0 && !self.message.is_empty() {
                    information.push_str(&format!("  ·  {}", self.message));
                }
                ui.scope_builder(
                    UiBuilder::new()
                        .max_rect(information_rect)
                        .layout(Layout::left_to_right(Align::Center)),
                    |ui| {
                        ui.set_clip_rect(information_rect);
                        ui.add(Label::new(RichText::new(&information).size(12.0)).truncate())
                            .on_hover_text(&information);
                    },
                );
                ui.scope_builder(
                    UiBuilder::new()
                        .max_rect(zoom_rect)
                        .layout(Layout::right_to_left(Align::Center)),
                    |ui| {
                        ui.set_clip_rect(zoom_rect);
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.spacing_mut().button_padding = vec2(3.0, 1.0);
                        ui.spacing_mut().interact_size.y = 18.0;
                        ui.spacing_mut().slider_width = if compact { 84.0 } else { 108.0 };
                        ui.style_mut()
                            .text_styles
                            .insert(TextStyle::Button, FontId::proportional(12.0));
                        let zoom_in = ui
                            .add_sized([20.0, 20.0], Button::new("+"))
                            .on_hover_text("Zoom in");
                        zoom_in.widget_info(|| {
                            WidgetInfo::labeled(WidgetType::Button, true, "Zoom in")
                        });
                        if zoom_in.clicked() {
                            self.zoom = (self.zoom * 2.0).min(MAX_ZOOM);
                        }
                        let mut zoom = self.zoom * 100.;
                        let zoom_slider = ui.add(
                            Slider::new(&mut zoom, MIN_ZOOM * 100.0..=MAX_ZOOM * 100.0)
                                .logarithmic(true)
                                .show_value(false),
                        );
                        zoom_slider.widget_info(|| {
                            WidgetInfo::slider(true, zoom as f64, "Zoom percentage")
                        });
                        self.zoom = zoom / 100.;
                        let zoom_out = ui
                            .add_sized([20.0, 20.0], Button::new("−"))
                            .on_hover_text("Zoom out");
                        zoom_out.widget_info(|| {
                            WidgetInfo::labeled(WidgetType::Button, true, "Zoom out")
                        });
                        if zoom_out.clicked() {
                            self.zoom = (self.zoom / 2.0).max(MIN_ZOOM);
                        }
                        if ui
                            .add_sized(
                                [48.0, 20.0],
                                Button::new(format!("{:.0}%", self.zoom * 100.0)),
                            )
                            .on_hover_text("Reset to 100%")
                            .clicked()
                        {
                            self.zoom = 1.;
                        }
                    },
                );
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(app: &mut PaintApp, ctx: &Context, key: Option<Key>) -> FullOutput {
        ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(640.0, 400.0))),
                events: key
                    .map(|key| Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers::NONE,
                    })
                    .into_iter()
                    .collect(),
                ..Default::default()
            },
            |ctx| {
                keytips::begin_frame(ctx);
                keytips::keyboard(ctx, Id::new("canvas"));
                app.titlebar(ctx);
                app.quick_access_below(ctx);
                keytips::finish_frame(ctx);
            },
        )
    }

    #[test]
    fn window_title_updates_on_document_changes_without_repainting_idle_frames() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let titles = |output: FullOutput| {
            output.viewport_output[&ViewportId::ROOT]
                .commands
                .iter()
                .filter_map(|command| match command {
                    ViewportCommand::Title(title) => Some(title.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(titles(frame(&mut app, &ctx, None)), ["Untitled - Paint 10"]);
        assert!(titles(frame(&mut app, &ctx, None)).is_empty());
        app.doc.begin();
        d::stamp(&mut app.doc.image, (10, 10), 1, BLACK, Brush::Round);
        app.doc.commit();
        assert_eq!(
            titles(frame(&mut app, &ctx, None)),
            ["*Untitled - Paint 10"]
        );
        app.file = Some(PathBuf::from("portrait.p10"));
        app.doc.mark_saved();
        assert_eq!(
            titles(frame(&mut app, &ctx, None)),
            ["portrait.p10 - Paint 10"]
        );
        assert!(titles(frame(&mut app, &ctx, None)).is_empty());
    }

    #[test]
    fn quick_access_keytips_activate_real_buttons_and_respect_disabled_undo() {
        for below_ribbon in [false, true] {
            let ctx = Context::default();
            let mut app = PaintApp::new_with_context(&ctx, false);
            app.quick_access.commands = QuickCommand::ALL.to_vec();
            app.quick_access.below_ribbon = below_ribbon;
            app.doc.begin();
            d::stamp(&mut app.doc.image, (10, 10), 1, BLACK, Brush::Round);
            app.doc.commit();
            frame(&mut app, &ctx, None);
            frame(&mut app, &ctx, Some(Key::F10));
            frame(&mut app, &ctx, Some(Key::Num4));
            assert_eq!(app.doc.image.get_pixel(10, 10).0, WHITE);
            assert!(app.doc.can_redo());
            assert!(!keytips::active(&ctx));
            frame(&mut app, &ctx, Some(Key::F10));
            frame(&mut app, &ctx, Some(Key::Num4));
            assert!(!app.doc.can_undo());
            assert!(app.doc.can_redo());
            assert!(keytips::active(&ctx));
            frame(&mut app, &ctx, Some(Key::Escape));
        }
    }

    #[test]
    fn quick_access_customize_keytip_opens_the_real_menu() {
        let ctx = Context::default();
        ctx.enable_accesskit();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.quick_access = Default::default();
        frame(&mut app, &ctx, None);
        frame(&mut app, &ctx, Some(Key::F10));
        let output = frame(&mut app, &ctx, Some(Key::Num0));
        let nodes = &output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes;
        for command in QuickCommand::ALL {
            assert!(
                nodes.iter().any(|(_, node)| {
                    node.role() == egui::accesskit::Role::CheckBox
                        && node.label() == Some(command.name())
                }),
                "missing toolbar choice {}",
                command.name()
            );
        }
        frame(&mut app, &ctx, Some(Key::Escape));
        let output = frame(&mut app, &ctx, None);
        assert!(!output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .any(|(_, node)| node.role() == egui::accesskit::Role::CheckBox));
    }

    #[test]
    fn narrow_window_keeps_toolbar_caption_and_zoom_controls_separate() {
        let ctx = Context::default();
        ctx.enable_accesskit();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.quick_access.commands = QuickCommand::ALL.to_vec();
        app.quick_access.below_ribbon = false;
        app.file = Some(PathBuf::from("A long pixel art project name.p10"));
        app.cursor = Some((891, 592));
        app.selection = Some(Region {
            x: 20,
            y: 20,
            w: 750,
            h: 550,
        });
        app.zoom = MAX_ZOOM;
        let output = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(500.0, 400.0))),
                ..Default::default()
            },
            |ctx| {
                app.titlebar(ctx);
                app.status(ctx);
            },
        );
        let nodes = &output.platform_output.accesskit_update.unwrap().nodes;
        let bounds = |label: &str| {
            nodes
                .iter()
                .find(|(_, node)| node.label() == Some(label))
                .unwrap_or_else(|| panic!("missing control {label}"))
                .1
                .bounds()
                .unwrap()
        };
        let minimize = bounds("Minimize");
        for command in QuickCommand::ALL {
            let button = bounds(command.name());
            assert!(button.x0 >= 0.0 && button.x1 <= minimize.x0);
        }
        assert!(bounds("Customize Quick Access Toolbar").x1 <= minimize.x0);
        let controls = [
            bounds("3200%"),
            bounds("Zoom out"),
            bounds("Zoom percentage"),
            bounds("Zoom in"),
        ];
        for control in controls {
            assert!(control.x0 >= 0.0 && control.x1 <= 500.0);
            assert!(control.y0 >= 374.0 && control.y1 <= 400.0);
        }
        for pair in controls.windows(2) {
            assert!(pair[0].x1 <= pair[1].x0);
        }
        let information = nodes
            .iter()
            .find(|(_, node)| {
                node.value()
                    .is_some_and(|label| label.starts_with("900 × 600 px"))
            })
            .expect("canvas information")
            .1
            .bounds()
            .unwrap();
        assert!(information.x1 < controls[0].x0);
        assert_eq!(app.zoom, MAX_ZOOM);
    }
}
