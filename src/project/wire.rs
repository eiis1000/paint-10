//! All modern project versions share one deduplicated font table. Versions 4/5
//! store layers; version 5 adds math text. Versions 2/3 contain a single layer.

use super::*;
use crate::document::{Color, ImageEdits, LinearTransform, Point, SourceClip};
use crate::text::{
    EmbeddedFont, FontBytes, TextAlignment, TextFormat, TextSizeMode, TextSpan, TextStyle,
};
use std::collections::HashMap;
use std::ops::Range;

const MAX_FONT_ASSETS: usize = 4096;

#[derive(Serialize, Deserialize)]
pub(super) struct LayerProject {
    version: u32,
    mono: bool,
    resolution: crate::metadata::Resolution,
    active_layer: usize,
    #[serde(deserialize_with = "deserialize_fonts")]
    fonts: Vec<FontBytes>,
    #[serde(deserialize_with = "deserialize_layers")]
    layers: Vec<WireLayer>,
}

#[derive(Serialize, Deserialize)]
struct WireLayer {
    name: String,
    visible: bool,
    locked: bool,
    opacity: u8,
    is_background: bool,
    #[serde(with = "pixels")]
    image: RgbaImage,
    #[serde(deserialize_with = "deserialize_objects")]
    objects: Vec<WireObject>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Project {
    version: u32,
    #[serde(default)]
    mono: bool,
    #[serde(default)]
    resolution: crate::metadata::Resolution,
    #[serde(with = "pixels")]
    image: RgbaImage,
    #[serde(deserialize_with = "deserialize_fonts")]
    fonts: Vec<FontBytes>,
    #[serde(deserialize_with = "deserialize_objects")]
    objects: Vec<WireObject>,
}

#[derive(Serialize, Deserialize)]
struct WireObject {
    kind: WireKind,
    pos: Point,
    angle: f32,
    scale: f32,
    #[serde(default)]
    color_key: Option<Color>,
    #[serde(default)]
    transform: LinearTransform,
    #[serde(default)]
    image_edits: ImageEdits,
    #[serde(default)]
    source_clip: Option<SourceClip>,
}

#[derive(Serialize, Deserialize)]
enum WireKind {
    Raster(#[serde(with = "pixels")] RgbaImage),
    Image(#[serde(with = "pixels")] RgbaImage),
    Text { text: String, format: WireFormat },
}

#[derive(Clone, Serialize, Deserialize)]
struct WireStyle {
    font_name: String,
    font: Option<u32>,
    #[serde(default)]
    font_index: u32,
    size: f32,
    color: Color,
    bold: bool,
    italic: bool,
    underline: bool,
    strikeout: bool,
}

#[derive(Serialize, Deserialize)]
struct WireSpan {
    range: Range<usize>,
    style: WireStyle,
}

#[derive(Serialize, Deserialize)]
struct WireFace {
    family: String,
    data: u32,
    index: u32,
    bold: bool,
    italic: bool,
}

#[derive(Serialize, Deserialize)]
struct WireFormat {
    #[serde(default)]
    latex: bool,
    #[serde(flatten)]
    style: WireStyle,
    #[serde(default)]
    size_mode: TextSizeMode,
    background: Option<Color>,
    width: u32,
    minimum_height: u32,
    alignment: TextAlignment,
    outline_width: u32,
    outline_color: Color,
    #[serde(deserialize_with = "deserialize_spans")]
    spans: Vec<WireSpan>,
    #[serde(deserialize_with = "deserialize_faces")]
    font_faces: Vec<WireFace>,
}

#[derive(Default)]
struct FontTable {
    fonts: Vec<FontBytes>,
    indices: HashMap<FontBytes, u32>,
    bytes: usize,
}

impl FontTable {
    fn insert(&mut self, font: &FontBytes) -> Result<Option<u32>, String> {
        if font.is_empty() {
            return Ok(None);
        }
        if let Some(index) = self.indices.get(font) {
            return Ok(Some(*index));
        }
        if self.fonts.len() >= MAX_FONT_ASSETS
            || self.bytes.saturating_add(font.len()) > MAX_OBJECT_BYTES
        {
            return Err("Project fonts exceed the 128 MB or 4,096 font limit.".into());
        }
        let index = self.fonts.len() as u32;
        self.bytes += font.len();
        self.fonts.push(font.clone());
        self.indices.insert(font.clone(), index);
        Ok(Some(index))
    }
}

impl Project {
    #[cfg(test)]
    pub(super) fn from_document(document: &Document) -> Result<Self, String> {
        let mut fonts = FontTable::default();
        let objects = document
            .objects
            .iter()
            .map(|object| WireObject::from_object(object, &mut fonts))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            version: 3,
            mono: document.mono,
            resolution: document.resolution,
            image: document.image.clone(),
            fonts: fonts.fonts,
            objects,
        })
    }

    pub(super) fn into_project(self) -> Result<super::Project, String> {
        if !matches!(self.version, 2 | 3) {
            return Err("Unsupported Paint 10 project version.".into());
        }
        self.resolution.validate()?;
        if self.fonts.iter().any(|font| font.is_empty()) {
            return Err("A project font-table asset cannot be empty.".into());
        }
        let mut referenced = vec![false; self.fonts.len()];
        for object in &self.objects {
            if let WireKind::Text { format, .. } = &object.kind {
                format.validate_references(&self.fonts, &mut referenced)?;
            }
        }
        if referenced.contains(&false) {
            return Err("The project font table contains an unused asset.".into());
        }
        let bytes = self.fonts.iter().map(|font| font.len()).sum::<usize>()
            + self
                .objects
                .iter()
                .map(WireObject::non_font_bytes)
                .sum::<usize>();
        if bytes > MAX_OBJECT_BYTES {
            return Err("Project assets exceed the 128 MB allocation limit.".into());
        }
        let objects = self
            .objects
            .into_iter()
            .map(|object| object.into_object(&self.fonts))
            .collect::<Result<Vec<_>, _>>()?;
        validate_objects(&objects)?;
        Ok(super::Project {
            version: self.version,
            mono: self.mono,
            resolution: self.resolution,
            image: self.image,
            objects,
        })
    }
}

impl LayerProject {
    pub(super) fn from_document(document: &Document) -> Result<Self, String> {
        let mut fonts = FontTable::default();
        let layers = document
            .layers()
            .iter()
            .map(|layer| {
                Ok(WireLayer {
                    name: layer.name.clone(),
                    visible: layer.visible,
                    locked: layer.locked,
                    opacity: layer.opacity,
                    is_background: layer.is_background,
                    image: layer.image.clone(),
                    objects: layer
                        .objects
                        .iter()
                        .map(|object| WireObject::from_object(object, &mut fonts))
                        .collect::<Result<_, _>>()?,
                })
            })
            .collect::<Result<_, String>>()?;
        let has_math = document
            .layers()
            .iter()
            .flat_map(|layer| &layer.objects)
            .any(|object| matches!(&object.kind, ObjectKind::Text { format, .. } if format.latex));
        Ok(Self {
            version: if has_math { 5 } else { 4 },
            mono: document.mono,
            resolution: document.resolution,
            active_layer: document.active_layer_index(),
            fonts: fonts.fonts,
            layers,
        })
    }

    pub(super) fn into_document(self) -> Result<Document, String> {
        self.resolution.validate()?;
        if !matches!(self.version, 4 | 5) {
            return Err("Unsupported Paint 10 project version.".into());
        }
        if self.fonts.iter().any(|font| font.is_empty()) {
            return Err("A project font-table asset cannot be empty.".into());
        }
        let mut referenced = vec![false; self.fonts.len()];
        let mut bytes = self.fonts.iter().map(|font| font.len()).sum::<usize>();
        let mut count = 0usize;
        for object in self.layers.iter().flat_map(|layer| &layer.objects) {
            count += 1;
            if count > MAX_OBJECTS {
                return Err(
                    "Project objects exceed the 1,000 object limit across all layers.".into(),
                );
            }
            bytes = bytes.saturating_add(object.non_font_bytes());
            if bytes > MAX_OBJECT_BYTES {
                return Err("Project assets exceed the 128 MB allocation limit.".into());
            }
            if let WireKind::Text { format, .. } = &object.kind {
                if format.latex && self.version < 5 {
                    return Err("LaTeX text requires project version 5.".into());
                }
                format.validate_references(&self.fonts, &mut referenced)?;
            }
        }
        if referenced.contains(&false) {
            return Err("The project font table contains an unused asset.".into());
        }
        let layers = self
            .layers
            .into_iter()
            .map(|layer| {
                Ok(crate::document::Layer {
                    name: layer.name,
                    visible: layer.visible,
                    locked: layer.locked,
                    opacity: layer.opacity,
                    is_background: layer.is_background,
                    image: layer.image,
                    objects: layer
                        .objects
                        .into_iter()
                        .map(|object| object.into_object(&self.fonts))
                        .collect::<Result<_, _>>()?,
                })
            })
            .collect::<Result<_, String>>()?;
        let mut document = Document::from_layers(layers, self.active_layer)?;
        document.mono = self.mono;
        document.resolution = self.resolution;
        validate_document(&document)?;
        Ok(document)
    }
}

fn resolve_font(index: Option<u32>, fonts: &[FontBytes]) -> Result<FontBytes, String> {
    match index {
        None => Ok(FontBytes::default()),
        Some(index) => fonts
            .get(index as usize)
            .cloned()
            .ok_or_else(|| "Invalid project font reference.".into()),
    }
}

impl WireObject {
    fn from_object(object: &Object, fonts: &mut FontTable) -> Result<Self, String> {
        let kind = match &object.kind {
            ObjectKind::Raster(image) => WireKind::Raster(image.clone()),
            ObjectKind::Image(image) => WireKind::Image(image.clone()),
            ObjectKind::Text { text, format } => WireKind::Text {
                text: text.clone(),
                format: WireFormat::from_format(format, fonts)?,
            },
        };
        Ok(Self {
            kind,
            pos: object.pos,
            angle: object.angle,
            scale: object.scale,
            color_key: object.color_key,
            transform: object.transform,
            image_edits: object.image_edits,
            source_clip: object.source_clip.clone(),
        })
    }

    fn into_object(self, fonts: &[FontBytes]) -> Result<Object, String> {
        let kind = match self.kind {
            WireKind::Raster(image) => ObjectKind::Raster(image),
            WireKind::Image(image) => ObjectKind::Image(image),
            WireKind::Text { text, format } => ObjectKind::Text {
                text,
                format: format.into_format(fonts)?,
            },
        };
        Ok(Object {
            kind,
            pos: self.pos,
            angle: self.angle,
            scale: self.scale,
            color_key: self.color_key,
            transform: self.transform,
            image_edits: self.image_edits,
            source_clip: self.source_clip,
        })
    }

    fn non_font_bytes(&self) -> usize {
        self.source_clip
            .as_ref()
            .map_or(0, SourceClip::memory_bytes)
            + match &self.kind {
                WireKind::Raster(image) | WireKind::Image(image) => image.as_raw().len(),
                WireKind::Text { text, format } => {
                    text.len()
                        + format.style.font_name.len()
                        + format
                            .spans
                            .iter()
                            .map(|span| {
                                std::mem::size_of::<TextSpan>() + span.style.font_name.len()
                            })
                            .sum::<usize>()
                        + format
                            .font_faces
                            .iter()
                            .map(|face| std::mem::size_of::<EmbeddedFont>() + face.family.len())
                            .sum::<usize>()
                }
            }
    }
}

impl WireStyle {
    fn from_style(
        style: crate::text::TextStyleRef<'_>,
        fonts: &mut FontTable,
    ) -> Result<Self, String> {
        Ok(Self {
            font_name: style.font_name.to_owned(),
            font: fonts.insert(style.font)?,
            font_index: style.font_index,
            size: style.size,
            color: style.color,
            bold: style.bold,
            italic: style.italic,
            underline: style.underline,
            strikeout: style.strikeout,
        })
    }

    fn into_style(self, fonts: &[FontBytes]) -> Result<TextStyle, String> {
        Ok(TextStyle {
            font_name: self.font_name,
            font: resolve_font(self.font, fonts)?,
            font_index: self.font_index,
            size: self.size,
            color: self.color,
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            strikeout: self.strikeout,
        })
    }
}

impl WireFormat {
    fn validate_references(
        &self,
        fonts: &[FontBytes],
        referenced: &mut [bool],
    ) -> Result<(), String> {
        let references = std::iter::once(self.style.font)
            .chain(self.spans.iter().map(|span| span.style.font))
            .chain(self.font_faces.iter().map(|face| Some(face.data)));
        for index in references.flatten() {
            let used = referenced
                .get_mut(index as usize)
                .ok_or("Invalid project font reference.")?;
            *used = true;
        }
        // Validate the selected collection indices, not an assumed face zero.
        // The same binary may intentionally be used through several TTC faces.
        for style in std::iter::once(&self.style).chain(self.spans.iter().map(|span| &span.style)) {
            style.clone().into_style(fonts)?.validate()?;
        }
        for face in &self.font_faces {
            EmbeddedFont {
                family: face.family.clone(),
                data: resolve_font(Some(face.data), fonts)?,
                index: face.index,
                bold: face.bold,
                italic: face.italic,
            }
            .validate()?;
        }
        Ok(())
    }

    fn from_format(format: &TextFormat, fonts: &mut FontTable) -> Result<Self, String> {
        let spans = format
            .spans
            .iter()
            .map(|span| {
                Ok(WireSpan {
                    range: span.range.clone(),
                    style: WireStyle::from_style(span.style.as_ref(), fonts)?,
                })
            })
            .collect::<Result<_, String>>()?;
        let font_faces = format
            .font_faces
            .iter()
            .map(|face| {
                Ok(WireFace {
                    family: face.family.clone(),
                    data: fonts
                        .insert(&face.data)?
                        .ok_or("An embedded project font cannot be empty.")?,
                    index: face.index,
                    bold: face.bold,
                    italic: face.italic,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(Self {
            style: WireStyle::from_style(format.default_style_ref(), fonts)?,
            latex: format.latex,
            size_mode: format.size_mode,
            background: format.background,
            width: format.width,
            minimum_height: format.minimum_height,
            alignment: format.alignment,
            outline_width: format.outline_width,
            outline_color: format.outline_color,
            spans,
            font_faces,
        })
    }

    fn into_format(self, fonts: &[FontBytes]) -> Result<TextFormat, String> {
        let style = self.style.into_style(fonts)?;
        Ok(TextFormat {
            latex: self.latex,
            font_name: style.font_name,
            font: style.font,
            font_index: style.font_index,
            size: style.size,
            size_mode: self.size_mode,
            color: style.color,
            bold: style.bold,
            italic: style.italic,
            underline: style.underline,
            strikeout: style.strikeout,
            background: self.background,
            width: self.width,
            minimum_height: self.minimum_height,
            alignment: self.alignment,
            outline_width: self.outline_width,
            outline_color: self.outline_color,
            spans: self
                .spans
                .into_iter()
                .map(|span| {
                    Ok(TextSpan {
                        range: span.range,
                        style: span.style.into_style(fonts)?,
                    })
                })
                .collect::<Result<_, String>>()?,
            font_faces: self
                .font_faces
                .into_iter()
                .map(|face| {
                    Ok(EmbeddedFont {
                        family: face.family,
                        data: resolve_font(Some(face.data), fonts)?,
                        index: face.index,
                        bold: face.bold,
                        italic: face.italic,
                    })
                })
                .collect::<Result<_, String>>()?,
        })
    }
}

fn bounded_sequence<'de, D, T>(
    deserializer: D,
    count_limit: usize,
    byte_limit: usize,
    bytes: fn(&T) -> usize,
) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Bounded<T> {
        count_limit: usize,
        byte_limit: usize,
        bytes: fn(&T) -> usize,
    }
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Bounded<T> {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a bounded list of project assets or references")
        }

        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut sequence: A,
        ) -> Result<Self::Value, A::Error> {
            let mut entries = Vec::new();
            let mut bytes = 0usize;
            loop {
                if entries.len() == self.count_limit {
                    if sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {
                        return Err(serde::de::Error::custom(
                            "Project reference count exceeds its limit.",
                        ));
                    }
                    return Ok(entries);
                }
                let Some(entry) = sequence.next_element::<T>()? else {
                    return Ok(entries);
                };
                bytes = bytes.saturating_add((self.bytes)(&entry));
                if bytes > self.byte_limit {
                    return Err(serde::de::Error::custom(
                        "Project assets exceed their allocation limit.",
                    ));
                }
                entries.push(entry);
            }
        }
    }
    deserializer.deserialize_seq(Bounded {
        count_limit,
        byte_limit,
        bytes,
    })
}

fn deserialize_fonts<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<FontBytes>, D::Error> {
    bounded_sequence(d, MAX_FONT_ASSETS, MAX_OBJECT_BYTES, |font: &FontBytes| {
        font.len()
    })
}

fn deserialize_objects<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Vec<WireObject>, D::Error> {
    bounded_sequence(d, MAX_OBJECTS, MAX_OBJECT_BYTES, WireObject::non_font_bytes)
}

fn deserialize_layers<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<WireLayer>, D::Error> {
    struct Layers;
    impl<'de> serde::de::Visitor<'de> for Layers {
        type Value = Vec<WireLayer>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a bounded stack of Paint 10 layers")
        }

        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut sequence: A,
        ) -> Result<Self::Value, A::Error> {
            let mut layers = Vec::new();
            let mut bytes = 0usize;
            let mut objects = 0usize;
            let mut dimensions = None;
            loop {
                if layers.len() == crate::document::MAX_LAYERS {
                    if sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {
                        return Err(serde::de::Error::custom(
                            "Project exceeds the 64 layer limit.",
                        ));
                    }
                    return Ok(layers);
                }
                let Some(layer) = sequence.next_element::<WireLayer>()? else {
                    return Ok(layers);
                };
                if layer.name.trim().is_empty() || layer.name.len() > 256 {
                    return Err(serde::de::Error::custom(
                        "Layer names must contain 1–256 bytes of text.",
                    ));
                }
                if dimensions.is_some_and(|size| size != layer.image.dimensions()) {
                    return Err(serde::de::Error::custom(
                        "Every layer must have the same canvas dimensions.",
                    ));
                }
                dimensions = Some(layer.image.dimensions());
                objects += layer.objects.len();
                if objects > MAX_OBJECTS {
                    return Err(serde::de::Error::custom(
                        "Project objects exceed the 1,000 object limit across all layers.",
                    ));
                }
                bytes = bytes
                    .saturating_add(layer.image.as_raw().len())
                    .saturating_add(layer.name.len())
                    .saturating_add(
                        layer
                            .objects
                            .iter()
                            .map(WireObject::non_font_bytes)
                            .sum::<usize>(),
                    );
                if bytes > crate::document::MAX_LAYER_BYTES {
                    return Err(serde::de::Error::custom(
                        "Layers exceed the 192 MB image allocation limit.",
                    ));
                }
                layers.push(layer);
            }
        }
    }
    d.deserialize_seq(Layers)
}

fn deserialize_spans<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<WireSpan>, D::Error> {
    bounded_sequence(
        d,
        crate::text::MAX_SPANS,
        MAX_OBJECT_BYTES,
        |span: &WireSpan| std::mem::size_of::<TextSpan>() + span.style.font_name.len(),
    )
}

fn deserialize_faces<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<WireFace>, D::Error> {
    bounded_sequence(
        d,
        crate::text::MAX_FONT_FACES,
        MAX_OBJECT_BYTES,
        |face: &WireFace| std::mem::size_of::<EmbeddedFont>() + face.family.len(),
    )
}
