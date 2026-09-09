use super::*;

fn frame(app: &mut PaintApp, ctx: &Context, size: Vec2, events: Vec<Event>) -> FullOutput {
    let mut input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
        time: Some(ctx.cumulative_pass_nr() as f64 / 30.0),
        events,
        ..Default::default()
    };
    eframe::App::raw_input_hook(app, ctx, &mut input);
    ctx.run(input, |ctx| {
        app.shortcut(ctx);
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

fn text_rect(output: &FullOutput, label: &str) -> Rect {
    output
        .shapes
        .iter()
        .find_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape {
                (text.galley.text() == label)
                    .then(|| Rect::from_min_size(text.pos, text.galley.size()))
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("No visible {label:?}"))
}

fn click(app: &mut PaintApp, ctx: &Context, position: Pos2, button: PointerButton) {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec2(900.0, 650.0),
            vec![
                Event::PointerMoved(position),
                Event::PointerButton {
                    pos: position,
                    button,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
}

fn freeform_app(ctx: &Context) -> PaintApp {
    let mut app = PaintApp::new_with_context(ctx, false);
    app.doc = Document::new(80, 60);
    app.doc.add_layer().unwrap();
    app.doc.image = RgbaImage::from_fn(80, 60, |x, y| Rgba([x as u8, y as u8, 120, 255]));
    app.tool = Tool::Select;
    app.free_select = true;
    app.selection = Some(Region {
        x: 10,
        y: 10,
        w: 30,
        h: 20,
    });
    app.free_points = vec![(10, 10), (39, 10), (10, 29)];
    app
}

#[test]
fn combined_selection_transform_keeps_original_alpha_and_has_one_undo() {
    let ctx = Context::default();
    let mut app = freeform_app(&ctx);
    let source = app.selected_image().unwrap();
    assert!(source.pixels().any(|pixel| pixel[3] == 0));
    let before = app.doc.layers().to_vec();
    app.resize_skew_rotate_picture(15, 10, 12.0, -7.0, 37.0)
        .unwrap();
    assert_eq!(app.doc.active_layer_index(), 1);
    assert!(app.doc.layers()[0] == before[0]);
    let object = &app.doc.objects[app.object.unwrap()];
    assert!(matches!(&object.kind, ObjectKind::Image(image) if image == &source));
    assert_eq!(object.angle, 37.0);
    assert!(object.render().pixels().any(|pixel| pixel[3] == 0));
    app.doc.undo();
    assert!(app.doc.layers() == before);
    assert!(!app.doc.can_undo());
}

#[test]
fn repeated_freeform_rotation_keeps_source_and_does_not_fill_transparent_corners() {
    let ctx = Context::default();
    let mut app = freeform_app(&ctx);
    let source = app.selected_image().unwrap();
    let before = app.doc.layers().to_vec();
    assert!(app.rotate_picture(37.0, true));
    assert!(app.rotate_picture(0.0, true));
    let object = &app.doc.objects[app.object.unwrap()];
    assert!(matches!(&object.kind, ObjectKind::Image(image) if image == &source));
    assert_eq!(object.render(), source);
    app.doc.undo();
    app.doc.undo();
    assert!(app.doc.layers() == before);
    assert!(!app.doc.can_undo());
}

#[test]
fn combined_transform_keeps_text_editable_and_whole_picture_layers_intact() {
    let ctx = Context::default();
    let mut app = PaintApp::new_with_context(&ctx, false);
    app.doc = Document::new(80, 60);
    app.doc.add_layer().unwrap();
    let index = app.doc.add_object(Object::new(
        ObjectKind::Text {
            text: "Editable".into(),
            format: Default::default(),
        },
        (5, 8),
    ));
    app.select_object(index);
    let before = app.doc.layers().to_vec();
    app.resize_skew_rotate_picture(40, 20, 8.0, 0.0, -23.0)
        .unwrap();
    assert!(
        matches!(&app.doc.objects[index].kind, ObjectKind::Text { text, .. } if text == "Editable")
    );
    app.doc.undo();
    assert!(app.doc.layers() == before);
    assert!(!app.doc.can_undo());

    app.clear_selection();
    app.resize_skew_rotate_picture(40, 30, 12.0, 0.0, 90.0)
        .unwrap();
    assert_eq!(app.doc.active_layer_index(), 1);
    assert_eq!(app.doc.layer_count(), 2);
    assert!(matches!(
        app.doc.layers()[1].objects[0].kind,
        ObjectKind::Text { .. }
    ));
    assert_eq!(app.doc.image.height(), 47);
    app.doc.undo();
    assert!(app.doc.layers() == before);
    assert!(!app.doc.can_undo());
}

#[test]
fn invalid_combined_transform_preserves_freeform_selection_and_pixels() {
    let ctx = Context::default();
    let mut app = freeform_app(&ctx);
    let before = app.doc.layers().to_vec();
    let points = app.free_points.clone();
    for (skew_x, skew_y, angle) in [(45.0, 45.0, 20.0), (0.0, 0.0, f32::NAN)] {
        assert!(app
            .resize_skew_rotate_picture(20, 10, skew_x, skew_y, angle)
            .is_err());
        assert!(app.doc.layers() == before);
        assert_eq!(app.free_points, points);
        assert!(app.selection.is_some());
        assert!(app.object.is_none());
        assert!(!app.doc.can_undo());
    }
}

#[test]
fn resize_cancel_keeps_modified_text_open_and_uses_its_current_dimensions() {
    let ctx = Context::default();
    let mut app = PaintApp::new_with_context(&ctx, false);
    app.doc = Document::new(160, 90);
    let index = app.doc.add_object(Object::new(
        ObjectKind::Text {
            text: "Before".into(),
            format: Default::default(),
        },
        (5, 8),
    ));
    app.edit_text_object(index);
    let state = app.text_edit.as_mut().unwrap();
    state.text = "A much longer caption".into();
    let expected = state.format.dimensions(&state.text);
    let before = app.doc.layers().to_vec();
    app.action(Action::Resize, &ctx);
    assert!(app.text_edit.is_some());
    assert_eq!((app.resize_w, app.resize_h), expected);
    let size = vec2(500.0, 400.0);
    for _ in 0..5 {
        frame(&mut app, &ctx, size, vec![]);
    }
    frame(
        &mut app,
        &ctx,
        size,
        vec![key(Key::Escape, Modifiers::NONE)],
    );
    assert!(app.dialog.is_none());
    assert_eq!(
        app.text_edit.as_ref().unwrap().text,
        "A much longer caption"
    );
    assert!(app.doc.layers() == before);
    assert!(!app.doc.can_undo());
}

#[test]
fn resize_measures_an_unfinished_polygon_without_committing_it() {
    let ctx = Context::default();
    let mut app = PaintApp::new_with_context(&ctx, false);
    app.doc = Document::new(160, 90);
    app.tool = Tool::Polygon;
    app.polygon = vec![(10, 10), (40, 10), (20, 30)];
    app.doc.begin();
    app.action(Action::Resize, &ctx);
    assert_eq!(app.polygon.len(), 3);
    assert!(app.shape_draft.is_none());
    assert!(app.resize_w < 40 && app.resize_h < 30);
    let expected = (app.resize_w, app.resize_h);
    app.resize_skew_rotate_picture(expected.0, expected.1, 0.0, 0.0, 90.0)
        .unwrap();
    assert_eq!(app.doc.image.dimensions(), (160, 90));
    let object = &app.doc.objects[app.object.unwrap()];
    assert_eq!(object.rendered_dimensions(), Some((expected.1, expected.0)));
}

#[test]
fn resize_dialog_accepts_rotation_by_keyboard_and_cancel_keeps_a_shape_draft() {
    let ctx = Context::default();
    let mut app = PaintApp::new_with_context(&ctx, false);
    app.doc = Document::new(80, 60);
    let size = vec2(500.0, 400.0);
    frame(&mut app, &ctx, size, vec![]);
    frame(
        &mut app,
        &ctx,
        size,
        vec![key(Key::W, Modifiers::CTRL | Modifiers::COMMAND)],
    );
    for _ in 0..5 {
        frame(&mut app, &ctx, size, vec![]);
    }
    assert!(app.dialog == Some(Dialog::Resize));
    let output = frame(&mut app, &ctx, size, vec![]);
    for label in ["Horizontal:", "Vertical:", "Rotation:", "OK", "Cancel"] {
        let bounds = text_rect(&output, label);
        assert!(Rect::from_min_size(Pos2::ZERO, size).contains_rect(bounds));
        let clip = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    (text.galley.text() == label).then_some(shape.clip_rect)
                } else {
                    None
                }
            })
            .unwrap();
        assert!(
            clip.contains_rect(bounds),
            "{label} is clipped: {bounds:?} in {clip:?}"
        );
    }
    for _ in 0..6 {
        frame(&mut app, &ctx, size, vec![key(Key::Tab, Modifiers::NONE)]);
    }
    frame(&mut app, &ctx, size, vec![Event::Text("37".into())]);
    frame(&mut app, &ctx, size, vec![key(Key::Enter, Modifiers::NONE)]);
    for _ in 0..5 {
        frame(&mut app, &ctx, size, vec![]);
    }
    assert_eq!(app.angle, 37.0);
    assert!(app.dialog.is_none());
    assert_eq!(
        app.doc.image.dimensions(),
        d::rotation_size(80, 60, 37.0).unwrap()
    );
    app.doc.undo();
    assert_eq!(app.doc.image.dimensions(), (80, 60));
    assert!(!app.doc.can_undo());

    app.set_tool(Tool::Rectangle);
    app.doc.begin();
    app.start_shape_draft(
        ShapeGeometry::Primitive {
            tool: Tool::Rectangle,
            start: (10, 10),
            end: (40, 30),
        },
        0,
    );
    let before = app.doc.composite();
    app.action(Action::Resize, &ctx);
    assert!(app.shape_draft.is_some());
    for _ in 0..5 {
        frame(&mut app, &ctx, size, vec![]);
    }
    frame(
        &mut app,
        &ctx,
        size,
        vec![key(Key::Escape, Modifiers::NONE)],
    );
    assert!(app.dialog.is_none());
    assert!(app.shape_draft.is_some());
    assert_eq!(app.doc.composite(), before);
    assert!(!app.doc.can_undo());
}

#[test]
fn freeform_right_click_exposes_rotate_and_custom_angle_applies_to_selection() {
    let ctx = Context::default();
    let mut app = freeform_app(&ctx);
    let size = vec2(900.0, 650.0);
    for _ in 0..3 {
        frame(&mut app, &ctx, size, vec![]);
    }
    let position = app.canvas_rect.min + vec2(15.0, 15.0);
    click(&mut app, &ctx, position, PointerButton::Secondary);
    frame(&mut app, &ctx, size, vec![]);
    let output = frame(&mut app, &ctx, size, vec![]);
    let glyph_left = |label: &str| {
        output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    (text.galley.text() == label)
                        .then(|| text.pos.x + text.galley.rows[0].glyphs[0].pos.x)
                } else {
                    None
                }
            })
            .unwrap()
    };
    assert!((glyph_left("Rotate") - glyph_left("Crop")).abs() < 1.0);
    let rotate = text_rect(&output, "Rotate").center();
    click(&mut app, &ctx, rotate, PointerButton::Primary);
    frame(&mut app, &ctx, size, vec![]);
    let output = frame(&mut app, &ctx, size, vec![]);
    text_rect(&output, "Rotate right 90°");
    click(
        &mut app,
        &ctx,
        text_rect(&output, "Custom angle…").center(),
        PointerButton::Primary,
    );
    for _ in 0..5 {
        frame(&mut app, &ctx, size, vec![]);
    }
    assert!(app.dialog == Some(Dialog::Rotate));
    frame(&mut app, &ctx, size, vec![Event::Text("45".into())]);
    frame(&mut app, &ctx, size, vec![key(Key::Enter, Modifiers::NONE)]);
    for _ in 0..5 {
        frame(&mut app, &ctx, size, vec![]);
    }
    assert!(app.dialog.is_none());
    assert_eq!(app.doc.active_layer_index(), 1);
    assert_eq!(app.doc.image.dimensions(), (80, 60));
    let object = &app.doc.objects[app.object.unwrap()];
    assert_eq!(object.angle, 45.0);
    assert!(object.render().pixels().any(|pixel| pixel[3] == 0));
}

#[test]
fn a_closed_lasso_keeps_its_selection_and_exposes_rotation() {
    let ctx = Context::default();
    let mut app = freeform_app(&ctx);
    app.clear_selection();
    let size = vec2(900.0, 650.0);
    for _ in 0..3 {
        frame(&mut app, &ctx, size, vec![]);
    }
    let start = app.canvas_rect.min + vec2(10.0, 10.0);
    frame(
        &mut app,
        &ctx,
        size,
        vec![
            Event::PointerMoved(start),
            Event::PointerButton {
                pos: start,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    for offset in [vec2(30.0, 0.0), vec2(15.0, 20.0), Vec2::ZERO] {
        frame(
            &mut app,
            &ctx,
            size,
            vec![Event::PointerMoved(start + offset)],
        );
    }
    frame(
        &mut app,
        &ctx,
        size,
        vec![Event::PointerButton {
            pos: start,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert!(app.selection.is_some());
    assert_eq!(app.free_points.first(), app.free_points.last());
    let selected = app.selected_image().unwrap();
    assert!(selected.pixels().any(|pixel| pixel[3] == 0));
    assert!(selected.pixels().any(|pixel| pixel[3] == 255));
    assert!(!app.doc.can_undo());

    click(
        &mut app,
        &ctx,
        start + vec2(15.0, 5.0),
        PointerButton::Secondary,
    );
    let output = frame(&mut app, &ctx, size, vec![]);
    text_rect(&output, "Rotate");
    assert!(app.selection.is_some());
}
