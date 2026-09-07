use super::*;

impl PaintApp {
    pub(in crate::app) fn shortcut(&mut self, ctx: &Context) {
        if self.dialog.is_some() || self.pending.is_some() {
            return;
        }
        // Menus own Escape and arrow keys. Closing a menu must not discard the
        // shape or text currently being edited underneath it.
        if keytips::popup_open(ctx) {
            return;
        }
        let temporary_ribbon = Id::new("paint10-ribbon-revealed");
        if ctx.data(|data| data.get_temp::<bool>(temporary_ribbon).unwrap_or(false))
            && ctx.input_mut(|i| consume_shortcut(i, Modifiers::NONE, Key::Escape))
        {
            ctx.data_mut(|data| data.insert_temp(temporary_ribbon, false));
            return;
        }
        for key in [Key::W, Key::Q] {
            if ctx.input_mut(|input| consume_shortcut(input, Modifiers::MAC_CMD, key)) {
                self.action(Action::Close, ctx);
                return;
            }
        }
        let global = [
            (Modifiers::CTRL, Key::N, Action::New),
            (Modifiers::CTRL, Key::O, Action::Open),
            (Modifiers::CTRL, Key::S, Action::Save),
            (Modifiers::CTRL | Modifiers::SHIFT, Key::S, Action::SaveAs),
            (Modifiers::CTRL, Key::W, Action::Resize),
            (Modifiers::CTRL, Key::E, Action::Properties),
            (Modifiers::CTRL | Modifiers::SHIFT, Key::X, Action::Crop),
            (
                Modifiers::CTRL | Modifiers::SHIFT,
                Key::V,
                Action::PasteFrom,
            ),
            (Modifiers::CTRL, Key::P, Action::Print),
            (
                Modifiers::CTRL | Modifiers::SHIFT,
                Key::N,
                Action::ClearPicture,
            ),
            (Modifiers::NONE, Key::F12, Action::SaveAs),
        ];
        for (mods, key, action) in global {
            if ctx.input_mut(|i| consume_shortcut(i, mods, key)) {
                self.action(action, ctx);
                return;
            }
        }
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::NONE, Key::F11)) {
            self.show_picture(ctx);
            return;
        }
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::NONE, Key::F1)) {
            self.dialog = Some(Dialog::About);
            return;
        }
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::CTRL, Key::F1)) {
            self.collapsed = !self.collapsed;
        }
        self.text_shortcuts(ctx);
        if self.text_edit.is_some() {
            return;
        }
        for (mods, key, action) in [
            (Modifiers::CTRL, Key::Z, Action::Undo),
            (Modifiers::CTRL, Key::Y, Action::Redo),
            (Modifiers::CTRL | Modifiers::SHIFT, Key::Z, Action::Redo),
            (Modifiers::CTRL, Key::A, Action::SelectAll),
            (Modifiers::CTRL | Modifiers::SHIFT, Key::I, Action::Invert),
        ] {
            if ctx.input_mut(|i| consume_shortcut(i, mods, key)) {
                self.action(action, ctx);
                return;
            }
        }
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::NONE, Key::Delete)) {
            self.delete_selection();
        }
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            match event {
                Event::Copy => {
                    self.copy();
                }
                Event::Cut => self.cut(),
                Event::Paste(_) => {
                    self.paste_clipboard();
                }
                _ => {}
            }
        }
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::NONE, Key::Escape)) {
            self.doc.cancel();
            self.curve = None;
            self.shape_draft = None;
            self.polygon.clear();
            self.gesture = None;
            self.clear_selection();
            self.preview = false;
            self.fullscreen = false;
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
            self.refresh = true;
        }
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::CTRL, Key::G)) {
            self.grid = !self.grid;
        }
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::CTRL, Key::R)) {
            self.rulers = !self.rulers;
        }
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::CTRL, Key::PageUp)) {
            self.zoom = (self.zoom * 2.0).min(MAX_ZOOM);
        }
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::CTRL, Key::PageDown)) {
            self.zoom = (self.zoom / 2.0).max(MIN_ZOOM);
        }
        if ctx.input_mut(|i| {
            consume_shortcut(i, Modifiers::CTRL, Key::Plus)
                || consume_shortcut(i, Modifiers::CTRL | Modifiers::SHIFT, Key::Plus)
                || consume_shortcut(i, Modifiers::CTRL, Key::Equals)
        }) {
            self.size = (self.size + 1).min(100);
        }
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::CTRL, Key::Minus)) {
            self.size = self.size.saturating_sub(1).max(1);
        }
        for (key, delta) in [
            (Key::ArrowLeft, (-1, 0)),
            (Key::ArrowRight, (1, 0)),
            (Key::ArrowUp, (0, -1)),
            (Key::ArrowDown, (0, 1)),
        ] {
            if ctx
                .memory(|memory| memory.focused().is_none() || memory.has_focus(Id::new("canvas")))
                && ctx.input_mut(|i| consume_shortcut(i, Modifiers::NONE, key))
            {
                if let Some(shape) = &self.shape_draft {
                    self.shape_draft = Some(shape.translated(delta));
                    self.redraw_shape();
                } else if let Some(i) = self.lift_selection() {
                    self.doc.objects[i].pos.0 += delta.0;
                    self.doc.objects[i].pos.1 += delta.1;
                    self.doc.commit();
                    self.refresh = true;
                }
            }
        }
    }
}

pub(super) fn consume_shortcut(input: &mut InputState, modifiers: Modifiers, key: Key) -> bool {
    let mut consumed = false;
    input.events.retain(|event| {
        let matched = matches!(event, Event::Key { key: pressed_key, modifiers: pressed_modifiers, pressed: true, .. }
            if *pressed_key == key && shortcut_modifiers_match(*pressed_modifiers, modifiers));
        consumed |= matched;
        !matched
    });
    consumed
}

fn shortcut_modifiers_match(pressed: Modifiers, expected: Modifiers) -> bool {
    // egui's command flag is Command on macOS and Ctrl on Windows/Linux.
    // Keep physical Ctrl working without treating Linux Super as Command.
    pressed.matches_exact(expected)
        || (expected.ctrl
            && !expected.command
            && !expected.mac_cmd
            && pressed.matches_exact(Modifiers {
                ctrl: false,
                command: true,
                ..expected
            }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_command_and_physical_control_keep_exact_shortcut_variants() {
        for pressed in [
            Modifiers::CTRL,
            Modifiers::CTRL | Modifiers::COMMAND,
            Modifiers::MAC_CMD | Modifiers::COMMAND,
        ] {
            for key in [Key::S, Key::N, Key::B, Key::E] {
                let mut input = InputState::default();
                input.events.push(Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: pressed | Modifiers::SHIFT,
                });
                assert!(!consume_shortcut(&mut input, Modifiers::CTRL, key));
                assert!(consume_shortcut(
                    &mut input,
                    Modifiers::CTRL | Modifiers::SHIFT,
                    key
                ));
            }
        }
        // egui-winit does not set command/mac_cmd for Super on Linux.
        assert!(!shortcut_modifiers_match(Modifiers::NONE, Modifiers::CTRL));
        assert!(!shortcut_modifiers_match(
            Modifiers::ALT | Modifiers::COMMAND,
            Modifiers::CTRL
        ));
    }

    #[test]
    fn native_command_opens_properties_and_control_remains_available() {
        for modifiers in [
            Modifiers::CTRL,
            Modifiers::CTRL | Modifiers::COMMAND,
            Modifiers::MAC_CMD | Modifiers::COMMAND,
            Modifiers::NONE,
        ] {
            let ctx = Context::default();
            let mut app = PaintApp::new_with_context(&ctx, false);
            let _ = ctx.run(
                RawInput {
                    events: vec![Event::Key {
                        key: Key::E,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    }],
                    ..Default::default()
                },
                |ctx| {
                    if !app.ribbon_keyboard(ctx) {
                        app.shortcut(ctx);
                    }
                    app.canvas(ctx);
                    app.dialogs(ctx);
                },
            );
            assert_eq!(
                matches!(app.dialog, Some(Dialog::Properties)),
                modifiers != Modifiers::NONE
            );
        }
    }

    #[test]
    fn native_command_close_keeps_the_unsaved_guard_and_control_w_resizes() {
        for key in [Key::W, Key::Q] {
            let ctx = Context::default();
            ctx.set_os(egui::os::OperatingSystem::Mac);
            let mut app = PaintApp::new_with_context(&ctx, false);
            app.doc.begin();
            app.doc.image.put_pixel(0, 0, Rgba(BLACK));
            app.doc.commit();
            let _ = ctx.run(
                RawInput {
                    events: vec![Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers::MAC_CMD | Modifiers::COMMAND,
                    }],
                    ..Default::default()
                },
                |ctx| {
                    if !app.ribbon_keyboard(ctx) {
                        app.shortcut(ctx);
                    }
                    app.canvas(ctx);
                    app.dialogs(ctx);
                },
            );
            assert!(matches!(app.pending, Some(Action::Close)));
            assert!(app.doc.dirty());
            assert!(!app.allow_close);
        }
        let ctx = Context::default();
        ctx.set_os(egui::os::OperatingSystem::Mac);
        let mut app = PaintApp::new_with_context(&ctx, false);
        let _ = ctx.run(
            RawInput {
                events: vec![Event::Key {
                    key: Key::W,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::CTRL,
                }],
                ..Default::default()
            },
            |ctx| app.shortcut(ctx),
        );
        assert!(matches!(app.dialog, Some(Dialog::Resize)));
        assert!(app.pending.is_none());
    }

    #[test]
    fn native_command_and_control_wheel_zoom_the_actual_canvas() {
        for modifiers in [
            Modifiers::CTRL,
            Modifiers::CTRL | Modifiers::COMMAND,
            Modifiers::MAC_CMD | Modifiers::COMMAND,
            Modifiers::NONE,
        ] {
            let ctx = Context::default();
            let mut app = PaintApp::new_with_context(&ctx, false);
            let initial = app.zoom;
            let _ = ctx.run(
                RawInput {
                    modifiers,
                    events: vec![Event::MouseWheel {
                        unit: MouseWheelUnit::Point,
                        delta: vec2(0.0, 12.0),
                        modifiers,
                    }],
                    ..Default::default()
                },
                |ctx| app.canvas(ctx),
            );
            assert_eq!(
                app.zoom,
                initial
                    * if modifiers == Modifiers::NONE {
                        1.0
                    } else {
                        1.25
                    }
            );
        }
    }

    #[test]
    fn a_selection_moves_when_the_canvas_has_keyboard_focus() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.selection = Some(Region {
            x: 10,
            y: 20,
            w: 30,
            h: 40,
        });
        let _ = ctx.run(RawInput::default(), |ctx| {
            app.canvas(ctx);
            ctx.memory_mut(|memory| memory.request_focus(Id::new("canvas")));
        });
        let _ = ctx.run(
            RawInput {
                events: vec![Event::Key {
                    key: Key::ArrowRight,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ctx| {
                app.shortcut(ctx);
                app.canvas(ctx);
            },
        );
        assert_eq!(app.doc.objects[app.object.unwrap()].pos, (11, 20));
    }

    #[test]
    fn arrow_keys_move_an_active_shape_without_committing_it() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.doc.begin();
        app.start_shape_draft(
            ShapeGeometry::Primitive {
                tool: Tool::Rectangle,
                start: (20, 20),
                end: (60, 60),
            },
            0,
        );
        let before = app.selected_region().unwrap();
        let _ = ctx.run(
            RawInput {
                events: vec![Event::Key {
                    key: Key::ArrowRight,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ctx| app.shortcut(ctx),
        );
        let after = app.selected_region().unwrap();
        assert_eq!(after.x, before.x + 1);
        assert_eq!(after.w, before.w);
        assert!(app.shape_draft.is_some());
        assert!(app.doc.objects.is_empty());
    }

    #[test]
    fn shifted_actions_are_not_consumed_by_plain_shortcuts() {
        for key in [Key::S, Key::X, Key::N, Key::V] {
            let mut input = InputState::default();
            input.events.push(Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::CTRL | Modifiers::COMMAND | Modifiers::SHIFT,
            });
            assert!(!consume_shortcut(&mut input, Modifiers::CTRL, key));
            assert!(consume_shortcut(
                &mut input,
                Modifiers::CTRL | Modifiers::SHIFT,
                key
            ));
            assert!(input.events.is_empty());
        }
    }
}
