use super::*;

fn command(ui: &mut Ui, label: &str, shortcut: &str, key: &str, enabled: bool) -> Response {
    let response = ui.add_enabled(enabled, Button::new(label).shortcut_text(shortcut));
    keytips::register(ui, &response, "file", "File", key, keytips::Kind::Button);
    response
}

impl PaintApp {
    pub(in crate::app) fn file_menu(&mut self, ui: &mut Ui, ctx: &Context) {
        ui.set_min_width(232.0);
        ScrollArea::vertical()
            .id_salt("file_menu_scroll")
            .max_height((ctx.screen_rect().height() - 70.0).max(100.0))
            .show(ui, |ui| self.file_menu_contents(ui, ctx));
    }

    fn file_menu_contents(&mut self, ui: &mut Ui, ctx: &Context) {
        for (label, shortcut, key, action) in [
            ("New", "Ctrl+N", "N", Action::New),
            ("Open", "Ctrl+O", "O", Action::Open),
            ("Save", "Ctrl+S", "S", Action::Save),
            ("Save as…", "F12", "A", Action::SaveAs),
        ] {
            if command(ui, label, shortcut, key, true).clicked() {
                ui.close_menu();
                self.action(action, ctx);
            }
        }
        if command(ui, "Save a copy…", "", "Y", true).clicked() {
            ui.close_menu();
            self.export_copy(ctx, false);
        }
        let has_selection = self.selected_region().is_some()
            || self
                .object
                .and_then(|index| self.doc.objects.get(index))
                .is_some_and(|object| object.rendered_dimensions().is_some())
            || self
                .text_edit
                .as_ref()
                .is_some_and(|edit| !edit.text.is_empty());
        if command(ui, "Save selection as…", "", "L", has_selection).clicked() {
            ui.close_menu();
            self.export_copy(ctx, true);
        }
        ui.separator();
        if command(ui, "Print…", "Ctrl+P", "P", true).clicked() {
            self.action(Action::Print, ctx);
            ui.close_menu();
        }
        if command(ui, "Print preview", "", "V", true).clicked() {
            self.finish_editing();
            self.print_preview = Some(crate::print_preview::PrintPreview::new(
                self.doc.composite(),
            ));
            ui.close_menu();
        }
        if command(ui, "Page setup…", "", "U", true).clicked() {
            self.finish_editing();
            self.dialog = Some(Dialog::Print);
            ui.close_menu();
        }
        if command(ui, "From scanner or camera…", "", "C", self.job.is_none()).clicked() {
            self.dialog = Some(Dialog::Import);
            self.start_job(ctx, || {
                JobResult::Devices(crate::integration::enumerate_devices())
            });
            ui.close_menu();
        }
        if command(ui, "Send in email…", "", "E", self.job.is_none()).clicked() {
            self.finish_editing();
            let image = self.doc.composite();
            self.start_job(ctx, move || {
                JobResult::Status(
                    crate::integration::compose_email(&image).map(|_| "Email draft opened".into()),
                )
            });
            ui.close_menu();
        }
        if command(ui, "Set as desktop background…", "", "B", true).clicked() {
            self.finish_editing();
            if let Some(size) = ctx.input(|input| input.viewport().monitor_size) {
                self.wallpaper_size = (size.x as u32, size.y as u32);
            }
            self.dialog = Some(Dialog::Wallpaper);
            ui.close_menu();
        }
        if command(ui, "Properties", "Ctrl+E", "R", true).clicked() {
            self.action(Action::Properties, ctx);
            ui.close_menu();
        }
        if command(ui, "About Paint 10", "", "T", true).clicked() {
            self.dialog = Some(Dialog::About);
            ui.close_menu();
        }
        if !self.recent.is_empty() {
            ui.separator();
            ui.label("Recent pictures");
            for (index, path) in self.recent.clone().into_iter().enumerate() {
                let key = ((index + 1) % 10).to_string();
                let label = path.file_name().unwrap_or_default().to_string_lossy();
                if command(ui, &label, "", &key, true)
                    .on_hover_text(path.display().to_string())
                    .clicked()
                {
                    self.pending_path = Some(path);
                    self.action(Action::Open, ctx);
                    ui.close_menu();
                }
            }
        }
        ui.separator();
        if command(ui, "Exit", "", "X", true).clicked() {
            self.action(Action::Close, ctx);
            ui.close_menu();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(app: &mut PaintApp, ctx: &Context, key: Option<Key>) -> FullOutput {
        frame_at_size(app, ctx, key, vec2(640.0, 600.0))
    }

    fn frame_at_size(
        app: &mut PaintApp,
        ctx: &Context,
        key: Option<Key>,
        size: Vec2,
    ) -> FullOutput {
        ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
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
                TopBottomPanel::top("file_tabs").show(ctx, |ui| {
                    let menu = egui::menu::menu_custom_button(ui, Button::new("File"), |ui| {
                        app.file_menu(ui, ctx);
                    });
                    keytips::register(
                        ui,
                        &menu.response,
                        "tabs",
                        "Tabs",
                        "F",
                        keytips::Kind::Menu { scope: "file" },
                    );
                });
                keytips::finish_frame(ctx);
            },
        )
    }

    #[test]
    fn file_keytips_use_real_menu_actions_and_recent_file_numbers() {
        let ctx = Context::default();
        ctx.enable_accesskit();
        let mut app = PaintApp::new_with_context(&ctx, false);
        frame(&mut app, &ctx, None);
        frame(&mut app, &ctx, Some(Key::F10));
        frame(&mut app, &ctx, Some(Key::F));
        frame(&mut app, &ctx, Some(Key::V));
        assert!(app.print_preview.is_some());
        assert!(!keytips::active(&ctx));

        app.print_preview = None;
        app.doc.begin();
        d::stamp(&mut app.doc.image, (10, 10), 1, BLACK, Brush::Round);
        app.doc.commit();
        let path = PathBuf::from("/tmp/paint10-recent-keytip-test.png");
        app.recent = vec![path.clone()];
        frame(&mut app, &ctx, None);
        frame(&mut app, &ctx, Some(Key::F10));
        frame(&mut app, &ctx, Some(Key::F));
        frame(&mut app, &ctx, Some(Key::Num1));
        assert_eq!(app.pending_path, Some(path));
        assert!(matches!(app.pending, Some(Action::Open)));
        assert_eq!(app.doc.image.get_pixel(10, 10).0, BLACK);
    }

    #[test]
    fn file_export_choices_expose_selection_availability() {
        let ctx = Context::default();
        ctx.enable_accesskit();
        let mut app = PaintApp::new_with_context(&ctx, false);
        frame(&mut app, &ctx, None);
        frame(&mut app, &ctx, Some(Key::F10));
        frame(&mut app, &ctx, Some(Key::F));
        let output = frame(&mut app, &ctx, Some(Key::L));
        assert!(keytips::active(&ctx));
        let nodes = &output.platform_output.accesskit_update.unwrap().nodes;
        for (label, disabled) in [("Save a copy…", false), ("Save selection as…", true)] {
            let node = &nodes
                .iter()
                .find(|(_, node)| node.label() == Some(label))
                .unwrap()
                .1;
            assert_eq!(node.is_disabled(), disabled, "{label}");
        }
        app.selection = Some(Region {
            x: 0,
            y: 0,
            w: 8,
            h: 8,
        });
        let output = frame(&mut app, &ctx, None);
        assert!(output
            .platform_output
            .accesskit_update
            .unwrap()
            .nodes
            .iter()
            .any(|(_, node)| {
                node.label() == Some("Save selection as…") && !node.is_disabled()
            }));
        app.clear_selection();
        let index = app.doc.add_object(Object::new(
            ObjectKind::Image(RgbaImage::from_pixel(3, 2, Rgba(BLACK))),
            (2000, 2000),
        ));
        app.select_object(index);
        let output = frame(&mut app, &ctx, None);
        assert!(output
            .platform_output
            .accesskit_update
            .unwrap()
            .nodes
            .iter()
            .any(|(_, node)| {
                node.label() == Some("Save selection as…") && !node.is_disabled()
            }));
    }

    #[test]
    fn short_window_can_activate_the_last_recent_file() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.doc.begin();
        d::stamp(&mut app.doc.image, (0, 0), 1, BLACK, Brush::Round);
        app.doc.commit();
        app.recent = (1..=10)
            .map(|index| PathBuf::from(format!("/tmp/recent-picture-{index}.png")))
            .collect();
        for key in [None, Some(Key::F10), Some(Key::F), Some(Key::Num0)] {
            frame_at_size(&mut app, &ctx, key, vec2(500.0, 400.0));
        }
        assert_eq!(app.pending_path, app.recent.last().cloned());
        assert!(matches!(app.pending, Some(Action::Open)));
        assert!(app.doc.dirty());
    }
}
