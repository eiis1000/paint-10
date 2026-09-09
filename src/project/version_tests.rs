use super::*;
use crate::text::{EmbeddedFont, FontBytes, TextAlignment, TextFormat};

fn packed(value: &impl Serialize) -> Vec<u8> {
    let mut encoder =
        flate2::write::ZlibEncoder::new(b"PAINT10\0".to_vec(), flate2::Compression::default());
    serde_json::to_writer(&mut encoder, value).unwrap();
    encoder.finish().unwrap()
}

fn legacy(document: &Document) -> Vec<u8> {
    packed(&Project {
        version: 1,
        mono: document.mono,
        resolution: document.resolution,
        image: document.image.clone(),
        objects: document.objects.clone(),
    })
}

fn unpacked(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_reader(flate2::read::ZlibDecoder::new(&bytes[8..])).unwrap()
}

fn collection() -> FontBytes {
    let fonts: [&[u8]; 2] = [
        epaint_default_fonts::HACK_REGULAR,
        crate::text::DEFAULT_FONT,
    ];
    let mut collection = b"ttcf\0\x01\0\0\0\0\0\x02".to_vec();
    collection.resize(20, 0);
    for (face_index, font) in fonts.into_iter().enumerate() {
        while !collection.len().is_multiple_of(4) {
            collection.push(0);
        }
        let start = collection.len();
        collection[12 + face_index * 4..16 + face_index * 4]
            .copy_from_slice(&(start as u32).to_be_bytes());
        collection.extend_from_slice(font);
        let tables = u16::from_be_bytes(font[4..6].try_into().unwrap());
        for table in 0..tables as usize {
            let offset = start + 12 + table * 16 + 8;
            let original = u32::from_be_bytes(collection[offset..offset + 4].try_into().unwrap());
            collection[offset..offset + 4]
                .copy_from_slice(&(original + start as u32).to_be_bytes());
        }
    }
    collection.into()
}

fn caption_document(font: FontBytes, count: usize) -> Document {
    let mut document = Document::from_image(RgbaImage::new(480, 180));
    for index in 0..count {
        let mut format = TextFormat {
            font_name: "Shared caption font".into(),
            font: font.clone(),
            font_faces: vec![EmbeddedFont {
                family: "Shared face metadata".into(),
                data: font.clone(),
                index: 0,
                bold: true,
                italic: false,
            }],
            width: 240,
            ..Default::default()
        };
        format
            .modify_style(1..4, |style| style.underline = true)
            .unwrap();
        document.objects.push(Object::new(
            ObjectKind::Text {
                text: "Caption".into(),
                format,
            },
            (index as i32 * 2, index as i32 * 3),
        ));
    }
    document
}

#[test]
fn shared_font_table_stores_each_binary_once_and_reuses_decoded_allocations() {
    let document = caption_document(FontBytes::from(epaint_default_fonts::UBUNTU_LIGHT), 12);
    let bytes = encode(&document).unwrap();
    let json = unpacked(&bytes);
    assert_eq!(json["version"], 3);
    assert_eq!(json["fonts"].as_array().unwrap().len(), 1);
    assert!(bytes.len() * 8 < legacy(&document).len());

    let reopened = decode(&bytes).unwrap();
    assert!(reopened.objects == document.objects);
    let ObjectKind::Text { format: first, .. } = &reopened.objects[0].kind else {
        panic!("Caption remains editable");
    };
    for object in &reopened.objects {
        let ObjectKind::Text { format, .. } = &object.kind else {
            panic!()
        };
        for font in format.font_assets() {
            assert!(first.font.shares_storage_with(font));
        }
    }
}

#[test]
fn all_project_versions_preserve_collection_faces_rich_format_transforms_and_pixels() {
    let mut document = caption_document(collection(), 2);
    document.resolution = crate::metadata::Resolution { x: 300.0, y: 150.0 };
    document
        .image
        .put_pixel(7, 8, image::Rgba([20, 40, 80, 127]));
    for object in &mut document.objects {
        let ObjectKind::Text { text, format } = &mut object.kind else {
            panic!()
        };
        *text = "AسلامB".into();
        format.font_index = 1;
        format.font_faces[0].index = 1;
        format.font_faces[0].italic = true;
        format.spans[0].style.font_index = 0;
        format.spans[0].style.bold = true;
        format.spans[0].style.strikeout = true;
        format.spans[0].style.color = [80, 30, 220, 190];
        format.background = Some([240, 230, 210, 240]);
        format.alignment = TextAlignment::Right;
        format.minimum_height = 96;
        format.outline_width = 2;
        format.outline_color = [1, 2, 3, 255];
        object.rotate_to(17.0).unwrap();
        object.resize_rendered(260, 140).unwrap();
    }
    let expected = document.composite();
    let current = encode(&document).unwrap();
    let mut v2 = unpacked(&current);
    v2["version"] = serde_json::json!(2);
    for object in v2["objects"].as_array_mut().unwrap() {
        object.as_object_mut().unwrap().remove("image_edits");
        object.as_object_mut().unwrap().remove("source_clip");
    }
    for bytes in [legacy(&document), packed(&v2), current] {
        let reopened = decode(&bytes).unwrap();
        assert!(reopened.objects == document.objects);
        assert_eq!(reopened.image, document.image);
        assert_eq!(reopened.resolution, document.resolution);
        assert_eq!(reopened.composite(), expected);
        assert!(!reopened.dirty());
        let formats: Vec<_> = reopened
            .objects
            .iter()
            .filter_map(|object| {
                if let ObjectKind::Text { format, .. } = &object.kind {
                    Some(format)
                } else {
                    None
                }
            })
            .collect();
        assert!(formats[0].font.shares_storage_with(&formats[1].font));
        assert!(formats[0]
            .font
            .shares_storage_with(&formats[0].font_faces[0].data));
    }
}

#[test]
fn empty_default_fonts_need_no_table_entry_and_keep_identical_pixels() {
    let mut document = Document::new(100, 80);
    document.objects.push(Object::new(
        ObjectKind::Text {
            text: "Default".into(),
            format: TextFormat {
                font: FontBytes::default(),
                ..Default::default()
            },
        },
        (2, 3),
    ));
    let bytes = encode(&document).unwrap();
    let json = unpacked(&bytes);
    assert_eq!(json["fonts"], serde_json::json!([]));
    assert!(json["objects"][0]["kind"]["Text"]["format"]["font"].is_null());
    let reopened = decode(&bytes).unwrap();
    assert!(reopened.objects == document.objects);
    assert_eq!(reopened.composite(), document.composite());
}

#[test]
fn both_project_versions_preserve_legacy_and_em_text_size_modes() {
    for mode in [
        crate::text::TextSizeMode::FontHeight,
        crate::text::TextSizeMode::Em,
    ] {
        let mut document = caption_document(FontBytes::from(epaint_default_fonts::UBUNTU_LIGHT), 1);
        let ObjectKind::Text { format, .. } = &mut document.objects[0].kind else {
            panic!("caption");
        };
        format.size_mode = mode;
        let expected = document.composite();
        for bytes in [legacy(&document), encode(&document).unwrap()] {
            let reopened = decode(&bytes).unwrap();
            assert!(reopened.objects == document.objects);
            assert_eq!(reopened.composite(), expected);
            if mode == crate::text::TextSizeMode::FontHeight {
                let mut old_json = unpacked(&bytes);
                old_json["objects"][0]["kind"]["Text"]["format"]
                    .as_object_mut()
                    .unwrap()
                    .remove("size_mode");
                let old = decode(&packed(&old_json)).unwrap();
                assert!(old.objects == document.objects);
                assert_eq!(old.composite(), expected);
            }
        }
    }
}

#[test]
fn malformed_font_references_and_table_payloads_fail_closed() {
    let document = caption_document(FontBytes::from(epaint_default_fonts::UBUNTU_LIGHT), 1);
    let original = unpacked(&encode(&document).unwrap());
    for path in [
        "/objects/0/kind/Text/format/font",
        "/objects/0/kind/Text/format/spans/0/style/font",
        "/objects/0/kind/Text/format/font_faces/0/data",
    ] {
        let mut bad = original.clone();
        *bad.pointer_mut(path).unwrap() = serde_json::json!(42);
        assert!(decode(&packed(&bad))
            .err()
            .unwrap()
            .contains("font reference"));
    }
    for bytes in [serde_json::json!([]), serde_json::json!([1, 2, 3])] {
        let mut bad = original.clone();
        bad["fonts"][0] = bytes;
        assert!(decode(&packed(&bad)).is_err());
    }
    let mut bad = original;
    bad["objects"][0]["kind"]["Text"]["format"]["font_index"] = serde_json::json!(99);
    assert!(decode(&packed(&bad)).is_err());
}

#[test]
fn v2_reference_arrays_are_bounded_during_deserialization() {
    let document = caption_document(FontBytes::from(epaint_default_fonts::UBUNTU_LIGHT), 1);
    let original = unpacked(&encode(&document).unwrap());
    for (path, count) in [
        ("/objects", MAX_OBJECTS + 1),
        (
            "/objects/0/kind/Text/format/spans",
            crate::text::MAX_SPANS + 1,
        ),
        (
            "/objects/0/kind/Text/format/font_faces",
            crate::text::MAX_FONT_FACES + 1,
        ),
    ] {
        let mut bad = original.clone();
        let array = bad.pointer_mut(path).unwrap().as_array_mut().unwrap();
        array.resize(count, array[0].clone());
        assert!(decode(&packed(&bad))
            .err()
            .unwrap()
            .contains("reference count"));
    }
    let mut bad = original;
    bad["fonts"] = serde_json::Value::Array(vec![serde_json::json!([]); 4097]);
    assert!(decode(&packed(&bad))
        .err()
        .unwrap()
        .contains("reference count"));
}
