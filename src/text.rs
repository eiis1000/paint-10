use crate::document::{blend, Color, BLACK, MAX_PIXELS};
use ab_glyph::{point, Font, FontRef, GlyphId, ScaleFont};
use image::{Rgba, RgbaImage};
use std::ops::Range;

const MAX_TEXT_CHARS: usize = 1024 * 1024;
const MAX_SPANS: usize = 4096;
const MAX_FONT_BYTES: usize = 32 * 1024 * 1024;
const MAX_FORMAT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_TEXT_WIDTH: u32 = 16384;
pub const FONT_POINT_RANGE: std::ops::RangeInclusive<f32> = 6.0..=200.0;

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

#[derive(Clone, Copy, Debug)]
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
    pub spans: Vec<TextSpan>,
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
            spans: vec![],
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
        validate_style(self.default_style_ref())?;
        if self.spans.len() > MAX_SPANS || self.memory_bytes() > MAX_FORMAT_BYTES {
            return Err("The text formatting exceeds the project limit.".into());
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

    fn layout(&self, text: &str) -> TextLayout<'_> {
        let mut styles = vec![RenderStyle::new(self.default_style_ref())];
        styles.extend(
            self.spans
                .iter()
                .take(MAX_SPANS)
                .map(|span| RenderStyle::new(span.style.as_ref())),
        );
        let mut chars = vec![];
        let mut span_index = 0;
        let mut source = text.chars().take(MAX_TEXT_CHARS).enumerate().peekable();
        while let Some((index, character)) = source.next() {
            while span_index < self.spans.len() && self.spans[span_index].range.end <= index {
                span_index += 1;
            }
            let style = if self
                .spans
                .get(span_index)
                .is_some_and(|span| span.range.contains(&index))
                && span_index + 1 < styles.len()
            {
                span_index + 1
            } else {
                0
            };
            if character == '\r' && source.peek().is_some_and(|(_, next)| *next == '\n') {
                continue;
            }
            if character == '\t' {
                chars.extend((0..4).map(|_| StyledChar {
                    character: ' ',
                    style,
                }));
            } else {
                chars.push(StyledChar { character, style });
            }
        }
        let width = self.width.clamp(10, MAX_TEXT_WIDTH);
        let max_height = (MAX_PIXELS / width as u64).min(16384) as u32;
        let mut lines = vec![];
        let mut top = 0.0;
        let mut start = 0;
        while start <= chars.len() && top + 4.0 < max_height as f32 {
            let paragraph_end = chars[start..]
                .iter()
                .position(|c| c.character == '\n')
                .map_or(chars.len(), |offset| start + offset);
            let blank_style = chars.get(start).map_or(0, |c| c.style);
            if start == paragraph_end {
                let line = layout_line(&[], &styles, blank_style, top);
                top += line.height;
                lines.push(line);
            } else {
                while start < paragraph_end && top + 4.0 < max_height as f32 {
                    let end = wrap_end(&chars, start, paragraph_end, &styles, width as f32 - 2.0);
                    let mut visible_end = end;
                    if end < paragraph_end {
                        while visible_end > start
                            && chars[visible_end - 1].character.is_whitespace()
                        {
                            visible_end -= 1;
                        }
                    }
                    let line = layout_line(&chars[start..visible_end], &styles, blank_style, top);
                    top += line.height;
                    lines.push(line);
                    start = end;
                    if start < paragraph_end {
                        while start < paragraph_end && chars[start].character.is_whitespace() {
                            start += 1;
                        }
                    }
                }
            }
            if paragraph_end == chars.len() {
                break;
            }
            start = paragraph_end + 1;
        }
        TextLayout {
            styles,
            lines,
            width,
            height: (top + 4.0)
                .ceil()
                .max(self.minimum_height as f32)
                .max(1.0)
                .min(max_height as f32) as u32,
        }
    }

    pub fn render(&self, text: &str) -> RgbaImage {
        let layout = self.layout(text);
        let mut image = RgbaImage::from_pixel(
            layout.width,
            layout.height,
            Rgba(self.background.unwrap_or([0, 0, 0, 0])),
        );
        for line in &layout.lines {
            for glyph in &line.glyphs {
                let style = &layout.styles[glyph.style];
                if let Some(outline) = style.font.outline_glyph(
                    glyph
                        .id
                        .with_scale_and_position(style.size, point(glyph.x, line.baseline)),
                ) {
                    let bounds = outline.px_bounds();
                    outline.draw(|x, y, coverage| {
                        let y = y as i32 + bounds.min.y as i32;
                        let shear = if style.source.italic {
                            ((line.baseline - y as f32) * 0.2).round() as i32
                        } else {
                            0
                        };
                        let x = x as i32 + bounds.min.x as i32 + shear;
                        let mut color = style.source.color;
                        color[3] = (coverage * color[3] as f32) as u8;
                        blend(&mut image, x, y, color);
                        if style.source.bold {
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
}

impl<'a> RenderStyle<'a> {
    fn new(source: TextStyleRef<'a>) -> Self {
        let fallback = || FontRef::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT).unwrap();
        let mut font =
            FontRef::try_from_slice_and_index(font_bytes(source.font), source.font_index)
                .unwrap_or_else(|_| fallback());
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
        }
    }

    fn overhang(&self) -> f32 {
        (if self.source.italic {
            self.size * 0.2
        } else {
            0.0
        }) + if self.source.bold { 1.0 } else { 0.0 }
    }
}

#[derive(Clone, Copy)]
struct StyledChar {
    character: char,
    style: usize,
}

struct PositionedGlyph {
    id: GlyphId,
    style: usize,
    x: f32,
    advance: f32,
}

struct TextLine {
    glyphs: Vec<PositionedGlyph>,
    baseline: f32,
    height: f32,
}

struct TextLayout<'a> {
    styles: Vec<RenderStyle<'a>>,
    lines: Vec<TextLine>,
    width: u32,
    height: u32,
}

fn advance(
    character: StyledChar,
    previous: Option<(GlyphId, usize)>,
    styles: &[RenderStyle<'_>],
) -> (GlyphId, f32, f32) {
    let style = &styles[character.style];
    let scaled = style.font.as_scaled(style.size);
    let id = scaled.glyph_id(character.character);
    let kern = previous.map_or(0.0, |(previous, previous_style)| {
        let prior = &styles[previous_style];
        if prior.size == style.size
            && (previous_style == character.style || prior.source.font == style.source.font)
        {
            scaled.kern(previous, id)
        } else {
            0.0
        }
    });
    (id, scaled.h_advance(id), kern)
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
                advance: step,
            };
            x += step;
            previous = Some((id, character.style));
            glyph
        })
        .collect();
    TextLine {
        glyphs,
        baseline: top + ascent,
        height: ascent + descent + gap,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut format: TextFormat = serde_json::from_value(value).unwrap();
        assert!(format.spans.is_empty());
        assert_eq!(format.minimum_height, 0);
        assert_eq!(format.font_index, 0);
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
