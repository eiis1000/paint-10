use super::*;

fn frame(app: &mut PaintApp, ctx: &Context, events: Vec<Event>) -> FullOutput {
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

fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn keys(app: &mut PaintApp, ctx: &Context, keys: &[Key]) {
    for &pressed in keys {
        frame(app, ctx, vec![key(pressed, Modifiers::NONE)]);
    }
}

fn settle(app: &mut PaintApp, ctx: &Context) {
    for _ in 0..4 {
        frame(app, ctx, vec![]);
    }
}

fn shape_app(ctx: &Context) -> PaintApp {
    let mut app = PaintApp::new_with_context(ctx, false);
    app.doc = Document::new(120, 80);
    app.set_tool(Tool::Rectangle);
    app.doc.begin();
    app.start_shape_draft(
        ShapeGeometry::Primitive {
            tool: Tool::Rectangle,
            start: (20, 20),
            end: (70, 50),
        },
        0,
    );
    settle(&mut app, ctx);
    app
}

fn keyboard_platforms() -> [(egui::os::OperatingSystem, Modifiers); 4] {
    use egui::os::OperatingSystem::{Mac, Nix, Windows};

    // Events enter below egui-winit, so include its logical command flag.
    // Paint also supports physical Ctrl+A on Mac alongside native Command+A.
    [
        (Windows, Modifiers::CTRL | Modifiers::COMMAND),
        (Nix, Modifiers::CTRL | Modifiers::COMMAND),
        (Mac, Modifiers::CTRL | Modifiers::COMMAND),
        (Mac, Modifiers::MAC_CMD | Modifiers::COMMAND),
    ]
}

fn enter_custom_size(
    app: &mut PaintApp,
    ctx: &Context,
    value: &str,
    settle_popup: bool,
    select_all_modifiers: Modifiers,
) {
    keys(app, ctx, &[Key::F10, Key::H, Key::W]);
    if settle_popup {
        settle(app, ctx);
    }
    keys(app, ctx, &[Key::C]);
    settle(app, ctx);
    frame(app, ctx, vec![key(Key::A, select_all_modifiers)]);
    frame(app, ctx, vec![Event::Text(value.into())]);
    keys(app, ctx, &[Key::Enter]);
    settle(app, ctx);
}

#[test]
fn custom_width_shortcuts_preserve_the_full_supported_range() {
    for (os, select_all_modifiers) in keyboard_platforms() {
        let ctx = Context::default();
        ctx.set_os(os);
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.set_tool(Tool::Brush);
        settle(&mut app, &ctx);
        for (value, increased) in [(137, 138), (500, 500)] {
            enter_custom_size(
                &mut app,
                &ctx,
                &value.to_string(),
                true,
                select_all_modifiers,
            );
            assert_eq!(app.size, value);
            frame(&mut app, &ctx, vec![key(Key::Plus, Modifiers::CTRL)]);
            assert_eq!(app.size, increased);
            frame(&mut app, &ctx, vec![key(Key::Minus, Modifiers::CTRL)]);
            assert_eq!(app.size, increased - 1);
        }
    }
}

#[test]
fn immediate_custom_size_keytip_overrides_pending_popup_focus() {
    for (os, select_all_modifiers) in keyboard_platforms() {
        let ctx = Context::default();
        ctx.set_os(os);
        let mut app = PaintApp::new_with_context(&ctx, false);
        settle(&mut app, &ctx);
        enter_custom_size(&mut app, &ctx, "137", false, select_all_modifiers);
        assert_eq!(app.size, 137);
        assert!(app.selection.is_none());
        assert!(!app.doc.dirty());
    }
}

#[test]
fn immediate_custom_size_typing_preserves_text_after_queued_keytips() {
    for coalesced in [false, true] {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        settle(&mut app, &ctx);
        let mut batches = vec![];
        for pressed in [Key::F10, Key::H, Key::W, Key::C] {
            batches.push(vec![key(pressed, Modifiers::NONE)]);
        }
        for (pressed, text) in [(Key::Num1, "1"), (Key::Num3, "3"), (Key::Num7, "7")] {
            batches.push(vec![
                key(pressed, Modifiers::NONE),
                Event::Text(text.into()),
            ]);
        }
        batches.push(vec![key(Key::Enter, Modifiers::NONE)]);
        if coalesced {
            frame(&mut app, &ctx, batches.into_iter().flatten().collect());
        } else {
            for events in batches {
                frame(&mut app, &ctx, events);
            }
        }
        for _ in 0..24 {
            if !ctx.has_requested_repaint() {
                break;
            }
            frame(&mut app, &ctx, vec![]);
        }
        assert_eq!(app.size, 137, "coalesced input: {coalesced}");
        assert!(app.selection.is_none());
        assert!(!app.doc.dirty());
    }
}

#[test]
fn custom_rotation_cancel_preserves_the_draft_and_apply_targets_only_that_shape() {
    for (os, select_all_modifiers) in keyboard_platforms() {
        let ctx = Context::default();
        ctx.set_os(os);
        let mut app = shape_app(&ctx);
        let before = app.doc.composite();
        let selected = app.selected_image().unwrap();

        keys(
            &mut app,
            &ctx,
            &[Key::F10, Key::H, Key::R, Key::O, Key::Num6],
        );
        settle(&mut app, &ctx);
        assert!(matches!(app.dialog, Some(Dialog::Rotate)));
        keys(&mut app, &ctx, &[Key::Escape]);
        settle(&mut app, &ctx);
        assert!(app.dialog.is_none());
        assert!(app.shape_draft.is_some());
        assert_eq!(app.doc.composite(), before);
        assert!(!app.doc.can_undo());

        keys(
            &mut app,
            &ctx,
            &[Key::F10, Key::H, Key::R, Key::O, Key::Num6],
        );
        settle(&mut app, &ctx);
        frame(&mut app, &ctx, vec![key(Key::A, select_all_modifiers)]);
        frame(&mut app, &ctx, vec![Event::Text("90".into())]);
        keys(&mut app, &ctx, &[Key::Enter]);
        settle(&mut app, &ctx);
        assert!(app.dialog.is_none());
        assert_eq!(app.doc.image.dimensions(), (120, 80));
        assert!(app.shape_draft.is_none());
        assert_eq!(app.selected_image().unwrap(), imageops::rotate90(&selected));

        app.action(Action::Undo, &ctx);
        assert_eq!(app.doc.composite(), before);
        app.action(Action::Undo, &ctx);
        assert!(app.doc.image.pixels().all(|pixel| pixel.0 == WHITE));
        assert!(!app.doc.dirty());
    }
}

#[test]
fn invert_active_shape_selects_the_surroundings_and_delete_undo_preserves_the_shape() {
    let ctx = Context::default();
    let mut app = shape_app(&ctx);
    let before = app.doc.composite();
    keys(
        &mut app,
        &ctx,
        &[Key::F10, Key::H, Key::Z, Key::S, Key::Num4],
    );
    settle(&mut app, &ctx);
    let image = app.selected_image().unwrap();
    assert_eq!(image.dimensions(), (120, 80));
    assert_eq!(image.get_pixel(0, 0)[3], 255);
    assert_eq!(image.get_pixel(40, 35)[3], 0);
    assert!(app.shape_draft.is_none());
    assert_eq!(app.doc.composite(), before);

    app.colors[1] = [210, 40, 60, 255];
    frame(&mut app, &ctx, vec![key(Key::Delete, Modifiers::NONE)]);
    assert_eq!(app.doc.image.get_pixel(0, 0).0, app.colors[1]);
    assert_eq!(app.doc.image.get_pixel(40, 35), before.get_pixel(40, 35));
    app.action(Action::Undo, &ctx);
    assert_eq!(app.doc.composite(), before);
    app.action(Action::Undo, &ctx);
    assert!(app.doc.image.pixels().all(|pixel| pixel.0 == WHITE));
    assert!(!app.doc.dirty());
}
