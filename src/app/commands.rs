use super::*;

fn stroke_size_group(tool: Tool) -> Option<usize> {
    match tool {
        Tool::Brush => Some(0),
        Tool::Pencil => Some(1),
        Tool::Eraser => Some(2),
        tool if Tool::SHAPES.contains(&tool) => Some(3),
        _ => None,
    }
}

impl PaintApp {
    pub(in crate::app) fn active_layer_editable(&self) -> bool {
        let layer = self.doc.active_layer();
        layer.visible && !layer.locked
    }

    pub(in crate::app) fn ensure_active_layer_editable(&mut self) -> bool {
        if self.active_layer_editable() {
            return true;
        }
        let layer = self.doc.active_layer();
        self.message = if !layer.visible {
            format!(
                "Show the layer \"{}\" in Layers before editing it.",
                layer.name
            )
        } else {
            format!(
                "Unlock the layer \"{}\" in Layers before editing it.",
                layer.name
            )
        };
        false
    }

    /// The original Paint background uses Color 2; added sheets erase to clear.
    pub(in crate::app) fn editing_background(&self) -> Color {
        if self.doc.active_layer().is_background {
            self.colors[1]
        } else {
            [0, 0, 0, 0]
        }
    }

    fn action_edits_active_layer(&self, action: Action) -> bool {
        matches!(
            action,
            Action::Cut
                | Action::Paste
                | Action::PasteFrom
                | Action::Clear
                | Action::ClearPicture
                | Action::Invert
        ) || (matches!(action, Action::Resize | Action::Rotate(_) | Action::Flip(_))
            && (self.object.is_some() || self.selection.is_some() || self.shape_draft.is_some()))
    }

    /// Finish the current drawing operation before another command consumes
    /// the document, while retaining the committed shape's selection bounds.
    pub(in crate::app) fn finish_editing(&mut self) -> Option<Region> {
        self.finish_layer_opacity();
        self.commit_text();
        self.finish_polygon();
        let shape = self.commit_shape();
        if self.curve.take().is_some() {
            self.doc.commit();
        }
        shape
    }

    pub(in crate::app) fn set_tool(&mut self, tool: Tool) {
        if self.dialog.is_some() || self.pending.is_some() {
            return;
        }
        self.measure.enabled = false;
        self.finish_editing();
        if tool == Tool::Picker {
            self.previous_tool = self.tool;
        }
        if let Some(group) = stroke_size_group(self.tool) {
            self.tool_sizes[group] = self.size;
        }
        if let Some(group) = stroke_size_group(tool) {
            self.size = self.tool_sizes[group];
        }
        self.tool = tool;
        self.clear_selection();
        self.message.clear();
    }

    pub(in crate::app) fn action(&mut self, action: Action, ctx: &Context) {
        if self.dialog.is_some() || self.pending.is_some() {
            return;
        }
        self.finish_layer_opacity();
        if self.action_edits_active_layer(action) && !self.ensure_active_layer_editable() {
            return;
        }
        if self.text_history_action(action, ctx) || self.text_clipboard_action(action, ctx) {
            return;
        }
        if matches!(action, Action::Resize) {
            // Opening a transform dialog is not an edit. In particular, Cancel
            // must leave a live text box or adjustable shape exactly as it was.
            self.execute(action, ctx);
            return;
        }
        if matches!(action, Action::Paste) {
            // Reading an empty or unavailable clipboard must not finish an
            // unrelated text/shape edit. Valid image insertion settles it.
            self.paste_clipboard();
            return;
        }
        if self.shape_draft.is_some() {
            match action {
                Action::Copy => {
                    self.copy();
                    return;
                }
                Action::Cut => {
                    self.cut();
                    return;
                }
                Action::Clear => {
                    self.delete_selection();
                    return;
                }
                _ => {}
            }
        }
        if let Some(bounds) = self.finish_editing() {
            if matches!(
                action,
                Action::Copy
                    | Action::Cut
                    | Action::Crop
                    | Action::Resize
                    | Action::Rotate(_)
                    | Action::Flip(_)
                    | Action::Invert
                    | Action::Clear
            ) {
                self.selection = Some(bounds);
            }
        }
        if matches!(action, Action::New | Action::Open | Action::Close) && self.doc.dirty() {
            self.pending = Some(action);
            return;
        }
        self.execute(action, ctx);
    }

    pub(in crate::app) fn execute(&mut self, action: Action, ctx: &Context) {
        if self.action_edits_active_layer(action) && !self.ensure_active_layer_editable() {
            return;
        }
        match action {
            Action::New => {
                self.doc = Document::new(900, 600);
                self.reset_layer_panel_state();
                self.measure.reset();
                self.file = None;
                self.clear_selection();
                self.refresh = true;
                self.message = "New picture created".into();
            }
            Action::Open => {
                #[cfg(target_arch = "wasm32")]
                self.choose_browser_file(true);
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(path) = self
                    .pending_path
                    .take()
                    .or_else(|| Self::file_dialog().pick_file())
                {
                    self.load(path);
                }
            }
            Action::Save => {
                self.save(false);
            }
            Action::SaveAs => {
                self.save(true);
            }
            Action::Close => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
                #[cfg(target_arch = "wasm32")]
                {
                    self.message = "Close this browser tab to exit Paint 10.".into();
                    ctx.request_repaint();
                }
            }
            Action::Undo => {
                self.doc.undo();
                self.reset_layer_panel_state();
                self.clear_selection();
                self.refresh = true;
            }
            Action::Redo => {
                self.doc.redo();
                self.reset_layer_panel_state();
                self.clear_selection();
                self.refresh = true;
            }
            Action::SelectAll => {
                self.tool = Tool::Select;
                self.clear_selection();
                self.selection = Some(Region {
                    x: 0,
                    y: 0,
                    w: self.doc.image.width(),
                    h: self.doc.image.height(),
                });
            }
            Action::Copy => {
                self.copy();
            }
            Action::Cut => self.cut(),
            Action::Paste => self.paste_clipboard(),
            Action::PasteFrom => {
                #[cfg(target_arch = "wasm32")]
                self.choose_browser_file(false);
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(path) = Self::file_dialog().pick_file() {
                    match Self::read_image(&path) {
                        Ok(img) => self.insert_image(img),
                        Err(e) => self.message = e,
                    }
                }
            }
            Action::Crop => {
                if let Some(r) = self.selected_region() {
                    self.doc.begin();
                    if let Err(error) = self.doc.crop_canvas(r) {
                        self.doc.cancel();
                        self.message = error;
                        return;
                    }
                    self.doc.commit();
                    self.clear_selection();
                    self.refresh = true;
                }
            }
            Action::Resize => {
                let (w, h) = self.resize_dimensions();
                self.resize_w = w;
                self.resize_h = h;
                self.percent = false;
                self.skew_x = 0.;
                self.skew_y = 0.;
                self.angle = 0.0;
                self.dialog = Some(Dialog::Resize);
            }
            Action::Properties => {
                self.resize_w = self.doc.image.width();
                self.resize_h = self.doc.image.height();
                self.prop_mono = self.doc.mono;
                self.dialog = Some(Dialog::Properties);
            }
            Action::Rotate(angle) => {
                self.rotate_picture(angle, false);
            }
            Action::Flip(horizontal) => self.flip_picture(horizontal),
            Action::ClearPicture => {
                self.clear_selection();
                self.execute(Action::Clear, ctx);
            }
            Action::Clear => {
                if self.selection.is_some() || self.object.is_some() {
                    self.delete_selection();
                } else {
                    let bg = self.editing_background();
                    self.doc.begin();
                    self.doc.objects.clear();
                    self.doc.image = RgbaImage::from_pixel(
                        self.doc.image.width(),
                        self.doc.image.height(),
                        Rgba(bg),
                    );
                    self.doc.commit();
                    self.refresh = true;
                }
            }
            Action::Invert => {
                if let Some((mut edits, _)) = self.image_edit_target() {
                    edits.invert = !edits.invert;
                    if let Err(error) = self.apply_image_edits(edits) {
                        self.message = error;
                    }
                } else {
                    self.transform_with_key(
                        |img| {
                            let mut img = img.clone();
                            imageops::invert(&mut img);
                            img
                        },
                        |[r, g, b, a]| [255 - r, 255 - g, 255 - b, a],
                    );
                }
            }
            Action::Print => {
                self.dialog = Some(Dialog::Print);
            }
        }
    }

    pub(in crate::app) fn transform(&mut self, f: impl FnOnce(&RgbaImage) -> RgbaImage) {
        self.transform_with_key(f, |key| key);
    }

    fn flip_picture(&mut self, horizontal: bool) {
        if let Some(index) = self.object {
            self.doc.begin();
            let transform = &mut self.doc.objects[index].transform;
            if horizontal {
                transform.xx = -transform.xx;
                transform.xy = -transform.xy;
            } else {
                transform.yx = -transform.yx;
                transform.yy = -transform.yy;
            }
            self.doc.commit();
            self.refresh = true;
        } else if self.selection.is_none() {
            self.doc.begin();
            self.doc.flip_content(horizontal);
            self.doc.commit();
            self.refresh = true;
        } else {
            self.transform(|image| {
                if horizontal {
                    imageops::flip_horizontal(image)
                } else {
                    imageops::flip_vertical(image)
                }
            });
        }
    }

    /// Resizing an object includes the portion outside the canvas; cropping
    /// and copying use the visible selection instead.
    pub(in crate::app) fn resize_dimensions(&self) -> (u32, u32) {
        if let Some(state) = &self.text_edit {
            let mut object = state
                .index
                .and_then(|index| self.doc.objects.get(index))
                .cloned()
                .unwrap_or_else(|| {
                    Object::new(
                        ObjectKind::Text {
                            text: String::new(),
                            format: state.format.clone(),
                        },
                        state.origin,
                    )
                });
            object.kind = ObjectKind::Text {
                text: state.text.clone(),
                format: state.format.clone(),
            };
            if let Some(dimensions) = object.rendered_dimensions() {
                return dimensions;
            }
        }
        self.object
            .and_then(|index| self.doc.objects[index].rendered_dimensions())
            .or_else(|| self.selected_region().map(|region| (region.w, region.h)))
            .or_else(|| {
                self.pending_polygon_bounds()
                    .map(|region| (region.w, region.h))
            })
            .unwrap_or(self.doc.image.dimensions())
    }

    #[cfg(test)]
    pub(in crate::app) fn resize_picture(
        &mut self,
        width: u32,
        height: u32,
        skew_x: f32,
        skew_y: f32,
    ) -> Result<(), String> {
        self.resize_skew_rotate_picture(width, height, skew_x, skew_y, 0.0)
    }

    /// Apply the dialog's transforms in displayed coordinates, keeping their
    /// original image/text data and committing the combined change only once.
    pub(in crate::app) fn resize_skew_rotate_picture(
        &mut self,
        width: u32,
        height: u32,
        skew_x: f32,
        skew_y: f32,
        angle: f32,
    ) -> Result<(), String> {
        if !d::valid_size(width, height) {
            return Err("Dimensions must be positive and fit within 16 megapixels.".into());
        }
        super::transforms::SkewPlan::new(width, height, skew_x, skew_y)?
            .validate_rotation(angle)?;
        let angle = angle.rem_euclid(360.0);
        if (width, height) == self.resize_dimensions()
            && skew_x == 0.0
            && skew_y == 0.0
            && angle == 0.0
        {
            return Ok(());
        }
        if let Some(bounds) = self.finish_editing() {
            self.selection = Some(bounds);
        }
        if self.object.is_none() && self.selection.is_none() {
            let sampling = if self.pixel_resize {
                d::ImageSampling::Nearest
            } else {
                d::ImageSampling::Smooth
            };
            self.doc.begin();
            let result = self
                .doc
                .resize_content(width, height, sampling)
                .and_then(|()| {
                    if skew_x == 0.0 && skew_y == 0.0 {
                        Ok(())
                    } else {
                        self.doc.skew_content(skew_x, skew_y, self.colors[1])
                    }
                })
                .and_then(|()| {
                    if angle == 0.0 {
                        Ok(())
                    } else {
                        self.doc.rotate_content(angle, self.colors[1])
                    }
                });
            if let Err(error) = result {
                self.doc.cancel();
                return Err(error);
            }
            if (angle.rem_euclid(180.0) - 90.0).abs() < 0.001 {
                std::mem::swap(&mut self.doc.resolution.x, &mut self.doc.resolution.y);
            }
            self.doc.commit();
            self.refresh = true;
            return Ok(());
        }
        if !self.ensure_active_layer_editable() {
            return Err(self.message.clone());
        }
        let mut resized = if let Some(index) = self.object {
            self.doc.objects[index].clone()
        } else {
            let region = self
                .selection
                .ok_or("The selection is no longer available.")?;
            Object::new(
                ObjectKind::Image(
                    self.selected_image()
                        .ok_or("The selection is no longer available.")?,
                ),
                (region.x as i32, region.y as i32),
            )
        };
        if matches!(resized.kind, ObjectKind::Image(_)) {
            resized.image_edits.sampling = if self.pixel_resize {
                d::ImageSampling::Nearest
            } else {
                d::ImageSampling::Smooth
            };
        }
        resized.resize_rendered(width, height)?;
        resized.skew_rendered(skew_x, skew_y)?;
        resized.rotate_to(resized.angle + angle)?;
        let index = self
            .lift_selection()
            .ok_or("The selection is no longer available.")?;
        // A lifted free-form selection already carries its alpha coverage.
        // Keep its color key without introducing opaque rotation/skew corners.
        resized.color_key = self.doc.objects[index].color_key;
        self.doc.objects[index] = resized;
        self.doc.commit();
        self.refresh = true;
        Ok(())
    }

    fn transform_with_key(
        &mut self,
        f: impl FnOnce(&RgbaImage) -> RgbaImage,
        map_key: impl FnOnce(Color) -> Color,
    ) {
        self.doc.begin();
        if let Some(i) = self.object {
            let rendered = self.doc.objects[i].render_unkeyed();
            let img = f(&rendered);
            self.doc.objects[i].kind = ObjectKind::Image(img);
            self.doc.objects[i].angle = 0.;
            self.doc.objects[i].scale = 1.;
            self.doc.objects[i].transform = Default::default();
            self.doc.objects[i].image_edits = Default::default();
            self.doc.objects[i].source_clip = None;
            self.doc.objects[i].color_key = self.doc.objects[i].color_key.map(map_key);
        } else if let Some(r) = self.selection {
            let source = self.selected_image().unwrap();
            let img = f(&source);
            self.doc.flatten();
            self.clear_selected_pixels(r, &source);
            let i = self.doc.add_object(Object {
                kind: ObjectKind::Image(img),
                pos: (r.x as i32, r.y as i32),
                angle: 0.,
                scale: 1.,
                color_key: self.transparent.then(|| map_key(self.colors[1])),
                transform: Default::default(),
                image_edits: Default::default(),
                source_clip: None,
            });
            self.select_object(i);
        } else {
            self.doc.flatten();
            self.doc.image = f(&self.doc.image);
        }
        if let Some(key) = self
            .object
            .and_then(|index| self.doc.objects[index].color_key)
        {
            self.colors[1] = key;
        }
        self.doc.commit();
        self.refresh = true;
    }

    pub(in crate::app) fn rotate_picture(&mut self, angle: f32, absolute: bool) -> bool {
        if (self.object.is_some() || self.selection.is_some() || self.shape_draft.is_some())
            && !self.ensure_active_layer_editable()
        {
            return false;
        }
        // The custom-angle dialog can open over an unfinished shape or text
        // box. Apply consumes that edit; Cancel leaves it adjustable.
        if let Some(bounds) = self.finish_editing() {
            self.selection = Some(bounds);
        }
        if let Some(index) = self.object {
            let object = &self.doc.objects[index];
            let requested = if absolute {
                angle
            } else {
                object.angle + angle
            };
            let mut rotated = object.clone();
            if let Err(error) = rotated.rotate_to(requested) {
                self.message = error;
                return false;
            }
            self.doc.begin();
            self.doc.objects[index] = rotated;
            self.doc.commit();
            self.refresh = true;
        } else {
            let (width, height) = self
                .selected_region()
                .map(|region| (region.w, region.h))
                .unwrap_or(self.doc.image.dimensions());
            if d::rotation_size(width, height, angle).is_none() {
                self.message = "The rotated picture would exceed the 16 megapixel limit.".into();
                return false;
            }
            let quarter_turn = (angle.rem_euclid(180.0) - 90.0).abs() < 0.001;
            if self.selection.is_none() {
                self.doc.begin();
                if let Err(error) = self.doc.rotate_content(angle, self.colors[1]) {
                    self.doc.cancel();
                    self.message = error;
                    return false;
                }
                if quarter_turn {
                    std::mem::swap(&mut self.doc.resolution.x, &mut self.doc.resolution.y);
                }
                self.doc.commit();
                self.refresh = true;
            } else {
                let Some(index) = self.lift_selection() else {
                    return false;
                };
                if let Err(error) = self.doc.objects[index].rotate_to(angle) {
                    self.doc.cancel();
                    self.clear_selection();
                    self.message = error;
                    return false;
                }
                self.doc.commit();
                self.refresh = true;
            }
        }
        true
    }

    pub(in crate::app) fn finish_polygon(&mut self) {
        if self.polygon.is_empty() {
            return;
        }
        let points = std::mem::take(&mut self.polygon);
        self.start_shape_draft(ShapeGeometry::Polygon(points), self.polygon_color_slot);
    }
}

#[cfg(test)]
mod workflow_tests;

#[cfg(test)]
mod transform_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_art_resize_preserves_palette_alpha_and_undo() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let original = RgbaImage::from_fn(4, 4, |x, y| {
            Rgba(if x == y {
                [230, 40, 70, 255]
            } else {
                [0, 0, 0, 0]
            })
        });
        app.doc = Document::from_image(original.clone());
        app.pixel_resize = true;
        app.resize_picture(32, 32, 0.0, 0.0).unwrap();
        assert_eq!(app.doc.image.dimensions(), (32, 32));
        for (x, y, pixel) in app.doc.composite().enumerate_pixels() {
            assert_eq!(pixel, original.get_pixel(x / 8, y / 8));
        }
        app.doc.undo();
        assert_eq!(app.doc.image, original);

        let index = app
            .doc
            .add_object(Object::new(ObjectKind::Image(original.clone()), (-2, 0)));
        app.select_object(index);
        app.resize_picture(16, 16, 0.0, 0.0).unwrap();
        let object = &app.doc.objects[index];
        assert_eq!(object.pos, (-2, 0));
        assert!(object.editable());
        for (x, y, pixel) in object.render().enumerate_pixels() {
            assert_eq!(pixel, original.get_pixel(x / 4, y / 4));
        }
    }

    #[test]
    fn pencil_defaults_to_one_pixel_and_remembers_its_own_size() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.size = 20;
        app.set_tool(Tool::Pencil);
        assert_eq!(app.size, 1);
        app.size = 5;
        app.set_tool(Tool::Eraser);
        assert_eq!(app.size, 8);
        app.set_tool(Tool::Brush);
        assert_eq!(app.size, 20);
        app.set_tool(Tool::Pencil);
        assert_eq!(app.size, 5);
    }

    #[test]
    fn unavailable_image_paste_keeps_uncommitted_text_open() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let index = app.doc.add_object(Object::new(
            ObjectKind::Text {
                text: "Before".into(),
                format: Default::default(),
            },
            (10, 10),
        ));
        app.edit_text_object(index);
        app.text_edit.as_mut().unwrap().text = "Still editing".into();

        app.action(Action::Paste, &ctx);

        assert_eq!(app.text_edit.as_ref().unwrap().text, "Still editing");
        assert!(
            matches!(&app.doc.objects[index].kind, ObjectKind::Text { text, .. } if text == "Before")
        );
    }

    #[test]
    fn flips_preserve_editable_rotated_text_and_match_visible_raster() {
        let ctx = Context::default();
        for horizontal in [true, false] {
            let mut app = PaintApp::new_with_context(&ctx, false);
            let mut object = Object::new(
                ObjectKind::Text {
                    text: "Mirror".into(),
                    format: Default::default(),
                },
                (12, 20),
            );
            object.rotate_to(90.0).unwrap();
            let before = object.render();
            let index = app.doc.add_object(object);
            app.select_object(index);

            app.action(Action::Flip(horizontal), &ctx);

            let flipped = &app.doc.objects[index];
            assert!(matches!(flipped.kind, ObjectKind::Text { .. }));
            assert_eq!(flipped.pos, (12, 20));
            let expected = if horizontal {
                imageops::flip_horizontal(&before)
            } else {
                imageops::flip_vertical(&before)
            };
            assert_eq!(flipped.render(), expected);
            app.doc.undo();
            assert_eq!(app.doc.objects[index].render(), before);
        }
    }

    #[test]
    fn resize_dialog_uses_complete_off_canvas_object_and_preserves_text() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let mut object = Object::new(
            ObjectKind::Text {
                text: "Still editable".into(),
                format: Default::default(),
            },
            (-30, -5),
        );
        object.rotate_to(90.0).unwrap();
        let original = object.rendered_dimensions().unwrap();
        let index = app.doc.add_object(object);
        app.select_object(index);
        app.execute(Action::Resize, &ctx);
        assert_eq!((app.resize_w, app.resize_h), original);

        app.resize_picture(original.0 * 2, original.1 * 3, 0.0, 0.0)
            .unwrap();
        let resized = &app.doc.objects[index];
        assert_eq!(resized.pos, (-30, -5));
        assert_eq!(
            resized.rendered_dimensions(),
            Some((original.0 * 2, original.1 * 3))
        );
        assert!(matches!(&resized.kind, ObjectKind::Text { text, .. } if text == "Still editable"));
        app.doc.undo();
        assert_eq!(app.doc.objects[index].rendered_dimensions(), Some(original));
    }

    #[test]
    fn inverting_a_keyed_picture_preserves_transparency_and_undo() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let mut image = RgbaImage::from_pixel(3, 2, Rgba(WHITE));
        image.put_pixel(1, 0, Rgba(BLACK));
        let mut object = Object::new(ObjectKind::Image(image), (10, 10));
        object.color_key = Some(WHITE);
        app.doc.begin();
        let index = app.doc.add_object(object);
        app.doc.commit();
        app.select_object(index);
        app.transparent = true;

        app.action(Action::Invert, &ctx);
        let selected = &app.doc.objects[index];
        assert_eq!(selected.color_key, Some(BLACK));
        assert_eq!(selected.render().get_pixel(0, 0)[3], 0);
        assert_eq!(*selected.render().get_pixel(1, 0), Rgba(WHITE));
        app.sync_image_transparency();
        assert_eq!(app.doc.objects[index].color_key, Some(BLACK));

        app.action(Action::Undo, &ctx);
        assert_eq!(app.doc.objects[index].color_key, Some(WHITE));
        assert_eq!(
            *app.doc.objects[index].render().get_pixel(1, 0),
            Rgba(BLACK)
        );
    }

    #[test]
    fn inserting_a_picture_preserves_uncommitted_text_edits() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let index = app.doc.add_object(Object::new(
            ObjectKind::Text {
                text: "Before".into(),
                format: Default::default(),
            },
            (10, 10),
        ));
        app.edit_text_object(index);
        app.text_edit.as_mut().unwrap().text = "After".into();

        app.insert_image(RgbaImage::from_pixel(5, 5, Rgba(BLACK)));

        assert!(app.text_edit.is_none());
        assert!(matches!(
            &app.doc.objects[index].kind,
            ObjectKind::Text { text, .. } if text == "After"
        ));
        assert!(matches!(
            app.doc.objects[app.object.unwrap()].kind,
            ObjectKind::Image(_)
        ));
    }
}
