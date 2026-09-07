use super::dialogs::{default_button, dialog_button, initial_focus};
use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

impl PaintApp {
    pub(in crate::app) fn properties_dialog(&mut self, ui: &mut Ui) -> bool {
        let file_metadata = self
            .file
            .as_ref()
            .and_then(|path| std::fs::metadata(path).ok());
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
                    || "Not saved".into(),
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
                        .map_or_else(|| "Not saved".into(), format_file_date),
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
        ui.label("Resize the canvas without stretching the picture.");
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
                    let response = ui.add(
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
