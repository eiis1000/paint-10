use super::*;

fn pending_polygon(context: &Context) -> PaintApp {
    let mut app = PaintApp::new_with_context(context, false);
    app.doc = Document::new(160, 120);
    app.set_tool(Tool::Polygon);
    app.colors[0] = [35, 80, 120, 255];
    for frame in 0..3 {
        pointer_app_frame_at(&mut app, context, vec![], frame as f64 * 0.03);
    }
    let origin = app.canvas_rect.min;
    let mut first_edge = coalesced_click(origin + vec2(20.5, 20.5), PointerButton::Primary);
    first_edge.pop();
    first_edge.extend([
        Event::PointerMoved(origin + vec2(120.5, 20.5)),
        Event::PointerButton {
            pos: origin + vec2(120.5, 20.5),
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        },
    ]);
    pointer_app_frame_at(&mut app, context, first_edge, 1.0);
    pointer_app_frame_at(
        &mut app,
        context,
        coalesced_click(origin + vec2(70.5, 100.5), PointerButton::Primary),
        2.0,
    );
    assert_eq!(app.polygon.len(), 3);
    assert!(app.shape_draft.is_none());
    app
}

fn plain_enter() -> Event {
    Event::Key {
        key: Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn coalesced_polygon_enters_finish_then_apply_before_the_next_palette_click() {
    for next_color in [false, true] {
        let context = Context::default();
        context.enable_accesskit();
        let mut app = pending_polygon(&context);
        let expected = app.doc.composite();
        let output = pointer_app_frame_at(&mut app, &context, vec![], 2.1);
        let mut released = plain_enter();
        if let Event::Key { pressed, .. } = &mut released {
            *pressed = false;
        }
        let mut events = vec![plain_enter(), released.clone(), plain_enter(), released];
        if next_color {
            let bounds = output
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap()
                .nodes
                .iter()
                .find(|(_, node)| node.label() == Some("Red, red 237, green 28, blue 36"))
                .unwrap()
                .1
                .bounds()
                .unwrap();
            events.extend(coalesced_click(
                pos2(
                    ((bounds.x0 + bounds.x1) / 2.0) as f32,
                    ((bounds.y0 + bounds.y1) / 2.0) as f32,
                ),
                PointerButton::Primary,
            ));
        }
        pointer_app_frame_at(&mut app, &context, events, 3.0);
        for frame in 1..12 {
            if !context.has_requested_repaint() {
                break;
            }
            pointer_app_frame_at(&mut app, &context, vec![], 3.0 + frame as f64 * 0.7);
        }
        assert!(app.polygon.is_empty());
        assert!(app.shape_draft.is_none(), "Both Enter presses must execute");
        assert_eq!(app.doc.composite(), expected);
        if next_color {
            assert_eq!(app.colors[0], [237, 28, 36, 255]);
        }
        assert!(app.doc.can_undo());
        app.action(Action::Undo, &context);
        assert!(app.doc.composite().pixels().all(|pixel| pixel.0 == WHITE));
        assert!(!app.doc.can_undo());
        assert!(!app.doc.dirty());
    }
}

#[test]
fn shape_key_queue_preserves_modified_keys_and_other_input_owners() {
    for owner in [
        "control", "text", "ribbon", "popup", "dialog", "pending", "measure", "drag",
    ] {
        let context = Context::default();
        let mut app = pending_polygon(&context);
        match owner {
            "control" => context.memory_mut(|memory| {
                memory.request_focus(Id::new("numeric-control"));
            }),
            "text" => {
                let index = app.doc.add_object(Object::new(
                    ObjectKind::Text {
                        text: "Caption".into(),
                        format: Default::default(),
                    },
                    (10, 10),
                ));
                app.edit_text_object(index);
            }
            "ribbon" => keytips::enter_ribbon(&context, Id::new("canvas")),
            "popup" => context.memory_mut(|memory| {
                memory.open_popup(Id::new("context-menu"));
            }),
            "dialog" => app.dialog = Some(Dialog::Properties),
            "pending" => app.pending = Some(Action::New),
            "measure" => app.measure.enabled = true,
            "drag" => {
                app.gesture = Some(Gesture::CanvasSize {
                    start: (0, 0),
                    original: (160, 120),
                    axis: 1,
                })
            }
            _ => unreachable!(),
        }
        let events = vec![plain_enter(), plain_enter()];
        let mut input = RawInput {
            events: events.clone(),
            time: Some(3.0),
            ..Default::default()
        };
        app.canvas_raw_input(&context, &mut input);
        assert_eq!(input.events, events, "{owner} owns the input batch");
    }

    let context = Context::default();
    let app = pending_polygon(&context);
    let mut modified = plain_enter();
    if let Event::Key { modifiers, .. } = &mut modified {
        *modifiers = Modifiers::CTRL;
    }
    let events = vec![modified.clone(), modified];
    let mut input = RawInput {
        events: events.clone(),
        ..Default::default()
    };
    app.canvas_raw_input(&context, &mut input);
    assert_eq!(input.events, events);
}
