use super::*;

impl PaintApp {
    /// The source image and current adjustment settings, without committing an
    /// active shape or text draft merely because a dialog was opened.
    pub(in crate::app) fn image_edit_target(&self) -> Option<(d::ImageEdits, (u32, u32))> {
        if self.text_edit.is_some() {
            return None;
        }
        if let Some(object) = self.object.and_then(|index| self.doc.objects.get(index)) {
            return matches!(object.kind, ObjectKind::Image(_))
                .then(|| (object.image_edits, object.source_dimensions()));
        }
        let dimensions = self
            .selected_region()
            .map(|region| (region.w, region.h))
            .unwrap_or(self.doc.image.dimensions());
        Some((d::ImageEdits::default(), dimensions))
    }

    pub(in crate::app) fn image_edit_object(&self) -> Result<Object, String> {
        if self.text_edit.is_some() {
            return Err("Finish editing the text before adjusting a picture.".into());
        }
        if let Some(object) = self.object.and_then(|index| self.doc.objects.get(index)) {
            return if matches!(object.kind, ObjectKind::Image(_)) {
                Ok(object.clone())
            } else {
                Err("Select a picture or an area of the canvas to edit its image.".into())
            };
        }
        let position = self
            .selected_region()
            .map(|region| (region.x as i32, region.y as i32))
            .unwrap_or((0, 0));
        let source = self
            .selected_image()
            .unwrap_or_else(|| self.doc.composite());
        Ok(Object::new(ObjectKind::Image(source), position))
    }

    pub(in crate::app) fn apply_image_edits(&mut self, edits: d::ImageEdits) -> Result<(), String> {
        let mut object = self.image_edit_object()?;
        object.set_image_edits(edits)?;
        if let Some(index) = self.object {
            if self.doc.objects[index] == object {
                return Ok(());
            }
            self.doc.begin();
            self.doc.objects[index] = object;
        } else {
            if let Some(bounds) = self.finish_editing() {
                self.selection = Some(bounds);
            }
            self.doc.begin();
            if let Some(region) = self.selection {
                let selected = self
                    .selected_image()
                    .ok_or("The selection is no longer available.")?;
                self.doc.flatten();
                self.clear_selected_pixels(region, &selected);
            } else {
                self.doc.objects.clear();
                self.doc.image = RgbaImage::new(self.doc.image.width(), self.doc.image.height());
            }
            let index = self.doc.add_object(object);
            self.select_object(index);
        }
        self.doc.commit();
        self.tool = Tool::Select;
        self.select_image_options();
        self.refresh = true;
        Ok(())
    }

    pub(in crate::app) fn reset_image_edits(&mut self) -> Result<(), String> {
        let Some(index) = self.object else {
            return Ok(());
        };
        let mut object = self.image_edit_object()?;
        object.image_edits = Default::default();
        object.source_clip = None;
        object.angle = 0.0;
        object.scale = 1.0;
        object.transform = Default::default();
        object.color_key = None;
        if self.doc.objects[index] == object {
            return Ok(());
        }
        self.doc.begin();
        self.doc.objects[index] = object;
        self.doc.commit();
        self.select_image_options();
        self.refresh = true;
        Ok(())
    }

    pub(in crate::app) fn selection_contains(&self, point: Point) -> bool {
        let Some(region) = self.selection.filter(|region| region.contains(point)) else {
            return false;
        };
        if let Some(mask) = &self.mask {
            mask.get_pixel_checked(point.0 as u32 - region.x, point.1 as u32 - region.y)
                .is_some_and(|pixel| pixel[0] > 0)
        } else {
            self.free_points.is_empty() || inside_polygon(point, &self.free_points)
        }
    }

    pub(in crate::app) fn clear_selected_pixels(&mut self, region: Region, selected: &RgbaImage) {
        clear_covered_pixels(&mut self.doc.image, region, selected, self.colors[1]);
    }

    pub(in crate::app) fn clear_selection(&mut self) {
        self.selection = None;
        self.object = None;
        self.free_points.clear();
        self.mask = None;
    }

    pub(in crate::app) fn select_object(&mut self, index: usize) {
        self.clear_selection();
        self.object = Some(index);
    }

    pub(in crate::app) fn selected_region(&self) -> Option<Region> {
        if let Some(shape) = &self.shape_draft {
            return shape.bounds(&self.doc.image);
        }
        if let Some(obj) = self.object.and_then(|i| self.doc.objects.get(i)) {
            let (width, height) = obj.rendered_dimensions()?;
            let left = i64::from(obj.pos.0).max(0);
            let top = i64::from(obj.pos.1).max(0);
            let right =
                (i64::from(obj.pos.0) + i64::from(width)).min(i64::from(self.doc.image.width()));
            let bottom =
                (i64::from(obj.pos.1) + i64::from(height)).min(i64::from(self.doc.image.height()));
            (right > left && bottom > top).then(|| Region {
                x: left as u32,
                y: top as u32,
                w: (right - left) as u32,
                h: (bottom - top) as u32,
            })
        } else {
            self.selection
        }
    }

    pub(in crate::app) fn selected_image(&self) -> Option<RgbaImage> {
        if let Some(shape) = &self.shape_draft {
            return shape
                .bounds(&self.doc.image)
                .map(|bounds| bounds.extract(&self.doc.composite()));
        }
        if let Some(obj) = self.object.and_then(|i| self.doc.objects.get(i)) {
            return Some(obj.render());
        }
        self.selection.map(|r| {
            let mut img = r.extract(&self.doc.composite());
            if let Some(mask) = &self.mask {
                for (x, y, p) in img.enumerate_pixels_mut() {
                    if !mask
                        .get_pixel_checked(x, y)
                        .is_some_and(|pixel| pixel[0] > 0)
                    {
                        p[3] = 0;
                    }
                }
            } else if !self.free_points.is_empty() {
                for (x, y, p) in img.enumerate_pixels_mut() {
                    if !inside_polygon(
                        (x as i32 + r.x as i32, y as i32 + r.y as i32),
                        &self.free_points,
                    ) {
                        *p = Rgba([0, 0, 0, 0]);
                    }
                }
            }
            img
        })
    }

    pub(in crate::app) fn copy(&mut self) -> bool {
        // Preserve keyed source colors, so Paste can still switch between
        // Opaque and Transparent selection without losing background pixels.
        let image = self
            .object
            .and_then(|index| self.doc.objects.get(index))
            .map(Object::render_unkeyed)
            .or_else(|| self.selected_image());
        if let Some(img) = image {
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(cb) = &mut self.clipboard {
                if let Err(error) = cb.set_image(arboard::ImageData {
                    width: img.width() as usize,
                    height: img.height() as usize,
                    bytes: Cow::Borrowed(img.as_raw()),
                }) {
                    self.message = format!("Could not copy the selection: {error}");
                    return false;
                }
            }
            #[cfg(target_arch = "wasm32")]
            self.copy_browser_image(&img);
            self.copied = Some(img);
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.message = if self.clipboard.is_some() {
                    "Selection copied"
                } else {
                    "Selection copied within Paint 10; the system clipboard is unavailable"
                }
                .into();
            }
            #[cfg(target_arch = "wasm32")]
            {
                self.message = "Selection copied within Paint 10".into();
            }
            return true;
        }
        false
    }

    pub(in crate::app) fn cut(&mut self) {
        if self.copy() {
            self.delete_selection();
        }
    }

    pub(in crate::app) fn delete_selection(&mut self) {
        if self.shape_draft.take().is_some() {
            self.doc.cancel();
            self.refresh = true;
            return;
        }
        self.doc.begin();
        if let Some(i) = self.object.take() {
            self.doc.objects.remove(i);
        } else if let Some(r) = self.selection {
            self.doc.flatten();
            if let Some(mask) = &self.mask {
                for (x, y, p) in mask.enumerate_pixels() {
                    if p[0] > 0 {
                        self.doc
                            .image
                            .put_pixel(r.x + x, r.y + y, Rgba(self.colors[1]));
                    }
                }
            } else if self.free_points.is_empty() {
                r.clear(&mut self.doc.image, self.colors[1]);
            } else {
                for y in r.y..r.y + r.h {
                    for x in r.x..r.x + r.w {
                        if inside_polygon((x as i32, y as i32), &self.free_points) {
                            self.doc.image.put_pixel(x, y, Rgba(self.colors[1]));
                        }
                    }
                }
            }
        }
        self.doc.commit();
        self.clear_selection();
        self.refresh = true;
    }

    pub(in crate::app) fn paste_clipboard(&mut self) {
        #[cfg(target_arch = "wasm32")]
        self.paste_browser_clipboard();
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Once a system clipboard exists it is authoritative. Its contents may
            // have changed to text (or been cleared) since our last image copy.
            // The local fallback is only for systems without clipboard access.
            let result = if let Some(clipboard) = &mut self.clipboard {
                clipboard
                    .get_image()
                    .map_err(|error| match error {
                        arboard::Error::ContentNotAvailable => {
                            "No picture on the clipboard. Use Paste from to insert a file.".into()
                        }
                        error => format!("Could not read the clipboard: {error}"),
                    })
                    .and_then(|image| {
                        let width = u32::try_from(image.width).unwrap_or(u32::MAX);
                        let height = u32::try_from(image.height).unwrap_or(u32::MAX);
                        if !d::valid_size(width, height) {
                            return Err("Clipboard picture exceeds the image size limit.".into());
                        }
                        RgbaImage::from_raw(width, height, image.bytes.into_owned())
                            .ok_or_else(|| "The clipboard picture has invalid pixel data.".into())
                    })
            } else {
                self.copied
                    .clone()
                    .ok_or_else(|| "No picture has been copied in Paint 10.".into())
            };
            match result {
                Ok(image) => self.insert_image(image),
                Err(error) => self.message = error,
            }
        }
    }

    pub(in crate::app) fn insert_image(&mut self, img: RgbaImage) {
        if !d::valid_size(img.width(), img.height()) {
            self.message = "Picture is too large (16 megapixel limit).".into();
            return;
        }
        let expanded_width = img.width().max(self.doc.image.width());
        let expanded_height = img.height().max(self.doc.image.height());
        if !d::valid_size(expanded_width, expanded_height) {
            self.message = "The expanded canvas would exceed the 16 megapixel limit.".into();
            return;
        }
        self.finish_editing();
        self.doc.begin();
        if img.width() > self.doc.image.width() || img.height() > self.doc.image.height() {
            if let Err(error) =
                self.doc
                    .resize_canvas(expanded_width, expanded_height, self.colors[1])
            {
                self.doc.cancel();
                self.message = error;
                return;
            }
        }
        let i = self.doc.add_object(Object {
            kind: ObjectKind::Image(img),
            pos: (0, 0),
            angle: 0.,
            scale: 1.,
            color_key: self.transparent.then_some(self.colors[1]),
            transform: Default::default(),
            image_edits: d::ImageEdits {
                matte: Some(self.colors[1]),
                sampling: if self.pixel_resize {
                    d::ImageSampling::Nearest
                } else {
                    d::ImageSampling::Smooth
                },
                ..Default::default()
            },
            source_clip: None,
        });
        self.doc.commit();
        self.tool = Tool::Select;
        self.select_object(i);
        self.refresh = true;
        self.message =
            "Drag the picture to move it. Rotate and Resize also work on the selected picture."
                .into();
    }

    pub(in crate::app) fn invert_selection(&mut self) {
        if let Some(bounds) = self.finish_editing() {
            self.selection = Some(bounds);
        }
        if let Some(selected) = self.selected_image() {
            let origin = self
                .object
                .and_then(|index| self.doc.objects.get(index))
                .map(|object| object.pos)
                .or_else(|| {
                    self.selected_region()
                        .map(|region| (region.x as i32, region.y as i32))
                })
                .unwrap_or((0, 0));
            let mask = invert_coverage(self.doc.image.dimensions(), origin, &selected);
            self.object = None;
            self.selection = Some(Region {
                x: 0,
                y: 0,
                w: mask.width(),
                h: mask.height(),
            });
            self.mask = Some(mask);
            self.free_points.clear();
        }
    }

    pub(in crate::app) fn lift_selection(&mut self) -> Option<usize> {
        if self.object.is_none() && self.selection.is_none() {
            return None;
        }
        self.doc.begin();
        if let Some(i) = self.object {
            return Some(i);
        }
        let r = self.selection?;
        let img = self.selected_image()?;
        self.doc.flatten();
        self.clear_selected_pixels(r, &img);
        let i = self.doc.add_object(Object {
            kind: ObjectKind::Image(img),
            pos: (r.x as i32, r.y as i32),
            angle: 0.,
            scale: 1.,
            color_key: self.transparent.then_some(self.colors[1]),
            transform: Default::default(),
            image_edits: Default::default(),
            source_clip: None,
        });
        self.select_object(i);
        Some(i)
    }

    pub(in crate::app) fn sync_image_transparency(&mut self) {
        let Some(index) = self.object else {
            return;
        };
        let Some(object) = self.doc.objects.get(index) else {
            return;
        };
        if !matches!(object.kind, ObjectKind::Image(_)) {
            return;
        }
        let key = self.transparent.then_some(self.colors[1]);
        if object.color_key == key {
            return;
        }
        let separate_edit = self.gesture.is_none();
        if separate_edit {
            self.doc.begin();
        }
        self.doc.objects[index].color_key = key;
        if separate_edit {
            self.doc.commit();
        }
        self.refresh = true;
    }

    pub(in crate::app) fn select_image_options(&mut self) {
        if let Some(object) = self.object.and_then(|index| self.doc.objects.get(index)) {
            if matches!(object.kind, ObjectKind::Image(_)) {
                self.transparent = object.color_key.is_some();
                if let Some(key) = object.color_key {
                    self.colors[1] = key;
                }
            }
        }
    }
}

/// Invert the current rendered coverage, including moved free-form selections.
fn invert_coverage(
    dimensions: (u32, u32),
    origin: Point,
    selected: &RgbaImage,
) -> image::GrayImage {
    let mut mask = image::GrayImage::from_pixel(dimensions.0, dimensions.1, image::Luma([255]));
    for (x, y, pixel) in selected.enumerate_pixels() {
        let destination_x = origin.0 as i64 + x as i64;
        let destination_y = origin.1 as i64 + y as i64;
        if pixel[3] > 0
            && destination_x >= 0
            && destination_y >= 0
            && destination_x < dimensions.0 as i64
            && destination_y < dimensions.1 as i64
        {
            mask.put_pixel(destination_x as u32, destination_y as u32, image::Luma([0]));
        }
    }
    mask
}

/// Selection images encode free-form and inverted coverage in alpha.
fn clear_covered_pixels(
    image: &mut RgbaImage,
    region: Region,
    selected: &RgbaImage,
    background: Color,
) {
    for (x, y, pixel) in selected.enumerate_pixels() {
        if pixel[3] > 0 && region.x + x < image.width() && region.y + y < image.height() {
            image.put_pixel(region.x + x, region.y + y, Rgba(background));
        }
    }
}

fn inside_polygon(p: Point, points: &[Point]) -> bool {
    let mut inside = false;
    for (a, b) in points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
    {
        if (a.1 > p.1) != (b.1 > p.1)
            && ((p.0 as f64)
                < (b.0 - a.0) as f64 * (p.1 - a.1) as f64 / (b.1 - a.1) as f64 + a.0 as f64)
        {
            inside = !inside;
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_dialog_preview_is_read_only_and_reset_recovers_source_pixels() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let source = RgbaImage::from_fn(19, 13, |x, y| Rgba([x as u8 * 11, y as u8 * 17, 90, 128]));
        app.insert_image(source.clone());
        let index = app.object.unwrap();
        let before = app.doc.objects[index].clone();
        let edits = d::ImageEdits {
            crop: Some(Region {
                x: 2,
                y: 3,
                w: 8,
                h: 7,
            }),
            brightness: 25.0,
            saturation: -35.0,
            matte: None,
            ..Default::default()
        };
        assert_eq!(
            app.image_edit_object()
                .unwrap()
                .preview_image_edits(edits, 64)
                .unwrap()
                .dimensions(),
            (8, 7)
        );
        assert!(app.doc.objects[index] == before);
        app.apply_image_edits(edits).unwrap();
        app.resize_picture(2, 2, 0.0, 0.0).unwrap();
        app.rotate_picture(33.0, true);
        let before_reset = app.doc.objects[index].clone();
        app.reset_image_edits().unwrap();
        assert_eq!(app.doc.objects[index].render(), source);
        assert!(
            matches!(&app.doc.objects[index].kind, ObjectKind::Image(original) if *original == source)
        );
        app.doc.undo();
        assert!(app.doc.objects[index] == before_reset);
    }

    #[test]
    fn canvas_adjustments_are_undoable_and_do_not_require_a_prior_selection() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let original = RgbaImage::from_pixel(8, 6, Rgba([35, 80, 130, 255]));
        app.doc = Document::from_image(original.clone());
        app.apply_image_edits(d::ImageEdits {
            invert: true,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            app.doc.composite().get_pixel(3, 2),
            &Rgba([220, 175, 125, 255])
        );
        assert!(app.object.is_some());
        app.doc.undo();
        assert_eq!(app.doc.composite(), original);
    }

    #[test]
    fn visible_object_selection_is_an_intersection_without_phantom_edge_pixels() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let index = app.doc.add_object(Object::new(
            ObjectKind::Image(RgbaImage::from_pixel(20, 10, Rgba(BLACK))),
            (-5, -3),
        ));
        app.select_object(index);
        let visible = app.selected_region().unwrap();
        assert_eq!((visible.x, visible.y, visible.w, visible.h), (0, 0, 15, 7));
        for position in [(-20, 0), (0, -10), (900, 0), (0, 600), (i32::MAX, i32::MAX)] {
            app.doc.objects[index].pos = position;
            assert!(app.selected_region().is_none(), "{position:?}");
        }
    }

    #[test]
    fn copied_transparent_selection_can_be_pasted_opaque_again() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let mut image = RgbaImage::from_pixel(3, 2, Rgba(WHITE));
        image.put_pixel(1, 0, Rgba(BLACK));
        app.transparent = true;
        app.insert_image(image);
        let original = app.object.unwrap();
        assert_eq!(app.doc.objects[original].render().get_pixel(0, 0)[3], 0);

        assert!(app.copy());
        app.paste_clipboard();
        let pasted = app.object.unwrap();
        assert_ne!(original, pasted);
        assert_eq!(app.doc.objects[pasted].render().get_pixel(0, 0)[3], 0);
        app.transparent = false;
        app.sync_image_transparency();
        assert_eq!(
            *app.doc.objects[pasted].render().get_pixel(0, 0),
            Rgba(WHITE)
        );
        assert_eq!(
            *app.doc.objects[pasted].render().get_pixel(1, 0),
            Rgba(BLACK)
        );
    }

    #[test]
    fn pasted_alpha_uses_color_two_and_remains_reversible_by_selection_mode() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let background = [20, 50, 200, 255];
        app.colors[1] = background;
        app.insert_image(RgbaImage::new(2, 2));
        let index = app.object.unwrap();
        assert_eq!(
            *app.doc.objects[index].render().get_pixel(0, 0),
            Rgba(background)
        );
        app.transparent = true;
        app.sync_image_transparency();
        assert_eq!(app.doc.objects[index].render().get_pixel(0, 0)[3], 0);
        app.transparent = false;
        app.sync_image_transparency();
        assert_eq!(
            *app.doc.objects[index].render().get_pixel(0, 0),
            Rgba(background)
        );
    }

    #[test]
    fn clearing_a_mask_preserves_unselected_pixels_in_its_bounds() {
        let red = Rgba([255, 0, 0, 255]);
        let mut image = RgbaImage::from_pixel(6, 5, red);
        let mut selection = RgbaImage::new(3, 2);
        selection.put_pixel(0, 0, Rgba(BLACK));
        selection.put_pixel(2, 1, Rgba(BLACK));
        clear_covered_pixels(
            &mut image,
            Region {
                x: 2,
                y: 1,
                w: 3,
                h: 2,
            },
            &selection,
            WHITE,
        );
        for (x, y, pixel) in image.enumerate_pixels() {
            assert_eq!(
                *pixel,
                if (x, y) == (2, 1) || (x, y) == (4, 2) {
                    Rgba(WHITE)
                } else {
                    red
                }
            );
        }
    }

    #[test]
    fn inversion_uses_resized_coverage_and_clips_moved_free_form_pixels() {
        let expanded = RgbaImage::from_pixel(1000, 600, Rgba(BLACK));
        let inverse = invert_coverage((1000, 600), (0, 0), &expanded);
        assert!(inverse.pixels().all(|pixel| pixel[0] == 0));

        let mut free_form = RgbaImage::new(3, 2);
        free_form.put_pixel(0, 0, Rgba(BLACK));
        free_form.put_pixel(2, 1, Rgba(BLACK));
        let inverse = invert_coverage((6, 5), (-1, 2), &free_form);
        for (x, y, pixel) in inverse.enumerate_pixels() {
            assert_eq!(pixel[0], if (x, y) == (1, 3) { 0 } else { 255 });
        }
    }
}
