use super::*;
use crate::document::{ImageEdits, Region, WHITE};
use crate::text::{FontBytes, TextFormat};

fn unpack(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_reader(flate2::read::ZlibDecoder::new(&bytes[8..])).unwrap()
}

fn pack(json: &serde_json::Value) -> Vec<u8> {
    let mut encoder =
        flate2::write::ZlibEncoder::new(b"PAINT10\0".to_vec(), flate2::Compression::default());
    serde_json::to_writer(&mut encoder, json).unwrap();
    encoder.finish().unwrap()
}

fn layered_document() -> Document {
    let mut document = Document::new(40, 30);
    document.resolution = crate::metadata::Resolution { x: 300.0, y: 150.0 };
    document.active_layer_mut().name = "Paper".into();
    document.active_layer_mut().locked = true;
    document.add_layer().unwrap();
    document.active_layer_mut().name = "Photo".into();
    let source = RgbaImage::from_fn(32, 24, |x, y| {
        image::Rgba([x as u8 * 5, y as u8 * 5, 160, 200])
    });
    let mut object = Object::new(ObjectKind::Image(source), (4, 3));
    object
        .set_image_edits(ImageEdits {
            crop: Some(Region {
                x: 2,
                y: 3,
                w: 20,
                h: 18,
            }),
            hue: 35.0,
            ..Default::default()
        })
        .unwrap();
    object.rotate_to(17.0).unwrap();
    document.add_object(object);
    document.active_layer_mut().opacity = 180;
    let font = FontBytes::from(epaint_default_fonts::HACK_REGULAR);
    for name in ["Captions", "Alternative captions"] {
        document.add_layer().unwrap();
        document.active_layer_mut().name = name.into();
        document.add_object(Object::new(
            ObjectKind::Text {
                text: "Editable caption".into(),
                format: TextFormat {
                    font: font.clone(),
                    width: 100,
                    ..Default::default()
                },
            },
            (1, 2),
        ));
    }
    document.active_layer_mut().visible = false;
    document.set_active_layer(2).unwrap();
    document
}

#[test]
fn version_four_roundtrips_layer_metadata_original_images_and_shared_fonts() {
    let document = layered_document();
    let expected = document.composite();
    let bytes = encode(&document).unwrap();
    let json = unpack(&bytes);
    assert_eq!(json["version"], 4);
    assert_eq!(json["layers"].as_array().unwrap().len(), 4);
    assert_eq!(json["fonts"].as_array().unwrap().len(), 1);
    assert!(json.get("image").is_none());
    assert!(json.get("objects").is_none());
    let reopened = decode(&bytes).unwrap();
    assert!(reopened.layers() == document.layers());
    assert_eq!(reopened.active_layer_index(), 2);
    assert_eq!(reopened.resolution, document.resolution);
    assert_eq!(reopened.composite(), expected);
    assert!(!reopened.dirty());
    assert!(!reopened.can_undo());
    let fonts: Vec<_> = reopened
        .layers()
        .iter()
        .flat_map(|layer| &layer.objects)
        .filter_map(|object| match &object.kind {
            ObjectKind::Text { format, .. } => Some(&format.font),
            _ => None,
        })
        .collect();
    assert!(fonts[0].shares_storage_with(fonts[1]));
    let ObjectKind::Image(source) = &reopened.layers()[1].objects[0].kind else {
        panic!()
    };
    assert_eq!(source.dimensions(), (32, 24));
    assert_eq!(reopened.layers()[1].objects[0].image_edits.hue, 35.0);
}

#[test]
fn earlier_projects_load_as_one_visible_unlocked_paper_layer() {
    let source = layered_document();
    let mut single = Document::from_image(source.layers()[1].image.clone());
    single.objects = source.layers()[1].objects.clone();
    let mut legacy = serde_json::to_value(wire::Project::from_document(&single).unwrap()).unwrap();
    for version in [2, 3] {
        legacy["version"] = version.into();
        let document = decode(&pack(&legacy)).unwrap();
        assert_eq!(document.layer_count(), 1);
        assert_eq!(document.active_layer_index(), 0);
        assert_eq!(document.active_layer().name, "Background");
        assert!(document.active_layer().visible);
        assert!(document.active_layer().is_background);
        assert!(!document.active_layer().locked);
        assert_eq!(document.active_layer().opacity, 255);
        assert_eq!(document.composite(), single.composite());
        assert!(document.objects == single.objects);
    }
    let legacy = serde_json::to_value(Project {
        version: 1,
        mono: false,
        resolution: Default::default(),
        image: single.image.clone(),
        objects: single.objects.clone(),
    })
    .unwrap();
    let document = decode(&pack(&legacy)).unwrap();
    assert_eq!(document.layer_count(), 1);
    assert!(document.active_layer().is_background);
    assert_eq!(document.composite(), single.composite());
}

#[test]
fn malformed_layer_metadata_and_stack_structure_fail_closed() {
    let original = unpack(&encode(&layered_document()).unwrap());
    for (path, value) in [
        ("/active_layer", serde_json::json!(4)),
        ("/active_layer", serde_json::json!(-1)),
        ("/layers/0/name", serde_json::json!("   ")),
        ("/layers/0/name", serde_json::json!("x".repeat(257))),
        ("/layers/0/opacity", serde_json::json!(256)),
        ("/layers/0/opacity", serde_json::json!(-1)),
        ("/layers/0/opacity", serde_json::json!(0.5)),
        ("/layers/1/is_background", serde_json::json!(true)),
        ("/layers/1/objects/0/scale", serde_json::json!(0)),
        (
            "/layers/2/objects/0/kind/Text/format/font",
            serde_json::json!(99),
        ),
    ] {
        let mut json = original.clone();
        *json.pointer_mut(path).unwrap() = value;
        assert!(decode(&pack(&json)).is_err(), "accepted invalid {path}");
    }
    let mut empty = original.clone();
    empty["layers"] = serde_json::json!([]);
    assert!(decode(&pack(&empty)).is_err());
    let mut oversized = original.clone();
    let mut layer = original["layers"][1].clone();
    layer["objects"] = serde_json::json!([]);
    oversized["layers"] = serde_json::Value::Array(vec![layer; crate::document::MAX_LAYERS + 1]);
    assert!(decode(&pack(&oversized))
        .err()
        .unwrap()
        .contains("64 layer"));
    let mut mismatch = original;
    let small = unpack(&encode(&Document::new(2, 2)).unwrap());
    mismatch["layers"][1]["image"] = small["layers"][0]["image"].clone();
    assert!(decode(&pack(&mismatch))
        .err()
        .unwrap()
        .contains("same canvas"));
}

#[test]
fn object_limits_apply_across_layers_on_save_and_load() {
    let mut document = Document::from_image(RgbaImage::new(1, 1));
    let object = Object::new(
        ObjectKind::Image(RgbaImage::from_pixel(1, 1, image::Rgba(WHITE))),
        (0, 0),
    );
    document.objects = vec![object.clone(); 500];
    document.add_layer().unwrap();
    document.objects = vec![object.clone(); 500];
    let mut json = unpack(&encode(&document).unwrap());
    document.objects.push(object);
    assert!(encode(&document).unwrap_err().contains("1,000 object"));
    let objects = json["layers"][1]["objects"].as_array_mut().unwrap();
    objects.push(objects[0].clone());
    assert!(decode(&pack(&json)).err().unwrap().contains("1,000 object"));
}
