use super::*;

fn frame(app: &mut PaintApp, ctx: &Context, width: f32, events: Vec<Event>) -> FullOutput {
    let mut input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width, 500.0))),
        time: Some(ctx.cumulative_pass_nr() as f64 / 20.0),
        events,
        ..Default::default()
    };
    eframe::App::raw_input_hook(app, ctx, &mut input);
    ctx.run(input, |ctx| {
        app.ribbon_keyboard(ctx);
        app.ribbon(ctx);
        app.keyboard_menu(ctx);
    })
}

fn settle(app: &mut PaintApp, ctx: &Context, width: f32) -> FullOutput {
    for _ in 0..4 {
        frame(app, ctx, width, vec![]);
    }
    frame(app, ctx, width, vec![])
}

fn bounds(output: &FullOutput, label: &str) -> Rect {
    let rect = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some(label))
        .unwrap_or_else(|| panic!("Missing accessible control: {label}"))
        .1
        .bounds()
        .unwrap();
    Rect::from_min_max(
        pos2(rect.x0 as f32, rect.y0 as f32),
        pos2(rect.x1 as f32, rect.y1 as f32),
    )
}

fn click(app: &mut PaintApp, ctx: &Context, width: f32, point: Pos2) {
    frame(
        app,
        ctx,
        width,
        vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
            Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
        ],
    );
}

fn assert_connected(output: &FullOutput, main: &str, menu: &str) {
    let main = bounds(output, main);
    let menu = bounds(output, menu);
    assert_eq!(main.x_range(), menu.x_range());
    assert_eq!(main.bottom(), menu.top());
    assert_eq!(menu.height(), 20.0);
}

#[test]
fn expanded_split_controls_keep_separate_actions_and_menu_targets() {
    let ctx = Context::default();
    ctx.enable_accesskit();
    let mut app = PaintApp::new_with_context(&ctx, false);
    app.set_tool(Tool::Pencil);
    let output = settle(&mut app, &ctx, 1200.0);
    for (main, menu) in [
        ("Paste", "Paste options"),
        ("Select", "Selection options"),
        ("Brushes", "Choose a brush"),
    ] {
        assert_connected(&output, main, menu);
    }

    let menu = bounds(&output, "Selection options");
    click(&mut app, &ctx, 1200.0, menu.center());
    let output = settle(&mut app, &ctx, 1200.0);
    assert_eq!(app.tool, Tool::Pencil, "The arrow must not select the tool");
    let free = bounds(&output, "Free-form selection");
    click(&mut app, &ctx, 1200.0, free.center());
    assert!(app.free_select);
    assert_eq!(app.tool, Tool::Select);

    let output = settle(&mut app, &ctx, 1200.0);
    let menu = bounds(&output, "Choose a brush");
    click(&mut app, &ctx, 1200.0, menu.center());
    let output = settle(&mut app, &ctx, 1200.0);
    assert_eq!(app.tool, Tool::Select);
    let brush = bounds(&output, Brush::Watercolor.name());
    click(&mut app, &ctx, 1200.0, brush.center());
    assert_eq!(app.brush, Brush::Watercolor);
    assert_eq!(app.tool, Tool::Brush);

    let output = settle(&mut app, &ctx, 1200.0);
    let main = bounds(&output, "Select");
    click(&mut app, &ctx, 1200.0, main.center());
    assert_eq!(app.tool, Tool::Select);
    assert!(!keytips::popup_open(&ctx));
    let output = settle(&mut app, &ctx, 1200.0);
    let main = bounds(&output, "Brushes");
    click(&mut app, &ctx, 1200.0, main.center());
    assert_eq!(app.tool, Tool::Brush);
    assert_eq!(app.brush, Brush::Watercolor);
    assert!(!keytips::popup_open(&ctx));
}

#[test]
fn collapsed_image_group_retains_connected_selection_and_nested_menu_actions() {
    let ctx = Context::default();
    ctx.enable_accesskit();
    let mut app = PaintApp::new_with_context(&ctx, false);
    app.set_tool(Tool::Pencil);
    let output = settle(&mut app, &ctx, 500.0);
    let image = bounds(&output, "Image");
    click(&mut app, &ctx, 500.0, image.center());
    let output = settle(&mut app, &ctx, 500.0);
    assert_connected(&output, "Select", "Selection options");
    let menu = bounds(&output, "Selection options");
    click(&mut app, &ctx, 500.0, menu.center());
    let output = settle(&mut app, &ctx, 500.0);
    let free = bounds(&output, "Free-form selection");
    assert!(free.left() >= 0.0 && free.right() <= 500.0);
    click(&mut app, &ctx, 500.0, free.center());
    assert!(app.free_select);
    assert_eq!(app.tool, Tool::Select);
}
