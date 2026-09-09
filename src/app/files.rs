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
        if crate::raster_io::detect_format(path) == Some(RasterFormat::Project) {
            // Paste from and file drop insert a project's visible picture.
            // Open retains its editable objects through the document loader.
            crate::project::load(path).map(|document| document.composite())
        } else {
            crate::raster_io::decode(path)
        }
    }

    fn remember_file(&mut self, path: &std::path::Path) {
        if !self.persist_preferences {
            return;
        }
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
        match read_document(&path, format) {
            Ok(document) => {
                self.doc = document;
                self.reset_layer_panel_state();
                self.measure.reset();
                self.register_document_fonts();
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
        self.save_with_format(save_as, None)
    }

    pub(in crate::app) fn save_as_format(&mut self, format: RasterFormat) -> bool {
        self.finish_editing();
        self.save_with_format(true, Some(format))
    }

    fn save_with_format(&mut self, save_as: bool, preferred: Option<RasterFormat>) -> bool {
        self.save_with_dialog(save_as, preferred, crate::file_dialogs::save_dialog)
    }

    fn save_with_dialog(
        &mut self,
        save_as: bool,
        preferred: Option<RasterFormat>,
        choose: impl FnOnce(
            Option<&std::path::Path>,
            RasterFormat,
        ) -> Result<Option<SaveChoice>, String>,
    ) -> bool {
        self.commit_shape();
        let initial_format = preferred
            .or_else(|| {
                self.file.as_deref().and_then(|path| {
                    crate::raster_io::detect_format(path).or_else(|| RasterFormat::from_path(path))
                })
            })
            .unwrap_or(if self.doc.mono {
                RasterFormat::BmpMono
            } else {
                RasterFormat::Png
            });
        let choice = if save_as || self.file.is_none() {
            match choose(self.file.as_deref(), initial_format) {
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

    pub(in crate::app) fn export_copy(&mut self, ctx: &Context, selection_only: bool) {
        let selection = match self.prepare_export(selection_only) {
            Ok(selection) => selection,
            Err(error) => {
                self.message = error;
                return;
            }
        };
        let suffix = if selection_only { "selection" } else { "copy" };
        let source = self
            .file
            .as_deref()
            .unwrap_or_else(|| std::path::Path::new("Untitled"));
        let name = source.file_stem().unwrap_or_default().to_string_lossy();
        let suggested = source.with_file_name(format!("{name} - {suffix}.png"));
        let title = if selection_only {
            "Save selection as — Paint 10"
        } else {
            "Save a copy — Paint 10"
        };
        match crate::file_dialogs::save_dialog_with_title(
            Some(&suggested),
            RasterFormat::Png,
            title,
        ) {
            Ok(Some(choice)) => {
                self.message = match self.write_export(&choice, selection.as_ref()) {
                    Ok(()) => format!("Saved {suffix} to {}", choice.path.display()),
                    Err(error) => format!("Could not save {suffix}: {error}"),
                };
            }
            Ok(None) => {}
            Err(error) => self.message = error,
        }
        ctx.request_repaint();
    }

    fn prepare_export(&mut self, selection_only: bool) -> Result<Option<RgbaImage>, String> {
        let shape = self.finish_editing();
        if selection_only {
            self.selected_image()
                .or_else(|| shape.map(|bounds| bounds.extract(&self.doc.active_composite())))
                .map(Some)
                .ok_or_else(|| "Select part of the picture before saving a selection.".into())
        } else {
            Ok(None)
        }
    }

    /// Writing a copy cannot alter the open document or its saved revision.
    fn write_export(
        &self,
        choice: &SaveChoice,
        selection: Option<&RgbaImage>,
    ) -> Result<(), String> {
        if self.file.as_ref().is_some_and(|source| {
            source == &choice.path
                || source
                    .canonicalize()
                    .ok()
                    .is_some_and(|source| choice.path.canonicalize().ok().as_ref() == Some(&source))
        }) {
            return Err(
                "Choose a different filename for the copy. Use Save to replace the open file."
                    .into(),
            );
        }
        if choice.format == RasterFormat::Project {
            if let Some(image) = selection {
                let mut document = Document::from_image(image.clone());
                document.resolution = self.doc.resolution;
                document.mono = self.doc.mono;
                crate::project::save(&document, &choice.path)
            } else {
                crate::project::save(&self.doc, &choice.path)
            }
        } else {
            let composite;
            let image = if let Some(selection) = selection {
                selection
            } else {
                composite = self.doc.composite();
                &composite
            };
            crate::raster_io::encode_with_resolution(image, choice.format, self.doc.resolution)
                .and_then(|bytes| crate::project::atomic_write(&choice.path, &bytes))
        }
    }
}

fn read_document(path: &std::path::Path, format: Option<RasterFormat>) -> Result<Document, String> {
    if format == Some(RasterFormat::Project) {
        crate::project::load(path)
    } else {
        crate::raster_io::decode_with_resolution(path).map(|(image, resolution)| {
            let mut document = Document::from_image(image);
            document.mono = format == Some(RasterFormat::BmpMono);
            document.resolution = resolution;
            document
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn importing_a_project_inserts_its_composite_as_one_movable_undoable_picture() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.p10");
        let mut source = Document::new(80, 60);
        let mut picture = Object::new(
            ObjectKind::Image(RgbaImage::from_pixel(12, 8, Rgba([30, 90, 160, 128]))),
            (5, 6),
        );
        picture.angle = 90.0;
        source.add_object(picture);
        source.add_object(Object::new(
            ObjectKind::Text {
                text: "Caption".into(),
                format: crate::text::TextFormat {
                    width: 70,
                    ..Default::default()
                },
            },
            (2, 24),
        ));
        let expected = source.composite();
        let source_bytes = crate::project::encode(&source).unwrap();
        std::fs::write(&path, &source_bytes).unwrap();

        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.doc = Document::new(100, 80);
        let original = app.doc.composite();
        let working_file = directory.path().join("working.p10");
        app.file = Some(working_file.clone());
        app.doc.mark_saved();

        // Both native Paste from and native file drop use this import path.
        let imported = PaintApp::read_image(&path).unwrap();
        assert_eq!(imported, expected);
        app.insert_image(imported);
        assert_eq!(app.file, Some(working_file));
        let selected = app.object.expect("the imported picture is selected");
        assert_eq!(app.doc.objects[selected].render(), expected);
        assert!(matches!(
            app.doc.objects[selected].kind,
            ObjectKind::Image(_)
        ));
        assert_eq!(
            app.doc
                .objects
                .iter()
                .filter(|object| !matches!(object.kind, ObjectKind::Raster(_)))
                .count(),
            1,
            "the original raster backdrop is separate from the inserted picture"
        );
        assert!(app.doc.dirty());

        app.action(Action::Undo, &ctx);
        assert_eq!(app.doc.composite(), original);
        assert!(app.doc.objects.is_empty());
        assert!(!app.doc.dirty());
        assert_eq!(std::fs::read(&path).unwrap(), source_bytes);
    }

    #[test]
    fn image_import_detects_project_contents_and_preserves_raster_alpha() {
        let directory = tempfile::tempdir().unwrap();
        let image = RgbaImage::from_fn(8, 5, |x, y| {
            Rgba([x as u8 * 30, y as u8 * 50, 17, (x * 31) as u8])
        });
        let project = directory.path().join("project-without-extension");
        std::fs::write(
            &project,
            crate::project::encode(&Document::from_image(image.clone())).unwrap(),
        )
        .unwrap();
        assert_eq!(PaintApp::read_image(&project).unwrap(), image);

        let raster = directory.path().join("sprite.png");
        std::fs::write(
            &raster,
            crate::raster_io::encode(&image, RasterFormat::Png).unwrap(),
        )
        .unwrap();
        assert_eq!(PaintApp::read_image(&raster).unwrap(), image);

        let invalid = directory.path().join("broken.p10");
        std::fs::write(&invalid, b"PAINT10\0incomplete project").unwrap();
        assert!(PaintApp::read_image(&invalid).is_err());
    }

    #[test]
    fn explicit_save_as_formats_reach_the_chooser_without_changing_the_open_file() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let path = PathBuf::from("/tmp/paint10-open-project.p10");
        app.file = Some(path.clone());
        app.doc.mark_saved();
        for format in RasterFormat::ALL {
            let mut called = false;
            assert!(
                !app.save_with_dialog(true, Some(format), |initial_path, initial_format| {
                    called = true;
                    assert_eq!(initial_path, Some(path.as_path()));
                    assert_eq!(initial_format, format);
                    Ok(None)
                })
            );
            assert!(called);
            assert_eq!(app.file.as_ref(), Some(&path));
            assert!(!app.doc.dirty());
        }
        app.save_with_dialog(true, None, |_, initial_format| {
            assert_eq!(initial_format, RasterFormat::Project);
            Ok(None)
        });
    }

    #[test]
    fn draft_shape_export_uses_active_pixels_but_picture_export_uses_visible_stack() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.doc = Document::from_image(RgbaImage::from_pixel(80, 60, Rgba([20, 160, 80, 255])));
        app.doc.add_layer().unwrap();
        app.doc.add_layer().unwrap();
        app.doc.image = RgbaImage::from_pixel(80, 60, Rgba([25, 70, 220, 255]));
        app.doc.set_active_layer(1).unwrap();
        app.set_tool(Tool::Rectangle);
        app.doc.begin();
        app.start_shape_draft(
            ShapeGeometry::Primitive {
                tool: Tool::Rectangle,
                start: (10, 10),
                end: (50, 40),
            },
            0,
        );
        let bounds = app
            .shape_draft
            .as_ref()
            .unwrap()
            .bounds(&app.doc.image)
            .unwrap();
        let selection = app.prepare_export(true).unwrap().unwrap();
        assert_eq!(selection, bounds.extract(&app.doc.active_composite()));
        assert_ne!(selection, bounds.extract(&app.doc.composite()));
        let directory = tempfile::tempdir().unwrap();
        let choice = SaveChoice {
            path: directory.path().join("stack.png"),
            format: RasterFormat::Png,
        };
        app.write_export(&choice, None).unwrap();
        assert_eq!(
            PaintApp::read_image(&choice.path).unwrap(),
            app.doc.composite()
        );
        let choice = SaveChoice {
            path: directory.path().join("stack.p10"),
            format: RasterFormat::Project,
        };
        app.write_export(&choice, None).unwrap();
        let restored = read_document(&choice.path, Some(RasterFormat::Project)).unwrap();
        assert!(restored.layers() == app.doc.layers());
        assert_eq!(restored.active_layer_index(), 1);
    }

    #[test]
    fn exported_copies_preserve_the_source_file_layers_and_saved_revision() {
        for dirty in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let ctx = Context::default();
            let mut app = PaintApp::new_with_context(&ctx, false);
            app.doc = Document::new(24, 16);
            app.doc.begin();
            app.doc.add_object(Object::new(
                ObjectKind::Image(RgbaImage::from_pixel(3, 2, Rgba([20, 50, 90, 128]))),
                (4, 5),
            ));
            app.doc.commit();
            app.doc.mark_saved();
            let source = directory.path().join("editable.p10");
            crate::project::save(&app.doc, &source).unwrap();
            let source_bytes = std::fs::read(&source).unwrap();
            app.file = Some(source.clone());
            if dirty {
                app.doc.begin();
                d::stamp(&mut app.doc.image, (0, 0), 1, BLACK, Brush::Round);
                app.doc.commit();
            }
            let composite = app.doc.composite();
            let objects = app.doc.objects.clone();
            for format in [RasterFormat::Png, RasterFormat::Tiff, RasterFormat::Project] {
                let choice = SaveChoice {
                    path: directory
                        .path()
                        .join(format!("copy.{}", format.extensions()[0])),
                    format,
                };
                let selection = app.prepare_export(false).unwrap();
                app.write_export(&choice, selection.as_ref()).unwrap();
                assert_eq!(app.file, Some(source.clone()));
                assert_eq!(app.doc.dirty(), dirty);
                assert_eq!(app.doc.composite(), composite);
                assert!(app.doc.objects == objects);
                assert_eq!(std::fs::read(&source).unwrap(), source_bytes);
                let exported = read_document(&choice.path, Some(format)).unwrap();
                assert_eq!(exported.composite(), composite);
                if format == RasterFormat::Project {
                    assert!(exported.objects == objects);
                }
            }
            assert!(app
                .write_export(
                    &SaveChoice {
                        path: source,
                        format: RasterFormat::Project
                    },
                    None
                )
                .is_err());
        }
    }

    #[test]
    fn selection_export_keeps_dimensions_mask_alpha_and_resolution() {
        let directory = tempfile::tempdir().unwrap();
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.doc = Document::from_image(RgbaImage::from_fn(12, 10, |x, y| {
            Rgba([x as u8 * 20, y as u8 * 20, 100, 255])
        }));
        app.doc.resolution = paint_10::metadata::Resolution { x: 300.0, y: 150.0 };
        app.selection = Some(Region {
            x: 4,
            y: 3,
            w: 3,
            h: 2,
        });
        app.mask = Some(image::GrayImage::from_fn(3, 2, |x, y| {
            image::Luma([if x == y { 0 } else { 255 }])
        }));
        let selection = app.prepare_export(true).unwrap().unwrap();
        assert_eq!(selection.dimensions(), (3, 2));
        assert_eq!(selection.get_pixel(0, 0)[3], 0);
        assert_eq!(selection.get_pixel(1, 0).0, [100, 60, 100, 255]);
        for format in [RasterFormat::Png, RasterFormat::Project] {
            let choice = SaveChoice {
                path: directory
                    .path()
                    .join(format!("selection.{}", format.extensions()[0])),
                format,
            };
            app.write_export(&choice, Some(&selection)).unwrap();
            let exported = read_document(&choice.path, Some(format)).unwrap();
            assert_eq!(exported.image, selection);
            assert!((exported.resolution.x - 300.0).abs() < 0.1);
            assert!((exported.resolution.y - 150.0).abs() < 0.1);
        }
        assert!(app.file.is_none());
        assert!(!app.doc.dirty());
        assert_eq!(app.doc.image.dimensions(), (12, 10));
        app.clear_selection();
        let object_image = RgbaImage::from_pixel(8, 5, Rgba([10, 90, 150, 64]));
        let index = app.doc.add_object(Object::new(
            ObjectKind::Image(object_image.clone()),
            (20, 20),
        ));
        app.select_object(index);
        let selected_object = app.prepare_export(true).unwrap().unwrap();
        assert_eq!(selected_object, object_image);
        assert_eq!(app.doc.image.dimensions(), (12, 10));
    }

    #[test]
    fn opening_transparent_png_preserves_pixel_colors_and_alpha() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sprite.png");
        let image = RgbaImage::from_fn(8, 5, |x, y| {
            Rgba([x as u8 * 30, y as u8 * 50, 17, (x * 31) as u8])
        });
        std::fs::write(
            &path,
            crate::raster_io::encode(&image, RasterFormat::Png).unwrap(),
        )
        .unwrap();
        let document = read_document(&path, Some(RasterFormat::Png)).unwrap();
        assert_eq!(document.image, image);
        assert!(!document.dirty());
    }

    #[test]
    fn exporting_finishes_pending_text_and_keeps_it_editable_and_unsaved() {
        let directory = tempfile::tempdir().unwrap();
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.doc = Document::new(320, 160);
        let index = app.doc.add_object(Object::new(
            ObjectKind::Text {
                text: "Before".into(),
                format: crate::text::TextFormat::default(),
            },
            (10, 10),
        ));
        app.doc.mark_saved();
        app.edit_text_object(index);
        app.text_edit.as_mut().unwrap().text = "Meme caption".into();
        let selection = app.prepare_export(false).unwrap();
        app.write_export(
            &SaveChoice {
                path: directory.path().join("caption.png"),
                format: RasterFormat::Png,
            },
            selection.as_ref(),
        )
        .unwrap();
        assert!(app.text_edit.is_none());
        assert!(app.doc.dirty());
        assert!(app.file.is_none());
        assert!(
            matches!(&app.doc.objects[index].kind, ObjectKind::Text { text, .. } if text == "Meme caption")
        );
        app.edit_text_object(index);
        assert_eq!(app.text_edit.as_ref().unwrap().text, "Meme caption");
    }
}
