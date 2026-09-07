use super::*;

impl PaintApp {
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
            return Some(shape.bounds(&self.doc.image));
        }
        if let Some(obj) = self.object.and_then(|i| self.doc.objects.get(i)) {
            let img = obj.render();
            Some(Region::between(
                obj.pos,
                (
                    obj.pos.0 + img.width() as i32 - 1,
                    obj.pos.1 + img.height() as i32 - 1,
                ),
                &self.doc.image,
            ))
        } else {
            self.selection
        }
    }

    pub(in crate::app) fn selected_image(&self) -> Option<RgbaImage> {
        if let Some(shape) = &self.shape_draft {
            return Some(shape.bounds(&self.doc.image).extract(&self.doc.composite()));
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

    pub(in crate::app) fn copy(&mut self) {
        if let Some(img) = self.selected_image() {
            if let Some(cb) = &mut self.clipboard {
                let _ = cb.set_image(arboard::ImageData {
                    width: img.width() as usize,
                    height: img.height() as usize,
                    bytes: Cow::Borrowed(img.as_raw()),
                });
            }
            self.copied = Some(img);
            self.message = "Selection copied".into();
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
        let img = self
            .clipboard
            .as_mut()
            .and_then(|c| c.get_image().ok())
            .and_then(|img| {
                RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.into_owned())
            })
            .or_else(|| self.copied.clone());
        if let Some(img) = img {
            self.insert_image(img);
        } else {
            self.message = "No picture on the clipboard. Use Paste from to insert a file.".into();
        }
    }

    pub(in crate::app) fn insert_image(&mut self, img: RgbaImage) {
        self.commit_shape();
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
        self.doc.begin();
        if img.width() > self.doc.image.width() || img.height() > self.doc.image.height() {
            self.doc.image = d::resize_canvas(
                &self.doc.image,
                expanded_width,
                expanded_height,
                if self.doc.objects.is_empty() {
                    WHITE
                } else {
                    [0, 0, 0, 0]
                },
            );
        }
        let i = self.doc.add_object(Object {
            kind: ObjectKind::Image(img),
            pos: (0, 0),
            angle: 0.,
            scale: 1.,
            color_key: self.transparent.then_some(self.colors[1]),
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
