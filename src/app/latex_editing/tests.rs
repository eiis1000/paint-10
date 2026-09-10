use super::*;

fn editor_app(ctx: &Context, source: &str) -> PaintApp {
    let mut app = PaintApp::new_with_context(ctx, false);
    app.tool = Tool::Text;
    app.text_tab = true;
    app.text_edit = Some(TextEditState {
        index: None,
        origin: (20, 30),
        text: source.into(),
        format: TextFormat::default(),
        focus: true,
        selection: 0..source.chars().count(),
        insertion_style: None,
        history: Default::default(),
        palette_colors: app.colors,
    });
    app
}

fn frame(app: &mut PaintApp, ctx: &Context, events: Vec<Event>, size: Vec2) -> FullOutput {
    let mut input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
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
        app.status(ctx);
        app.canvas(ctx);
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

#[test]
fn equation_apply_reopen_edit_and_undo_preserve_sources_and_transforms() {
    let ctx = Context::default();
    let source = r"x = \frac{-b \pm \sqrt{b^2-4ac}}{2a}";
    let mut app = editor_app(&ctx, source);
    app.open_latex_editor();
    assert!(app.text_edit.is_none());
    assert!(app.latex_has_unsaved_changes());
    app.apply_latex().unwrap();
    app.dialog = None;
    assert_eq!(app.tool, Tool::Text);
    assert!(
        !app.text_format.latex,
        "ordinary Text must still create plain boxes"
    );
    // Adding a retained object first stores the white canvas as a raster below it.
    assert_eq!(app.doc.objects.len(), 2);
    let index = app.object.expect("the equation is selected");
    assert!(matches!(app.doc.objects[0].kind, ObjectKind::Raster(_)));
    assert!(
        matches!(&app.doc.objects[index].kind, ObjectKind::Text { text, format }
        if text == source && format.latex)
    );
    app.doc.objects[index].angle = 27.0;
    let saved = app.doc.objects[index].clone();
    app.edit_text_object(index);
    assert!(app.dialog == Some(Dialog::Latex));
    assert!(!app.latex_has_unsaved_changes());
    app.latex_edit.as_mut().unwrap().source = r"\sum_{k=1}^n k".into();
    assert!(app.latex_has_unsaved_changes());
    app.apply_latex().unwrap();
    assert_eq!(app.doc.objects[index].angle, 27.0);
    app.doc.undo();
    assert!(app.doc.objects[index] == saved);
    app.doc.undo();
    assert!(app.doc.objects.is_empty());
}

#[test]
fn invalid_equations_never_replace_original_text_and_cancel_restores_the_editor() {
    let ctx = Context::default();
    let mut app = editor_app(&ctx, "original words");
    app.open_latex_editor();
    app.latex_edit.as_mut().unwrap().source = r"\frac{".into();
    assert!(app.apply_latex().is_err());
    assert!(app.doc.objects.is_empty());
    assert!(app.latex_edit.is_some());
    app.cancel_latex();
    let editor = app.text_edit.as_ref().unwrap();
    assert_eq!(editor.text, "original words");
    assert_eq!(editor.selection, 0..14);
    assert!(!editor.format.latex);
    assert!(editor.focus);
    assert!(!app.doc.can_undo());
}

#[test]
fn opening_source_accepts_immediate_typing_and_enter_inserts_a_newline() {
    for size in [vec2(1180.0, 800.0), vec2(500.0, 400.0)] {
        let ctx = Context::default();
        let mut app = editor_app(&ctx, "old source");
        app.open_latex_editor();
        frame(&mut app, &ctx, vec![Event::Text("x^2".into())], size);
        for _ in 0..5 {
            frame(&mut app, &ctx, vec![], size);
        }
        assert_eq!(app.latex_edit.as_ref().unwrap().source, "x^2");
        frame(&mut app, &ctx, vec![key(Key::Enter, Modifiers::NONE)], size);
        for _ in 0..3 {
            frame(&mut app, &ctx, vec![], size);
        }
        assert!(app.dialog == Some(Dialog::Latex));
        assert_eq!(app.latex_edit.as_ref().unwrap().source, "x^2\n");
        frame(
            &mut app,
            &ctx,
            vec![key(Key::Escape, Modifiers::NONE)],
            size,
        );
        assert!(app.dialog.is_none());
        assert!(app.latex_edit.is_none());
        assert_eq!(app.text_edit.as_ref().unwrap().text, "old source");
    }
}

#[test]
fn source_shortcut_applies_on_native_command_and_control_platforms() {
    use egui::os::OperatingSystem::{Mac, Nix, Windows};
    for (os, modifiers) in [
        (Nix, Modifiers::CTRL | Modifiers::COMMAND),
        (Windows, Modifiers::CTRL | Modifiers::COMMAND),
        (Mac, Modifiers::MAC_CMD | Modifiers::COMMAND),
    ] {
        let ctx = Context::default();
        ctx.set_os(os);
        let mut app = editor_app(&ctx, "x^2");
        app.open_latex_editor();
        for _ in 0..5 {
            frame(&mut app, &ctx, vec![], vec2(1180.0, 800.0));
        }
        frame(
            &mut app,
            &ctx,
            vec![key(Key::Enter, modifiers)],
            vec2(1180.0, 800.0),
        );
        assert!(app.dialog.is_none());
        assert!(app.latex_edit.is_none());
        assert_eq!(app.doc.objects.len(), 2);
        let index = app.object.expect("the equation is selected");
        assert!(matches!(app.doc.objects[0].kind, ObjectKind::Raster(_)));
        assert!(
            matches!(&app.doc.objects[index].kind, ObjectKind::Text { text, format }
            if text == "x^2" && format.latex)
        );
    }
}

#[test]
fn equation_draft_cannot_change_a_hidden_or_locked_layer() {
    let ctx = Context::default();
    let mut app = editor_app(&ctx, "x");
    app.doc.active_layer_mut().locked = true;
    app.open_latex_editor();
    assert!(app.latex_edit.is_none());
    assert!(app.text_edit.is_some());
    app.doc.active_layer_mut().locked = false;
    app.open_latex_editor();
    app.doc.active_layer_mut().visible = false;
    assert!(app.apply_latex().is_err());
    assert!(app.doc.objects.is_empty());
}

#[test]
fn applied_equation_drags_and_reopens_without_leaving_the_text_tool() {
    let ctx = Context::default();
    let size = vec2(1180.0, 800.0);
    let mut app = editor_app(&ctx, r"\frac{a}{b}");
    app.text_edit.as_mut().unwrap().format.minimum_height = 100;
    app.open_latex_editor();
    app.apply_latex().unwrap();
    app.dialog = None;
    let index = app.object.unwrap();
    let before = app.doc.objects[index].clone();
    for _ in 0..4 {
        frame(&mut app, &ctx, vec![], size);
    }
    let start = app.canvas_rect.min + vec2(100.5, 70.5) * app.zoom;
    let end = start + vec2(40.0, 35.0) * app.zoom;
    let hover = frame(&mut app, &ctx, vec![Event::PointerMoved(start)], size);
    assert_eq!(hover.platform_output.cursor_icon, CursorIcon::Move);
    for (position, pressed) in [(start, true), (end, false)] {
        frame(
            &mut app,
            &ctx,
            vec![
                Event::PointerMoved(position),
                Event::PointerButton {
                    pos: position,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
            size,
        );
    }
    let mut moved = before.clone();
    moved.pos = (before.pos.0 + 40, before.pos.1 + 35);
    assert!(app.doc.objects[index] == moved);
    assert_eq!(app.tool, Tool::Text);
    assert!(app.text_edit.is_none());
    assert!(app.gesture.is_none());

    // A source edit remains a double-click, distinct from the completed drag.
    for _ in 0..20 {
        frame(&mut app, &ctx, vec![], size);
    }
    for _ in 0..2 {
        let events = [true, false]
            .into_iter()
            .map(|pressed| Event::PointerButton {
                pos: end,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            })
            .collect();
        frame(&mut app, &ctx, events, size);
    }
    for _ in 0..3 {
        frame(&mut app, &ctx, vec![], size);
    }
    assert!(app.dialog == Some(Dialog::Latex));
    assert_eq!(app.latex_edit.as_ref().unwrap().source, r"\frac{a}{b}");
    assert!(app.doc.objects[index] == moved);
    app.cancel_latex();
    app.dialog = None;
    app.doc.undo();
    assert!(app.doc.objects[index] == before);
}
