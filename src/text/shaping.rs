//! OpenType shaping and Unicode bidi resolution, sharing ab_glyph's pixel scale.
//! The old simple-script path remains intact so existing Latin artwork retains
//! its exact kerning, glyph coverage and wrapping.

use super::*;
use unicode_bidi::{BidiClass, BidiInfo, Level, ParagraphBidiInfo};
use unicode_script::{Script, UnicodeScript};
use unicode_segmentation::UnicodeSegmentation;

pub(super) fn default_ignorable(character: char) -> bool {
    matches!(character as u32,
        0x00ad | 0x034f | 0x061c | 0x115f..=0x1160 | 0x17b4..=0x17b5 |
        0x180b..=0x180f | 0x200b..=0x200f | 0x202a..=0x202e | 0x2060..=0x206f |
        0x3164 | 0xfe00..=0xfe0f | 0xfeff | 0xffa0 | 0xfff0..=0xfff8 |
        0x1bca0..=0x1bca3 | 0x1d173..=0x1d17a | 0xe0000..=0xe0fff)
}

pub(super) fn required_chars(chars: &[StyledChar]) -> bool {
    chars.iter().any(|character| required(character.character))
}

pub(super) fn required(character: char) -> bool {
    default_ignorable(character)
        || !matches!(
            character.script(),
            Script::Latin | Script::Common | Script::Greek | Script::Cyrillic | Script::Unknown
        )
}

/// A combining sequence must use one covering font when one is available.
/// Keep the original style metadata; only the ephemeral render face changes.
pub(super) fn unify_grapheme_fonts<'a>(
    chars: &mut [StyledChar],
    styles: &mut Vec<RenderStyle<'a>>,
    available: &[(usize, FontRef<'a>)],
    faces: &'a [EmbeddedFont],
) {
    if !required_chars(chars) {
        return;
    }
    let text: String = chars.iter().map(|character| character.character).collect();
    let mut start = 0;
    let mut cache = std::collections::HashMap::new();
    for grapheme in text.graphemes(true) {
        let end = start + grapheme.chars().count();
        let cluster = &mut chars[start..end];
        if cluster.len() > 1 {
            let base = cluster[0].style;
            let covered = |font: &FontRef<'_>| {
                cluster.iter().all(|character| {
                    default_ignorable(character.character)
                        || character.character.is_control()
                        || font_supports_outline(font, character.character)
                })
            };
            let target = if covered(&styles[base].font) {
                Some((base, None))
            } else {
                available
                    .iter()
                    .find(|(_, font)| covered(font))
                    .map(|(face, _)| (base, Some(*face)))
            };
            if let Some((base, face)) = target {
                for character in cluster {
                    let source_style = character.style;
                    // Different paint properties may split a shape run, but
                    // contextual joining still sees neighboring characters.
                    let target = if let Some(face) = face {
                        let source_style = if styles[source_style].source == styles[base].source {
                            base
                        } else {
                            source_style
                        };
                        *cache.entry((source_style, face)).or_insert_with(|| {
                            let index = styles.len();
                            styles.push(RenderStyle::from_face(
                                styles[source_style].source,
                                &faces[face],
                            ));
                            index
                        })
                    } else if styles[source_style].source == styles[base].source {
                        base
                    } else {
                        source_style
                    };
                    character.style = target;
                }
            }
        }
        start = end;
    }
}

pub(super) struct Paragraph<'a> {
    chars: &'a [StyledChar],
    text: String,
    offsets: Vec<usize>,
    levels: Vec<Level>,
    classes: Vec<BidiClass>,
    base_level: Level,
    scripts: Vec<Script>,
    widths: Vec<f32>,
    boundaries: Vec<bool>,
    wrap_boundaries: Vec<bool>,
}

impl<'a> Paragraph<'a> {
    pub(super) fn new(chars: &'a [StyledChar], styles: &[RenderStyle<'_>]) -> Self {
        let text: String = chars.iter().map(|character| character.character).collect();
        let offsets: Vec<_> = text
            .char_indices()
            .map(|(offset, _)| offset)
            .chain(std::iter::once(text.len()))
            .collect();
        let bidi = BidiInfo::new(&text, None);
        let base_level = bidi
            .paragraphs
            .first()
            .map_or(Level::ltr(), |paragraph| paragraph.level);
        let levels = offsets[..chars.len()]
            .iter()
            .map(|&offset| bidi.levels[offset])
            .collect();
        let classes = offsets[..chars.len()]
            .iter()
            .map(|&offset| bidi.original_classes[offset])
            .collect();
        let mut scripts: Vec<_> = chars
            .iter()
            .map(|character| character.character.script())
            .collect();
        let mut previous = Script::Common;
        for script in &mut scripts {
            if matches!(script, Script::Inherited | Script::Common | Script::Unknown) {
                *script = previous;
            } else {
                previous = *script;
            }
        }
        let mut next = Script::Latin;
        for script in scripts.iter_mut().rev() {
            if *script == Script::Common {
                *script = next;
            } else {
                next = *script;
            }
        }
        let mut boundaries = vec![false; chars.len() + 1];
        boundaries[0] = true;
        let mut end = 0;
        for grapheme in text.graphemes(true) {
            end += grapheme.chars().count();
            boundaries[end] = true;
        }
        let mut paragraph = Self {
            chars,
            text,
            offsets,
            levels,
            classes,
            base_level,
            scripts,
            widths: vec![0.0; chars.len()],
            wrap_boundaries: boundaries.clone(),
            boundaries,
        };
        let line = paragraph.line(0..chars.len(), styles, 0, 0.0);
        let mut previous_cluster = None;
        for cell in line.cells {
            if let Ok(index) =
                chars.binary_search_by_key(&cell.source_index, |character| character.source_index)
            {
                paragraph.widths[index] += cell.advance.abs();
            }
            if previous_cluster.as_ref() != Some(&cell.wrap_cluster) {
                let start = chars
                    .partition_point(|character| character.source_index < cell.wrap_cluster.start);
                let end = chars
                    .partition_point(|character| character.source_index < cell.wrap_cluster.end);
                paragraph.wrap_boundaries[start + 1..end].fill(false);
                previous_cluster = Some(cell.wrap_cluster);
            }
        }
        paragraph
    }

    pub(super) fn wrap_end(&self, start: usize, width: f32, styles: &[RenderStyle<'_>]) -> usize {
        let mut end = start;
        let mut x = 0.0;
        let mut last_break = None;
        while end < self.chars.len() {
            let next = (end + 1..=self.chars.len())
                .find(|&index| self.wrap_boundaries[index])
                .unwrap();
            let step: f32 = self.widths[end..next].iter().sum();
            if x + step > width && end > start {
                break;
            }
            x += step;
            end = next;
            if self.chars[end - 1].character.is_whitespace() {
                last_break = Some(end);
            }
        }
        let end = if end < self.chars.len() {
            last_break.unwrap_or(end)
        } else {
            end
        };
        // Joining changes at a line boundary. Recheck the actual line shape,
        // then shrink at whole clusters using a bounded binary search.
        if self.line(start..end, styles, 0, 0.0).width <= width {
            return end;
        }
        let candidates: Vec<_> = (start + 1..=end)
            .filter(|&index| self.wrap_boundaries[index])
            .collect();
        let mut low = 0;
        let mut high = candidates.len();
        while low + 1 < high {
            let middle = (low + high) / 2;
            if self.line(start..candidates[middle], styles, 0, 0.0).width <= width {
                low = middle;
            } else {
                high = middle;
            }
        }
        candidates[low]
    }

    pub(super) fn line(
        &self,
        range: Range<usize>,
        styles: &[RenderStyle<'_>],
        blank_style: usize,
        top: f32,
    ) -> TextLine {
        let mut line = layout_line(&[], styles, blank_style, top);
        line.right_to_left = self.base_level.is_rtl();
        let chars = &self.chars[range.clone()];
        if chars.is_empty() {
            return line;
        }
        // Retain paragraph resolution across wrapping, then ask unicode-bidi
        // to apply exact line rules to just this slice. This avoids re-resolving
        // a wrapped RTL continuation or cloning the entire paragraph per row.
        let text = &self.text[self.offsets[range.start]..self.offsets[range.end]];
        let bidi = ParagraphBidiInfo {
            text,
            original_classes: chars
                .iter()
                .zip(&self.classes[range.clone()])
                .flat_map(|(character, class)| {
                    std::iter::repeat_n(*class, character.character.len_utf8())
                })
                .collect(),
            levels: chars
                .iter()
                .zip(&self.levels[range.clone()])
                .flat_map(|(character, level)| {
                    std::iter::repeat_n(*level, character.character.len_utf8())
                })
                .collect(),
            paragraph_level: self.base_level,
            is_pure_ltr: false,
        };
        let levels = bidi.reordered_levels_per_char(0..text.len());
        let mut runs: Vec<Range<usize>> = Vec::new();
        let mut start = range.start;
        for index in range.start + 1..range.end {
            if levels[index - range.start] != levels[start - range.start]
                || self.scripts[index] != self.scripts[start]
                || self.chars[index].style != self.chars[start].style
            {
                runs.push(start..index);
                start = index;
            }
        }
        runs.push(start..range.end);
        let run_levels: Vec<_> = runs
            .iter()
            .map(|run| levels[run.start - range.start])
            .collect();
        let mut x = 1.0;
        for run_index in BidiInfo::reorder_visual(&run_levels) {
            let run = runs[run_index].clone();
            let rtl = run_levels[run_index].is_rtl();
            let mut shaped = self.shape_run(run.clone(), styles, rtl, range.clone());
            for glyph in &mut shaped.glyphs {
                glyph.x += x;
            }
            for cell in &mut shaped.cells {
                cell.x += x;
            }
            x += shaped.advance;
            line.glyphs.extend(shaped.glyphs);
            line.cells.extend(shaped.cells);
        }
        line.cells.sort_by_key(|cell| cell.source_index);
        let ascent = chars
            .iter()
            .map(|character| styles[character.style].ascent)
            .fold(0.0_f32, f32::max);
        let descent = chars
            .iter()
            .map(|character| styles[character.style].descent)
            .fold(0.0_f32, f32::max);
        let gap = chars
            .iter()
            .map(|character| styles[character.style].gap)
            .fold(0.0_f32, f32::max);
        line.baseline = top + ascent;
        line.height = ascent + descent + gap;
        line.advance = x - 1.0;
        line.width = line.advance
            + chars
                .iter()
                .map(|character| styles[character.style].overhang())
                .fold(0.0_f32, f32::max);
        line
    }

    fn shape_run(
        &self,
        range: Range<usize>,
        styles: &[RenderStyle<'_>],
        rtl: bool,
        line_range: Range<usize>,
    ) -> TextLine {
        let chars = &self.chars[range.clone()];
        if !rtl && !required_chars(chars) {
            let mut line = layout_line(chars, styles, chars[0].style, 0.0);
            for glyph in &mut line.glyphs {
                glyph.x -= 1.0;
            }
            for cell in &mut line.cells {
                cell.x -= 1.0;
            }
            return line;
        }
        let style = &styles[chars[0].style];
        let face = rustybuzz::Face::from_slice(style.font.font_data(), style.font_index)
            .expect("validated outline font is a valid shaping face");
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        for (index, character) in chars.iter().enumerate() {
            buffer.add(render_character(character.character, style), index as u32);
        }
        buffer
            .set_pre_context(&self.text[self.offsets[line_range.start]..self.offsets[range.start]]);
        buffer.set_post_context(&self.text[self.offsets[range.end]..self.offsets[line_range.end]]);
        buffer.set_direction(if rtl {
            rustybuzz::Direction::RightToLeft
        } else {
            rustybuzz::Direction::LeftToRight
        });
        if let Ok(script) = self.scripts[range.start].short_name().parse() {
            buffer.set_script(script);
        }
        buffer.guess_segment_properties();
        // Required ligatures (Arabic/Indic) remain enabled. Discretionary Latin
        // typography is deliberately not a side effect of this correctness fix.
        let features = if matches!(
            self.scripts[range.start],
            Script::Latin | Script::Greek | Script::Cyrillic
        ) {
            vec!["liga=0".parse().unwrap(), "clig=0".parse().unwrap()]
        } else {
            Vec::new()
        };
        let shaped = rustybuzz::shape(&face, &features, buffer);
        let scale = style.font.as_scaled(style.size).h_scale_factor();
        let mut line = layout_line(&[], styles, chars[0].style, 0.0);
        let mut clusters: Vec<_> = shaped
            .glyph_infos()
            .iter()
            .map(|info| info.cluster as usize)
            .collect();
        clusters.push(chars.len());
        clusters.sort_unstable();
        clusters.dedup();
        let mut x = 0.0;
        let mut index = 0;
        while index < shaped.len() {
            let cluster = shaped.glyph_infos()[index].cluster as usize;
            let end = clusters[clusters.binary_search(&cluster).unwrap() + 1];
            let invisible = chars[cluster..end]
                .iter()
                .all(|character| default_ignorable(character.character));
            let start_x = x;
            while index < shaped.len() && shaped.glyph_infos()[index].cluster as usize == cluster {
                let info = &shaped.glyph_infos()[index];
                let position = &shaped.glyph_positions()[index];
                let advance = if invisible {
                    0.0
                } else {
                    position.x_advance as f32 * scale
                };
                if !invisible {
                    line.glyphs.push(PositionedGlyph {
                        id: GlyphId(info.glyph_id as u16),
                        style: chars[0].style,
                        x: x + position.x_offset as f32 * scale,
                        y: -position.y_offset as f32 * scale,
                        advance,
                        source_index: chars[cluster].source_index,
                    });
                }
                x += advance;
                index += 1;
            }
            let wrap_cluster = chars[cluster].source_index..chars[end - 1].source_index + 1;
            let starts: Vec<_> = (cluster..end)
                .filter(|offset| *offset == cluster || self.boundaries[range.start + offset])
                .collect();
            let step = (x - start_x) / starts.len().max(1) as f32;
            for (grapheme, &start) in starts.iter().enumerate() {
                let end = starts.get(grapheme + 1).copied().unwrap_or(end);
                let leading = if rtl {
                    x - step * grapheme as f32
                } else {
                    start_x + step * grapheme as f32
                };
                let advance = if rtl { -step } else { step };
                for (offset, character) in chars.iter().enumerate().take(end).skip(start) {
                    line.cells.push(CharacterCell {
                        source_index: character.source_index,
                        style: character.style,
                        x: if offset == start {
                            leading
                        } else {
                            leading + advance
                        },
                        advance: if offset == start { advance } else { 0.0 },
                        wrap_cluster: wrap_cluster.clone(),
                    });
                }
            }
        }
        line.advance = x;
        line.width = x + style.overhang();
        line
    }
}
