use super::*;

fn frame(app: &mut PaintApp, ctx: &Context, scale: f32, events: Vec<Event>) -> FullOutput {
    let mut input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1257.0, 750.0) / scale)),
        time: Some(ctx.cumulative_pass_nr() as f64 / 30.0),
        events,
        ..Default::default()
    };
    input
        .viewports
        .get_mut(&ViewportId::ROOT)
        .unwrap()
        .native_pixels_per_point = Some(scale);
    eframe::App::raw_input_hook(app, ctx, &mut input);
    ctx.run(input, |ctx| {
        if !app.ribbon_keyboard(ctx) {
            app.shortcut(ctx);
        }
        app.titlebar(ctx);
        app.ribbon(ctx);
        app.status(ctx);
        app.canvas(ctx);
        app.keyboard_menu(ctx);
        app.dialogs(ctx);
    })
}

fn settle(app: &mut PaintApp, ctx: &Context, scale: f32) -> FullOutput {
    for _ in 0..12 {
        frame(app, ctx, scale, vec![]);
    }
    frame(app, ctx, scale, vec![])
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
        .unwrap_or_else(|| panic!("Missing {label}"))
        .1
        .bounds()
        .unwrap();
    Rect::from_min_max(
        pos2(rect.x0 as f32, rect.y0 as f32),
        pos2(rect.x1 as f32, rect.y1 as f32),
    )
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn click(pos: Pos2) -> Vec<Event> {
    vec![
        Event::PointerMoved(pos),
        button(pos, true),
        button(pos, false),
    ]
}

#[test]
fn drawing_a_first_row_polygon_keeps_the_shape_gallery_position() {
    for (scale, separate_press) in [(1.0, false), (1.0, true), (1.05, false), (1.05, true)] {
        let ctx = Context::default();
        ctx.enable_accesskit();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.set_tool(Tool::Select);
        app.size = 137;
        let output = settle(&mut app, &ctx, scale);
        let initial = bounds(&output, "Polygon");
        let size = bounds(&output, "Brush and outline size, 137 pixels");
        frame(&mut app, &ctx, scale, click(size.center()));
        let output = settle(&mut app, &ctx, scale);
        let preset = bounds(&output, "3 px");
        frame(&mut app, &ctx, scale, click(preset.center()));
        let output = settle(&mut app, &ctx, scale);
        let polygon = bounds(&output, "Polygon");
        if separate_press {
            frame(
                &mut app,
                &ctx,
                scale,
                vec![
                    Event::PointerMoved(polygon.center()),
                    button(polygon.center(), true),
                ],
            );
            frame(&mut app, &ctx, scale, vec![button(polygon.center(), false)]);
        } else {
            frame(&mut app, &ctx, scale, click(polygon.center()));
        }
        let output = settle(&mut app, &ctx, scale);
        assert_eq!(
            bounds(&output, "Polygon"),
            initial,
            "Selecting must not scroll the gallery at scale {scale}, separate press {separate_press}"
        );
        assert_eq!(app.tool, Tool::Polygon);
        let origin = app.canvas_rect.min;
        let start = origin + vec2(125.0, 80.0);
        let end = origin + vec2(250.0, 80.0);
        frame(
            &mut app,
            &ctx,
            scale,
            vec![
                Event::PointerMoved(start),
                button(start, true),
                Event::PointerMoved(end),
                button(end, false),
            ],
        );
        frame(&mut app, &ctx, scale, click(origin + vec2(190.0, 165.0)));
        let enter = Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        frame(&mut app, &ctx, scale, vec![enter.clone(), enter]);
        let output = settle(&mut app, &ctx, scale);
        let red = bounds(&output, "Red, red 237, green 28, blue 36");
        frame(&mut app, &ctx, scale, click(red.center()));
        let output = settle(&mut app, &ctx, scale);
        let final_rect = bounds(&output, "Polygon");
        assert_eq!(
            final_rect, initial,
            "Drawing must not scroll the gallery at scale {scale}, separate press {separate_press}"
        );
        assert!(app.shape_draft.is_none());
    }
}

#[test]
fn shape_gallery_still_scrolls_with_its_arrows_and_mouse_wheel() {
    let scale = 1.05;
    let ctx = Context::default();
    ctx.enable_accesskit();
    let mut app = PaintApp::new_with_context(&ctx, false);
    let output = settle(&mut app, &ctx, scale);
    let initial = bounds(&output, "Polygon");

    let down = bounds(&output, "Scroll shapes down");
    frame(&mut app, &ctx, scale, click(down.center()));
    let output = settle(&mut app, &ctx, scale);
    assert_eq!(
        bounds(&output, "Polygon"),
        initial.translate(vec2(0.0, -25.0))
    );

    let up = bounds(&output, "Scroll shapes up");
    frame(&mut app, &ctx, scale, click(up.center()));
    let output = settle(&mut app, &ctx, scale);
    assert_eq!(bounds(&output, "Polygon"), initial);

    frame(
        &mut app,
        &ctx,
        scale,
        vec![
            Event::PointerMoved(initial.center()),
            Event::MouseWheel {
                unit: MouseWheelUnit::Point,
                delta: vec2(0.0, -50.0),
                modifiers: Modifiers::NONE,
            },
        ],
    );
    let output = settle(&mut app, &ctx, scale);
    assert_eq!(
        bounds(&output, "Polygon"),
        initial.translate(vec2(0.0, -25.0))
    );
}
