use super::dialogs::{default_button, dialog_button, initial_focus, numeric_input};
use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

impl PaintApp {
    pub(in crate::app) fn properties_dialog(&mut self, ui: &mut Ui) -> bool {
        let file_metadata = self
            .file
            .as_ref()
            .and_then(|path| std::fs::metadata(path).ok());
        let unavailable_metadata = if cfg!(target_arch = "wasm32") {
            "Unavailable in browser"
        } else {
            "Not saved"
        };
        Grid::new("image_file_properties")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("File:");
                ui.label(
                    self.file
                        .as_ref()
                        .and_then(|path| path.file_name())
                        .map_or_else(
                            || "Not saved".into(),
                            |name| name.to_string_lossy().into_owned(),
                        ),
                );
                ui.end_row();
                ui.label("Size on disk:");
                ui.label(file_metadata.as_ref().map_or_else(
                    || unavailable_metadata.into(),
                    |metadata| {
                        format!(
                            "{} bytes ({:.1} KiB)",
                            metadata.len(),
                            metadata.len() as f64 / 1024.0
                        )
                    },
                ));
                ui.end_row();
                ui.label("Last saved:");
                ui.label(
                    file_metadata
                        .as_ref()
                        .and_then(|metadata| metadata.modified().ok())
                        .map_or_else(|| unavailable_metadata.into(), format_file_date),
                );
                ui.end_row();
                ui.label("Resolution:");
                ui.label(format!(
                    "{:.2} × {:.2} dots per inch",
                    self.doc.resolution.x, self.doc.resolution.y
                ));
                ui.end_row();
            });
        ui.separator();
        ui.strong("Canvas size");
        ui.horizontal(|ui| {
            ui.label("Units:");
            ui.radio_value(&mut self.unit, 0, "Pixels");
            ui.radio_value(&mut self.unit, 1, "Inches");
            ui.radio_value(&mut self.unit, 2, "Centimeters");
        });
        let (horizontal, vertical) = self.doc.resolution.pixels_per_unit(self.unit);
        Grid::new("image_physical_dimensions")
            .num_columns(2)
            .spacing(vec2(16.0, 10.0))
            .show(ui, |ui| {
                for (label, pixels, factor) in [
                    ("Width:", &mut self.resize_w, horizontal),
                    ("Height:", &mut self.resize_h, vertical),
                ] {
                    ui.label(label);
                    let mut dimension = *pixels as f64 / factor;
                    let response = numeric_input(
                        ui,
                        DragValue::new(&mut dimension)
                            .range(1.0 / factor..=16384.0 / factor)
                            .speed(1.0 / factor)
                            .max_decimals(if self.unit == 0 { 0 } else { 4 }),
                    );
                    if label == "Width:" {
                        initial_focus(ui, &response);
                    }
                    if response.changed() {
                        *pixels = (dimension * factor).round().max(1.0) as u32;
                    }
                    ui.end_row();
                }
            });
        if dialog_button(ui, "Default dimensions") {
            (self.resize_w, self.resize_h) = d::DEFAULT_CANVAS_SIZE;
        }
        ui.horizontal(|ui| {
            ui.label("Colors:");
            ui.radio_value(&mut self.prop_mono, false, "Color");
            ui.radio_value(&mut self.prop_mono, true, "Black and white");
        });
        let valid = d::valid_size(self.resize_w, self.resize_h);
        if !valid {
            ui.colored_label(Color32::RED, "The canvas must fit within 16 megapixels.");
        }
        ui.add_space(12.0);
        let mut close = false;
        ui.horizontal(|ui| {
            if default_button(ui, "OK", valid) {
                self.doc.begin();
                self.doc.mono = self.prop_mono;
                if let Err(error) =
                    self.doc
                        .resize_canvas(self.resize_w, self.resize_h, self.colors[1])
                {
                    self.doc.cancel();
                    self.message = error;
                    return;
                }
                self.doc.commit();
                self.clear_selection();
                self.refresh = true;
                close = true;
            }
            close |= dialog_button(ui, "Cancel");
        });
        close
    }
}

fn format_file_date(time: SystemTime) -> String {
    let seconds = match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs() as i64,
        Err(error) => {
            let duration = error.duration();
            -(duration.as_secs() as i64) - i64::from(duration.subsec_nanos() > 0)
        }
    };
    #[cfg(target_os = "linux")]
    if let Ok(date) = gtk::glib::DateTime::from_unix_local(seconds) {
        if let Ok(formatted) = date.format("%Y-%m-%d %H:%M:%S %z") {
            return formatted.into();
        }
    }
    format_utc_date(seconds)
}

fn format_utc_date(seconds: i64) -> String {
    // Gregorian dates repeat on a 400-year cycle; this also handles dates before 1970.
    let days = seconds.div_euclid(86400) + 719468;
    let era = days.div_euclid(146097);
    let day_of_era = days - era * 146097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = month_index + if month_index < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    let clock = seconds.rem_euclid(86400);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        clock / 3600,
        clock / 60 % 60,
        clock % 60
    )
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
        let time = ctx.input(|input| input.time) + 0.045;
        let mut input = RawInput {
            time: Some(time),
            events,
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1180.0, 800.0))),
            ..Default::default()
        };
        eframe::App::raw_input_hook(app, ctx, &mut input);
        ctx.run(input, |ctx| {
            if !app.ribbon_keyboard(ctx) {
                app.shortcut(ctx);
            }
            app.titlebar(ctx);
            app.ribbon(ctx);
            app.quick_access_below(ctx);
            app.status(ctx);
            app.canvas(ctx);
            app.thumbnail(ctx);
            app.keyboard_menu(ctx);
            app.dialogs(ctx);
        })
    }

    #[test]
    fn closing_properties_preserves_the_following_zoom_reset_click() {
        for sequence in 0..4 {
            let ctx = Context::default();
            let mut app = PaintApp::new_with_context(&ctx, false);
            app.zoom = 8.0;
            let output = app_frame(&mut app, &ctx, Vec::new());
            let reset = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Text(text) if text.galley.job.text == "800%" => {
                        Some(text.pos + text.galley.size() / 2.0)
                    }
                    _ => None,
                })
                .expect("visible zoom reset button");
            app_frame(&mut app, &ctx, vec![key(Key::E, Modifiers::CTRL)]);
            for _ in 0..3 {
                app_frame(&mut app, &ctx, Vec::new());
            }
            app_frame(&mut app, &ctx, vec![Event::Text("1920".into())]);
            app_frame(&mut app, &ctx, vec![key(Key::Tab, Modifiers::NONE)]);
            let click = vec![
                Event::PointerMoved(reset),
                Event::PointerButton {
                    pos: reset,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
                Event::PointerButton {
                    pos: reset,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ];
            let mut closing = Vec::new();
            if sequence == 1 || sequence == 3 {
                // The last number and Enter leave the click in the modal queue.
                closing.push(Event::Text("1080".into()));
            } else {
                app_frame(&mut app, &ctx, vec![Event::Text("1080".into())]);
            }
            closing.push(key(Key::Enter, Modifiers::NONE));
            if sequence == 2 {
                app_frame(&mut app, &ctx, closing);
                app_frame(&mut app, &ctx, click);
            } else {
                closing.extend(click);
                if sequence == 3 {
                    closing.push(key(Key::PageDown, Modifiers::CTRL));
                }
                app_frame(&mut app, &ctx, closing);
            }
            for _ in 0..4 {
                app_frame(&mut app, &ctx, Vec::new());
            }
            assert!(app.dialog.is_none());
            assert_eq!(app.doc.image.dimensions(), (1920, 1080));
            if sequence != 3 {
                assert_eq!(app.zoom, 1.0, "close/click sequence {sequence}");
                app_frame(&mut app, &ctx, vec![key(Key::PageDown, Modifiers::CTRL)]);
            }
            assert_eq!(app.zoom, 0.5);
        }
    }

    #[test]
    fn properties_accepts_separate_digits_and_tab_without_corrupting_dimensions() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app_frame(&mut app, &ctx, Vec::new());
        app_frame(&mut app, &ctx, vec![key(Key::E, Modifiers::CTRL)]);
        for _ in 0..3 {
            app_frame(&mut app, &ctx, Vec::new());
        }
        for digit in ["7", "2", "0"] {
            app_frame(&mut app, &ctx, vec![Event::Text(digit.into())]);
        }
        assert_eq!(app.resize_w, 720);
        app_frame(&mut app, &ctx, vec![key(Key::Tab, Modifiers::NONE)]);
        for digit in ["5", "6", "0"] {
            app_frame(&mut app, &ctx, vec![Event::Text(digit.into())]);
        }
        assert_eq!((app.resize_w, app.resize_h), (720, 560));
        app_frame(&mut app, &ctx, vec![key(Key::Enter, Modifiers::NONE)]);
        assert!(app.dialog.is_none());
        assert_eq!(app.doc.image.dimensions(), (720, 560));
    }

    #[test]
    fn properties_keeps_typing_during_opening_and_initial_focus() {
        for first_input_frame in 0..=3 {
            let ctx = Context::default();
            let mut app = PaintApp::new_with_context(&ctx, false);
            app_frame(&mut app, &ctx, Vec::new());
            let mut opening = vec![key(Key::E, Modifiers::CTRL)];
            if first_input_frame == 0 {
                opening.extend([Event::Text("9".into()), Event::Text("6".into())]);
            }
            app_frame(&mut app, &ctx, opening);
            if first_input_frame > 0 {
                for _ in 1..first_input_frame {
                    app_frame(&mut app, &ctx, Vec::new());
                }
                app_frame(
                    &mut app,
                    &ctx,
                    vec![Event::Text("9".into()), Event::Text("6".into())],
                );
            }
            app_frame(&mut app, &ctx, vec![key(Key::Tab, Modifiers::NONE)]);
            app_frame(&mut app, &ctx, Vec::new());
            app_frame(&mut app, &ctx, vec![Event::Text("64".into())]);
            app_frame(&mut app, &ctx, vec![key(Key::Enter, Modifiers::NONE)]);
            for _ in 0..5 {
                app_frame(&mut app, &ctx, Vec::new());
            }
            assert!(app.dialog.is_none());
            assert_eq!(
                app.doc.image.dimensions(),
                (96, 64),
                "first input in frame {first_input_frame} after opening"
            );
        }
    }

    #[test]
    fn numeric_dialogs_keep_the_first_number_before_the_window_has_focus() {
        for dialog in [
            Dialog::Resize,
            Dialog::Colors,
            Dialog::Rotate,
            Dialog::Print,
        ] {
            let ctx = Context::default();
            let mut app = PaintApp::new_with_context(&ctx, false);
            app_frame(&mut app, &ctx, Vec::new());
            if dialog == Dialog::Resize {
                app.action(Action::Resize, &ctx);
                app.aspect = false;
            } else {
                app.dialog = Some(dialog);
            }
            app_frame(&mut app, &ctx, vec![Event::Text("96".into())]);
            for _ in 0..4 {
                app_frame(&mut app, &ctx, Vec::new());
            }
            match dialog {
                Dialog::Resize => assert_eq!(app.resize_w, 96),
                Dialog::Colors => assert_eq!(app.colors[app.active_color], [96, 0, 0, 255]),
                Dialog::Rotate => assert_eq!(app.angle, 96.0),
                Dialog::Print => assert_eq!(app.page.margin_left_mm, 96.0),
                _ => unreachable!(),
            }
            app_frame(&mut app, &ctx, vec![key(Key::Escape, Modifiers::NONE)]);
            assert!(app.dialog.is_none());
            assert!(!app.doc.dirty());
        }
    }

    #[test]
    fn escape_cancels_opening_before_early_typing_can_reach_the_canvas() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app_frame(&mut app, &ctx, Vec::new());
        app_frame(
            &mut app,
            &ctx,
            vec![key(Key::E, Modifiers::CTRL), Event::Text("96".into())],
        );
        app_frame(
            &mut app,
            &ctx,
            vec![
                key(Key::G, Modifiers::CTRL),
                key(Key::Escape, Modifiers::NONE),
            ],
        );
        for _ in 0..4 {
            app_frame(&mut app, &ctx, Vec::new());
        }
        assert!(app.dialog.is_none());
        assert_eq!(app.doc.image.dimensions(), (900, 600));
        assert!(!app.doc.dirty());
        assert!(app.text_edit.is_none());
        assert!(!app.grid);
    }

    #[test]
    fn disabled_default_action_does_not_hold_dialog_keyboard_input_forever() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.dialog = Some(Dialog::Import);
        assert!(app.devices.devices.is_empty());
        for _ in 0..4 {
            app_frame(&mut app, &ctx, Vec::new());
        }
        // Capture is unavailable without a device. Cancel remains an enabled
        // initial focus target and accepts Enter without querying any hardware.
        app_frame(&mut app, &ctx, vec![key(Key::Enter, Modifiers::NONE)]);
        assert!(app.dialog.is_none());
        assert!(app.job.is_none());
    }

    #[test]
    fn properties_preserves_digits_on_both_sides_of_a_batched_tab() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app_frame(&mut app, &ctx, Vec::new());
        app_frame(&mut app, &ctx, vec![key(Key::E, Modifiers::CTRL)]);
        for _ in 0..3 {
            app_frame(&mut app, &ctx, Vec::new());
        }
        app_frame(&mut app, &ctx, vec![Event::Text("7".into())]);
        app_frame(
            &mut app,
            &ctx,
            vec![
                Event::Text("2".into()),
                Event::Text("0".into()),
                key(Key::Tab, Modifiers::NONE),
                Event::Text("5".into()),
                Event::Text("6".into()),
                Event::Text("0".into()),
                key(Key::Enter, Modifiers::NONE),
            ],
        );
        for _ in 0..4 {
            app_frame(&mut app, &ctx, Vec::new());
        }
        assert!(app.dialog.is_none());
        assert_eq!(app.doc.image.dimensions(), (720, 560));
    }

    #[test]
    fn modal_numeric_tab_entry_also_preserves_resize_colors_and_print_margins() {
        for dialog in [Dialog::Resize, Dialog::Colors, Dialog::Print] {
            let ctx = Context::default();
            let mut app = PaintApp::new_with_context(&ctx, false);
            app_frame(&mut app, &ctx, Vec::new());
            match dialog {
                Dialog::Resize => {
                    app.action(Action::Resize, &ctx);
                    app.aspect = false;
                }
                _ => app.dialog = Some(dialog),
            }
            for _ in 0..4 {
                app_frame(&mut app, &ctx, Vec::new());
            }
            let (first, second) = match dialog {
                Dialog::Resize => ("720", "560"),
                Dialog::Colors => ("128", "64"),
                Dialog::Print => ("18.5", "24.5"),
                _ => unreachable!(),
            };
            app_frame(
                &mut app,
                &ctx,
                vec![
                    Event::Text(first.into()),
                    key(Key::Tab, Modifiers::NONE),
                    Event::Text(second.into()),
                ],
            );
            for _ in 0..4 {
                app_frame(&mut app, &ctx, Vec::new());
            }
            match dialog {
                Dialog::Resize => assert_eq!((app.resize_w, app.resize_h), (720, 560)),
                Dialog::Colors => assert_eq!(app.colors[app.active_color], [128, 64, 0, 255]),
                Dialog::Print => {
                    assert_eq!(app.page.margin_left_mm, 18.5);
                    assert_eq!(app.page.margin_right_mm, 24.5);
                }
                _ => unreachable!(),
            }
            assert!(app.dialog == Some(dialog));
            app_frame(&mut app, &ctx, vec![key(Key::Escape, Modifiers::NONE)]);
            assert!(app.dialog.is_none());
            assert_eq!(app.doc.image.dimensions(), (900, 600));
        }
    }

    #[test]
    fn escape_discards_pending_modal_typing_before_it_can_accept_the_dialog() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app_frame(&mut app, &ctx, Vec::new());
        app_frame(&mut app, &ctx, vec![key(Key::E, Modifiers::CTRL)]);
        for _ in 0..3 {
            app_frame(&mut app, &ctx, Vec::new());
        }
        app_frame(
            &mut app,
            &ctx,
            vec![
                Event::Text("720".into()),
                key(Key::Tab, Modifiers::NONE),
                Event::Text("560".into()),
                key(Key::Enter, Modifiers::NONE),
            ],
        );
        app_frame(&mut app, &ctx, vec![key(Key::Escape, Modifiers::NONE)]);
        for _ in 0..4 {
            app_frame(&mut app, &ctx, Vec::new());
        }
        assert!(app.dialog.is_none());
        assert_eq!(app.doc.image.dimensions(), (900, 600));
        assert!(!app.doc.dirty());
    }

    #[test]
    fn properties_accepts_a_number_typed_in_the_same_batch_as_its_mouse_click() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app_frame(&mut app, &ctx, Vec::new());
        app_frame(&mut app, &ctx, vec![key(Key::E, Modifiers::CTRL)]);
        for _ in 0..3 {
            app_frame(&mut app, &ctx, Vec::new());
        }
        let width = ctx.memory(|memory| memory.focused().unwrap());
        app_frame(&mut app, &ctx, vec![key(Key::Tab, Modifiers::NONE)]);
        let point = ctx.read_response(width).unwrap().rect.center();
        app_frame(
            &mut app,
            &ctx,
            vec![
                Event::Text("560".into()),
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
                key(Key::A, Modifiers::CTRL | Modifiers::COMMAND),
                Event::Text("720".into()),
                key(Key::Enter, Modifiers::NONE),
            ],
        );
        for _ in 0..3 {
            app_frame(&mut app, &ctx, Vec::new());
        }
        assert!(app.dialog.is_none());
        assert_eq!(app.doc.image.dimensions(), (720, 560));
    }

    #[test]
    fn physical_units_use_each_axis_resolution() {
        let resolution = paint_10::metadata::Resolution { x: 300.0, y: 150.0 };
        let (x, y) = resolution.pixels_per_unit(1);
        assert_eq!((600.0 / x, 600.0 / y), (2.0, 4.0));
        let (x, y) = resolution.pixels_per_unit(2);
        assert!((600.0 / x - 5.08).abs() < 0.0001);
        assert!((600.0 / y - 10.16).abs() < 0.0001);
    }

    #[test]
    fn fallback_dates_handle_epoch_leap_days_and_negative_timestamps() {
        assert_eq!(format_utc_date(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(format_utc_date(951782400), "2000-02-29 00:00:00 UTC");
        assert_eq!(format_utc_date(-1), "1969-12-31 23:59:59 UTC");
    }
}
