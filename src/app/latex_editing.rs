//! Editable LaTeX source, with an explicit preview before changing the picture.

use super::*;
use crate::text::{TextAlignment, TextFormat};

#[cfg(test)]
mod tests;

pub(super) struct LatexDraft {
    original_editor: Option<TextEditState>,
    index: Option<usize>,
    origin: Point,
    source: String,
    format: TextFormat,
    preview: Option<LatexPreview>,
    initialize_editor: bool,
}

struct LatexPreview {
    source: String,
    format: TextFormat,
    result: Result<TextureHandle, String>,
    dimensions: (u32, u32),
}

impl LatexDraft {
    fn new(index: Option<usize>, origin: Point, source: String, mut format: TextFormat) -> Self {
        format.latex = true;
        format.spans.clear();
        Self {
            original_editor: None,
            index,
            origin,
            source,
            format,
            preview: None,
            initialize_editor: true,
        }
    }

    fn refresh_preview(&mut self, ctx: &Context) {
        if self
            .preview
            .as_ref()
            .is_some_and(|preview| preview.source == self.source && preview.format == self.format)
        {
            return;
        }
        let raster = if self.source.trim().is_empty() {
            Err("Enter an equation to preview it.".into())
        } else {
            crate::latex::render(&self.source, &self.format)
        };
        let mut dimensions = (0, 0);
        let result = raster.map(|raster| {
            dimensions = raster.dimensions();
            // A large equation must still fit the GPU's texture limit. This
            // smaller preview never replaces the equation's retained source.
            let limit = ctx.input(|input| input.max_texture_side).clamp(1, 1600) as u32;
            let preview = if raster.width().max(raster.height()) > limit {
                imageops::thumbnail(&raster, limit, limit)
            } else {
                raster
            };
            let pixels = display::image(
                [preview.width() as usize, preview.height() as usize],
                preview.as_raw(),
            );
            ctx.load_texture("latex-preview", pixels, TextureOptions::LINEAR)
        });
        self.preview = Some(LatexPreview {
            source: self.source.clone(),
            format: self.format.clone(),
            result,
            dimensions,
        });
    }
}

impl PaintApp {
    pub(super) fn open_latex_editor(&mut self) {
        if !self.ensure_active_layer_editable() || self.dialog.is_some() {
            return;
        }
        let Some(state) = self.text_edit.take() else {
            return;
        };
        let mut draft = LatexDraft::new(
            state.index,
            state.origin,
            state.text.clone(),
            state.format.clone(),
        );
        draft.original_editor = Some(state);
        self.latex_edit = Some(draft);
        self.dialog = Some(Dialog::Latex);
        self.dialog_error = None;
        self.refresh = true;
    }

    pub(super) fn edit_latex_object(&mut self, index: usize) {
        if !self.ensure_active_layer_editable() {
            return;
        }
        let Some(object) = self.doc.objects.get(index) else {
            return;
        };
        let ObjectKind::Text { text, format } = &object.kind else {
            return;
        };
        self.latex_edit = Some(LatexDraft::new(
            Some(index),
            object.pos,
            text.clone(),
            format.clone(),
        ));
        self.dialog = Some(Dialog::Latex);
        self.dialog_error = None;
    }

    pub(super) fn cancel_latex(&mut self) {
        if let Some(draft) = self.latex_edit.take() {
            self.text_edit = draft.original_editor;
            if let Some(state) = &mut self.text_edit {
                state.focus = true;
            }
            self.text_tab = self.text_edit.is_some();
            self.refresh = true;
        }
    }

    fn apply_latex(&mut self) -> Result<(), String> {
        let draft = self
            .latex_edit
            .as_ref()
            .ok_or("No equation is being edited.")?;
        if draft.source.trim().is_empty() {
            return Err("Enter an equation before applying it.".into());
        }
        if !self.doc.active_layer().visible || self.doc.active_layer().locked {
            return Err("The active layer must be visible and unlocked.".into());
        }
        draft.format.validate_for_text(&draft.source)?;
        if draft.index.is_none() {
            let objects = self
                .doc
                .layers()
                .iter()
                .map(|layer| layer.objects.len())
                .sum::<usize>();
            let raster = usize::from(self.doc.image.pixels().any(|pixel| pixel[3] > 0));
            if objects + raster + 1 > d::MAX_OBJECTS {
                return Err("The picture has reached its editable object limit.".into());
            }
        }
        let draft = self.latex_edit.take().expect("validated equation draft");
        self.colors[0] = draft.format.color;
        if let Some(background) = draft.format.background {
            self.colors[1] = background;
        }
        self.text_edit = Some(TextEditState {
            index: draft.index,
            origin: draft.origin,
            text: draft.source,
            format: draft.format,
            focus: false,
            selection: 0..0,
            insertion_style: None,
            history: Default::default(),
            palette_colors: self.colors,
        });
        self.commit_text();
        self.message = "Equation applied".into();
        Ok(())
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(super) fn latex_has_unsaved_changes(&self) -> bool {
        self.latex_edit.as_ref().is_some_and(|draft| {
            if draft.original_editor.is_some() {
                return !draft.source.is_empty();
            }
            !draft
                .index
                .and_then(|index| self.doc.objects.get(index))
                .is_some_and(|object| {
                    matches!(&object.kind, ObjectKind::Text { text, format }
                    if text == &draft.source && format == &draft.format)
                })
        })
    }

    pub(super) fn latex_dialog(&mut self, ui: &mut Ui) -> bool {
        let Some(mut draft) = self.latex_edit.take() else {
            return true;
        };
        let apply_shortcut = !keytips::popup_open(ui.ctx())
            && ui
                .ctx()
                .input_mut(|input| shortcuts::consume_shortcut(input, Modifiers::CTRL, Key::Enter));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.strong("Source");
            ComboBox::from_id_salt("latex_examples")
                .selected_text("Examples")
                .width(130.0)
                .show_ui(ui, |ui| {
                    theme::menu(ui);
                    for (label, source) in [
                        (
                            "Fraction and root",
                            r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}",
                        ),
                        ("Sum", r"\sum_{k=1}^{n} k = \frac{n(n+1)}{2}"),
                        (
                            "Integral",
                            r"\int_0^\infty e^{-x^2}\,dx = \frac{\sqrt{\pi}}{2}",
                        ),
                        ("Matrix", r"\begin{pmatrix}a & b \\ c & d\end{pmatrix}"),
                    ] {
                        if ui.add(theme::MenuItem::new(label)).clicked() {
                            draft.source = source.into();
                        }
                    }
                })
                .response
                .on_hover_text("Replace the draft with a sample equation");
        });
        let source_id = Id::new("paint10-latex-source");
        if draft.initialize_editor && ui.is_enabled() && !ui.is_sizing_pass() {
            TextEdit::store_state(ui.ctx(), source_id, Default::default());
            draft.initialize_editor = false;
        }
        let source = ui.add_sized(
            [ui.available_width(), 96.0],
            TextEdit::multiline(&mut draft.source)
                .id(source_id)
                .font(TextStyle::Monospace)
                .desired_rows(4)
                .char_limit(crate::latex::MAX_SOURCE_BYTES)
                .hint_text(r"E = mc^2"),
        );
        source.widget_info(|| WidgetInfo::labeled(WidgetType::TextEdit, true, "LaTeX source"));
        dialogs::initial_focus(ui, &source);

        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("Size");
            let mut points = crate::text::pixels_to_points(draft.format.size);
            let size = dialogs::numeric_input(
                ui,
                DragValue::new(&mut points)
                    .range(crate::text::FONT_POINT_RANGE)
                    .suffix(" pt")
                    .update_while_editing(false),
            );
            size.widget_info(|| WidgetInfo::labeled(WidgetType::DragValue, true, "Equation size"));
            if size.changed() {
                draft.format.size = crate::text::points_to_pixels(points);
            }
            ui.label("Color");
            ui.color_edit_button_srgba_unmultiplied(&mut draft.format.color);
            let mut opaque = draft.format.background.is_some();
            if ui.checkbox(&mut opaque, "Opaque background").changed() {
                draft.format.background = opaque.then_some(self.colors[1]);
            }
            if let Some(color) = &mut draft.format.background {
                ui.color_edit_button_srgba_unmultiplied(color);
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Align");
            ComboBox::from_id_salt("latex_alignment")
                .width(74.0)
                .selected_text(match draft.format.alignment {
                    TextAlignment::Left => "Left",
                    TextAlignment::Center => "Center",
                    TextAlignment::Right => "Right",
                })
                .show_ui(ui, |ui| {
                    for (value, label) in [
                        (TextAlignment::Left, "Left"),
                        (TextAlignment::Center, "Center"),
                        (TextAlignment::Right, "Right"),
                    ] {
                        ui.selectable_value(&mut draft.format.alignment, value, label);
                    }
                });
            ui.label("Outline");
            dialogs::numeric_input(
                ui,
                DragValue::new(&mut draft.format.outline_width)
                    .range(0..=crate::text::MAX_TEXT_OUTLINE)
                    .suffix(" px")
                    .update_while_editing(false),
            );
            ui.color_edit_button_srgba_unmultiplied(&mut draft.format.outline_color);
        });
        draft.refresh_preview(ui.ctx());
        ui.add_space(6.0);
        ui.strong("Preview");
        let (panel, _) = ui.allocate_exact_size(vec2(ui.available_width(), 112.0), Sense::hover());
        canvas::checkerboard(ui.painter(), panel, 12.0);
        let preview = draft
            .preview
            .as_ref()
            .expect("equation preview initialized");
        let valid = preview.result.is_ok();
        match &preview.result {
            Ok(texture) => {
                let available = panel.shrink(10.0);
                let size = texture.size_vec2();
                let scale = (available.width() / size.x)
                    .min(available.height() / size.y)
                    .min(1.0);
                ui.painter().image(
                    texture.id(),
                    Rect::from_center_size(available.center(), size * scale),
                    Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                ui.small(format!(
                    "{} × {} pixels",
                    preview.dimensions.0, preview.dimensions.1
                ));
            }
            Err(error) => {
                ui.add(Label::new(RichText::new(error).color(ui.visuals().error_fg_color)).wrap());
            }
        }
        let mut apply = false;
        let mut cancel = false;
        dialogs::dialog_actions(ui, &["Apply", "Cancel"], |ui| {
            ui.add_enabled_ui(valid, |ui| {
                apply = dialogs::dialog_button(ui, "Apply");
            });
            cancel = dialogs::dialog_button(ui, "Cancel");
        });
        self.latex_edit = Some(draft);
        if cancel {
            self.cancel_latex();
            return true;
        }
        if apply || (valid && apply_shortcut) {
            match self.apply_latex() {
                Ok(()) => return true,
                Err(error) => self.dialog_error = Some(error),
            }
        }
        false
    }
}
