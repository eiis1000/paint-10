//! Contextual text controls, grouped like Paint with compact editing additions.

use super::*;
use crate::text::TextAlignment;

const FONT_SIZES: [f32; 22] = [
    6.0, 8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0, 28.0, 36.0, 48.0,
    60.0, 72.0, 96.0, 120.0, 144.0, 200.0,
];

impl PaintApp {
    pub(in crate::app) fn text_ribbon(&mut self, ui: &mut Ui, origin: Pos2, ctx: &Context) {
        if self.text_edit.is_none() {
            return;
        }
        let groups = [
            ribbon_layout::Group {
                label: "Clipboard",
                width: 120.0,
                icon: Icon::Paste,
                keys: "ZC",
                popup: "text_clipboard",
            },
            ribbon_layout::Group {
                label: "Font",
                width: 222.0,
                icon: Icon::Tool(Tool::Text),
                keys: "ZF",
                popup: "text_font",
            },
            ribbon_layout::Group {
                label: "Background",
                width: 122.0,
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
                label: "Paragraph",
                width: 94.0,
                icon: Icon::Tool(Tool::Text),
                keys: "ZP",
                popup: "text_paragraph",
            },
            ribbon_layout::Group {
                label: "Effects",
                width: 158.0,
                icon: Icon::Outline,
                keys: "ZX",
                popup: "text_effects",
            },
            ribbon_layout::Group {
                label: "Editing",
                width: 86.0,
                icon: Icon::Tool(Tool::Text),
                keys: "ZE",
                popup: "text_finish",
            },
        ];
        let widths = ribbon_layout::widths(
            &groups,
            ui.max_rect().right() - origin.x,
            &[6, 5, 4, 2, 3, 1, 0],
        );

        // Copy, Cut and Paste need the live character selection.
        ribbon_layout::show(ui, origin, widths[0], "text", groups[0], |ui, origin| {
            self.clipboard_group(ui, origin, ctx);
        });
        let Some(mut state) = self.text_edit.take() else {
            return;
        };
        let original_style = active_style(&state);
        let mut style = original_style.clone();
        let mut done = false;
        let mut cancel = false;
        let mut latex = false;
        let mut x = origin.x + widths[0];
        for (index, (group, width)) in groups.into_iter().zip(widths).enumerate().skip(1) {
            ribbon_layout::show(ui, pos2(x, origin.y), width, "text", group, |ui, origin| {
                if index == 3 {
                    self.colors_group_at(ui, origin, 0.0);
                    return;
                }
                Self::group(ui, origin, 0.0, group.width - 1.0, group.label);
                ui.scope_builder(
                    UiBuilder::new().max_rect(Rect::from_min_size(
                        origin + vec2(8.0, 8.0),
                        vec2(group.width - 16.0, 82.0),
                    )),
                    |ui| {
                        ui.spacing_mut().item_spacing = vec2(3.0, 5.0);
                        ui.spacing_mut().button_padding = vec2(4.0, 3.0);
                        match index {
                            1 => self.text_font_group(ui, &mut style),
                            2 => self.text_background_group(ui, &mut state),
                            4 => Self::text_paragraph_group(ui, &mut state),
                            5 => self.text_effects_group(ui, &mut state),
                            _ => {
                                ui.spacing_mut().item_spacing.y = 3.0;
                                let response = ui.add_sized([66.0, 25.0], Button::new("Done"));
                                ribbon_controls::named(ui, &response, "Done");
                                done = response.clicked();
                                response.on_hover_text("Finish this text box (Ctrl+Enter)");
                                let response = ui.add_sized([66.0, 25.0], Button::new("Cancel"));
                                ribbon_controls::register(
                                    ui,
                                    &response,
                                    "Q",
                                    keytips::Kind::Button,
                                );
                                cancel = response.clicked();
                                response.on_hover_text("Discard changes to this text box (Escape)");
                                let response = ui.add_sized([66.0, 25.0], Button::new("LaTeX…"));
                                ribbon_controls::register(
                                    ui,
                                    &response,
                                    "LX",
                                    keytips::Kind::Button,
                                );
                                latex = response.clicked();
                                response.on_hover_text(
                                    "Compile this box as an editable LaTeX equation",
                                );
                            }
                        }
                    },
                );
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
            } else if latex {
                self.open_latex_editor();
            }
        }
    }

    fn text_font_group(&mut self, ui: &mut Ui, style: &mut DocumentTextStyle) {
        self.font_control(ui, style);
        ui.horizontal(|ui| {
            let mut points = crate::text::pixels_to_points(style.size);
            let original_points = points;
            let size = ui.add_sized(
                [62.0, 22.0],
                DragValue::new(&mut points)
                    .range(crate::text::FONT_POINT_RANGE)
                    .update_while_editing(false)
                    .suffix(" pt"),
            );
            text_control(ui, &size, "Font size", false);
            let list = ribbon::ribbon_menu_button(ui, "", "FP", "font_sizes", None, |ui| {
                ScrollArea::vertical()
                    .drag_to_scroll(false)
                    .max_height(300.0)
                    .show(ui, |ui| {
                        for (index, value) in FONT_SIZES.into_iter().enumerate() {
                            let response = ui.add(
                                theme::MenuItem::new(&format!("{value} pt"))
                                    .width(90.0)
                                    .selected((points - value).abs() < 0.01),
                            );
                            ribbon_controls::register(
                                ui,
                                &response,
                                &format!("{:02}", index + 1),
                                keytips::Kind::Button,
                            );
                            if response.clicked() {
                                points = value;
                                ui.close_menu();
                            }
                        }
                    });
            });
            list.response
                .widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, true, "Font size list"));
            list.response.on_hover_text("Choose a font size");
            for (label, glyph, grow, keys) in [
                ("Grow font", RichText::new("A").size(17.0), true, "FG"),
                ("Shrink font", RichText::new("A").size(11.0), false, "FH"),
            ] {
                let response = ui.add_sized([26.0, 22.0], Button::new(glyph));
                response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
                ribbon_controls::register(ui, &response, keys, keytips::Kind::Button);
                if response.clicked() {
                    points = adjacent_font_size(points, grow);
                }
                response.on_hover_text(label);
            }
            if points != original_points {
                style.size = crate::text::points_to_pixels(points);
            }
        });
        ui.horizontal(|ui| {
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
                    RichText::new("ab").strikethrough(),
                    &mut style.strikeout,
                ),
            ] {
                let response = ui.add_sized([28.0, 23.0], Button::new(glyph).selected(*selected));
                if response.clicked() {
                    *selected = !*selected;
                }
                text_control(ui, &response, label, *selected);
            }
        });
    }

    fn text_background_group(&self, ui: &mut Ui, state: &mut TextEditState) {
        for (label, opaque) in [("Opaque", true), ("Transparent", false)] {
            let selected = state.format.background.is_some() == opaque;
            let response = ui.add_sized([106.0, 27.0], Button::new(label).selected(selected));
            text_control(ui, &response, label, selected);
            if response.clicked() && !selected {
                let before = TextSnapshot::capture(state);
                state.format.background = opaque.then_some(self.colors[1]);
                state
                    .history
                    .record(before, ui.input(|input| input.time), false);
                state.focus = true;
            }
            response.on_hover_text(if opaque {
                "Fill the text box with Color 2"
            } else {
                "Keep the picture visible behind the text"
            });
        }
    }

    fn text_paragraph_group(ui: &mut Ui, state: &mut TextEditState) {
        ui.label(RichText::new("Alignment").small());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (alignment, label, keys) in [
                (TextAlignment::Left, "Align left", "NL"),
                (TextAlignment::Center, "Center", "NC"),
                (TextAlignment::Right, "Align right", "NR"),
            ] {
                let response = alignment_button(ui, alignment, state.format.alignment, label);
                ribbon_controls::register(ui, &response, keys, keytips::Kind::Button);
                if response.clicked() && state.format.alignment != alignment {
                    let before = TextSnapshot::capture(state);
                    state.format.alignment = alignment;
                    state
                        .history
                        .record(before, ui.input(|input| input.time), false);
                    state.focus = true;
                }
                response.on_hover_text(label);
            }
        });
    }

    fn text_effects_group(&self, ui: &mut Ui, state: &mut TextEditState) {
        let before = TextSnapshot::capture(state);
        let original_format = state.format.clone();
        let max_outline =
            crate::text::MAX_TEXT_OUTLINE.min(state.format.width.saturating_sub(5) / 2);
        ui.horizontal(|ui| {
            let mut outlined = state.format.outline_width > 0;
            let response = ui.checkbox(&mut outlined, "Outline");
            ribbon_controls::register(ui, &response, "NO", keytips::Kind::Button);
            if response.changed() {
                state.format.outline_width = if outlined { 3.min(max_outline) } else { 0 };
            }
            let response = ui.add_sized(
                [44.0, 22.0],
                DragValue::new(&mut state.format.outline_width)
                    .range(0..=max_outline)
                    .update_while_editing(false)
                    .suffix(" px"),
            );
            ribbon_controls::register(ui, &response, "NW", keytips::Kind::NumericInput);
            response.widget_info(|| {
                WidgetInfo::labeled(WidgetType::DragValue, true, "Text outline width")
            });
            response.on_hover_text("Text outline width in image pixels");
        });
        ui.label(RichText::new("Outline color").small());
        ui.horizontal(|ui| {
            for (label, color, keys) in [
                ("Black outline", BLACK, "NB"),
                ("White outline", WHITE, "NH"),
                ("Use Color 1 for outline", self.colors[0], "NF"),
            ] {
                let response =
                    outline_color_button(ui, color, state.format.outline_color == color, label);
                ribbon_controls::register(ui, &response, keys, keytips::Kind::Button);
                if response.clicked() {
                    state.format.outline_color = color;
                    state.format.outline_width = state.format.outline_width.max(1).min(max_outline);
                }
                response.on_hover_text(label);
            }
        });
        if state.format != original_format {
            state
                .history
                .record(before, ui.input(|input| input.time), false);
            state.focus = true;
        }
    }
}

fn adjacent_font_size(points: f32, grow: bool) -> f32 {
    if grow {
        FONT_SIZES
            .into_iter()
            .find(|value| *value > points + 0.01)
            .unwrap_or(*crate::text::FONT_POINT_RANGE.end())
    } else {
        FONT_SIZES
            .into_iter()
            .rev()
            .find(|value| *value < points - 0.01)
            .unwrap_or(*crate::text::FONT_POINT_RANGE.start())
    }
}

fn alignment_button(
    ui: &mut Ui,
    alignment: TextAlignment,
    selected: TextAlignment,
    label: &str,
) -> Response {
    let response = ui.add_sized(
        [24.0, 25.0],
        Button::new("").selected(alignment == selected),
    );
    let center = response.rect.center();
    for (index, width) in [15.0, 10.0, 15.0, 10.0].into_iter().enumerate() {
        let x = match alignment {
            TextAlignment::Left => center.x - 7.5,
            TextAlignment::Center => center.x - width / 2.0,
            TextAlignment::Right => center.x + 7.5 - width,
        };
        let y = center.y - 6.0 + index as f32 * 4.0;
        ui.painter().line_segment(
            [pos2(x, y), pos2(x + width, y)],
            Stroke::new(1.0_f32, Color32::from_gray(40)),
        );
    }
    response.widget_info(|| {
        WidgetInfo::selected(WidgetType::Button, true, alignment == selected, label)
    });
    response
}

fn outline_color_button(ui: &mut Ui, color: Color, selected: bool, label: &str) -> Response {
    let response = ui.add_sized([30.0, 23.0], Button::new("").selected(selected));
    let rect = response.rect.shrink2(vec2(5.0, 4.0));
    ui.painter().rect(
        rect,
        0.0,
        display::color(color),
        Stroke::new(1.0_f32, Color32::from_gray(130)),
        StrokeKind::Inside,
    );
    if label == "Use Color 1 for outline" {
        let bright =
            u32::from(color[0]) * 299 + u32::from(color[1]) * 587 + u32::from(color[2]) * 114
                > 128_000;
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "1",
            FontId::proportional(11.0),
            if bright {
                Color32::BLACK
            } else {
                Color32::WHITE
            },
        );
    }
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, selected, label));
    response
}
