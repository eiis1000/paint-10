use super::*;
use crate::text::TextStyle as DocumentTextStyle;
use egui::text::{CCursor, CCursorRange};

#[derive(Default)]
pub(in crate::app) struct TextHistory {
    undo: Vec<TextSnapshot>,
    redo: Vec<TextSnapshot>,
    last_typing: Option<f64>,
    geometry: Option<TextSnapshot>,
}

#[derive(Clone)]
struct TextSnapshot {
    origin: Point,
    text: String,
    format: crate::text::TextFormat,
    selection: std::ops::Range<usize>,
    insertion_style: Option<DocumentTextStyle>,
}

#[derive(Clone)]
struct FontDraft {
    source: String,
    value: String,
    changed: bool,
}

impl TextSnapshot {
    fn capture(state: &TextEditState) -> Self {
        Self {
            origin: state.origin,
            text: state.text.clone(),
            format: state.format.clone(),
            selection: state.selection.clone(),
            insertion_style: state.insertion_style.clone(),
        }
    }

    fn restore(self, state: &mut TextEditState) {
        state.origin = self.origin;
        state.text = self.text;
        state.format = self.format;
        state.selection = self.selection;
        state.insertion_style = self.insertion_style;
        state.focus = true;
    }

    fn bytes(&self) -> usize {
        self.text.len()
            + self.format.memory_bytes()
            + self
                .insertion_style
                .as_ref()
                .map_or(0, |style| style.font.len())
    }
}

impl TextHistory {
    fn record(&mut self, before: TextSnapshot, time: f64, typing: bool) {
        let grouped = typing && self.last_typing.is_some_and(|last| time - last < 0.75);
        if !grouped {
            self.undo.push(before);
            let mut bytes: usize = self.undo.iter().map(TextSnapshot::bytes).sum();
            while bytes > 32 * 1024 * 1024 && self.undo.len() > 1 {
                bytes -= self.undo.remove(0).bytes();
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
        let pasted = if matches!(action, Action::Paste) {
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
        if let Some(state) = self.text_edit.take() {
            self.text_tab = false;
            let next_style = active_style(&state);
            self.text_format = state.format.clone();
            self.text_format.set_default_style(&next_style);
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
                self.tool = Tool::Select;
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

    pub(in crate::app) fn text_ribbon(&mut self, ui: &mut Ui, origin: Pos2, ctx: &Context) {
        let Some(mut state) = self.text_edit.take() else {
            return;
        };
        let original_style = active_style(&state);
        let mut style = original_style.clone();
        let mut done = false;
        let mut cancel = false;
        let groups = [
            ribbon_layout::Group {
                label: "Font",
                width: 311.0,
                icon: Icon::Tool(Tool::Text),
                keys: "ZF",
                popup: "text_font",
            },
            ribbon_layout::Group {
                label: "Background",
                width: 171.0,
                icon: Icon::Fill,
                keys: "ZB",
                popup: "text_background",
            },
            ribbon_layout::Group {
                label: "Colors",
                width: 347.0,
                icon: Icon::Colors,
                keys: "ZK",
                popup: "text_colors",
            },
            ribbon_layout::Group {
                label: "Caption",
                width: 181.0,
                icon: Icon::Tool(Tool::Text),
                keys: "ZP",
                popup: "text_caption",
            },
            ribbon_layout::Group {
                label: "Finish",
                width: 164.0,
                icon: Icon::Tool(Tool::Text),
                keys: "ZE",
                popup: "text_finish",
            },
        ];
        let widths =
            ribbon_layout::widths(&groups, ui.max_rect().right() - origin.x, &[2, 1, 4, 3, 0]);
        let mut x = origin.x;
        for (index, (group, width)) in groups.into_iter().zip(widths).enumerate() {
            ribbon_layout::show(ui, pos2(x, origin.y), width, "text", group, |ui, origin| {
                let origin = origin - vec2([0.0, 311.0, 482.0, 0.0, 829.0][index], 0.0);
                match index {
                    0 => {
                        Self::group(ui, origin, 0.0, 310.0, "Font");
                        ui.scope_builder(
                            UiBuilder::new().max_rect(Rect::from_min_size(
                                origin + vec2(12.0, 10.0),
                                vec2(286.0, 76.0),
                            )),
                            |ui| {
                                self.font_control(ui, &mut style);
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    let mut points = crate::text::pixels_to_points(style.size);
                                    let size = ui.add(
                                        DragValue::new(&mut points)
                                            .range(crate::text::FONT_POINT_RANGE)
                                            .update_while_editing(false)
                                            .suffix(" pt"),
                                    );
                                    text_control(ui, &size, "Font size", false);
                                    if size.changed() {
                                        style.size = crate::text::points_to_pixels(points);
                                    }
                                    for (label, glyph, selected) in [
                                        ("Bold", RichText::new("B").strong(), &mut style.bold),
                                        ("Italic", RichText::new("I").italics(), &mut style.italic),
                                        (
                                            "Underline",
                                            RichText::new("U").underline(),
                                            &mut style.underline,
                                        ),
                                        (
                                            "Strikeout",
                                            RichText::new("abc").strikethrough(),
                                            &mut style.strikeout,
                                        ),
                                    ] {
                                        let response = ui.toggle_value(selected, glyph);
                                        text_control(ui, &response, label, *selected);
                                    }
                                });
                            },
                        );
                    }
                    1 => {
                        Self::group(ui, origin, 311.0, 170.0, "Background");
                        ui.scope_builder(
                            UiBuilder::new().max_rect(Rect::from_min_size(
                                origin + vec2(323.0, 13.0),
                                vec2(148.0, 76.0),
                            )),
                            |ui| {
                                for (label, opaque) in [("Opaque", true), ("Transparent", false)] {
                                    let response = ui.selectable_label(
                                        state.format.background.is_some() == opaque,
                                        label,
                                    );
                                    text_control(
                                        ui,
                                        &response,
                                        label,
                                        state.format.background.is_some() == opaque,
                                    );
                                    if response.clicked()
                                        && state.format.background.is_some() != opaque
                                    {
                                        let before = TextSnapshot::capture(&state);
                                        state.format.background = opaque.then_some(self.colors[1]);
                                        state.history.record(
                                            before,
                                            ctx.input(|input| input.time),
                                            false,
                                        );
                                        state.focus = true;
                                    }
                                }
                            },
                        );
                    }
                    2 => self.colors_group_at(ui, origin, 482.0),
                    3 => self.caption_group(ui, origin, &mut state),
                    _ => {
                        Self::group(ui, origin, 829.0, 163.0, "Finish");
                        ui.scope_builder(
                            UiBuilder::new().max_rect(Rect::from_min_size(
                                origin + vec2(842.0, 12.0),
                                vec2(146.0, 76.0),
                            )),
                            |ui| {
                                ui.label("Select text to format it.");
                                ui.label("Click outside to finish.");
                                ui.horizontal(|ui| {
                                    done = ribbon_controls::command(ui, "Done").clicked();
                                    cancel = ribbon_controls::command(ui, "Cancel").clicked();
                                });
                            },
                        );
                    }
                }
            });
            x += width;
        }
        if original_style != style {
            if original_style.color != style.color {
                self.colors[0] = style.color;
            }
            let result = change_style(&mut state, ctx.input(|input| input.time), |selected| {
                if original_style.font != style.font
                    || original_style.font_name != style.font_name
                    || original_style.font_index != style.font_index
                {
                    selected.font.clone_from(&style.font);
                    selected.font_name.clone_from(&style.font_name);
                    selected.font_index = style.font_index;
                }
                if original_style.size != style.size {
                    selected.size = style.size;
                }
                if original_style.color != style.color {
                    selected.color = style.color;
                }
                if original_style.bold != style.bold {
                    selected.bold = style.bold;
                }
                if original_style.italic != style.italic {
                    selected.italic = style.italic;
                }
                if original_style.underline != style.underline {
                    selected.underline = style.underline;
                }
                if original_style.strikeout != style.strikeout {
                    selected.strikeout = style.strikeout;
                }
            });
            if let Err(error) = result {
                self.message = error;
            }
        }
        if cancel {
            self.refresh = true;
        } else {
            self.text_edit = Some(state);
            if done {
                self.commit_text();
            }
        }
    }

    fn caption_group(&mut self, ui: &mut Ui, origin: Pos2, state: &mut TextEditState) {
        use crate::text::TextAlignment;

        Self::group(ui, origin, 0.0, 180.0, "Caption");
        let before = TextSnapshot::capture(state);
        let original_format = state.format.clone();
        let max_outline =
            crate::text::MAX_TEXT_OUTLINE.min(state.format.width.saturating_sub(5) / 2);
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(
                origin + vec2(8.0, 8.0),
                vec2(165.0, 82.0),
            )),
            |ui| {
                ui.horizontal(|ui| {
                    for (label, value, keys) in [
                        ("Left", TextAlignment::Left, "NL"),
                        ("Center", TextAlignment::Center, "NC"),
                        ("Right", TextAlignment::Right, "NR"),
                    ] {
                        let response =
                            ui.selectable_value(&mut state.format.alignment, value, label);
                        ribbon_controls::register(ui, &response, keys, keytips::Kind::Button);
                    }
                });
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    let mut outlined = state.format.outline_width > 0;
                    let response = ui.checkbox(&mut outlined, "Outline");
                    ribbon_controls::register(ui, &response, "NO", keytips::Kind::Button);
                    if response.changed() {
                        state.format.outline_width = if outlined { 3.min(max_outline) } else { 0 };
                    }
                    let width = ui.add(
                        DragValue::new(&mut state.format.outline_width)
                            .range(0..=max_outline)
                            .update_while_editing(false)
                            .suffix(" px"),
                    );
                    ribbon_controls::register(ui, &width, "NW", keytips::Kind::NumericInput);
                    width.on_hover_text("Text outline width");
                });
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    for (label, color, keys) in [
                        ("Black", BLACK, "NB"),
                        ("White", WHITE, "NH"),
                        ("Color 1", self.colors[0], "NF"),
                    ] {
                        let response =
                            ui.selectable_label(state.format.outline_color == color, label);
                        ribbon_controls::register(ui, &response, keys, keytips::Kind::Button);
                        response
                            .clone()
                            .on_hover_text(format!("Use {label} for the text outline"));
                        if response.clicked() {
                            state.format.outline_color = color;
                            state.format.outline_width = state.format.outline_width.max(1);
                        }
                    }
                });
            },
        );
        if state.format != original_format {
            state
                .history
                .record(before, ui.input(|input| input.time), false);
            state.focus = true;
        }
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
                        .desired_width(235.0)
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
                    ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                        ui.set_min_width(235.0);
                        let mut matches = 0;
                        for name in std::iter::once("Sans serif")
                            .chain(self.font_names.iter().map(|(name, _)| name.as_str()))
                        {
                            if !name.to_lowercase().contains(&filter) {
                                continue;
                            }
                            matches += 1;
                            let choice = ui.selectable_label(style.font_name == name, name);
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
                self.message = format!("No usable installed font matches “{query}”.");
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
            style.font.clear();
            style.font_name = "Sans serif".into();
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
            .with_face_data(*id, |data, index| (data.to_vec(), index))
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

    pub(in crate::app) fn text_editor(&mut self, ctx: &Context) {
        let Some(mut state) = self.text_edit.take() else {
            ctx.data_mut(|data| data.remove::<Rect>(Id::new("paint10_text_editor_rect")));
            return;
        };
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
        let commit_requested = !popup_open
            && !modal_open
            && ctx.input_mut(|input| {
                super::shortcuts::consume_shortcut(input, Modifiers::CTRL, Key::Enter)
            });
        let insertion_style = active_style(&state);
        let input_id = Id::new("text_input");
        if first {
            let mut editor = TextEdit::load_state(ctx, input_id).unwrap_or_default();
            editor.cursor.set_char_range(Some(CCursorRange::two(
                CCursor::new(state.selection.start),
                CCursor::new(state.selection.end),
            )));
            editor.clear_undoer();
            editor.store(ctx, input_id);
        }
        let before = TextSnapshot::capture(&state);
        let original_text = state.text.clone();
        let original_format = state.format.clone();
        let zoom = self.zoom;
        let (padding_x, padding_y) = state.format.text_padding();
        let mut layouter = |ui: &Ui, text: &str, _width: f32| {
            let mut live_format = original_format.clone();
            if text != original_text {
                let _ = live_format.update_spans_for_edit_with_style(
                    &original_text,
                    text,
                    insertion_style.clone(),
                );
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
                ui.visuals_mut().selection.bg_fill =
                    Color32::from_rgba_unmultiplied(40, 140, 235, 85);
                let output = TextEdit::multiline(&mut state.text)
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
                if first {
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
                text_preview::paint(ui, picture, position, &state.text, &live_format, zoom);
                dashed_rect(ui.painter(), output.response.rect.expand(3.0));
                output
            })
            .inner;
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
                Ok(()) => state
                    .history
                    .record(before, ctx.input(|input| input.time), true),
                Err(error) => {
                    self.message = error;
                    before.restore(&mut state);
                }
            }
        }
        if let Some(range) = output.cursor_range {
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
        let done = !popup_open
            && !modal_open
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

fn apply_text_clipboard(
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
        ctx.run(
            RawInput {
                events,
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 720.0))),
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
        assert_eq!(state.format.style_at(0).font, collection);
        assert_eq!(state.format.style_at(0).font_index, 1);
        assert!(state.format.style_at(7).font.is_empty());
        let mut single_face = state.format.clone();
        single_face
            .modify_style(0..5, |style| {
                style.font = epaint_default_fonts::HACK_REGULAR.to_vec();
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
