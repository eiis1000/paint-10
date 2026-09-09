use super::*;

fn app() -> (PaintApp, Context) {
    let ctx = Context::default();
    let mut app = PaintApp::new_with_context(&ctx, false);
    app.doc = Document::new(40, 30);
    (app, ctx)
}

fn run_frame(app: &mut PaintApp, ctx: &Context, events: Vec<Event>) -> (FullOutput, Rect) {
    let mut canvas = Rect::NOTHING;
    let output = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(500.0, 400.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            app.shortcut(ctx);
            app.layers_panel(ctx);
            canvas = ctx.available_rect();
            app.canvas(ctx);
        },
    );
    (output, canvas)
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

fn text_position(output: &FullOutput, label: &str) -> Pos2 {
    output
        .shapes
        .iter()
        .find_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape {
                (text.galley.text() == label).then(|| text.pos + text.galley.size() / 2.0)
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("No visible {label:?} label"))
}

fn click(app: &mut PaintApp, ctx: &Context, position: Pos2) {
    for pressed in [true, false] {
        run_frame(
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
}

#[test]
fn pane_is_optional_and_leaves_canvas_space_on_a_small_window() {
    let (mut app, ctx) = app();
    let (_, canvas) = run_frame(&mut app, &ctx, vec![]);
    assert_eq!(canvas.width(), 500.0);
    app.reveal_layers();
    let (output, canvas) = run_frame(&mut app, &ctx, vec![]);
    assert!(canvas.width() >= 315.0, "{canvas:?}");
    assert!(canvas.height() >= 390.0);
    let add = text_position(&output, "Add");
    click(&mut app, &ctx, add);
    assert_eq!(app.doc.layer_count(), 2);
    assert_eq!(app.doc.active_layer_index(), 1);
    let (output, _) = run_frame(&mut app, &ctx, vec![]);
    let delete = text_position(&output, "Delete");
    click(&mut app, &ctx, delete);
    assert_eq!(app.doc.layer_count(), 1);
    app.doc.undo();
    assert_eq!(app.doc.layer_count(), 2);
}

#[test]
fn pane_buttons_keep_global_undo_and_inline_rename_owns_text_shortcuts() {
    let (mut app, ctx) = app();
    app.reveal_layers();
    let (output, _) = run_frame(&mut app, &ctx, vec![]);
    click(&mut app, &ctx, text_position(&output, "Add"));
    let command = Modifiers::CTRL | Modifiers::COMMAND;
    run_frame(&mut app, &ctx, vec![key(Key::Z, command)]);
    assert_eq!(app.doc.layer_count(), 1);
    run_frame(&mut app, &ctx, vec![key(Key::Y, command)]);
    assert_eq!(app.doc.layer_count(), 2);

    app.apply_layer_action(LayerAction::Rename(1), &ctx);
    run_frame(&mut app, &ctx, vec![]);
    run_frame(
        &mut app,
        &ctx,
        vec![key(Key::A, command), Event::Text("Captions".into())],
    );
    run_frame(&mut app, &ctx, vec![key(Key::Enter, Modifiers::NONE)]);
    assert_eq!(app.doc.layers()[1].name, "Captions");
    assert!(app.selection.is_none());
    assert!(app.layer_ui.rename.is_none());
    assert!(ctx.memory(|memory| memory.has_focus(Id::new("canvas"))));
}

#[test]
fn keyboard_context_menu_opens_layer_options_and_escape_preserves_pixels() {
    let (mut app, ctx) = app();
    app.reveal_layers();
    let (output, _) = run_frame(&mut app, &ctx, vec![]);
    click(&mut app, &ctx, text_position(&output, "Background"));
    let original = app.doc.composite();
    assert!(app.open_layer_context_menu(&ctx));
    // Egui measures a newly opened popup before painting its first visible pass.
    run_frame(&mut app, &ctx, vec![]);
    let (output, _) = run_frame(&mut app, &ctx, vec![]);
    text_position(&output, "Rename…");
    text_position(&output, "Opacity");
    assert!(!app.keyboard_context_menu);
    run_frame(&mut app, &ctx, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(app.layer_ui.keyboard_menu.is_none());
    assert!(app.doc.composite() == original);
}

#[test]
fn inline_rename_preserves_row_geometry_and_selects_the_existing_name() {
    let (mut app, ctx) = app();
    app.apply_layer_action(LayerAction::Add, &ctx);
    app.reveal_layers();
    let (output, _) = run_frame(&mut app, &ctx, vec![]);
    let background_position = text_position(&output, "Background");
    app.apply_layer_action(LayerAction::Rename(1), &ctx);
    let (output, _) = run_frame(&mut app, &ctx, vec![]);
    assert_eq!(text_position(&output, "Background"), background_position);
    run_frame(&mut app, &ctx, vec![Event::Text("Meme captions".into())]);
    run_frame(&mut app, &ctx, vec![key(Key::Enter, Modifiers::NONE)]);
    assert_eq!(app.doc.layers()[1].name, "Meme captions");
    let (output, _) = run_frame(&mut app, &ctx, vec![]);
    assert_eq!(text_position(&output, "Background"), background_position);
}

#[test]
fn switching_layers_commits_text_on_its_original_layer_and_keeps_text_tool() {
    let (mut app, ctx) = app();
    app.apply_layer_action(LayerAction::Add, &ctx);
    app.tool = Tool::Text;
    app.text_edit = Some(TextEditState {
        index: None,
        origin: (3, 4),
        text: "Caption".into(),
        format: crate::text::TextFormat::default(),
        focus: false,
        selection: 7..7,
        insertion_style: None,
        history: text_editing::TextHistory::default(),
        palette_colors: app.colors,
    });
    app.switch_layer(0, &ctx);
    assert!(app.text_edit.is_none());
    assert!(app.object.is_none());
    assert!(app.doc.layers()[0].objects.is_empty());
    assert_eq!(app.doc.layers()[1].objects.len(), 1);
    assert!(
        matches!(&app.doc.layers()[1].objects[0].kind, ObjectKind::Text { text, .. } if text == "Caption")
    );
    assert_eq!(app.tool, Tool::Text);
}

#[test]
fn switching_layers_finishes_a_gesture_without_reusing_its_target() {
    let (mut app, ctx) = app();
    app.apply_layer_action(LayerAction::Add, &ctx);
    app.doc.begin();
    app.doc.image.put_pixel(2, 3, Rgba([255, 0, 0, 255]));
    app.gesture = Some(Gesture::Paint {
        start: (2, 3),
        last: (2, 3),
        color: [255, 0, 0, 255],
        erase_target: None,
        first: false,
    });
    app.selection = Some(Region {
        x: 1,
        y: 1,
        w: 5,
        h: 5,
    });
    app.free_points.push((2, 2));
    app.mask = Some(image::GrayImage::new(5, 5));
    app.switch_layer(0, &ctx);
    assert!(app.gesture.is_none());
    assert!(app.selection.is_none());
    assert!(app.mask.is_none());
    assert!(app.free_points.is_empty());
    assert_eq!(app.doc.layers()[0].image.get_pixel(2, 3).0, WHITE);
    assert_eq!(
        app.doc.layers()[1].image.get_pixel(2, 3).0,
        [255, 0, 0, 255]
    );
    app.doc.undo();
    assert_eq!(app.doc.layers()[1].image.get_pixel(2, 3)[3], 0);
}

#[test]
fn an_opacity_drag_has_one_undo_and_never_changes_the_active_layer() {
    let (mut app, ctx) = app();
    app.apply_layer_action(LayerAction::Add, &ctx);
    for value in [220, 170, 90] {
        app.apply_layer_action(
            LayerAction::Opacity {
                index: 0,
                value,
                dragging: true,
            },
            &ctx,
        );
    }
    assert_eq!(app.doc.active_layer_index(), 1);
    app.finish_layer_opacity();
    assert_eq!(app.doc.layers()[0].opacity, 90);
    app.doc.undo();
    assert_eq!(app.doc.layers()[0].opacity, 255);
    assert_eq!(app.doc.layer_count(), 2);
    app.doc.undo();
    assert_eq!(app.doc.layer_count(), 1);
}

#[test]
fn the_visible_opacity_slider_changes_pixels_and_undoes_one_drag() {
    let (mut app, ctx) = app();
    app.apply_layer_action(LayerAction::Add, &ctx);
    app.doc.image.put_pixel(0, 0, Rgba(BLACK));
    app.reveal_layers();
    let (output, _) = run_frame(&mut app, &ctx, vec![]);
    click(&mut app, &ctx, text_position(&output, "More"));
    run_frame(&mut app, &ctx, vec![]);
    run_frame(&mut app, &ctx, vec![]);
    let rect = ctx
        .data(|data| data.get_temp::<Rect>(Id::new("paint10-layer-opacity-test-rect")))
        .unwrap();
    let start = rect.left_center() + vec2(85.0, 0.0);
    run_frame(
        &mut app,
        &ctx,
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
    for offset in [70.0, 50.0, 30.0] {
        run_frame(
            &mut app,
            &ctx,
            vec![Event::PointerMoved(rect.left_center() + vec2(offset, 0.0))],
        );
    }
    run_frame(
        &mut app,
        &ctx,
        vec![Event::PointerButton {
            pos: rect.left_center() + vec2(30.0, 0.0),
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert!(
        app.doc.active_layer().opacity < 200,
        "opacity={}",
        app.doc.active_layer().opacity
    );
    assert!(app.doc.composite().get_pixel(0, 0)[0] > 50);
    app.doc.undo();
    assert_eq!(app.doc.active_layer().opacity, 255);
    assert_eq!(app.doc.layer_count(), 2);
}

#[test]
fn pending_dialogs_block_layer_changes_and_reset_discards_stale_ui_indices() {
    let (mut app, ctx) = app();
    app.reveal_layers();
    app.apply_layer_action(LayerAction::Add, &ctx);
    app.dialog = Some(Dialog::Resize);
    app.switch_layer(0, &ctx);
    app.apply_layer_action(LayerAction::Delete(1), &ctx);
    assert_eq!(app.doc.active_layer_index(), 1);
    assert_eq!(app.doc.layer_count(), 2);
    app.dialog = None;
    app.apply_layer_action(LayerAction::Rename(1), &ctx);
    app.doc = Document::new(20, 20);
    app.reset_layer_panel_state();
    assert!(app.layer_ui.open);
    assert!(app.layer_ui.rename.is_none());
    assert!(app.layer_ui.thumbnails.is_empty());
}

#[test]
fn names_accept_unicode_without_exceeding_the_project_name_limit() {
    let (mut app, ctx) = app();
    app.apply_layer_action(
        LayerAction::SetName(0, format!("  {}  ", "🎨".repeat(128))),
        &ctx,
    );
    assert_eq!(app.doc.active_layer().name.len(), 256);
    assert_eq!(app.doc.active_layer().name.chars().count(), 64);
    app.apply_layer_action(LayerAction::SetName(0, " \t ".into()), &ctx);
    assert!(!app.doc.active_layer().name.is_empty());
    let bytes = crate::project::encode(&app.doc).unwrap();
    assert!(!bytes.is_empty());
}

#[test]
fn dragging_to_an_insertion_boundary_reorders_in_the_displayed_direction() {
    let (mut app, ctx) = app();
    app.apply_layer_action(LayerAction::Add, &ctx);
    app.apply_layer_action(LayerAction::Add, &ctx);
    app.reveal_layers();
    let (output, _) = run_frame(&mut app, &ctx, vec![]);
    let from = text_position(&output, "Background") + vec2(0.0, 8.0);
    let top = text_position(&output, "Layer 2") - vec2(0.0, 6.0);
    run_frame(
        &mut app,
        &ctx,
        vec![
            Event::PointerMoved(from),
            Event::PointerButton {
                pos: from,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    run_frame(
        &mut app,
        &ctx,
        vec![Event::PointerMoved(from + vec2(0.0, -12.0))],
    );
    run_frame(&mut app, &ctx, vec![Event::PointerMoved(top)]);
    run_frame(
        &mut app,
        &ctx,
        vec![Event::PointerButton {
            pos: top,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert_eq!(app.doc.layers()[2].name, "Background");
    assert_eq!(app.doc.layers()[0].name, "Layer 1");
    app.doc.undo();
    assert_eq!(app.doc.layers()[0].name, "Background");
    assert_eq!(drop_destination(2, 0, false), 0);
    assert_eq!(drop_destination(0, 2, true), 2);
    assert_eq!(drop_destination(1, 1, true), 1);
    assert_eq!(drop_destination(1, 1, false), 1);
}
