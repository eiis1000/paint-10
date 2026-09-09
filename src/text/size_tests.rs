use super::*;

#[test]
fn new_text_has_a_regular_bundled_face_and_old_empty_fonts_keep_their_face() {
    let new = TextFormat::default();
    assert_eq!(new.font_name, DEFAULT_FONT_NAME);
    assert_eq!(new.font.as_ref(), DEFAULT_FONT);
    let face = rustybuzz::Face::from_slice(DEFAULT_FONT, 0).unwrap();
    assert_eq!(face.weight().to_number(), 400);
    assert!(!face.is_italic());
    let old = TextFormat {
        font_name: "Sans serif".into(),
        font: FontBytes::default(),
        size_mode: TextSizeMode::FontHeight,
        ..Default::default()
    };
    let layout = old.layout("Legacy");
    assert_eq!(
        layout.styles[0].font.font_data(),
        epaint_default_fonts::UBUNTU_LIGHT
    );
    assert_eq!(layout.styles[0].size, old.size);
}

#[test]
fn point_sizes_follow_the_fonts_em_units_in_metrics_and_rasterization() {
    for bytes in [
        epaint_default_fonts::UBUNTU_LIGHT,
        epaint_default_fonts::HACK_REGULAR,
        DEFAULT_FONT,
    ] {
        let font = FontRef::try_from_slice(bytes).unwrap();
        for points in [9.0, 12.0, 18.0, 32.0] {
            let format = TextFormat {
                font: bytes.into(),
                size: points_to_pixels(points),
                width: 600,
                ..Default::default()
            };
            let layout = format.layout("H");
            let expected = font.pt_to_px_scale(points).unwrap();
            assert!((layout.styles[0].size - expected.y).abs() < 0.0001);
            let advance = font.h_advance_unscaled(font.glyph_id('H')) * points_to_pixels(points)
                / font.units_per_em().unwrap();
            assert!((layout.lines[0].advance - advance).abs() < 0.0001);
            let height_sized = TextFormat {
                size: layout.styles[0].size,
                size_mode: TextSizeMode::FontHeight,
                ..format.clone()
            };
            assert_eq!(format.render("H"), height_sized.render("H"));
        }
    }
}

#[test]
fn missing_size_mode_preserves_legacy_rich_text_pixels_and_geometry() {
    let text = "Old caption\nMixed text";
    let mut legacy = TextFormat {
        size_mode: TextSizeMode::FontHeight,
        width: 180,
        ..Default::default()
    };
    legacy
        .modify_style(4..11, |style| {
            style.size = 30.0;
            style.font = epaint_default_fonts::HACK_REGULAR.into();
            style.bold = true;
        })
        .unwrap();
    let mut json = serde_json::to_value(&legacy).unwrap();
    json.as_object_mut().unwrap().remove("size_mode");
    let decoded: TextFormat = serde_json::from_value(json).unwrap();
    assert_eq!(decoded.size_mode, TextSizeMode::FontHeight);
    assert!(decoded == legacy);
    assert_eq!(decoded.render(text), legacy.render(text));
    assert_eq!(decoded.dimensions(text), legacy.dimensions(text));
    assert_eq!(TextFormat::default().size_mode, TextSizeMode::Em);
}

#[test]
fn fallback_faces_resolve_their_own_em_scale_without_changing_saved_styles() {
    let fallback = include_bytes!("../../assets/test-fonts/NotoSansDevanagari.ttf");
    let format = TextFormat {
        size: points_to_pixels(18.0),
        font_faces: vec![EmbeddedFont {
            family: "Indic fallback".into(),
            data: fallback.into(),
            index: 0,
            bold: false,
            italic: false,
        }],
        ..Default::default()
    };
    let before = serde_json::to_vec(&format).unwrap();
    let expected = TextFormat {
        font: fallback.into(),
        ..format.clone()
    };
    // Use only unsupported letters: a literal space deliberately stays in the
    // requested base face and can have a different advance from the fallback.
    assert_eq!(format.render("किरणक्ष"), expected.render("किरणक्ष"));
    assert_eq!(serde_json::to_vec(&format).unwrap(), before);
}
