use super::*;

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::app) enum DownloadAction {
    #[default]
    None,
    Download,
    Cancel,
}

#[cfg(any(target_arch = "wasm32", test))]
pub(in crate::app) fn browser_download_controls(
    ctx: &Context,
    name: &mut String,
    format: &mut crate::raster_io::RasterFormat,
    copy: bool,
    error: Option<&str>,
    initial_focus: &mut bool,
) -> DownloadAction {
    let dialog_id = Id::new("browser_download");
    let filename_id = dialog_id.with("filename");
    let queued_id = dialog_id.with("opening_input");
    let popup_was_open = ctx.memory(|memory| memory.any_popup_open());
    let mut action = DownloadAction::None;
    let shown = egui::Modal::new(dialog_id)
        .frame(Frame::window(&ctx.style()))
        .show(ctx, |ui| {
            let width = 380.0_f32.min((ctx.screen_rect().width() - 56.0).max(180.0));
            ui.set_width(width);
            ui.spacing_mut().interact_size.y = 28.0;
            ui.heading(if copy {
                "Download a copy"
            } else {
                "Save picture"
            });
            ui.add_space(4.0);
            let download_note = concat!(
                "The browser downloads a file; ",
                "it cannot replace the original automatically."
            );
            ui.add(Label::new(RichText::new(download_note).weak()).wrap());
            ui.add_space(12.0);
            if *initial_focus {
                if ui.is_enabled() && !ui.is_sizing_pass() {
                    ctx.memory_mut(|memory| memory.request_focus(filename_id));
                    select_number(ctx, filename_id);
                    let mut queued = ctx.data_mut(|data| {
                        data.remove_temp::<Vec<Event>>(queued_id)
                            .unwrap_or_default()
                    });
                    ctx.input_mut(|input| {
                        queued.append(&mut input.events);
                        input.events = queued;
                    });
                    *initial_focus = false;
                } else {
                    let mut queued = Vec::new();
                    ctx.input_mut(|input| {
                        input.events.retain(|event| {
                            if matches!(
                                event,
                                Event::Text(_)
                                    | Event::Paste(_)
                                    | Event::Key {
                                        key: Key::Enter | Key::Tab,
                                        ..
                                    }
                            ) {
                                queued.push(event.clone());
                                false
                            } else {
                                true
                            }
                        });
                    });
                    ctx.data_mut(|data| {
                        data.get_temp_mut_or_default::<Vec<Event>>(queued_id)
                            .extend(queued);
                    });
                    ctx.request_repaint();
                }
            }
            let enter_filename = ui.is_enabled()
                && !popup_was_open
                && ctx.memory(|memory| memory.has_focus(filename_id))
                && ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Enter));
            let filename_label = ui.label("Filename");
            ui.add_sized([width, 28.0], TextEdit::singleline(name).id(filename_id))
                .labelled_by(filename_label.id);
            ui.add_space(8.0);
            let format_label = ui.label("File type");
            // Keep the original salt while moving the label above the field.
            ComboBox::from_id_salt("File type")
                .width(width)
                .selected_text(format.label())
                .show_ui(ui, |ui| {
                    theme::menu(ui);
                    for choice in crate::raster_io::RasterFormat::ALL {
                        if ui
                            .add(
                                theme::MenuItem::new(choice.label())
                                    .selected(*format == choice)
                                    .width(ui.available_width()),
                            )
                            .clicked()
                        {
                            *format = choice;
                        }
                    }
                })
                .response
                .labelled_by(format_label.id);
            if *format == crate::raster_io::RasterFormat::Project {
                ui.add_space(4.0);
                ui.add(
                    Label::new(
                        RichText::new("Keeps text, images, transparency, and editable transforms.")
                            .weak(),
                    )
                    .wrap(),
                );
            }
            if let Some(error) = error {
                ui.add_space(8.0);
                ui.add(Label::new(RichText::new(error).color(ui.visuals().error_fg_color)).wrap());
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let button_size = vec2(92.0, 30.0);
                ui.add_space((ui.available_width() - 2.0 * button_size.x - 8.0).max(0.0));
                ui.spacing_mut().item_spacing.x = 8.0;
                let valid = !name.trim().is_empty();
                if ui
                    .add_enabled(valid, Button::new("Download").min_size(button_size))
                    .clicked()
                    || (valid && enter_filename)
                {
                    action = DownloadAction::Download;
                }
                if ui
                    .add(Button::new("Cancel").min_size(button_size))
                    .clicked()
                {
                    action = DownloadAction::Cancel;
                }
            });
        });
    if shown.is_top_modal
        && !shown.any_popup_open
        && ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape))
    {
        action = DownloadAction::Cancel;
    }
    if action != DownloadAction::None {
        ctx.data_mut(|data| data.remove::<Vec<Event>>(queued_id));
    }
    action
}

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

fn pending_modal_input_key() -> Id {
    Id::new("paint10-modal-pending-input")
}

fn defer_modal_keyboard(ctx: &Context, events: &mut Vec<Event>) {
    let mut deferred = Vec::new();
    events.retain(|event| {
        let escape = matches!(event, Event::Key {
            key: Key::Escape, pressed: true, modifiers, ..
        } if modifiers.is_none());
        let keyboard = matches!(
            event,
            Event::Key { .. }
                | Event::Text(_)
                | Event::Paste(_)
                | Event::Copy
                | Event::Cut
                | Event::Ime(_)
        );
        if keyboard && !escape {
            deferred.push(event.clone());
            false
        } else {
            true
        }
    });
    if !deferred.is_empty() {
        ctx.data_mut(|data| {
            data.get_temp_mut_or_default::<Vec<Event>>(pending_modal_input_key())
                .extend(deferred);
        });
        ctx.request_repaint();
    }
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

fn settle_dialog_geometry(ctx: &Context, response: &Response) {
    // An anchored egui window positions itself using its previous frame's size.
    // Repaint after content changes so its controls settle before the next click.
    let key = response.id.with("paint10-dialog-size");
    let size = response.rect.size();
    let previous = ctx.data_mut(|data| {
        let previous = data.get_temp::<Vec2>(key);
        data.insert_temp(key, size);
        previous
    });
    if previous != Some(size) {
        ctx.request_repaint();
    }
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

fn color_spectrum(ui: &mut Ui, hsl: &mut [u16; 3]) -> bool {
    let before = *hsl;
    let (rect, response) = ui.allocate_exact_size(vec2(250.0, 166.0), Sense::click_and_drag());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "Hue and saturation"));
    if response.clicked() || response.dragged() {
        if let Some(point) = response.interact_pointer_pos() {
            hsl[0] =
                (((point.x - rect.left()) / rect.width()).clamp(0.0, 1.0) * 239.0).round() as u16;
            hsl[1] = ((1.0 - (point.y - rect.top()) / rect.height()).clamp(0.0, 1.0) * 240.0)
                .round() as u16;
        }
    }
    let mut mesh = egui::Mesh::default();
    for row in 0..=8 {
        for column in 0..=24 {
            let rgb = hsl_to_rgb([column * 10, 240 - row * 30, 120]);
            mesh.colored_vertex(
                rect.min
                    + vec2(
                        column as f32 / 24.0 * rect.width(),
                        row as f32 / 8.0 * rect.height(),
                    ),
                Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
            );
            if row < 8 && column < 24 {
                let index = u32::from(row * 25 + column);
                mesh.add_triangle(index, index + 1, index + 25);
                mesh.add_triangle(index + 1, index + 26, index + 25);
            }
        }
    }
    ui.painter().add(mesh);
    ui.painter().rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0_f32, Color32::from_gray(140)),
        StrokeKind::Inside,
    );
    let position = rect.min
        + vec2(
            hsl[0] as f32 / 239.0 * rect.width(),
            (1.0 - hsl[1] as f32 / 240.0) * rect.height(),
        );
    let painter = ui.painter().with_clip_rect(rect);
    painter.circle_stroke(position, 5.0, Stroke::new(3.0_f32, Color32::WHITE));
    painter.circle_stroke(position, 5.0, Stroke::new(1.0_f32, Color32::BLACK));
    response.on_hover_text(
        "Choose hue and saturation. The numeric fields provide exact keyboard entry.",
    );
    before != *hsl
}

fn luminosity_strip(ui: &mut Ui, hsl: &mut [u16; 3]) -> bool {
    let before = *hsl;
    let (rect, response) = ui.allocate_exact_size(vec2(20.0, 166.0), Sense::click_and_drag());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "Luminosity"));
    if response.clicked() || response.dragged() {
        if let Some(point) = response.interact_pointer_pos() {
            hsl[2] = ((1.0 - (point.y - rect.top()) / rect.height()).clamp(0.0, 1.0) * 240.0)
                .round() as u16;
        }
    }
    for row in 0..166 {
        let rgb = hsl_to_rgb([
            hsl[0],
            hsl[1],
            240 - (row as f32 / 165.0 * 240.0).round() as u16,
        ]);
        ui.painter().rect_filled(
            Rect::from_min_size(rect.min + vec2(0.0, row as f32), vec2(rect.width(), 1.0)),
            0.0,
            Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
        );
    }
    let y = rect.top() + (1.0 - hsl[2] as f32 / 240.0) * rect.height();
    let marker = Rect::from_center_size(pos2(rect.center().x, y), vec2(24.0, 4.0));
    ui.painter().rect_stroke(
        marker,
        0.0,
        Stroke::new(2.0_f32, Color32::WHITE),
        StrokeKind::Middle,
    );
    ui.painter().rect_stroke(
        marker,
        0.0,
        Stroke::new(1.0_f32, Color32::BLACK),
        StrokeKind::Middle,
    );
    response.on_hover_text("Adjust luminosity from black to white.");
    before != *hsl
}

fn color_swatch(ui: &mut Ui, color: Color, size: Vec2, label: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    register_button(ui, &response);
    canvas::checkerboard(ui.painter(), rect, 5.0);
    ui.painter().rect_filled(
        rect.shrink(1.0),
        0.0,
        Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3]),
    );
    ui.painter().rect_stroke(
        rect,
        0.0,
        Stroke::new(
            if response.hovered() || response.has_focus() {
                2.0_f32
            } else {
                1.0_f32
            },
            if response.hovered() || response.has_focus() {
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

fn prepare_modal(ctx: &Context, kind: &str) -> bool {
    let previous = ctx
        .data(|data| data.get_temp::<ModalKeys>(modal_key()))
        .unwrap_or_default();
    let popup = keytips::popup_open(ctx);
    let initial = previous.initial || previous.kind != kind;
    if initial && !popup {
        // Opening a Window may need a disabled sizing pass, followed by a pass
        // that assigns initial focus. Neither can consume text for that field.
        // Keep input that arrived with the opening command until it is ready.
        let mut events = ctx.input_mut(|input| std::mem::take(&mut input.events));
        defer_modal_keyboard(ctx, &mut events);
        ctx.input_mut(|input| input.events = events);
    }
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
                initial,
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

pub(in crate::app) fn numeric_input(ui: &mut Ui, value: DragValue<'_>) -> Response {
    let id = ui.next_auto_id();
    // Register before constructing DragValue's inner TextEdit so a Tab focus
    // transition can select the existing number before any typing is handled.
    if ui.is_enabled() && !ui.is_sizing_pass() {
        ui.memory_mut(|memory| memory.interested_in_focus(id, ui.layer_id()));
        if ui.memory(|memory| memory.has_focus(id) && !memory.had_focus_last_frame(id)) {
            select_number(ui.ctx(), id);
        }
    }
    ui.add(value)
}

fn select_number(ctx: &Context, id: Id) {
    let mut state = TextEdit::load_state(ctx, id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(0),
            egui::text::CCursor::new(usize::MAX),
        )));
    state.store(ctx, id);
}

pub(in crate::app) fn initial_focus(ui: &Ui, response: &Response) {
    if response.enabled() && take_initial_focus(ui) {
        response.request_focus();
        select_number(ui.ctx(), response.id);
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
    let response = ui.add(Button::new(label).min_size(ui.spacing().interact_size));
    register_button(ui, &response);
    if response.enabled() && take_initial_focus(ui) {
        response.request_focus();
    }
    response.clicked()
}

pub(in crate::app) fn default_button(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    let response = ui.add_enabled(
        enabled,
        Button::new(label).min_size(ui.spacing().interact_size),
    );
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
    pub(in crate::app) fn modal_raw_input(&mut self, ctx: &Context, input: &mut RawInput) {
        let key = pending_modal_input_key();
        if self.dialog.is_none() && self.pending.is_none() {
            if let Some(mut events) = ctx.data_mut(|data| data.remove_temp::<Vec<Event>>(key)) {
                // Successful dialog completion must preserve later input. Keep
                // a queued click ahead of newer commands, just as the normal
                // canvas input hook does for pointer release boundaries.
                events.append(&mut input.events);
                if let Some(release) = events.iter().position(|event| {
                    matches!(
                        event,
                        Event::PointerButton {
                            button: PointerButton::Primary | PointerButton::Secondary,
                            pressed: false,
                            ..
                        }
                    )
                }) {
                    let remainder = events.split_off(release + 1);
                    if !remainder.is_empty() {
                        if let Event::PointerButton { modifiers, .. } = events[release] {
                            input.modifiers = modifiers;
                        }
                        ctx.data_mut(|data| data.insert_temp(key, remainder));
                        ctx.request_repaint();
                    }
                }
                input.events = events;
            }
            return;
        }
        if keytips::popup_open(ctx) {
            return;
        }
        if input.events.iter().any(|event| {
            matches!(event, Event::Key { key: Key::Escape, pressed: true, modifiers, .. }
                if modifiers.is_none())
        }) {
            ctx.data_mut(|data| data.remove::<Vec<Event>>(key));
            return;
        }
        let mut events = ctx
            .data_mut(|data| data.remove_temp::<Vec<Event>>(key))
            .unwrap_or_default();
        events.append(&mut input.events);
        if ctx.data(|data| {
            data.get_temp::<ModalKeys>(modal_key())
                .is_none_or(|state| state.initial)
        }) {
            defer_modal_keyboard(ctx, &mut events);
            input.events = events;
            return;
        }
        let tab = events.iter().position(|event| {
            matches!(event, Event::Key { key: Key::Tab, pressed: true, modifiers, .. }
                if !modifiers.ctrl && !modifiers.command && !modifiers.alt)
        });
        let clicked_typing = events.iter().enumerate().find_map(|(index, event)| {
            if matches!(
                event,
                Event::PointerButton {
                    button: PointerButton::Primary,
                    pressed: false,
                    ..
                }
            ) && events[index + 1..]
                .iter()
                .any(|event| matches!(event, Event::Text(_) | Event::Paste(_)))
            {
                Some(index + 1)
            } else {
                None
            }
        });
        let typing_before_click = events.iter().enumerate().find_map(|(index, event)| {
            if matches!(
                event,
                Event::PointerButton {
                    button: PointerButton::Primary,
                    pressed: true,
                    ..
                }
            ) && events[..index]
                .iter()
                .any(|event| matches!(event, Event::Text(_) | Event::Paste(_)))
            {
                Some(index)
            } else {
                None
            }
        });
        let boundary = [
            tab.map(|index| if index == 0 { 1 } else { index }),
            clicked_typing,
            typing_before_click,
        ]
        .into_iter()
        .flatten()
        .min();
        if let Some(boundary) = boundary {
            // egui resolves Tab focus before widgets process text. Keep the
            // preceding text in its field, then give Tab a separate pass before
            // the following text, preserving the event order at any frame rate.
            // A clicked DragValue also enters text mode on the following frame.
            let remainder = events.split_off(boundary);
            if !remainder.is_empty() {
                ctx.data_mut(|data| data.insert_temp(key, remainder));
                ctx.request_repaint();
            }
        }
        input.events = events;
    }

    pub(in crate::app) fn dialogs(&mut self, ctx: &Context) {
        if ctx.data(|data| {
            data.get_temp::<Vec<Event>>(pending_modal_input_key())
                .is_some_and(|events| !events.is_empty())
        }) {
            ctx.request_repaint();
        }
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
                ctx.data_mut(|data| data.remove::<Vec<Event>>(pending_modal_input_key()));
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
                settle_dialog_geometry(ctx, &shown.response);
                ctx.memory_mut(|memory| memory.set_modal_layer(shown.response.layer_id));
            }
        }
        if !self.measure.enabled {
            self.text_editor(ctx);
        }
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
                settle_dialog_geometry(ctx, &shown.response);
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
                    self.resize_dimensions()
                };
            }
        });
        let original = self.resize_dimensions();
        Grid::new("dimensions")
            .num_columns(2)
            .spacing(vec2(16.0, 10.0))
            .show(ui, |ui| {
                ui.label("Horizontal:");
                let horizontal = numeric_input(
                    ui,
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
                if numeric_input(
                    ui,
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
        ui.checkbox(&mut self.pixel_resize, "Keep hard pixel edges (pixel art)")
            .on_hover_text("Use nearest-neighbor scaling to preserve the exact palette. Leave off for smoother photographs.");
        ui.separator();
        ui.strong("Skew (Degrees)");
        for (label, angle) in [
            ("Horizontal:", &mut self.skew_x),
            ("Vertical:", &mut self.skew_y),
        ] {
            ui.horizontal(|ui| {
                ui.label(label);
                numeric_input(ui, DragValue::new(angle).range(-89.0..=89.0).suffix("°"));
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
                if let Err(error) =
                    self.resize_picture(dimensions.0, dimensions.1, self.skew_x, self.skew_y)
                {
                    self.dialog_error = Some(error);
                    return;
                }
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
            let angle = numeric_input(
                ui,
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
        ui.set_width(430.0);
        ui.horizontal_top(|ui| {
            let spectrum_changed = color_spectrum(ui, &mut state.hsl);
            let luminosity_changed = luminosity_strip(ui, &mut state.hsl);
            if spectrum_changed || luminosity_changed {
                rgb = hsl_to_rgb(state.hsl);
            }
            ui.add_space(4.0);
            Grid::new("paint_color_channels")
                .num_columns(2)
                .spacing(vec2(8.0, 6.0))
                .show(ui, |ui| {
                    for (index, label) in ["Red", "Green", "Blue"].into_iter().enumerate() {
                        ui.label(label);
                        let previous_rgb = rgb;
                        let response = numeric_input(ui, DragValue::new(&mut rgb[index]));
                        if index == 0 {
                            initial_focus(ui, &response);
                        }
                        if previous_rgb != rgb {
                            state.hsl = rgb_to_hsl(rgb);
                        }
                        ui.end_row();
                    }
                    for (index, label, maximum) in [
                        (0, "Hue", 239),
                        (1, "Saturation", 240),
                        (2, "Luminosity", 240),
                    ] {
                        ui.label(label);
                        let previous_hsl = state.hsl;
                        numeric_input(ui, DragValue::new(&mut state.hsl[index]).range(0..=maximum));
                        if previous_hsl != state.hsl {
                            rgb = hsl_to_rgb(state.hsl);
                        }
                        ui.end_row();
                    }
                });
        });
        ui.add_space(8.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.label("Basic colors");
                Grid::new("dialog_basic_colors")
                    .num_columns(10)
                    .min_col_width(19.0)
                    .spacing(vec2(4.0, 4.0))
                    .show(ui, |ui| {
                        for (index, color) in ribbon::PALETTE.into_iter().enumerate() {
                            if color_swatch(
                                ui,
                                [color[0], color[1], color[2], 255],
                                vec2(19.0, 19.0),
                                ribbon::PALETTE_NAMES[index],
                            )
                            .clicked()
                            {
                                rgb = color;
                                state.hsl = rgb_to_hsl(rgb);
                            }
                            if index % 10 == 9 {
                                ui.end_row();
                            }
                        }
                    });
                ui.label("Custom colors");
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for index in 0..10 {
                        let saved = self.custom_colors.get(index).copied();
                        let label = saved.map_or_else(
                            || format!("Empty custom color {}", index + 1),
                            |[r, g, b, _]| {
                                format!("Custom color {}: #{r:02X}{g:02X}{b:02X}", index + 1)
                            },
                        );
                        if color_swatch(
                            ui,
                            saved.unwrap_or([255, 255, 255, 255]),
                            vec2(19.0, 19.0),
                            &label,
                        )
                        .clicked()
                        {
                            if let Some(color) = saved {
                                rgb = [color[0], color[1], color[2]];
                                state.hsl = rgb_to_hsl(rgb);
                            }
                        }
                    }
                });
            });
            ui.add_space(12.0);
            ui.vertical(|ui| {
                ui.label("Current");
                if color_swatch(
                    ui,
                    state.original,
                    vec2(68.0, 40.0),
                    "Restore the original color",
                )
                .clicked()
                {
                    rgb = [state.original[0], state.original[1], state.original[2]];
                    state.hsl = rgb_to_hsl(rgb);
                }
            });
            ui.vertical(|ui| {
                ui.label("New");
                color_swatch(
                    ui,
                    [rgb[0], rgb[1], rgb[2], 255],
                    vec2(68.0, 40.0),
                    "New color",
                );
            });
        });
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
        let add = ui.add_enabled(valid_hex, Button::new("Add to custom colors"));
        register_button(ui, &add);
        if add.clicked() {
            if self.custom_colors.len() == 10 {
                self.custom_colors.remove(0);
            }
            self.custom_colors.push(self.colors[self.active_color]);
            if let Err(error) = crate::preferences::save_custom_colors(&self.custom_colors) {
                self.dialog_error = Some(format!("Could not save custom colors: {error}"));
            }
        }
        ui.add_space(6.0);
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), 28.0),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.spacing_mut().interact_size = vec2(76.0, 28.0);
                if dialog_button(ui, "Cancel") {
                    self.colors[self.active_color] = state.original;
                    close = true;
                }
                if default_button(ui, "OK", valid_hex) {
                    close = true;
                }
            },
        );

        if self.colors[self.active_color] != c {
            // Palette and Hex edits occur after the RGB/HSL controls are drawn.
            // Repaint once so every preview and channel reflects the new color.
            ui.ctx().request_repaint();
        }

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
        use crate::printing::{PaperSize, MAX_PAGES, MAX_PAPER_MM};

        let mut close = false;

        ui.strong("Page setup");
        ui.horizontal(|ui| {
            ui.label("Paper:");
            let paper = ComboBox::from_id_salt("print_paper")
                .width(225.0)
                .selected_text(self.page.paper.name())
                .show_ui(ui, |ui| {
                    for paper in PaperSize::PRESETS {
                        let (width, height) = paper.dimensions_mm();
                        ui.selectable_value(
                            &mut self.page.paper,
                            paper,
                            format!("{} ({width:.2} × {height:.2} mm)", paper.name()),
                        );
                    }
                    ui.separator();
                    if ui
                        .selectable_label(
                            matches!(self.page.paper, PaperSize::Custom { .. }),
                            "Custom paper size",
                        )
                        .clicked()
                    {
                        let (width_mm, height_mm) = self.page.paper.dimensions_mm();
                        self.page.paper = PaperSize::Custom {
                            width_mm,
                            height_mm,
                        };
                    }
                });
            register_button(ui, &paper.response);
        });
        if let PaperSize::Custom {
            width_mm,
            height_mm,
        } = &mut self.page.paper
        {
            ui.label("Custom paper size (mm)");
            ui.horizontal(|ui| {
                ui.label("Width");
                numeric_input(
                    ui,
                    DragValue::new(width_mm)
                        .range(0.0..=MAX_PAPER_MM)
                        .clamp_existing_to_range(false)
                        .max_decimals(3)
                        .speed(0.5),
                );
                ui.label("Height");
                numeric_input(
                    ui,
                    DragValue::new(height_mm)
                        .range(0.0..=MAX_PAPER_MM)
                        .clamp_existing_to_range(false)
                        .max_decimals(3)
                        .speed(0.5),
                );
            });
            ui.small("Landscape swaps width and height.");
        } else {
            let (width, height) = self.page.paper.dimensions_mm();
            ui.label(format!("{width:.2} × {height:.2} mm"));
        }
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
                let response = numeric_input(ui, DragValue::new(value).range(0.0..=MAX_PAPER_MM));
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
                numeric_input(
                    ui,
                    DragValue::new(&mut self.page.fit_across).range(1..=MAX_PAGES),
                );
                ui.label("across by");
                numeric_input(
                    ui,
                    DragValue::new(&mut self.page.fit_down).range(1..=MAX_PAGES),
                );
                ui.label("down");
            });
        }
        if !self.page.fit {
            ui.horizontal(|ui| {
                ui.label("Scale:");
                numeric_input(
                    ui,
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
        let layout = self.page.layout(&self.doc.image);
        let valid = layout.is_ok();
        match layout {
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
            #[cfg(not(target_arch = "wasm32"))]
            if default_button(ui, "Print…", valid) {
                match crate::printing::print(&self.doc.composite(), &self.page) {
                    Ok(()) => {
                        self.message = "Print job submitted".into();
                        close = true;
                    }
                    Err(e) => self.dialog_error = Some(format!("Print: {e}")),
                }
            }
            #[cfg(target_arch = "wasm32")]
            if default_button(ui, "Download PDF", valid) {
                match crate::printing::pdf(&self.doc.composite(), &self.page).and_then(|bytes| {
                    crate::web::download("Paint10.pdf", "application/pdf", &bytes)
                }) {
                    Ok(()) => {
                        self.message = "PDF download started".into();
                        self.dialog_error = None;
                        close = true;
                    }
                    Err(error) => {
                        self.dialog_error = Some(format!("Could not download PDF: {error}"));
                    }
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            if ui
                .add_enabled_ui(valid, |ui| dialog_button(ui, "Save PDF…"))
                .inner
            {
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
            if ui
                .add_enabled_ui(valid, |ui| dialog_button(ui, "Preview"))
                .inner
            {
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
                    if numeric_input(ui, DragValue::new(&mut dpi).range(75..=600).suffix(" DPI"))
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
            let width = numeric_input(
                ui,
                DragValue::new(&mut self.wallpaper_size.0).range(1..=16384),
            );
            initial_focus(ui, &width);
            ui.label("×");
            numeric_input(
                ui,
                DragValue::new(&mut self.wallpaper_size.1).range(1..=16384),
            );
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

    #[test]
    fn color_dialog_fits_500_pixels_and_cancel_restores_the_original_rgba() {
        let ctx = Context::default();
        ctx.enable_accesskit();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let original = [40, 80, 120, 128];
        app.colors[0] = original;
        app.hex = "285078".into();
        app.dialog = Some(Dialog::Colors);
        let frame = |app: &mut PaintApp, events: Vec<Event>| {
            let mut input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(500.0, 500.0))),
                time: Some(ctx.cumulative_pass_nr() as f64 / 30.0),
                events,
                ..Default::default()
            };
            eframe::App::raw_input_hook(app, &ctx, &mut input);
            ctx.run(input, |ctx| app.dialogs(ctx))
        };
        frame(&mut app, vec![]);
        frame(&mut app, vec![]);
        let output = frame(&mut app, vec![]);
        let nodes = &output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes;
        let visible_controls: Vec<_> = nodes
            .iter()
            .filter_map(|(_, node)| Some((node.label().or(node.value())?, node.bounds()?)))
            .collect();
        for label in [
            "Hue and saturation",
            "Luminosity",
            "Basic colors",
            "Custom colors",
            "OK",
            "Cancel",
        ] {
            let bounds = nodes
                .iter()
                .find_map(|(_, node)| {
                    (node.label() == Some(label) || node.value() == Some(label))
                        .then(|| node.bounds())
                        .flatten()
                })
                .unwrap_or_else(|| panic!("missing {label}"));
            assert!(
                bounds.x0 >= 0.0 && bounds.x1 <= 500.0,
                "{label}: {bounds:?}; controls: {visible_controls:?}"
            );
            assert!(
                bounds.y0 >= 0.0 && bounds.y1 <= 500.0,
                "{label}: {bounds:?}"
            );
        }
        let bounds = nodes
            .iter()
            .find_map(|(_, node)| {
                (node.label() == Some("Hue and saturation"))
                    .then(|| node.bounds())
                    .flatten()
            })
            .unwrap();
        let point = pos2(
            (bounds.x0 + (bounds.x1 - bounds.x0) * 0.25) as f32,
            (bounds.y0 + 10.0) as f32,
        );
        frame(
            &mut app,
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
        frame(
            &mut app,
            vec![Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert_ne!(app.colors[0], original);
        assert_eq!(app.colors[0][3], 255);
        frame(
            &mut app,
            vec![Event::Key {
                key: Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert_eq!(app.colors[0], original);
        assert!(app.dialog.is_none());
        assert!(!app.doc.dirty() && !app.doc.can_undo());
    }

    #[test]
    fn anchored_color_dialog_settles_before_repaints_become_idle() {
        let ctx = Context::default();
        ctx.enable_accesskit();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.dialog = Some(Dialog::Colors);

        for error in [
            None,
            Some("Choose a color.\nChoose a color.\nChoose a color."),
        ] {
            app.dialog_error = error.map(str::to_owned);
            let mut previous = None;
            let mut settled = false;
            for _ in 0..12 {
                let mut input = RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(900.0, 700.0))),
                    time: Some(ctx.cumulative_pass_nr() as f64 / 30.0),
                    ..Default::default()
                };
                eframe::App::raw_input_hook(&mut app, &ctx, &mut input);
                let output = ctx.run(input, |ctx| app.dialogs(ctx));
                let bounds = output
                    .platform_output
                    .accesskit_update
                    .as_ref()
                    .unwrap()
                    .nodes
                    .iter()
                    .find_map(|(_, node)| {
                        (node.label() == Some("Hue and saturation"))
                            .then(|| node.bounds())
                            .flatten()
                    })
                    .expect("The real color controls must be present");
                let geometry = (bounds.x0, bounds.y0, bounds.x1, bounds.y1);
                let repaint = output.viewport_output[&ViewportId::ROOT].repaint_delay;
                if repaint != std::time::Duration::ZERO {
                    assert_eq!(
                        previous,
                        Some(geometry),
                        "The dialog must request another repaint while its controls are moving"
                    );
                    settled = true;
                    break;
                }
                previous = Some(geometry);
            }
            assert!(
                settled,
                "A settled dialog must not keep repainting continuously"
            );
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
                        let field = numeric_input(ui, DragValue::new(value).range(1..=1000));
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

    fn download_frame(
        ctx: &Context,
        events: Vec<Event>,
        name: &mut String,
        initial: &mut bool,
    ) -> DownloadAction {
        let mut action = DownloadAction::None;
        let _ = ctx.run(
            RawInput {
                events,
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(640.0, 480.0))),
                ..Default::default()
            },
            |ctx| {
                action = browser_download_controls(
                    ctx,
                    name,
                    &mut crate::raster_io::RasterFormat::Png,
                    false,
                    None,
                    initial,
                );
            },
        );
        action
    }

    #[test]
    fn browser_download_selects_filename_preserves_early_typing_and_accepts_enter() {
        for first_input_frame in 0..=2 {
            let ctx = Context::default();
            let mut name = "Untitled".to_owned();
            let mut initial = true;
            let mut downloaded = false;
            for frame in 0..4 {
                let events = if frame == first_input_frame {
                    vec![Event::Text("My picture".into()), enter()]
                } else {
                    Vec::new()
                };
                let action = download_frame(&ctx, events, &mut name, &mut initial);
                downloaded |= action == DownloadAction::Download;
                if downloaded {
                    break;
                }
            }
            assert!(downloaded, "initial input frame {first_input_frame}");
            assert_eq!(name, "My picture");
        }
    }

    #[test]
    fn browser_download_escape_cancels_and_enter_respects_focused_cancel() {
        let ctx = Context::default();
        let mut name = "Untitled".to_owned();
        let mut initial = true;
        for _ in 0..3 {
            download_frame(&ctx, Vec::new(), &mut name, &mut initial);
        }
        let tab = Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        for _ in 0..3 {
            assert_eq!(
                download_frame(&ctx, vec![tab.clone()], &mut name, &mut initial),
                DownloadAction::None
            );
        }
        assert_eq!(
            download_frame(&ctx, vec![enter()], &mut name, &mut initial),
            DownloadAction::Cancel
        );
        let escape = Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(
            download_frame(&ctx, vec![escape], &mut name, &mut initial),
            DownloadAction::Cancel
        );
        assert!(!ctx.input(|input| input.key_pressed(Key::Escape)));
    }

    #[test]
    fn browser_download_format_popup_owns_enter_and_escape() {
        let ctx = Context::default();
        let mut name = "Untitled".to_owned();
        let mut initial = true;
        for _ in 0..3 {
            download_frame(&ctx, Vec::new(), &mut name, &mut initial);
        }
        let key = |key| Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        download_frame(&ctx, vec![key(Key::Tab)], &mut name, &mut initial);
        assert_eq!(
            download_frame(&ctx, vec![enter()], &mut name, &mut initial),
            DownloadAction::None
        );
        assert!(ctx.memory(|memory| memory.any_popup_open()));
        download_frame(&ctx, Vec::new(), &mut name, &mut initial);
        assert_eq!(
            download_frame(&ctx, vec![key(Key::Escape)], &mut name, &mut initial),
            DownloadAction::None
        );
        assert!(!ctx.memory(|memory| memory.any_popup_open()));
        assert_eq!(
            download_frame(&ctx, vec![key(Key::Escape)], &mut name, &mut initial),
            DownloadAction::Cancel
        );
    }
}
