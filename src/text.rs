use crate::document::{blend, Color, BLACK, MAX_PIXELS};
use ab_glyph::{point, Font, FontRef, GlyphId, ScaleFont};
use image::{Rgba, RgbaImage};
use std::ops::Range;

mod editor_layout;
mod shaping;
#[cfg(test)]
mod shaping_tests;
pub use editor_layout::{EditorCaret, EditorSelectionRect};

const MAX_TEXT_CHARS: usize = 1024 * 1024;
const MAX_SPANS: usize = 4096;
pub const MAX_FONT_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_FORMAT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_TEXT_WIDTH: u32 = 16384;
pub const MAX_TEXT_OUTLINE: u32 = 32;
pub const FONT_POINT_RANGE: std::ops::RangeInclusive<f32> = 6.0..=200.0;
pub const MAX_FONT_FACES: usize = 128;

/// Whether the outline renderer can display a character from this face.
/// Color/bitmap-only glyph IDs do not count as a drawable outline.
pub fn font_supports_outline(font: &impl Font, character: char) -> bool {
    if shaping::default_ignorable(character) {
        return true;
    }
    let id = font.glyph_id(character);
    id.0 != 0 && (character.is_whitespace() || character.is_control() || font.outline(id).is_some())
}

/// A real font face retained with the text, including collection face indices.
/// Faces from other families also provide deterministic missing-glyph fallback.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EmbeddedFont {
    pub family: String,
    pub data: Vec<u8>,
    pub index: u32,
    pub bold: bool,
    pub italic: bool,
}

impl EmbeddedFont {
    pub fn validate(&self) -> Result<(), String> {
        if self.family.is_empty()
            || self.family.len() > 256
            || self.data.is_empty()
            || self.data.len() > MAX_FONT_BYTES
        {
            return Err("The embedded font exceeds the project limit.".into());
        }
        let font = FontRef::try_from_slice_and_index(&self.data, self.index)
            .map_err(|_| "Invalid embedded font or collection face index.")?;
        let scaled = font.as_scaled(24.0);
        if !(1.0..=1000.0).contains(&(scaled.height() + scaled.line_gap())) {
            return Err("Invalid font metrics.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TextAlignment {
    #[default]
    Left,
    Center,
    Right,
}

/// Logical text geometry in image pixels, relative to the text content inset.
#[derive(Clone, Debug)]
pub struct EditorLayout {
    pub width: f32,
    pub height: f32,
    pub rows: Vec<EditorRow>,
    pub elided: bool,
}

#[derive(Clone, Debug)]
pub struct EditorRow {
    pub glyphs: Vec<EditorGlyph>,
    pub source_range: Range<usize>,
    /// Grapheme boundaries ordered from the visual left edge to the right edge.
    pub visual_carets: Vec<EditorCaret>,
    pub right_to_left: bool,
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
    pub ascent: f32,
    pub ends_with_newline: bool,
}

/// One logical character cell, including invisible whitespace. Shaped glyphs
/// may span several cells. `x` is the leading edge; RTL advances are negative.
#[derive(Clone, Debug)]
pub struct EditorGlyph {
    pub character: char,
    pub character_index: usize,
    pub x: f32,
    pub advance: f32,
    pub baseline: f32,
    pub ascent: f32,
    pub height: f32,
}

fn default_outline_color() -> Color {
    BLACK
}

pub fn points_to_pixels(points: f32) -> f32 {
    points * 96.0 / 72.0
}

pub fn pixels_to_points(pixels: f32) -> f32 {
    pixels * 72.0 / 96.0
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TextStyle {
    pub font_name: String,
    pub font: Vec<u8>,
    #[serde(default)]
    pub font_index: u32,
    pub size: f32,
    pub color: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TextSpan {
    /// Unicode character indices, matching egui's text cursor indices.
    pub range: Range<usize>,
    pub style: TextStyle,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyleRef<'a> {
    pub font_name: &'a str,
    pub font: &'a [u8],
    pub font_index: u32,
    pub size: f32,
    pub color: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
}

impl<'a> TextStyleRef<'a> {
    pub fn to_owned(self) -> TextStyle {
        TextStyle {
            font_name: self.font_name.to_owned(),
            font: self.font.to_vec(),
            font_index: self.font_index,
            size: self.size,
            color: self.color,
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            strikeout: self.strikeout,
        }
    }

    pub fn font_bytes(self) -> &'a [u8] {
        font_bytes(self.font)
    }
}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TextFormat {
    pub font_name: String,
    pub font: Vec<u8>,
    #[serde(default)]
    pub font_index: u32,
    pub size: f32,
    pub color: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
    pub background: Option<Color>,
    pub width: u32,
    #[serde(default)]
    pub minimum_height: u32,
    #[serde(default)]
    pub alignment: TextAlignment,
    #[serde(default)]
    pub outline_width: u32,
    #[serde(default = "default_outline_color")]
    pub outline_color: Color,
    #[serde(default)]
    pub spans: Vec<TextSpan>,
    #[serde(default)]
    pub font_faces: Vec<EmbeddedFont>,
}

impl Default for TextFormat {
    fn default() -> Self {
        Self {
            font_name: "Sans serif".into(),
            font: vec![],
            font_index: 0,
            size: 24.0,
            color: BLACK,
            bold: false,
            italic: false,
            underline: false,
            strikeout: false,
            background: None,
            width: 280,
            minimum_height: 0,
            alignment: TextAlignment::Left,
            outline_width: 0,
            outline_color: BLACK,
            spans: vec![],
            font_faces: vec![],
        }
    }
}

impl TextStyle {
    pub fn as_ref(&self) -> TextStyleRef<'_> {
        TextStyleRef {
            font_name: &self.font_name,
            font: &self.font,
            font_index: self.font_index,
            size: self.size,
            color: self.color,
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            strikeout: self.strikeout,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_style(self.as_ref())
    }
}

impl TextFormat {
    pub fn needs_visual_layout(text: &str) -> bool {
        text.chars().any(shaping::required)
    }

    /// Insets shared by the editor and raster layout, including stroke antialiasing.
    pub fn text_padding(&self) -> (u32, u32) {
        let outline = self.outline_width.min(MAX_TEXT_OUTLINE);
        let inset = if outline == 0 { 0 } else { outline + 1 };
        (inset + 1, inset)
    }

    pub fn content_width(&self) -> u32 {
        self.width
            .clamp(10, MAX_TEXT_WIDTH)
            .saturating_sub(self.text_padding().0 * 2)
            .max(1)
    }

    pub fn default_style_ref(&self) -> TextStyleRef<'_> {
        TextStyleRef {
            font_name: &self.font_name,
            font: &self.font,
            font_index: self.font_index,
            size: self.size,
            color: self.color,
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            strikeout: self.strikeout,
        }
    }

    pub fn default_style(&self) -> TextStyle {
        self.default_style_ref().to_owned()
    }

    pub fn set_default_style(&mut self, style: &TextStyle) {
        self.font_name.clone_from(&style.font_name);
        self.font.clone_from(&style.font);
        self.font_index = style.font_index;
        self.size = style.size;
        self.color = style.color;
        self.bold = style.bold;
        self.italic = style.italic;
        self.underline = style.underline;
        self.strikeout = style.strikeout;
    }

    pub fn style_at(&self, character: usize) -> TextStyle {
        self.style_ref_at(character).to_owned()
    }

    pub fn style_ref_at(&self, character: usize) -> TextStyleRef<'_> {
        let index = self
            .spans
            .partition_point(|span| span.range.end <= character);
        self.spans
            .get(index)
            .filter(|span| span.range.contains(&character))
            .map_or_else(|| self.default_style_ref(), |span| span.style.as_ref())
    }

    /// Returns complete, non-overlapping style runs without copying embedded fonts.
    pub fn style_runs(&self, range: Range<usize>) -> Vec<(Range<usize>, TextStyleRef<'_>)> {
        let mut runs = vec![];
        let mut cursor = range.start;
        for span in &self.spans {
            if span.range.end <= cursor || span.range.start >= range.end {
                continue;
            }
            if cursor < span.range.start {
                runs.push((cursor..span.range.start, self.default_style_ref()));
            }
            let start = cursor.max(span.range.start);
            let end = range.end.min(span.range.end);
            runs.push((start..end, span.style.as_ref()));
            cursor = end;
        }
        if cursor < range.end {
            runs.push((cursor..range.end, self.default_style_ref()));
        }
        runs
    }

    pub fn apply_style(&mut self, range: Range<usize>, style: TextStyle) -> Result<(), String> {
        style.validate()?;
        if range.start > range.end || range.end > MAX_TEXT_CHARS {
            return Err("The text selection is outside the supported range.".into());
        }
        if range.is_empty() {
            return Ok(());
        }
        let mut spans = vec![];
        for span in &self.spans {
            if span.range.end <= range.start || span.range.start >= range.end {
                spans.push(span.clone());
                continue;
            }
            if span.range.start < range.start {
                spans.push(TextSpan {
                    range: span.range.start..range.start,
                    style: span.style.clone(),
                });
            }
            if span.range.end > range.end {
                spans.push(TextSpan {
                    range: range.end..span.range.end,
                    style: span.style.clone(),
                });
            }
        }
        if style != self.default_style() {
            spans.push(TextSpan { range, style });
        }
        self.replace_spans(spans)
    }

    /// Change only the requested property, preserving other formatting in a mixed selection.
    pub fn modify_style(
        &mut self,
        range: Range<usize>,
        mut change: impl FnMut(&mut TextStyle),
    ) -> Result<(), String> {
        if range.start > range.end || range.end > MAX_TEXT_CHARS {
            return Err("The text selection is outside the supported range.".into());
        }
        let runs: Vec<_> = self
            .style_runs(range)
            .into_iter()
            .map(|(range, style)| (range, style.to_owned()))
            .collect();
        let original = self.spans.clone();
        for (range, mut style) in runs {
            change(&mut style);
            if let Err(error) = self.apply_style(range, style) {
                self.spans = original;
                return Err(error);
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn update_spans_for_edit(&mut self, before: &str, after: &str) -> Result<(), String> {
        let (range, _) = changed_characters(before, after);
        let inherited = if range.is_empty() {
            range.start.saturating_sub(1)
        } else {
            range.start
        };
        self.update_spans_for_edit_with_style(before, after, self.style_at(inherited))
    }

    pub fn update_spans_for_edit_with_style(
        &mut self,
        before: &str,
        after: &str,
        insertion_style: TextStyle,
    ) -> Result<(), String> {
        if before == after {
            return Ok(());
        }
        if after.len() > MAX_TEXT_CHARS {
            return Err("A text box exceeds the 1 MB limit.".into());
        }
        insertion_style.validate()?;
        let (removed, inserted) = changed_characters(before, after);
        self.replace_span_range(removed, inserted, insertion_style)
    }

    /// Cursor positions disambiguate edits within runs of identical characters.
    pub fn update_spans_for_edit_at(
        &mut self,
        before: &str,
        after: &str,
        selection: Range<usize>,
        new_cursor: usize,
        insertion_style: TextStyle,
    ) -> Result<(), String> {
        if before == after {
            return Ok(());
        }
        if after.len() > MAX_TEXT_CHARS {
            return Err("A text box exceeds the 1 MB limit.".into());
        }
        insertion_style.validate()?;
        let old: Vec<_> = before.chars().collect();
        let new: Vec<_> = after.chars().collect();
        let candidate = if selection.start > selection.end || selection.end > old.len() {
            None
        } else if !selection.is_empty() && new.len() + selection.len() >= old.len() {
            Some((selection.clone(), new.len() + selection.len() - old.len()))
        } else if selection.is_empty() && new.len() >= old.len() {
            Some((selection.clone(), new.len() - old.len()))
        } else if selection.is_empty() {
            Some((
                new_cursor..new_cursor.saturating_add(old.len() - new.len()),
                0,
            ))
        } else {
            None
        };
        let change = candidate
            .filter(|(removed, inserted)| {
                removed.end <= old.len()
                    && removed.start + inserted <= new.len()
                    && old[..removed.start] == new[..removed.start]
                    && old[removed.end..] == new[removed.start + inserted..]
            })
            .unwrap_or_else(|| changed_characters(before, after));
        self.replace_span_range(change.0, change.1, insertion_style)
    }

    fn replace_span_range(
        &mut self,
        removed: Range<usize>,
        inserted: usize,
        insertion_style: TextStyle,
    ) -> Result<(), String> {
        let new_end = removed.start + inserted;
        let shift = |index: usize| new_end + (index - removed.end);
        let mut spans = vec![];
        for span in &self.spans {
            if span.range.start < removed.start {
                spans.push(TextSpan {
                    range: span.range.start..span.range.end.min(removed.start),
                    style: span.style.clone(),
                });
            }
            if span.range.end > removed.end {
                spans.push(TextSpan {
                    range: shift(span.range.start.max(removed.end))..shift(span.range.end),
                    style: span.style.clone(),
                });
            }
        }
        if inserted > 0 && insertion_style != self.default_style() {
            spans.push(TextSpan {
                range: removed.start..new_end,
                style: insertion_style,
            });
        }
        self.replace_spans(spans)
    }

    fn replace_spans(&mut self, mut spans: Vec<TextSpan>) -> Result<(), String> {
        spans.retain(|span| !span.range.is_empty());
        spans.sort_by_key(|span| span.range.start);
        let mut merged: Vec<TextSpan> = vec![];
        for span in spans {
            if let Some(previous) = merged.last_mut() {
                if previous.range.end == span.range.start && previous.style == span.style {
                    previous.range.end = span.range.end;
                    continue;
                }
            }
            merged.push(span);
        }
        let bytes = self.font.len()
            + self
                .font_faces
                .iter()
                .map(|face| face.data.len())
                .sum::<usize>()
            + merged
                .iter()
                .map(|span| span.style.font.len())
                .sum::<usize>();
        if merged.len() > MAX_SPANS || bytes > MAX_FORMAT_BYTES {
            return Err("This text box has reached its formatting limit.".into());
        }
        self.spans = merged;
        Ok(())
    }

    pub fn memory_bytes(&self) -> usize {
        self.font_name.len()
            + self.font.len()
            + self
                .font_faces
                .iter()
                .map(|face| {
                    std::mem::size_of::<EmbeddedFont>() + face.family.len() + face.data.len()
                })
                .sum::<usize>()
            + self
                .spans
                .iter()
                .map(|span| {
                    std::mem::size_of::<TextSpan>()
                        + span.style.font_name.len()
                        + span.style.font.len()
                })
                .sum::<usize>()
    }

    pub fn validate(&self) -> Result<(), String> {
        if !(10..=MAX_TEXT_WIDTH).contains(&self.width)
            || self.minimum_height > 16384
            || self.width as u64 * self.minimum_height as u64 > MAX_PIXELS
        {
            return Err("Invalid text-box dimensions.".into());
        }
        if self.outline_width > MAX_TEXT_OUTLINE || self.text_padding().0 * 2 >= self.width {
            return Err(
                "The text outline must be 0–32 pixels wide and fit inside the text box.".into(),
            );
        }
        validate_style(self.default_style_ref())?;
        if self.spans.len() > MAX_SPANS
            || self.font_faces.len() > MAX_FONT_FACES
            || self.memory_bytes() > MAX_FORMAT_BYTES
        {
            return Err("The text formatting exceeds the project limit.".into());
        }
        for face in &self.font_faces {
            face.validate()?;
        }
        let mut end = 0;
        for span in &self.spans {
            if span.range.start < end
                || span.range.start >= span.range.end
                || span.range.end > MAX_TEXT_CHARS
            {
                return Err("Text spans must be ordered, non-overlapping character ranges.".into());
            }
            span.style.validate()?;
            end = span.range.end;
        }
        Ok(())
    }

    pub fn validate_for_text(&self, text: &str) -> Result<(), String> {
        self.validate()?;
        if text.len() > MAX_TEXT_CHARS
            || self
                .spans
                .last()
                .is_some_and(|span| span.range.end > text.chars().count())
        {
            return Err("Text formatting refers to characters outside its text box.".into());
        }
        Ok(())
    }

    pub fn dimensions(&self, text: &str) -> (u32, u32) {
        let layout = self.layout(text);
        (layout.width, layout.height)
    }

    /// Reuse the raster layout for editor hit testing and selection. Explicit
    /// newlines belong to the preceding row; CRLF's CR is a zero-width glyph.
    pub fn editor_layout(&self, text: &str) -> EditorLayout {
        let layout = self.layout(text);
        let characters: Vec<_> = text.chars().take(MAX_TEXT_CHARS).collect();
        let (padding_x, padding_y) = self.text_padding();
        let mut rows = Vec::with_capacity(layout.lines.len());
        for line in &layout.lines {
            let mut glyphs = Vec::with_capacity(line.source_range.len());
            let mut rendered = line.cells.iter().peekable();
            let mut cursor_x = line.left;
            for character_index in line.source_range.clone() {
                let character = characters[character_index];
                let mut x = cursor_x;
                let mut end = cursor_x;
                let mut style = None;
                while rendered
                    .peek()
                    .is_some_and(|glyph| glyph.source_index == character_index)
                {
                    let glyph = rendered.next().unwrap();
                    if style.is_none() {
                        x = glyph.x;
                        style = Some(&layout.styles[glyph.style]);
                    }
                    end = glyph.x + glyph.advance;
                }
                let source_style;
                let style = match style {
                    Some(style) => style,
                    None => {
                        source_style =
                            RenderStyle::new(self.style_ref_at(character_index), &self.font_faces);
                        &source_style
                    }
                };
                glyphs.push(EditorGlyph {
                    character,
                    character_index,
                    x: x - padding_x as f32,
                    advance: end - x,
                    baseline: line.baseline - padding_y as f32,
                    ascent: style.ascent,
                    height: style.ascent + style.descent + style.gap,
                });
                cursor_x = end;
            }
            rows.push(EditorRow {
                glyphs,
                source_range: line.source_range.clone(),
                visual_carets: Vec::new(),
                right_to_left: line.right_to_left,
                left: line.left - padding_x as f32,
                top: line.top - padding_y as f32,
                width: line.advance,
                height: line.height,
                baseline: line.baseline - padding_y as f32,
                ascent: line.baseline - line.top,
                ends_with_newline: line.ends_with_newline,
            });
        }
        let mut editor = EditorLayout {
            width: self.content_width() as f32,
            height: (layout.height as f32 - 2.0 * padding_y as f32).max(0.0),
            rows,
            elided: layout.elided,
        };
        editor.populate_carets(text);
        editor
    }

    fn layout(&self, text: &str) -> TextLayout<'_> {
        let mut styles = vec![RenderStyle::new(self.default_style_ref(), &self.font_faces)];
        styles.extend(
            self.spans
                .iter()
                .take(MAX_SPANS)
                .map(|span| RenderStyle::new(span.style.as_ref(), &self.font_faces)),
        );
        let base_style_count = styles.len();
        let available_faces: Vec<_> = self
            .font_faces
            .iter()
            .take(MAX_FONT_FACES)
            .enumerate()
            .filter_map(|(index, face)| {
                FontRef::try_from_slice_and_index(&face.data, face.index)
                    .ok()
                    .map(|font| (index, font))
            })
            .collect();
        let mut fallback_styles = std::collections::HashMap::new();
        let mut character_styles = std::collections::HashMap::new();
        let mut chars = vec![];
        let mut span_index = 0;
        let mut source_count = 0;
        let mut source = text.chars().take(MAX_TEXT_CHARS).enumerate().peekable();
        while let Some((index, character)) = source.next() {
            source_count = index + 1;
            while span_index < self.spans.len() && self.spans[span_index].range.end <= index {
                span_index += 1;
            }
            let base_style = if self
                .spans
                .get(span_index)
                .is_some_and(|span| span.range.contains(&index))
                && span_index + 1 < base_style_count
            {
                span_index + 1
            } else {
                0
            };
            let style = if let Some(&cached) = character_styles.get(&(base_style, character)) {
                cached
            } else {
                let resolved = if !character.is_control()
                    && !character.is_whitespace()
                    && !shaping::default_ignorable(character)
                    && !font_supports_outline(&styles[base_style].font, character)
                {
                    let source = styles[base_style].source;
                    let fallback = available_faces
                        .iter()
                        .filter(|(_, font)| font_supports_outline(font, character))
                        .min_by_key(|(index, _)| {
                            let face = &self.font_faces[*index];
                            (
                                u8::from(!face.family.eq_ignore_ascii_case(source.font_name)),
                                u8::from(face.bold != source.bold)
                                    + u8::from(face.italic != source.italic),
                            )
                        });
                    if let Some((face_index, _)) = fallback {
                        *fallback_styles
                            .entry((base_style, *face_index))
                            .or_insert_with(|| {
                                let index = styles.len();
                                styles.push(RenderStyle::from_face(
                                    source,
                                    &self.font_faces[*face_index],
                                ));
                                index
                            })
                    } else {
                        *fallback_styles
                            .entry((base_style, usize::MAX))
                            .or_insert_with(|| {
                                let index = styles.len();
                                styles.push(RenderStyle::from_font(
                                    source,
                                    epaint_default_fonts::UBUNTU_LIGHT,
                                    0,
                                    false,
                                    false,
                                ));
                                index
                            })
                    }
                } else {
                    base_style
                };
                // Bound the cache independently of input length. Ordinary text
                // repeatedly uses a small character set in each style run.
                if character_styles.len() < 4096 {
                    character_styles.insert((base_style, character), resolved);
                }
                resolved
            };
            if character == '\r' && source.peek().is_some_and(|(_, next)| *next == '\n') {
                continue;
            }
            if character == '\t' {
                chars.extend((0..4).map(|_| StyledChar {
                    character: ' ',
                    style,
                    source_index: index,
                }));
            } else {
                chars.push(StyledChar {
                    character,
                    style,
                    source_index: index,
                });
            }
        }
        let width = self.width.clamp(10, MAX_TEXT_WIDTH);
        shaping::unify_grapheme_fonts(&mut chars, &mut styles, &available_faces, &self.font_faces);
        let (padding_x, padding_y) = self.text_padding();
        let content_width = self.content_width() as f32;
        let max_height = (MAX_PIXELS / width as u64).min(16384) as u32;
        let mut lines = vec![];
        let mut top = padding_y as f32;
        let mut start = 0;
        let mut source_start = 0;
        while start <= chars.len() && top + 4.0 < max_height as f32 {
            let paragraph_end = chars[start..]
                .iter()
                .position(|c| c.character == '\n')
                .map_or(chars.len(), |offset| start + offset);
            let blank_style = chars.get(start).map_or(0, |c| c.style);
            let paragraph_start = start;
            let shaped = shaping::required_chars(&chars[start..paragraph_end])
                .then(|| shaping::Paragraph::new(&chars[start..paragraph_end], &styles));
            if start == paragraph_end {
                let mut line = layout_line(&[], &styles, blank_style, top);
                let source_end = chars
                    .get(paragraph_end)
                    .map_or(source_count, |c| c.source_index);
                line.source_range = source_start..source_end;
                line.ends_with_newline = paragraph_end < chars.len();
                source_start = source_end + usize::from(line.ends_with_newline);
                top += line.height;
                lines.push(line);
            } else {
                while start < paragraph_end && top + 4.0 < max_height as f32 {
                    let end = shaped.as_ref().map_or_else(
                        || wrap_end(&chars, start, paragraph_end, &styles, content_width),
                        |paragraph| {
                            paragraph.wrap_end(start - paragraph_start, content_width, &styles)
                                + paragraph_start
                        },
                    );
                    let mut visible_end = end;
                    if end < paragraph_end {
                        while visible_end > start
                            && chars[visible_end - 1].character.is_whitespace()
                        {
                            visible_end -= 1;
                        }
                    }
                    let mut line = shaped.as_ref().map_or_else(
                        || layout_line(&chars[start..visible_end], &styles, blank_style, top),
                        |paragraph| {
                            paragraph.line(
                                start - paragraph_start..visible_end - paragraph_start,
                                &styles,
                                blank_style,
                                top,
                            )
                        },
                    );
                    top += line.height;
                    start = end;
                    if start < paragraph_end {
                        while start < paragraph_end && chars[start].character.is_whitespace() {
                            start += 1;
                        }
                    }
                    let source_end = chars.get(start).map_or(source_count, |c| c.source_index);
                    line.source_range = source_start..source_end;
                    line.ends_with_newline = start == paragraph_end && paragraph_end < chars.len();
                    source_start = source_end + usize::from(line.ends_with_newline);
                    lines.push(line);
                }
            }
            if paragraph_end == chars.len() {
                break;
            }
            start = paragraph_end + 1;
        }
        let factor = match self.alignment {
            TextAlignment::Left => 0.0,
            TextAlignment::Center => 0.5,
            TextAlignment::Right => 1.0,
        };
        for line in &mut lines {
            let offset = padding_x as f32 - 1.0 + (content_width - line.width).max(0.0) * factor;
            line.left += offset;
            for glyph in &mut line.glyphs {
                glyph.x += offset;
            }
            for cell in &mut line.cells {
                cell.x += offset;
            }
        }
        TextLayout {
            styles,
            lines,
            width,
            elided: source_start < source_count || text.chars().count() > MAX_TEXT_CHARS,
            height: (top + padding_y as f32 + 4.0)
                .ceil()
                .max(self.minimum_height as f32)
                .max(1.0)
                .min(max_height as f32) as u32,
        }
    }

    pub fn render(&self, text: &str) -> RgbaImage {
        let background = self.background.unwrap_or([0, 0, 0, 0]);
        if self.outline_width == 0 {
            return self.render_glyphs(text, background);
        }
        let foreground = self.render_glyphs(text, [0, 0, 0, 0]);
        let mut image =
            RgbaImage::from_pixel(foreground.width(), foreground.height(), Rgba(background));
        paint_text_outline(
            &mut image,
            &foreground,
            self.outline_width.min(MAX_TEXT_OUTLINE),
            self.outline_color,
        );
        crate::document::overlay(&mut image, &foreground, 0, 0);
        image
    }

    /// The stroke underlay at the same positions as render, without text fill or background.
    pub fn render_outline(&self, text: &str) -> RgbaImage {
        let foreground = self.render_glyphs(text, [0, 0, 0, 0]);
        let mut image = RgbaImage::new(foreground.width(), foreground.height());
        paint_text_outline(
            &mut image,
            &foreground,
            self.outline_width.min(MAX_TEXT_OUTLINE),
            self.outline_color,
        );
        image
    }

    fn render_glyphs(&self, text: &str, background: Color) -> RgbaImage {
        let layout = self.layout(text);
        let mut image = RgbaImage::from_pixel(layout.width, layout.height, Rgba(background));
        for line in &layout.lines {
            for glyph in &line.glyphs {
                let style = &layout.styles[glyph.style];
                if let Some(outline) =
                    style.font.outline_glyph(glyph.id.with_scale_and_position(
                        style.size,
                        point(glyph.x, line.baseline + glyph.y),
                    ))
                {
                    let bounds = outline.px_bounds();
                    outline.draw(|x, y, coverage| {
                        let y = y as i32 + bounds.min.y as i32;
                        let shear = if style.synthetic_italic {
                            ((line.baseline - y as f32) * 0.2).round() as i32
                        } else {
                            0
                        };
                        let x = x as i32 + bounds.min.x as i32 + shear;
                        let mut color = style.source.color;
                        color[3] = (coverage * color[3] as f32) as u8;
                        blend(&mut image, x, y, color);
                        if style.synthetic_bold {
                            blend(&mut image, x + 1, y, color);
                        }
                    });
                }
                for (enabled, y) in [
                    (style.source.underline, line.baseline + 2.0),
                    (style.source.strikeout, line.baseline - style.size * 0.28),
                ] {
                    if enabled {
                        for x in glyph.x.floor() as i32..(glyph.x + glyph.advance).ceil() as i32 {
                            for dy in 0..(style.size / 18.0).ceil() as i32 {
                                blend(&mut image, x, y as i32 + dy, style.source.color);
                            }
                        }
                    }
                }
            }
        }
        image
    }
}

/// A separable squared-distance transform produces round outlines in linear time.
fn paint_text_outline(output: &mut RgbaImage, foreground: &RgbaImage, radius: u32, color: Color) {
    if radius == 0 || color[3] == 0 || !foreground.pixels().any(|pixel| pixel[3] > 0) {
        return;
    }
    let width = foreground.width() as usize;
    let height = foreground.height() as usize;
    let reach = radius as i32 + 1;
    let mut distance = vec![f32::INFINITY; width * height];
    for y in 0..height {
        let mut previous = -reach;
        for x in 0..width {
            if foreground.get_pixel(x as u32, y as u32)[3] > 0 {
                previous = x as i32;
            }
            let delta = x as i32 - previous;
            if delta < reach {
                distance[y * width + x] = (delta * delta) as f32;
            }
        }
        let mut next = width as i32 + reach;
        for x in (0..width).rev() {
            if foreground.get_pixel(x as u32, y as u32)[3] > 0 {
                next = x as i32;
            }
            let delta = next - x as i32;
            if delta < reach {
                distance[y * width + x] = distance[y * width + x].min((delta * delta) as f32);
            }
        }
    }
    let mut sites = Vec::<usize>::with_capacity(height);
    let mut starts = Vec::<f64>::with_capacity(height);
    for x in 0..width {
        sites.clear();
        starts.clear();
        for y in 0..height {
            let value = f64::from(distance[y * width + x]);
            if !value.is_finite() {
                continue;
            }
            let mut start = f64::NEG_INFINITY;
            while let Some(&previous) = sites.last() {
                let previous_value = f64::from(distance[previous * width + x]);
                start = (value + (y * y) as f64 - previous_value - (previous * previous) as f64)
                    / (2.0 * (y - previous) as f64);
                if start > *starts.last().unwrap() {
                    break;
                }
                sites.pop();
                starts.pop();
            }
            if sites.is_empty() {
                start = f64::NEG_INFINITY;
            }
            sites.push(y);
            starts.push(start);
        }
        if sites.is_empty() {
            continue;
        }
        let mut site = 0;
        for y in 0..height {
            while site + 1 < sites.len() && starts[site + 1] <= y as f64 {
                site += 1;
            }
            let delta = y as f32 - sites[site] as f32;
            let distance = (distance[sites[site] * width + x] + delta * delta).sqrt();
            let coverage = (reach as f32 - distance).clamp(0.0, 1.0);
            if coverage > 0.0 {
                let mut pixel = color;
                pixel[3] = (f32::from(color[3]) * coverage).round() as u8;
                blend(output, x as i32, y as i32, pixel);
            }
        }
    }
}

fn font_bytes(bytes: &[u8]) -> &[u8] {
    if bytes.is_empty() {
        epaint_default_fonts::UBUNTU_LIGHT
    } else {
        bytes
    }
}

fn validate_style(style: TextStyleRef<'_>) -> Result<(), String> {
    if !style.size.is_finite() || !FONT_POINT_RANGE.contains(&pixels_to_points(style.size)) {
        return Err("Invalid text size.".into());
    }
    if style.font_name.len() > 256 || style.font.len() > MAX_FONT_BYTES {
        return Err("The embedded font exceeds the project limit.".into());
    }
    let font = FontRef::try_from_slice_and_index(font_bytes(style.font), style.font_index)
        .map_err(|_| "Invalid embedded font or collection face index.")?;
    let scaled = font.as_scaled(style.size);
    if !(1.0..=1000.0).contains(&(scaled.height() + scaled.line_gap())) {
        return Err("Invalid font metrics.".into());
    }
    Ok(())
}

fn changed_characters(before: &str, after: &str) -> (Range<usize>, usize) {
    let before: Vec<_> = before.chars().collect();
    let after: Vec<_> = after.chars().collect();
    let prefix = before
        .iter()
        .zip(&after)
        .take_while(|(a, b)| a == b)
        .count();
    let suffix = before[prefix..]
        .iter()
        .rev()
        .zip(after[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    (prefix..before.len() - suffix, after.len() - prefix - suffix)
}

struct RenderStyle<'a> {
    source: TextStyleRef<'a>,
    font: FontRef<'a>,
    size: f32,
    ascent: f32,
    descent: f32,
    gap: f32,
    font_index: u32,
    synthetic_bold: bool,
    synthetic_italic: bool,
}

impl<'a> RenderStyle<'a> {
    fn new(source: TextStyleRef<'a>, faces: &'a [EmbeddedFont]) -> Self {
        let matching = faces
            .iter()
            .take(MAX_FONT_FACES)
            .filter(|face| {
                face.family.eq_ignore_ascii_case(source.font_name)
                    && (!face.bold || source.bold)
                    && (!face.italic || source.italic)
            })
            .min_by_key(|face| {
                (
                    u8::from(face.bold != source.bold) + u8::from(face.italic != source.italic),
                    u8::from(
                        face.index != source.font_index || face.data != font_bytes(source.font),
                    ),
                )
            });
        if let Some(face) = matching {
            Self::from_face(source, face)
        } else {
            Self::from_font(
                source,
                font_bytes(source.font),
                source.font_index,
                false,
                false,
            )
        }
    }

    fn from_face(source: TextStyleRef<'a>, face: &'a EmbeddedFont) -> Self {
        Self::from_font(source, &face.data, face.index, face.bold, face.italic)
    }

    fn from_font(
        source: TextStyleRef<'a>,
        bytes: &'a [u8],
        index: u32,
        bold: bool,
        italic: bool,
    ) -> Self {
        let fallback = || FontRef::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT).unwrap();
        let parsed = FontRef::try_from_slice_and_index(bytes, index);
        let mut used_fallback = parsed.is_err();
        let mut font = parsed.unwrap_or_else(|_| fallback());
        let size = if source.size.is_finite() {
            source
                .size
                .clamp(points_to_pixels(6.0), points_to_pixels(200.0))
        } else {
            24.0
        };
        let scaled = font.as_scaled(size);
        if !(1.0..=1000.0).contains(&(scaled.height() + scaled.line_gap())) {
            font = fallback();
            used_fallback = true;
        }
        let scaled = font.as_scaled(size);
        let (ascent, descent, gap) = (
            scaled.ascent(),
            -scaled.descent(),
            scaled.line_gap().max(0.0),
        );
        Self {
            source,
            font,
            size,
            ascent,
            descent,
            gap,
            font_index: if used_fallback { 0 } else { index },
            synthetic_bold: source.bold && (!bold || used_fallback),
            synthetic_italic: source.italic && (!italic || used_fallback),
        }
    }

    fn overhang(&self) -> f32 {
        (if self.synthetic_italic {
            self.size * 0.2
        } else {
            0.0
        }) + if self.synthetic_bold { 1.0 } else { 0.0 }
    }
}

#[derive(Clone, Copy)]
struct StyledChar {
    character: char,
    style: usize,
    source_index: usize,
}

struct PositionedGlyph {
    id: GlyphId,
    style: usize,
    x: f32,
    y: f32,
    advance: f32,
    source_index: usize,
}

/// Logical character edges, independent of the number/order of shaped glyphs.
#[derive(Clone)]
struct CharacterCell {
    source_index: usize,
    style: usize,
    x: f32,
    advance: f32,
    wrap_cluster: Range<usize>,
}

struct TextLine {
    glyphs: Vec<PositionedGlyph>,
    cells: Vec<CharacterCell>,
    advance: f32,
    right_to_left: bool,
    baseline: f32,
    height: f32,
    width: f32,
    left: f32,
    top: f32,
    source_range: Range<usize>,
    ends_with_newline: bool,
}

struct TextLayout<'a> {
    styles: Vec<RenderStyle<'a>>,
    lines: Vec<TextLine>,
    width: u32,
    height: u32,
    elided: bool,
}

fn advance(
    character: StyledChar,
    previous: Option<(GlyphId, usize)>,
    styles: &[RenderStyle<'_>],
) -> (GlyphId, f32, f32) {
    let style = &styles[character.style];
    let scaled = style.font.as_scaled(style.size);
    let id = scaled.glyph_id(render_character(character.character, style));
    let kern = previous.map_or(0.0, |(previous, previous_style)| {
        let prior = &styles[previous_style];
        if prior.size == style.size
            && (previous_style == character.style
                || (prior.font.font_data() == style.font.font_data()
                    && prior.font_index == style.font_index))
        {
            scaled.kern(previous, id)
        } else {
            0.0
        }
    });
    (id, scaled.h_advance(id), kern)
}

/// Missing glyphs have visible fallback ink, but bidi/grapheme analysis must
/// still see the original Unicode character rather than a replacement '?'.
fn render_character(character: char, style: &RenderStyle<'_>) -> char {
    if !character.is_control()
        && !character.is_whitespace()
        && !shaping::default_ignorable(character)
        && style.font.glyph_id(character).0 == 0
    {
        '?'
    } else {
        character
    }
}

fn wrap_end(
    chars: &[StyledChar],
    start: usize,
    paragraph_end: usize,
    styles: &[RenderStyle<'_>],
    width: f32,
) -> usize {
    let mut end = start;
    let mut x = 0.0;
    let mut previous = None;
    let mut last_break = None;
    while end < paragraph_end {
        let character = chars[end];
        let (id, step, kern) = advance(character, previous, styles);
        if x + step + kern + styles[character.style].overhang() > width && end > start {
            break;
        }
        x += step + kern;
        previous = Some((id, character.style));
        end += 1;
        if character.character.is_whitespace() {
            last_break = Some(end);
        }
    }
    if end < paragraph_end {
        if let Some(word_end) = last_break {
            return word_end;
        }
    }
    end.max(start + 1)
}

fn layout_line(
    chars: &[StyledChar],
    styles: &[RenderStyle<'_>],
    blank_style: usize,
    top: f32,
) -> TextLine {
    let mut ascent: f32 = 0.0;
    let mut descent: f32 = 0.0;
    let mut gap: f32 = 0.0;
    for style in chars
        .iter()
        .map(|character| &styles[character.style])
        .chain(chars.is_empty().then_some(&styles[blank_style]))
    {
        ascent = ascent.max(style.ascent);
        descent = descent.max(style.descent);
        gap = gap.max(style.gap);
    }
    let mut x = 1.0;
    let mut previous = None;
    let glyphs = chars
        .iter()
        .map(|&character| {
            let (id, step, kern) = advance(character, previous, styles);
            x += kern;
            let glyph = PositionedGlyph {
                id,
                style: character.style,
                x,
                y: 0.0,
                advance: step,
                source_index: character.source_index,
            };
            x += step;
            previous = Some((id, character.style));
            glyph
        })
        .collect::<Vec<_>>();
    let cells = glyphs
        .iter()
        .map(|glyph| CharacterCell {
            source_index: glyph.source_index,
            style: glyph.style,
            x: glyph.x,
            advance: glyph.advance,
            wrap_cluster: glyph.source_index..glyph.source_index + 1,
        })
        .collect();
    TextLine {
        glyphs,
        cells,
        advance: x - 1.0,
        right_to_left: false,
        baseline: top + ascent,
        height: ascent + descent + gap,
        left: 1.0,
        top,
        source_range: 0..0,
        ends_with_newline: false,
        width: x - 1.0
            + chars
                .last()
                .map_or(0.0, |character| styles[character.style].overhang()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn embedded(family: &str, bytes: &[u8], bold: bool, italic: bool) -> EmbeddedFont {
        EmbeddedFont {
            family: family.into(),
            data: bytes.to_vec(),
            index: 0,
            bold,
            italic,
        }
    }

    #[test]
    fn embedded_variant_uses_its_real_metrics_without_extra_bold_or_shear() {
        // Different bundled faces make selecting the stored variant observable
        // in both glyph pixels and caret advances, without relying on host fonts.
        let mut format = TextFormat {
            font_name: "Variant fixture".into(),
            font: epaint_default_fonts::UBUNTU_LIGHT.to_vec(),
            bold: true,
            italic: true,
            font_faces: vec![
                embedded(
                    "Variant fixture",
                    epaint_default_fonts::UBUNTU_LIGHT,
                    false,
                    false,
                ),
                embedded(
                    "Variant fixture",
                    epaint_default_fonts::HACK_REGULAR,
                    true,
                    true,
                ),
            ],
            ..Default::default()
        };
        format.validate().unwrap();
        let expected = TextFormat {
            font: epaint_default_fonts::HACK_REGULAR.to_vec(),
            ..Default::default()
        };
        assert_eq!(
            format.render("Wide AV text"),
            expected.render("Wide AV text")
        );
        let actual = format.editor_layout("Wide AV text");
        let expected = expected.editor_layout("Wide AV text");
        assert_eq!(actual.rows[0].height, expected.rows[0].height);
        for (actual, expected) in actual.rows[0].glyphs.iter().zip(&expected.rows[0].glyphs) {
            assert_eq!(actual.x, expected.x);
            assert_eq!(actual.advance, expected.advance);
        }
        format.italic = false;
        let style = RenderStyle::new(format.default_style_ref(), &format.font_faces);
        assert!(style.synthetic_bold);
        assert!(!style.synthetic_italic);
        assert_eq!(style.font.font_data(), epaint_default_fonts::UBUNTU_LIGHT);
    }

    #[test]
    fn missing_glyph_fallback_keeps_original_characters_and_shared_geometry() {
        let character = '\u{1f600}';
        assert_eq!(
            FontRef::try_from_slice(epaint_default_fonts::HACK_REGULAR)
                .unwrap()
                .glyph_id(character)
                .0,
            0
        );
        assert_ne!(
            FontRef::try_from_slice(epaint_default_fonts::NOTO_EMOJI_REGULAR)
                .unwrap()
                .glyph_id(character)
                .0,
            0
        );
        let format = TextFormat {
            font: epaint_default_fonts::HACK_REGULAR.to_vec(),
            width: 100,
            font_faces: vec![embedded(
                "Noto Emoji",
                epaint_default_fonts::NOTO_EMOJI_REGULAR,
                false,
                false,
            )],
            ..Default::default()
        };
        let expected = TextFormat {
            font: epaint_default_fonts::NOTO_EMOJI_REGULAR.to_vec(),
            width: 100,
            ..Default::default()
        };
        assert_eq!(format.render("😀"), expected.render("😀"));
        let text = "A😀\tB😀\r\n😀";
        let layout = format.layout(text);
        assert_eq!(
            layout.styles.len(),
            2,
            "reuse the same fallback face for repeated glyphs"
        );
        for line in &layout.lines {
            for glyph in &line.glyphs {
                assert_ne!(glyph.id.0, 0);
            }
        }
        let editor = format.editor_layout(text);
        let reconstructed: String = editor
            .rows
            .iter()
            .flat_map(|row| {
                row.glyphs
                    .iter()
                    .map(|glyph| glyph.character)
                    .chain(row.ends_with_newline.then_some('\n'))
            })
            .collect();
        assert_eq!(reconstructed, text);
    }

    #[test]
    fn glyph_ids_without_outlines_use_a_drawable_fallback() {
        // Remove only the outline table from a real font. Its cmap and metrics
        // still resolve glyph IDs, reproducing the color/bitmap-only face case.
        let mut bytes = epaint_default_fonts::HACK_REGULAR.to_vec();
        let count = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
        let mut removed = false;
        for entry in bytes[12..].chunks_exact_mut(16).take(count) {
            if &entry[..4] == b"glyf" {
                entry[..4].copy_from_slice(b"TEST");
                removed = true;
            }
        }
        assert!(removed);
        let without_outlines = FontRef::try_from_slice(&bytes).unwrap();
        assert_ne!(without_outlines.glyph_id('A').0, 0);
        assert!(!font_supports_outline(&without_outlines, 'A'));
        let format = TextFormat {
            font: bytes,
            font_faces: vec![embedded(
                "Fallback",
                epaint_default_fonts::UBUNTU_LIGHT,
                false,
                false,
            )],
            ..Default::default()
        };
        assert_eq!(format.render("ABC"), TextFormat::default().render("ABC"));
        let mut without_supplied_fallback = format.clone();
        without_supplied_fallback.font_faces.clear();
        assert_eq!(
            without_supplied_fallback.render("ABC"),
            TextFormat::default().render("ABC")
        );
        assert_eq!(
            without_supplied_fallback.render("\u{10ffff}"),
            TextFormat::default().render("?")
        );
        assert_eq!(
            without_supplied_fallback.editor_layout("\u{10ffff}").rows[0].glyphs[0].character,
            '\u{10ffff}'
        );
    }

    #[test]
    fn matching_family_faces_preserve_exact_embedded_source_bytes() {
        let expected = TextFormat {
            font_name: "Same family".into(),
            font: epaint_default_fonts::HACK_REGULAR.to_vec(),
            ..Default::default()
        };
        let mut with_faces = expected.clone();
        with_faces.font_faces = vec![
            embedded(
                "Same family",
                epaint_default_fonts::UBUNTU_LIGHT,
                false,
                false,
            ),
            embedded(
                "Same family",
                epaint_default_fonts::HACK_REGULAR,
                false,
                false,
            ),
        ];
        assert_eq!(
            with_faces.render("Saved font version"),
            expected.render("Saved font version")
        );
    }

    #[test]
    fn embedded_faces_are_bounded_validated_and_optional_in_legacy_text() {
        let format = TextFormat::default();
        let mut value = serde_json::to_value(&format).unwrap();
        value.as_object_mut().unwrap().remove("font_faces");
        let legacy: TextFormat = serde_json::from_value(value).unwrap();
        assert!(legacy.font_faces.is_empty());
        assert_eq!(
            legacy.render("Existing project"),
            format.render("Existing project")
        );
        let face = embedded("Hack", epaint_default_fonts::HACK_REGULAR, false, false);
        let mut invalid = TextFormat {
            font_faces: vec![face.clone()],
            ..Default::default()
        };
        invalid.font_faces[0].index = u32::MAX;
        assert!(invalid.validate().is_err());
        invalid.font_faces[0] = face.clone();
        invalid.font_faces[0].data = b"not a font".to_vec();
        assert!(invalid.validate().is_err());
        invalid.font_faces = (0..=MAX_FONT_FACES)
            .map(|_| EmbeddedFont {
                data: vec![],
                ..face.clone()
            })
            .collect();
        assert!(invalid.validate().unwrap_err().contains("limit"));
        let with_face = TextFormat {
            font_faces: vec![face.clone()],
            ..Default::default()
        };
        assert!(with_face.memory_bytes() >= format.memory_bytes() + face.data.len());
    }

    #[test]
    fn editor_rows_preserve_original_character_indices_through_wrapping_and_whitespace() {
        for text in [
            "",
            "Caption test",
            "a\tword  another\tword\n",
            "\r\n\nalpha\r\nbeta\r\n",
            "   many    spaces   and a longwordwithoutbreaks   ",
            "é猫\tUnicode\r\nβγ\n",
            "\t\t\t",
        ] {
            for width in [30, 80, 280] {
                for alignment in [
                    TextAlignment::Left,
                    TextAlignment::Center,
                    TextAlignment::Right,
                ] {
                    let format = TextFormat {
                        width,
                        alignment,
                        outline_width: 2,
                        ..Default::default()
                    };
                    let layout = format.editor_layout(text);
                    assert!(!layout.rows.is_empty());
                    assert!(!layout.elided);
                    let mut reconstructed = String::new();
                    let mut index = 0;
                    for row in &layout.rows {
                        assert!(row.height > 0.0);
                        for glyph in &row.glyphs {
                            assert_eq!(glyph.character_index, index, "{text:?}, width {width}");
                            reconstructed.push(glyph.character);
                            index += 1;
                        }
                        if row.ends_with_newline {
                            reconstructed.push('\n');
                            index += 1;
                        }
                    }
                    assert_eq!(reconstructed, text);
                    assert_eq!(index, text.chars().count());
                    assert!(!layout.rows.last().unwrap().ends_with_newline);
                }
            }
        }
    }

    #[test]
    fn editor_geometry_uses_the_exact_raster_positions_for_mixed_caption_styles() {
        let mut format = TextFormat {
            size: 32.0,
            width: 240,
            outline_width: 3,
            alignment: TextAlignment::Center,
            ..Default::default()
        };
        let text = "Caption test\nWrapped words";
        format
            .modify_style(8..12, |style| {
                style.size = 54.0;
                style.bold = true;
            })
            .unwrap();
        format
            .modify_style(2..7, |style| {
                style.italic = true;
                style.underline = true;
            })
            .unwrap();
        let raster = format.layout(text);
        let editor = format.editor_layout(text);
        let (padding_x, padding_y) = format.text_padding();
        assert_eq!(editor.rows.len(), raster.lines.len());
        for (row, line) in editor.rows.iter().zip(&raster.lines) {
            assert_eq!(row.top + padding_y as f32, line.top);
            assert_eq!(row.baseline + padding_y as f32, line.baseline);
            for raster_glyph in &line.glyphs {
                let glyph = row
                    .glyphs
                    .iter()
                    .find(|glyph| glyph.character_index == raster_glyph.source_index)
                    .unwrap();
                assert_eq!(glyph.x + padding_x as f32, raster_glyph.x);
                assert!((glyph.advance - raster_glyph.advance).abs() < 0.001);
                assert_eq!(glyph.baseline + padding_y as f32, line.baseline);
                assert_eq!(glyph.ascent, raster.styles[raster_glyph.style].ascent);
            }
            let end = row
                .glyphs
                .last()
                .map_or(row.left, |glyph| glyph.x + glyph.advance);
            assert!((row.left + row.width - end).abs() < 0.001);
        }
    }

    #[test]
    fn editor_tabs_aggregate_spaces_and_crlf_keeps_a_zero_width_carriage_return() {
        let format = TextFormat::default();
        let tab = format.editor_layout("a\tb\r\n");
        let spaces = format.editor_layout("a    b\r\n");
        assert_eq!(tab.rows[0].glyphs.len(), 4);
        assert_eq!(tab.rows[0].glyphs[2].x, spaces.rows[0].glyphs[5].x);
        assert_eq!(tab.rows[0].glyphs[3].character, '\r');
        assert_eq!(tab.rows[0].glyphs[3].advance, 0.0);
        assert!(tab.rows[0].ends_with_newline);
        assert!(tab.rows[1].glyphs.is_empty());
    }

    #[test]
    fn white_bold_glyph_coverage_never_corrupts_its_fill_color() {
        let format = TextFormat {
            width: 420,
            size: 62.0,
            bold: true,
            color: [255, 255, 255, 255],
            ..Default::default()
        };
        let raster = format.render("CAPTION");
        assert!(raster.pixels().any(|pixel| (1..255).contains(&pixel[3])));
        assert!(raster
            .pixels()
            .filter(|pixel| pixel[3] > 0)
            .all(|pixel| pixel.0[..3] == [255, 255, 255]));
    }

    #[test]
    fn alignment_moves_wrapped_rich_text_without_changing_its_styles() {
        let mut format = TextFormat {
            font: epaint_default_fonts::HACK_REGULAR.to_vec(),
            size: 28.0,
            width: 160,
            ..Default::default()
        };
        format
            .modify_style(2..4, |style| {
                style.color = [255, 0, 0, 255];
                style.bold = true;
            })
            .unwrap();
        let text = "ABCD EFGH IJKL";
        let left = format.layout(text);
        assert!(left.lines.len() > 1);
        for (alignment, factor) in [(TextAlignment::Center, 0.5), (TextAlignment::Right, 1.0)] {
            let aligned = TextFormat {
                alignment,
                ..format.clone()
            };
            let layout = aligned.layout(text);
            assert_eq!(layout.lines.len(), left.lines.len());
            for (plain, moved) in left.lines.iter().zip(&layout.lines) {
                let offset = (format.content_width() as f32 - plain.width) * factor;
                assert!(offset > 0.0);
                for (before, after) in plain.glyphs.iter().zip(&moved.glyphs) {
                    assert_eq!(before.id, after.id);
                    assert_eq!(before.style, after.style);
                    assert!((after.x - before.x - offset).abs() < 0.001);
                }
            }
            let image = aligned.render(text);
            assert!(image
                .pixels()
                .any(|pixel| pixel[0] == 255 && pixel[1] == 0 && pixel[3] > 0));
        }
    }

    #[test]
    fn outlined_captions_preserve_rich_fill_and_fit_their_bounds() {
        let mut format = TextFormat {
            font: epaint_default_fonts::HACK_REGULAR.to_vec(),
            size: 52.0,
            width: 420,
            color: [255, 255, 255, 255],
            outline_width: 5,
            bold: true,
            ..Default::default()
        };
        format
            .modify_style(0..4, |style| style.color = [255, 60, 60, 255])
            .unwrap();
        for alignment in [
            TextAlignment::Left,
            TextAlignment::Center,
            TextAlignment::Right,
        ] {
            format.alignment = alignment;
            let image = format.render("MEME\nCAPTION");
            let outline = format.render_outline("MEME\nCAPTION");
            assert_eq!(outline.dimensions(), image.dimensions());
            assert!(image.pixels().any(|pixel| pixel.0 == [255, 60, 60, 255]));
            assert!(image.pixels().any(|pixel| pixel.0 == [255, 255, 255, 255]));
            assert!(image.pixels().any(|pixel| pixel.0 == BLACK));
            assert!(outline
                .pixels()
                .all(|pixel| pixel[3] == 0 || pixel.0[..3] == BLACK[..3]));
            assert!(image
                .enumerate_pixels()
                .filter(|(x, y, _)| *x == 0
                    || *y == 0
                    || *x + 1 == image.width()
                    || *y + 1 == image.height())
                .all(|(_, _, pixel)| pixel[3] == 0));
            let foreground = format.render_glyphs("MEME\nCAPTION", [0, 0, 0, 0]);
            assert!(outline
                .pixels()
                .zip(foreground.pixels())
                .any(|(stroke, fill)| stroke[3] > 0 && fill[3] == 0));
            let mut composed = outline;
            crate::document::overlay(&mut composed, &foreground, 0, 0);
            assert_eq!(composed, image);
        }
    }

    #[test]
    fn round_outline_distance_and_opacity_are_bounded() {
        let mut foreground = RgbaImage::new(11, 11);
        foreground.put_pixel(5, 5, Rgba([255, 255, 255, 255]));
        let mut outline = RgbaImage::new(11, 11);
        paint_text_outline(&mut outline, &foreground, 2, [0, 0, 0, 128]);
        assert_eq!(outline.get_pixel(3, 5)[3], 128);
        assert!((1..128).contains(&outline.get_pixel(3, 3)[3]));
        assert_eq!(outline.get_pixel(2, 5)[3], 0);
        assert_eq!(outline.get_pixel(2, 2)[3], 0);
        let invalid = TextFormat {
            outline_width: MAX_TEXT_OUTLINE + 1,
            ..Default::default()
        };
        assert!(invalid.validate().is_err());
        let too_narrow = TextFormat {
            width: 10,
            outline_width: 10,
            ..Default::default()
        };
        assert!(too_narrow.validate().is_err());
    }

    #[test]
    fn minimum_text_box_height_preserves_background_and_is_bounded() {
        let mut format = TextFormat {
            minimum_height: 120,
            background: Some([30, 80, 150, 255]),
            ..Default::default()
        };
        let image = format.render("Hello");
        assert_eq!(image.dimensions(), (280, 120));
        assert_eq!(image.get_pixel(279, 119).0, [30, 80, 150, 255]);
        format.minimum_height = 20000;
        assert!(format.validate_for_text("Hello").is_err());
    }

    #[test]
    fn explicit_blank_lines_and_crlf_are_preserved() {
        let format = TextFormat::default();
        let layout = format.layout("one\n\nthree\n");
        assert_eq!(layout.lines.len(), 4);
        assert!(layout.lines[1].glyphs.is_empty());
        assert!(layout.lines[3].glyphs.is_empty());
        assert_eq!(format.render("one\r\ntwo"), format.render("one\ntwo"));
    }

    #[test]
    fn wraps_at_words_and_breaks_long_words_without_losing_characters() {
        let font = FontRef::try_from_slice(epaint_default_fonts::HACK_REGULAR).unwrap();
        let scaled = font.as_scaled(20.0);
        let width = (scaled.h_advance(scaled.glyph_id('a')) * 5.0 + 2.5).ceil() as u32;
        let format = TextFormat {
            font: epaint_default_fonts::HACK_REGULAR.to_vec(),
            width,
            size: 20.0,
            ..Default::default()
        };
        let layout = format.layout("one two abcdefghij");
        assert_eq!(
            layout
                .lines
                .iter()
                .map(|line| line.glyphs.len())
                .collect::<Vec<_>>(),
            [3, 3, 5, 5]
        );
    }

    #[test]
    fn styles_change_pixels_and_opacity_is_respected() {
        let base = TextFormat {
            width: 200,
            ..Default::default()
        };
        let plain = base.render("Paint");
        assert!(plain.pixels().any(|p| p[3] == 0));
        assert!(plain.pixels().any(|p| p[3] > 0));
        for styled in [
            TextFormat {
                bold: true,
                ..base.clone()
            },
            TextFormat {
                italic: true,
                ..base.clone()
            },
            TextFormat {
                underline: true,
                ..base.clone()
            },
            TextFormat {
                strikeout: true,
                ..base.clone()
            },
        ] {
            assert_ne!(styled.render("Paint"), plain);
        }
        let opaque = TextFormat {
            background: Some([255, 255, 255, 255]),
            ..base
        }
        .render("Paint");
        assert!(opaque.pixels().all(|p| p[3] == 255));
    }

    #[test]
    fn invalid_metrics_render_bounded_and_fail_validation() {
        let bad = TextFormat {
            size: f32::NAN,
            font: vec![1, 2, 3],
            width: u32::MAX,
            ..Default::default()
        };
        assert!(bad.validate().is_err());
        let image = bad.render("Paint");
        assert!(crate::document::valid_size(image.width(), image.height()));
    }

    #[test]
    fn selected_formatting_preserves_neighbors_and_mixed_properties() {
        let mut format = TextFormat::default();
        format
            .modify_style(2..5, |style| style.bold = true)
            .unwrap();
        format
            .modify_style(4..7, |style| style.color = [255, 0, 0, 255])
            .unwrap();
        assert!(!format.style_at(1).bold);
        assert!(format.style_at(3).bold);
        assert!(format.style_at(4).bold);
        assert_eq!(format.style_at(4).color, [255, 0, 0, 255]);
        assert!(!format.style_at(5).bold);
        assert_eq!(format.style_at(7).color, BLACK);
        format.validate_for_text("abcdefgh").unwrap();
    }

    #[test]
    fn unicode_insertions_deletions_and_replacements_keep_style_ranges() {
        let mut format = TextFormat::default();
        format
            .modify_style(1..3, |style| style.bold = true)
            .unwrap();
        format.update_spans_for_edit("aé猫z", "aéX猫z").unwrap();
        assert_eq!(format.spans[0].range, 1..4);
        format.update_spans_for_edit("aéX猫z", "a猫z").unwrap();
        assert_eq!(format.spans[0].range, 1..2);
        format.update_spans_for_edit("a猫z", "a🙂🙂z").unwrap();
        assert_eq!(format.spans[0].range, 1..3);
        format.validate_for_text("a🙂🙂z").unwrap();
    }

    #[test]
    fn cursor_aware_edits_preserve_styles_among_identical_characters() {
        let mut format = TextFormat::default();
        format
            .modify_style(1..2, |style| style.bold = true)
            .unwrap();
        let plain = format.default_style();
        format
            .update_spans_for_edit_at("aaa", "aaaa", 1..1, 2, plain.clone())
            .unwrap();
        assert!(!format.style_at(1).bold);
        assert!(format.style_at(2).bold);
        format
            .update_spans_for_edit_at("aaaa", "aaa", 1..1, 1, plain)
            .unwrap();
        assert!(format.style_at(1).bold);
    }

    #[test]
    fn font_point_controls_match_internal_pixel_sizes() {
        assert_eq!(pixels_to_points(24.0), 18.0);
        assert_eq!(points_to_pixels(18.0), 24.0);
        for points in [6.0, 11.0, 18.0, 72.0, 200.0] {
            let format = TextFormat {
                size: points_to_pixels(points),
                ..Default::default()
            };
            format.validate().unwrap();
        }
    }

    #[test]
    fn rich_text_rendering_aligns_sizes_and_uses_selected_font_and_color() {
        let mut format = TextFormat {
            width: 300,
            ..Default::default()
        };
        format
            .modify_style(2..4, |style| {
                style.size = 48.0;
                style.color = [255, 0, 0, 255];
                style.font = epaint_default_fonts::HACK_REGULAR.to_vec();
                style.font_name = "Monospace".into();
                style.underline = true;
            })
            .unwrap();
        let layout = format.layout("abCD");
        assert_eq!(layout.lines.len(), 1);
        assert!(layout.lines[0].height >= 48.0);
        let image = format.render("abCD");
        assert!(image
            .pixels()
            .any(|pixel| pixel[0] > 0 && pixel[1] == 0 && pixel[3] > 0));
        assert!(image.pixels().any(|pixel| pixel[0] == 0 && pixel[3] > 0));
        assert_ne!(
            image,
            TextFormat {
                width: 300,
                ..Default::default()
            }
            .render("abCD")
        );
    }

    #[test]
    fn old_serialized_formats_default_to_no_spans_and_bad_ranges_are_rejected() {
        let mut value = serde_json::to_value(TextFormat::default()).unwrap();
        value.as_object_mut().unwrap().remove("spans");
        value.as_object_mut().unwrap().remove("minimum_height");
        value.as_object_mut().unwrap().remove("font_index");
        value.as_object_mut().unwrap().remove("alignment");
        value.as_object_mut().unwrap().remove("outline_width");
        value.as_object_mut().unwrap().remove("outline_color");
        let mut format: TextFormat = serde_json::from_value(value).unwrap();
        assert!(format.spans.is_empty());
        assert_eq!(format.minimum_height, 0);
        assert_eq!(format.font_index, 0);
        assert_eq!(format.alignment, TextAlignment::Left);
        assert_eq!(format.outline_width, 0);
        assert_eq!(format.outline_color, BLACK);
        let mut old_style = serde_json::to_value(format.default_style()).unwrap();
        old_style.as_object_mut().unwrap().remove("font_index");
        let old_style: TextStyle = serde_json::from_value(old_style).unwrap();
        assert_eq!(old_style.font_index, 0);
        let mut invalid_face = format.default_style();
        invalid_face.font_index = 1;
        assert!(invalid_face.validate().is_err());
        format
            .apply_style(
                0..20,
                TextStyle {
                    bold: true,
                    ..format.default_style()
                },
            )
            .unwrap();
        assert!(format.validate_for_text("short").is_err());
    }
}
