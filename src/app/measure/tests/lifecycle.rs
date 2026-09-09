use super::*;

fn measured_app(ctx: &Context) -> PaintApp {
    let mut app = PaintApp::new_with_context(ctx, false);
    app.doc = Document::new(48, 32);
    app.zoom = 8.0;
    app.measure.unit = Unit::Centimeters;
    app.set_measure_enabled(true, ctx);
    frame(&mut app, ctx, vec![]);
    frame(&mut app, ctx, vec![]);
    drag(&mut app, ctx, (10, 12), (35, 22));
    assert_eq!(app.measure.points, Some([(10, 12), (35, 22)]));
    app
}

fn assert_cleared(app: &PaintApp) {
    assert!(app.measure.points.is_none());
    assert!(app.measure.dragging.is_none());
    assert!(app.measure.enabled);
    assert_eq!(app.measure.unit, Unit::Centimeters);
}

#[test]
fn successful_new_clears_only_transient_measurement_state() {
    let ctx = Context::default();
    let mut app = measured_app(&ctx);
    app.measure.dragging = Some(1);
    app.action(Action::New, &ctx);
    assert_eq!(app.doc.image.dimensions(), (900, 600));
    assert_cleared(&app);
    frame(&mut app, &ctx, vec![]);
    assert_cleared(&app);
    assert!(!app.doc.dirty() && !app.doc.can_undo());
}

#[test]
fn canceling_new_or_open_preserves_the_existing_measurement() {
    for action in [Action::New, Action::Open] {
        let ctx = Context::default();
        let mut app = measured_app(&ctx);
        app.doc.begin();
        app.doc.image.put_pixel(0, 0, Rgba(BLACK));
        app.doc.commit();
        let picture = crate::project::encode(&app.doc).unwrap();
        let points = app.measure.points;
        app.action(action, &ctx);
        assert!(app.pending.is_some());
        assert_eq!(app.measure.points, points);
        for _ in 0..3 {
            frame(&mut app, &ctx, vec![]);
        }
        frame(&mut app, &ctx, vec![key(Key::Escape, Modifiers::NONE)]);
        frame(&mut app, &ctx, vec![]);
        assert!(app.pending.is_none());
        assert_eq!(app.measure.points, points);
        assert!(app.measure.enabled);
        assert_eq!(app.measure.unit, Unit::Centimeters);
        assert_eq!(crate::project::encode(&app.doc).unwrap(), picture);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn successful_raster_and_project_open_reset_even_same_size_measurements() {
    let directory = tempfile::tempdir().unwrap();
    for (format, name, dimensions) in [
        (crate::raster_io::RasterFormat::Png, "small.png", (8, 6)),
        (
            crate::raster_io::RasterFormat::Project,
            "same.p10",
            (48, 32),
        ),
    ] {
        let ctx = Context::default();
        let mut app = measured_app(&ctx);
        let document = Document::new(dimensions.0, dimensions.1);
        let bytes = if format == crate::raster_io::RasterFormat::Project {
            crate::project::encode(&document).unwrap()
        } else {
            crate::raster_io::encode(&document.image, format).unwrap()
        };
        let path = directory.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        app.measure.dragging = Some(1);
        app.load(path.clone());
        assert_eq!(app.file, Some(path));
        assert_eq!(app.doc.image.dimensions(), dimensions);
        assert_cleared(&app);
        frame(&mut app, &ctx, vec![]);
        assert_cleared(&app);
        assert!(!app.doc.dirty() && !app.doc.can_undo());
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn failed_open_preserves_picture_and_measurement() {
    let directory = tempfile::tempdir().unwrap();
    for (name, contents) in [
        ("invalid.png", b"invalid".as_slice()),
        ("invalid.p10", b"PAINT10\0incomplete".as_slice()),
    ] {
        let ctx = Context::default();
        let mut app = measured_app(&ctx);
        let points = app.measure.points;
        let picture = crate::project::encode(&app.doc).unwrap();
        let path = directory.path().join(name);
        std::fs::write(&path, contents).unwrap();
        app.load(path);
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.measure.points, points);
        assert_eq!(app.measure.unit, Unit::Centimeters);
        assert!(app.measure.enabled);
        assert_eq!(crate::project::encode(&app.doc).unwrap(), picture);
        assert!(app.file.is_none());
    }
}

#[test]
fn canvas_dimension_edits_clamp_points_without_making_them_document_history() {
    let ctx = Context::default();
    let mut app = measured_app(&ctx);
    let points = app.measure.points;
    app.doc.begin();
    app.doc.image.put_pixel(0, 0, Rgba(BLACK));
    app.doc.commit();
    frame(&mut app, &ctx, vec![]);
    assert_eq!(
        app.measure.points, points,
        "ordinary painting keeps the ruler"
    );

    app.resize_picture(48, 32, 0.0, 0.0).unwrap();
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.measure.points, points, "same-size resize is a no-op");
    assert!(app.resize_picture(0, 0, 0.0, 0.0).is_err());
    frame(&mut app, &ctx, vec![]);
    assert_eq!(
        app.measure.points, points,
        "invalid resize preserves the ruler"
    );

    app.measure.dragging = Some(1);
    app.resize_picture(20, 16, 0.0, 0.0).unwrap();
    frame(&mut app, &ctx, vec![]);
    let clamped = Some([(10, 12), (19, 15)]);
    assert_eq!(app.measure.points, clamped);
    assert!(app.measure.dragging.is_none());
    assert!(app.measure.enabled);
    assert_eq!(app.measure.unit, Unit::Centimeters);

    app.action(Action::Undo, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.doc.image.dimensions(), (48, 32));
    assert_eq!(
        app.measure.points, clamped,
        "Undo changes the image, not the ruler"
    );
    app.action(Action::Redo, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.doc.image.dimensions(), (20, 16));
    assert_eq!(app.measure.points, clamped);

    app.action(Action::Undo, &ctx);
    app.action(Action::Undo, &ctx);
    assert!(!app.doc.dirty() && !app.doc.can_undo());
}

#[test]
fn coalesced_measurement_keys_keep_each_press_modifier_and_order() {
    let ctx = Context::default();
    let mut app = measured_app(&ctx);
    let right = key(Key::ArrowRight, Modifiers::NONE);
    frame(&mut app, &ctx, vec![right.clone(), right.clone(), right]);
    assert_eq!(app.measure.points, Some([(10, 12), (38, 22)]));

    let mut released = key(Key::ArrowDown, Modifiers::SHIFT);
    if let Event::Key { pressed, .. } = &mut released {
        *pressed = false;
    }
    let mut repeated = key(Key::ArrowLeft, Modifiers::NONE);
    if let Event::Key { repeat, .. } = &mut repeated {
        *repeat = true;
    }
    frame(
        &mut app,
        &ctx,
        vec![
            key(Key::ArrowRight, Modifiers::SHIFT),
            key(Key::ArrowLeft, Modifiers::NONE),
            key(Key::ArrowLeft, Modifiers::SHIFT),
            key(Key::ArrowUp, Modifiers::NONE),
            released,
            repeated,
        ],
    );
    // Right clamps at 47 before subsequent Left steps. Releases do not move.
    assert_eq!(app.measure.points, Some([(10, 12), (35, 21)]));
    assert!(!app.doc.dirty() && !app.doc.can_undo());
}

#[test]
fn crop_and_rotate_keep_view_coordinates_inside_the_canvas_even_while_hidden() {
    for enabled in [true, false] {
        let ctx = Context::default();
        let mut app = measured_app(&ctx);
        app.measure.enabled = enabled;
        let points = app.measure.points;
        assert!(app.rotate_picture(180.0, false));
        frame(&mut app, &ctx, vec![]);
        assert_eq!(
            app.measure.points, points,
            "same-size rotation keeps coordinates"
        );

        assert!(app.rotate_picture(90.0, false));
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.doc.image.dimensions(), (32, 48));
        assert_eq!(app.measure.points, Some([(10, 12), (31, 22)]));

        app.selection = Some(Region {
            x: 4,
            y: 5,
            w: 8,
            h: 6,
        });
        app.action(Action::Crop, &ctx);
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.doc.image.dimensions(), (8, 6));
        assert_eq!(app.measure.points, Some([(7, 5), (7, 5)]));
        assert_eq!(app.measure.enabled, enabled);
    }
}

#[test]
fn measurement_reset_and_exit_follow_preceding_arrows_in_the_same_batch() {
    let ctx = Context::default();
    let mut app = measured_app(&ctx);
    frame(
        &mut app,
        &ctx,
        vec![
            key(Key::ArrowRight, Modifiers::NONE),
            key(Key::Escape, Modifiers::NONE),
        ],
    );
    assert_eq!(app.measure.points, Some([(10, 12), (36, 22)]));
    assert!(!app.measure.enabled);
    app.set_measure_enabled(true, &ctx);
    frame(
        &mut app,
        &ctx,
        vec![
            key(Key::ArrowRight, Modifiers::NONE),
            key(Key::Delete, Modifiers::NONE),
            key(Key::ArrowLeft, Modifiers::NONE),
        ],
    );
    assert!(app.measure.points.is_none());
    assert!(!app.doc.dirty() && !app.doc.can_undo());
}

#[test]
fn measurement_keys_respect_dialog_menu_and_numeric_focus_ownership() {
    for owner in ["dialog", "pending", "menu", "numeric"] {
        let ctx = Context::default();
        let mut app = measured_app(&ctx);
        let points = app.measure.points;
        let _ = ctx.run(
            RawInput {
                events: vec![key(Key::ArrowRight, Modifiers::NONE)],
                ..Default::default()
            },
            |ctx| {
                match owner {
                    "dialog" => app.dialog = Some(Dialog::Properties),
                    "pending" => app.pending = Some(Action::New),
                    "menu" => ctx.memory_mut(|memory| memory.open_popup(Id::new("measure_menu"))),
                    "numeric" => ctx.memory_mut(|memory| memory.request_focus(Id::new("number"))),
                    _ => unreachable!(),
                }
                app.shortcut(ctx);
                assert!(
                    ctx.input(|input| input.key_pressed(Key::ArrowRight)),
                    "{owner}"
                );
            },
        );
        assert_eq!(app.measure.points, points, "{owner}");
    }
}
