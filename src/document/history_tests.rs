use super::*;
use crate::text::{EmbeddedFont, FontBytes, TextFormat};

#[test]
fn caption_history_shares_fonts_and_retains_each_edit_and_redo() {
    let mut bytes = epaint_default_fonts::HACK_REGULAR.to_vec();
    bytes.resize(8 * 1024 * 1024, 0);
    let font = FontBytes::from(bytes);
    let mut format = TextFormat {
        font_name: "Original caption family".into(),
        font: font.clone(),
        font_faces: vec![EmbeddedFont {
            family: "Original caption face".into(),
            data: font.clone(),
            index: 0,
            bold: true,
            italic: false,
        }],
        ..Default::default()
    };
    format
        .modify_style(0..4, |style| style.underline = true)
        .unwrap();
    format.validate_for_text("Caption").unwrap();
    let mut document = Document::new(32, 32);
    for caption in 0..16 {
        document.objects.push(Object::new(
            ObjectKind::Text {
                text: "Caption".into(),
                format: format.clone(),
            },
            (caption, 0),
        ));
    }
    for step in 1..=12 {
        document.begin();
        document.objects[0].pos.1 = step;
        document.commit();
    }
    assert_eq!(document.undo.len(), 12);
    let mut fonts = FontMemory::default();
    let history_bytes: usize = document
        .undo
        .iter()
        .map(|snapshot| snapshot.bytes_with_fonts(&mut fonts))
        .sum();
    assert!(history_bytes < 9 * 1024 * 1024);
    for step in (0..12).rev() {
        document.undo();
        assert_eq!(document.objects[0].pos.1, step);
    }
    assert!(!document.can_undo());
    assert!(!document.dirty());
    for step in 1..=12 {
        document.redo();
        assert_eq!(document.objects[0].pos.1, step);
        let ObjectKind::Text { format, .. } = &document.objects[0].kind else {
            panic!()
        };
        assert!(format
            .font_assets()
            .all(|asset| font.shares_storage_with(asset)));
        assert_eq!(format.font_name, "Original caption family");
        assert_eq!(format.font_faces[0].family, "Original caption face");
    }
    document.begin();
    document.commit();
    assert_eq!(
        document.undo.len(),
        12,
        "A no-op must not consume undo history"
    );
}

#[test]
fn shared_font_accounting_still_trims_independent_raster_snapshots() {
    let mut document = Document::new(2048, 2048);
    for step in 1..=10 {
        document.edit(|image| image.put_pixel(0, 0, Rgba([step, 0, 0, 255])));
    }
    assert_eq!(document.undo.len(), 8);
    for step in (2..10).rev() {
        document.undo();
        assert_eq!(document.image.get_pixel(0, 0).0[0], step);
    }
    assert!(!document.can_undo());
}
