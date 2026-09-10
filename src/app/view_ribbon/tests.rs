use super::*;

use egui::accesskit::{Node, Role, Toggled};

const TOGGLES: [(&str, &str, Key); 6] = [
    ("Rulers", "Canvas", Key::R),
    ("Gridlines", "Canvas", Key::G),
    ("Layers", "Panels", Key::L),
    ("Thumbnail", "Panels", Key::T),
    ("Status bar", "Panels", Key::S),
    ("Measure distance", "Measure", Key::M),
];

fn fixture() -> (PaintApp, Context) {
    let ctx = Context::default();
    ctx.enable_accesskit();
    let mut app = PaintApp::new_with_context(&ctx, false);
    app.doc = Document::new(48, 32);
    app.view_tab = true;
    app.zoom = 2.0;
    (app, ctx)
}

fn frame(app: &mut PaintApp, ctx: &Context, size: Vec2, events: Vec<Event>) -> FullOutput {
    let mut input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
        time: Some(ctx.cumulative_pass_nr() as f64 / 30.0),
        events,
        ..Default::default()
    };
    eframe::App::raw_input_hook(app, ctx, &mut input);
    ctx.run(input, |ctx| {
        if !app.ribbon_keyboard(ctx) {
            app.shortcut(ctx);
        }
        if app.preview {
            CentralPanel::default().show(ctx, |_| {});
        } else {
            app.titlebar(ctx);
            app.ribbon(ctx);
            app.quick_access_below(ctx);
            app.status(ctx);
            app.layers_panel(ctx);
            app.canvas(ctx);
            app.thumbnail(ctx);
        }
        app.keyboard_menu(ctx);
        app.dialogs(ctx);
    })
}

fn settle(app: &mut PaintApp, ctx: &Context, size: Vec2) -> FullOutput {
    for _ in 0..4 {
        frame(app, ctx, size, vec![]);
    }
    frame(app, ctx, size, vec![])
}

fn find_node<'a>(output: &'a FullOutput, label: &str) -> Option<&'a Node> {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .map(|(_, node)| node)
        .find(|node| {
            node.label() == Some(label)
                && matches!(node.role(), Role::Button | Role::ComboBox)
                && (node.role() == Role::ComboBox
                    || matches!(
                        label,
                        "Pixels" | "Millimeters" | "Centimeters" | "Inches" | "Reset measurement"
                    )
                    || bounds(node).height() >= 64.0)
        })
}

fn node<'a>(output: &'a FullOutput, label: &str) -> &'a Node {
    find_node(output, label).unwrap_or_else(|| panic!("Missing View control: {label}"))
}

fn bounds(node: &Node) -> Rect {
    let bounds = node.bounds().unwrap();
    Rect::from_min_max(
        pos2(bounds.x0 as f32, bounds.y0 as f32),
        pos2(bounds.x1 as f32, bounds.y1 as f32),
    )
}

fn click(app: &mut PaintApp, ctx: &Context, size: Vec2, position: Pos2) {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            size,
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

fn keys(app: &mut PaintApp, ctx: &Context, size: Vec2, keys: &[Key]) {
    for &key in keys {
        frame(
            app,
            ctx,
            size,
            vec![Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        settle(app, ctx, size);
    }
}

fn reveal(app: &mut PaintApp, ctx: &Context, size: Vec2, group: &str, label: &str) -> FullOutput {
    let mut output = settle(app, ctx, size);
    if find_node(&output, label).is_none() {
        click(app, ctx, size, bounds(node(&output, group)).center());
        output = settle(app, ctx, size);
    }
    node(&output, label);
    output
}

fn dismiss_popup(app: &mut PaintApp, ctx: &Context, size: Vec2) {
    for _ in 0..3 {
        if !keytips::popup_open(ctx) {
            return;
        }
        keys(app, ctx, size, &[Key::Escape]);
    }
    assert!(!keytips::popup_open(ctx));
}

fn invoke_keytip(
    app: &mut PaintApp,
    ctx: &Context,
    size: Vec2,
    group: &str,
    label: &str,
    key: Key,
) {
    dismiss_popup(app, ctx, size);
    // Escape can close a group while leaving focus in the ribbon. Return to
    // the canvas before starting a fresh F10 sequence, as a user would.
    for _ in 0..3 {
        if !keytips::active(ctx) {
            break;
        }
        keys(app, ctx, size, &[Key::Escape]);
    }
    assert!(!keytips::active(ctx));
    let output = settle(app, ctx, size);
    let collapsed = find_node(&output, label).is_none();
    keys(app, ctx, size, &[Key::F10, Key::V]);
    if collapsed {
        let group_key = match group {
            "Zoom" => Key::Z,
            "Canvas" => Key::C,
            "Panels" => Key::P,
            "Display" => Key::D,
            "Measure" => Key::M,
            _ => panic!("Unknown View group: {group}"),
        };
        keys(app, ctx, size, &[Key::Z, group_key]);
    }
    keys(app, ctx, size, &[key]);
}

fn toggle_state(app: &PaintApp, ctx: &Context, label: &str) -> bool {
    match label {
        "Rulers" => app.rulers,
        "Gridlines" => app.grid,
        "Layers" => app.layer_ui.open,
        "Thumbnail" => PaintApp::thumbnail_enabled(ctx),
        "Status bar" => app.status_bar,
        "Measure distance" => app.measure.enabled,
        _ => panic!("Unknown View toggle: {label}"),
    }
}

fn assert_toggle(node: &Node, selected: bool) {
    assert_eq!(node.role(), Role::Button);
    assert_eq!(
        node.toggled(),
        Some(if selected {
            Toggled::True
        } else {
            Toggled::False
        })
    );
}

#[test]
fn view_uses_graphical_buttons_with_accessible_state_and_unclipped_labels() {
    for size in [vec2(1200.0, 760.0), vec2(500.0, 400.0)] {
        let (mut app, ctx) = fixture();
        for (group, labels) in [
            (
                "Zoom",
                &["Zoom in", "Zoom out", "100%", "Fit to window"][..],
            ),
            ("Canvas", &["Rulers", "Gridlines"][..]),
            ("Panels", &["Layers", "Thumbnail", "Status bar"][..]),
            ("Display", &["Full screen"][..]),
            (
                "Measure",
                &[
                    "Measure distance",
                    "Pixels",
                    "Millimeters",
                    "Centimeters",
                    "Inches",
                    "Reset measurement",
                ][..],
            ),
        ] {
            let output = reveal(&mut app, &ctx, size, group, labels[0]);
            let nodes = &output
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap()
                .nodes;
            assert!(nodes.iter().all(|(_, node)| node.role() != Role::CheckBox));
            let mut rectangles = Vec::new();
            for &label in labels {
                let control = node(&output, label);
                let rect = bounds(control);
                assert!(
                    Rect::from_min_size(Pos2::ZERO, size).contains_rect(rect),
                    "{label}: {rect:?}"
                );
                assert!(
                    rect.height()
                        >= if control.role() == Role::ComboBox {
                            18.0
                        } else {
                            24.0
                        }
                        && rect.width() >= 32.0,
                    "{label}: {rect:?}"
                );
                for previous in &rectangles {
                    assert!(
                        !rect.intersect(*previous).is_positive(),
                        "Overlapping {group} controls"
                    );
                }
                rectangles.push(rect);
                if TOGGLES.iter().any(|&(toggle, _, _)| toggle == label) {
                    assert_toggle(control, toggle_state(&app, &ctx, label));
                }
            }
            for shape in &output.shapes {
                if let Shape::Text(text) = &shape.shape {
                    let content = text.galley.text().replace('\n', " ");
                    if labels.contains(&content.as_str()) {
                        let rect = text.galley.rect.translate(text.pos.to_vec2());
                        assert!(
                            shape.clip_rect.contains_rect(rect),
                            "Clipped {content}: {rect:?}"
                        );
                    }
                }
            }
            keys(&mut app, &ctx, size, &[Key::Escape]);
        }
    }
}

#[test]
fn each_view_toggle_changes_actual_state_and_closes_compact_popups() {
    for size in [vec2(1200.0, 760.0), vec2(500.0, 400.0)] {
        for (label, group, _) in TOGGLES {
            let (mut app, ctx) = fixture();
            let original = app.doc.composite();
            for _ in 0..2 {
                let before = toggle_state(&app, &ctx, label);
                let output = reveal(&mut app, &ctx, size, group, label);
                assert_toggle(node(&output, label), before);
                click(&mut app, &ctx, size, bounds(node(&output, label)).center());
                settle(&mut app, &ctx, size);
                assert_eq!(
                    toggle_state(&app, &ctx, label),
                    !before,
                    "{label} at {size:?}"
                );
                assert!(
                    !keytips::popup_open(&ctx),
                    "{label} left its group popup open"
                );
                let output = reveal(&mut app, &ctx, size, group, label);
                assert_toggle(node(&output, label), !before);
                dismiss_popup(&mut app, &ctx, size);
            }
            assert_eq!(app.doc.composite(), original);
            assert!(!app.doc.can_undo());
        }
    }
}

#[test]
fn view_keytips_reach_toggles_in_expanded_and_collapsed_groups() {
    for size in [vec2(1200.0, 760.0), vec2(500.0, 400.0)] {
        for (label, group, key) in TOGGLES {
            let (mut app, ctx) = fixture();
            let before = toggle_state(&app, &ctx, label);
            invoke_keytip(&mut app, &ctx, size, group, label, key);
            assert_eq!(
                toggle_state(&app, &ctx, label),
                !before,
                "{label} at {size:?}"
            );
            assert!(!keytips::popup_open(&ctx));
            assert!(!app.doc.can_undo());
        }
    }
}

#[test]
fn zoom_limits_and_thumbnail_availability_disable_the_actual_controls() {
    for size in [vec2(1200.0, 760.0), vec2(500.0, 400.0)] {
        for (zoom, label) in [(MIN_ZOOM, "Zoom out"), (MAX_ZOOM, "Zoom in")] {
            let (mut app, ctx) = fixture();
            app.zoom = zoom;
            let output = reveal(&mut app, &ctx, size, "Zoom", label);
            assert!(node(&output, label).is_disabled());
            click(&mut app, &ctx, size, bounds(node(&output, label)).center());
            settle(&mut app, &ctx, size);
            assert_eq!(app.zoom, zoom);
        }
        for zoom in [0.5, 1.0, 2.0] {
            let (mut app, ctx) = fixture();
            app.zoom = zoom;
            let output = reveal(&mut app, &ctx, size, "Panels", "Thumbnail");
            assert_eq!(node(&output, "Thumbnail").is_disabled(), zoom <= 1.0);
            click(
                &mut app,
                &ctx,
                size,
                bounds(node(&output, "Thumbnail")).center(),
            );
            settle(&mut app, &ctx, size);
            assert_eq!(PaintApp::thumbnail_enabled(&ctx), zoom > 1.0);
        }
    }
}

#[test]
fn zoom_and_full_screen_commands_work_by_keyboard_and_pointer() {
    for size in [vec2(1200.0, 760.0), vec2(500.0, 400.0)] {
        let (mut app, ctx) = fixture();
        for (label, key, expected) in [
            ("Zoom in", Key::I, 4.0),
            ("Zoom out", Key::O, 2.0),
            ("100%", Key::Num1, 1.0),
        ] {
            invoke_keytip(&mut app, &ctx, size, "Zoom", label, key);
            assert_eq!(app.zoom, expected);
            assert!(!keytips::popup_open(&ctx));
        }
        for keyboard in [false, true] {
            if keyboard {
                invoke_keytip(&mut app, &ctx, size, "Display", "Full screen", Key::F);
            } else {
                let output = reveal(&mut app, &ctx, size, "Display", "Full screen");
                click(
                    &mut app,
                    &ctx,
                    size,
                    bounds(node(&output, "Full screen")).center(),
                );
            }
            assert!(app.preview && app.fullscreen);
            keys(&mut app, &ctx, size, &[Key::Escape]);
            assert!(!app.preview && !app.fullscreen);
        }
        assert!(!app.doc.can_undo());
    }
}

#[test]
fn fit_to_window_uses_the_canvas_viewport_with_layers_and_rulers() {
    for size in [vec2(1200.0, 760.0), vec2(500.0, 400.0)] {
        for rulers in [false, true] {
            let mut previous_zoom = None;
            for layers in [false, true] {
                let (mut app, ctx) = fixture();
                app.doc = Document::new(2000, 600);
                app.rulers = rulers;
                app.layer_ui.open = layers;
                settle(&mut app, &ctx, size);
                invoke_keytip(&mut app, &ctx, size, "Zoom", "Fit to window", Key::W);
                settle(&mut app, &ctx, size);
                let viewport = ctx
                    .data(|data| data.get_temp::<Rect>(Id::new("paint10_canvas_viewport")))
                    .unwrap();
                assert!(
                    viewport.expand(1.0).contains_rect(app.canvas_rect),
                    "Canvas {:?} exceeds {viewport:?}",
                    app.canvas_rect
                );
                assert!(app.zoom > MIN_ZOOM && app.zoom < 1.0);
                if let Some(without_layers) = previous_zoom {
                    assert!(
                        app.zoom < without_layers,
                        "Layers must reduce available canvas width"
                    );
                }
                previous_zoom = Some(app.zoom);
                assert!(!keytips::popup_open(&ctx));
                assert!(!app.doc.can_undo());
            }
        }
    }
}

#[test]
fn measurement_units_and_reset_operate_on_a_real_ruler_without_image_history() {
    for size in [vec2(1200.0, 760.0), vec2(500.0, 400.0)] {
        let (mut app, ctx) = fixture();
        invoke_keytip(&mut app, &ctx, size, "Measure", "Measure distance", Key::M);
        let start = app.canvas_rect.min + vec2(4.0, 4.0) * app.zoom;
        let end = app.canvas_rect.min + vec2(24.0, 14.0) * app.zoom;
        for (position, pressed) in [(start, true), (end, false)] {
            frame(
                &mut app,
                &ctx,
                size,
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
        let output = reveal(&mut app, &ctx, size, "Measure", "Reset measurement");
        assert!(!node(&output, "Reset measurement").is_disabled());
        dismiss_popup(&mut app, &ctx, size);
        invoke_keytip(&mut app, &ctx, size, "Measure", "Inches", Key::U);
        keys(&mut app, &ctx, size, &[Key::I]);
        let output = settle(&mut app, &ctx, size);
        let visible_text: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                Shape::Text(text) => Some(text.galley.text()),
                _ => None,
            })
            .collect();
        assert!(
            visible_text.iter().any(|text| text.ends_with(" in")),
            "No physical measurement at {size:?}; enabled={}; labels={visible_text:?}",
            app.measure.enabled,
        );
        invoke_keytip(&mut app, &ctx, size, "Measure", "Reset measurement", Key::C);
        let output = reveal(&mut app, &ctx, size, "Measure", "Reset measurement");
        assert!(node(&output, "Reset measurement").is_disabled());
        assert!(app.measure.enabled);
        assert!(!app.doc.can_undo());
    }
}

#[test]
fn measurement_unit_buttons_update_the_ruler_in_expanded_and_compact_groups() {
    for size in [vec2(1200.0, 760.0), vec2(500.0, 400.0)] {
        let (mut app, ctx) = fixture();
        app.layer_ui.open = true;
        app.rulers = true;
        let output = reveal(&mut app, &ctx, size, "Measure", "Measure distance");
        click(
            &mut app,
            &ctx,
            size,
            bounds(node(&output, "Measure distance")).center(),
        );
        settle(&mut app, &ctx, size);
        assert!(app.measure.enabled);

        let start = app.canvas_rect.min + vec2(4.0, 4.0) * app.zoom;
        let end = app.canvas_rect.min + vec2(24.0, 14.0) * app.zoom;
        for (position, pressed) in [(start, true), (end, false)] {
            frame(
                &mut app,
                &ctx,
                size,
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

        for (label, suffix) in [
            ("Inches", " in"),
            ("Millimeters", " mm"),
            ("Centimeters", " cm"),
            ("Pixels", " px"),
        ] {
            let output = reveal(&mut app, &ctx, size, "Measure", label);
            assert!(!node(&output, "Reset measurement").is_disabled());
            click(&mut app, &ctx, size, bounds(node(&output, label)).center());
            let output = settle(&mut app, &ctx, size);
            assert!(output.shapes.iter().any(|shape| {
                matches!(&shape.shape, Shape::Text(text) if text.galley.text().ends_with(suffix))
            }), "Clicking {label} must update the ruler at {size:?}");
            let output = reveal(&mut app, &ctx, size, "Measure", label);
            assert_toggle(node(&output, label), true);
            dismiss_popup(&mut app, &ctx, size);
        }
        assert!(app.measure.enabled);
        assert!(!app.doc.can_undo());
    }
}
