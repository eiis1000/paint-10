use super::*;
use crate::text::{FontBytes, FontMemory, TextStyle as DocumentTextStyle};
use egui::text::{CCursor, CCursorRange};
#[cfg(test)]
mod history_tests;
#[cfg(test)]
mod lifecycle_tests;
mod ribbon_ui;
mod unicode_input;

pub(in crate::app) use unicode_input::word_selection;
use unicode_input::{snap_editor_selection, text_navigation, update_composition, GraphemeBuffer};

#[derive(Default)]
pub(in crate::app) struct TextHistory {
    undo: Vec<TextSnapshot>,
    redo: Vec<TextSnapshot>,
    last_typing: Option<f64>,
    geometry: Option<TextSnapshot>,
    cursor: Option<CCursorRange>,
    composition: Option<TextSnapshot>,
}

#[derive(Clone)]
struct TextSnapshot {
    origin: Point,
    text: String,
    format: crate::text::TextFormat,
    selection: std::ops::Range<usize>,
    insertion_style: Option<DocumentTextStyle>,
    cursor: Option<CCursorRange>,
}

#[derive(Clone)]
struct FontDraft {
    source: String,
    value: String,
    changed: bool,
}

#[derive(Clone)]
struct FontPreview {
    name: String,
    face: Option<fontdb::ID>,
    texture: TextureHandle,
}

impl TextSnapshot {
    fn capture(state: &TextEditState) -> Self {
        Self {
            origin: state.origin,
            text: state.text.clone(),
            format: state.format.clone(),
            selection: state.selection.clone(),
            insertion_style: state.insertion_style.clone(),
            cursor: state.history.cursor,
        }
    }

    fn restore(self, state: &mut TextEditState) {
        state.origin = self.origin;
        state.text = self.text;
        state.format = self.format;
        state.selection = self.selection;
        state.insertion_style = self.insertion_style;
        state.history.cursor = self.cursor;
        state.focus = true;
    }

    fn bytes_with_fonts(&self, fonts: &mut FontMemory) -> usize {
        self.text.len()
            + self.format.memory_bytes_with_fonts(fonts)
            + self.insertion_style.as_ref().map_or(0, |style| {
                style.font_name.len() + fonts.include(&style.font)
            })
    }
}

impl TextHistory {
    fn record(&mut self, before: TextSnapshot, time: f64, typing: bool) {
        let grouped = typing && self.last_typing.is_some_and(|last| time - last < 0.75);
        if !grouped {
            self.undo.push(before);
            let mut fonts = FontMemory::default();
            let mut bytes = 0usize;
            for index in (0..self.undo.len()).rev() {
                bytes = bytes.saturating_add(self.undo[index].bytes_with_fonts(&mut fonts));
                if bytes > 32 * 1024 * 1024 && index + 1 < self.undo.len() {
                    self.undo.drain(..=index);
                    break;
                }
            }
        }
        self.redo.clear();
        self.last_typing = typing.then_some(time);
    }
}

impl PaintApp {
    pub(in crate::app) fn begin_text_geometry_history(&mut self) {
        if let Some(state) = &mut self.text_edit {
            state.history.geometry = Some(TextSnapshot::capture(state));
        }
    }

    pub(in crate::app) fn finish_text_geometry_history(&mut self, time: f64) {
        if let Some(state) = &mut self.text_edit {
            if let Some(before) = state.history.geometry.take() {
                if before.origin != state.origin || before.format != state.format {
                    state.history.record(before, time, false);
                }
            }
        }
    }

    pub(in crate::app) fn text_selection_action(&mut self, action: Action, ctx: &Context) {
        let Some(state) = self.text_edit.as_mut() else {
            return;
        };
        if matches!(action, Action::SelectAll) {
            state.selection = 0..state.text.chars().count();
            state.focus = true;
        } else if matches!(action, Action::Clear) {
            if let Err(error) = apply_text_clipboard(state, action, None, ctx) {
                self.message = error;
            }
        }
    }

    pub(in crate::app) fn text_can_undo(&self) -> bool {
        self.text_edit
            .as_ref()
            .is_some_and(|state| !state.history.undo.is_empty())
    }

    pub(in crate::app) fn text_can_redo(&self) -> bool {
        self.text_edit
            .as_ref()
            .is_some_and(|state| !state.history.redo.is_empty())
    }

    pub(in crate::app) fn text_history_action(&mut self, action: Action, _ctx: &Context) -> bool {
        let Some(state) = self.text_edit.as_mut() else {
            return false;
        };
        let redo = match action {
            Action::Undo => false,
            Action::Redo => true,
            _ => return false,
        };
        let snapshot = if redo {
            state.history.redo.pop()
        } else {
            state.history.undo.pop()
        };
        if let Some(snapshot) = snapshot {
            let current = TextSnapshot::capture(state);
            if redo {
                state.history.undo.push(current);
            } else {
                state.history.redo.push(current);
            }
            snapshot.restore(state);
            state.history.last_typing = None;
            self.colors[0] = active_style(state).color;
            if let Some(background) = state.format.background {
                self.colors[1] = background;
            }
            state.palette_colors = self.colors;
        }
        true
    }

    /// Route ribbon and menu clipboard commands to the active text selection.
    /// Returning false lets the caller handle image-only clipboard contents.
    pub(in crate::app) fn text_clipboard_action(&mut self, action: Action, ctx: &Context) -> bool {
        if self.text_edit.is_none() || !matches!(action, Action::Copy | Action::Cut | Action::Paste)
        {
            return false;
        }
        let pasted: Option<String> = if matches!(action, Action::Paste) {
            #[cfg(target_arch = "wasm32")]
            {
                self.paste_browser_clipboard();
                return true;
            }
            #[cfg(not(target_arch = "wasm32"))]
            match self
                .clipboard
                .as_mut()
                .and_then(|clipboard| clipboard.get_text().ok())
            {
                Some(text) => Some(text),
                None => return false,
            }
        } else {
            None
        };
        let state = self.text_edit.as_mut().expect("active text editor");
        let result = apply_text_clipboard(state, action, pasted.as_deref(), ctx);
        if let Err(error) = result {
            self.message = error;
        }
        true
    }

    pub(in crate::app) fn edit_text_object(&mut self, index: usize) {
        if let ObjectKind::Text { text, format } = &self.doc.objects[index].kind {
            self.colors[0] = format.style_at(0).color;
            if let Some(background) = format.background {
                self.colors[1] = background;
            }
            self.text_tab = true;
            self.text_edit = Some(TextEditState {
                index: Some(index),
                origin: self.doc.objects[index].pos,
                text: text.clone(),
                format: format.clone(),
                focus: true,
                selection: 0..0,
                insertion_style: None,
                history: TextHistory::default(),
                palette_colors: self.colors,
            });
            self.refresh = true;
        }
    }

    pub(in crate::app) fn commit_text(&mut self) {
        if let Some(mut state) = self.text_edit.take() {
            if let Some(before) = state.history.composition.take() {
                // A global Save/tool change can end editing before the input
                // method commits. Only confirmed text belongs in the project.
                before.restore(&mut state);
            }
            self.text_tab = false;
            let next_style = active_style(&state);
            self.text_format = state.format.clone();
            self.text_format.set_default_style(&next_style);
            self.text_format.size_mode = crate::text::TextSizeMode::Em;
            // A new box inherits the current font controls, not the previous box's spans.
            self.text_format.spans.clear();
            if !state.text.is_empty() {
                self.doc.begin();
                let kind = ObjectKind::Text {
                    text: state.text,
                    format: state.format,
                };
                let index = if let Some(index) = state.index {
                    self.doc.objects[index].kind = kind;
                    self.doc.objects[index].pos = state.origin;
                    index
                } else {
                    self.doc.add_object(Object::new(kind, state.origin))
                };
                self.doc.commit();
                self.object = Some(index);
            } else if let Some(index) = state.index {
                self.doc.begin();
                self.doc.objects.remove(index);
                self.doc.commit();
                self.clear_selection();
            }
            self.refresh = true;
        }
    }

    pub(in crate::app) fn text_shortcuts(&mut self, ctx: &Context) {
        if keytips::popup_open(ctx) {
            return;
        }
        if self.text_edit.as_ref().is_some_and(|state| {
            state.history.composition.is_some()
                || ctx.input(|input| {
                    input
                        .events
                        .iter()
                        .any(|event| matches!(event, Event::Ime(ImeEvent::Preedit(text) | ImeEvent::Commit(text)) if !text.is_empty()))
                })
        }) {
            // Composition owns cancellation and history until its final text
            // is committed. Intermediate predictions are not ordinary edits.
            return;
        }
        if self.text_edit.is_some()
            && ctx.input(|input| {
                input
                    .events
                    .iter()
                    .any(|event| matches!(event, Event::Paste(text) if text.is_empty()))
            })
        {
            ctx.input_mut(|input| {
                input
                    .events
                    .retain(|event| !matches!(event, Event::Paste(text) if text.is_empty()))
            });
            self.action(Action::Paste, ctx);
        }
        if self.text_edit.is_none() {
            return;
        }
        let redo = ctx.input_mut(|input| {
            super::shortcuts::consume_shortcut(input, Modifiers::CTRL, Key::Y)
                || super::shortcuts::consume_shortcut(
                    input,
                    Modifiers::CTRL | Modifiers::SHIFT,
                    Key::Z,
                )
        });
        let undo = !redo
            && ctx.input_mut(|input| {
                super::shortcuts::consume_shortcut(input, Modifiers::CTRL, Key::Z)
            });
        if undo || redo {
            self.text_history_action(if redo { Action::Redo } else { Action::Undo }, ctx);
        }
        let mut state = self.text_edit.take().expect("active text editor");
        let time = ctx.input(|input| input.time);
        for key in [Key::B, Key::I, Key::U] {
            if ctx
                .input_mut(|input| super::shortcuts::consume_shortcut(input, Modifiers::CTRL, key))
            {
                let current = active_style(&state);
                let result = change_style(&mut state, time, |style| match key {
                    Key::B => style.bold = !current.bold,
                    Key::I => style.italic = !current.italic,
                    Key::U => style.underline = !current.underline,
                    _ => {}
                });
                if let Err(error) = result {
                    self.message = error;
                }
            }
        }
        self.text_edit = Some(state);
    }

    fn font_control(&mut self, ui: &mut Ui, style: &mut DocumentTextStyle) {
        let input_id = Id::new("paint10_font_name");
        let draft_id = input_id.with("draft");
        let mut draft = ui.ctx().data(|data| data.get_temp::<FontDraft>(draft_id));
        if draft
            .as_ref()
            .is_none_or(|draft| draft.source != style.font_name)
        {
            draft = Some(FontDraft {
                source: style.font_name.clone(),
                value: style.font_name.clone(),
                changed: false,
            });
        }
        let mut draft = draft.expect("font draft initialized");
        let mut chosen = None;
        let response = ui
            .horizontal(|ui| {
                let response = ui.add(
                    TextEdit::singleline(&mut draft.value)
                        .id(input_id)
                        .desired_width((ui.available_width() - 34.0).max(80.0))
                        .char_limit(200),
                );
                text_control(ui, &response, "Font", false);
                draft.changed |= response.changed();
                let filter = if draft.changed {
                    draft.value.trim().to_lowercase()
                } else {
                    String::new()
                };
                let list = ribbon::ribbon_menu_button(ui, "", "FL", "fonts", None, |ui| {
                    let row_count = std::iter::once("Sans serif")
                        .chain(self.font_names.iter().map(|(name, _)| name.as_str()))
                        .filter(|name| name.to_lowercase().contains(&filter))
                        .count();
                    let extra_rows = usize::from(cfg!(target_arch = "wasm32"));
                    let height = ((row_count.max(1) + extra_rows) as f32 * 31.0).min(280.0);
                    ScrollArea::vertical()
                        .drag_to_scroll(false)
                        .max_height(280.0)
                        .min_scrolled_height(height)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.set_min_width(280.0);
                            let mut matches = 0;
                            for name in std::iter::once("Sans serif")
                                .chain(self.font_names.iter().map(|(name, _)| name.as_str()))
                            {
                                if !name.to_lowercase().contains(&filter) {
                                    continue;
                                }
                                matches += 1;
                                let face = self
                                    .font_names
                                    .iter()
                                    .find(|(family, _)| family == name)
                                    .map(|(_, id)| *id);
                                let choice = font_choice(
                                    ui,
                                    &self.font_db,
                                    name,
                                    face,
                                    style.font_name == name,
                                );
                                ribbon_controls::register(
                                    ui,
                                    &choice,
                                    &format!("{matches:03}"),
                                    keytips::Kind::Button,
                                );
                                if choice.clicked() {
                                    chosen = Some(name.to_owned());
                                    ui.close_menu();
                                }
                            }
                            if matches == 0 {
                                ui.label("No matching fonts");
                            }
                            #[cfg(target_arch = "wasm32")]
                            {
                                ui.separator();
                                let load = ui.button("Load font… (TTF, OTF, TTC)");
                                ribbon_controls::register(ui, &load, "L", keytips::Kind::Button);
                                if load.clicked() {
                                    self.choose_browser_font();
                                    ui.close_menu();
                                }
                            }
                        });
                });
                text_control(ui, &list.response, "Font list", false);
                ribbon_controls::register(
                    ui,
                    &list.response,
                    "FL",
                    keytips::Kind::Menu { scope: "fonts" },
                );
                response
            })
            .inner;
        let committed = draft.changed && response.lost_focus() && !keytips::popup_open(ui.ctx());
        if chosen.is_some() || committed {
            let query = chosen.as_deref().unwrap_or(&draft.value).trim();
            if !self.apply_font_name(query, style) {
                self.message = format!("No available font matches “{query}”.");
            }
            draft = FontDraft {
                source: style.font_name.clone(),
                value: style.font_name.clone(),
                changed: false,
            };
        }
        ui.ctx().data_mut(|data| data.insert_temp(draft_id, draft));
    }

    fn apply_font_name(&self, query: &str, style: &mut DocumentTextStyle) -> bool {
        if query.is_empty() {
            return false;
        }
        let lower = query.to_lowercase();
        if "sans serif".starts_with(&lower) {
            style.font = crate::text::DEFAULT_FONT.into();
            style.font_name = crate::text::DEFAULT_FONT_NAME.into();
            style.font_index = 0;
            return true;
        }
        let candidate = self
            .font_names
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(query))
            .or_else(|| {
                self.font_names
                    .iter()
                    .find(|(name, _)| name.to_lowercase().starts_with(&lower))
            });
        let Some((name, id)) = candidate else {
            return false;
        };
        let Some((data, index)) = self
            .font_db
            .with_face_data(*id, |data, index| (FontBytes::from(data), index))
        else {
            return false;
        };
        if ab_glyph::FontRef::try_from_slice_and_index(&data, index).is_err() {
            return false;
        }
        style.font = data;
        style.font_name.clone_from(name);
        style.font_index = index;
        true
    }

    pub(in crate::app) fn register_document_fonts(&mut self) {
        let mut visited = Vec::<&[u8]>::new();
        let mut added = false;
        for object in &self.doc.objects {
            let ObjectKind::Text { format, .. } = &object.kind else {
                continue;
            };
            for data in format.font_assets().map(FontBytes::as_slice) {
                if data.is_empty() || visited.contains(&data) {
                    continue;
                }
                visited.push(data);
                let loaded = self.font_db.faces().any(|face| {
                    self.font_db
                        .with_face_data(face.id, |existing, _| existing == data)
                        .unwrap_or(false)
                });
                if !loaded {
                    // Register a collection once; fontdb discovers every face.
                    // The document keeps its exact original bytes and indices.
                    self.font_db.load_font_data(data.to_vec());
                    added = true;
                }
            }
        }
        if added {
            self.font_names = font_families(&self.font_db);
        }
    }

    pub(in crate::app) fn text_editor(&mut self, ctx: &Context) {
        let Some(mut state) = self.text_edit.take() else {
            ctx.data_mut(|data| data.remove::<Rect>(Id::new("paint10_text_editor_rect")));
            return;
        };
        let composition_ends = update_composition(&mut state, ctx);
        if self.dialog.is_none() && self.pending.is_none() {
            if let Err(error) = sync_palette(&mut state, self.colors, ctx.input(|input| input.time))
            {
                self.message = error;
            }
        }
        let position =
            self.canvas_rect.min + vec2(state.origin.0 as f32, state.origin.1 as f32) * self.zoom;
        let popup_open = keytips::popup_open(ctx);
        let modal_open = self.dialog.is_some() || self.pending.is_some();
        let first = state.focus && !popup_open && !modal_open;
        let unchanged_existing_text = state
            .index
            .and_then(|index| self.doc.objects.get(index))
            .is_some_and(|object| {
                matches!(&object.kind, ObjectKind::Text { text, format }
                    if text == &state.text && format == &state.format)
            });
        if first && !unchanged_existing_text {
            // Opening a saved box is not an edit. Keep its exact stored font
            // assets until text or formatting changes require new faces.
            prepare_text_fonts(&self.font_db, &mut state.format, &state.text);
        }
        let composing = state.history.composition.is_some();
        let commit_requested = !composing
            && !popup_open
            && !modal_open
            && ctx.input_mut(|input| {
                super::shortcuts::consume_shortcut(input, Modifiers::CTRL, Key::Enter)
            });
        let insertion_style = active_style(&state);
        let input_id = Id::new("text_input");
        if first {
            let mut editor = TextEdit::load_state(ctx, input_id).unwrap_or_default();
            let cursor = state
                .history
                .cursor
                .filter(|cursor| {
                    let [start, end] = cursor.sorted();
                    (start.index..end.index) == state.selection
                })
                .unwrap_or_else(|| {
                    CCursorRange::two(
                        CCursor::new(state.selection.start),
                        CCursor::new(state.selection.end),
                    )
                });
            editor.cursor.set_char_range(Some(cursor));
            editor.clear_undoer();
            editor.store(ctx, input_id);
        }
        let before = TextSnapshot::capture(&state);
        let original_text = state.text.clone();
        let original_format = state.format.clone();
        let zoom = self.zoom;
        let font_db = &self.font_db;
        let (padding_x, padding_y) = state.format.text_padding();
        let mut layouter = |ui: &Ui, text: &str, _width: f32| {
            let mut live_format = original_format.clone();
            if text != original_text {
                let _ = live_format.update_spans_for_edit_with_style(
                    &original_text,
                    text,
                    insertion_style.clone(),
                );
                prepare_text_fonts(font_db, &mut live_format, text);
            }
            if text.is_empty() {
                live_format.set_default_style(&insertion_style);
            }
            text_preview::galley(ui, text, &live_format, zoom)
        };
        let output = Area::new(Id::new("inline_text"))
            .order(Order::Foreground)
            .fixed_pos(position)
            .constrain(false)
            .show(ctx, |ui| {
                ui.set_clip_rect(self.canvas_rect.intersect(ctx.screen_rect()));
                // Reserve the image below the editor's selection and caret, then
                // fill it with the final text from this frame (including typing).
                let picture = ui.painter().add(egui::Shape::Noop);
                ui.visuals_mut().selection.bg_fill = display::color([40, 140, 235, 85]);
                if !popup_open && !modal_open && !commit_requested && !composing {
                    let viewport = ctx
                        .data(|data| data.get_temp::<Rect>(Id::new("paint10_canvas_viewport")))
                        .unwrap_or(self.canvas_rect.intersect(ctx.screen_rect()));
                    text_navigation(
                        ui,
                        &state.text,
                        &state.format,
                        &mut layouter,
                        viewport.height(),
                        zoom,
                    );
                }
                let interaction = text_preview::begin_interaction(ui, &state.text, &state.format);
                let mut buffer = GraphemeBuffer {
                    text: &mut state.text,
                    composing,
                };
                let mut output = TextEdit::multiline(&mut buffer)
                    .id(input_id)
                    .layouter(&mut layouter)
                    .frame(false)
                    .margin(vec2(padding_x as f32, padding_y as f32) * zoom)
                    .char_limit(16000)
                    .desired_width(state.format.content_width() as f32 * zoom)
                    .desired_rows(2)
                    .min_size(vec2(
                        state.format.width as f32 * zoom,
                        state.format.minimum_height as f32 * zoom,
                    ))
                    .show(ui);
                if first && !output.response.has_focus() {
                    output.response.request_focus();
                }
                let mut live_format = original_format.clone();
                let new_cursor = output
                    .cursor_range
                    .map_or(state.selection.end, |range| range.primary.ccursor.index);
                let _ = live_format.update_spans_for_edit_at(
                    &original_text,
                    &state.text,
                    state.selection.clone(),
                    new_cursor,
                    insertion_style.clone(),
                );
                if state.text != original_text {
                    prepare_text_fonts(font_db, &mut live_format, &state.text);
                }
                text_preview::finish_interaction(
                    ui,
                    &mut output,
                    &state.text,
                    &live_format,
                    zoom,
                    interaction,
                );
                if !composing {
                    snap_editor_selection(&state.text, &mut output, ctx, input_id);
                }
                text_preview::paint(ui, picture, position, &state.text, &live_format, zoom);
                dashed_rect(ui.painter(), output.response.rect.expand(3.0));
                output
            })
            .inner;
        if ctx.data_mut(|data| data.remove_temp::<bool>(input_id.with("navigation_scroll")))
            == Some(true)
        {
            if let Some(cursor) = output.cursor_range {
                let layout = state.format.editor_layout(&state.text);
                let caret = layout.caret(cursor.primary.ccursor.index);
                let caret = vec2(caret.x, caret.y + caret.height * 0.5) * zoom
                    + vec2(padding_x as f32, padding_y as f32) * zoom;
                let viewport = ctx
                    .data(|data| data.get_temp::<Rect>(Id::new("paint10_canvas_viewport")))
                    .unwrap_or(self.canvas_rect.intersect(ctx.screen_rect()));
                if !viewport.contains(position + caret) {
                    self.center_canvas_on(
                        (
                            state.origin.0 + (caret.x / zoom).round() as i32,
                            state.origin.1 + (caret.y / zoom).round() as i32,
                        ),
                        ctx,
                    );
                }
            }
        }
        ctx.data_mut(|data| {
            data.insert_temp(Id::new("paint10_text_editor_rect"), output.response.rect)
        });
        state.focus &= popup_open || modal_open;
        if state.text != original_text {
            let new_cursor = output
                .cursor_range
                .map_or(state.selection.end, |range| range.primary.ccursor.index);
            match state.format.update_spans_for_edit_at(
                &original_text,
                &state.text,
                state.selection.clone(),
                new_cursor,
                insertion_style.clone(),
            ) {
                Ok(()) => {
                    if !composing {
                        state
                            .history
                            .record(before, ctx.input(|input| input.time), true);
                    }
                }
                Err(error) => {
                    self.message = error;
                    before.restore(&mut state);
                }
            }
            prepare_text_fonts(font_db, &mut state.format, &state.text);
        }
        if let Some(range) = output.cursor_range {
            state.history.cursor = Some(range.as_ccursor_range());
            let selection = range.as_sorted_char_range();
            if selection != state.selection {
                state.selection = selection;
                state.insertion_style = None;
                if state.text == original_text {
                    state.history.last_typing = None;
                }
                self.colors[0] = active_style(&state).color;
                state.palette_colors = self.colors;
            }
        }
        if composition_ends {
            if let Some(before) = state.history.composition.take() {
                if before.text != state.text {
                    state
                        .history
                        .record(before, ctx.input(|input| input.time), false);
                } else {
                    // An unchanged prediction must not add automatically
                    // discovered font assets to an existing saved object.
                    state.format = before.format;
                }
            }
        }
        let done = !popup_open
            && !modal_open
            && state.history.composition.is_none()
            && !self.text_geometry_gesture()
            && (commit_requested
                || (!first
                    && ctx.input(|input| input.pointer.any_pressed())
                    && ctx
                        .input(|input| input.pointer.interact_pos())
                        .is_some_and(|point| {
                            self.canvas_rect.contains(point)
                                && ctx.layer_id_at(point) == Some(LayerId::background())
                                && !output.response.rect.expand(8.0).contains(point)
                        })));
        let cancel = !popup_open && ctx.input(|input| input.key_pressed(Key::Escape));
        if cancel {
            self.refresh = true;
        } else {
            self.text_edit = Some(state);
            if done {
                self.commit_text();
            }
        }
    }
}

pub(in crate::app) fn font_families(database: &fontdb::Database) -> Vec<(String, fontdb::ID)> {
    let mut names: Vec<_> = database
        .faces()
        .filter_map(|face| face.families.first().map(|family| family.0.clone()))
        .collect();
    names.sort_by_key(|name| name.to_lowercase());
    names.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    names
        .into_iter()
        .filter_map(|name| {
            let id = database.query(&fontdb::Query {
                families: &[fontdb::Family::Name(&name)],
                ..Default::default()
            })?;
            Some((name, id))
        })
        .collect()
}

fn font_choice(
    ui: &mut Ui,
    database: &fontdb::Database,
    name: &str,
    face: Option<fontdb::ID>,
    selected: bool,
) -> Response {
    let response = ui.add(theme::MenuItem::new(name).selected(selected).width(280.0));
    if !ui.is_rect_visible(response.rect) {
        return response;
    }
    let cache_id = Id::new("paint10-font-previews");
    let mut cache = ui
        .ctx()
        .data(|data| data.get_temp::<Vec<FontPreview>>(cache_id))
        .unwrap_or_default();
    let existing = cache
        .iter()
        .find(|preview| preview.name == name && preview.face == face);
    let texture = if let Some(existing) = existing {
        existing.texture.clone()
    } else {
        let mut format = crate::text::TextFormat {
            width: 1024,
            size: 17.0,
            ..Default::default()
        };
        if let Some((data, index)) = face.and_then(|id| {
            database.with_face_data(id, |data, index| (FontBytes::from(data), index))
        }) {
            format.font = data;
            format.font_index = index;
        }
        let raster = format.render(name);
        let texture = ui.ctx().load_texture(
            format!("font-preview-{name}"),
            display::image(
                [raster.width() as usize, raster.height() as usize],
                raster.as_raw(),
            ),
            TextureOptions::LINEAR,
        );
        if cache.len() >= 32 {
            cache.remove(0);
        }
        cache.push(FontPreview {
            name: name.into(),
            face,
            texture: texture.clone(),
        });
        ui.ctx().data_mut(|data| data.insert_temp(cache_id, cache));
        texture
    };
    // Keep the shared menu's selection/checkmark and accessibility label, while
    // replacing only its glyphs with an example from the actual family.
    let text_rect = Rect::from_min_max(
        response.rect.min + vec2(30.0, 1.0),
        response.rect.max - vec2(2.0, 1.0),
    );
    let fill = if selected {
        ui.visuals().selection.bg_fill
    } else if response.hovered() || response.has_focus() {
        ui.visuals().widgets.hovered.weak_bg_fill
    } else {
        ui.visuals().window_fill
    };
    let painter = ui
        .painter()
        .with_clip_rect(text_rect.intersect(ui.clip_rect()));
    painter.rect_filled(text_rect, 0.0, fill);
    let size = texture.size_vec2();
    painter.image(
        texture.id(),
        Rect::from_min_size(text_rect.left_center() - vec2(0.0, size.y / 2.0), size),
        Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    response.on_hover_text(name)
}

fn prepare_text_fonts(
    database: &fontdb::Database,
    format: &mut crate::text::TextFormat,
    text: &str,
) {
    // Legacy projects keep font bytes directly on their spans. Make those
    // exact faces available as fallbacks without replacing the chosen family.
    let legacy: Vec<_> = std::iter::once(format.default_style_ref())
        .chain(format.spans.iter().map(|span| span.style.as_ref()))
        .filter_map(|style| {
            let data = style.font_bytes();
            if format
                .font_faces
                .iter()
                .any(|face| face.index == style.font_index && face.data.as_ref() == data)
            {
                return None;
            }
            Some(crate::text::EmbeddedFont {
                family: style.font_name.to_owned(),
                data: data.into(),
                index: style.font_index,
                bold: false,
                italic: false,
            })
        })
        .collect();
    for face in legacy {
        if format.font_faces.len() < crate::text::MAX_FONT_FACES
            && !format
                .font_faces
                .iter()
                .any(|stored| stored.index == face.index && stored.data == face.data)
            && can_embed_font(format, &face)
            && face.validate().is_ok()
        {
            format.font_faces.push(face);
        }
    }
    let mut families: Vec<_> = std::iter::once(format.font_name.clone())
        .chain(format.spans.iter().map(|span| span.style.font_name.clone()))
        .collect();
    families.sort();
    families.dedup();
    for family in families {
        for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
            let id = database.query(&fontdb::Query {
                families: &[fontdb::Family::Name(&family)],
                weight: if bold {
                    fontdb::Weight::BOLD
                } else {
                    fontdb::Weight::NORMAL
                },
                style: if italic {
                    fontdb::Style::Italic
                } else {
                    fontdb::Style::Normal
                },
                ..Default::default()
            });
            if let Some(id) = id {
                embed_font_face(database, format, id, &family);
            }
        }
    }
    let mut missing: std::collections::BTreeSet<_> = text
        .chars()
        .filter(|character| !character.is_control() && !character.is_whitespace())
        .collect();
    let base_fonts: Vec<_> = std::iter::once(format.default_style_ref())
        .chain(format.spans.iter().map(|span| span.style.as_ref()))
        .filter_map(|style| {
            ab_glyph::FontRef::try_from_slice_and_index(style.font_bytes(), style.font_index).ok()
        })
        .collect();
    let embedded: Vec<_> = format
        .font_faces
        .iter()
        .filter_map(|face| ab_glyph::FontRef::try_from_slice_and_index(&face.data, face.index).ok())
        .collect();
    missing.retain(|character| {
        !base_fonts
            .iter()
            .chain(&embedded)
            .any(|font| crate::text::font_supports_outline(font, *character))
    });
    if missing.is_empty() {
        return;
    }
    for face in database.faces() {
        let Some(family) = face.families.first().map(|name| name.0.as_str()) else {
            continue;
        };
        let covered = database
            .with_face_data(face.id, |data, index| {
                let Ok(font) = ab_glyph::FontRef::try_from_slice_and_index(data, index) else {
                    return Vec::new();
                };
                missing
                    .iter()
                    .copied()
                    .filter(|character| crate::text::font_supports_outline(&font, *character))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !covered.is_empty() && embed_font_face(database, format, face.id, family) {
            for character in covered {
                missing.remove(&character);
            }
            if missing.is_empty() {
                break;
            }
        }
    }
}

fn embed_font_face(
    database: &fontdb::Database,
    format: &mut crate::text::TextFormat,
    id: fontdb::ID,
    family: &str,
) -> bool {
    let Some(face) = database.face(id) else {
        return false;
    };
    let bold = face.weight >= fontdb::Weight::BOLD;
    let italic = face.style != fontdb::Style::Normal;
    if format.font_faces.len() >= crate::text::MAX_FONT_FACES {
        return false;
    }
    let Some(embedded) = database.with_face_data(id, |data, index| {
        if format.font_faces.iter().any(|face| {
            face.family == family
                && face.index == index
                && face.data.as_ref() == data
                && face.bold == bold
                && face.italic == italic
        }) {
            return None;
        }
        Some(crate::text::EmbeddedFont {
            family: family.into(),
            data: data.into(),
            index,
            bold,
            italic,
        })
    }) else {
        return false;
    };
    let Some(embedded) = embedded else {
        return true;
    };
    if embedded.validate().is_err() || !can_embed_font(format, &embedded) {
        return false;
    }
    format.font_faces.push(embedded);
    true
}

fn can_embed_font(format: &crate::text::TextFormat, face: &crate::text::EmbeddedFont) -> bool {
    let mut fonts = FontMemory::default();
    let current = format.memory_bytes_with_fonts(&mut fonts);
    let additional = std::mem::size_of::<crate::text::EmbeddedFont>()
        + face.family.len()
        + fonts.include(&face.data);
    current.saturating_add(additional) <= crate::text::MAX_FORMAT_BYTES
}

fn text_control(ui: &Ui, response: &Response, name: &'static str, selected: bool) {
    ribbon_controls::named(ui, response, name);
    response.widget_info(|| match name {
        "Font" => WidgetInfo::labeled(WidgetType::TextEdit, response.enabled(), name),
        "Font list" => WidgetInfo::labeled(WidgetType::ComboBox, response.enabled(), name),
        "Font size" => WidgetInfo::labeled(WidgetType::DragValue, response.enabled(), name),
        _ => WidgetInfo::selected(WidgetType::Button, response.enabled(), selected, name),
    });
    ui.ctx()
        .data_mut(|data| data.insert_temp(Id::new(("paint10_text_control", name)), response.id));
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
    response.clone().on_hover_text(name);
}

fn active_style(state: &TextEditState) -> DocumentTextStyle {
    state.insertion_style.clone().unwrap_or_else(|| {
        let index = if state.selection.is_empty() {
            state.selection.start.saturating_sub(1)
        } else {
            state.selection.start
        };
        state.format.style_at(index)
    })
}

fn sync_palette(state: &mut TextEditState, colors: [Color; 2], time: f64) -> Result<(), String> {
    if state.palette_colors[0] != colors[0] {
        change_style(state, time, |style| style.color = colors[0])?;
    }
    if state.palette_colors[1] != colors[1] && state.format.background.is_some() {
        let before = TextSnapshot::capture(state);
        state.format.background = Some(colors[1]);
        state.history.record(before, time, false);
    }
    state.palette_colors = colors;
    Ok(())
}

pub(in crate::app) fn apply_text_clipboard(
    state: &mut TextEditState,
    action: Action,
    pasted: Option<&str>,
    ctx: &Context,
) -> Result<(), String> {
    let selection = state.selection.clone();
    let offsets: Vec<_> = state
        .text
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(state.text.len()))
        .collect();
    if selection.start > selection.end || selection.end >= offsets.len() {
        return Err("The text selection is no longer valid.".into());
    }
    if matches!(action, Action::Copy | Action::Cut) {
        if selection.is_empty() {
            return Ok(());
        }
        ctx.copy_text(state.text[offsets[selection.start]..offsets[selection.end]].to_owned());
    }
    let inserted = match action {
        Action::Copy => {
            state.focus = true;
            return Ok(());
        }
        Action::Cut | Action::Clear => "",
        Action::Paste => match pasted {
            Some(text) if !text.is_empty() => text,
            _ => return Ok(()),
        },
        _ => return Ok(()),
    };
    let count = state.text.chars().count() - selection.len() + inserted.chars().count();
    if count > 16000 {
        return Err("A text box can contain at most 16,000 characters.".into());
    }
    let before = TextSnapshot::capture(state);
    let style = active_style(state);
    let mut text = state.text.clone();
    text.replace_range(offsets[selection.start]..offsets[selection.end], inserted);
    let caret = selection.start + inserted.chars().count();
    let mut format = state.format.clone();
    format.update_spans_for_edit_at(&state.text, &text, selection, caret, style)?;
    format.validate_for_text(&text)?;
    state.text = text;
    state.format = format;
    state.selection = caret..caret;
    state.insertion_style = None;
    state.focus = true;
    state
        .history
        .record(before, ctx.input(|input| input.time), false);
    Ok(())
}

fn change_style(
    state: &mut TextEditState,
    time: f64,
    mut change: impl FnMut(&mut DocumentTextStyle),
) -> Result<(), String> {
    let before = TextSnapshot::capture(state);
    if state.selection.is_empty() {
        let mut style = active_style(state);
        change(&mut style);
        style.validate()?;
        if state.text.is_empty() {
            state.format.set_default_style(&style);
        }
        state.insertion_style = Some(style);
    } else {
        state.format.modify_style(state.selection.clone(), change)?;
        state.insertion_style = None;
    }
    state.history.record(before, time, false);
    state.focus = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    mod picker_scroll_tests;
    mod ribbon_ui_tests;
    mod undo_order_tests;

    fn key(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn app_frame(app: &mut PaintApp, ctx: &Context, events: Vec<Event>) -> FullOutput {
        app_frame_at_width(app, ctx, events, 1200.0)
    }

    fn app_frame_at_width(
        app: &mut PaintApp,
        ctx: &Context,
        events: Vec<Event>,
        width: f32,
    ) -> FullOutput {
        ctx.run(
            RawInput {
                events,
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width, 720.0))),
                ..Default::default()
            },
            |ctx| {
                if !app.ribbon_keyboard(ctx) {
                    app.shortcut(ctx);
                }
                app.titlebar(ctx);
                app.ribbon(ctx);
                app.status(ctx);
                app.canvas(ctx);
                app.keyboard_menu(ctx);
                app.dialogs(ctx);
            },
        )
    }

    fn editing_app(ctx: &Context, text: &str) -> PaintApp {
        let mut app = PaintApp::new_with_context(ctx, false);
        app.text_tab = true;
        let mut state = state(text);
        state.origin = (20, 20);
        state.selection = 0..text.chars().count();
        state.focus = true;
        app.text_edit = Some(state);
        for _ in 0..3 {
            app_frame(&mut app, ctx, Vec::new());
        }
        app
    }

    fn timed_input_frame(
        app: &mut PaintApp,
        ctx: &Context,
        events: Vec<Event>,
        time: f64,
    ) -> FullOutput {
        let mut input = RawInput {
            events,
            time: Some(time),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 720.0))),
            ..Default::default()
        };
        eframe::App::raw_input_hook(app, ctx, &mut input);
        ctx.run(input, |ctx| {
            if !app.ribbon_keyboard(ctx) {
                app.shortcut(ctx);
            }
            app.titlebar(ctx);
            app.ribbon(ctx);
            app.status(ctx);
            app.canvas(ctx);
            app.keyboard_menu(ctx);
            app.dialogs(ctx);
        })
    }

    #[test]
    fn character_navigation_and_deletion_preserve_whole_graphemes() {
        for grapheme in ["e\u{301}", "👩\u{200d}💻", "🇺🇸", "👍🏽", "क्ष"] {
            let ctx = Context::default();
            let text = format!("A{grapheme}B");
            let end = 1 + grapheme.chars().count();
            let mut app = editing_app(&ctx, &text);
            let state = app.text_edit.as_mut().unwrap();
            state.selection = end..end;
            state.focus = true;
            timed_input_frame(&mut app, &ctx, vec![], 1.0);
            timed_input_frame(
                &mut app,
                &ctx,
                vec![key(Key::ArrowLeft, Modifiers::SHIFT)],
                2.0,
            );
            assert_eq!(
                app.text_edit.as_ref().unwrap().selection,
                1..end,
                "{grapheme}"
            );
            timed_input_frame(
                &mut app,
                &ctx,
                vec![key(Key::ArrowRight, Modifiers::NONE)],
                3.0,
            );
            assert_eq!(app.text_edit.as_ref().unwrap().selection, end..end);
            timed_input_frame(
                &mut app,
                &ctx,
                vec![key(Key::Backspace, Modifiers::NONE)],
                4.0,
            );
            assert_eq!(app.text_edit.as_ref().unwrap().text, "AB", "{grapheme}");
            timed_input_frame(&mut app, &ctx, vec![key(Key::Z, Modifiers::CTRL)], 5.0);
            assert_eq!(app.text_edit.as_ref().unwrap().text, text);
            timed_input_frame(
                &mut app,
                &ctx,
                vec![key(Key::ArrowLeft, Modifiers::NONE)],
                6.0,
            );
            assert_eq!(app.text_edit.as_ref().unwrap().selection, 1..1);
            timed_input_frame(&mut app, &ctx, vec![key(Key::Delete, Modifiers::NONE)], 7.0);
            assert_eq!(app.text_edit.as_ref().unwrap().text, "AB", "{grapheme}");
        }
    }

    #[test]
    fn ordered_typing_then_grapheme_arrow_and_replacement_keeps_the_caret() {
        let ctx = Context::default();
        let mut app = editing_app(&ctx, "");
        timed_input_frame(
            &mut app,
            &ctx,
            vec![
                Event::Text("Ae\u{301}B".into()),
                key(Key::ArrowLeft, Modifiers::NONE),
                key(Key::ArrowLeft, Modifiers::SHIFT),
                Event::Text("X".into()),
            ],
            1.0,
        );
        for step in 0..5 {
            timed_input_frame(&mut app, &ctx, vec![], 1.1 + step as f64 * 0.1);
        }
        assert_eq!(app.text_edit.as_ref().unwrap().text, "AXB");
        assert_eq!(app.text_edit.as_ref().unwrap().selection, 2..2);
    }

    fn rtl_editing_app(ctx: &Context, text: &str) -> PaintApp {
        let mut app = editing_app(ctx, text);
        let state = app.text_edit.as_mut().unwrap();
        state.format.font = crate::text::DEFAULT_FONT.into();
        state.format.font_name = "DejaVu Sans".into();
        state.format.size = 40.0;
        state.selection = 0..0;
        state.focus = true;
        timed_input_frame(&mut app, ctx, vec![], 0.5);
        app
    }

    #[test]
    fn rtl_arrow_selection_and_click_then_typing_follow_visual_carets() {
        let ctx = Context::default();
        let mut app = rtl_editing_app(&ctx, "שלום");
        timed_input_frame(
            &mut app,
            &ctx,
            vec![key(Key::ArrowLeft, Modifiers::NONE)],
            1.0,
        );
        assert_eq!(app.text_edit.as_ref().unwrap().selection, 1..1);
        timed_input_frame(
            &mut app,
            &ctx,
            vec![key(Key::ArrowLeft, Modifiers::SHIFT)],
            2.0,
        );
        assert_eq!(app.text_edit.as_ref().unwrap().selection, 1..2);
        let copied = timed_input_frame(&mut app, &ctx, vec![Event::Copy], 3.0);
        assert!(copied.platform_output.commands.iter().any(|command| {
            matches!(command, egui::OutputCommand::CopyText(text) if text == "ל")
        }));
        timed_input_frame(
            &mut app,
            &ctx,
            vec![key(Key::ArrowRight, Modifiers::NONE)],
            4.0,
        );
        assert_eq!(app.text_edit.as_ref().unwrap().selection, 1..1);
        let end = timed_input_frame(&mut app, &ctx, vec![key(Key::End, Modifiers::NONE)], 4.1);
        assert_eq!(app.text_edit.as_ref().unwrap().selection, 4..4);
        let home = timed_input_frame(&mut app, &ctx, vec![key(Key::Home, Modifiers::NONE)], 4.2);
        assert_eq!(app.text_edit.as_ref().unwrap().selection, 0..0);
        assert!(
            home.platform_output.ime.unwrap().cursor_rect.left()
                > end.platform_output.ime.unwrap().cursor_rect.left()
        );
        let state = app.text_edit.as_ref().unwrap();
        let caret = state.format.editor_layout(&state.text).caret(2);
        let (px, py) = state.format.text_padding();
        let point = app.canvas_rect.min
            + vec2(
                state.origin.0 as f32 + px as f32 + caret.x,
                state.origin.1 as f32 + py as f32 + caret.y + caret.height * 0.5,
            );
        timed_input_frame(
            &mut app,
            &ctx,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
                Event::Text("!".into()),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ],
            5.0,
        );
        for step in 0..5 {
            timed_input_frame(&mut app, &ctx, vec![], 5.1 + step as f64 * 0.1);
        }
        assert_eq!(app.text_edit.as_ref().unwrap().text, "של!ום");
    }

    #[test]
    fn mixed_direction_multiline_selection_copies_and_replaces_logical_text() {
        let ctx = Context::default();
        let original = "abc שלום xyz\nעוד line";
        let mut app = rtl_editing_app(&ctx, original);
        let state = app.text_edit.as_mut().unwrap();
        state.selection = 4..16;
        state.focus = true;
        timed_input_frame(&mut app, &ctx, vec![], 1.0);
        let copied = timed_input_frame(&mut app, &ctx, vec![Event::Copy], 2.0);
        assert!(copied.platform_output.commands.iter().any(|command| {
            matches!(command, egui::OutputCommand::CopyText(text) if text == "שלום xyz\nעוד")
        }));
        timed_input_frame(&mut app, &ctx, vec![Event::Paste("חדש\nnew".into())], 3.0);
        assert_eq!(app.text_edit.as_ref().unwrap().text, "abc חדש\nnew line");
        timed_input_frame(&mut app, &ctx, vec![key(Key::Z, Modifiers::CTRL)], 4.0);
        assert_eq!(app.text_edit.as_ref().unwrap().text, original);
        assert_eq!(app.text_edit.as_ref().unwrap().selection, 4..16);
    }

    #[test]
    fn word_navigation_moves_visually_through_rtl_words() {
        for (os, modifier) in [
            (
                egui::os::OperatingSystem::Windows,
                Modifiers::CTRL | Modifiers::COMMAND,
            ),
            (egui::os::OperatingSystem::Mac, Modifiers::ALT),
        ] {
            let ctx = Context::default();
            ctx.set_os(os);
            let mut app = rtl_editing_app(&ctx, "שלום עולם");
            timed_input_frame(&mut app, &ctx, vec![key(Key::ArrowLeft, modifier)], 1.0);
            assert_eq!(app.text_edit.as_ref().unwrap().selection, 4..4);
            timed_input_frame(
                &mut app,
                &ctx,
                vec![key(Key::ArrowLeft, modifier | Modifiers::SHIFT)],
                2.0,
            );
            assert_eq!(app.text_edit.as_ref().unwrap().selection, 4..9);
            let copy = timed_input_frame(&mut app, &ctx, vec![Event::Copy], 3.0);
            assert!(copy.platform_output.commands.iter().any(|command| {
                matches!(command, egui::OutputCommand::CopyText(text) if text == " עולם")
            }));
            let state = app.text_edit.as_mut().unwrap();
            state.selection = 9..9;
            state.focus = true;
            timed_input_frame(&mut app, &ctx, vec![], 4.0);
            timed_input_frame(&mut app, &ctx, vec![key(Key::Backspace, modifier)], 5.0);
            assert_eq!(app.text_edit.as_ref().unwrap().text, "שלום ");
        }
    }

    #[test]
    fn double_click_selects_one_unicode_word_in_rtl_greek_and_cyrillic() {
        for (text, selected) in [
            ("שלום עולם", "עולם"),
            ("άλφα βήτα", "βήτα"),
            ("один два", "два"),
        ] {
            let ctx = Context::default();
            let mut app = rtl_editing_app(&ctx, text);
            let state = app.text_edit.as_ref().unwrap();
            let layout = state.format.editor_layout(&state.text);
            let glyph = &layout.rows[0].glyphs[6];
            let (px, py) = state.format.text_padding();
            let point = app.canvas_rect.min
                + vec2(
                    state.origin.0 as f32 + px as f32 + glyph.x + glyph.advance * 0.45,
                    state.origin.1 as f32 + py as f32 + glyph.baseline - glyph.ascent * 0.5,
                );
            for (time, pressed) in [(1.0, true), (1.01, false), (1.1, true), (1.11, false)] {
                timed_input_frame(
                    &mut app,
                    &ctx,
                    vec![
                        Event::PointerMoved(point),
                        Event::PointerButton {
                            pos: point,
                            button: PointerButton::Primary,
                            pressed,
                            modifiers: Modifiers::NONE,
                        },
                    ],
                    time,
                );
            }
            let copied = timed_input_frame(&mut app, &ctx, vec![Event::Copy], 2.0);
            assert!(
                copied.platform_output.commands.iter().any(|command| {
                    matches!(command, egui::OutputCommand::CopyText(text) if text == selected)
                }),
                "Wrong double-click selection for {text:?}: {:?}",
                app.text_edit.as_ref().unwrap().selection
            );
        }
    }

    #[test]
    fn first_complex_input_frame_emits_the_visual_ime_caret() {
        let ctx = Context::default();
        let mut app = rtl_editing_app(&ctx, "");
        let output = timed_input_frame(&mut app, &ctx, vec![Event::Text("שלום".into())], 1.0);
        let state = app.text_edit.as_ref().unwrap();
        let caret = state.format.editor_layout(&state.text).caret(4);
        let (px, py) = state.format.text_padding();
        let expected = app.canvas_rect.min
            + vec2(
                state.origin.0 as f32 + px as f32 + caret.x,
                state.origin.1 as f32 + py as f32 + caret.y,
            );
        let actual = output
            .platform_output
            .ime
            .expect("Focused text supplies IME geometry")
            .cursor_rect
            .min;
        assert!(
            (actual - expected).length() < 1.0,
            "actual {actual:?}, expected {expected:?}"
        );
    }

    #[test]
    fn asynchronous_text_paste_restores_focus_and_preserves_multiline_history() {
        let ctx = Context::default();
        let original = "Aé\n猫B\nC";
        let mut app = editing_app(&ctx, original);
        let state = app.text_edit.as_mut().unwrap();
        state.selection = 1..6;
        state
            .format
            .modify_style(2..4, |style| style.bold = true)
            .unwrap();
        state.focus = true;
        timed_input_frame(&mut app, &ctx, vec![], 1.0);
        let original_format = app.text_edit.as_ref().unwrap().format.clone();
        ctx.memory_mut(|memory| memory.request_focus(Id::new("clipboard_ribbon_button")));
        assert!(!ctx.memory(|memory| memory.has_focus(Id::new("text_input"))));
        apply_text_clipboard(
            app.text_edit.as_mut().unwrap(),
            Action::Paste,
            Some("β\nγ"),
            &ctx,
        )
        .unwrap();
        timed_input_frame(&mut app, &ctx, vec![], 2.0);
        assert_eq!(app.text_edit.as_ref().unwrap().text, "Aβ\nγC");
        assert_eq!(app.text_edit.as_ref().unwrap().selection, 4..4);
        assert!(ctx.memory(|memory| memory.has_focus(Id::new("text_input"))));
        timed_input_frame(&mut app, &ctx, vec![key(Key::Z, Modifiers::CTRL)], 3.0);
        let state = app.text_edit.as_ref().unwrap();
        assert_eq!(state.text, original);
        assert_eq!(state.selection, 1..6);
        assert!(state.format == original_format);
        assert!(!app.text_can_undo());
    }

    #[test]
    fn ime_commit_is_one_undo_step_even_when_predictions_are_slow() {
        let ctx = Context::default();
        let mut app = editing_app(&ctx, "Before ");
        let state = app.text_edit.as_mut().unwrap();
        state.selection = 7..7;
        state.focus = true;
        timed_input_frame(&mut app, &ctx, vec![], 1.0);
        let original_format = app.text_edit.as_ref().unwrap().format.clone();
        for (time, event) in [
            (2.0, ImeEvent::Enabled),
            (3.0, ImeEvent::Preedit("に".into())),
            (4.0, ImeEvent::Preedit("にほ".into())),
        ] {
            let output = timed_input_frame(&mut app, &ctx, vec![Event::Ime(event)], time);
            assert!(output.platform_output.ime.is_some());
            assert!(
                !app.text_can_undo(),
                "Preedit must not enter typing history"
            );
        }
        timed_input_frame(
            &mut app,
            &ctx,
            vec![Event::Ime(ImeEvent::Commit("日本".into()))],
            5.0,
        );
        assert_eq!(app.text_edit.as_ref().unwrap().text, "Before 日本");
        timed_input_frame(&mut app, &ctx, vec![key(Key::Z, Modifiers::CTRL)], 6.0);
        let state = app.text_edit.as_ref().unwrap();
        assert_eq!(state.text, "Before ");
        assert!(state.format == original_format);
        assert_eq!(state.selection, 7..7);
        assert!(!app.text_can_undo());
        timed_input_frame(&mut app, &ctx, vec![key(Key::Y, Modifiers::CTRL)], 7.0);
        assert_eq!(app.text_edit.as_ref().unwrap().text, "Before 日本");
    }

    #[test]
    fn enabling_an_input_method_keeps_ordinary_typing_history_and_commit() {
        for keyboard_commit in [false, true] {
            let ctx = Context::default();
            ctx.set_os(egui::os::OperatingSystem::Mac);
            let mut app = editing_app(&ctx, "");
            timed_input_frame(&mut app, &ctx, vec![Event::Ime(ImeEvent::Enabled)], 1.0);
            timed_input_frame(&mut app, &ctx, vec![Event::Text("Abc".into())], 2.0);
            timed_input_frame(
                &mut app,
                &ctx,
                vec![key(Key::Backspace, Modifiers::NONE)],
                2.1,
            );
            assert_eq!(app.text_edit.as_ref().unwrap().text, "Ab");
            assert!(app
                .text_edit
                .as_ref()
                .unwrap()
                .history
                .composition
                .is_none());
            assert!(app.text_can_undo());
            timed_input_frame(
                &mut app,
                &ctx,
                vec![key(Key::Z, Modifiers::MAC_CMD | Modifiers::COMMAND)],
                3.0,
            );
            assert_eq!(app.text_edit.as_ref().unwrap().text, "");
            timed_input_frame(
                &mut app,
                &ctx,
                vec![key(Key::Y, Modifiers::MAC_CMD | Modifiers::COMMAND)],
                4.0,
            );
            assert_eq!(app.text_edit.as_ref().unwrap().text, "Ab");
            if keyboard_commit {
                timed_input_frame(&mut app, &ctx, vec![key(Key::Enter, Modifiers::CTRL)], 5.0);
                assert!(app.text_edit.is_none());
            } else {
                // Save and tool changes share this commit path.
                app.commit_text();
            }
            let ObjectKind::Text { text, .. } = &app.doc.objects[app.object.unwrap()].kind else {
                panic!("Confirmed typing must remain a text object");
            };
            assert_eq!(text, "Ab");
        }
    }

    #[test]
    fn coalesced_ime_compositions_preserve_each_committed_undo_boundary() {
        let ctx = Context::default();
        let mut app = editing_app(&ctx, "");
        timed_input_frame(
            &mut app,
            &ctx,
            vec![
                Event::Ime(ImeEvent::Enabled),
                Event::Ime(ImeEvent::Preedit("a".into())),
                Event::Ime(ImeEvent::Commit("亜".into())),
                Event::Ime(ImeEvent::Disabled),
                Event::Ime(ImeEvent::Enabled),
                Event::Ime(ImeEvent::Preedit("kan".into())),
                Event::Ime(ImeEvent::Commit("漢".into())),
                Event::Ime(ImeEvent::Disabled),
                key(Key::Z, Modifiers::CTRL),
            ],
            1.0,
        );
        for step in 0..5 {
            timed_input_frame(&mut app, &ctx, vec![], 1.1 + step as f64 * 0.1);
        }
        assert_eq!(app.text_edit.as_ref().unwrap().text, "亜");
        timed_input_frame(&mut app, &ctx, vec![key(Key::Z, Modifiers::CTRL)], 2.0);
        assert_eq!(app.text_edit.as_ref().unwrap().text, "");
        assert!(!app.text_can_undo());
    }

    #[test]
    fn confirmed_typing_before_same_frame_preedit_survives_cancel_and_undo() {
        for commit in [false, true] {
            let ctx = Context::default();
            let mut app = editing_app(&ctx, "");
            timed_input_frame(
                &mut app,
                &ctx,
                vec![
                    Event::Text("prefix".into()),
                    Event::Ime(ImeEvent::Enabled),
                    Event::Ime(ImeEvent::Preedit("に".into())),
                ],
                1.0,
            );
            for step in 0..3 {
                timed_input_frame(&mut app, &ctx, vec![], 1.1 + step as f64 * 0.1);
            }
            assert_eq!(app.text_edit.as_ref().unwrap().text, "prefixに");
            timed_input_frame(
                &mut app,
                &ctx,
                vec![Event::Ime(ImeEvent::Commit(if commit {
                    "日本".into()
                } else {
                    String::new()
                }))],
                2.0,
            );
            if commit {
                assert_eq!(app.text_edit.as_ref().unwrap().text, "prefix日本");
                timed_input_frame(&mut app, &ctx, vec![key(Key::Z, Modifiers::CTRL)], 3.0);
            }
            assert_eq!(app.text_edit.as_ref().unwrap().text, "prefix");
            timed_input_frame(&mut app, &ctx, vec![key(Key::Z, Modifiers::CTRL)], 4.0);
            assert_eq!(app.text_edit.as_ref().unwrap().text, "");
            assert!(!app.text_can_undo());
        }
    }

    #[test]
    fn ime_cancellation_preserves_the_open_box_and_its_exact_saved_object() {
        for ending in [
            vec![
                Event::Ime(ImeEvent::Preedit(String::new())),
                key(Key::Escape, Modifiers::NONE),
            ],
            vec![Event::Ime(ImeEvent::Commit(String::new()))],
            vec![Event::Ime(ImeEvent::Disabled)],
            vec![key(Key::Z, Modifiers::CTRL)],
        ] {
            let ctx = Context::default();
            let mut app = editing_app(&ctx, "Keep this caption");
            app.commit_text();
            app.doc.mark_saved();
            let object = app.object.unwrap();
            let saved = app.doc.objects[object].clone();
            let pixels = app.doc.composite();
            app.edit_text_object(object);
            timed_input_frame(&mut app, &ctx, vec![], 1.0);
            timed_input_frame(&mut app, &ctx, vec![Event::Ime(ImeEvent::Enabled)], 2.0);
            timed_input_frame(
                &mut app,
                &ctx,
                vec![Event::Ime(ImeEvent::Preedit("にほ".into()))],
                3.0,
            );
            timed_input_frame(&mut app, &ctx, ending, 4.0);
            for step in 0..3 {
                timed_input_frame(&mut app, &ctx, vec![], 4.1 + step as f64 * 0.1);
            }
            let state = app
                .text_edit
                .as_ref()
                .expect("Composition cancellation must keep editing");
            assert_eq!(state.text, "Keep this caption");
            assert!(state.history.composition.is_none());
            assert!(!app.text_can_undo());
            app.commit_text();
            assert!(app.doc.objects[object] == saved);
            assert_eq!(app.doc.composite(), pixels);
            assert!(!app.doc.dirty());
        }
    }

    #[test]
    fn ending_text_editing_does_not_save_an_unconfirmed_ime_prediction() {
        let ctx = Context::default();
        let mut app = editing_app(&ctx, "Confirmed caption");
        app.commit_text();
        app.doc.mark_saved();
        let object = app.object.unwrap();
        let saved = app.doc.objects[object].clone();
        app.edit_text_object(object);
        timed_input_frame(&mut app, &ctx, vec![], 1.0);
        timed_input_frame(&mut app, &ctx, vec![Event::Ime(ImeEvent::Enabled)], 2.0);
        timed_input_frame(
            &mut app,
            &ctx,
            vec![Event::Ime(ImeEvent::Preedit("draft".into()))],
            3.0,
        );
        app.commit_text();
        assert!(app.doc.objects[object] == saved);
        assert!(!app.doc.dirty());
    }

    #[test]
    fn native_command_formats_undoes_redoes_and_commits_selected_text() {
        for modifiers in [
            Modifiers::CTRL,
            Modifiers::CTRL | Modifiers::COMMAND,
            Modifiers::MAC_CMD | Modifiers::COMMAND,
        ] {
            let ctx = Context::default();
            if modifiers == Modifiers::CTRL || modifiers.mac_cmd {
                ctx.set_os(egui::os::OperatingSystem::Mac);
            }
            let mut app = editing_app(&ctx, "One two");
            app.text_edit.as_mut().unwrap().selection = 0..3;
            app.text_edit.as_mut().unwrap().focus = true;
            app_frame(&mut app, &ctx, vec![]);
            for keycode in [Key::B, Key::I, Key::U] {
                app_frame(&mut app, &ctx, vec![key(keycode, modifiers)]);
            }
            let format = &app.text_edit.as_ref().unwrap().format;
            assert!(
                format.style_at(0).bold
                    && format.style_at(0).italic
                    && format.style_at(0).underline
            );
            assert!(
                !format.style_at(4).bold
                    && !format.style_at(4).italic
                    && !format.style_at(4).underline
            );
            app_frame(&mut app, &ctx, vec![key(Key::Z, modifiers)]);
            assert!(!app.text_edit.as_ref().unwrap().format.style_at(0).underline);
            app_frame(
                &mut app,
                &ctx,
                vec![key(Key::Z, modifiers | Modifiers::SHIFT)],
            );
            assert!(app.text_edit.as_ref().unwrap().format.style_at(0).underline);
            app_frame(&mut app, &ctx, vec![key(Key::Enter, modifiers)]);
            assert!(app.text_edit.is_none());
            let ObjectKind::Text { text, format } = &app.doc.objects[app.object.unwrap()].kind
            else {
                panic!("expected editable text")
            };
            assert_eq!(text, "One two");
            assert!(format.style_at(0).underline);
        }
    }

    #[test]
    fn native_command_and_normalized_control_a_select_all_in_mac_text_fields() {
        for modifiers in [
            Modifiers::MAC_CMD | Modifiers::COMMAND,
            // egui-winit adds the logical command flag only to physical Ctrl+A.
            Modifiers::CTRL | Modifiers::COMMAND,
        ] {
            let ctx = Context::default();
            ctx.set_os(egui::os::OperatingSystem::Mac);
            let mut app = editing_app(&ctx, "One two");
            let state = app.text_edit.as_mut().unwrap();
            state.selection = 3..3;
            state.focus = true;
            app_frame(&mut app, &ctx, Vec::new());
            app_frame(
                &mut app,
                &ctx,
                vec![key(Key::A, modifiers), Event::Text("Replacement".into())],
            );
            assert_eq!(app.text_edit.as_ref().unwrap().text, "Replacement");
        }
    }

    #[test]
    fn backward_selection_keeps_its_anchor_after_formatting_and_undo() {
        let ctx = Context::default();
        let mut app = editing_app(&ctx, "Hello world");
        app_frame(&mut app, &ctx, vec![key(Key::End, Modifiers::NONE)]);
        for _ in 0..3 {
            app_frame(&mut app, &ctx, vec![key(Key::ArrowLeft, Modifiers::SHIFT)]);
        }
        assert_eq!(app.text_edit.as_ref().unwrap().selection, 8..11);
        app_frame(&mut app, &ctx, vec![key(Key::B, Modifiers::CTRL)]);
        app_frame(&mut app, &ctx, vec![key(Key::ArrowLeft, Modifiers::SHIFT)]);
        assert_eq!(app.text_edit.as_ref().unwrap().selection, 7..11);
        app_frame(&mut app, &ctx, vec![key(Key::Z, Modifiers::CTRL)]);
        assert_eq!(app.text_edit.as_ref().unwrap().selection, 8..11);
        app_frame(&mut app, &ctx, vec![key(Key::ArrowLeft, Modifiers::SHIFT)]);
        assert_eq!(app.text_edit.as_ref().unwrap().selection, 7..11);
        assert!(!app.text_edit.as_ref().unwrap().format.style_at(9).bold);
    }

    #[test]
    fn text_paragraph_navigation_preserves_shift_and_native_mac_commands() {
        for (os, modifier) in [
            (
                egui::os::OperatingSystem::Windows,
                Modifiers::CTRL | Modifiers::COMMAND,
            ),
            (egui::os::OperatingSystem::Mac, Modifiers::ALT),
        ] {
            let ctx = Context::default();
            ctx.set_os(os);
            let mut app = editing_app(&ctx, "One line\nSecond line\nThird line");
            let state = app.text_edit.as_mut().unwrap();
            state.selection = 4..4;
            state.focus = true;
            app_frame(&mut app, &ctx, Vec::new());
            app_frame(&mut app, &ctx, vec![key(Key::ArrowDown, modifier)]);
            assert_eq!(app.text_edit.as_ref().unwrap().selection, 9..9);
            app_frame(
                &mut app,
                &ctx,
                vec![key(Key::ArrowDown, modifier | Modifiers::SHIFT)],
            );
            assert_eq!(app.text_edit.as_ref().unwrap().selection, 9..21);
            app_frame(&mut app, &ctx, vec![key(Key::ArrowUp, modifier)]);
            assert_eq!(app.text_edit.as_ref().unwrap().selection, 9..9);
            if os == egui::os::OperatingSystem::Mac {
                app_frame(
                    &mut app,
                    &ctx,
                    vec![key(Key::ArrowDown, Modifiers::MAC_CMD | Modifiers::COMMAND)],
                );
                assert_eq!(app.text_edit.as_ref().unwrap().selection, 31..31);
            }
        }
    }

    #[test]
    fn page_navigation_moves_within_long_text_and_ordered_typing_stays_at_the_caret() {
        let ctx = Context::default();
        let text = (0..100)
            .map(|line| format!("Line {line}\n"))
            .collect::<String>();
        let mut app = editing_app(&ctx, &text);
        let state = app.text_edit.as_mut().unwrap();
        state.selection = 0..0;
        state.focus = true;
        app_frame(&mut app, &ctx, Vec::new());
        app_frame(&mut app, &ctx, vec![key(Key::PageDown, Modifiers::NONE)]);
        let caret = app.text_edit.as_ref().unwrap().selection.start;
        assert!(caret > 0 && caret < text.chars().count());
        app_frame(&mut app, &ctx, vec![key(Key::PageUp, Modifiers::SHIFT)]);
        assert_eq!(app.text_edit.as_ref().unwrap().selection, 0..caret);
        app_frame(
            &mut app,
            &ctx,
            vec![
                Event::Text("First\nSecond".into()),
                key(Key::ArrowUp, Modifiers::CTRL),
                Event::Text("!".into()),
            ],
        );
        for _ in 0..3 {
            app_frame(&mut app, &ctx, Vec::new());
        }
        assert!(app
            .text_edit
            .as_ref()
            .unwrap()
            .text
            .starts_with("First\n!Second"));
    }

    #[test]
    fn font_picker_includes_light_families_and_typing_embeds_an_emoji_fallback() {
        let ctx = Context::default();
        let mut app = editing_app(&ctx, "Hello");
        app.font_db
            .load_font_data(epaint_default_fonts::UBUNTU_LIGHT.to_vec());
        app.font_db
            .load_font_data(epaint_default_fonts::NOTO_EMOJI_REGULAR.to_vec());
        app.font_names = font_families(&app.font_db);
        assert!(app.font_names.iter().any(|(name, _)| name == "Ubuntu"));
        // DejaVu covers the grinning face already. Use an emoji absent from
        // the new regular default so this still exercises actual fallback.
        let default = ab_glyph::FontRef::try_from_slice(crate::text::DEFAULT_FONT).unwrap();
        let emoji =
            ab_glyph::FontRef::try_from_slice(epaint_default_fonts::NOTO_EMOJI_REGULAR).unwrap();
        assert!(!crate::text::font_supports_outline(&default, '🎨'));
        assert!(crate::text::font_supports_outline(&emoji, '🎨'));
        app_frame(&mut app, &ctx, vec![Event::Text("Hello 🎨".into())]);
        let state = app.text_edit.as_ref().unwrap();
        assert_eq!(state.text, "Hello 🎨");
        assert!(state
            .format
            .font_faces
            .iter()
            .any(|face| face.data.as_ref() == epaint_default_fonts::NOTO_EMOJI_REGULAR));
        let rendered = state.format.render("🎨");
        let mut without_fallback = state.format.clone();
        without_fallback.font_faces.clear();
        assert_ne!(rendered, without_fallback.render("🎨"));
        state.format.validate_for_text(&state.text).unwrap();
    }

    #[test]
    fn font_popup_grows_after_loading_families_and_paints_the_last_preview() {
        let ctx = Context::default();
        let mut app = editing_app(&ctx, "Hello");
        app.font_db
            .load_font_data(epaint_default_fonts::HACK_REGULAR.to_vec());
        app.font_names = font_families(&app.font_db);
        for code in [Key::F10, Key::T, Key::F, Key::L] {
            app_frame(&mut app, &ctx, vec![key(code, Modifiers::NONE)]);
        }
        for _ in 0..3 {
            app_frame(&mut app, &ctx, Vec::new());
        }
        for _ in 0..3 {
            app_frame(&mut app, &ctx, vec![key(Key::Escape, Modifiers::NONE)]);
        }
        app.font_db
            .load_font_data(epaint_default_fonts::UBUNTU_LIGHT.to_vec());
        app.font_db
            .load_font_data(epaint_default_fonts::NOTO_EMOJI_REGULAR.to_vec());
        app.font_names = font_families(&app.font_db);
        for code in [Key::F10, Key::T, Key::F, Key::L] {
            app_frame(&mut app, &ctx, vec![key(code, Modifiers::NONE)]);
        }
        for _ in 0..3 {
            app_frame(&mut app, &ctx, Vec::new());
        }
        let output = app_frame(&mut app, &ctx, Vec::new());
        let cache = ctx
            .data(|data| data.get_temp::<Vec<FontPreview>>(Id::new("paint10-font-previews")))
            .unwrap();
        let last = cache
            .iter()
            .find(|preview| preview.name == "Ubuntu")
            .expect("last newly loaded family is visible");
        let clip = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Mesh(mesh) if mesh.texture_id == last.texture.id() => {
                    Some(shape.clip_rect)
                }
                _ => None,
            })
            .expect("actual font preview is painted");
        assert!(clip.height() >= 24.0, "last preview clipped to {clip:?}");
        let layer = ctx.layer_id_at(clip.center()).unwrap();
        let popup = ctx.memory(|memory| memory.area_rect(layer.id)).unwrap();
        assert!(
            popup.bottom() - clip.bottom() < 12.0,
            "short font gallery has blank space below its last row: {popup:?}, {clip:?}"
        );
    }

    #[test]
    fn font_popup_scrolls_to_the_last_family_in_a_long_list() {
        let ctx = Context::default();
        let mut app = editing_app(&ctx, "Hello");
        app.font_db
            .load_font_data(epaint_default_fonts::HACK_REGULAR.to_vec());
        let face = app.font_db.faces().next().unwrap().id;
        app.font_names = (0..32)
            .map(|index| (format!("Family {index:02}"), face))
            .collect();
        for code in [Key::F10, Key::T, Key::F, Key::L] {
            app_frame(&mut app, &ctx, vec![key(code, Modifiers::NONE)]);
        }
        for _ in 0..3 {
            app_frame(&mut app, &ctx, Vec::new());
        }
        let output = app_frame(&mut app, &ctx, Vec::new());
        let first = text_position(&output, "Family 00");
        let layer = ctx.layer_id_at(first).unwrap();
        let popup = ctx.memory(|memory| memory.area_rect(layer.id)).unwrap();
        assert!(popup.height() <= 300.0, "long gallery must remain bounded");
        app_frame(
            &mut app,
            &ctx,
            vec![
                Event::PointerMoved(first),
                Event::MouseWheel {
                    unit: MouseWheelUnit::Point,
                    delta: vec2(0.0, -2000.0),
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        for _ in 0..30 {
            app_frame(&mut app, &ctx, Vec::new());
        }
        let output = app_frame(&mut app, &ctx, Vec::new());
        let cache = ctx
            .data(|data| data.get_temp::<Vec<FontPreview>>(Id::new("paint10-font-previews")))
            .unwrap();
        let last = cache
            .iter()
            .find(|preview| preview.name == "Family 31")
            .expect("scrolling reaches the last family");
        assert!(output.shapes.iter().any(|shape| {
            matches!(&shape.shape, Shape::Mesh(mesh)
                if mesh.texture_id == last.texture.id() && shape.clip_rect.height() >= 24.0)
        }));
    }

    #[test]
    fn project_font_registration_exposes_collections_without_changing_the_document() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let initial_font_count = app.font_db.faces().count();
        let collection = font_collection(&[
            epaint_default_fonts::UBUNTU_LIGHT,
            epaint_default_fonts::HACK_REGULAR,
        ]);
        let mut format = crate::text::TextFormat {
            font_name: "Hack".into(),
            font: collection.clone().into(),
            font_index: 1,
            font_faces: vec![crate::text::EmbeddedFont {
                family: "Noto Emoji".into(),
                data: epaint_default_fonts::NOTO_EMOJI_REGULAR.into(),
                index: 0,
                bold: false,
                italic: false,
            }],
            ..Default::default()
        };
        format
            .modify_style(0..1, |style| {
                style.font_name = "Ubuntu".into();
                style.font_index = 0;
            })
            .unwrap();
        app.doc.add_object(Object::new(
            ObjectKind::Text {
                text: "AB".into(),
                format,
            },
            (2, 2),
        ));
        app.doc.mark_saved();
        let before = crate::project::encode(&app.doc).unwrap();
        let pixels = app.doc.composite();
        app.register_document_fonts();
        let count = app.font_db.faces().count();
        assert_eq!(
            count,
            initial_font_count + 3,
            "a repeated collection is loaded once"
        );
        for name in ["Ubuntu", "Hack", "Noto Emoji"] {
            assert!(app.font_names.iter().any(|(family, _)| family == name));
        }
        let mut style = crate::text::TextFormat::default().default_style();
        assert!(app.apply_font_name("Hack", &mut style));
        assert_eq!(style.font.as_ref(), collection.as_slice());
        assert_eq!(style.font_index, 1);
        app.register_document_fonts();
        assert_eq!(app.font_db.faces().count(), count);
        assert_eq!(crate::project::encode(&app.doc).unwrap(), before);
        assert!(app.doc.composite() == pixels);
        assert!(!app.doc.dirty());
    }

    fn click(app: &mut PaintApp, ctx: &Context, point: Pos2) -> FullOutput {
        app_frame(
            app,
            ctx,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        app_frame(
            app,
            ctx,
            vec![Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        )
    }

    fn text_position(output: &FullOutput, label: &str) -> Pos2 {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if text.galley.job.text == label => {
                    Some(text.pos + text.galley.size() / 2.0)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("No visible control named {label:?}"))
    }

    fn state(text: &str) -> TextEditState {
        TextEditState {
            index: None,
            origin: (0, 0),
            text: text.into(),
            format: crate::text::TextFormat::default(),
            focus: false,
            selection: 0..0,
            insertion_style: None,
            history: TextHistory::default(),
            palette_colors: [BLACK, WHITE],
        }
    }

    #[test]
    fn selection_formatting_and_typing_style_have_separate_scopes() {
        let mut state = state("First second");
        state.selection = 0..5;
        change_style(&mut state, 0.0, |style| style.bold = true).unwrap();
        assert!(state.format.style_at(2).bold);
        assert!(!state.format.style_at(7).bold);
        state.selection = 12..12;
        change_style(&mut state, 1.0, |style| style.italic = true).unwrap();
        assert!(!state.format.style_at(7).italic);
        assert!(state.insertion_style.as_ref().unwrap().italic);
    }

    #[test]
    fn formatting_history_restores_text_and_styles_together() {
        let mut state = state("Hello");
        state.selection = 0..5;
        change_style(&mut state, 0.0, |style| style.bold = true).unwrap();
        let before = state.history.undo.pop().unwrap();
        before.restore(&mut state);
        assert_eq!(state.text, "Hello");
        assert!(!state.format.style_at(1).bold);
        assert_eq!(state.selection, 0..5);
    }

    #[test]
    fn font_size_accepts_separate_digit_events_without_typing_into_text() {
        let ctx = Context::default();
        let mut app = editing_app(&ctx, "Hello");
        let output = app_frame(&mut app, &ctx, vec![]);
        let size = text_position(&output, "18 pt");
        click(&mut app, &ctx, size);
        app_frame(&mut app, &ctx, vec![Event::Text("7".into())]);
        assert_ne!(
            ctx.memory(|memory| memory.focused()),
            Some(Id::new("text_input"))
        );
        app_frame(&mut app, &ctx, vec![Event::Text("2".into())]);
        assert_eq!(app.text_edit.as_ref().unwrap().text, "Hello");
        app_frame(&mut app, &ctx, vec![key(Key::Enter, Modifiers::NONE)]);
        let state = app.text_edit.as_ref().unwrap();
        assert_eq!(state.text, "Hello");
        assert_eq!(
            crate::text::pixels_to_points(state.format.style_at(0).size),
            72.0
        );
    }

    #[test]
    fn font_name_accepts_separate_typing_events_and_applies_a_matching_name() {
        let ctx = Context::default();
        let mut app = editing_app(&ctx, "First second");
        let collection = font_collection(&[
            epaint_default_fonts::UBUNTU_LIGHT,
            epaint_default_fonts::HACK_REGULAR,
        ]);
        app.font_db.load_font_data(collection.clone());
        app.font_names = app
            .font_db
            .faces()
            .map(|face| (face.families[0].0.clone(), face.id))
            .collect();
        app.text_edit.as_mut().unwrap().selection = 0..5;
        app.text_edit.as_mut().unwrap().focus = true;
        app_frame(&mut app, &ctx, vec![]);
        // Use the real key tip to select the editable font field.
        for input in [
            key(Key::F10, Modifiers::NONE),
            key(Key::T, Modifiers::NONE),
            key(Key::F, Modifiers::NONE),
            key(Key::F, Modifiers::NONE),
        ] {
            app_frame(&mut app, &ctx, vec![input]);
        }
        for character in ["h", "a", "c", "k"] {
            app_frame(&mut app, &ctx, vec![Event::Text(character.into())]);
            assert_eq!(
                ctx.memory(|memory| memory.focused()),
                Some(Id::new("paint10_font_name"))
            );
            assert_eq!(app.text_edit.as_ref().unwrap().text, "First second");
        }
        app_frame(&mut app, &ctx, vec![key(Key::Enter, Modifiers::NONE)]);
        let state = app.text_edit.as_ref().unwrap();
        assert_eq!(state.text, "First second");
        assert_eq!(state.format.style_at(0).font_name, "Hack");
        assert_eq!(
            state.format.style_at(0).font.as_ref(),
            collection.as_slice()
        );
        assert_eq!(state.format.style_at(0).font_index, 1);
        assert_eq!(
            state.format.style_at(7).font.as_ref(),
            crate::text::DEFAULT_FONT
        );
        let mut single_face = state.format.clone();
        single_face
            .modify_style(0..5, |style| {
                style.font = epaint_default_fonts::HACK_REGULAR.into();
                style.font_index = 0;
            })
            .unwrap();
        assert_eq!(
            state.format.render(&state.text),
            single_face.render(&state.text)
        );
        let output = app_frame(&mut app, &ctx, vec![]);
        text_position(&output, "Hack");
        app.commit_text();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("collection-face.p10");
        crate::project::save(&app.doc, &path).unwrap();
        let reopened = crate::project::load(&path).unwrap();
        assert!(app.doc.objects == reopened.objects);
        assert_eq!(app.doc.composite(), reopened.composite());
    }

    // Construct a real TTC from bundled test fonts. SFNT table offsets in a
    // collection are absolute, so each directory must be rebased after packing.
    fn font_collection(fonts: &[&[u8]]) -> Vec<u8> {
        let mut bytes = b"ttcf\0\x01\0\0".to_vec();
        bytes.extend_from_slice(&(fonts.len() as u32).to_be_bytes());
        bytes.resize(12 + fonts.len() * 4, 0);
        for (index, font) in fonts.iter().enumerate() {
            bytes.resize(bytes.len().next_multiple_of(4), 0);
            let offset = bytes.len();
            bytes[12 + index * 4..16 + index * 4].copy_from_slice(&(offset as u32).to_be_bytes());
            bytes.extend_from_slice(font);
            let tables = u16::from_be_bytes(font[4..6].try_into().unwrap()) as usize;
            for table in 0..tables {
                let field = 12 + table * 16 + 8;
                let value = u32::from_be_bytes(font[field..field + 4].try_into().unwrap());
                bytes[offset + field..offset + field + 4]
                    .copy_from_slice(&(value + offset as u32).to_be_bytes());
            }
        }
        bytes
    }

    #[test]
    fn control_enter_commits_without_inserting_a_newline() {
        let ctx = Context::default();
        let mut app = editing_app(&ctx, "Hello");
        app_frame(&mut app, &ctx, vec![key(Key::Enter, Modifiers::CTRL)]);
        assert!(app.text_edit.is_none());
        let selected = app.object.expect("The committed text remains selected");
        assert!(
            matches!(&app.doc.objects[selected].kind, ObjectKind::Text { text, .. } if text == "Hello")
        );
    }

    #[test]
    fn moving_existing_text_commits_its_position_and_undo_restores_it() {
        let ctx = Context::default();
        let mut app = editing_app(&ctx, "Hello");
        app.commit_text();
        let index = app.object.unwrap();
        let original = app.doc.objects[index].pos;
        app.edit_text_object(index);
        app.text_edit.as_mut().unwrap().origin = (70, 80);
        app.commit_text();
        assert_eq!(app.doc.objects[index].pos, (70, 80));
        app.doc.undo();
        assert_eq!(app.doc.objects[index].pos, original);
        assert!(
            matches!(&app.doc.objects[index].kind, ObjectKind::Text { text, .. } if text == "Hello")
        );
    }

    #[test]
    fn text_palette_changes_only_selected_characters_and_opaque_background() {
        let ctx = Context::default();
        let mut app = editing_app(&ctx, "First second");
        app.text_edit.as_mut().unwrap().selection = 0..5;
        app.text_edit.as_mut().unwrap().focus = true;
        app_frame(&mut app, &ctx, vec![]);
        // Locate the actual painted swatch instead of assuming ribbon coordinates.
        let output = app_frame(&mut app, &ctx, vec![]);
        let swatch = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Rect(rect)
                    if rect.fill == Color32::from_rgb(237, 28, 36) && rect.rect.width() < 20.0 =>
                {
                    Some(rect.rect.center())
                }
                _ => None,
            })
            .expect("The Text ribbon must contain the red palette swatch");
        click(&mut app, &ctx, swatch);
        let state = app.text_edit.as_ref().unwrap();
        assert_eq!(state.format.style_at(1).color, [237, 28, 36, 255]);
        assert_eq!(state.format.style_at(7).color, BLACK);
        app.text_edit.as_mut().unwrap().format.background = Some(WHITE);
        app.colors[1] = [0, 162, 232, 255];
        app_frame(&mut app, &ctx, vec![]);
        assert_eq!(
            app.text_edit.as_ref().unwrap().format.background,
            Some([0, 162, 232, 255])
        );
    }

    #[test]
    fn home_copy_and_cut_keep_the_text_editor_and_unicode_selection() {
        let ctx = Context::default();
        let mut app = editing_app(&ctx, "Héllo world");
        app.text_edit.as_mut().unwrap().selection = 1..5;
        app.text_edit.as_mut().unwrap().focus = true;
        app.text_tab = false;
        let output = app_frame(&mut app, &ctx, vec![]);
        let copy = text_position(&output, "Copy");
        let copied = click(&mut app, &ctx, copy);
        assert!(copied.platform_output.commands.iter().any(|command| {
            matches!(command, egui::OutputCommand::CopyText(text) if text == "éllo")
        }));
        assert_eq!(app.text_edit.as_ref().unwrap().text, "Héllo world");
        assert!(app.doc.objects.is_empty());
        let cut = text_position(&copied, "Cut");
        click(&mut app, &ctx, cut);
        assert_eq!(app.text_edit.as_ref().unwrap().text, "H world");
        assert!(app.doc.objects.is_empty());
        app_frame(&mut app, &ctx, vec![key(Key::Z, Modifiers::CTRL)]);
        assert_eq!(app.text_edit.as_ref().unwrap().text, "Héllo world");
    }

    #[test]
    fn text_ribbon_clipboard_preserves_unicode_selection_in_wide_and_narrow_windows() {
        for width in [1440.0, 500.0] {
            let ctx = Context::default();
            ctx.enable_accesskit();
            let mut app = editing_app(&ctx, "Héllo world");
            app.text_edit.as_mut().unwrap().selection = 1..5;
            app.text_edit.as_mut().unwrap().focus = true;
            let frame = |app: &mut PaintApp, events| app_frame_at_width(app, &ctx, events, width);
            let open_clipboard = |app: &mut PaintApp| {
                let mut output = frame(app, vec![]);
                if width < 600.0 {
                    let clipboard = text_position(&output, "Clipboard");
                    for pressed in [true, false] {
                        output = frame(
                            app,
                            vec![
                                Event::PointerMoved(clipboard),
                                Event::PointerButton {
                                    pos: clipboard,
                                    button: PointerButton::Primary,
                                    pressed,
                                    modifiers: Modifiers::NONE,
                                },
                            ],
                        );
                    }
                    for _ in 0..3 {
                        output = frame(app, vec![]);
                    }
                }
                output
            };
            frame(&mut app, vec![]);
            let output = open_clipboard(&mut app);
            let nodes = &output
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap()
                .nodes;
            for label in ["Paste", "Cut", "Copy"] {
                let node = nodes
                    .iter()
                    .find(|(_, node)| node.label() == Some(label))
                    .unwrap_or_else(|| panic!("{label} is missing at {width}px"));
                assert!(!node.1.is_disabled(), "{label} is disabled at {width}px");
            }
            let press = |point, pressed| Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            };
            let copy = text_position(&output, "Copy");
            frame(&mut app, vec![Event::PointerMoved(copy), press(copy, true)]);
            let copied = frame(&mut app, vec![press(copy, false)]);
            assert!(copied.platform_output.commands.iter().any(|command| {
                matches!(command, egui::OutputCommand::CopyText(text) if text == "éllo")
            }));
            frame(&mut app, vec![]);
            assert!(!keytips::popup_open(&ctx));
            let reopened = open_clipboard(&mut app);
            let cut = text_position(&reopened, "Cut");
            frame(&mut app, vec![Event::PointerMoved(cut), press(cut, true)]);
            frame(&mut app, vec![press(cut, false)]);
            assert_eq!(app.text_edit.as_ref().unwrap().text, "H world");
            assert!(app.doc.objects.is_empty());
            frame(&mut app, vec![]);
            assert!(!keytips::popup_open(&ctx));
            frame(&mut app, vec![key(Key::Z, Modifiers::CTRL)]);
            assert_eq!(app.text_edit.as_ref().unwrap().text, "Héllo world");
            assert_eq!(app.text_edit.as_ref().unwrap().selection, 1..5);
        }
    }

    #[test]
    fn text_ribbon_cut_keytip_does_not_activate_cancel() {
        let ctx = Context::default();
        let mut app = editing_app(&ctx, "Keep editing");
        for events in [
            vec![],
            vec![key(Key::F10, Modifiers::NONE)],
            vec![key(Key::T, Modifiers::NONE)],
            vec![key(Key::X, Modifiers::NONE)],
        ] {
            app_frame_at_width(&mut app, &ctx, events, 1440.0);
        }
        assert_eq!(app.text_edit.as_ref().unwrap().text, "");
        assert!(app.doc.objects.is_empty());
        assert!(!keytips::active(&ctx));
    }

    #[test]
    fn text_paste_preserves_formatting_and_is_undoable() {
        let ctx = Context::default();
        let mut app = editing_app(&ctx, "one two");
        let state = app.text_edit.as_mut().unwrap();
        state.selection = 0..3;
        change_style(state, 0.0, |style| style.bold = true).unwrap();
        apply_text_clipboard(state, Action::Paste, Some("é"), &ctx).unwrap();
        assert_eq!(state.text, "é two");
        assert!(state.format.style_at(0).bold);
        assert!(!state.format.style_at(3).bold);
        app_frame(&mut app, &ctx, vec![key(Key::Z, Modifiers::CTRL)]);
        assert_eq!(app.text_edit.as_ref().unwrap().text, "one two");
    }

    #[test]
    fn contextual_tabs_cycle_in_both_directions_without_closing_text() {
        let ctx = Context::default();
        let mut app = editing_app(&ctx, "Hello");
        app_frame(&mut app, &ctx, vec![key(Key::Tab, Modifiers::CTRL)]);
        assert!(!app.text_tab && !app.view_tab);
        app_frame(
            &mut app,
            &ctx,
            vec![key(Key::Tab, Modifiers::CTRL | Modifiers::SHIFT)],
        );
        assert!(app.text_tab);
        app_frame(
            &mut app,
            &ctx,
            vec![key(Key::Tab, Modifiers::CTRL | Modifiers::SHIFT)],
        );
        assert!(app.view_tab && !app.text_tab);
        assert_eq!(app.text_edit.as_ref().unwrap().text, "Hello");
    }

    #[test]
    fn text_keytips_activate_bold_on_the_selected_word() {
        let ctx = Context::default();
        let mut app = editing_app(&ctx, "Hello");
        for input in [
            key(Key::F10, Modifiers::NONE),
            key(Key::T, Modifiers::NONE),
            key(Key::B, Modifiers::NONE),
        ] {
            app_frame(&mut app, &ctx, vec![input]);
        }
        let state = app.text_edit.as_ref().unwrap();
        assert_eq!(state.text, "Hello");
        assert!(state.format.style_at(0).bold);
    }

    #[test]
    fn text_context_menu_contains_text_actions_without_image_operations() {
        let ctx = Context::default();
        let mut app = editing_app(&ctx, "Hello");
        app_frame(&mut app, &ctx, vec![key(Key::F10, Modifiers::SHIFT)]);
        app_frame(&mut app, &ctx, Vec::new());
        let output = app_frame(&mut app, &ctx, Vec::new());
        let visible: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) => Some(text.galley.job.text.as_str()),
                _ => None,
            })
            .collect();
        assert!(visible.contains(&"Select all"));
        assert!(!visible.contains(&"Invert colors"));
        assert!(!visible.contains(&"Crop"));
        assert_ne!(
            ctx.memory(|memory| memory.focused()),
            Some(Id::new("text_input"))
        );
        assert!(app.text_edit.is_some());
    }
}
