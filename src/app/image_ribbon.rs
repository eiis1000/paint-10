//! Image commands stay together and remain available in compact ribbon groups.

use super::ribbon_controls as controls;
use super::ribbon_layout::Group;
use super::*;

impl PaintApp {
    pub(in crate::app) fn image_ribbon(&mut self, ui: &mut Ui, origin: Pos2, ctx: &Context) {
        let groups = [
            Group {
                label: "Transform",
                width: 238.0,
                icon: Icon::Resize,
                keys: "ZT",
                popup: "image_transform",
            },
            Group {
                label: "Crop",
                width: 182.0,
                icon: Icon::Crop,
                keys: "ZC",
                popup: "image_crop",
            },
            Group {
                label: "Adjust",
                width: 160.0,
                icon: Icon::Colors,
                keys: "ZA",
                popup: "image_adjust",
            },
            Group {
                label: "Original",
                width: 188.0,
                icon: Icon::Undo,
                keys: "ZO",
                popup: "image_original",
            },
        ];
        let widths = ribbon_layout::widths(&groups, ui.available_width(), &[3, 2, 1, 0]);
        let mut x = 0.0;
        for (index, (group, width)) in groups.into_iter().zip(widths).enumerate() {
            ribbon_layout::show(ui, origin + vec2(x, 0.0), width, "image", group, |ui, o| {
                Self::group(ui, o, 0.0, group.width - 1.0, group.label);
                match index {
                    0 => self.image_transform_group(ui, o, ctx),
                    1 => self.image_crop_group(ui, o, ctx),
                    2 => self.image_adjust_group(ui, o),
                    _ => self.image_original_group(ui, o),
                }
            });
            x += width;
        }
    }

    fn image_transform_group(&mut self, ui: &mut Ui, o: Pos2, ctx: &Context) {
        let resize = image_tile(ui, o + vec2(4.0, 5.0), "Resize", Icon::Resize, "S", true);
        if resize.on_hover_text("Resize and skew (Ctrl+W)").clicked() {
            // Opening a dialog must not finish a shape or text draft.
            self.execute(Action::Resize, ctx);
        }
        let left = o + vec2(78.0, 5.0);
        if image_row(ui, left, 154.0, "Rotate right 90°", Icon::Rotate, "R", true).clicked() {
            self.action(Action::Rotate(90.0), ctx);
        }
        if image_row(
            ui,
            left + vec2(0.0, 28.0),
            154.0,
            "Rotate left 90°",
            Icon::Rotate,
            "L",
            true,
        )
        .clicked()
        {
            self.action(Action::Rotate(270.0), ctx);
        }
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(
                left + vec2(0.0, 57.0),
                vec2(154.0, 27.0),
            )),
            |ui| {
                super::ribbon::ribbon_menu_button(
                    ui,
                    "Rotate and flip",
                    "F",
                    "image_rotate",
                    Some(Icon::Rotate),
                    |ui| {
                        for (label, action, key) in [
                            ("Rotate 180°", Action::Rotate(180.0), "1"),
                            ("Flip horizontal", Action::Flip(true), "H"),
                            ("Flip vertical", Action::Flip(false), "V"),
                        ] {
                            let response = ui.add(theme::MenuItem::new(label).width(230.0));
                            controls::register(ui, &response, key, keytips::Kind::Button);
                            if response.clicked() {
                                self.action(action, ctx);
                                ui.close_menu();
                            }
                        }
                        ui.separator();
                        let custom = ui.add(theme::MenuItem::new("Custom angle…").width(230.0));
                        controls::register(ui, &custom, "A", keytips::Kind::Button);
                        if custom.clicked() {
                            self.angle = self
                                .object
                                .map(|index| self.doc.objects[index].angle)
                                .unwrap_or(0.0);
                            self.dialog = Some(Dialog::Rotate);
                            ui.close_menu();
                        }
                    },
                );
            },
        );
    }

    fn image_crop_group(&mut self, ui: &mut Ui, o: Pos2, ctx: &Context) {
        if image_tile(
            ui,
            o + vec2(7.0, 5.0),
            "Crop image…",
            Icon::Crop,
            "C",
            self.image_edit_target().is_some(),
        )
        .on_hover_text("Trim an image while keeping its original pixels available for later edits.")
        .clicked()
        {
            self.dialog = Some(Dialog::ImageCrop);
        }
        if image_tile(
            ui,
            o + vec2(95.0, 5.0),
            "Crop picture",
            Icon::Crop,
            "P",
            self.selected_region().is_some(),
        )
        .on_hover_text("Crop the entire picture to the selection (Ctrl+Shift+X).")
        .clicked()
        {
            self.action(Action::Crop, ctx);
        }
    }

    fn image_adjust_group(&mut self, ui: &mut Ui, o: Pos2) {
        let target = self.image_edit_target();
        if image_row(
            ui,
            o + vec2(4.0, 5.0),
            151.0,
            "Adjust colors…",
            Icon::Colors,
            "A",
            target.is_some(),
        )
        .clicked()
        {
            self.dialog = Some(Dialog::ImageAdjustments);
        }
        for (row, label, key, grayscale) in [
            (1.0, "Grayscale", "G", true),
            (2.0, "Invert colors", "N", false),
        ] {
            if image_row(
                ui,
                o + vec2(4.0, 5.0 + row * 28.0),
                151.0,
                label,
                Icon::Colors,
                key,
                target.is_some(),
            )
            .clicked()
            {
                if let Some((mut edits, _)) = target {
                    if grayscale {
                        edits.grayscale = !edits.grayscale;
                    } else {
                        edits.invert = !edits.invert;
                    }
                    if let Err(error) = self.apply_image_edits(edits) {
                        self.message = error;
                    }
                }
            }
        }
    }

    fn image_original_group(&mut self, ui: &mut Ui, o: Pos2) {
        let target = self.object.and_then(|_| self.image_edit_target());
        if image_row(ui, o + vec2(4.0, 5.0), 179.0, "Reset image", Icon::Undo, "O", target.is_some())
            .on_hover_text("Restore the original image, including its size, crop, rotation and colors. Undo restores your edits.")
            .clicked()
        {
            if let Err(error) = self.reset_image_edits() {
                self.message = error;
            }
        }
        if let Some((_, (width, height))) = target {
            ui.painter().text(
                o + vec2(10.0, 45.0),
                Align2::LEFT_CENTER,
                format!("Original: {width} × {height} px"),
                FontId::proportional(12.0),
                Color32::from_gray(65),
            );
            ui.painter().text(
                o + vec2(10.0, 70.0),
                Align2::LEFT_CENTER,
                "Original pixels are preserved",
                FontId::proportional(11.0),
                Color32::from_gray(100),
            );
        } else {
            ui.painter().text(
                o + vec2(10.0, 51.0),
                Align2::LEFT_CENTER,
                "Select an image to restore",
                FontId::proportional(11.0),
                Color32::from_gray(100),
            );
        }
    }
}

fn image_tile(
    ui: &mut Ui,
    position: Pos2,
    label: &str,
    icon: Icon,
    keys: &str,
    enabled: bool,
) -> Response {
    let response = icons::button(
        ui,
        label,
        Rect::from_min_size(position, vec2(74.0, 82.0)),
        icon,
        label,
        false,
        enabled,
    );
    controls::register(ui, &response, keys, keytips::Kind::Button);
    if response.clicked() {
        ui.close_menu();
    }
    response
}

fn image_row(
    ui: &mut Ui,
    position: Pos2,
    width: f32,
    label: &str,
    icon: Icon,
    keys: &str,
    enabled: bool,
) -> Response {
    let rect = Rect::from_min_size(position, vec2(width, 26.0));
    let mut builder = UiBuilder::new().id_salt(label).max_rect(rect);
    if !enabled {
        builder = builder.disabled();
    }
    let mut child = ui.new_child(builder);
    let response = child.add_sized(rect.size(), Button::new("").frame(false));
    let mut painter = child.painter().clone();
    if !enabled {
        painter.set_opacity(0.4);
    }
    icons::draw(
        &painter,
        Rect::from_min_size(rect.min + vec2(4.0, 4.0), vec2(18.0, 18.0)),
        icon,
    );
    painter.text(
        rect.min + vec2(28.0, 13.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        Color32::from_gray(35),
    );
    if response.has_focus() {
        painter.rect_stroke(
            rect.shrink(1.0),
            0.0,
            Stroke::new(1.0_f32, BLUE),
            StrokeKind::Inside,
        );
    }
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, label));
    keytips::set_badge_anchor(ui, &response, rect.left_center() + vec2(14.0, 0.0));
    controls::register(ui, &response, keys, keytips::Kind::Button);
    if response.clicked() {
        ui.close_menu();
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(app: &mut PaintApp, ctx: &Context, width: f32, events: Vec<Event>) {
        let mut input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width, 760.0))),
            events,
            time: Some(ctx.cumulative_pass_nr() as f64 / 30.0),
            ..Default::default()
        };
        eframe::App::raw_input_hook(app, ctx, &mut input);
        let _ = ctx.run(input, |ctx| {
            if !app.ribbon_keyboard(ctx) {
                app.shortcut(ctx);
            }
            app.titlebar(ctx);
            app.ribbon(ctx);
            app.status(ctx);
            app.canvas(ctx);
            app.keyboard_menu(ctx);
            app.dialogs(ctx);
        });
    }

    fn keys(app: &mut PaintApp, ctx: &Context, width: f32, keys: &[Key]) {
        for &key in keys {
            frame(
                app,
                ctx,
                width,
                vec![Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }],
            );
            for _ in 0..3 {
                frame(app, ctx, width, vec![]);
            }
        }
    }

    #[test]
    fn image_dialog_keytips_work_in_expanded_and_compact_groups_without_changing_the_image() {
        for width in [1200.0, 520.0] {
            for (dialog, group, key) in [
                (Dialog::Resize, None, Key::S),
                (Dialog::ImageCrop, Some(Key::C), Key::C),
                (Dialog::ImageAdjustments, Some(Key::A), Key::A),
            ] {
                let ctx = Context::default();
                let mut app = PaintApp::new_with_context(&ctx, false);
                app.insert_image(RgbaImage::from_pixel(16, 12, Rgba([20, 80, 140, 255])));
                let objects = app.doc.objects.clone();
                let image = app.doc.composite();
                frame(&mut app, &ctx, width, vec![]);
                keys(&mut app, &ctx, width, &[Key::F10, Key::I]);
                assert!(app.image_tab);
                if width < 600.0 {
                    if let Some(group) = group {
                        keys(&mut app, &ctx, width, &[Key::Z, group]);
                    }
                }
                keys(&mut app, &ctx, width, &[key]);
                assert!(app.dialog == Some(dialog), "Image dialog at {width} px");
                keys(&mut app, &ctx, width, &[Key::Escape]);
                assert!(app.dialog.is_none());
                assert!(app.doc.objects == objects);
                assert_eq!(app.doc.composite(), image);
            }
        }
    }

    #[test]
    fn image_quick_adjustment_and_reset_keep_the_original_pixels() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let original = RgbaImage::from_pixel(16, 12, Rgba([20, 80, 140, 255]));
        app.insert_image(original.clone());
        frame(&mut app, &ctx, 1200.0, vec![]);
        keys(&mut app, &ctx, 1200.0, &[Key::F10, Key::I, Key::G]);
        let index = app.object.unwrap();
        assert!(app.doc.objects[index].image_edits.grayscale);
        assert!(app.doc.objects[index].kind == ObjectKind::Image(original.clone()));
        assert!(app.doc.objects[index]
            .render()
            .pixels()
            .all(|pixel| pixel[0] == pixel[1] && pixel[1] == pixel[2]));
        keys(&mut app, &ctx, 1200.0, &[Key::F10, Key::I, Key::O]);
        assert!(!app.doc.objects[index].image_edits.grayscale);
        assert_eq!(app.doc.objects[index].render(), original);
    }
}
