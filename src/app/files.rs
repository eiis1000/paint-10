use super::*;
use crate::{file_dialogs::SaveChoice, raster_io::RasterFormat};

impl PaintApp {
    pub(in crate::app) fn file_dialog() -> rfd::FileDialog {
        rfd::FileDialog::new()
            .add_filter(
                "Pictures and Paint 10 projects",
                &[
                    "png", "jpg", "jpeg", "jpe", "jfif", "bmp", "dib", "gif", "tif", "tiff",
                    "webp", "ico", "p10",
                ],
            )
            .add_filter("All files", &["*"])
    }

    pub(in crate::app) fn read_image(path: &std::path::Path) -> Result<RgbaImage, String> {
        crate::raster_io::decode(path)
    }

    fn remember_file(&mut self, path: &std::path::Path) {
        match crate::preferences::record_file(path) {
            Ok(recent) => self.recent = recent,
            Err(error) => {
                self.message
                    .push_str(&format!(". Recent files could not be updated: {error}"));
            }
        }
    }

    pub(in crate::app) fn load(&mut self, path: PathBuf) {
        let format =
            crate::raster_io::detect_format(&path).or_else(|| RasterFormat::from_path(&path));
        let project = format == Some(RasterFormat::Project);
        let result = if project {
            crate::project::load(&path)
        } else {
            crate::raster_io::decode_with_resolution(&path).map(|(image, resolution)| {
                let mut opaque = RgbaImage::from_pixel(image.width(), image.height(), Rgba(WHITE));
                imageops::overlay(&mut opaque, &image, 0, 0);
                let mut document = Document::from_image(opaque);
                document.mono = format == Some(RasterFormat::BmpMono);
                document.resolution = resolution;
                document
            })
        };
        match result {
            Ok(document) => {
                self.doc = document;
                self.file = Some(path.clone());
                self.clear_selection();
                self.refresh = true;
                self.message = if project {
                    "Editable project opened"
                } else {
                    "Picture opened"
                }
                .into();
                self.remember_file(&path);
            }
            Err(error) => self.message = error,
        }
    }

    pub(in crate::app) fn save(&mut self, save_as: bool) -> bool {
        self.commit_shape();
        let initial_format = self
            .file
            .as_deref()
            .and_then(|path| {
                crate::raster_io::detect_format(path).or_else(|| RasterFormat::from_path(path))
            })
            .unwrap_or(if self.doc.mono {
                RasterFormat::BmpMono
            } else {
                RasterFormat::Png
            });
        let choice = if save_as || self.file.is_none() {
            match crate::file_dialogs::save_dialog(self.file.as_deref(), initial_format) {
                Ok(Some(choice)) => choice,
                Ok(None) => return false,
                Err(error) => {
                    self.message = error;
                    return false;
                }
            }
        } else {
            SaveChoice {
                path: self.file.clone().expect("existing destination"),
                format: initial_format,
            }
        };
        let result = if choice.format == RasterFormat::Project {
            crate::project::save(&self.doc, &choice.path)
        } else {
            crate::raster_io::encode_with_resolution(
                &self.doc.composite(),
                choice.format,
                self.doc.resolution,
            )
            .and_then(|bytes| crate::project::atomic_write(&choice.path, &bytes))
        };
        match result {
            Ok(()) => {
                self.doc.mark_saved();
                self.message = format!("Saved {}", choice.path.display());
                self.remember_file(&choice.path);
                self.file = Some(choice.path);
                true
            }
            Err(error) => {
                self.message = format!("Could not save: {error}");
                false
            }
        }
    }
}
