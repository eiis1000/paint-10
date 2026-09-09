//! Unicode editing boundaries and ordered text, history and IME input.

use super::*;
use unicode_segmentation::UnicodeSegmentation;

impl PaintApp {
    pub(in crate::app) fn text_raw_input(&self, ctx: &Context, input: &mut RawInput) {
        let pending = Id::new("paint10_text_ordered_input");
        let mut events = ctx
            .data_mut(|data| data.remove_temp::<Vec<Event>>(pending))
            .unwrap_or_default();
        if self.text_edit.is_none() {
            events.retain(|event| !matches!(event, Event::Ime(_)));
            events.append(&mut input.events);
            input.events = events;
            return;
        }
        events.append(&mut input.events);
        let mut boundary = events
            .iter()
            .position(|event| matches!(event, Event::Ime(ImeEvent::Commit(_) | ImeEvent::Disabled)))
            .map(|index| {
                let mut end = index + 1;
                if matches!(events[index], Event::Ime(ImeEvent::Commit(_)))
                    && matches!(events.get(end), Some(Event::Ime(ImeEvent::Disabled)))
                {
                    end += 1;
                }
                end
            })
            .unwrap_or(events.len());
        if let Some(index) = events.iter().position(is_history_shortcut) {
            // The ribbon/shortcut pass runs before TextEdit. Finish preceding
            // input first, then give each Undo/Redo its own pass so commands
            // cannot overtake typing or consume each other in the same frame.
            boundary = boundary.min(if index == 0 { 1 } else { index });
        }
        if self
            .text_edit
            .as_ref()
            .is_some_and(|state| state.history.composition.is_none())
        {
            if let Some(start) = events.iter().position(|event| {
                matches!(event, Event::Ime(ImeEvent::Preedit(text) | ImeEvent::Commit(text)) if !text.is_empty())
            }) {
                let preceding_input = events[..start].iter().any(|event| {
                    matches!(event,
                        Event::Text(_) | Event::Paste(_) | Event::Cut
                        | Event::Key { pressed: true, .. }
                        | Event::PointerButton { .. } | Event::PointerMoved(_))
                });
                if preceding_input {
                    // Capture the composition only after earlier confirmed
                    // edits and selection changes have reached TextEdit.
                    boundary = boundary.min(start);
                }
            }
        }
        let deferred = events.split_off(boundary);
        input.events = events;
        if !deferred.is_empty() {
            // Confirmed text must reach TextEdit before a subsequent history
            // command or composition observes the state.
            ctx.data_mut(|data| data.insert_temp(pending, deferred));
            ctx.request_repaint();
        }
    }
}

fn is_history_shortcut(event: &Event) -> bool {
    let Event::Key {
        key,
        modifiers,
        pressed: true,
        ..
    } = event
    else {
        return false;
    };
    let matches =
        |expected| super::super::shortcuts::shortcut_modifiers_match(*modifiers, expected);
    matches!(key, Key::Z | Key::Y) && matches(Modifiers::CTRL)
        || *key == Key::Z && matches(Modifiers::CTRL | Modifiers::SHIFT)
}

/// The document stores scalar indices, while editing stops at complete user
/// characters. Keeping this conversion here also preserves existing projects.
fn grapheme_boundaries(text: &str) -> Vec<usize> {
    let mut boundaries = vec![0];
    let mut index = 0;
    for grapheme in text.graphemes(true) {
        index += grapheme.chars().count();
        boundaries.push(index);
    }
    boundaries
}

fn adjacent_grapheme(text: &str, index: usize, forward: bool) -> usize {
    let boundaries = grapheme_boundaries(text);
    if forward {
        boundaries
            .iter()
            .copied()
            .find(|boundary| *boundary > index)
            .unwrap_or_else(|| *boundaries.last().unwrap())
    } else {
        boundaries
            .iter()
            .rev()
            .copied()
            .find(|boundary| *boundary < index)
            .unwrap_or(0)
    }
}

fn word_boundary(text: &str, index: usize, forward: bool) -> usize {
    let mut byte = 0;
    let mut character = 0;
    let mut previous_start = 0;
    for (start, word) in text.unicode_word_indices() {
        character += text[byte..start].chars().count();
        let end = character + word.chars().count();
        if forward && end > index {
            return end;
        }
        if !forward && character >= index {
            return previous_start;
        }
        previous_start = character;
        character = end;
        byte = start + word.len();
    }
    if forward {
        text.chars().count()
    } else {
        previous_start
    }
}

pub(in crate::app) fn word_selection(text: &str, index: usize) -> std::ops::Range<usize> {
    let index = index.min(text.chars().count().saturating_sub(1));
    let mut start = 0;
    for segment in text.split_word_bounds() {
        let end = start + segment.chars().count();
        if index < end {
            return start..end;
        }
        start = end;
    }
    start..start
}

pub(super) struct GraphemeBuffer<'a> {
    pub(super) text: &'a mut String,
    pub(super) composing: bool,
}

impl egui::TextBuffer for GraphemeBuffer<'_> {
    fn is_mutable(&self) -> bool {
        true
    }

    fn as_str(&self) -> &str {
        self.text.as_str()
    }

    fn insert_text(&mut self, text: &str, index: usize) -> usize {
        egui::TextBuffer::insert_text(self.text, text, index)
    }

    fn delete_char_range(&mut self, range: std::ops::Range<usize>) {
        egui::TextBuffer::delete_char_range(self.text, range);
    }

    fn delete_selected_ccursor_range(&mut self, [mut start, mut end]: [CCursor; 2]) -> CCursor {
        if !self.composing && start.index != end.index {
            let boundaries = grapheme_boundaries(self.text);
            if let Err(next) = boundaries.binary_search(&start.index) {
                start.index = boundaries[next.saturating_sub(1)];
            }
            if let Err(next) = boundaries.binary_search(&end.index) {
                end.index = boundaries
                    .get(next)
                    .copied()
                    .unwrap_or_else(|| *boundaries.last().unwrap());
            }
        }
        self.delete_char_range(start.index..end.index);
        start.prefer_next_row = true;
        start
    }

    fn delete_previous_char(&mut self, cursor: CCursor) -> CCursor {
        let start = adjacent_grapheme(self.text, cursor.index, false);
        self.delete_char_range(start..cursor.index);
        CCursor::new(start)
    }

    fn delete_next_char(&mut self, cursor: CCursor) -> CCursor {
        let end = adjacent_grapheme(self.text, cursor.index, true);
        self.delete_char_range(cursor.index..end);
        cursor
    }

    fn delete_previous_word(&mut self, cursor: CCursor) -> CCursor {
        if self.text.is_ascii() {
            return egui::TextBuffer::delete_previous_word(self.text, cursor);
        }
        let start = word_boundary(self.text, cursor.index, false);
        self.delete_selected_ccursor_range([CCursor::new(start), cursor])
    }

    fn delete_next_word(&mut self, cursor: CCursor) -> CCursor {
        if self.text.is_ascii() {
            return egui::TextBuffer::delete_next_word(self.text, cursor);
        }
        let end = word_boundary(self.text, cursor.index, true);
        self.delete_selected_ccursor_range([cursor, CCursor::new(end)])
    }
}

pub(super) fn snap_editor_selection(
    text: &str,
    output: &mut egui::text_edit::TextEditOutput,
    ctx: &Context,
    id: Id,
) {
    let Some(mut range) = output.cursor_range else {
        return;
    };
    let boundaries = grapheme_boundaries(text);
    let snap = |index: usize, upward: Option<bool>| match boundaries.binary_search(&index) {
        Ok(_) => index,
        Err(next) => {
            let lower = boundaries[next.saturating_sub(1)];
            let upper = boundaries.get(next).copied().unwrap_or(lower);
            if upward.unwrap_or(upper.saturating_sub(index) <= index.saturating_sub(lower)) {
                upper
            } else {
                lower
            }
        }
    };
    let mut primary = range.primary.ccursor;
    let mut secondary = range.secondary.ccursor;
    if range.is_empty() {
        primary.index = snap(primary.index, None);
        secondary.index = primary.index;
    } else {
        let backward = primary.index < secondary.index;
        primary.index = snap(primary.index, Some(!backward));
        secondary.index = snap(secondary.index, Some(backward));
    }
    if primary != range.primary.ccursor || secondary != range.secondary.ccursor {
        range.primary = output.galley.from_ccursor(primary);
        range.secondary = output.galley.from_ccursor(secondary);
        output.state.cursor.set_range(Some(range));
        output.state.clone().store(ctx, id);
        output.cursor_range = Some(range);
    }
}

/// Returns true when this frame delivers the final composition text. Preedit
/// drafts remain outside typing history until that explicit event arrives.
pub(super) fn update_composition(state: &mut TextEditState, ctx: &Context) -> bool {
    ctx.input_mut(|input| {
        input.events.retain(|event| {
            // On macOS Enabled can mean that an input method is available,
            // rather than that a composition has begun. egui also suppresses
            // ordinary Backspace/arrows after Enabled, so deliver it only
            // immediately before a real preedit or direct commit below.
            !matches!(event, Event::Ime(ImeEvent::Enabled))
                && !(state.history.composition.is_none()
                    && matches!(event, Event::Ime(ImeEvent::Preedit(text) | ImeEvent::Commit(text)) if text.is_empty()))
        });
    });
    let events = ctx.input(|input| input.events.clone());
    let starts = events.iter().any(|event| {
        matches!(event, Event::Ime(ImeEvent::Preedit(text) | ImeEvent::Commit(text)) if !text.is_empty())
    });
    if starts && state.history.composition.is_none() {
        state.history.composition = Some(TextSnapshot::capture(state));
        state.history.last_typing = None;
        ctx.input_mut(|input| {
            let index = input
                .events
                .iter()
                .position(|event| {
                    matches!(
                        event,
                        Event::Ime(ImeEvent::Preedit(_) | ImeEvent::Commit(_))
                    )
                })
                .unwrap();
            input.events.insert(index, Event::Ime(ImeEvent::Enabled));
        });
    }
    if state.history.composition.is_none() {
        return false;
    }
    let committed = events
        .iter()
        .any(|event| matches!(event, Event::Ime(ImeEvent::Commit(text)) if !text.is_empty()));
    let canceled = events.iter().any(|event| {
        matches!(event, Event::Ime(ImeEvent::Commit(text)) if text.is_empty())
            || (!committed && matches!(event, Event::Ime(ImeEvent::Disabled)))
    }) || ctx.input_mut(|input| {
        crate::app::shortcuts::consume_shortcut(input, Modifiers::NONE, Key::Escape)
            || crate::app::shortcuts::consume_shortcut(input, Modifiers::CTRL, Key::Z)
    });
    if canceled {
        let before = state.history.composition.take().unwrap();
        before.restore(state);
        state.history.last_typing = None;
        // egui's IME cursor state is private. Reset the widget's transient state
        // and let the restored logical selection receive focus below.
        egui::text_edit::TextEditState::default().store(ctx, Id::new("text_input"));
        ctx.input_mut(|input| input.events.retain(|event| !matches!(event, Event::Ime(_))));
        false
    } else {
        committed
    }
}

pub(super) fn text_navigation(
    ui: &Ui,
    text: &str,
    format: &crate::text::TextFormat,
    layouter: &mut impl FnMut(&Ui, &str, f32) -> std::sync::Arc<egui::Galley>,
    viewport_height: f32,
    zoom: f32,
) {
    let ctx = ui.ctx();
    let input_id = Id::new("text_input");
    let queued_id = input_id.with("navigation_input");
    if !ui.is_enabled() || !ctx.memory(|memory| memory.has_focus(input_id)) {
        return;
    }
    let mac = ctx.os() == egui::os::OperatingSystem::Mac;
    let navigation = |event: &Event| match event {
        Event::Key {
            key,
            modifiers,
            pressed: true,
            ..
        } => {
            let paragraph = matches!(key, Key::ArrowUp | Key::ArrowDown)
                && !modifiers.mac_cmd
                && if mac {
                    modifiers.alt && !modifiers.ctrl
                } else {
                    modifiers.ctrl && !modifiers.alt
                };
            let page = matches!(key, Key::PageUp | Key::PageDown)
                && !modifiers.ctrl
                && !modifiers.alt
                && !modifiers.command;
            let horizontal = matches!(key, Key::ArrowLeft | Key::ArrowRight);
            let vertical = matches!(key, Key::ArrowUp | Key::ArrowDown)
                && !modifiers.ctrl
                && !modifiers.alt
                && !modifiers.command;
            (paragraph || page || horizontal || vertical).then_some((*key, *modifiers, paragraph))
        }
        _ => None,
    };
    let mut events = ctx.data_mut(|data| {
        data.remove_temp::<Vec<Event>>(queued_id)
            .unwrap_or_default()
    });
    ctx.input_mut(|input| events.append(&mut input.events));
    if crate::text::TextFormat::needs_visual_layout(text) {
        let mut pointer_seen = false;
        let after_pointer = events.iter().position(|event| {
            if matches!(event, Event::PointerButton { pressed: true, .. }) {
                pointer_seen = true;
            }
            pointer_seen
                && matches!(
                    event,
                    Event::Text(_)
                        | Event::Paste(_)
                        | Event::Cut
                        | Event::Ime(_)
                        | Event::Key { pressed: true, .. }
                )
        });
        if let Some(index) = after_pointer {
            // Complex hit testing is corrected after TextEdit's pointer pass.
            // Its resulting logical caret must precede subsequent typing.
            let deferred = events.split_off(index);
            ctx.data_mut(|data| data.insert_temp(queued_id, deferred));
            ctx.input_mut(|input| input.events = events);
            ctx.request_repaint();
            return;
        }
    }
    if let Some(index) = events.iter().position(|event| navigation(event).is_some()) {
        let preceding_edit = events[..index].iter().any(|event| {
            matches!(
                event,
                Event::Text(_)
                    | Event::Paste(_)
                    | Event::Cut
                    | Event::Ime(_)
                    | Event::Key { pressed: true, .. }
                    | Event::PointerButton { .. }
            )
        });
        if preceding_edit {
            // Let the real TextEdit finish preceding input before calculating
            // a paragraph/page move from its resulting text and caret.
            let deferred = events.split_off(index);
            ctx.data_mut(|data| data.insert_temp(queued_id, deferred));
            ctx.request_repaint();
        } else {
            let (key, modifiers, paragraph) = navigation(&events.remove(index)).unwrap();
            let galley = layouter(ui, text, ui.available_width());
            let layout = format.editor_layout(text);
            let mut editor = TextEdit::load_state(ctx, input_id).unwrap_or_default();
            let mut cursor = editor
                .cursor
                .range(&galley)
                .unwrap_or_else(|| egui::text::CursorRange::one(galley.begin()));
            if paragraph {
                let characters: Vec<_> = text.chars().collect();
                let current = cursor.primary.ccursor.index.min(characters.len());
                let next = if key == Key::ArrowUp {
                    characters[..current.saturating_sub(1)]
                        .iter()
                        .rposition(|ch| *ch == '\n')
                        .map_or(0, |position| position + 1)
                } else {
                    characters[current..]
                        .iter()
                        .position(|ch| *ch == '\n')
                        .map_or(characters.len(), |position| current + position + 1)
                };
                cursor.primary = galley.from_ccursor(CCursor::new(next));
            } else if matches!(key, Key::ArrowLeft | Key::ArrowRight) {
                let right = key == Key::ArrowRight;
                let next = if modifiers.ctrl || modifiers.alt || modifiers.command {
                    let current = cursor.primary.ccursor.index;
                    let visual_next = layout.horizontal_cursor(current, right);
                    let logical_key = if !modifiers.mac_cmd && visual_next != current {
                        if visual_next > current {
                            Key::ArrowRight
                        } else {
                            Key::ArrowLeft
                        }
                    } else {
                        key
                    };
                    let index = if !text.is_ascii() && (modifiers.ctrl || modifiers.alt) {
                        word_boundary(text, current, logical_key == Key::ArrowRight)
                    } else {
                        cursor.on_key_press(ctx.os(), &galley, &modifiers, logical_key);
                        cursor.primary.ccursor.index
                    };
                    if grapheme_boundaries(text).binary_search(&index).is_ok() {
                        index
                    } else {
                        adjacent_grapheme(text, index, logical_key == Key::ArrowRight)
                    }
                } else if !modifiers.shift && !cursor.is_empty() {
                    let mut ends = [
                        layout.caret(cursor.primary.ccursor.index),
                        layout.caret(cursor.secondary.ccursor.index),
                    ];
                    ends.sort_by(|left, right| {
                        left.row.cmp(&right.row).then(left.x.total_cmp(&right.x))
                    });
                    ends[usize::from(right)].index
                } else {
                    layout.horizontal_cursor(cursor.primary.ccursor.index, right)
                };
                cursor.primary = galley.from_ccursor(CCursor::new(next));
            } else if matches!(key, Key::ArrowUp | Key::ArrowDown) {
                let next = layout.vertical_cursor(
                    cursor.primary.ccursor.index,
                    if key == Key::ArrowUp { -1 } else { 1 },
                );
                cursor.primary = galley.from_ccursor(CCursor::new(next));
            } else {
                let caret = layout.caret(cursor.primary.ccursor.index);
                let distance = (viewport_height / zoom).max(caret.height)
                    * if key == Key::PageUp { -1.0 } else { 1.0 };
                let next = layout.hit_test(caret.x, caret.y + caret.height * 0.5 + distance);
                cursor.primary = galley.from_ccursor(CCursor::new(next));
            }
            if !modifiers.shift {
                cursor.secondary = cursor.primary;
            }
            editor.cursor.set_range(Some(cursor));
            editor.store(ctx, input_id);
            ctx.data_mut(|data| data.insert_temp(input_id.with("navigation_scroll"), true));
            if let Some(next) = events.iter().position(|event| navigation(event).is_some()) {
                let deferred = events.split_off(next);
                ctx.data_mut(|data| data.insert_temp(queued_id, deferred));
                ctx.request_repaint();
            }
        }
    }
    ctx.input_mut(|input| input.events = events);
}
