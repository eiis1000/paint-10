use super::dialogs::{
    default_button, dialog_button, initial_focus, numeric_input, register_button,
};
use super::*;
use crate::color::{self, Space};
use palette::palette_controls;
use picker::{gradient_mesh, visual_picker, Picker};

mod palette;
mod picker;

#[cfg(test)]
mod tests;

#[derive(Clone)]
struct Editor {
    original: Color,
    rgba: Color,
    space: Space,
    values: [f64; 4],
    paint: [f64; 4],
    hsv: [f64; 4],
    picker: Picker,
    text: String,
    text_error: Option<String>,
    numeric_invalid: bool,
    in_gamut: bool,
    authored: Option<(Space, [f64; 4])>,
    custom_slot: usize,
}

impl Editor {
    fn new(rgba: Color) -> Self {
        let rgb = [rgba[0], rgba[1], rgba[2]];
        Self {
            original: rgba,
            rgba,
            space: Space::PaintHsl,
            values: color::coordinates(Space::PaintHsl, rgb),
            paint: color::coordinates(Space::PaintHsl, rgb),
            hsv: color::coordinates(Space::Hsv, rgb),
            picker: Picker::Paint,
            text: hex(rgba),
            text_error: None,
            numeric_invalid: false,
            in_gamut: true,
            authored: None,
            custom_slot: 0,
        }
    }

    fn rgb(&self) -> [u8; 3] {
        [self.rgba[0], self.rgba[1], self.rgba[2]]
    }

    fn set_color(&mut self, rgba: Color) {
        self.rgba = rgba;
        self.values = color::coordinates(self.space, self.rgb());
        self.paint = color::coordinates(Space::PaintHsl, self.rgb());
        self.hsv = color::coordinates(Space::Hsv, self.rgb());
        self.text = hex(rgba);
        self.text_error = None;
        self.in_gamut = true;
        self.authored = None;
    }

    fn set_rgb(&mut self, rgb: [u8; 3]) {
        self.set_color([rgb[0], rgb[1], rgb[2], self.rgba[3]]);
    }

    fn apply_coordinates(&mut self, space: Space, values: [f64; 4]) {
        let converted = color::from_coordinates(space, values);
        self.set_rgb(converted.rgb);
        self.in_gamut = converted.in_gamut;
        if matches!(space, Space::Oklab | Space::Oklch) {
            self.authored = Some((space, values));
        }
        if self.space == space {
            self.values = values;
        }
        match space {
            Space::PaintHsl => self.paint = values,
            Space::Hsv => self.hsv = values,
            _ => {}
        }
    }

    fn fit_to_srgb(&mut self) {
        let Some((space, [lightness, second, third, _])) = self.authored else {
            return;
        };
        let lch = if space == Space::Oklab {
            [
                lightness,
                second.hypot(third),
                third.atan2(second).to_degrees().rem_euclid(360.0),
            ]
        } else {
            [lightness, second, third]
        };
        let [lightness, chroma, hue] = color::fit_oklch(lch);
        let converted = color::from_coordinates(Space::Oklch, [lightness, chroma, hue, 0.0]);
        self.set_rgb(converted.rgb);
    }

    fn store_custom_color(&mut self, slots: &mut Vec<Color>) {
        slots.resize(crate::preferences::CUSTOM_COLOR_COUNT, WHITE);
        slots[self.custom_slot] = self.rgba;
        let columns = crate::preferences::CUSTOM_COLOR_COUNT / 2;
        self.custom_slot = if self.custom_slot < columns {
            self.custom_slot + columns
        } else {
            (self.custom_slot - columns + 1) % columns
        };
    }
}

fn key() -> Id {
    Id::new("paint10-color-editor")
}

fn hex(rgba: Color) -> String {
    color::format_hex(rgba, rgba[3] != 255)
        .trim_start_matches('#')
        .to_owned()
}

pub(super) fn restore(app: &mut PaintApp, ctx: &Context) {
    if let Some(state) = ctx.data(|data| data.get_temp::<Editor>(key())) {
        app.colors[app.active_color] = state.original;
        app.hex = hex(state.original);
        ctx.request_repaint();
    }
}

pub(super) fn clear(ctx: &Context) {
    ctx.data_mut(|data| data.remove::<Editor>(key()));
}

impl PaintApp {
    pub(super) fn remember_custom_color(&mut self, color: Color) {
        crate::preferences::remember_custom_color(&mut self.recent_custom_colors, color);
        if !self.persist_preferences {
            return;
        }
        if let Err(error) =
            crate::preferences::save_custom_palette(&self.custom_colors, &self.recent_custom_colors)
        {
            if self.dialog == Some(Dialog::Colors) {
                self.dialog_error = Some(error);
            } else {
                self.message = format!("Could not save custom colors: {error}");
            }
        }
    }

    pub(super) fn colors_dialog(&mut self, ui: &mut Ui) -> bool {
        let original_frame = self.colors[self.active_color];
        let mut state = ui
            .ctx()
            .data(|data| data.get_temp::<Editor>(key()))
            .unwrap_or_else(|| Editor::new(original_frame));
        let previous_visual = (
            state.rgba,
            state.space,
            state.picker,
            state.paint,
            state.hsv,
            state.custom_slot,
        );
        let width = 440.0_f32.min((ui.ctx().screen_rect().width() - 42.0).max(320.0));
        ui.set_width(width);
        ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
        ui.spacing_mut().interact_size.y = 22.0;
        let picker_height = if ui.ctx().screen_rect().height() < 550.0 {
            96.0
        } else {
            148.0
        };

        ScrollArea::vertical()
            .id_salt("color_editor_controls")
            .max_height((ui.ctx().screen_rect().height() - 150.0).max(160.0))
            .auto_shrink([false, true])
            .drag_to_scroll(false)
            .show(ui, |ui| {
                let content_width = ui.available_width();
                mode_controls(ui, &mut state);
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        vec2(content_width - 198.0, picker_height),
                        Layout::left_to_right(Align::Min),
                        |ui| visual_picker(ui, &mut state, picker_height),
                    );
                    ui.allocate_ui_with_layout(
                        vec2(192.0, picker_height),
                        Layout::top_down(Align::Min),
                        |ui| coordinate_controls(ui, &mut state),
                    );
                });
                alpha_controls(ui, &mut state);
                if let Some(color) = palette_controls(ui, &mut state, &self.custom_colors) {
                    self.remember_custom_color(color);
                }
            });
        color_text(ui, &mut state);

        ui.allocate_ui_with_layout(
            vec2(width, 20.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                if state.numeric_invalid {
                    ui.colored_label(ui.visuals().error_fg_color, "Finish the current number.");
                } else if state.text_error.is_some() {
                    ui.colored_label(ui.visuals().error_fg_color, "Invalid color text.")
                        .on_hover_text(state.text_error.as_deref().unwrap_or_default());
                } else if !state.in_gamut {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        "Outside sRGB; preview is clipped.",
                    );
                    if state.authored.is_some() {
                        if ui.small_button("Fit to sRGB").clicked() {
                            state.fit_to_srgb();
                        }
                    } else if ui.small_button("Use clipped color").clicked() {
                        state.set_color(state.rgba);
                    }
                } else if let Some(error) = &self.dialog_error {
                    ui.add(
                        Label::new(RichText::new(error).color(ui.visuals().error_fg_color))
                            .truncate(),
                    )
                    .on_hover_text(error);
                } else {
                    let note = if state.space == Space::Cmyk {
                        "CMYK is an unprofiled approximation. Canvas: sRGB."
                    } else {
                        "Canvas: sRGB · alpha 0 = transparent, 255 = opaque"
                    };
                    ui.label(RichText::new(note).small().weak());
                }
            },
        );

        let valid = state.text_error.is_none() && !state.numeric_invalid;
        let mut close = false;
        ui.allocate_ui_with_layout(
            vec2(width, 28.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                let add = ui
                    .add_enabled(valid, Button::new("Add to custom colors"))
                    .on_hover_text(format!(
                        concat!(
                            "Store in custom color slot {}. ",
                            "Click a slot to choose where to replace a color."
                        ),
                        state.custom_slot + 1
                    ));
                register_button(ui, &add);
                if add.clicked() {
                    state.store_custom_color(&mut self.custom_colors);
                    self.remember_custom_color(state.rgba);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().interact_size = vec2(76.0, 28.0);
                    if dialog_button(ui, "Cancel") {
                        state.set_color(state.original);
                        close = true;
                    }
                    if default_button(ui, "OK", valid) {
                        close = true;
                    }
                });
            },
        );
        self.colors[self.active_color] = state.rgba;
        self.hex = state.text.clone();
        if previous_visual
            != (
                state.rgba,
                state.space,
                state.picker,
                state.paint,
                state.hsv,
                state.custom_slot,
            )
        {
            ui.ctx().request_repaint();
        }
        ui.ctx().data_mut(|data| data.insert_temp(key(), state));
        close
    }
}

fn mode_controls(ui: &mut Ui, state: &mut Editor) {
    ui.horizontal(|ui| {
        ui.label("Coordinates");
        let previous = state.space;
        ui.add_enabled_ui(!state.numeric_invalid, |ui| {
            ComboBox::from_id_salt("color_space")
                .width(182.0)
                .height(260.0)
                .selected_text(state.space.label())
                .show_ui(ui, |ui| {
                    theme::menu(ui);
                    for space in Space::ALL {
                        if ui
                            .add(theme::MenuItem::new(space.label()).selected(state.space == space))
                            .clicked()
                        {
                            state.space = space;
                            ui.close_menu();
                        }
                    }
                })
                .response
                .widget_info(|| {
                    WidgetInfo::labeled(WidgetType::ComboBox, true, "Color coordinates")
                });
        });
        if state.space != previous {
            state.values = state
                .authored
                .filter(|(space, _)| *space == state.space)
                .map(|(_, values)| values)
                .unwrap_or_else(|| color::coordinates(state.space, state.rgb()));
        }
        ComboBox::from_id_salt("color_picker")
            .width(125.0)
            .selected_text(state.picker.label())
            .show_ui(ui, |ui| {
                theme::menu(ui);
                for picker in [Picker::Paint, Picker::Hsv] {
                    if ui
                        .add(theme::MenuItem::new(picker.label()).selected(state.picker == picker))
                        .clicked()
                    {
                        state.picker = picker;
                        ui.close_menu();
                    }
                }
            })
            .response
            .widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, true, "Visual color picker"));
    });
}

fn coordinate_controls(ui: &mut Ui, state: &mut Editor) {
    state.numeric_invalid = false;
    ui.spacing_mut().interact_size = vec2(60.0, 18.0);
    ui.spacing_mut().item_spacing.y = 2.0;
    ui.push_id(format!("coordinates-{:?}", state.space), |ui| {
        if state.space == Space::PaintHsl {
            ui.columns(2, |columns| {
                let mut rgb = state.rgb().map(f64::from);
                let previous = rgb;
                channel_grid(
                    &mut columns[0],
                    Space::Rgb,
                    &mut rgb,
                    true,
                    true,
                    &mut state.numeric_invalid,
                );
                if rgb != previous {
                    state.set_rgb(rgb.map(|value| value.round() as u8));
                }
                let previous = state.values;
                channel_grid(
                    &mut columns[1],
                    Space::PaintHsl,
                    &mut state.values[..3],
                    false,
                    true,
                    &mut state.numeric_invalid,
                );
                if state.values != previous {
                    state.apply_coordinates(Space::PaintHsl, state.values);
                }
            });
        } else {
            let previous = state.values;
            let count = state.space.channels().len();
            channel_grid(
                ui,
                state.space,
                &mut state.values[..count],
                true,
                false,
                &mut state.numeric_invalid,
            );
            if state.values != previous {
                state.apply_coordinates(state.space, state.values);
            }
        }
    });
}

fn channel_grid(
    ui: &mut Ui,
    space: Space,
    values: &mut [f64],
    first: bool,
    compact: bool,
    invalid: &mut bool,
) {
    if compact {
        ui.spacing_mut().interact_size.x = 44.0;
    }
    Grid::new(("channels", space.label()))
        .min_col_width(0.0)
        .spacing(vec2(4.0, 2.0))
        .show(ui, |ui| {
            channel_rows(ui, space, values, first, compact, invalid)
        });
}

fn channel_rows(
    ui: &mut Ui,
    space: Space,
    values: &mut [f64],
    first: bool,
    compact: bool,
    invalid: &mut bool,
) {
    for (index, (channel, value)) in space.channels().iter().zip(values).enumerate() {
        let display_label = match (compact, channel.label) {
            (true, "Saturation") => "Sat",
            (true, "Luminosity") => "Lum",
            _ => channel.label,
        };
        let label = ui.label(display_label).on_hover_text(channel.label);
        label
            .widget_info(|| WidgetInfo::labeled(WidgetType::Label, ui.is_enabled(), channel.label));
        let response = numeric_input(
            ui,
            DragValue::new(value)
                .range(channel.min..=channel.max)
                .clamp_existing_to_range(false)
                .max_decimals(channel.decimals)
                .speed(if channel.decimals == 0 || channel.max >= 239.0 {
                    1.0
                } else if channel.max == 100.0 {
                    0.5
                } else {
                    (channel.max - channel.min) / 250.0
                })
                .suffix(channel.suffix),
        )
        .labelled_by(label.id);
        if first && index == 0 {
            initial_focus(ui, &response);
        }
        *invalid |= invalid_number(ui, &response);
        ui.end_row();
    }
}

fn invalid_number(ui: &Ui, response: &Response) -> bool {
    let invalid = (response.has_focus() || response.lost_focus())
        && ui.data(|data| {
            data.get_temp::<String>(response.id).is_some_and(|text| {
                !text
                    .replace('−', "-")
                    .trim()
                    .parse::<f64>()
                    .is_ok_and(f64::is_finite)
            })
        });
    if invalid && response.lost_focus() {
        response.request_focus();
    }
    invalid
}

fn alpha_controls(ui: &mut Ui, state: &mut Editor) {
    ui.horizontal(|ui| {
        let label = ui.label("Alpha");
        let previous = state.rgba[3];
        let response = numeric_input(ui, DragValue::new(&mut state.rgba[3]).range(0..=255))
            .labelled_by(label.id);
        state.numeric_invalid |= invalid_number(ui, &response);
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::click_and_drag());
        canvas::checkerboard(ui.painter(), rect, 5.0);
        gradient_mesh(ui.painter(), rect, 1, 1, |x, _| {
            Color32::from_rgba_unmultiplied(
                state.rgba[0],
                state.rgba[1],
                state.rgba[2],
                (x * 255.0).round() as u8,
            )
        });
        if response.clicked() || response.dragged() {
            if let Some(point) = response.interact_pointer_pos() {
                state.rgba[3] = (((point.x - rect.left()) / rect.width()).clamp(0.0, 1.0) * 255.0)
                    .round() as u8;
            }
        }
        response.widget_info(|| {
            WidgetInfo::slider(
                ui.is_enabled(),
                f64::from(state.rgba[3]),
                "Alpha transparency",
            )
        });
        let x = rect.left() + state.rgba[3] as f32 / 255.0 * rect.width();
        ui.painter()
            .vline(x, rect.y_range(), Stroke::new(3.0_f32, Color32::WHITE));
        ui.painter()
            .vline(x, rect.y_range(), Stroke::new(1.0_f32, Color32::BLACK));
        if previous != state.rgba[3] {
            state.text = hex(state.rgba);
            state.text_error = None;
        }
    });
}

fn swatch(ui: &mut Ui, color: Color, size: Vec2, label: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    register_button(ui, &response);
    canvas::checkerboard(ui.painter(), rect, 5.0);
    ui.painter().rect_filled(
        rect.shrink(1.0),
        0.0,
        Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3]),
    );
    let active = response.hovered() || response.has_focus();
    ui.painter().rect_stroke(
        rect,
        0.0,
        Stroke::new(
            if active { 2.0_f32 } else { 1.0_f32 },
            if active {
                BLUE
            } else {
                Color32::from_gray(140)
            },
        ),
        StrokeKind::Inside,
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
    response.on_hover_text(label)
}

fn color_text(ui: &mut Ui, state: &mut Editor) {
    ui.horizontal(|ui| {
        let label = ui.label("Color text");
        let input = TextEdit::singleline(&mut state.text).id_salt("color_text");
        let response = ui
            .add_sized(vec2(ui.available_width(), 24.0), input)
            .labelled_by(label.id);
        if response.changed() {
            match color::parse_color_with_alpha(&state.text) {
                Ok(parsed) => {
                    let text = state.text.clone();
                    let mut rgba = parsed.rgba;
                    if !parsed.explicit_alpha {
                        rgba[3] = state.rgba[3];
                    }
                    state.set_color(rgba);
                    if let Some((space, coordinates)) = parsed.coordinates {
                        state.space = space;
                        state.values = coordinates;
                        state.authored = Some((space, coordinates));
                    }
                    state.in_gamut = parsed.in_gamut;
                    state.text = text;
                }
                Err(error) => state.text_error = Some(error),
            }
        }
        response.on_hover_text(concat!(
            "Paste 3/4/6/8-digit hex, a CSS color name, or ",
            "rgb(), hsl(), oklab(), oklch(), color(srgb …). ",
            "Alpha is preserved unless explicitly included."
        ));
    });
}
