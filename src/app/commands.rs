use super::*;

impl PaintApp {
    pub(in crate::app) fn set_tool(&mut self, tool: Tool) {
        if self.dialog.is_some() || self.pending.is_some() {
            return;
        }
        self.commit_text();
        self.finish_polygon();
        self.commit_shape();
        if self.curve.take().is_some() {
            self.doc.commit();
        }
        if tool == Tool::Picker {
            self.previous_tool = self.tool;
        }
        self.tool = tool;
        self.clear_selection();
        self.message = match tool {
            Tool::Select => {
                "Drag to select. Drag a selection to move it; double-click text to edit.".into()
            }
            Tool::Text => {
                "Click on the canvas to add text. Existing text stays editable with Select.".into()
            }
            Tool::Curve => "Drag a line, then click to set its bend.".into(),
            _ => format!(
                "{} — left button: Color 1; right button: Color 2. Hold Shift to constrain.",
                tool.name()
            ),
        };
    }

    pub(in crate::app) fn action(&mut self, action: Action, ctx: &Context) {
        if self.dialog.is_some() || self.pending.is_some() {
            return;
        }
        if self.shape_draft.is_some() {
            match action {
                Action::Copy => {
                    self.copy();
                    return;
                }
                Action::Cut => {
                    self.copy();
                    self.delete_selection();
                    return;
                }
                Action::Clear => {
                    self.delete_selection();
                    return;
                }
                _ => {}
            }
        }
        self.commit_text();
        self.finish_polygon();
        if let Some(bounds) = self.commit_shape() {
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
        if self.curve.take().is_some() {
            self.doc.commit();
        }
        if matches!(action, Action::New | Action::Open | Action::Close) && self.doc.dirty() {
            self.pending = Some(action);
            return;
        }
        self.execute(action, ctx);
    }

    pub(in crate::app) fn execute(&mut self, action: Action, ctx: &Context) {
        match action {
            Action::New => {
                self.doc = Document::new(900, 600);
                self.file = None;
                self.clear_selection();
                self.refresh = true;
            }
            Action::Open => {
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
                self.allow_close = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            Action::Undo => {
                self.doc.undo();
                self.clear_selection();
                self.refresh = true;
            }
            Action::Redo => {
                self.doc.redo();
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
            Action::Copy => self.copy(),
            Action::Cut => {
                self.copy();
                self.delete_selection();
            }
            Action::Paste => self.paste_clipboard(),
            Action::PasteFrom => {
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
                    self.doc.flatten();
                    self.doc.image = r.extract(&self.doc.image);
                    self.doc.commit();
                    self.clear_selection();
                    self.refresh = true;
                }
            }
            Action::Resize => {
                let (w, h) = self
                    .selected_region()
                    .map(|r| (r.w, r.h))
                    .unwrap_or(self.doc.image.dimensions());
                self.resize_w = w;
                self.resize_h = h;
                self.percent = false;
                self.skew_x = 0.;
                self.skew_y = 0.;
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
            Action::Flip(horizontal) => self.transform(|img| {
                if horizontal {
                    imageops::flip_horizontal(img)
                } else {
                    imageops::flip_vertical(img)
                }
            }),
            Action::ClearPicture => {
                self.clear_selection();
                self.execute(Action::Clear, ctx);
            }
            Action::Clear => {
                if self.selection.is_some() || self.object.is_some() {
                    self.delete_selection();
                } else {
                    let bg = self.colors[1];
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
            Action::Invert => self.transform(|img| {
                let mut img = img.clone();
                imageops::invert(&mut img);
                img
            }),
            Action::Print => {
                self.dialog = Some(Dialog::Print);
            }
        }
    }

    pub(in crate::app) fn transform(&mut self, f: impl FnOnce(&RgbaImage) -> RgbaImage) {
        self.doc.begin();
        if let Some(i) = self.object {
            let rendered = self.doc.objects[i].render_unkeyed();
            let img = f(&rendered);
            self.doc.objects[i].kind = ObjectKind::Image(img);
            self.doc.objects[i].angle = 0.;
            self.doc.objects[i].scale = 1.;
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
                color_key: self.transparent.then_some(self.colors[1]),
            });
            self.select_object(i);
        } else {
            self.doc.flatten();
            self.doc.image = f(&self.doc.image);
        }
        self.doc.commit();
        self.refresh = true;
    }

    pub(in crate::app) fn rotate_picture(&mut self, angle: f32, absolute: bool) -> bool {
        if let Some(index) = self.object {
            let object = &self.doc.objects[index];
            let requested = if absolute {
                angle
            } else {
                object.angle + angle
            };
            let mut unrotated = object.clone();
            unrotated.angle = 0.0;
            let image = unrotated.render();
            if d::rotation_size(image.width(), image.height(), requested).is_none() {
                self.message = "The rotated picture would exceed the 16 megapixel limit.".into();
                return false;
            }
            self.doc.begin();
            self.doc.objects[index].angle = requested.rem_euclid(360.0);
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
            if self.selection.is_none() && quarter_turn {
                self.doc.begin();
                self.doc.flatten();
                self.doc.image = d::rotate(&self.doc.image, angle, WHITE);
                std::mem::swap(&mut self.doc.resolution.x, &mut self.doc.resolution.y);
                self.doc.commit();
                self.refresh = true;
            } else {
                self.transform(|image| d::rotate(image, angle, WHITE));
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
