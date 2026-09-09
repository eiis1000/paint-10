use super::*;

fn frame(app: &mut PaintApp, ctx: &Context, events: Vec<Event>) {
    let mut input = RawInput {
        events,
        time: Some(ctx.cumulative_pass_nr() as f64 / 30.0),
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 850.0))),
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

fn settle(app: &mut PaintApp, ctx: &Context) {
    for _ in 0..3 {
        frame(app, ctx, vec![]);
    }
}

fn key(app: &mut PaintApp, ctx: &Context, key: Key, modifiers: Modifiers) {
    frame(
        app,
        ctx,
        vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
    );
    settle(app, ctx);
}

fn click(app: &mut PaintApp, ctx: &Context, point: Point) {
    let position = app.canvas_rect.min + vec2(point.0 as f32, point.1 as f32) * app.zoom;
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec![
                Event::PointerMoved(position),
                Event::PointerButton {
                    pos: position,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
}

fn text_app(ctx: &Context) -> PaintApp {
    let mut app = PaintApp::new_with_context(ctx, false);
    app.doc = Document::new(600, 420);
    app.set_tool(Tool::Text);
    settle(&mut app, ctx);
    app
}

fn create_box(app: &mut PaintApp, ctx: &Context, origin: Point, text: &str) {
    click(app, ctx, origin);
    settle(app, ctx);
    let state = app.text_edit.as_ref().expect("Text click opens a new box");
    assert!(state.index.is_none());
    assert_eq!(state.origin, origin);
    frame(app, ctx, vec![Event::Text(text.into())]);
    assert_eq!(app.text_edit.as_ref().unwrap().text, text);
}

fn text(app: &PaintApp, index: usize) -> &str {
    app.doc
        .objects
        .iter()
        .filter_map(|object| match &object.kind {
            ObjectKind::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .nth(index)
        .expect("Completed text must remain editable")
}

fn text_count(app: &PaintApp) -> usize {
    app.doc
        .objects
        .iter()
        .filter(|object| matches!(object.kind, ObjectKind::Text { .. }))
        .count()
}

#[test]
fn completing_text_keeps_the_tool_for_successive_boxes_and_separate_undo_steps() {
    for click_outside in [false, true] {
        let ctx = Context::default();
        let mut app = text_app(&ctx);
        for (index, origin, content) in [
            (0, (20, 20), "First caption"),
            (1, (320, 140), "Second caption"),
        ] {
            create_box(&mut app, &ctx, origin, content);
            if click_outside {
                click(&mut app, &ctx, (100, 350));
                settle(&mut app, &ctx);
            } else {
                key(&mut app, &ctx, Key::Enter, Modifiers::CTRL);
            }
            assert!(app.text_edit.is_none());
            assert_eq!(app.tool, Tool::Text);
            assert_eq!(text_count(&app), index + 1);
            assert_eq!(text(&app, index), content);
            let selected = app.object.expect("Committed text remains selected");
            assert!(matches!(
                &app.doc.objects[selected].kind,
                ObjectKind::Text { text, .. } if text == content
            ));
        }
        key(&mut app, &ctx, Key::Z, Modifiers::CTRL);
        assert_eq!(text_count(&app), 1);
        assert_eq!(text(&app, 0), "First caption");
        key(&mut app, &ctx, Key::Z, Modifiers::CTRL);
        assert!(app.doc.objects.is_empty());
        assert!(!app.doc.dirty());
        assert_eq!(app.tool, Tool::Text);
        key(&mut app, &ctx, Key::Y, Modifiers::CTRL);
        assert_eq!(text(&app, 0), "First caption");
        assert_eq!(app.tool, Tool::Text);
    }
}

#[test]
fn explicit_tool_switches_still_commit_text_and_select_the_requested_tool() {
    for tool in [Tool::Brush, Tool::Select, Tool::Rectangle] {
        let ctx = Context::default();
        let mut app = text_app(&ctx);
        create_box(&mut app, &ctx, (20, 20), "Keep this caption");
        app.set_tool(tool);
        assert!(app.text_edit.is_none());
        assert_eq!(app.tool, tool);
        assert!(app.object.is_none());
        assert_eq!(text(&app, 0), "Keep this caption");
        app.action(Action::Undo, &ctx);
        assert!(app.doc.objects.is_empty());
        assert!(!app.doc.dirty());
        assert_eq!(app.tool, tool);
    }
}

#[test]
fn empty_and_canceled_text_keep_text_selected_without_document_edits() {
    for cancel in [false, true] {
        let ctx = Context::default();
        let mut app = text_app(&ctx);
        create_box(
            &mut app,
            &ctx,
            (20, 20),
            if cancel { "Discard me" } else { "" },
        );
        if cancel {
            key(&mut app, &ctx, Key::Escape, Modifiers::NONE);
        } else {
            key(&mut app, &ctx, Key::Enter, Modifiers::CTRL);
        }
        assert!(app.text_edit.is_none());
        assert_eq!(app.tool, Tool::Text);
        assert!(app.doc.objects.is_empty());
        assert!(!app.doc.dirty());
        assert!(!app.doc.can_undo());
        create_box(&mut app, &ctx, (320, 140), "Next caption");
    }
}

#[test]
fn double_click_editing_keeps_select_and_preserves_cancel_and_commit_history() {
    for cancel in [false, true] {
        let ctx = Context::default();
        let mut app = text_app(&ctx);
        create_box(&mut app, &ctx, (20, 20), "Original");
        key(&mut app, &ctx, Key::Enter, Modifiers::CTRL);
        app.doc.mark_saved();
        let index = app.object.unwrap();
        let original = app.doc.objects[index].clone();
        app.set_tool(Tool::Select);
        // The creation click must expire before this separate double-click.
        // Otherwise egui counts all three releases as a triple-click.
        for _ in 0..20 {
            frame(&mut app, &ctx, vec![]);
        }
        click(&mut app, &ctx, (45, 35));
        assert_eq!(app.object, Some(index), "First click selects retained text");
        click(&mut app, &ctx, (45, 35));
        assert!(ctx.input(|input| input.pointer.button_double_clicked(PointerButton::Primary)));
        assert!(
            ctx.read_response(Id::new("canvas"))
                .unwrap()
                .double_clicked(),
            "A completed click selection must not leave a stale resize handle at the pointer"
        );
        settle(&mut app, &ctx);
        let state = app
            .text_edit
            .as_ref()
            .expect("Double-click reopens retained text");
        assert_eq!(state.index, Some(index));
        assert_eq!(app.tool, Tool::Select);
        key(&mut app, &ctx, Key::A, Modifiers::CTRL | Modifiers::COMMAND);
        frame(&mut app, &ctx, vec![Event::Text("Revised".into())]);
        assert_eq!(app.text_edit.as_ref().unwrap().text, "Revised");
        if cancel {
            key(&mut app, &ctx, Key::Escape, Modifiers::NONE);
            assert!(app.doc.objects[index] == original);
            assert!(!app.doc.dirty());
        } else {
            key(&mut app, &ctx, Key::Enter, Modifiers::CTRL);
            assert_eq!(text(&app, 0), "Revised");
            app.action(Action::Undo, &ctx);
            assert!(app.doc.objects[index] == original);
            assert!(!app.doc.dirty());
        }
        assert!(app.text_edit.is_none());
        assert_eq!(app.tool, Tool::Select);
    }
}

#[test]
fn finishing_legacy_text_keeps_its_pixels_but_new_boxes_use_normal_point_sizes() {
    let ctx = Context::default();
    let mut app = text_app(&ctx);
    let original = Object::new(
        ObjectKind::Text {
            text: "Legacy caption".into(),
            format: crate::text::TextFormat {
                size_mode: crate::text::TextSizeMode::FontHeight,
                ..Default::default()
            },
        },
        (20, 20),
    );
    let index = app.doc.add_object(original.clone());
    app.doc.mark_saved();
    let pixels = app.doc.composite();
    app.set_tool(Tool::Select);
    app.edit_text_object(index);
    app.commit_text();
    assert!(app.doc.objects[index] == original);
    assert_eq!(app.doc.composite(), pixels);
    assert!(!app.doc.dirty());
    assert_eq!(app.text_format.size_mode, crate::text::TextSizeMode::Em);

    app.set_tool(Tool::Text);
    settle(&mut app, &ctx);
    create_box(&mut app, &ctx, (320, 140), "New caption");
    assert_eq!(
        app.text_edit.as_ref().unwrap().format.size_mode,
        crate::text::TextSizeMode::Em
    );
    assert!(app.doc.objects[index] == original);
}
