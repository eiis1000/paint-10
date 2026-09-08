use super::EditorLayout;
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorCaret {
    pub index: usize,
    pub x: f32,
    pub y: f32,
    pub height: f32,
    pub row: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct EditorSelectionRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl EditorLayout {
    pub(super) fn populate_carets(&mut self, text: &str) {
        let mut boundaries = vec![0];
        let mut count = 0;
        for grapheme in text.graphemes(true) {
            count += grapheme.chars().count();
            boundaries.push(count);
        }
        for (row_index, row) in self.rows.iter_mut().enumerate() {
            for index in row
                .source_range
                .clone()
                .chain(std::iter::once(row.source_range.end))
            {
                if boundaries.binary_search(&index).is_err() {
                    continue;
                }
                let x = row.glyphs.get(index - row.source_range.start).map_or_else(
                    || {
                        row.glyphs
                            .last()
                            .map_or(row.left, |glyph| glyph.x + glyph.advance)
                    },
                    |glyph| glyph.x,
                );
                row.visual_carets.push(EditorCaret {
                    index,
                    x,
                    y: row.top,
                    height: row.height,
                    row: row_index,
                });
            }
            row.visual_carets
                .sort_by(|a, b| a.x.total_cmp(&b.x).then(a.index.cmp(&b.index)));
        }
    }

    /// Logical indices always remain Unicode scalar offsets. At a directional
    /// boundary the following character supplies the primary caret position.
    pub fn caret(&self, index: usize) -> EditorCaret {
        self.rows
            .iter()
            .rev()
            .flat_map(|row| &row.visual_carets)
            .min_by_key(|caret| caret.index.abs_diff(index))
            .copied()
            .unwrap_or(EditorCaret {
                index: 0,
                x: 0.0,
                y: 0.0,
                height: self.height,
                row: 0,
            })
    }

    pub fn hit_test(&self, x: f32, y: f32) -> usize {
        let row = self.rows.iter().min_by(|a, b| {
            let distance = |top: f32, height: f32| (top - y).max(0.0) + (y - top - height).max(0.0);
            distance(a.top, a.height).total_cmp(&distance(b.top, b.height))
        });
        row.and_then(|row| {
            row.visual_carets
                .iter()
                .min_by(|a, b| (a.x - x).abs().total_cmp(&(b.x - x).abs()))
        })
        .map_or(0, |caret| caret.index)
    }

    pub fn horizontal_cursor(&self, index: usize, right: bool) -> usize {
        let caret = self.caret(index);
        let Some(row) = self.rows.get(caret.row) else {
            return index;
        };
        let current = row
            .visual_carets
            .iter()
            .position(|stop| stop.index == caret.index)
            .unwrap_or(0);
        let adjacent = if right {
            current.checked_add(1)
        } else {
            current.checked_sub(1)
        };
        if let Some(next) = adjacent.and_then(|next| row.visual_carets.get(next)) {
            return next.index;
        }
        let forward = right != row.right_to_left;
        let adjacent_row = if forward {
            caret.row.checked_add(1)
        } else {
            caret.row.checked_sub(1)
        };
        adjacent_row
            .and_then(|index| self.rows.get(index))
            .and_then(|row| {
                if right {
                    row.visual_carets
                        .iter()
                        .find(|stop| stop.index != caret.index)
                } else {
                    row.visual_carets
                        .iter()
                        .rev()
                        .find(|stop| stop.index != caret.index)
                }
            })
            .map_or(caret.index, |stop| stop.index)
    }

    pub fn vertical_cursor(&self, index: usize, row_delta: isize) -> usize {
        let caret = self.caret(index);
        let row = caret
            .row
            .saturating_add_signed(row_delta)
            .min(self.rows.len().saturating_sub(1));
        self.rows.get(row).map_or(index, |row| {
            self.hit_test(caret.x, row.top + row.height / 2.0)
        })
    }

    /// A logical selection can occupy several disjoint visual intervals in a
    /// mixed-direction line. Never draw one rectangle between its endpoints.
    pub fn selection_rects(&self, selection: Range<usize>) -> Vec<EditorSelectionRect> {
        let mut rectangles = Vec::new();
        for row in &self.rows {
            let mut intervals: Vec<_> = row
                .glyphs
                .iter()
                .filter(|glyph| selection.contains(&glyph.character_index) && glyph.advance != 0.0)
                .map(|glyph| {
                    (
                        glyph.x.min(glyph.x + glyph.advance),
                        glyph.x.max(glyph.x + glyph.advance),
                    )
                })
                .collect();
            if row.ends_with_newline && selection.contains(&row.source_range.end) {
                let edge = self.caret(row.source_range.end).x;
                intervals.push((edge, edge + row.height * 0.25));
            }
            intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut merged: Vec<(f32, f32)> = Vec::new();
            for (left, right) in intervals {
                if let Some(previous) = merged.last_mut() {
                    if left <= previous.1 + 0.5 {
                        previous.1 = previous.1.max(right);
                        continue;
                    }
                }
                merged.push((left, right));
            }
            rectangles.extend(merged.into_iter().map(|(left, right)| EditorSelectionRect {
                x: left,
                y: row.top,
                width: right - left,
                height: row.height,
            }));
        }
        rectangles
    }
}
