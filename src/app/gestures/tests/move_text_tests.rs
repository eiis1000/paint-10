use super::*;
use crate::text::TextFormat;

fn retained_text_app(latex: bool, angle: f32) -> (PaintApp, Context, usize) {
    let ctx = Context::default();
    let mut app = PaintApp::new_with_context(&ctx, false);
    let mut document = Document::new(700, 450);
    let mut object = Object::new(
        ObjectKind::Text {
            text: if latex { r"\frac{a}{b}" } else { "Caption" }.into(),
            format: TextFormat {
                latex,
                width: 240,
                minimum_height: 100,
                ..Default::default()
            },
        },
        (50, 70),
    );
    object.angle = angle;
    let index = document.add_object(object);
    app.doc = crate::project::decode(&crate::project::encode(&document).unwrap()).unwrap();
    app.set_tool(Tool::Select);
    for _ in 0..4 {
        pointer_app_frame(&mut app, &ctx, vec![]);
    }
    (app, ctx, index)
}

fn drag(app: &mut PaintApp, ctx: &Context, start: Pos2, end: Pos2, batched: bool) {
    let press = Event::PointerButton {
        pos: start,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    };
    let release = Event::PointerButton {
        pos: end,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    };
    if batched {
        pointer_app_frame_at(
            app,
            ctx,
            vec![
                Event::PointerMoved(start),
                press,
                Event::PointerMoved(end),
                release,
            ],
            1.0,
        );
    } else {
        pointer_app_frame_at(app, ctx, vec![Event::PointerMoved(start), press], 1.0);
        pointer_app_frame_at(app, ctx, vec![Event::PointerMoved(end)], 1.1);
        pointer_app_frame_at(app, ctx, vec![release], 1.2);
    }
}

#[test]
fn first_select_drag_moves_loaded_text_boxes_from_transparent_space_and_undoes_once() {
    for latex in [false, true] {
        for angle in [0.0, 27.0] {
            for batched in [false, true] {
                let (mut app, ctx, index) = retained_text_app(latex, angle);
                let before = app.doc.objects[index].clone();
                let image = before.render();
                let (x, y, _) = image
                    .enumerate_pixels()
                    .find(|(x, y, pixel)| {
                        *x > 20
                            && *y > 20
                            && *x + 20 < image.width()
                            && *y + 20 < image.height()
                            && pixel[3] == 0
                    })
                    .expect("The text box has draggable whitespace away from its handles");
                let start = app.canvas_rect.min
                    + vec2(
                        before.pos.0 as f32 + x as f32 + 0.5,
                        before.pos.1 as f32 + y as f32 + 0.5,
                    ) * app.zoom;
                let end = start + vec2(37.0, 24.0) * app.zoom;
                drag(&mut app, &ctx, start, end, batched);

                let mut moved = before.clone();
                moved.pos = (before.pos.0 + 37, before.pos.1 + 24);
                assert_eq!(app.object, Some(index));
                assert!(app.doc.objects[index] == moved,
                    "The first drag must move retained text; latex={latex}, angle={angle}, batched={batched}");
                assert!(app.selection.is_none());
                assert!(app.text_edit.is_none());
                assert!(app.dialog.is_none());
                assert!(app.gesture.is_none());
                app.doc.undo();
                assert!(app.doc.objects[index] == before);
                assert!(!app.doc.dirty());
                assert!(!app.doc.can_undo());
            }
        }
    }
}

#[test]
fn explicit_area_selection_still_works_over_images_and_text_lassos() {
    for over_image in [false, true] {
        let (mut app, ctx, index) = retained_text_app(true, 0.0);
        app.free_select = !over_image;
        if over_image {
            app.doc.objects[index].kind =
                ObjectKind::Image(RgbaImage::from_pixel(240, 100, Rgba([90, 120, 180, 255])));
        }
        app.doc.mark_saved();
        let before = app.doc.objects[index].clone();
        let start = app.canvas_rect.min + vec2(80.5, 100.5) * app.zoom;
        let end = start + vec2(50.0, 35.0) * app.zoom;
        drag(&mut app, &ctx, start, end, false);
        assert!(app.selection.is_some());
        assert!(app.object.is_none());
        assert!(app.doc.objects[index] == before);
        assert!(!app.doc.dirty());
    }
}

#[test]
fn text_drag_cannot_modify_a_hidden_or_locked_layer() {
    for hidden in [false, true] {
        let (mut app, ctx, index) = retained_text_app(true, 0.0);
        app.doc.active_layer_mut().visible = !hidden;
        app.doc.active_layer_mut().locked = !hidden;
        app.doc.mark_saved();
        let before = app.doc.objects[index].clone();
        let start = app.canvas_rect.min + vec2(80.5, 100.5) * app.zoom;
        let end = start + vec2(50.0, 35.0) * app.zoom;
        drag(&mut app, &ctx, start, end, false);
        assert!(app.doc.objects[index] == before);
        assert!(app.gesture.is_none());
        assert!(!app.doc.dirty());
    }
}
