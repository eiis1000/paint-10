use super::*;

impl PaintApp {
    pub(in crate::app) fn shortcut(&mut self, ctx: &Context) {
        if self.dialog.is_some() || self.pending.is_some() {
            return;
        }
        if self.layer_panel_owns_keyboard(ctx) {
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
        if self.measure_shortcuts(ctx) {
            return;
        }
        if !self.measure.enabled {
            self.text_shortcuts(ctx);
        }
        if self.text_edit.is_some() && !self.measure.enabled {
            return;
        }
        if !self.measure.enabled
            && self.shape_draft.is_some()
            && self.gesture.is_none()
            && ctx
                .memory(|memory| memory.focused().is_none() || memory.has_focus(Id::new("canvas")))
            && ctx.input_mut(|input| consume_shortcut(input, Modifiers::NONE, Key::Enter))
        {
            self.commit_shape();
            self.message = "Shape applied".into();
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
                    #[cfg(not(target_arch = "wasm32"))]
                    self.paste_clipboard();
                    #[cfg(target_arch = "wasm32")]
                    {
                        self.message =
                            "Clipboard contains text. Create or edit a text box to paste it."
                                .into();
                    }
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
            self.size = (self.size + 1).min(500);
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

pub(super) fn shortcut_modifiers_match(pressed: Modifiers, expected: Modifiers) -> bool {
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

    fn shape_frame(app: &mut PaintApp, ctx: &Context, events: Vec<Event>) -> FullOutput {
        let mut input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 850.0))),
            time: Some(ctx.cumulative_pass_nr() as f64 / 30.0),
            events,
            ..Default::default()
        };
        eframe::App::raw_input_hook(app, ctx, &mut input);
        ctx.run(input, |ctx| {
            if !app.ribbon_keyboard(ctx) {
                app.shortcut(ctx);
            }
            app.titlebar(ctx);
            app.ribbon(ctx);
            app.quick_access_below(ctx);
            app.status(ctx);
            app.canvas(ctx);
            app.keyboard_menu(ctx);
            app.dialogs(ctx);
        })
    }

    fn enter(modifiers: Modifiers) -> Event {
        Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn enter_applies_a_drawn_shape_before_subsequent_color_changes() {
        let context = Context::default();
        context.enable_accesskit();
        let mut app = PaintApp::new_with_context(&context, false);
        app.doc = Document::new(240, 180);
        app.colors = [[210, 40, 60, 255], WHITE];
        let mut output = shape_frame(&mut app, &context, vec![]);
        for _ in 0..2 {
            output = shape_frame(&mut app, &context, vec![]);
        }
        let bounds = output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some(Tool::Rectangle.name()))
            .unwrap()
            .1
            .bounds()
            .unwrap();
        let tool_position = pos2(
            ((bounds.x0 + bounds.x1) / 2.0) as f32,
            ((bounds.y0 + bounds.y1) / 2.0) as f32,
        );
        shape_frame(
            &mut app,
            &context,
            vec![
                Event::PointerMoved(tool_position),
                Event::PointerButton {
                    pos: tool_position,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
                Event::PointerButton {
                    pos: tool_position,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        assert_eq!(app.tool, Tool::Rectangle);
        let start = app.canvas_rect.min + vec2(20.5, 20.5) * app.zoom;
        let end = app.canvas_rect.min + vec2(100.5, 80.5) * app.zoom;
        shape_frame(
            &mut app,
            &context,
            vec![
                Event::PointerMoved(start),
                Event::PointerButton {
                    pos: start,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
                Event::PointerMoved(end),
                Event::PointerButton {
                    pos: end,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        assert!(app.shape_draft.is_some());
        assert!(app.gesture.is_none());
        let drawn = app.doc.composite();
        assert!(drawn.pixels().any(|pixel| pixel.0 == app.colors[0]));
        shape_frame(&mut app, &context, vec![enter(Modifiers::NONE)]);
        assert!(app.shape_draft.is_none());
        app.colors[0] = [30, 100, 220, 255];
        for _ in 0..2 {
            shape_frame(&mut app, &context, vec![]);
        }
        assert!(app.rendered == drawn);
        app.doc.undo();
        assert!(app.doc.composite().pixels().all(|pixel| pixel.0 == WHITE));
        assert!(!app.doc.can_undo());
    }

    #[test]
    fn shape_enter_leaves_modified_keys_and_focused_controls_available() {
        for (modifiers, focus) in [
            (Modifiers::SHIFT, None),
            (Modifiers::CTRL, None),
            (Modifiers::NONE, Some(Id::new("ribbon-control"))),
        ] {
            let context = Context::default();
            let mut app = PaintApp::new_with_context(&context, false);
            app.doc.begin();
            app.start_shape_draft(
                ShapeGeometry::Primitive {
                    tool: Tool::Rectangle,
                    start: (20, 20),
                    end: (60, 60),
                },
                0,
            );
            let _ = context.run(
                RawInput {
                    events: vec![enter(modifiers)],
                    ..Default::default()
                },
                |context| {
                    if let Some(focus) = focus {
                        context.memory_mut(|memory| memory.request_focus(focus));
                    }
                    app.shortcut(context);
                    assert!(context.input(|input| input.key_pressed(Key::Enter)));
                },
            );
            assert!(app.shape_draft.is_some());
        }
    }

    #[test]
    fn shape_enter_does_not_bypass_modal_popup_or_active_drag_ownership() {
        for mode in ["dialog", "pending", "popup", "drag"] {
            let context = Context::default();
            let mut app = PaintApp::new_with_context(&context, false);
            app.doc.begin();
            app.start_shape_draft(
                ShapeGeometry::Primitive {
                    tool: Tool::Rectangle,
                    start: (20, 20),
                    end: (60, 60),
                },
                0,
            );
            match mode {
                "dialog" => app.dialog = Some(Dialog::Properties),
                "pending" => app.pending = Some(Action::New),
                "drag" => {
                    app.gesture = Some(Gesture::MoveShape {
                        start: (20, 20),
                        original: app.shape_draft.clone().unwrap(),
                    });
                }
                _ => {}
            }
            let _ = context.run(
                RawInput {
                    events: vec![enter(Modifiers::NONE)],
                    ..Default::default()
                },
                |context| {
                    if mode == "popup" {
                        context.memory_mut(|memory| memory.open_popup(Id::new("shape-popup")));
                    }
                    app.shortcut(context);
                    assert!(context.input(|input| input.key_pressed(Key::Enter)));
                },
            );
            assert!(app.shape_draft.is_some(), "{mode}");
        }
    }

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
