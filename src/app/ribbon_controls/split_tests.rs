use super::*;

fn frame(app: &mut PaintApp, ctx: &Context, width: f32, events: Vec<Event>) -> FullOutput {
    let mut input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width, 400.0))),
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
        .find(|(_, node)| {
            node.label() == Some(label) && node.role() == egui::accesskit::Role::Button
        })
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

#[test]
fn shape_style_menus_align_and_remain_usable_in_expanded_and_collapsed_ribbons() {
    for width in [1200.0, 500.0] {
        let ctx = Context::default();
        ctx.enable_accesskit();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let mut output = settle(&mut app, &ctx, width);
        if width == 500.0 {
            let shapes = bounds(&output, "Shapes");
            click(&mut app, &ctx, width, shapes.center());
            output = settle(&mut app, &ctx, width);
        }

        let outline = bounds(&output, "Shape outline style");
        let fill = bounds(&output, "Shape fill style");
        assert_eq!(outline.x_range(), fill.x_range());
        assert_eq!(outline.size(), fill.size());
        assert_eq!(outline.height(), 26.0);
        assert_eq!(fill.top() - outline.bottom(), 4.0);
        assert!(fill.right() <= width);

        // Exercise the arrow edge, not only the text in the enlarged slot.
        click(&mut app, &ctx, width, fill.right_center() - vec2(5.0, 0.0));
        output = settle(&mut app, &ctx, width);
        let watercolor = bounds(&output, PaintStyle::Watercolor.name());
        assert!(watercolor.left() >= 0.0 && watercolor.right() <= width);
        click(&mut app, &ctx, width, watercolor.center());
        assert_eq!(app.fill, PaintStyle::Watercolor);
    }
}

#[test]
fn ribbon_header_actions_share_geometry_and_keep_collapse_behavior() {
    for width in [1200.0, 500.0] {
        let ctx = Context::default();
        ctx.enable_accesskit();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let output = settle(&mut app, &ctx, width);
        let help = bounds(&output, "Help");
        let collapse = bounds(&output, "Minimize the ribbon");
        assert_eq!(help.size(), vec2(24.0, 24.0));
        assert_eq!(help.size(), collapse.size());
        assert_eq!(help.y_range(), collapse.y_range());
        assert_eq!(help.left() - collapse.right(), 2.0);
        assert!(help.right() <= width);

        click(&mut app, &ctx, width, collapse.center());
        assert!(app.collapsed);
        let output = settle(&mut app, &ctx, width);
        let expand = bounds(&output, "Expand the ribbon");
        assert_eq!(expand, collapse);
        click(&mut app, &ctx, width, expand.center());
        assert!(!app.collapsed);

        let output = settle(&mut app, &ctx, width);
        click(&mut app, &ctx, width, bounds(&output, "Help").center());
        assert!(matches!(app.dialog, Some(Dialog::About)));
    }
}
