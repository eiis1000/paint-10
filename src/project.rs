use crate::document::{
    valid_size, Document, Object, ObjectBudget, ObjectKind, MAX_OBJECTS, MAX_OBJECT_BYTES,
};
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::Path,
};

#[cfg(test)]
mod layer_tests;
#[cfg(test)]
mod version_tests;
mod wire;

const MAX_PROJECT_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct Project {
    version: u32,
    #[serde(default)]
    mono: bool,
    #[serde(default)]
    resolution: crate::metadata::Resolution,
    #[serde(with = "pixels")]
    image: RgbaImage,
    #[serde(deserialize_with = "deserialize_objects")]
    objects: Vec<Object>,
}

pub fn save(doc: &Document, path: &Path) -> Result<(), String> {
    atomic_write(path, &encode(doc)?)
}

/// Encode layers and source-preserving objects in the bounded version-4 format.
/// The loader also accepts the single-layer versions 1, 2, and 3.
pub fn encode(doc: &Document) -> Result<Vec<u8>, String> {
    doc.resolution.validate()?;
    validate_document(doc)?;
    let data = wire::LayerProject::from_document(doc)?;
    let mut out = vec![];
    out.extend(b"PAINT10\0");
    let mut encoder = BoundedWriter {
        inner: flate2::write::ZlibEncoder::new(out, flate2::Compression::default()),
        remaining: MAX_PROJECT_BYTES,
    };
    serde_json::to_writer(&mut encoder, &data).map_err(|e| e.to_string())?;
    encoder.inner.finish().map_err(|e| e.to_string())
}

/// Keep save and load limits symmetric without allocating the JSON in memory.
struct BoundedWriter<W> {
    inner: W,
    remaining: u64,
}

impl<W: Write> Write for BoundedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() as u64 > self.remaining {
            return Err(std::io::Error::other("Project exceeds the 256 MB limit."));
        }
        let written = self.inner.write(bytes)?;
        self.remaining -= written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

pub fn load(path: &Path) -> Result<Document, String> {
    decode_reader(std::fs::File::open(path).map_err(|e| e.to_string())?)
}

pub fn decode(bytes: &[u8]) -> Result<Document, String> {
    if bytes.len() as u64 > MAX_PROJECT_BYTES {
        return Err("Project exceeds the 256 MB limit.".into());
    }
    decode_reader(bytes)
}

fn decode_reader(mut file: impl Read) -> Result<Document, String> {
    let mut magic = [0; 8];
    file.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if &magic != b"PAINT10\0" {
        return Err("This is not a Paint 10 project.".into());
    }
    let mut bytes = vec![];
    flate2::read::ZlibDecoder::new(file)
        .take(MAX_PROJECT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_PROJECT_BYTES {
        return Err("Project exceeds the 256 MB limit.".into());
    }
    #[derive(Deserialize)]
    struct Version {
        version: u32,
    }
    let version: Version =
        serde_json::from_slice(&bytes).map_err(|e| format!("Invalid project: {e}"))?;
    if version.version == 4 {
        return serde_json::from_slice::<wire::LayerProject>(&bytes)
            .map_err(|e| format!("Invalid project: {e}"))?
            .into_document();
    }
    let data = match version.version {
        1 => serde_json::from_slice::<Project>(&bytes)
            .map_err(|e| format!("Invalid project: {e}"))?,
        2 | 3 => serde_json::from_slice::<wire::Project>(&bytes)
            .map_err(|e| format!("Invalid project: {e}"))?
            .into_project()?,
        _ => return Err("Unsupported Paint 10 project version.".into()),
    };
    data.resolution.validate()?;
    let mut doc = Document::from_image(data.image);
    doc.objects = data.objects;
    doc.mono = data.mono;
    doc.resolution = data.resolution;
    Ok(doc)
}

fn validate_document(doc: &Document) -> Result<(), String> {
    crate::document::validate_layers(doc.layers(), doc.active_layer_index())?;
    for object in doc.layers().iter().flat_map(|layer| &layer.objects) {
        validate_object(object)?;
    }
    Ok(())
}

fn validate_object(object: &Object) -> Result<usize, String> {
    object.image_edits.validate(object.source_dimensions())?;
    if let Some(clip) = &object.source_clip {
        clip.validate()?;
    }
    if !object.angle.is_finite()
        || !object.scale.is_finite()
        || object.scale <= 0.
        || object.scale > 16.
        || !object.transform.valid()
    {
        return Err("Invalid project object transform.".into());
    }
    if [object.pos.0, object.pos.1]
        .iter()
        .any(|&p| !(i32::MIN + 32768..=i32::MAX - 32768).contains(&p))
    {
        return Err("Object position is outside the supported range.".into());
    }
    let (width, height, bytes) = match &object.kind {
        ObjectKind::Raster(image) | ObjectKind::Image(image) => {
            (image.width(), image.height(), image.as_raw().len())
        }
        ObjectKind::Text { text, format } => {
            format.validate_for_text(text)?;
            let (width, height) = format.dimensions(text);
            (width, height, text.len() + format.memory_bytes())
        }
    };
    let width = (width as f64 * object.scale as f64).round().max(1.) as u32;
    let height = (height as f64 * object.scale as f64).round().max(1.) as u32;
    if !valid_size(width, height) || object.rendered_dimensions().is_none() {
        return Err("An object transform exceeds the canvas allocation limit.".into());
    }
    Ok(bytes)
}

fn validate_objects(objects: &[Object]) -> Result<(), String> {
    let mut budget = ObjectBudget::default();
    for object in objects {
        budget.add(object)?;
        validate_object(object)?;
    }
    Ok(())
}

fn deserialize_objects<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<Object>, D::Error> {
    struct Objects;
    impl<'de> serde::de::Visitor<'de> for Objects {
        type Value = Vec<Object>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a bounded list of Paint 10 objects")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut sequence: A,
        ) -> Result<Self::Value, A::Error> {
            let mut objects = vec![];
            let mut budget = ObjectBudget::default();
            while let Some(object) = sequence.next_element::<Object>()? {
                budget.add(&object).map_err(serde::de::Error::custom)?;
                validate_object(&object).map_err(serde::de::Error::custom)?;
                objects.push(object);
            }
            Ok(objects)
        }
    }
    deserializer.deserialize_seq(Objects)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let directory = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(directory).map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn atomic_write(_path: &Path, _bytes: &[u8]) -> Result<(), String> {
    Err("Browser files must be saved using Download.".into())
}

pub mod pixels {
    use super::*;
    pub fn serialize<S: serde::Serializer>(img: &RgbaImage, s: S) -> Result<S::Ok, S::Error> {
        let mut bytes = std::io::Cursor::new(vec![]);
        img.write_to(&mut bytes, image::ImageFormat::Png)
            .map_err(serde::ser::Error::custom)?;
        s.serialize_bytes(bytes.get_ref())
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<RgbaImage, D::Error> {
        let bytes = Vec::<u8>::deserialize(d)?;
        let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(serde::de::Error::custom)?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(96 * 1024 * 1024);
        limits.max_image_width = Some(16384);
        limits.max_image_height = Some(16384);
        reader.limits(limits);
        let img = reader
            .decode()
            .map_err(serde::de::Error::custom)?
            .to_rgba8();
        if !valid_size(img.width(), img.height()) {
            return Err(serde::de::Error::custom(
                "Image dimensions exceed the canvas limit",
            ));
        }
        Ok(img)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_project_bytes_preserve_editable_objects_and_metadata() {
        let mut doc =
            Document::from_image(RgbaImage::from_pixel(40, 30, image::Rgba([12, 34, 56, 0])));
        doc.resolution = crate::metadata::Resolution { x: 300.0, y: 150.0 };
        doc.begin();
        doc.add_object(Object::new(
            ObjectKind::Text {
                text: "Still editable".into(),
                format: crate::text::TextFormat {
                    width: 140,
                    ..Default::default()
                },
            },
            (3, 4),
        ));
        doc.commit();
        let reopened = decode(&encode(&doc).unwrap()).unwrap();
        assert!(reopened.objects == doc.objects);
        assert_eq!(reopened.composite(), doc.composite());
        assert_eq!(reopened.resolution, doc.resolution);
        assert!(!reopened.dirty());
        assert!(decode(b"wrong project header").is_err());
    }

    #[test]
    fn project_bytes_preserve_font_variants_and_missing_glyph_fallback() {
        let format = crate::text::TextFormat {
            font: epaint_default_fonts::HACK_REGULAR.into(),
            font_faces: vec![crate::text::EmbeddedFont {
                family: "Noto Emoji".into(),
                data: epaint_default_fonts::NOTO_EMOJI_REGULAR.into(),
                index: 0,
                bold: false,
                italic: false,
            }],
            ..Default::default()
        };
        let mut document = Document::new(120, 80);
        document.add_object(Object::new(
            ObjectKind::Text {
                text: "A😀B".into(),
                format,
            },
            (4, 5),
        ));
        let reopened = decode(&encode(&document).unwrap()).unwrap();
        assert!(reopened.objects == document.objects);
        assert_eq!(reopened.composite(), document.composite());
        let ObjectKind::Text { text, format } = &reopened.objects.last().unwrap().kind else {
            panic!("text remains editable");
        };
        assert_eq!(text, "A😀B");
        assert_eq!(
            format.font_faces[0].data.as_ref(),
            epaint_default_fonts::NOTO_EMOJI_REGULAR
        );
    }

    #[test]
    fn transparent_project_keeps_base_pixels_and_editable_objects() {
        let mut image = RgbaImage::new(180, 100);
        image.put_pixel(5, 5, image::Rgba([30, 80, 200, 127]));
        let mut document = Document::from_image(image);
        document.add_object(Object::new(
            ObjectKind::Text {
                text: "Caption".into(),
                format: crate::text::TextFormat {
                    width: 120,
                    outline_width: 2,
                    ..Default::default()
                },
            },
            (15, 15),
        ));
        document.add_object(Object::new(
            ObjectKind::Image(RgbaImage::from_pixel(
                10,
                10,
                image::Rgba([200, 80, 30, 128]),
            )),
            (140, 70),
        ));
        let expected = document.composite();
        assert_eq!(expected.get_pixel(179, 99)[3], 0);
        assert_eq!(expected.get_pixel(5, 5).0, [30, 80, 200, 127]);
        assert_eq!(expected.get_pixel(140, 70).0, [200, 80, 30, 128]);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("transparent.p10");
        save(&document, &path).unwrap();
        let loaded = load(&path).unwrap();
        assert!(loaded.objects == document.objects);
        assert_eq!(loaded.composite(), expected);
    }

    #[test]
    fn outlined_aligned_rich_caption_survives_transforms_and_project_roundtrip() {
        let mut format = crate::text::TextFormat {
            alignment: crate::text::TextAlignment::Center,
            outline_width: 4,
            outline_color: [0, 0, 0, 255],
            color: [255, 255, 255, 255],
            size: 48.0,
            bold: true,
            width: 300,
            ..Default::default()
        };
        format
            .modify_style(0..4, |style| style.color = [255, 80, 80, 255])
            .unwrap();
        let mut object = Object::new(
            ObjectKind::Text {
                text: "MEME CAPTION".into(),
                format,
            },
            (20, 30),
        );
        object.rotate_to(17.0).unwrap();
        object.resize_rendered(350, 130).unwrap();
        let mut document = Document::new(500, 250);
        let index = document.add_object(object);
        let original = document.composite();
        document.begin();
        let ObjectKind::Text { format, .. } = &mut document.objects[index].kind else {
            panic!();
        };
        format.alignment = crate::text::TextAlignment::Right;
        document.commit();
        document.undo();
        assert_eq!(document.composite(), original);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("caption.p10");
        save(&document, &path).unwrap();
        let loaded = load(&path).unwrap();
        let before = document.objects[index].transform;
        let after = loaded.objects[index].transform;
        assert_eq!(
            [after.xx, after.xy, after.yx, after.yy],
            [before.xx, before.xy, before.yx, before.yy]
        );
        assert!(loaded.objects == document.objects);
        assert_eq!(loaded.composite(), original);
    }

    #[test]
    fn transformed_text_roundtrips_and_old_projects_default_to_identity() {
        let mut object = Object::new(
            ObjectKind::Text {
                text: "Hello world".into(),
                format: crate::text::TextFormat::default(),
            },
            (-20, 5),
        );
        object.rotate_to(90.0).unwrap();
        object.resize_rendered(120, 420).unwrap();
        assert_eq!(object.render().dimensions(), (120, 420));
        object.rotate_to(37.0).unwrap();
        let mut document = Document::new(600, 600);
        document.add_object(object);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("transformed-text.p10");
        save(&document, &path).unwrap();
        let loaded = load(&path).unwrap();
        assert!(loaded.objects == document.objects);
        assert_eq!(loaded.composite(), document.composite());

        let original = Object::new(ObjectKind::Image(RgbaImage::new(3, 2)), (0, 0));
        let mut json = serde_json::to_value(&original).unwrap();
        json.as_object_mut().unwrap().remove("transform");
        let restored: Object = serde_json::from_value(json).unwrap();
        assert!(restored == original);
    }

    #[test]
    fn invalid_affine_transforms_are_rejected_without_rendering() {
        let mut object = Object::new(ObjectKind::Image(RgbaImage::new(3, 2)), (0, 0));
        object.transform.xx = 0.0;
        assert!(validate_object(&object).is_err());
        object.transform.xx = f64::NAN;
        assert!(validate_object(&object).is_err());
        object.transform.xx = 1_000_000.0;
        assert!(validate_object(&object).is_err());
        object.transform.xx = 1.0;
        assert!(validate_object(&object).is_ok());
    }

    #[test]
    fn invalid_saves_preserve_the_existing_project() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("drawing.p10");
        let mut document = Document::new(2, 2);
        save(&document, &path).unwrap();
        let saved = std::fs::read(&path).unwrap();
        document.objects =
            vec![Object::new(ObjectKind::Image(RgbaImage::new(1, 1)), (0, 0)); MAX_OBJECTS + 1];
        assert!(save(&document, &path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), saved);
        document.objects.truncate(1);
        document.objects[0].scale = f32::NAN;
        assert!(save(&document, &path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), saved);
        assert!(load(&path).is_ok());
    }

    #[test]
    fn serialization_stops_at_the_loaders_size_limit() {
        let mut writer = BoundedWriter {
            inner: Vec::new(),
            remaining: 3,
        };
        writer.write_all(b"abc").unwrap();
        assert!(writer.write_all(b"d").is_err());
        assert_eq!(writer.inner, b"abc");
    }
    use crate::document::{ObjectKind, BLACK};
    #[test]
    fn editable_objects_roundtrip() {
        let mut doc = Document::new(64, 64);
        doc.mono = true;
        doc.resolution = crate::metadata::Resolution { x: 300.0, y: 150.0 };
        let mut format = crate::text::TextFormat {
            size: 18.0,
            color: BLACK,
            bold: true,
            ..Default::default()
        };
        format
            .modify_style(1..4, |style| {
                style.font = epaint_default_fonts::HACK_REGULAR.into();
                style.font_name = "Monospace".into();
                style.color = [255, 0, 0, 255];
                style.italic = true;
                style.size = 26.0;
            })
            .unwrap();
        doc.add_object(Object {
            kind: ObjectKind::Text {
                text: "Hello".into(),
                format,
            },
            pos: (4, 5),
            angle: 25.,
            scale: 1.,
            color_key: Some(crate::document::WHITE),
            transform: Default::default(),
            image_edits: Default::default(),
            source_clip: None,
        });
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("test.p10");
        save(&doc, &path).unwrap();
        let reopened = load(&path).unwrap();
        assert_eq!(doc.composite(), reopened.composite());
        assert!(doc.objects == reopened.objects);
        assert_eq!(doc.resolution, reopened.resolution);
        assert!(matches!(
            reopened.objects.last().unwrap().kind,
            ObjectKind::Text { .. }
        ));
        assert!(reopened.mono);
    }
    #[test]
    fn corrupt_project_fails_closed() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("bad.p10");
        std::fs::write(&path, b"not a project").unwrap();
        assert!(load(&path).is_err());
    }

    #[test]
    fn invalid_object_data_fails_closed_before_rendering() {
        fn write_malformed_fixture(doc: &Document, path: &Path) {
            let fixture = Project {
                version: 1,
                mono: doc.mono,
                resolution: doc.resolution,
                image: doc.image.clone(),
                objects: doc.objects.clone(),
            };
            let mut encoder = flate2::write::ZlibEncoder::new(
                b"PAINT10\0".to_vec(),
                flate2::Compression::default(),
            );
            serde_json::to_writer(&mut encoder, &fixture).unwrap();
            std::fs::write(path, encoder.finish().unwrap()).unwrap();
        }
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("invalid.p10");
        let bad_formats = [
            crate::text::TextFormat {
                size: 0.,
                ..Default::default()
            },
            crate::text::TextFormat {
                font: vec![1, 2, 3].into(),
                ..Default::default()
            },
            crate::text::TextFormat {
                width: u32::MAX,
                ..Default::default()
            },
        ];
        for format in bad_formats {
            let mut doc = Document::new(8, 8);
            doc.objects.push(Object {
                kind: ObjectKind::Text {
                    text: "bad".into(),
                    format,
                },
                pos: (0, 0),
                scale: 1.,
                angle: 0.,
                color_key: None,
                transform: Default::default(),
                image_edits: Default::default(),
                source_clip: None,
            });
            assert!(save(&doc, &path).is_err());
            write_malformed_fixture(&doc, &path);
            assert!(load(&path).is_err());
        }
        let mut doc = Document::new(8, 8);
        doc.objects.push(Object {
            kind: ObjectKind::Image(RgbaImage::new(512, 512)),
            pos: (0, 0),
            scale: 16.,
            angle: 0.,
            color_key: None,
            transform: Default::default(),
            image_edits: Default::default(),
            source_clip: None,
        });
        assert!(save(&doc, &path).is_err());
        write_malformed_fixture(&doc, &path);
        assert!(load(&path).is_err());
    }
}
