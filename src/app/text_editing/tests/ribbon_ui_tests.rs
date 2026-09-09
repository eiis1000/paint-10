use super::*;

fn bounds(output: &FullOutput, label: &str) -> Rect {
    let node = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some(label))
        .unwrap_or_else(|| panic!("Missing text control {label}"));
    let rect = node.1.bounds().unwrap();
    Rect::from_min_max(
        pos2(rect.x0 as f32, rect.y0 as f32),
        pos2(rect.x1 as f32, rect.y1 as f32),
    )
}

#[test]
fn normal_window_exposes_font_background_alignment_and_effect_controls() {
    let ctx = Context::default();
    ctx.enable_accesskit();
    let mut app = editing_app(&ctx, "Short text sample");
    let mut output = app_frame_at_width(&mut app, &ctx, vec![], 1180.0);
    for _ in 0..3 {
        output = app_frame_at_width(&mut app, &ctx, vec![], 1180.0);
    }
    for label in [
        "Paste",
        "Cut",
        "Copy",
        "Font",
        "Font list",
        "Font size",
        "Font size list",
        "Grow font",
        "Shrink font",
        "Bold",
        "Italic",
        "Underline",
        "Strikeout",
        "Opaque",
        "Transparent",
        "Align left",
        "Center",
        "Align right",
        "Text outline width",
        "Black outline",
        "White outline",
        "Use Color 1 for outline",
        "Done",
        "Cancel",
    ] {
        let rect = bounds(&output, label);
        assert!(
            rect.left() >= 0.0 && rect.right() <= 1180.0,
            "{label}: {rect:?}"
        );
        assert!(
            rect.bottom() < app.canvas_rect.top(),
            "{label} overlaps the canvas"
        );
    }
}

#[test]
fn font_presets_and_size_steps_format_only_the_selection_and_undo() {
    let ctx = Context::default();
    ctx.enable_accesskit();
    let mut app = editing_app(&ctx, "First second");
    app.text_edit.as_mut().unwrap().selection = 0..5;
    app.text_edit.as_mut().unwrap().focus = true;
    app_frame(&mut app, &ctx, vec![]);
    let output = app_frame(&mut app, &ctx, vec![]);
    click(&mut app, &ctx, bounds(&output, "Font size list").center());
    let mut output = app_frame(&mut app, &ctx, vec![]);
    for _ in 0..3 {
        output = app_frame(&mut app, &ctx, vec![]);
    }
    click(&mut app, &ctx, bounds(&output, "12 pt").center());
    app_frame(&mut app, &ctx, vec![]);
    let state = app.text_edit.as_ref().unwrap();
    assert_eq!(state.text, "First second");
    assert_eq!(
        crate::text::pixels_to_points(state.format.style_at(0).size),
        12.0
    );
    assert_eq!(
        crate::text::pixels_to_points(state.format.style_at(7).size),
        18.0
    );
    assert_eq!(state.selection, 0..5);
    assert!(!keytips::popup_open(&ctx));
    let output = app_frame(&mut app, &ctx, vec![]);
    click(&mut app, &ctx, bounds(&output, "Grow font").center());
    app_frame(&mut app, &ctx, vec![]);
    assert_eq!(
        crate::text::pixels_to_points(app.text_edit.as_ref().unwrap().format.style_at(0).size),
        14.0,
    );
    app_frame(&mut app, &ctx, vec![key(Key::Z, Modifiers::CTRL)]);
    assert_eq!(
        crate::text::pixels_to_points(app.text_edit.as_ref().unwrap().format.style_at(0).size),
        12.0,
    );
    app_frame(&mut app, &ctx, vec![key(Key::Z, Modifiers::CTRL)]);
    assert_eq!(
        crate::text::pixels_to_points(app.text_edit.as_ref().unwrap().format.style_at(0).size),
        18.0,
    );
}
