use super::*;

fn bounds(output: &FullOutput, label: &str) -> Rect {
    let node = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some(label))
        .unwrap_or_else(|| panic!("Missing {label}"));
    let rect = node.1.bounds().unwrap();
    Rect::from_min_max(
        pos2(rect.x0 as f32, rect.y0 as f32),
        pos2(rect.x1 as f32, rect.y1 as f32),
    )
}

fn settle(app: &mut PaintApp, ctx: &Context) -> FullOutput {
    for _ in 0..12 {
        app_frame(app, ctx, vec![]);
    }
    app_frame(app, ctx, vec![])
}

fn opened(ctx: &Context) -> (PaintApp, FullOutput) {
    let mut app = editing_app(ctx, "Caption stays editable");
    app.font_db
        .load_font_data(epaint_default_fonts::HACK_REGULAR.to_vec());
    let face = app.font_db.faces().next().unwrap().id;
    app.font_names = (0..20)
        .map(|index| (format!("Family {index:02}"), face))
        .collect();
    for code in [Key::F10, Key::T, Key::F, Key::L] {
        app_frame(&mut app, ctx, vec![key(code, Modifiers::NONE)]);
    }
    let output = settle(&mut app, ctx);
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
fn font_picker_does_not_scroll_from_movement_before_a_press() {
    let ctx = Context::default();
    ctx.enable_accesskit();
    let (mut app, output) = opened(&ctx);
    let initial = bounds(&output, "Sans serif");
    let target = initial.center();
    app_frame(
        &mut app,
        &ctx,
        vec![Event::PointerMoved(target + vec2(0.0, 170.0))],
    );
    app_frame(
        &mut app,
        &ctx,
        vec![Event::PointerMoved(target), button(target, true)],
    );
    let output = settle(&mut app, &ctx);
    assert_eq!(bounds(&output, "Sans serif"), initial);
    app_frame(&mut app, &ctx, vec![button(target, false)]);
    settle(&mut app, &ctx);
    let edit = app.text_edit.as_ref().unwrap();
    assert_eq!(edit.text, "Caption stays editable");
    assert_eq!(
        edit.format.style_at(0).font_name,
        crate::text::DEFAULT_FONT_NAME
    );
    assert!(!keytips::popup_open(&ctx));
}

#[test]
fn font_picker_keyboard_reaches_and_selects_an_off_screen_family() {
    let ctx = Context::default();
    ctx.enable_accesskit();
    let (mut app, _) = opened(&ctx);
    let mut reached_last = false;
    for _ in 0..24 {
        app_frame(&mut app, &ctx, vec![key(Key::ArrowDown, Modifiers::NONE)]);
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
        if node.1.label() == Some("Family 19") {
            let last = bounds(&output, "Family 19");
            let layer = ctx.layer_id_at(last.center()).unwrap();
            let popup = ctx.memory(|memory| memory.area_rect(layer.id)).unwrap();
            assert!(popup.contains_rect(last));
            reached_last = true;
            break;
        }
    }
    assert!(reached_last);
    app_frame(&mut app, &ctx, vec![key(Key::Enter, Modifiers::NONE)]);
    settle(&mut app, &ctx);
    let edit = app.text_edit.as_ref().unwrap();
    assert_eq!(edit.format.style_at(0).font_name, "Family 19");
    assert_eq!(edit.text, "Caption stays editable");
}
