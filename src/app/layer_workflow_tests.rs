use super::*;

fn frame(app: &mut PaintApp, ctx: &Context, events: Vec<Event>) {
    let mut input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0))),
        time: Some(ctx.cumulative_pass_nr() as f64 / 30.0),
        events,
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
        app.layers_panel(ctx);
        app.canvas(ctx);
        app.dialogs(ctx);
    });
}

fn layered_app() -> (PaintApp, Context) {
    let ctx = Context::default();
    let mut app = PaintApp::new_with_context(&ctx, false);
    app.doc = Document::from_image(RgbaImage::from_fn(40, 30, |x, _| {
        Rgba(if x < 20 {
            [20, 180, 50, 255]
        } else {
            [20, 50, 180, 255]
        })
    }));
    app.doc.begin();
    app.doc.add_layer().unwrap();
    app.doc.commit();
    app.doc.mark_saved();
    frame(&mut app, &ctx, vec![]);
    (app, ctx)
}

fn click_pixel(app: &mut PaintApp, ctx: &Context, point: Point) {
    let position = app.canvas_rect.min + vec2(point.0 as f32 + 0.5, point.1 as f32 + 0.5);
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
    frame(app, ctx, vec![]);
}

#[test]
fn fill_and_eraser_use_active_pixels_and_preserve_the_background() {
    let (mut app, ctx) = layered_app();
    let background = app.doc.layers()[0].clone();
    let red = [230, 35, 50, 255];
    app.colors[0] = red;
    app.set_tool(Tool::Fill);
    click_pixel(&mut app, &ctx, (8, 8));
    assert!(app.doc.image.pixels().all(|pixel| pixel.0 == red));
    assert!(app.doc.layers()[0] == background);

    app.set_tool(Tool::Eraser);
    app.size = 1;
    click_pixel(&mut app, &ctx, (8, 8));
    assert_eq!(app.doc.image.get_pixel(8, 8)[3], 0);
    assert_eq!(
        app.doc.composite().get_pixel(8, 8),
        background.image.get_pixel(8, 8)
    );
    assert!(app.doc.layers()[0] == background);
    app.action(Action::Undo, &ctx);
    assert_eq!(app.doc.image.get_pixel(8, 8).0, red);
}

#[test]
fn locked_and_hidden_layers_reject_drawing_text_paste_and_delete() {
    for hidden in [false, true] {
        let (mut app, ctx) = layered_app();
        app.doc.active_layer_mut().locked = !hidden;
        app.doc.active_layer_mut().visible = !hidden;
        let layers = app.doc.layers().to_vec();
        for tool in [
            Tool::Pencil,
            Tool::Fill,
            Tool::Eraser,
            Tool::Text,
            Tool::Rectangle,
        ] {
            app.set_tool(tool);
            click_pixel(&mut app, &ctx, (8, 8));
            assert!(app.text_edit.is_none());
            assert!(app.doc.layers() == layers);
        }
        app.insert_image(RgbaImage::from_pixel(5, 5, Rgba(BLACK)));
        app.execute(Action::ClearPicture, &ctx);
        app.delete_selection();
        assert!(app.doc.layers() == layers);
        assert!(!app.doc.dirty());
        assert!(app.message.contains(if hidden { "Show" } else { "Unlock" }));
    }
}

#[test]
fn cutting_a_layer_selection_never_copies_or_clears_another_layer() {
    let (mut app, ctx) = layered_app();
    let background = app.doc.layers()[0].clone();
    app.doc.image.put_pixel(5, 5, Rgba([240, 20, 30, 255]));
    app.selection = Some(Region {
        x: 4,
        y: 4,
        w: 4,
        h: 4,
    });
    app.cut();
    let copied = app.copied.as_ref().unwrap();
    assert_eq!(copied.get_pixel(1, 1).0, [240, 20, 30, 255]);
    assert_eq!(copied.get_pixel(0, 0)[3], 0);
    assert!(app.doc.image.pixels().all(|pixel| pixel[3] == 0));
    assert!(app.doc.layers()[0] == background);
    app.action(Action::Undo, &ctx);
    assert_eq!(app.doc.image.get_pixel(5, 5).0, [240, 20, 30, 255]);
}

#[test]
fn canvas_resize_and_skew_keep_the_stack_and_can_be_undone_together() {
    let (mut app, ctx) = layered_app();
    app.insert_image(RgbaImage::from_pixel(9, 7, Rgba([240, 20, 30, 255])));
    app.clear_selection();
    let layers = app.doc.layers().to_vec();
    app.pixel_resize = true;
    app.resize_picture(80, 60, 20.0, 0.0).unwrap();
    assert_eq!(app.doc.layer_count(), 2);
    let dimensions = app.doc.image.dimensions();
    assert!(dimensions.0 > 80 && dimensions.1 == 60);
    assert!(app
        .doc
        .layers()
        .iter()
        .all(|layer| layer.image.dimensions() == dimensions));
    assert!(
        matches!(&app.doc.objects[0].kind, ObjectKind::Image(image) if image.dimensions() == (9, 7))
    );
    app.action(Action::Undo, &ctx);
    assert!(app.doc.layers() == layers);
}

#[test]
fn image_adjustments_only_capture_the_active_layer_and_keep_original_pixels() {
    let (mut app, _ctx) = layered_app();
    let background = app.doc.layers()[0].clone();
    app.doc.image.put_pixel(6, 7, Rgba([240, 20, 30, 255]));
    app.apply_image_edits(d::ImageEdits {
        invert: true,
        ..Default::default()
    })
    .unwrap();
    assert!(app.doc.layers()[0] == background);
    let ObjectKind::Image(source) = &app.doc.objects[app.object.unwrap()].kind else {
        panic!("adjustments must retain the active image source");
    };
    assert_eq!(source.get_pixel(0, 0)[3], 0);
    assert_eq!(source.get_pixel(6, 7).0, [240, 20, 30, 255]);
    app.reset_image_edits().unwrap();
    assert_eq!(
        app.doc.active_composite().get_pixel(6, 7).0,
        [240, 20, 30, 255]
    );
}

#[test]
fn view_keytip_toggles_layers_without_creating_a_layer_or_dirtying_picture() {
    let (mut app, ctx) = layered_app();
    for key in [Key::F10, Key::V, Key::L] {
        frame(
            &mut app,
            &ctx,
            vec![Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        frame(&mut app, &ctx, vec![]);
    }
    assert!(app.layer_ui.open);
    assert_eq!(app.doc.layer_count(), 2);
    assert!(!app.doc.dirty());
}
