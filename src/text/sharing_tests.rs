use super::*;

fn caption_format() -> TextFormat {
    let font = FontBytes::from(epaint_default_fonts::HACK_REGULAR);
    TextFormat {
        font_name: "Caption family".into(),
        font: font.clone(),
        font_faces: vec![EmbeddedFont {
            family: "Retained family alias".into(),
            data: font,
            index: 0,
            bold: true,
            italic: false,
        }],
        ..Default::default()
    }
}

#[test]
fn rich_format_clones_keep_font_identity_and_rendered_pixels() {
    let mut original = caption_format();
    original
        .modify_style(1..4, |style| {
            style.font_name = "Selected family alias".into();
            style.underline = true;
            style.color = [210, 30, 40, 200];
        })
        .unwrap();
    let expected_pixels = original.render("Caption");
    let cloned = original.clone();
    let decoded: TextFormat =
        serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
    for format in [cloned, decoded] {
        assert!(format == original);
        assert_eq!(format.render("Caption"), expected_pixels);
        assert!(format
            .font_assets()
            .all(|asset| original.font.shares_storage_with(asset)));
        assert_eq!(format.font_faces[0].family, "Retained family alias");
        assert!(format.font_faces[0].bold);
        assert_eq!(format.spans[0].style.font_name, "Selected family alias");
        assert!(format
            .default_style_ref()
            .to_owned()
            .font
            .shares_storage_with(&original.font));
    }
}

#[test]
fn memory_accounting_counts_payload_once_and_metadata_for_each_caption() {
    let mut format = caption_format();
    format
        .modify_style(1..4, |style| style.bold = true)
        .unwrap();
    let payload_bytes = format.font.len();
    let metadata_bytes = format.memory_bytes() - payload_bytes;
    assert!(metadata_bytes > 0 && metadata_bytes < 1024);
    let mut fonts = FontMemory::default();
    let total: usize = (0..16)
        .map(|_| format.memory_bytes_with_fonts(&mut fonts))
        .sum();
    assert_eq!(total, payload_bytes + 16 * metadata_bytes);

    let other_font = FontBytes::from(epaint_default_fonts::UBUNTU_LIGHT);
    // Family names and collection face indices do not identify byte payloads.
    format.font_faces.push(EmbeddedFont {
        family: format.font_faces[0].family.clone(),
        data: other_font.clone(),
        index: 0,
        bold: false,
        italic: true,
    });
    format.font_faces[0].index = 1;
    let mut fonts = FontMemory::default();
    let unique: usize = format.font_assets().map(|font| fonts.include(font)).sum();
    assert_eq!(unique, payload_bytes + other_font.len());
}

#[test]
fn repeated_rich_styles_do_not_consume_the_unique_font_budget() {
    let mut bytes = epaint_default_fonts::HACK_REGULAR.to_vec();
    bytes.resize(8 * 1024 * 1024, 0);
    let mut format = TextFormat {
        font: bytes.into(),
        ..Default::default()
    };
    for character in (0..32).step_by(2) {
        format
            .modify_style(character..character + 1, |style| style.bold = true)
            .unwrap();
    }
    format.validate_for_text(&"A".repeat(32)).unwrap();
    assert_eq!(format.spans.len(), 16);
    assert!(format.memory_bytes() < 9 * 1024 * 1024);
    assert!(format
        .font_assets()
        .all(|font| format.font.shares_storage_with(font)));
}
