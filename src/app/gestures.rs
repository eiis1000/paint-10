use super::*;

struct CanvasPointer {
    raw: Point,
    clamped: Point,
    shift: bool,
    released: bool,
}

impl PaintApp {
    pub(in crate::app) fn text_geometry_gesture(&self) -> bool {
        matches!(
            self.gesture,
            Some(Gesture::MoveText { .. } | Gesture::ResizeText { .. })
        )
    }

    pub(in crate::app) fn continue_text_geometry_gesture(
        &mut self,
        ui: &Ui,
        ctx: &Context,
        rect: Rect,
    ) {
        if !self.text_geometry_gesture() {
            return;
        }
        if let Some(position) = ctx.input(|input| input.pointer.interact_pos()) {
            let raw = self.point(position, rect);
            self.continue_canvas_gesture(
                ui,
                rect,
                ctx,
                CanvasPointer {
                    raw,
                    clamped: raw,
                    shift: false,
                    released: ctx.input(|input| input.pointer.any_released()),
                },
            );
        }
    }

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
            if pressed {
                curve.dragging = true;
            }
            if !curve.dragging {
                return;
            }
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
                self.magnify_at(p, right, ctx);
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
                        transform: Default::default(),
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
                    first: true,
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
                Gesture::MoveText { start, origin } => {
                    if let Some(state) = &mut self.text_edit {
                        state.origin = (origin.0 + raw.0 - start.0, origin.1 + raw.1 - start.1);
                    }
                    if released {
                        self.finish_text_geometry_history(ctx.input(|input| input.time));
                    }
                }
                Gesture::ResizeText {
                    start,
                    origin,
                    width,
                    height,
                    minimum_height,
                    handle,
                } => {
                    if let Some(state) = &mut self.text_edit {
                        let (position, (new_width, new_height)) = resized_bounds(
                            *origin,
                            (*width, *height),
                            *handle,
                            (raw.0 - start.0, raw.1 - start.1),
                            false,
                        );
                        let mut format = state.format.clone();
                        format.width = new_width.max(10);
                        format.minimum_height = if raw == *start {
                            *minimum_height
                        } else {
                            new_height
                        };
                        if format.validate_for_text(&state.text).is_ok() {
                            state.origin = position;
                            state.format = format;
                        }
                    }
                    if released {
                        self.finish_text_geometry_history(ctx.input(|input| input.time));
                    }
                }
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
                    start,
                    base,
                    handle,
                } => {
                    if index.is_none() && raw != *start {
                        *index = self.lift_selection();
                        *base = index.map(|index| self.doc.objects[index].clone());
                    }
                    let Some((index, base)) = index.as_ref().zip(base.as_ref()) else {
                        if !released {
                            self.gesture = Some(gesture);
                        }
                        return;
                    };
                    let (position, (w, h)) = resized_bounds(
                        base.pos,
                        (original.w, original.h),
                        *handle,
                        (raw.0 - start.0, raw.1 - start.1),
                        shift,
                    );
                    if d::valid_size(w, h) {
                        let mut object = base.clone();
                        match object.resize_rendered(w, h) {
                            Ok(()) => {
                                object.pos = position;
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
                                minimum_height: r.h,
                                ..self.text_format.clone()
                            },
                            focus: true,
                            selection: 0..0,
                            insertion_style: None,
                            history: Default::default(),
                            palette_colors: self.colors,
                        });
                        self.refresh = true;
                    }
                }
                Gesture::Paint {
                    start,
                    last,
                    color,
                    erase_target,
                    first,
                } => {
                    let mut end = if self.tool.is_shape() { p } else { raw };
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
                            if self.tool == Tool::Polygon {
                                Tool::Line
                            } else {
                                self.tool
                            },
                            *start,
                            end,
                            self.size,
                            Some((*color, self.outline)),
                            Some((self.colors[usize::from(erase_target.is_none())], self.fill)),
                        );
                    } else if shift && matches!(self.tool, Tool::Brush | Tool::Pencil) {
                        let delta = (raw.0 - start.0, raw.1 - start.1);
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
                            let first_event = if *first {
                                i.events
                                    .iter()
                                    .position(|event| {
                                        matches!(event, Event::PointerButton { pressed: true, .. })
                                    })
                                    .map_or(0, |index| index + 1)
                            } else {
                                0
                            };
                            i.events[first_event..]
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
                        for point in moves.into_iter().chain(std::iter::once(raw)) {
                            if !*first && point == *last {
                                continue;
                            }
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
                                if *first {
                                    d::stamp(&mut self.doc.image, *last, width, c, brush);
                                }
                                d::line_from_previous(
                                    &mut self.doc.image,
                                    *last,
                                    point,
                                    width,
                                    c,
                                    brush,
                                );
                            }
                            *first = false;
                            *last = point;
                            self.refresh = true;
                        }
                        if self.tool == Tool::Brush && self.brush == Brush::Airbrush {
                            self.spray_seed = self.spray_seed.wrapping_add(1);
                            d::airbrush_stamp(&mut self.doc.image, raw, width, c, self.spray_seed);
                            self.refresh = true;
                            ctx.request_repaint_after(std::time::Duration::from_millis(30));
                        }
                    }
                    if self.tool.is_shape() || shift {
                        *last = end;
                        self.refresh = true;
                        *first = false;
                    }
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
                        copy.transform = Default::default();
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

/// Move the requested edges by the pointer delta, preserving off-canvas origins.
fn resized_bounds(
    origin: Point,
    dimensions: (u32, u32),
    handle: usize,
    delta: Point,
    preserve_aspect: bool,
) -> (Point, (u32, u32)) {
    let (mut x, mut y) = origin;
    let (mut right, mut bottom) = (x + dimensions.0 as i32, y + dimensions.1 as i32);
    if matches!(handle, 0 | 2 | 6) {
        x = (x + delta.0).min(right - 1);
    }
    if matches!(handle, 1 | 3 | 7) {
        right = (right + delta.0).max(x + 1);
    }
    if matches!(handle, 0 | 1 | 4) {
        y = (y + delta.1).min(bottom - 1);
    }
    if matches!(handle, 2 | 3 | 5) {
        bottom = (bottom + delta.1).max(y + 1);
    }
    let (mut width, mut height) = ((right - x) as u32, (bottom - y) as u32);
    if preserve_aspect {
        if matches!(handle, 4 | 5) {
            width = (height as f64 * dimensions.0 as f64 / dimensions.1 as f64)
                .round()
                .max(1.0) as u32;
            x = origin.0 + (dimensions.0 as i32 - width as i32) / 2;
        } else {
            height = (width as f64 * dimensions.1 as f64 / dimensions.0 as f64)
                .round()
                .max(1.0) as u32;
            if matches!(handle, 0 | 1) {
                y = bottom - height as i32;
            }
            if matches!(handle, 6 | 7) {
                y = origin.1 + (dimensions.1 as i32 - height as i32) / 2;
            }
        }
    }
    ((x, y), (width, height))
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

    fn gesture_frame(app: &mut PaintApp, ctx: &Context, point: Point, released: bool) {
        let _ = ctx.run(RawInput::default(), |ctx| {
            CentralPanel::default().show(ctx, |ui| {
                app.continue_canvas_gesture(
                    ui,
                    Rect::from_min_size(Pos2::ZERO, vec2(200.0, 100.0)),
                    ctx,
                    CanvasPointer {
                        raw: point,
                        clamped: (point.0.clamp(0, 199), point.1.clamp(0, 99)),
                        shift: false,
                        released,
                    },
                );
            });
        });
    }

    #[test]
    fn stationary_marker_frames_do_not_replay_the_initial_stamp() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.doc = Document::new(200, 100);
        app.tool = Tool::Brush;
        app.brush = Brush::Marker;
        app.size = 12;
        app.doc.begin();
        app.gesture = Some(Gesture::Paint {
            start: (20, 20),
            last: (20, 20),
            color: BLACK,
            erase_target: None,
            first: true,
        });
        gesture_frame(&mut app, &ctx, (20, 20), false);
        let once = app.doc.image.clone();
        for _ in 0..10 {
            gesture_frame(&mut app, &ctx, (20, 20), false);
        }
        gesture_frame(&mut app, &ctx, (20, 20), true);
        assert_eq!(app.doc.image, once);
    }

    #[test]
    fn freehand_motion_outside_canvas_does_not_paint_along_its_edge() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.doc = Document::new(200, 100);
        app.tool = Tool::Pencil;
        app.size = 1;
        app.doc.begin();
        app.gesture = Some(Gesture::Paint {
            start: (20, 20),
            last: (20, 20),
            color: BLACK,
            erase_target: None,
            first: true,
        });
        gesture_frame(&mut app, &ctx, (30, -20), false);
        gesture_frame(&mut app, &ctx, (150, -20), true);
        assert!((40..150).all(|x| app.doc.image.get_pixel(x, 0).0 == WHITE));
        assert_eq!(app.doc.image.get_pixel(20, 20).0, BLACK);
    }

    #[test]
    fn clicking_a_raster_selection_handle_does_not_lift_or_dirty_pixels() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let region = Region {
            x: 20,
            y: 20,
            w: 50,
            h: 30,
        };
        app.selection = Some(region);
        app.gesture = Some(Gesture::ResizeObject {
            index: None,
            original: region,
            start: (73, 35),
            base: None,
            handle: 7,
        });
        gesture_frame(&mut app, &ctx, (73, 35), true);
        assert!(app.doc.objects.is_empty());
        assert!(!app.doc.dirty());
        assert!(app.selection.is_some());
    }

    #[test]
    fn curve_preview_does_not_follow_hover_between_bend_drags() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.doc = Document::new(200, 100);
        app.doc.begin();
        d::styled_curve(
            &mut app.doc.image,
            (20, 30),
            (150, 30),
            (80, 75),
            3,
            BLACK,
            PaintStyle::Solid,
        );
        let preview = app.doc.image.clone();
        app.curve = Some(CurveBend {
            start: (20, 30),
            end: (150, 30),
            first: Some((80, 75)),
            color_slot: 0,
            dragging: false,
        });
        let _ = ctx.run(
            RawInput {
                events: vec![Event::PointerMoved(pos2(100.0, 20.0))],
                ..Default::default()
            },
            |ctx| {
                CentralPanel::default().show(ctx, |ui| {
                    let rect = Rect::from_min_size(Pos2::ZERO, vec2(200.0, 100.0));
                    let response =
                        ui.interact(rect, Id::new("curve-test"), Sense::click_and_drag());
                    app.canvas_input(ui, &response, rect, ctx);
                });
            },
        );
        assert_eq!(app.doc.image, preview);
        assert!(app.curve.is_some());
    }

    #[test]
    fn initial_polygon_drag_previews_only_the_first_edge() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.doc = Document::new(200, 100);
        app.tool = Tool::Polygon;
        app.doc.begin();
        app.gesture = Some(Gesture::Paint {
            start: (20, 20),
            last: (20, 20),
            color: BLACK,
            erase_target: None,
            first: true,
        });
        gesture_frame(&mut app, &ctx, (120, 70), false);
        let mut expected = Document::new(200, 100).image;
        d::styled_shape(
            &mut expected,
            Tool::Line,
            (20, 20),
            (120, 70),
            app.size,
            Some((BLACK, app.outline)),
            Some((WHITE, app.fill)),
        );
        assert_eq!(app.doc.image, expected);
    }

    fn editor_frame(app: &mut PaintApp, ctx: &Context, events: Vec<Event>) {
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 800.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                app.canvas(ctx);
                app.text_editor(ctx);
            },
        );
    }

    #[test]
    fn active_text_box_handle_reflows_without_committing_the_editor() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.text_edit = Some(TextEditState {
            index: None,
            origin: (40, 40),
            text: "Hello world".into(),
            format: crate::text::TextFormat {
                width: 200,
                minimum_height: 60,
                ..Default::default()
            },
            focus: true,
            selection: 0..0,
            insertion_style: None,
            history: Default::default(),
            palette_colors: app.colors,
        });
        editor_frame(&mut app, &ctx, vec![]);
        editor_frame(&mut app, &ctx, vec![]);
        let editor = ctx
            .data(|data| data.get_temp::<Rect>(Id::new("paint10_text_editor_rect")))
            .unwrap();
        let press = editor.expand(3.0).right_center();
        editor_frame(
            &mut app,
            &ctx,
            vec![
                Event::PointerMoved(press),
                Event::PointerButton {
                    pos: press,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        assert!(app.text_geometry_gesture());
        let end = press + vec2(80.0, 0.0);
        editor_frame(&mut app, &ctx, vec![Event::PointerMoved(end)]);
        editor_frame(
            &mut app,
            &ctx,
            vec![Event::PointerButton {
                pos: end,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        let state = app
            .text_edit
            .as_ref()
            .expect("Resizing must keep the text editor open");
        assert_eq!(state.format.width, 280);
        assert_eq!(state.text, "Hello world");
        assert_eq!(state.origin, (40, 40));
        assert!(app.doc.objects.is_empty());
        app.text_history_action(Action::Undo, &ctx);
        assert_eq!(app.text_edit.as_ref().unwrap().format.width, 200);
        app.text_history_action(Action::Redo, &ctx);
        assert_eq!(app.text_edit.as_ref().unwrap().format.width, 280);

        editor_frame(&mut app, &ctx, vec![]);
        let editor = ctx
            .data(|data| data.get_temp::<Rect>(Id::new("paint10_text_editor_rect")))
            .unwrap();
        let press = editor.expand(3.0).left_top() + vec2(30.0, 0.0);
        editor_frame(
            &mut app,
            &ctx,
            vec![
                Event::PointerMoved(press),
                Event::PointerButton {
                    pos: press,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        let end = press + vec2(15.0, 20.0);
        editor_frame(&mut app, &ctx, vec![Event::PointerMoved(end)]);
        editor_frame(
            &mut app,
            &ctx,
            vec![Event::PointerButton {
                pos: end,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert_eq!(app.text_edit.as_ref().unwrap().origin, (55, 60));
        app.text_history_action(Action::Undo, &ctx);
        assert_eq!(app.text_edit.as_ref().unwrap().origin, (40, 40));
    }

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
    fn rotated_text_handle_touch_preserves_text_and_history() {
        let mut object = Object::new(
            ObjectKind::Text {
                text: "Hello world".into(),
                format: crate::text::TextFormat::default(),
            },
            (-20, 5),
        );
        object.rotate_to(90.0).unwrap();
        let dimensions = object.rendered_dimensions().unwrap();
        let mut document = Document::new(200, 500);
        let index = document.add_object(object.clone());
        document.mark_saved();
        document.begin();
        for handle in 0..8 {
            let (position, size) = resized_bounds(object.pos, dimensions, handle, (0, 0), false);
            document.objects[index]
                .resize_rendered(size.0, size.1)
                .unwrap();
            document.objects[index].pos = position;
            assert!(document.objects[index] == object);
        }
        document.commit();
        assert!(!document.dirty());
        assert!(!document.can_undo());
    }

    #[test]
    fn resize_handles_keep_the_full_off_canvas_rectangle() {
        assert_eq!(
            resized_bounds((-20, 10), (100, 50), 7, (0, 0), false),
            ((-20, 10), (100, 50))
        );
        assert_eq!(
            resized_bounds((-20, 10), (100, 50), 7, (20, 0), false),
            ((-20, 10), (120, 50))
        );
        assert_eq!(
            resized_bounds((-20, 10), (100, 50), 5, (0, 25), true),
            ((-45, 10), (150, 75))
        );
    }
}
