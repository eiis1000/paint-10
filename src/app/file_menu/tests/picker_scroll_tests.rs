use super::*;

const SIZE: Vec2 = vec2(500.0, 400.0);

fn settle(app: &mut PaintApp, ctx: &Context) -> FullOutput {
    for _ in 0..12 {
        frame_at_size(app, ctx, None, SIZE);
    }
    frame_at_size(app, ctx, None, SIZE)
}

fn opened(ctx: &Context, label: &str) -> (PaintApp, FullOutput) {
    let mut app = PaintApp::new_with_context(ctx, false);
    app.recent = (1..=10)
        .map(|index| PathBuf::from(format!("/tmp/recent-{index}.png")))
        .collect();
    for code in [None, Some(Key::F10), Some(Key::F)] {
        frame_at_size(&mut app, ctx, code, SIZE);
    }
    let mut output = settle(&mut app, ctx);
    if label == "PNG picture" {
        let save = bounds(&output, "Save as…").center();
        frame_events(&mut app, ctx, vec![Event::PointerMoved(save)], SIZE);
        output = settle(&mut app, ctx);
    }
    (app, output)
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn file_picker_rows_do_not_scroll_from_movement_before_a_press() {
    for label in ["New", "1  recent-1.png", "PNG picture"] {
        let ctx = Context::default();
        ctx.enable_accesskit();
        let (mut app, output) = opened(&ctx, label);
        let initial = bounds(&output, label);
        let target = initial.center();
        frame_events(
            &mut app,
            &ctx,
            vec![Event::PointerMoved(target + vec2(0.0, 170.0))],
            SIZE,
        );
        frame_events(
            &mut app,
            &ctx,
            vec![Event::PointerMoved(target), button(target, true)],
            SIZE,
        );
        let output = settle(&mut app, &ctx);
        assert_eq!(bounds(&output, label), initial, "{label} moved on press");
        // Release outside the menu: this geometry test must not open a file
        // chooser or execute one of the commands under inspection.
        frame_events(
            &mut app,
            &ctx,
            vec![
                Event::PointerMoved(pos2(499.0, 399.0)),
                button(pos2(499.0, 399.0), false),
            ],
            SIZE,
        );
        assert!(app.pending.is_none());
        assert!(app.file.is_none());
        assert!(!app.doc.dirty());
    }
}

#[test]
fn file_picker_columns_remain_scrollable_by_wheel_and_keyboard() {
    for label in ["New", "1  recent-1.png", "PNG picture"] {
        let ctx = Context::default();
        ctx.enable_accesskit();
        let (mut app, output) = opened(&ctx, label);
        let initial = bounds(&output, label);
        frame_events(
            &mut app,
            &ctx,
            vec![
                Event::PointerMoved(initial.center()),
                Event::MouseWheel {
                    unit: MouseWheelUnit::Point,
                    delta: vec2(0.0, -100.0),
                    modifiers: Modifiers::NONE,
                },
            ],
            SIZE,
        );
        let output = settle(&mut app, &ctx);
        assert!(
            bounds(&output, label).top() < initial.top(),
            "{label} did not scroll"
        );
    }

    let ctx = Context::default();
    ctx.enable_accesskit();
    let (mut app, _) = opened(&ctx, "New");
    let mut reached_exit = false;
    for _ in 0..24 {
        frame_at_size(&mut app, &ctx, Some(Key::ArrowDown), SIZE);
        let output = settle(&mut app, &ctx);
        let focused = ctx.memory(|memory| memory.focused()).unwrap();
        let node = output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .find(|(id, _)| id.0 == focused.value())
            .unwrap();
        if node.1.label() == Some("Exit") {
            let exit = bounds(&output, "Exit");
            assert!(exit.top() >= 0.0 && exit.bottom() <= SIZE.y);
            reached_exit = true;
            break;
        }
    }
    assert!(
        reached_exit,
        "Keyboard navigation must reach off-screen File commands"
    );
    assert!(!app.doc.dirty());
}
