use super::*;
use crate::text::{TextStyle as DocumentTextStyle, TextStyleRef};
use egui::text::{CCursor, CCursorRange};
use std::hash::{Hash, Hasher};

#[derive(Default)]
pub(in crate::app) struct TextHistory {
    undo: Vec<TextSnapshot>,
    redo: Vec<TextSnapshot>,
    last_typing: Option<f64>,
}

#[derive(Clone)]
struct TextSnapshot {
    text: String,
    format: crate::text::TextFormat,
    selection: std::ops::Range<usize>,
    insertion_style: Option<DocumentTextStyle>,
}

impl TextSnapshot {
    fn capture(state: &TextEditState) -> Self {
        Self {
            text: state.text.clone(),
            format: state.format.clone(),
            selection: state.selection.clone(),
            insertion_style: state.insertion_style.clone(),
        }
    }

    fn restore(self, state: &mut TextEditState) {
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
    pub(in crate::app) fn edit_text_object(&mut self, index: usize) {
        if let ObjectKind::Text { text, format } = &self.doc.objects[index].kind {
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
        if ctx.memory(|memory| memory.any_popup_open()) {
            return;
        }
        let Some(mut state) = self.text_edit.take() else {
            return;
        };
        let time = ctx.input(|input| input.time);
        let redo = ctx.input_mut(|input| {
            input.consume_key(Modifiers::CTRL, Key::Y)
                || input.consume_key(Modifiers::CTRL | Modifiers::SHIFT, Key::Z)
        });
        let undo = !redo && ctx.input_mut(|input| input.consume_key(Modifiers::CTRL, Key::Z));
        if undo || redo {
            let snapshot = if redo {
                state.history.redo.pop()
            } else {
                state.history.undo.pop()
            };
            if let Some(snapshot) = snapshot {
                let current = TextSnapshot::capture(&state);
                if redo {
                    state.history.undo.push(current);
                } else {
                    state.history.redo.push(current);
                }
                snapshot.restore(&mut state);
                state.history.last_typing = None;
            }
        }
        for key in [Key::B, Key::I, Key::U] {
            if ctx.input_mut(|input| input.consume_key(Modifiers::CTRL, key)) {
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
        Self::group(ui, origin, 0.0, 310.0, "Font");
        Self::group(ui, origin, 311.0, 170.0, "Background");
        Self::group(ui, origin, 482.0, 170.0, "Colors");
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(
                origin + vec2(12.0, 10.0),
                vec2(286.0, 76.0),
            )),
            |ui| {
                ComboBox::from_id_salt("text_font")
                    .width(270.0)
                    .selected_text(&style.font_name)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(style.font.is_empty(), "Sans serif")
                            .clicked()
                        {
                            style.font.clear();
                            style.font_name = "Sans serif".into();
                        }
                        for (name, id) in &self.font_names {
                            if ui
                                .selectable_label(style.font_name == *name, name)
                                .clicked()
                            {
                                if let Some(data) =
                                    self.font_db.with_face_data(*id, |data, _| data.to_vec())
                                {
                                    if ab_glyph::FontRef::try_from_slice(&data).is_ok() {
                                        style.font = data;
                                        style.font_name = name.clone();
                                    }
                                }
                            }
                        }
                    });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let mut points = crate::text::pixels_to_points(style.size);
                    if ui
                        .add(
                            DragValue::new(&mut points)
                                .range(crate::text::FONT_POINT_RANGE)
                                .suffix(" pt"),
                        )
                        .changed()
                    {
                        style.size = crate::text::points_to_pixels(points);
                    }
                    ui.toggle_value(&mut style.bold, RichText::new("B").strong());
                    ui.toggle_value(&mut style.italic, RichText::new("I").italics());
                    ui.toggle_value(&mut style.underline, RichText::new("U").underline());
                    ui.toggle_value(&mut style.strikeout, RichText::new("abc").strikethrough());
                });
            },
        );
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(
                origin + vec2(323.0, 13.0),
                vec2(148.0, 76.0),
            )),
            |ui| {
                for (label, opaque) in [("Opaque", true), ("Transparent", false)] {
                    if ui
                        .selectable_label(state.format.background.is_some() == opaque, label)
                        .clicked()
                        && state.format.background.is_some() != opaque
                    {
                        let before = TextSnapshot::capture(&state);
                        state.format.background = opaque.then_some(self.colors[1]);
                        state
                            .history
                            .record(before, ctx.input(|input| input.time), false);
                        state.focus = true;
                    }
                }
            },
        );
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(
                origin + vec2(495.0, 13.0),
                vec2(148.0, 76.0),
            )),
            |ui| {
                ui.horizontal(|ui| {
                    ui.label("Text");
                    let mut rgb = [style.color[0], style.color[1], style.color[2]];
                    if ui.color_edit_button_srgb(&mut rgb).changed() {
                        style.color = [rgb[0], rgb[1], rgb[2], 255];
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Background");
                    let mut rgb = [self.colors[1][0], self.colors[1][1], self.colors[1][2]];
                    if ui.color_edit_button_srgb(&mut rgb).changed() {
                        self.colors[1] = [rgb[0], rgb[1], rgb[2], 255];
                        if state.format.background.is_some() {
                            let before = TextSnapshot::capture(&state);
                            state.format.background = Some(self.colors[1]);
                            state
                                .history
                                .record(before, ctx.input(|input| input.time), false);
                        }
                    }
                });
            },
        );
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(
                origin + vec2(669.0, 12.0),
                vec2(320.0, 76.0),
            )),
            |ui| {
                ui.label("Select text to format it. Click outside to finish.");
                ui.label("Double-click with Select to edit again.");
                ui.horizontal(|ui| {
                    done = ui.button("Done").clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            },
        );
        if original_style != style {
            if original_style.color != style.color {
                self.colors[0] = style.color;
            }
            let result = change_style(&mut state, ctx.input(|input| input.time), |selected| {
                if original_style.font != style.font || original_style.font_name != style.font_name
                {
                    selected.font.clone_from(&style.font);
                    selected.font_name.clone_from(&style.font_name);
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

    pub(in crate::app) fn text_editor(&mut self, ctx: &Context) {
        let Some(mut state) = self.text_edit.take() else {
            return;
        };
        let position =
            self.canvas_rect.min + vec2(state.origin.0 as f32, state.origin.1 as f32) * self.zoom;
        let popup_open = ctx.memory(|memory| memory.any_popup_open());
        let first = state.focus && !popup_open;
        let insertion_style = active_style(&state);
        register_text_fonts(ctx, &state.format, &insertion_style);
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
        let mut layouter = |ui: &Ui, text: &str, width: f32| {
            let mut live_format = original_format.clone();
            if text != original_text {
                let _ = live_format.update_spans_for_edit_with_style(
                    &original_text,
                    text,
                    insertion_style.clone(),
                );
            }
            let job = text_job(
                ctx,
                text,
                &live_format,
                insertion_style.as_ref(),
                width,
                zoom,
                false,
            );
            ui.fonts(|fonts| fonts.layout_job(job))
        };
        let output = Area::new(Id::new("inline_text"))
            .order(Order::Foreground)
            .fixed_pos(position)
            .constrain(false)
            .show(ctx, |ui| {
                ui.set_clip_rect(self.canvas_rect.intersect(ctx.screen_rect()));
                if let Some(background) = state.format.background {
                    ui.painter().rect_filled(
                        Rect::from_min_size(
                            position,
                            vec2(
                                state.format.width as f32 * zoom,
                                state.format.dimensions(&state.text).1 as f32 * zoom,
                            ),
                        ),
                        0.0,
                        Color32::from_rgb(background[0], background[1], background[2]),
                    );
                }
                let output = TextEdit::multiline(&mut state.text)
                    .id(input_id)
                    .layouter(&mut layouter)
                    .frame(false)
                    .margin(0.0)
                    .char_limit(16000)
                    .desired_width(state.format.width as f32 * zoom)
                    .desired_rows(2)
                    .show(ui);
                if first {
                    output.response.request_focus();
                }
                if state.format.bold
                    || state.format.spans.iter().any(|span| span.style.bold)
                    || insertion_style.bold
                {
                    let mut live_format = original_format.clone();
                    let _ = live_format.update_spans_for_edit_with_style(
                        &original_text,
                        &state.text,
                        insertion_style.clone(),
                    );
                    let job = text_job(
                        ctx,
                        &state.text,
                        &live_format,
                        insertion_style.as_ref(),
                        output.response.rect.width(),
                        zoom,
                        true,
                    );
                    let bold = ui.fonts(|fonts| fonts.layout_job(job));
                    ui.painter()
                        .galley(output.galley_pos + vec2(zoom, 0.0), bold, Color32::WHITE);
                }
                dashed_rect(ui.painter(), output.response.rect.expand(3.0));
                output
            })
            .inner;
        state.focus &= popup_open;
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
            }
        }
        let done = !popup_open
            && (ctx.input(|input| input.modifiers.ctrl && input.key_pressed(Key::Enter))
                || (!first
                    && ctx.input(|input| input.pointer.any_pressed())
                    && ctx
                        .input(|input| input.pointer.interact_pos())
                        .is_some_and(|point| {
                            self.canvas_rect.contains(point)
                                && !output.response.rect.expand(6.0).contains(point)
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

fn font_name(bytes: &[u8]) -> String {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hash);
    format!("paint-text-{:016x}", hash.finish())
}

fn register_text_fonts(
    ctx: &Context,
    format: &crate::text::TextFormat,
    insertion: &DocumentTextStyle,
) {
    let mut sources = std::collections::BTreeMap::new();
    for style in std::iter::once(format.default_style_ref())
        .chain(format.spans.iter().map(|span| span.style.as_ref()))
        .chain(std::iter::once(insertion.as_ref()))
    {
        let bytes = style.font_bytes();
        sources.entry(font_name(bytes)).or_insert(bytes);
    }
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    sources.keys().for_each(|key| key.hash(&mut hash));
    let signature = hash.finish();
    let cache_id = Id::new("paint-text-font-signature");
    if ctx.data(|data| data.get_temp::<u64>(cache_id)) == Some(signature) {
        return;
    }
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in sources {
        fonts.font_data.insert(
            name.clone(),
            std::sync::Arc::new(FontData::from_owned(bytes.to_vec())),
        );
        fonts
            .families
            .insert(FontFamily::Name(name.clone().into()), vec![name]);
    }
    ctx.set_fonts(fonts);
    ctx.data_mut(|data| data.insert_temp(cache_id, signature));
    ctx.request_repaint();
}

fn text_job(
    ctx: &Context,
    text: &str,
    format: &crate::text::TextFormat,
    insertion: TextStyleRef<'_>,
    width: f32,
    zoom: f32,
    bold_overlay: bool,
) -> egui::text::LayoutJob {
    let offsets: Vec<_> = text
        .char_indices()
        .map(|(offset, _)| offset)
        .chain(std::iter::once(text.len()))
        .collect();
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = width;
    let runs = if text.is_empty() {
        vec![(0..0, insertion)]
    } else {
        format.style_runs(0..offsets.len() - 1)
    };
    for (range, style) in runs {
        let family = FontFamily::Name(font_name(style.font_bytes()).into());
        let family = if ctx.fonts(|fonts| fonts.families().contains(&family)) {
            family
        } else {
            FontFamily::Proportional
        };
        let color = if bold_overlay && !style.bold {
            Color32::TRANSPARENT
        } else {
            Color32::from_rgba_unmultiplied(
                style.color[0],
                style.color[1],
                style.color[2],
                style.color[3],
            )
        };
        job.append(
            &text[offsets[range.start]..offsets[range.end]],
            0.0,
            egui::TextFormat {
                font_id: FontId::new(style.size * zoom, family),
                color,
                italics: style.italic,
                underline: if style.underline && !bold_overlay {
                    Stroke::new(zoom.max(1.0), color)
                } else {
                    Stroke::NONE
                },
                strikethrough: if style.strikeout && !bold_overlay {
                    Stroke::new(zoom.max(1.0), color)
                } else {
                    Stroke::NONE
                },
                ..Default::default()
            },
        );
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
