use super::*;

struct CanvasPointer {
    raw: Point,
    clamped: Point,
    shift: bool,
    released: bool,
}

impl PaintApp {
    pub(in crate::app) fn canvas_input(
        &mut self,
        ui: &Ui,
        response: &Response,
        rect: Rect,
        ctx: &Context,
    ) {
        let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) else {
            return;
        };
        let raw = self.point(pos, rect);
        let p = (
            raw.0.clamp(0, self.doc.image.width() as i32 - 1),
            raw.1.clamp(0, self.doc.image.height() as i32 - 1),
        );
        self.cursor = if response.hovered() { Some(p) } else { None };
        let press_pos = pointer_press_in(ui, ctx, rect, self.tool == Tool::Select);
        let pressed = press_pos.is_some();
        let released = ctx.input(|i| i.pointer.any_released());
        let shift = ctx.input(|i| i.modifiers.shift);
        if !self.polygon.is_empty() {
            self.doc.restore_preview();
            let mut pts = self.polygon.clone();
            pts.push(p);
            d::styled_polygon(
                &mut self.doc.image,
                &pts,
                Some((self.polygon_color, self.outline)),
                Some((self.colors[1 - self.polygon_color_slot], self.fill)),
                self.size,
            );
            self.refresh = true;
            if pressed && response.hovered() {
                self.polygon.push(p);
            }
            if response.double_clicked() || ctx.input(|i| i.key_pressed(Key::Enter)) {
                self.finish_polygon();
            }
            return;
        }
        if let Some(mut curve) = self.curve {
            let color = self.colors[curve.color_slot];
            self.doc.restore_preview();
            if let Some(c1) = curve.first {
                d::styled_cubic(
                    &mut self.doc.image,
                    curve.start,
                    curve.end,
                    [c1, p],
                    self.size,
                    color,
                    self.outline,
                );
            } else {
                d::styled_curve(
                    &mut self.doc.image,
                    curve.start,
                    curve.end,
                    p,
                    self.size,
                    color,
                    self.outline,
                );
            }
            self.refresh = true;
            if pressed {
                curve.dragging = true;
            }
            if released && curve.dragging {
                if let Some(first) = curve.first {
                    self.curve = None;
                    self.start_shape_draft(
                        ShapeGeometry::Curve {
                            start: curve.start,
                            end: curve.end,
                            controls: [first, p],
                        },
                        curve.color_slot,
                    );
                    return;
                } else {
                    curve.first = Some(p);
                    curve.dragging = false;
                    self.message = "Click and drag to set the second bend.".into();
                }
            }
            self.curve = Some(curve);
            return;
        }
        if response.double_clicked() && self.tool == Tool::Select {
            let hit = self
                .doc
                .objects
                .iter()
                .enumerate()
                .rev()
                .find_map(|(i, o)| {
                    if !matches!(o.kind, ObjectKind::Text { .. }) {
                        return None;
                    }
                    let img = o.render();
                    (p.0 >= o.pos.0
                        && p.1 >= o.pos.1
                        && p.0 < o.pos.0 + img.width() as i32
                        && p.1 < o.pos.1 + img.height() as i32)
                        .then_some(i)
                });
            if let Some(i) = hit {
                self.doc.cancel();
                self.gesture = None;
                self.select_object(i);
                self.edit_text_object(i);
                return;
            }
        }
        if let Some(press_pos) = press_pos.filter(|p| {
            rect.contains(*p)
                && ui.clip_rect().contains(*p)
                && !matches!(
                    self.gesture,
                    Some(
                        Gesture::ResizeObject { .. }
                            | Gesture::CanvasSize { .. }
                            | Gesture::ResizeShape { .. }
                    )
                )
        }) {
            self.begin_canvas_gesture(press_pos, rect, ctx);
        }
        self.continue_canvas_gesture(
            ui,
            rect,
            ctx,
            CanvasPointer {
                raw,
                clamped: p,
                shift,
                released,
            },
        );
    }
    fn begin_canvas_gesture(&mut self, press_pos: Pos2, rect: Rect, ctx: &Context) {
        let p = self.point(press_pos, rect);
        if self.begin_shape_gesture(p) {
            return;
        }
        let right = ctx.input(|i| {
            i.events.iter().any(|e| {
                matches!(
                    e,
                    Event::PointerButton {
                        button: PointerButton::Secondary,
                        pressed: true,
                        ..
                    }
                )
            })
        });
        let color = self.colors[usize::from(right)];
        match self.tool {
            Tool::Fill => {
                let before = self.doc.composite();
                let mut after = before.clone();
                d::flood_fill(&mut after, p, color);
                self.doc.begin();
                for (x, y, px) in after.enumerate_pixels() {
                    if px != before.get_pixel(x, y) {
                        self.doc.image.put_pixel(x, y, *px);
                    }
                }
                self.doc.commit();
                self.refresh = true;
            }
            Tool::Picker => {
                self.colors[usize::from(right)] = self.rendered.get_pixel(p.0 as u32, p.1 as u32).0;
                self.set_tool(self.previous_tool);
            }
            Tool::Magnifier => {
                self.zoom = (self.zoom * if right { 0.5 } else { 2. }).clamp(0.125, 8.);
            }
            Tool::Text => {
                self.gesture = Some(Gesture::TextBox { start: p });
            }
            Tool::Select => {
                let hit = self
                    .doc
                    .objects
                    .iter()
                    .enumerate()
                    .rev()
                    .find_map(|(i, o)| {
                        if !o.editable() {
                            return None;
                        }
                        let img = o.render();
                        let x = p.0 - o.pos.0;
                        let y = p.1 - o.pos.1;
                        if x >= 0 && y >= 0 && x < img.width() as i32 && y < img.height() as i32 {
                            Some(i)
                        } else {
                            None
                        }
                    });
                let duplicate = ctx.input(|i| i.modifiers.ctrl);
                if let Some(mut i) =
                    hit.filter(|index| self.object == Some(*index) && !self.selection_contains(p))
                {
                    self.doc.begin();
                    if duplicate {
                        let obj = self.doc.objects[i].clone();
                        i = self.doc.add_object(obj);
                    }
                    self.select_object(i);
                    self.gesture = Some(Gesture::Move {
                        start: p,
                        origin: self.doc.objects[i].pos,
                        index: i,
                        last_stamp: p,
                    });
                } else if self.selection_contains(p) {
                    let r = self.selection.unwrap();
                    let img = self.selected_image().unwrap();
                    self.doc.begin();
                    self.doc.flatten();
                    if !duplicate {
                        self.clear_selected_pixels(r, &img);
                    }
                    let i = self.doc.add_object(Object {
                        kind: ObjectKind::Image(img),
                        pos: (r.x as i32, r.y as i32),
                        angle: 0.,
                        scale: 1.,
                        color_key: self.transparent.then_some(self.colors[1]),
                    });
                    self.select_object(i);
                    self.gesture = Some(Gesture::Move {
                        start: p,
                        origin: (r.x as i32, r.y as i32),
                        index: i,
                        last_stamp: p,
                    });
                } else {
                    self.clear_selection();
                    self.gesture = Some(Gesture::Select {
                        start: p,
                        points: vec![p],
                        click_candidate: hit,
                    });
                }
            }
            _ => {
                self.doc.begin();
                self.gesture = Some(Gesture::Paint {
                    start: p,
                    last: p,
                    color,
                    erase_target: right.then_some(self.colors[0]),
                });
            }
        }
    }

    fn continue_canvas_gesture(
        &mut self,
        ui: &Ui,
        rect: Rect,
        ctx: &Context,
        pointer: CanvasPointer,
    ) {
        let CanvasPointer {
            raw,
            clamped: p,
            shift,
            released,
        } = pointer;
        if let Some(mut gesture) = self.gesture.take() {
            match &mut gesture {
                Gesture::MoveShape { start, original } => {
                    self.shape_draft =
                        Some(original.translated((raw.0 - start.0, raw.1 - start.1)));
                    self.redraw_shape();
                }
                Gesture::ResizeShape {
                    bounds,
                    original,
                    handle,
                } => {
                    self.resize_shape(original, *bounds, *handle, raw, shift);
                }
                Gesture::ResizeObject {
                    index,
                    original,
                    source,
                    base,
                    handle,
                } => {
                    let (mut x, mut y, mut right, mut bottom) = (
                        original.x as i32,
                        original.y as i32,
                        (original.x + original.w) as i32,
                        (original.y + original.h) as i32,
                    );
                    if matches!(*handle, 0 | 2 | 6) {
                        x = raw.0.min(right - 1);
                    }
                    if matches!(*handle, 1 | 3 | 7) {
                        right = raw.0.max(x + 1);
                    }
                    if matches!(*handle, 0 | 1 | 4) {
                        y = raw.1.min(bottom - 1);
                    }
                    if matches!(*handle, 2 | 3 | 5) {
                        bottom = raw.1.max(y + 1);
                    }
                    let mut w = (right - x) as u32;
                    let mut h = (bottom - y) as u32;
                    if shift {
                        if matches!(*handle, 4 | 5) {
                            w = (h as f32 * original.w as f32 / original.h as f32)
                                .round()
                                .max(1.) as u32;
                            x = original.x as i32 + (original.w as i32 - w as i32) / 2;
                        } else {
                            h = (w as f32 * original.h as f32 / original.w as f32)
                                .round()
                                .max(1.) as u32;
                            if matches!(*handle, 0 | 1) {
                                y = bottom - h as i32;
                            }
                        }
                    }
                    if d::valid_size(w, h) {
                        let resized = if matches!(base.kind, ObjectKind::Text { .. }) {
                            resize_text_object(base, w, original.w, shift)
                        } else {
                            let mut object = base.clone();
                            object.kind = ObjectKind::Image(imageops::resize(
                                source,
                                w,
                                h,
                                imageops::FilterType::Nearest,
                            ));
                            object.angle = 0.;
                            object.scale = 1.;
                            Ok(object)
                        };
                        match resized {
                            Ok(mut object) => {
                                object.pos = (x, y);
                                self.doc.objects[*index] = object;
                                self.refresh = true;
                            }
                            Err(error) => self.message = error,
                        }
                    }
                    if released {
                        self.doc.commit();
                    }
                }
                Gesture::TextBox { start } => {
                    let r = Region::between(*start, p, &self.doc.image);
                    dashed_rect(
                        ui.painter(),
                        Rect::from_min_size(
                            rect.min + vec2(r.x as f32, r.y as f32) * self.zoom,
                            vec2(r.w as f32, r.h as f32) * self.zoom,
                        ),
                    );
                    if released {
                        self.text_tab = true;
                        self.text_edit = Some(TextEditState {
                            index: None,
                            origin: (r.x as i32, r.y as i32),
                            text: String::new(),
                            format: crate::text::TextFormat {
                                color: self.colors[0],
                                width: if r.w > 20 { r.w } else { 280 },
                                ..self.text_format.clone()
                            },
                            focus: true,
                            selection: 0..0,
                            insertion_style: None,
                            history: Default::default(),
                        });
                        self.refresh = true;
                    }
                }
                Gesture::Paint {
                    start,
                    last,
                    color,
                    erase_target,
                } => {
                    let mut end = p;
                    if shift && self.tool.is_shape() {
                        let dx = p.0 - start.0;
                        let dy = p.1 - start.1;
                        if matches!(self.tool, Tool::Line | Tool::Curve) {
                            let angle = (dy as f32).atan2(dx as f32);
                            let snap = (angle / std::f32::consts::FRAC_PI_4).round()
                                * std::f32::consts::FRAC_PI_4;
                            let len = ((dx * dx + dy * dy) as f32).sqrt();
                            end = (
                                start.0 + (snap.cos() * len).round() as i32,
                                start.1 + (snap.sin() * len).round() as i32,
                            );
                        } else {
                            let side = dx.abs().max(dy.abs());
                            end = (start.0 + side * dx.signum(), start.1 + side * dy.signum());
                        }
                    }
                    if self.tool.is_shape() {
                        self.doc.restore_preview();
                        d::styled_shape(
                            &mut self.doc.image,
                            self.tool,
                            *start,
                            end,
                            self.size,
                            Some((*color, self.outline)),
                            Some((self.colors[usize::from(erase_target.is_none())], self.fill)),
                        );
                    } else if shift && matches!(self.tool, Tool::Brush | Tool::Pencil) {
                        let delta = (p.0 - start.0, p.1 - start.1);
                        let angle = (delta.1 as f32).atan2(delta.0 as f32);
                        let snapped = (angle / std::f32::consts::FRAC_PI_4).round()
                            * std::f32::consts::FRAC_PI_4;
                        let length = ((delta.0 * delta.0 + delta.1 * delta.1) as f32).sqrt();
                        end = (
                            start.0 + (snapped.cos() * length).round() as i32,
                            start.1 + (snapped.sin() * length).round() as i32,
                        );
                        self.doc.restore_preview();
                        d::line(
                            &mut self.doc.image,
                            *start,
                            end,
                            self.size,
                            *color,
                            if self.tool == Tool::Brush {
                                self.brush
                            } else {
                                Brush::Round
                            },
                        );
                    } else {
                        let width = self.size;
                        let c = if self.tool == Tool::Eraser {
                            self.colors[1]
                        } else {
                            *color
                        };
                        let moves: Vec<Point> = ctx.input(|i| {
                            i.events
                                .iter()
                                .filter_map(|e| {
                                    if let Event::PointerMoved(pos) = e {
                                        Some(self.point(*pos, rect))
                                    } else {
                                        None
                                    }
                                })
                                .collect()
                        });
                        for point in moves.into_iter().chain(std::iter::once(p)) {
                            let point = (
                                point.0.clamp(0, self.doc.image.width() as i32 - 1),
                                point.1.clamp(0, self.doc.image.height() as i32 - 1),
                            );
                            if self.tool == Tool::Eraser {
                                if erase_target.is_some() {
                                    let before = self.doc.composite();
                                    let mut after = before.clone();
                                    d::erase_line(
                                        &mut after,
                                        *last,
                                        point,
                                        width,
                                        c,
                                        *erase_target,
                                    );
                                    for (x, y, pixel) in after.enumerate_pixels() {
                                        if pixel != before.get_pixel(x, y) {
                                            self.doc.image.put_pixel(x, y, *pixel);
                                        }
                                    }
                                } else {
                                    d::erase_line(
                                        &mut self.doc.image,
                                        *last,
                                        point,
                                        width,
                                        c,
                                        None,
                                    );
                                }
                            } else {
                                let brush = if self.tool == Tool::Brush {
                                    self.brush
                                } else {
                                    Brush::Round
                                };
                                d::line(&mut self.doc.image, *last, point, width, c, brush);
                            }
                            *last = point;
                        }
                        if self.tool == Tool::Brush && self.brush == Brush::Airbrush {
                            self.spray_seed = self.spray_seed.wrapping_add(1);
                            d::airbrush_stamp(&mut self.doc.image, p, width, c, self.spray_seed);
                            ctx.request_repaint_after(std::time::Duration::from_millis(30));
                        }
                    }
                    *last = end;
                    self.refresh = true;
                    if released {
                        if self.tool == Tool::Curve {
                            self.curve = Some(CurveBend {
                                start: *start,
                                end,
                                first: None,
                                color_slot: usize::from(erase_target.is_some()),
                                dragging: false,
                            });
                            self.message =
                                "Click to set the first bend; then click for a second bend.".into();
                        } else if self.tool == Tool::Polygon {
                            self.polygon = vec![*start, end];
                            self.polygon_color = *color;
                            self.polygon_color_slot = usize::from(erase_target.is_some());
                            self.message =
                                "Click to add vertices. Double-click or Enter to finish.".into();
                        } else if self.tool.is_shape() {
                            self.start_shape_draft(
                                ShapeGeometry::Primitive {
                                    tool: self.tool,
                                    start: *start,
                                    end,
                                },
                                usize::from(erase_target.is_some()),
                            );
                        } else {
                            self.doc.commit();
                        }
                    }
                }
                Gesture::Select {
                    start,
                    points,
                    click_candidate,
                } => {
                    self.selection = Some(Region::between(*start, p, &self.doc.image));
                    if self.free_select {
                        if points.last() != Some(&p) {
                            points.push(p);
                        }
                        let min_x = points.iter().map(|p| p.0).min().unwrap();
                        let max_x = points.iter().map(|p| p.0).max().unwrap();
                        let min_y = points.iter().map(|p| p.1).min().unwrap();
                        let max_y = points.iter().map(|p| p.1).max().unwrap();
                        self.selection = Some(Region::between(
                            (min_x, min_y),
                            (max_x, max_y),
                            &self.doc.image,
                        ));
                        self.free_points = points.clone();
                        let line: Vec<Pos2> = points
                            .iter()
                            .map(|p| rect.min + vec2(p.0 as f32, p.1 as f32) * self.zoom)
                            .collect();
                        ui.painter()
                            .add(Shape::line(line, Stroke::new(1.0_f32, BLUE)));
                    }
                    if released && *start == p {
                        self.clear_selection();
                        if let Some(index) = *click_candidate {
                            self.select_object(index);
                        }
                        self.select_image_options();
                    }
                }
                Gesture::Move {
                    start,
                    origin,
                    index,
                    last_stamp,
                } => {
                    if shift && raw != *last_stamp && self.doc.objects.len() < 1000 {
                        let mut copy = self.doc.objects[*index].clone();
                        copy.kind = ObjectKind::Raster(copy.render());
                        copy.angle = 0.;
                        copy.scale = 1.;
                        copy.color_key = None;
                        self.doc.objects.insert(*index, copy);
                        *index += 1;
                        self.object = Some(*index);
                        *last_stamp = raw;
                    }
                    self.doc.objects[*index].pos =
                        (origin.0 + raw.0 - start.0, origin.1 + raw.1 - start.1);
                    self.refresh = true;
                    if released {
                        self.doc.commit();
                    }
                }
                Gesture::CanvasSize {
                    start,
                    original,
                    axis,
                } => {
                    let w = if *axis & 1 > 0 {
                        (original.0 as i32 + raw.0 - start.0).max(1) as u32
                    } else {
                        original.0
                    };
                    let h = if *axis & 2 > 0 {
                        (original.1 as i32 + raw.1 - start.1).max(1) as u32
                    } else {
                        original.1
                    };
                    if d::valid_size(w, h) {
                        self.clear_selection();
                        self.doc.restore_preview();
                        self.doc.image = d::resize_canvas(
                            &self.doc.image,
                            w,
                            h,
                            if self.doc.objects.is_empty() {
                                self.colors[1]
                            } else {
                                [0, 0, 0, 0]
                            },
                        );
                        self.refresh = true;
                    }
                    if released {
                        self.doc.commit();
                    }
                }
            }
            if !released {
                self.gesture = Some(gesture);
            }
        }
    }
}

/// Resize text without applying the same width change to both layout and scale.
fn resize_text_object(
    source: &Object,
    width: u32,
    original_width: u32,
    preserve_aspect: bool,
) -> Result<Object, String> {
    let mut object = source.clone();
    let ObjectKind::Text { text, format } = &mut object.kind else {
        return Err("Only text objects can use text-box resizing.".into());
    };
    if preserve_aspect {
        object.scale *= width as f32 / original_width.max(1) as f32;
        if !object.scale.is_finite() || object.scale <= 0.0 || object.scale > 16.0 {
            return Err("Text can be enlarged to at most 1600%.".into());
        }
    } else {
        let layout_width = (width as f32 / object.scale).round().max(10.0);
        if layout_width > crate::text::MAX_TEXT_WIDTH as f32 {
            return Err("The text box would exceed the maximum supported width.".into());
        }
        format.width = layout_width as u32;
    }
    format.validate_for_text(text)?;
    let (raw_width, raw_height) = format.dimensions(text);
    let rendered_width = (raw_width as f64 * object.scale as f64).round().max(1.0) as u32;
    let rendered_height = (raw_height as f64 * object.scale as f64).round().max(1.0) as u32;
    if !d::valid_size(rendered_width, rendered_height)
        || d::rotation_size(rendered_width, rendered_height, object.angle).is_none()
    {
        return Err("The resized text would exceed the 16 megapixel limit.".into());
    }
    Ok(object)
}

/// Test the original button-down position, including ownership of overlapping UI.
pub(in crate::app) fn pointer_press_in(
    ui: &Ui,
    ctx: &Context,
    rect: Rect,
    primary_only: bool,
) -> Option<Pos2> {
    if ctx.memory(|memory| memory.any_popup_open()) {
        return None;
    }
    let events = ctx.input(|input| input.events.clone());
    events.iter().find_map(|event| {
        let Event::PointerButton {
            pos,
            button,
            pressed: true,
            ..
        } = event
        else {
            return None;
        };
        let permitted_button = if primary_only {
            *button == PointerButton::Primary
        } else {
            matches!(button, PointerButton::Primary | PointerButton::Secondary)
        };
        (permitted_button
            && rect.contains(*pos)
            && ui.clip_rect().contains(*pos)
            && ctx.layer_id_at(*pos) == Some(ui.layer_id()))
        .then_some(*pos)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &Context, input: RawInput, popup: bool) -> Option<Pos2> {
        let mut result = None;
        let _ = ctx.run(input, |ctx| {
            CentralPanel::default().show(ctx, |ui| {
                if popup {
                    ctx.memory_mut(|memory| memory.open_popup(Id::new("test-popup")));
                }
                result = pointer_press_in(
                    ui,
                    ctx,
                    Rect::from_min_max(pos2(20., 20.), pos2(300., 300.)),
                    true,
                );
            });
        });
        result
    }

    fn press_and_move() -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
            events: vec![
                Event::PointerMoved(pos2(100., 100.)),
                Event::PointerButton {
                    pos: pos2(100., 100.),
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
                Event::PointerMoved(pos2(500., 500.)),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn fast_drag_uses_button_down_position_even_after_pointer_leaves() {
        let ctx = Context::default();
        frame(&ctx, RawInput::default(), false);
        assert_eq!(frame(&ctx, press_and_move(), false), Some(pos2(100., 100.)));
    }

    #[test]
    fn open_popup_prevents_canvas_gesture_start() {
        let ctx = Context::default();
        frame(&ctx, RawInput::default(), false);
        assert_eq!(frame(&ctx, press_and_move(), true), None);
    }

    #[test]
    fn shift_text_resize_scales_once_and_survives_project_roundtrip() {
        let object = Object::new(
            ObjectKind::Text {
                text: "Hello world".into(),
                format: crate::text::TextFormat::default(),
            },
            (4, 5),
        );
        let doubled = resize_text_object(&object, 560, 280, true).unwrap();
        assert_eq!(doubled.render().width(), 560);
        let tripled = resize_text_object(&doubled, 840, 560, true).unwrap();
        assert_eq!(tripled.render().width(), 840);
        let ObjectKind::Text { format, .. } = &tripled.kind else {
            panic!()
        };
        assert_eq!(format.width, 280);
        assert_eq!(tripled.scale, 3.0);

        let mut document = Document::new(1000, 200);
        document.add_object(tripled);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("resized-text.p10");
        crate::project::save(&document, &path).unwrap();
        assert_eq!(
            crate::project::load(&path).unwrap().composite(),
            document.composite()
        );
        assert!(resize_text_object(&object, 5000, 280, true).is_err());
    }

    #[test]
    fn wide_text_boxes_reflow_without_changing_the_existing_scale() {
        let mut object = Object::new(
            ObjectKind::Text {
                text: "Hello world".into(),
                format: crate::text::TextFormat::default(),
            },
            (0, 0),
        );
        object.scale = 2.0;
        let resized = resize_text_object(&object, 1000, 560, false).unwrap();
        assert_eq!(resized.scale, 2.0);
        assert_eq!(resized.render().width(), 1000);
        object.scale = 1.0;
        let wide = resize_text_object(&object, 10000, 280, false).unwrap();
        let ObjectKind::Text { text, format } = &wide.kind else {
            panic!()
        };
        assert!(format.validate_for_text(text).is_ok());
        assert_eq!(wide.render().width(), 10000);
    }
}
