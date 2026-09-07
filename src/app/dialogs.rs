use super::*;

#[derive(Clone, Default)]
struct ModalKeys {
    kind: String,
    initial: bool,
    enter_default: bool,
    buttons: Vec<Id>,
    focused: Option<Id>,
    numeric_text: Option<String>,
    invalid_number: bool,
}

fn modal_key() -> Id {
    Id::new("paint10-modal-keyboard")
}

#[derive(Clone)]
struct ColorDialogState {
    original: Color,
    rgb: [u8; 3],
    hsl: [u16; 3],
}

fn color_dialog_key() -> Id {
    Id::new("paint10-color-dialog")
}

fn rgb_to_hsl(rgb: [u8; 3]) -> [u16; 3] {
    let [red, green, blue] = rgb.map(|channel| f64::from(channel) / 255.0);
    let maximum = red.max(green).max(blue);
    let minimum = red.min(green).min(blue);
    let difference = maximum - minimum;
    let lightness = (maximum + minimum) / 2.0;
    if difference == 0.0 {
        return [160, 0, (lightness * 240.0).round() as u16];
    }
    let hue = if maximum == red {
        ((green - blue) / difference).rem_euclid(6.0)
    } else if maximum == green {
        (blue - red) / difference + 2.0
    } else {
        (red - green) / difference + 4.0
    };
    let saturation = difference / (1.0 - (2.0 * lightness - 1.0).abs());
    [
        ((hue * 40.0).round() as u16) % 240,
        (saturation * 240.0).round() as u16,
        (lightness * 240.0).round() as u16,
    ]
}

fn hsl_to_rgb([hue, saturation, lightness]: [u16; 3]) -> [u8; 3] {
    let hue = f64::from(hue % 240) / 40.0;
    let saturation = f64::from(saturation.min(240)) / 240.0;
    let lightness = f64::from(lightness.min(240)) / 240.0;
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let secondary = chroma * (1.0 - (hue % 2.0 - 1.0).abs());
    let base = lightness - chroma / 2.0;
    let channels = match hue as u8 {
        0 => [chroma, secondary, 0.0],
        1 => [secondary, chroma, 0.0],
        2 => [0.0, chroma, secondary],
        3 => [0.0, secondary, chroma],
        4 => [secondary, 0.0, chroma],
        _ => [chroma, 0.0, secondary],
    };
    channels.map(|channel| ((channel + base) * 255.0).round().clamp(0.0, 255.0) as u8)
}

fn prepare_modal(ctx: &Context, kind: &str) -> bool {
    let previous = ctx
        .data(|data| data.get_temp::<ModalKeys>(modal_key()))
        .unwrap_or_default();
    let popup = ctx.memory(|memory| memory.any_popup_open());
    let focused_button = ctx
        .memory(|memory| memory.focused())
        .is_some_and(|id| previous.buttons.contains(&id));
    let focused = ctx.memory(|memory| memory.focused());
    let numeric_text = focused.and_then(|id| ctx.data(|data| data.get_temp::<String>(id)));
    let enter_default = !popup
        && !focused_button
        && ctx.input(|input| input.key_pressed(Key::Enter) && input.modifiers.is_none());
    let escape = !popup
        && ctx.input_mut(|input| {
            super::shortcuts::consume_shortcut(input, Modifiers::NONE, Key::Escape)
        });
    ctx.data_mut(|data| {
        data.insert_temp(
            modal_key(),
            ModalKeys {
                initial: previous.initial || previous.kind != kind,
                kind: kind.into(),
                enter_default,
                buttons: Vec::new(),
                focused,
                numeric_text,
                invalid_number: false,
            },
        )
    });
    escape
}

pub(in crate::app) fn initial_focus(ui: &Ui, response: &Response) {
    if response.enabled() && take_initial_focus(ui) {
        response.request_focus();
        let mut state = TextEdit::load_state(ui.ctx(), response.id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(usize::MAX),
            )));
        state.store(ui.ctx(), response.id);
        ui.ctx().request_repaint();
    }
}

fn take_initial_focus(ui: &Ui) -> bool {
    // A new Window first lays out an invisible, disabled sizing pass. Keep
    // focus pending until its controls can actually receive keyboard input.
    if !ui.is_enabled() || ui.is_sizing_pass() {
        return false;
    }
    ui.ctx().data_mut(|data| {
        let state = data.get_temp_mut_or_default::<ModalKeys>(modal_key());
        std::mem::take(&mut state.initial)
    })
}

fn register_button(ui: &Ui, response: &Response) {
    ui.ctx().data_mut(|data| {
        data.get_temp_mut_or_default::<ModalKeys>(modal_key())
            .buttons
            .push(response.id)
    });
}

pub(in crate::app) fn dialog_button(ui: &mut Ui, label: &str) -> bool {
    let response = ui.button(label);
    register_button(ui, &response);
    response.clicked()
}

pub(in crate::app) fn default_button(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    let response = ui.add_enabled(enabled, Button::new(label));
    register_button(ui, &response);
    if response.enabled() && take_initial_focus(ui) {
        response.request_focus();
    }
    let enter = ui.ctx().data_mut(|data| {
        std::mem::take(
            &mut data
                .get_temp_mut_or_default::<ModalKeys>(modal_key())
                .enter_default,
        )
    });
    if !response.enabled() || !(response.clicked() || enter) {
        return false;
    }
    let state = ui
        .ctx()
        .data(|data| data.get_temp::<ModalKeys>(modal_key()).unwrap_or_default());
    let numeric_text = state
        .focused
        .and_then(|id| ui.ctx().data(|data| data.get_temp::<String>(id)))
        .or(state.numeric_text);
    if numeric_text.as_ref().is_some_and(|text| {
        let text: String = text
            .chars()
            .filter(|character| !character.is_whitespace())
            .map(|character| if character == '−' { '-' } else { character })
            .collect();
        !text.parse::<f64>().is_ok_and(f64::is_finite)
    }) {
        ui.ctx().data_mut(|data| {
            data.get_temp_mut_or_default::<ModalKeys>(modal_key())
                .invalid_number = true
        });
        if let Some(id) = state.focused {
            if let Some(text) = numeric_text {
                ui.ctx().data_mut(|data| data.insert_temp(id, text));
            }
            ui.memory_mut(|memory| memory.request_focus(id));
        }
        return false;
    }
    true
}

impl PaintApp {
    pub(in crate::app) fn dialogs(&mut self, ctx: &Context) {
        let kind = if self.pending.is_some() {
            Some("Unsaved changes")
        } else {
            self.dialog.map(|dialog| match dialog {
                Dialog::Resize => "Resize and Skew",
                Dialog::Rotate => "Rotate",
                Dialog::Colors => "Edit Colors",
                Dialog::Properties => "Image Properties",
                Dialog::About => "About Paint 10",
                Dialog::Print => "Page Setup and Print",
                Dialog::Import => "From Scanner or Camera",
                Dialog::Wallpaper => "Set as Desktop Background",
            })
        };
        if let Some(kind) = kind {
            if prepare_modal(ctx, kind) {
                if self.dialog == Some(Dialog::Colors) {
                    if let Some(state) =
                        ctx.data(|data| data.get_temp::<ColorDialogState>(color_dialog_key()))
                    {
                        self.colors[self.active_color] = state.original;
                    }
                }
                if self.dialog == Some(Dialog::Import) {
                    self.job_cancel
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                }
                self.pending = None;
                self.pending_path = None;
                self.dialog = None;
                self.dialog_error = None;
                ctx.data_mut(|data| data.remove::<ModalKeys>(modal_key()));
                ctx.data_mut(|data| data.remove::<ColorDialogState>(color_dialog_key()));
                return;
            }
        } else {
            ctx.data_mut(|data| data.remove::<ModalKeys>(modal_key()));
        }
        if let Some(action) = self.pending {
            let shown = Window::new("Paint 10")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("Do you want to save changes to this picture?");
                    ui.add_space(12.);
                    ui.horizontal(|ui| {
                        if default_button(ui, "Save", true) {
                            if self.save(false) {
                                self.pending = None;
                                self.execute(action, ctx);
                            } else if self.message.starts_with("Could not save:")
                                || self.message.contains("Save As")
                            {
                                self.dialog_error = Some(self.message.clone());
                            }
                        }
                        if dialog_button(ui, "Don't save") {
                            self.pending = None;
                            self.execute(action, ctx);
                        }
                        if dialog_button(ui, "Cancel") {
                            self.pending = None;
                            self.pending_path = None;
                        }
                    });
                    if let Some(error) = &self.dialog_error {
                        ui.colored_label(Color32::RED, error);
                    }
                });
            if let Some(shown) = shown {
                ctx.memory_mut(|memory| memory.set_modal_layer(shown.response.layer_id));
            }
        }
        self.text_editor(ctx);
        if let Some(dialog) = self.dialog {
            let title = kind.expect("dialog title");
            let mut open = true;
            let mut close = false;
            let shown = Window::new(title)
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(340.)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    close = match dialog {
                        Dialog::Resize => self.resize_dialog(ui),
                        Dialog::Properties => self.properties_dialog(ui),
                        Dialog::Rotate => self.rotation_dialog(ui),
                        Dialog::Colors => self.colors_dialog(ui),
                        Dialog::About => self.about_dialog(ui),
                        Dialog::Print => self.print_dialog(ui),
                        Dialog::Import => self.import_dialog(ui, ctx),
                        Dialog::Wallpaper => self.wallpaper_dialog(ui, ctx),
                    };
                    if ctx.data(|data| {
                        data.get_temp::<ModalKeys>(modal_key())
                            .is_some_and(|state| state.invalid_number)
                    }) {
                        self.dialog_error = Some("Enter a valid, finite number.".into());
                    }
                    if let Some(error) = &self.dialog_error {
                        ui.colored_label(Color32::RED, error);
                    }
                });
            if let Some(shown) = shown {
                ctx.memory_mut(|memory| memory.set_modal_layer(shown.response.layer_id));
            }
            if !open || close {
                if !open && dialog == Dialog::Import {
                    self.job_cancel
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                }
                if !open && dialog == Dialog::Colors {
                    if let Some(state) =
                        ctx.data(|data| data.get_temp::<ColorDialogState>(color_dialog_key()))
                    {
                        self.colors[self.active_color] = state.original;
                    }
                }
                self.dialog = None;
                self.dialog_error = None;
            }
        }
        if self.pending.is_none() && self.dialog.is_none() {
            self.dialog_error = None;
            ctx.data_mut(|data| data.remove::<ModalKeys>(modal_key()));
            ctx.data_mut(|data| data.remove::<ColorDialogState>(color_dialog_key()));
        }
    }

    fn resize_dialog(&mut self, ui: &mut Ui) -> bool {
        ui.strong("Resize");
        ui.horizontal(|ui| {
            ui.label("By:");
            let previous = self.percent;
            ui.radio_value(&mut self.percent, true, "Percentage");
            ui.radio_value(&mut self.percent, false, "Pixels");
            if previous != self.percent {
                (self.resize_w, self.resize_h) = if self.percent {
                    (100, 100)
                } else {
                    self.selected_region()
                        .map(|region| (region.w, region.h))
                        .unwrap_or(self.doc.image.dimensions())
                };
            }
        });
        let original = self
            .selected_region()
            .map(|region| (region.w, region.h))
            .unwrap_or(self.doc.image.dimensions());
        Grid::new("dimensions")
            .num_columns(2)
            .spacing(vec2(16.0, 10.0))
            .show(ui, |ui| {
                ui.label("Horizontal:");
                let horizontal = ui.add(
                    DragValue::new(&mut self.resize_w)
                        .range(1..=16384)
                        .speed(1.0),
                );
                initial_focus(ui, &horizontal);
                if horizontal.changed() && self.aspect {
                    self.resize_h = if self.percent {
                        self.resize_w
                    } else {
                        (self.resize_w as f64 * original.1 as f64 / original.0 as f64)
                            .round()
                            .max(1.0) as u32
                    };
                }
                ui.end_row();
                ui.label("Vertical:");
                if ui
                    .add(
                        DragValue::new(&mut self.resize_h)
                            .range(1..=16384)
                            .speed(1.0),
                    )
                    .changed()
                    && self.aspect
                {
                    self.resize_w = if self.percent {
                        self.resize_h
                    } else {
                        (self.resize_h as f64 * original.0 as f64 / original.1 as f64)
                            .round()
                            .max(1.0) as u32
                    };
                }
                ui.end_row();
            });
        ui.checkbox(&mut self.aspect, "Maintain aspect ratio");
        ui.separator();
        ui.strong("Skew (Degrees)");
        for (label, angle) in [
            ("Horizontal:", &mut self.skew_x),
            ("Vertical:", &mut self.skew_y),
        ] {
            ui.horizontal(|ui| {
                ui.label(label);
                ui.add(DragValue::new(angle).range(-89.0..=89.0).suffix("°"));
            });
        }
        ui.add_space(12.0);
        let mut close = false;
        ui.horizontal(|ui| {
            if default_button(ui, "OK", true) {
                let dimensions = if self.percent {
                    (
                        (original.0 as f64 * self.resize_w as f64 / 100.0)
                            .round()
                            .max(1.0) as u32,
                        (original.1 as f64 * self.resize_h as f64 / 100.0)
                            .round()
                            .max(1.0) as u32,
                    )
                } else {
                    (self.resize_w, self.resize_h)
                };
                if !d::valid_size(dimensions.0, dimensions.1) {
                    self.dialog_error =
                        Some("Dimensions must be positive and fit within 16 megapixels.".into());
                    return;
                }
                let plan = match super::transforms::SkewPlan::new(
                    dimensions.0,
                    dimensions.1,
                    self.skew_x,
                    self.skew_y,
                ) {
                    Ok(plan) => plan,
                    Err(error) => {
                        self.dialog_error = Some(error);
                        return;
                    }
                };
                self.transform(|image| {
                    plan.apply(&imageops::resize(
                        image,
                        dimensions.0,
                        dimensions.1,
                        imageops::FilterType::CatmullRom,
                    ))
                });
                close = true;
            }
            close |= dialog_button(ui, "Cancel");
        });
        close
    }

    fn rotation_dialog(&mut self, ui: &mut Ui) -> bool {
        let mut close = false;

        ui.label(if self.object.is_some() {
            "Rotate the selected object. Text remains editable."
        } else {
            "Rotate the selection or the whole picture."
        });
        ui.horizontal(|ui| {
            ui.label("Angle:");
            let angle = ui.add(
                DragValue::new(&mut self.angle)
                    .range(-180.0..=180.0)
                    .suffix("°"),
            );
            initial_focus(ui, &angle);
            ui.add(Slider::new(&mut self.angle, -180.0..=180.0).show_value(false));
        });
        ui.horizontal(|ui| {
            if default_button(ui, "Apply", true) {
                if self.rotate_picture(self.angle, true) {
                    close = true;
                } else {
                    self.dialog_error = Some(self.message.clone());
                }
            }
            if dialog_button(ui, "Cancel") {
                close = true;
            }
        });

        close
    }

    fn colors_dialog(&mut self, ui: &mut Ui) -> bool {
        let mut close = false;

        let c = self.colors[self.active_color];
        let mut rgb = [c[0], c[1], c[2]];
        let mut state = ui
            .ctx()
            .data(|data| data.get_temp::<ColorDialogState>(color_dialog_key()))
            .unwrap_or_else(|| ColorDialogState {
                original: c,
                rgb,
                hsl: rgb_to_hsl(rgb),
            });
        let mut picked = Color32::from_rgb(c[0], c[1], c[2]);
        if egui::color_picker::color_picker_color32(
            ui,
            &mut picked,
            egui::color_picker::Alpha::Opaque,
        ) {
            rgb = [picked.r(), picked.g(), picked.b()];
        }
        ui.horizontal(|ui| {
            for (i, label) in ["Red", "Green", "Blue"].into_iter().enumerate() {
                ui.label(label);
                let response = ui.add(DragValue::new(&mut rgb[i]));
                if i == 0 {
                    initial_focus(ui, &response);
                }
            }
        });
        if rgb != state.rgb {
            state.hsl = rgb_to_hsl(rgb);
        }
        let previous_hsl = state.hsl;
        Grid::new("paint_hls").num_columns(2).show(ui, |ui| {
            for (index, label, maximum) in [
                (0, "Hue", 239),
                (1, "Saturation", 240),
                (2, "Luminosity", 240),
            ] {
                ui.label(label);
                ui.add(DragValue::new(&mut state.hsl[index]).range(0..=maximum));
                ui.end_row();
            }
        });
        if previous_hsl != state.hsl {
            rgb = hsl_to_rgb(state.hsl);
        }
        if rgb != [c[0], c[1], c[2]] {
            self.colors[self.active_color] = [rgb[0], rgb[1], rgb[2], 255];
            self.hex = format!("{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]);
        }
        ui.horizontal(|ui| {
            ui.label("Hex #");
            if ui.text_edit_singleline(&mut self.hex).changed() {
                if let Ok(v) = u32::from_str_radix(self.hex.trim_start_matches('#'), 16) {
                    if self.hex.trim_start_matches('#').len() == 6 {
                        self.colors[self.active_color] =
                            [(v >> 16) as u8, (v >> 8) as u8, v as u8, 255];
                        rgb = [(v >> 16) as u8, (v >> 8) as u8, v as u8];
                        state.hsl = rgb_to_hsl(rgb);
                    }
                }
            }
        });
        state.rgb = rgb;
        ui.ctx()
            .data_mut(|data| data.insert_temp(color_dialog_key(), state.clone()));
        let hex = self.hex.trim_start_matches('#');
        let valid_hex = hex.len() == 6 && u32::from_str_radix(hex, 16).is_ok();
        if !valid_hex {
            ui.colored_label(
                Color32::RED,
                "Enter six hexadecimal digits, such as 00A2E8.",
            );
        }
        if dialog_button(ui, "Add to custom colors") {
            if self.custom_colors.len() == 10 {
                self.custom_colors.remove(0);
            }
            self.custom_colors.push(self.colors[self.active_color]);
            if let Err(error) = crate::preferences::save_custom_colors(&self.custom_colors) {
                self.dialog_error = Some(format!("Could not save custom colors: {error}"));
            }
        }
        ui.horizontal(|ui| {
            if default_button(ui, "OK", valid_hex) {
                close = true;
            }
            if dialog_button(ui, "Cancel") {
                self.colors[self.active_color] = state.original;
                close = true;
            }
        });

        close
    }

    fn about_dialog(&mut self, ui: &mut Ui) -> bool {
        let mut close = false;

        ui.heading("Paint 10");
        ui.label("The familiar Paint experience, built in Rust.");
        ui.separator();
        ui.label("Draw with the left mouse button (Color 1) or right mouse button (Color 2). Hold Shift for straight lines, circles, and squares.");
        ui.label("Select: drag a region to move/copy/crop it. Click a pasted image to select it. Double-click text to edit it again.");
        ui.label("Ctrl+Z / Ctrl+Y: undo / redo\nCtrl+C / X / V: copy / cut / paste\nCtrl+W: resize and skew\nCtrl+E: canvas properties\nCtrl+Shift+X: crop\nCtrl+G / Ctrl+R: grid / rulers\nCtrl+mouse wheel: zoom\nEscape: cancel / deselect\nF11: view picture");
        ui.separator();
        ui.label("Save as .p10 to keep text and images editable after reopening. PNG, JPEG, BMP, GIF, and TIFF produce ordinary flattened pictures.");
        if default_button(ui, "OK", true) {
            close = true;
        }

        close
    }

    fn print_dialog(&mut self, ui: &mut Ui) -> bool {
        let mut close = false;

        ui.strong("Page setup");
        ui.horizontal(|ui| {
            ui.label("Paper:");
            ui.selectable_value(&mut self.page.a4, false, "Letter");
            ui.selectable_value(&mut self.page.a4, true, "A4");
        });
        ui.horizontal(|ui| {
            ui.label("Orientation:");
            ui.selectable_value(&mut self.page.landscape, false, "Portrait");
            ui.selectable_value(&mut self.page.landscape, true, "Landscape");
        });
        ui.label("Margins (mm)");
        Grid::new("print_margins").num_columns(4).show(ui, |ui| {
            for (i, (label, value)) in [
                ("Left", &mut self.page.margin_left_mm),
                ("Right", &mut self.page.margin_right_mm),
                ("Top", &mut self.page.margin_top_mm),
                ("Bottom", &mut self.page.margin_bottom_mm),
            ]
            .into_iter()
            .enumerate()
            {
                ui.label(label);
                let response = ui.add(DragValue::new(value).range(0.0..=100.0));
                if i == 0 {
                    initial_focus(ui, &response);
                }
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
        ui.checkbox(&mut self.page.fit, "Fit to pages");
        if self.page.fit {
            ui.horizontal(|ui| {
                ui.add(DragValue::new(&mut self.page.fit_across).range(1..=10));
                ui.label("across by");
                ui.add(DragValue::new(&mut self.page.fit_down).range(1..=10));
                ui.label("down");
            });
        }
        if !self.page.fit {
            ui.horizontal(|ui| {
                ui.label("Scale:");
                ui.add(
                    DragValue::new(&mut self.page.scale)
                        .range(1.0..=500.0)
                        .suffix("%"),
                );
            });
        }
        ui.horizontal(|ui| {
            ui.label("Center:");
            ui.checkbox(&mut self.page.center_h, "Horizontally");
            ui.checkbox(&mut self.page.center_v, "Vertically");
        });
        match self.page.layout(&self.doc.image) {
            Ok(layout) => {
                ui.label(format!(
                    "{} page(s) · {} across × {} down",
                    layout.page_count(),
                    layout.columns,
                    layout.rows
                ));
            }
            Err(e) => {
                ui.colored_label(Color32::RED, e);
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            if default_button(ui, "Print…", self.page.layout(&self.doc.image).is_ok()) {
                match crate::printing::print(&self.doc.composite(), &self.page) {
                    Ok(()) => {
                        self.message = "Print job submitted".into();
                        close = true;
                    }
                    Err(e) => self.dialog_error = Some(format!("Print: {e}")),
                }
            }
            if dialog_button(ui, "Save PDF…") {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PDF document", &["pdf"])
                    .set_file_name("Untitled.pdf")
                    .save_file()
                {
                    match crate::printing::pdf(&self.doc.composite(), &self.page)
                        .and_then(|bytes| crate::project::atomic_write(&path, &bytes))
                    {
                        Ok(()) => {
                            self.message = "PDF saved".into();
                            self.dialog_error = None;
                        }
                        Err(e) => self.dialog_error = Some(format!("Could not save PDF: {e}")),
                    }
                }
            }
            if dialog_button(ui, "Preview") {
                self.print_preview = Some(crate::print_preview::PrintPreview::new(
                    self.doc.composite(),
                ));
                close = true;
            }
            if dialog_button(ui, "Close") {
                close = true;
            }
        });

        close
    }

    fn import_dialog(&mut self, ui: &mut Ui, ctx: &Context) -> bool {
        let mut close = false;

        if self.job.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Waiting for the device…");
            });
        } else if self.devices.devices.is_empty() {
            ui.label("No scanners or cameras found.");
        }
        for warning in &self.devices.warnings {
            ui.label(warning);
        }
        for (i, device) in self.devices.devices.iter().enumerate() {
            ui.radio_value(&mut self.device_index, i, &device.name);
        }
        if let Some(device) = self.devices.devices.get(self.device_index) {
            if device.kind == crate::integration::DeviceKind::Scanner {
                ui.horizontal(|ui| {
                    ui.label("Resolution:");
                    let mut dpi = self.capture_settings.resolution_dpi.unwrap_or(150);
                    if ui
                        .add(DragValue::new(&mut dpi).range(75..=600).suffix(" DPI"))
                        .changed()
                    {
                        self.capture_settings.resolution_dpi = Some(dpi);
                    }
                });
                ComboBox::from_id_salt("scan_mode")
                    .selected_text(format!("{:?}", self.capture_settings.scan_mode))
                    .show_ui(ui, |ui| {
                        use crate::integration::ScanMode;
                        for (mode, label) in [
                            (ScanMode::DeviceDefault, "Device default"),
                            (ScanMode::Color, "Color"),
                            (ScanMode::Gray, "Grayscale"),
                            (ScanMode::Lineart, "Black and white"),
                        ] {
                            ui.selectable_value(&mut self.capture_settings.scan_mode, mode, label);
                        }
                    });
            }
        }
        ui.horizontal(|ui| {
            if default_button(
                ui,
                "Capture",
                self.job.is_none() && self.devices.devices.get(self.device_index).is_some(),
            ) {
                let device = self.devices.devices[self.device_index].clone();
                let settings = self.capture_settings.clone();
                let cancel = self.job_cancel.clone();
                self.start_job(ctx, move || {
                    JobResult::Image(crate::integration::capture(&device, &settings, &cancel))
                });
            }
            let refresh = ui.add_enabled(self.job.is_none(), Button::new("Refresh"));
            register_button(ui, &refresh);
            if refresh.clicked() {
                self.start_job(ctx, || {
                    JobResult::Devices(crate::integration::enumerate_devices())
                });
            }
            if dialog_button(ui, "Cancel") {
                self.job_cancel
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                close = true;
            }
        });

        close
    }

    fn wallpaper_dialog(&mut self, ui: &mut Ui, ctx: &Context) -> bool {
        let mut close = false;

        ui.label("Choose how the picture should fit your screen.");
        use crate::integration::WallpaperStyle;
        ui.horizontal_wrapped(|ui| {
            for (style, name) in [
                (WallpaperStyle::Fill, "Fill"),
                (WallpaperStyle::Fit, "Fit"),
                (WallpaperStyle::Stretch, "Stretch"),
                (WallpaperStyle::Tile, "Tile"),
                (WallpaperStyle::Center, "Center"),
            ] {
                ui.selectable_value(&mut self.wallpaper_style, style, name);
            }
        });
        ui.horizontal(|ui| {
            ui.label("Screen:");
            let width = ui.add(DragValue::new(&mut self.wallpaper_size.0).range(1..=16384));
            initial_focus(ui, &width);
            ui.label("×");
            ui.add(DragValue::new(&mut self.wallpaper_size.1).range(1..=16384));
        });
        ui.horizontal(|ui| {
            if default_button(ui, "Apply…", self.job.is_none()) {
                let img = self.doc.composite();
                let style = self.wallpaper_style;
                let size = self.wallpaper_size;
                self.start_job(ctx, move || {
                    JobResult::Status(
                        crate::integration::set_wallpaper(&img, style, size)
                            .map(|_| "Desktop background updated".into()),
                    )
                });
                close = true;
            }
            if dialog_button(ui, "Cancel") {
                close = true;
            }
        });

        close
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paint_hls_matches_primary_colors_and_gray() {
        for (rgb, hsl) in [
            ([255, 0, 0], [0, 240, 120]),
            ([255, 255, 0], [40, 240, 120]),
            ([0, 255, 0], [80, 240, 120]),
            ([0, 255, 255], [120, 240, 120]),
            ([0, 0, 255], [160, 240, 120]),
            ([255, 0, 255], [200, 240, 120]),
            ([0, 0, 0], [160, 0, 0]),
            ([255, 255, 255], [160, 0, 240]),
        ] {
            assert_eq!(rgb_to_hsl(rgb), hsl);
            assert_eq!(hsl_to_rgb(hsl), rgb);
        }
        for hue in 0..240 {
            assert_eq!(hsl_to_rgb([hue, 0, 120]), [128, 128, 128]);
        }
    }

    #[test]
    fn paint_hls_round_trip_stays_within_integer_quantization() {
        for red in (0..=255).step_by(17) {
            for green in (0..=255).step_by(17) {
                for blue in (0..=255).step_by(17) {
                    let rgb = [red, green, blue];
                    let hsl = rgb_to_hsl(rgb);
                    assert!(hsl[0] < 240 && hsl[1] <= 240 && hsl[2] <= 240);
                    let restored = hsl_to_rgb(hsl);
                    for (original, restored) in rgb.into_iter().zip(restored) {
                        assert!(original.abs_diff(restored) <= 4, "{rgb:?} -> {hsl:?}");
                    }
                }
            }
        }
    }

    fn enter() -> Event {
        Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    fn modal_frame(ctx: &Context, events: Vec<Event>, value: &mut u32) -> (bool, bool) {
        let mut accepted = false;
        let mut canceled = false;
        let _ = ctx.run(
            RawInput {
                events,
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(640.0, 480.0))),
                ..Default::default()
            },
            |ctx| {
                canceled |= prepare_modal(ctx, "Test dialog");
                let shown = Window::new("Test dialog")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        let field = ui.add(DragValue::new(value).range(1..=1000));
                        initial_focus(ui, &field);
                        accepted |= default_button(ui, "OK", true);
                        canceled |= dialog_button(ui, "Cancel");
                    });
                if let Some(shown) = shown {
                    ctx.memory_mut(|memory| memory.set_modal_layer(shown.response.layer_id));
                }
            },
        );
        (accepted, canceled)
    }

    #[test]
    fn modal_initial_numeric_focus_selects_all_and_enter_accepts() {
        let ctx = Context::default();
        let mut value = 100;
        assert_eq!(modal_frame(&ctx, vec![], &mut value), (false, false));
        assert_eq!(ctx.memory(|memory| memory.focused()), None);
        // The first frame measures the Window; the next one is interactive.
        assert_eq!(modal_frame(&ctx, vec![], &mut value), (false, false));
        assert_eq!(
            modal_frame(&ctx, vec![Event::Text("42".into())], &mut value),
            (false, false)
        );
        assert_eq!(value, 42);
        assert_eq!(modal_frame(&ctx, vec![enter()], &mut value), (true, false));
    }

    #[test]
    fn modal_rejects_invalid_numeric_input_without_losing_focus() {
        let ctx = Context::default();
        let mut value = 100;
        modal_frame(&ctx, vec![], &mut value);
        modal_frame(&ctx, vec![], &mut value);
        modal_frame(&ctx, vec![Event::Text("invalid".into())], &mut value);
        let field = ctx.memory(|memory| memory.focused());
        assert_eq!(modal_frame(&ctx, vec![enter()], &mut value), (false, false));
        assert_eq!(value, 100);
        assert_eq!(ctx.memory(|memory| memory.focused()), field);
        assert!(ctx.data(|data| data
            .get_temp::<ModalKeys>(modal_key())
            .unwrap()
            .invalid_number));
    }

    #[test]
    fn enter_on_cancel_does_not_also_activate_the_default() {
        let ctx = Context::default();
        let mut value = 100;
        modal_frame(&ctx, vec![], &mut value);
        modal_frame(&ctx, vec![], &mut value);
        let cancel = ctx.data(|data| {
            let state = data.get_temp::<ModalKeys>(modal_key()).unwrap();
            *state.buttons.last().unwrap()
        });
        ctx.memory_mut(|memory| memory.request_focus(cancel));
        assert_eq!(modal_frame(&ctx, vec![enter()], &mut value), (false, true));
    }
}
