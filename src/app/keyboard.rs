use super::shortcuts::consume_shortcut;
use super::*;

impl PaintApp {
    pub(in crate::app) fn show_picture(&mut self, ctx: &Context) {
        self.commit_text();
        self.finish_polygon();
        self.commit_shape();
        if self.curve.take().is_some() {
            self.doc.commit();
        }
        self.preview = true;
        self.fullscreen = true;
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(true));
    }

    pub(in crate::app) fn hide_picture(&mut self, ctx: &Context) {
        self.preview = false;
        self.fullscreen = false;
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
    }

    pub(in crate::app) fn ribbon_keyboard(&mut self, ctx: &Context) -> bool {
        keytips::begin_frame(ctx);
        if self.dialog.is_some() || self.pending.is_some() {
            keytips::cancel(ctx, false);
            self.keyboard_context_menu = false;
            return false;
        }
        let origin = if self.text_edit.is_some() {
            Id::new("text_input")
        } else {
            Id::new("canvas")
        };
        for (index, command) in self.quick_access.commands.clone().into_iter().enumerate() {
            let key = [
                Key::Num1,
                Key::Num2,
                Key::Num3,
                Key::Num4,
                Key::Num5,
                Key::Num6,
                Key::Num7,
                Key::Num8,
            ][index];
            if ctx.input_mut(|input| consume_shortcut(input, Modifiers::ALT, key)) {
                keytips::cancel(ctx, false);
                keytips::mark_alt_used(ctx);
                ctx.input_mut(|input| {
                    input
                        .events
                        .retain(|event| !matches!(event, Event::Text(_)))
                });
                self.quick_action(command, ctx);
                return true;
            }
        }
        if ctx.input_mut(|input| consume_shortcut(input, Modifiers::SHIFT, Key::F10)) {
            keytips::cancel(ctx, false);
            if self.open_layer_context_menu(ctx) {
                return true;
            }
            self.keyboard_context_menu = true;
            ctx.data_mut(|data| data.insert_temp(Id::new("paint10_context_initial_focus"), true));
            if let Some(state) = &mut self.text_edit {
                state.focus = false;
            }
            return true;
        }
        let backward = ctx.input_mut(|input| {
            consume_shortcut(input, Modifiers::CTRL | Modifiers::SHIFT, Key::Tab)
        });
        if backward || ctx.input_mut(|input| consume_shortcut(input, Modifiers::CTRL, Key::Tab)) {
            let count = if self.text_edit.is_some() { 4 } else { 3 };
            let current = if self.text_tab {
                3
            } else if self.image_tab {
                2
            } else {
                usize::from(self.view_tab)
            };
            let next = (current + if backward { count - 1 } else { 1 }) % count;
            self.text_tab = next == 3;
            self.image_tab = next == 2;
            self.view_tab = next == 1;
            if self.collapsed {
                ctx.data_mut(|data| data.insert_temp(Id::new("paint10-ribbon-revealed"), true));
            }
            keytips::switch_tab(ctx, ["home", "view", "image", "text"][next]);
            return true;
        }
        if ctx.input_mut(|input| {
            consume_shortcut(input, Modifiers::NONE, Key::F6)
                || consume_shortcut(input, Modifiers::SHIFT, Key::F6)
        }) {
            if ctx.memory(|memory| memory.focused().is_none() || memory.has_focus(origin)) {
                keytips::enter_ribbon(ctx, origin);
            } else {
                keytips::cancel(ctx, false);
                ctx.memory_mut(|memory| memory.request_focus(origin));
            }
            return true;
        }
        if self.keyboard_context_menu {
            return true;
        }
        let tab = if self.text_tab {
            "text"
        } else if self.image_tab {
            "image"
        } else if self.view_tab {
            "view"
        } else {
            "home"
        };
        keytips::current_tab(ctx, tab);
        let handled = keytips::keyboard(ctx, origin);
        if keytips::active(ctx) {
            if let Some(state) = &mut self.text_edit {
                state.focus = false;
            }
        }
        handled
    }

    pub(in crate::app) fn keyboard_menu(&mut self, ctx: &Context) {
        keytips::finish_frame(ctx);
        if self.keyboard_context_menu {
            let mut open = true;
            Window::new(if self.text_edit.is_some() {
                "Text"
            } else {
                "Selection"
            })
            .id(Id::new("keyboard_context_menu"))
            .order(Order::Foreground)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_pos(self.canvas_rect.min + vec2(20.0, 20.0))
            .show(ctx, |ui| self.selection_menu(ui, ctx));
            if !open || ctx.input(|input| input.key_pressed(Key::Escape)) {
                self.keyboard_context_menu = false;
                ctx.input_mut(|input| {
                    consume_shortcut(input, Modifiers::NONE, Key::Escape);
                });
                ctx.memory_mut(|memory| {
                    memory.request_focus(if self.text_edit.is_some() {
                        Id::new("text_input")
                    } else {
                        Id::new("canvas")
                    })
                });
            }
        }
    }

    pub(in crate::app) fn selection_menu(&mut self, ui: &mut Ui, ctx: &Context) {
        theme::menu(ui);
        if self.text_edit.is_some() {
            let selected = self
                .text_edit
                .as_ref()
                .is_some_and(|state| !state.selection.is_empty());
            for (label, shortcut, action, enabled) in [
                ("Cut", "Ctrl+X", Action::Cut, selected),
                ("Copy", "Ctrl+C", Action::Copy, selected),
                ("Paste", "Ctrl+V", Action::Paste, true),
                ("Delete", "Del", Action::Clear, selected),
                ("Select all", "Ctrl+A", Action::SelectAll, true),
            ] {
                let response =
                    ui.add_enabled(enabled, theme::MenuItem::new(label).shortcut(shortcut));
                self.focus_keyboard_context(ui, &response);
                if response.clicked() {
                    if matches!(action, Action::Clear | Action::SelectAll) {
                        self.text_selection_action(action, ctx);
                    } else {
                        self.action(action, ctx);
                    }
                    self.keyboard_context_menu = false;
                    ui.close_menu();
                }
            }
            return;
        }
        let selected = self.selected_region().is_some();
        for (label, shortcut, action, enabled) in [
            ("Cut", "Ctrl+X", Action::Cut, selected),
            ("Copy", "Ctrl+C", Action::Copy, selected),
            ("Paste", "Ctrl+V", Action::Paste, true),
            ("Delete", "Del", Action::Clear, selected),
            ("Select all", "Ctrl+A", Action::SelectAll, true),
            ("Crop", "Ctrl+Shift+X", Action::Crop, selected),
            ("Resize, skew, and rotate…", "Ctrl+W", Action::Resize, true),
            ("Invert colors", "Ctrl+Shift+I", Action::Invert, true),
        ] {
            if label == "Crop" {
                ui.separator();
            }
            let response = ui.add_enabled(enabled, theme::MenuItem::new(label).shortcut(shortcut));
            self.focus_keyboard_context(ui, &response);
            if response.clicked() {
                self.action(action, ctx);
                self.keyboard_context_menu = false;
                ui.close_menu();
            }
            if matches!(action, Action::Resize) {
                let mut rotate_label = egui::text::LayoutJob::simple_singleline(
                    "Rotate".into(),
                    FontId::proportional(13.0),
                    ui.visuals().text_color(),
                );
                // Match MenuItem's icon gutter without shifting the submenu arrow.
                rotate_label.sections[0].leading_space = 32.0 - ui.spacing().button_padding.x;
                let menu = ui.menu_button(rotate_label, |ui| {
                    theme::menu(ui);
                    for (label, action, icon) in [
                        ("Rotate right 90°", Action::Rotate(90.0), Icon::RotateRight),
                        ("Rotate left 90°", Action::Rotate(270.0), Icon::RotateLeft),
                        ("Rotate 180°", Action::Rotate(180.0), Icon::Rotate),
                        ("Flip vertical", Action::Flip(false), Icon::FlipVertical),
                        ("Flip horizontal", Action::Flip(true), Icon::FlipHorizontal),
                    ] {
                        if ui
                            .add(theme::MenuItem::new(label).icon(Some(icon)))
                            .clicked()
                        {
                            self.action(action, ctx);
                            self.keyboard_context_menu = false;
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    if ui.add(theme::MenuItem::new("Custom angle…")).clicked() {
                        self.angle = self
                            .object
                            .map(|index| self.doc.objects[index].angle)
                            .unwrap_or(0.0);
                        self.dialog = Some(Dialog::Rotate);
                        self.keyboard_context_menu = false;
                        ui.close_menu();
                    }
                });
                self.focus_keyboard_context(ui, &menu.response);
            }
        }
        if selected && ui.add(theme::MenuItem::new("Invert selection")).clicked() {
            self.invert_selection();
            self.keyboard_context_menu = false;
            ui.close_menu();
        }
        if self.image_edit_target().is_some() {
            ui.separator();
            for (label, dialog) in [
                ("Adjust image colors…", Dialog::ImageAdjustments),
                ("Crop image…", Dialog::ImageCrop),
            ] {
                let response = ui.add(theme::MenuItem::new(label));
                self.focus_keyboard_context(ui, &response);
                if response.clicked() {
                    self.dialog = Some(dialog);
                    self.keyboard_context_menu = false;
                    ui.close_menu();
                }
            }
        }
    }

    fn focus_keyboard_context(&self, ui: &Ui, response: &Response) {
        if self.keyboard_context_menu && response.enabled() && !ui.is_sizing_pass() {
            let initial = ui.ctx().data_mut(|data| {
                std::mem::take(
                    data.get_temp_mut_or_default::<bool>(Id::new("paint10_context_initial_focus")),
                )
            });
            if initial {
                response.request_focus();
                ui.ctx().request_repaint();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_alt_number_uses_custom_toolbar_order_without_opening_keytips() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.quick_access.commands = vec![
            crate::preferences::QuickCommand::Save,
            crate::preferences::QuickCommand::Undo,
        ];
        app.doc.begin();
        app.doc.image.put_pixel(10, 10, Rgba(BLACK));
        app.doc.commit();
        let _ = ctx.run(
            RawInput {
                events: vec![
                    Event::Key {
                        key: Key::Num2,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers::ALT,
                    },
                    Event::Text("2".into()),
                ],
                ..Default::default()
            },
            |ctx| {
                assert!(app.ribbon_keyboard(ctx));
                assert!(!ctx.input(|input| input
                    .events
                    .iter()
                    .any(|event| matches!(event, Event::Text(_)))));
            },
        );
        assert_eq!(*app.doc.image.get_pixel(10, 10), Rgba(WHITE));
        assert!(!keytips::active(&ctx));
    }
}
