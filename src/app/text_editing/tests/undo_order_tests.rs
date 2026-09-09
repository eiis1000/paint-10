use super::*;

fn retained_caption(ctx: &Context) -> PaintApp {
    let mut app = PaintApp::new_with_context(ctx, false);
    let text = "Original rich caption";
    let mut format = crate::text::TextFormat {
        width: 600,
        ..Default::default()
    };
    format
        .modify_style(0..8, |style| {
            style.bold = true;
            style.color = [180, 25, 70, 255];
        })
        .unwrap();
    let index = app.doc.add_object(Object::new(
        ObjectKind::Text {
            text: text.into(),
            format,
        },
        (20, 20),
    ));
    app.doc.mark_saved();
    app.tool = Tool::Select;
    app.object = Some(index);
    app.edit_text_object(index);
    for index in 0..3 {
        timed_input_frame(&mut app, ctx, vec![], index as f64 * 0.05);
        assert!(
            app.text_edit.is_some(),
            "Editor closed in setup frame {index}"
        );
    }
    app
}

#[test]
fn trailing_text_and_immediate_undo_restore_the_original_rich_caption() {
    for undo_modifiers in [
        Modifiers::CTRL,
        Modifiers::CTRL | Modifiers::COMMAND,
        Modifiers::MAC_CMD | Modifiers::COMMAND,
    ] {
        let ctx = Context::default();
        if undo_modifiers.mac_cmd {
            ctx.set_os(egui::os::OperatingSystem::Mac);
        }
        let mut app = retained_caption(&ctx);
        let command = if undo_modifiers.mac_cmd {
            undo_modifiers
        } else {
            Modifiers::CTRL | Modifiers::COMMAND
        };
        timed_input_frame(&mut app, &ctx, vec![key(Key::A, command)], 0.2);
        let original = TextSnapshot::capture(app.text_edit.as_ref().unwrap());
        assert_eq!(original.selection, 0..original.text.chars().count());
        timed_input_frame(
            &mut app,
            &ctx,
            vec![Event::Text("Replacement tes".into())],
            0.3,
        );
        timed_input_frame(
            &mut app,
            &ctx,
            vec![Event::Text("t".into()), key(Key::Z, undo_modifiers)],
            0.4,
        );
        for frame in 0..4 {
            timed_input_frame(&mut app, &ctx, vec![], 0.5 + frame as f64 * 0.05);
        }
        let state = app.text_edit.as_ref().unwrap();
        assert_eq!(state.text, original.text);
        assert!(state.format == original.format);
        assert_eq!(state.selection, original.selection);
        assert!(
            !app.doc.dirty(),
            "editing has not committed the retained object"
        );
        timed_input_frame(&mut app, &ctx, vec![key(Key::Y, undo_modifiers)], 0.8);
        assert_eq!(app.text_edit.as_ref().unwrap().text, "Replacement test");
    }
}

fn settle_input(app: &mut PaintApp, ctx: &Context, start: f64) {
    for frame in 0..8 {
        timed_input_frame(app, ctx, vec![], start + frame as f64 * 0.01);
    }
}

#[test]
fn one_batch_preserves_text_undo_redo_and_following_text_order() {
    let ctx = Context::default();
    let mut app = retained_caption(&ctx);
    let command = Modifiers::CTRL | Modifiers::COMMAND;
    timed_input_frame(&mut app, &ctx, vec![key(Key::A, command)], 0.2);
    timed_input_frame(
        &mut app,
        &ctx,
        vec![
            Event::Text("A".into()),
            key(Key::Z, command),
            key(Key::Z, command | Modifiers::SHIFT),
            Event::Text("B".into()),
        ],
        0.3,
    );
    settle_input(&mut app, &ctx, 0.4);
    assert_eq!(app.text_edit.as_ref().unwrap().text, "AB");
    timed_input_frame(&mut app, &ctx, vec![key(Key::Z, command)], 0.6);
    assert_eq!(app.text_edit.as_ref().unwrap().text, "A");
    timed_input_frame(&mut app, &ctx, vec![key(Key::Z, command)], 0.7);
    assert_eq!(
        app.text_edit.as_ref().unwrap().text,
        "Original rich caption"
    );
}

#[test]
fn repeated_history_commands_in_one_batch_each_apply_once() {
    let ctx = Context::default();
    let mut app = retained_caption(&ctx);
    let command = Modifiers::CTRL | Modifiers::COMMAND;
    timed_input_frame(&mut app, &ctx, vec![key(Key::A, command)], 0.2);
    timed_input_frame(&mut app, &ctx, vec![Event::Text("A".into())], 0.3);
    timed_input_frame(&mut app, &ctx, vec![Event::Text("B".into())], 1.3);
    let mut repeated = key(Key::Z, command);
    if let Event::Key { repeat, .. } = &mut repeated {
        *repeat = true;
    }
    timed_input_frame(&mut app, &ctx, vec![key(Key::Z, command), repeated], 1.4);
    settle_input(&mut app, &ctx, 1.5);
    assert_eq!(
        app.text_edit.as_ref().unwrap().text,
        "Original rich caption"
    );
    timed_input_frame(
        &mut app,
        &ctx,
        vec![
            key(Key::Y, command),
            key(Key::Z, command | Modifiers::SHIFT),
        ],
        1.7,
    );
    settle_input(&mut app, &ctx, 1.8);
    assert_eq!(app.text_edit.as_ref().unwrap().text, "AB");
}

#[test]
fn confirmed_ime_text_then_typing_and_undo_keep_separate_history_steps() {
    let ctx = Context::default();
    let mut app = retained_caption(&ctx);
    let command = Modifiers::CTRL | Modifiers::COMMAND;
    timed_input_frame(&mut app, &ctx, vec![key(Key::A, command)], 0.2);
    timed_input_frame(
        &mut app,
        &ctx,
        vec![
            Event::Ime(ImeEvent::Enabled),
            Event::Ime(ImeEvent::Preedit("に".into())),
            Event::Ime(ImeEvent::Commit("日".into())),
            Event::Ime(ImeEvent::Disabled),
            Event::Text("!".into()),
            key(Key::Z, command),
        ],
        0.3,
    );
    settle_input(&mut app, &ctx, 0.4);
    assert_eq!(app.text_edit.as_ref().unwrap().text, "日");
    assert!(app
        .text_edit
        .as_ref()
        .unwrap()
        .history
        .composition
        .is_none());
    timed_input_frame(&mut app, &ctx, vec![key(Key::Z, command)], 0.6);
    assert_eq!(
        app.text_edit.as_ref().unwrap().text,
        "Original rich caption"
    );
}
