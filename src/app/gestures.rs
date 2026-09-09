use super::*;

struct CanvasPointer {
    raw: Point,
    clamped: Point,
    shift: bool,
    released: bool,
}

impl PaintApp {
    /// Complete a canvas interaction before a subsequent toolbar or keyboard
    /// action is resolved. egui visits the ribbon before the canvas, regardless
    /// of the order in which the operating system delivered their input events.
    pub(in crate::app) fn canvas_raw_input(&self, ctx: &Context, input: &mut RawInput) {
        let pending = Id::new("paint10_canvas_deferred_input");
        let mut events = ctx
            .data_mut(|data| data.remove_temp::<Vec<(Event, f64)>>(pending))
            .unwrap_or_default();
        let time = input
            .time
            .unwrap_or_else(|| ctx.input(|state| state.time) + input.predicted_dt as f64);
        events.extend(
            std::mem::take(&mut input.events)
                .into_iter()
                .map(|event| (event, time)),
        );
        let shape_keys = self.canvas_owns_shape_keys(ctx);
        let boundary = if self.dialog.is_some() || self.pending.is_some() {
            events.len()
        } else {
            events
                .iter()
                .position(|(event, _)| {
                    is_drawing_button_release(event)
                        || (shape_keys && is_shape_completion_key(event))
                })
                .map_or(events.len(), |index| index + 1)
        };
        let deferred = events.split_off(boundary);
        if let Some((event, time)) = events.last() {
            // A slow repaint must not turn two clicks received together into
            // separate single clicks. Advance time to the last processed event.
            input.time = Some(*time);
            if !deferred.is_empty() {
                if let Event::PointerButton { modifiers, .. } | Event::Key { modifiers, .. } = event
                {
                    input.modifiers = *modifiers;
                }
            }
        }
        input.events = events.into_iter().map(|(event, _)| event).collect();
        if !deferred.is_empty() {
            ctx.data_mut(|data| data.insert_temp(pending, deferred));
            ctx.request_repaint();
        }
    }

    fn canvas_owns_shape_keys(&self, ctx: &Context) -> bool {
        (!self.polygon.is_empty() || self.curve.is_some() || self.shape_draft.is_some())
            && self.gesture.is_none()
            && self.text_edit.is_none()
            && self.dialog.is_none()
            && self.pending.is_none()
            && self.print_preview.is_none()
            && !self.preview
            && !self.measure.enabled
            && !self.browser_dialog_open()
            && !keytips::active(ctx)
            && !keytips::popup_open(ctx)
            && ctx
                .memory(|memory| memory.focused().is_none() || memory.has_focus(Id::new("canvas")))
    }

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
        if let Some(position) = gesture_pointer_position(ctx, None) {
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
        let hover = self.point(pos, rect);
        self.cursor = response.hovered().then_some((
            hover.0.clamp(0, self.doc.image.width() as i32 - 1),
            hover.1.clamp(0, self.doc.image.height() as i32 - 1),
        ));
        let press_pos = pointer_press_in(
            ui,
            ctx,
            rect,
            matches!(self.tool, Tool::Select | Tool::Text),
        );
        let raw = self.point(
            gesture_pointer_position(ctx, press_pos).unwrap_or(pos),
            rect,
        );
        let p = (
            raw.0.clamp(0, self.doc.image.width() as i32 - 1),
            raw.1.clamp(0, self.doc.image.height() as i32 - 1),
        );
        let pressed = press_pos.is_some();
        let released = ctx.input(|i| i.pointer.any_released());
        let shift = ctx.input(|i| i.modifiers.shift);
        if !self.polygon.is_empty() {
            self.doc.restore_preview();
            let mut pts = self.polygon.clone();
            pts.push(p);
            let fill = self.shape_fill(self.polygon_color_slot);
            d::styled_polygon(
                &mut self.doc.image,
                &pts,
                Some((self.polygon_color, self.outline)),
                Some(fill),
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
        if response.double_clicked() && self.tool == Tool::Select && self.active_layer_editable() {
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
                            | Gesture::LineEndpoint { .. }
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
        if !matches!(self.tool, Tool::Select | Tool::Picker | Tool::Magnifier)
            && !self.ensure_active_layer_editable()
        {
            return;
        }
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
                let before = self.doc.active_composite();
                let mut after = before.clone();
                d::flood_fill(&mut after, p, color);
                self.doc.begin();
                if color[3] < 255 {
                    if after != before {
                        // An alpha overlay cannot remove the objects beneath it.
                        // Preserve the filled composite; undo restores editability.
                        self.doc.image = after;
                        self.doc.objects.clear();
                        self.clear_selection();
                    }
                } else {
                    for (x, y, px) in after.enumerate_pixels() {
                        if px != before.get_pixel(x, y) {
                            self.doc.image.put_pixel(x, y, *px);
                        }
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
                    if !self.ensure_active_layer_editable() {
                        return;
                    }
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
                    if !self.ensure_active_layer_editable() {
                        return;
                    }
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
                        image_edits: Default::default(),
                        source_clip: None,
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
                    start,
                    handle,
                } => {
                    self.resize_shape(
                        original,
                        *bounds,
                        *handle,
                        (raw.0 - start.0, raw.1 - start.1),
                        shift,
                    );
                }
                Gesture::LineEndpoint {
                    start,
                    original,
                    endpoint,
                } => {
                    self.drag_line_endpoint(
                        original,
                        *endpoint,
                        (raw.0 - start.0, raw.1 - start.1),
                        shift,
                    );
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
                        *base = index.map(|index| Box::new(self.doc.objects[index].clone()));
                    }
                    let Some((index, base)) = index.as_ref().zip(base.as_deref()) else {
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
                        if matches!(object.kind, ObjectKind::Image(_)) {
                            object.image_edits.sampling = if self.pixel_resize {
                                d::ImageSampling::Nearest
                            } else {
                                d::ImageSampling::Smooth
                            };
                        }
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
                            let len = (dx as f32).hypot(dy as f32);
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
                        let fill = self.shape_fill(usize::from(erase_target.is_some()));
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
                            Some(fill),
                        );
                    } else if shift && matches!(self.tool, Tool::Brush | Tool::Pencil) {
                        let delta = (raw.0 - start.0, raw.1 - start.1);
                        let angle = (delta.1 as f32).atan2(delta.0 as f32);
                        let snapped = (angle / std::f32::consts::FRAC_PI_4).round()
                            * std::f32::consts::FRAC_PI_4;
                        let length = (delta.0 as f32).hypot(delta.1 as f32);
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
                            self.editing_background()
                        } else {
                            *color
                        };
                        let moves: Vec<Point> = ctx.input(|i| {
                            let first_event = if *first {
                                i.events
                                    .iter()
                                    .position(|event| {
                                        matches!(event, Event::PointerButton { pos, pressed: true, .. }
                                            if self.point(*pos, rect) == *start)
                                    })
                                    .map_or(0, |index| index + 1)
                            } else {
                                0
                            };
                            i.events[first_event..]
                                .iter()
                                .take_while(|event| !is_drawing_button_release(event))
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
                                if erase_target.is_some()
                                    || (c[3] < 255 && !self.doc.objects.is_empty())
                                {
                                    let before = self.doc.active_composite();
                                    let mut after = before.clone();
                                    d::erase_line(
                                        &mut after,
                                        *last,
                                        point,
                                        width,
                                        c,
                                        *erase_target,
                                    );
                                    if c[3] < 255 {
                                        if after != before {
                                            // Transparent pixels in an overlay cannot erase
                                            // objects underneath it. Keep the visible result;
                                            // the existing undo transaction restores objects.
                                            self.doc.image = after;
                                            self.doc.objects.clear();
                                            self.clear_selection();
                                        }
                                    } else {
                                        for (x, y, pixel) in after.enumerate_pixels() {
                                            if pixel != before.get_pixel(x, y) {
                                                self.doc.image.put_pixel(x, y, *pixel);
                                            }
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
                    let clicked = *start == p
                        && (!self.free_select || points.iter().all(|point| point == start));
                    if released && clicked {
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
                    let object_count: usize = self
                        .doc
                        .layers()
                        .iter()
                        .map(|layer| layer.objects.len())
                        .sum();
                    if shift && raw != *last_stamp && object_count < d::MAX_OBJECTS {
                        let mut copy = self.doc.objects[*index].clone();
                        copy.kind = ObjectKind::Raster(copy.render());
                        copy.angle = 0.;
                        copy.scale = 1.;
                        copy.color_key = None;
                        copy.transform = Default::default();
                        copy.image_edits = Default::default();
                        copy.source_clip = None;
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
                        if let Err(error) = self.doc.resize_canvas(w, h, self.colors[1]) {
                            self.message = error;
                        }
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

fn is_drawing_button_release(event: &Event) -> bool {
    matches!(
        event,
        Event::PointerButton {
            button: PointerButton::Primary | PointerButton::Secondary,
            pressed: false,
            ..
        }
    )
}

fn is_shape_completion_key(event: &Event) -> bool {
    matches!(event, Event::Key {
        key: Key::Enter | Key::Escape,
        pressed: true,
        modifiers,
        ..
    } if modifiers.matches_exact(Modifiers::NONE))
}

/// A frame may contain hover motion after release. That motion updates the
/// cursor, but must not change the endpoint of the completed canvas gesture.
fn gesture_pointer_position(ctx: &Context, press: Option<Pos2>) -> Option<Pos2> {
    ctx.input(|input| {
        let start = press
            .and_then(|press| {
                input.events.iter().position(|event| {
            matches!(event, Event::PointerButton { pos, pressed: true, .. } if *pos == press)
        })
            })
            .unwrap_or(0);
        input
            .events
            .iter()
            .skip(start)
            .find_map(|event| match event {
                Event::PointerButton { pos, .. } if is_drawing_button_release(event) => Some(*pos),
                _ => None,
            })
            .or_else(|| input.pointer.interact_pos())
    })
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
    if keytips::popup_open(ctx) {
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

    mod key_order_tests;
    mod reopen_text_tests;

    fn pointer_app_frame(app: &mut PaintApp, ctx: &Context, events: Vec<Event>) -> FullOutput {
        pointer_app_frame_at(app, ctx, events, ctx.cumulative_pass_nr() as f64 / 30.0)
    }

    fn pointer_app_frame_at(
        app: &mut PaintApp,
        ctx: &Context,
        events: Vec<Event>,
        time: f64,
    ) -> FullOutput {
        let mut input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 850.0))),
            time: Some(time),
            events,
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
            app.keyboard_menu(ctx);
            app.dialogs(ctx);
            if app.refresh {
                ctx.request_repaint();
            }
        })
    }

    fn coalesced_click(position: Pos2, button: PointerButton) -> Vec<Event> {
        vec![
            Event::PointerMoved(position),
            Event::PointerButton {
                pos: position,
                button,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
            Event::PointerButton {
                pos: position,
                button,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
        ]
    }

    #[test]
    fn right_click_with_text_tool_opens_context_without_creating_a_text_box() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.set_tool(Tool::Text);
        for _ in 0..3 {
            pointer_app_frame(&mut app, &ctx, vec![]);
        }
        pointer_app_frame(
            &mut app,
            &ctx,
            coalesced_click(pos2(100.0, 250.0), PointerButton::Secondary),
        );
        for _ in 0..3 {
            pointer_app_frame(&mut app, &ctx, vec![]);
        }
        assert!(app.text_edit.is_none());
        assert!(app.gesture.is_none());
        assert!(!app.doc.dirty());
        assert!(keytips::popup_open(&ctx));
    }

    #[test]
    fn coalesced_canvas_clicks_update_pixels_and_the_settled_texture() {
        let blue = [63, 72, 204, 255];
        let green = [30, 180, 80, 255];
        for tool in [Tool::Pencil, Tool::Eraser, Tool::Fill, Tool::Picker] {
            for button in [PointerButton::Primary, PointerButton::Secondary] {
                let context = Context::default();
                context.enable_accesskit();
                let mut app = PaintApp::new_with_context(&context, false);
                let mut source = RgbaImage::new(32, 24);
                for y in 4..13 {
                    for x in 4..13 {
                        source.put_pixel(
                            x,
                            y,
                            Rgba(if x == 4 || x == 12 || y == 4 || y == 12 {
                                [237, 28, 36, 255]
                            } else {
                                blue
                            }),
                        );
                    }
                }
                app.doc = Document::from_image(source.clone());
                app.zoom = 32.0;
                app.colors = [
                    if tool == Tool::Eraser { blue } else { green },
                    [255, 255, 255, 0],
                ];
                if tool == Tool::Pencil {
                    app.colors[1] = [250, 120, 20, 255];
                }
                app.refresh = true;
                let mut output = pointer_app_frame(&mut app, &context, vec![]);
                for _ in 0..2 {
                    output = pointer_app_frame(&mut app, &context, vec![]);
                }
                let bounds = output
                    .platform_output
                    .accesskit_update
                    .as_ref()
                    .unwrap()
                    .nodes
                    .iter()
                    .find(|(_, node)| node.label() == Some(tool.name()))
                    .unwrap()
                    .1
                    .bounds()
                    .unwrap();
                let tool_position = pos2(
                    ((bounds.x0 + bounds.x1) / 2.0) as f32,
                    ((bounds.y0 + bounds.y1) / 2.0) as f32,
                );
                pointer_app_frame(
                    &mut app,
                    &context,
                    coalesced_click(tool_position, PointerButton::Primary),
                );
                assert_eq!(app.tool, tool);
                let position = app.canvas_rect.min + vec2(8.5, 8.5) * app.zoom;
                let expected = if tool == Tool::Picker {
                    blue
                } else if tool == Tool::Eraser || button == PointerButton::Secondary {
                    app.colors[1]
                } else {
                    app.colors[0]
                };
                let clicked =
                    pointer_app_frame(&mut app, &context, coalesced_click(position, button));
                assert_eq!(
                    app.doc.composite().get_pixel(8, 8).0,
                    expected,
                    "{tool:?} {button:?}: document"
                );
                assert!(app.gesture.is_none());
                let uploaded = pointer_app_frame(&mut app, &context, vec![]);
                pointer_app_frame(&mut app, &context, vec![]);
                assert_eq!(
                    app.rendered,
                    app.doc.composite(),
                    "{tool:?} {button:?}: rendered cache"
                );
                if tool == Tool::Picker {
                    assert_eq!(
                        app.colors[usize::from(button == PointerButton::Secondary)],
                        blue
                    );
                    assert!(!app.doc.dirty());
                } else {
                    let texture = app.texture.as_ref().unwrap().id();
                    let delta = &uploaded
                        .textures_delta
                        .set
                        .iter()
                        .chain(clicked.textures_delta.set.iter())
                        .find(|(id, _)| *id == texture)
                        .expect("settled frame must upload changed canvas pixels")
                        .1;
                    let egui::ImageData::Color(image) = &delta.image else {
                        panic!("canvas texture must be RGBA")
                    };
                    assert_eq!(image.pixels[8 * 32 + 8], display::color(expected));
                    app.doc.undo();
                    assert_eq!(app.doc.composite(), source);
                    assert!(!app.doc.can_undo());
                }
            }
        }
    }

    #[test]
    fn transparent_fill_changes_retained_objects_and_undo_restores_them() {
        for opacity in [0, 96] {
            let context = Context::default();
            let mut app = PaintApp::new_with_context(&context, false);
            app.doc = Document::from_image(RgbaImage::new(32, 24));
            app.doc.add_object(Object::new(
                ObjectKind::Image(RgbaImage::from_pixel(9, 9, Rgba([63, 72, 204, 255]))),
                (4, 4),
            ));
            let original_objects = app.doc.objects.clone();
            let original_pixels = app.doc.composite();
            app.doc.mark_saved();
            app.zoom = 32.0;
            app.set_tool(Tool::Fill);
            app.colors[1] = [255, 255, 255, opacity];
            for _ in 0..3 {
                pointer_app_frame(&mut app, &context, vec![]);
            }
            let position = app.canvas_rect.min + vec2(8.5, 8.5) * app.zoom;
            pointer_app_frame(
                &mut app,
                &context,
                coalesced_click(position, PointerButton::Secondary),
            );
            pointer_app_frame(&mut app, &context, vec![]);
            assert_eq!(app.rendered.get_pixel(8, 8).0, app.colors[1]);
            assert_eq!(app.rendered.get_pixel(0, 0)[3], 0);
            assert!(app.doc.objects.is_empty());
            app.doc.undo();
            assert_eq!(app.doc.composite(), original_pixels);
            assert!(app.doc.objects == original_objects);
            assert!(!app.doc.dirty());
            assert!(!app.doc.can_undo());

            app.colors[1] = [0, 0, 0, 0];
            app.refresh = true;
            let outside = app.canvas_rect.min + vec2(1.5, 1.5) * app.zoom;
            pointer_app_frame(
                &mut app,
                &context,
                coalesced_click(outside, PointerButton::Secondary),
            );
            assert!(app.doc.objects == original_objects);
            assert!(!app.doc.dirty());
            assert!(!app.doc.can_undo());
        }
    }

    #[test]
    fn pointer_motion_after_release_never_extends_a_finished_stroke() {
        for tool in [Tool::Pencil, Tool::Eraser] {
            for coalesced in [false, true] {
                let context = Context::default();
                let mut app = PaintApp::new_with_context(&context, false);
                let (background, painted) = if tool == Tool::Pencil {
                    (WHITE, BLACK)
                } else {
                    (BLACK, WHITE)
                };
                app.doc = Document::from_image(RgbaImage::from_pixel(32, 24, Rgba(background)));
                app.zoom = 32.0;
                app.set_tool(tool);
                app.size = 1;
                app.colors = [BLACK, WHITE];
                for _ in 0..3 {
                    pointer_app_frame(&mut app, &context, vec![]);
                }
                let start = app.canvas_rect.min + vec2(8.5, 8.5) * app.zoom;
                let hover = app.canvas_rect.min + vec2(12.5, 8.5) * app.zoom;
                let mut events = coalesced_click(start, PointerButton::Primary);
                if !coalesced {
                    let release = events.pop().unwrap();
                    pointer_app_frame(&mut app, &context, events);
                    events = vec![release];
                }
                events.push(Event::PointerMoved(hover));
                pointer_app_frame(&mut app, &context, events);
                pointer_app_frame(&mut app, &context, vec![]);
                assert!(app.gesture.is_none());
                assert_eq!(app.rendered.get_pixel(8, 8).0, painted);
                assert_eq!(
                    app.rendered.get_pixel(12, 8).0,
                    background,
                    "{tool:?}, coalesced {coalesced}"
                );
                assert_eq!(
                    app.rendered
                        .pixels()
                        .filter(|pixel| pixel.0 != background)
                        .count(),
                    1
                );
                app.doc.undo();
                assert!(app
                    .doc
                    .composite()
                    .pixels()
                    .all(|pixel| pixel.0 == background));
            }
        }
    }

    #[test]
    fn a_ribbon_tool_click_after_release_cannot_change_the_preceding_drag() {
        for button in [PointerButton::Primary, PointerButton::Secondary] {
            for newer_canvas_click in [false, true] {
                let context = Context::default();
                context.enable_accesskit();
                let mut app = PaintApp::new_with_context(&context, false);
                app.doc = Document::new(240, 180);
                app.zoom = 2.0;
                app.set_tool(Tool::Oval);
                app.colors = [[38, 73, 82, 255], [160, 120, 90, 255]];
                app.outline = PaintStyle::None;
                app.fill = PaintStyle::Solid;
                let mut output = pointer_app_frame(&mut app, &context, vec![]);
                for _ in 0..2 {
                    output = pointer_app_frame(&mut app, &context, vec![]);
                }
                let brush_bounds = output
                    .platform_output
                    .accesskit_update
                    .as_ref()
                    .unwrap()
                    .nodes
                    .iter()
                    .find(|(_, node)| node.label() == Some("Brushes"))
                    .unwrap()
                    .1
                    .bounds()
                    .unwrap();
                let brush_position = pos2(
                    ((brush_bounds.x0 + brush_bounds.x1) / 2.0) as f32,
                    ((brush_bounds.y0 + brush_bounds.y1) / 2.0) as f32,
                );
                let point = |x: f32, y: f32| app.canvas_rect.min + vec2(x, y) * app.zoom;
                let start = point(20.5, 20.5);
                let middle = point(70.5, 55.5);
                let end = point(180.5, 140.5);
                let next_stroke = point(210.5, 160.5);
                let mut press = coalesced_click(start, button);
                press.pop();
                press.push(Event::PointerMoved(middle));
                pointer_app_frame(&mut app, &context, press);
                assert!(app.gesture.is_some());
                let mut release_and_switch = vec![
                    Event::PointerMoved(end),
                    Event::PointerButton {
                        pos: end,
                        button,
                        pressed: false,
                        modifiers: Modifiers::NONE,
                    },
                ];
                release_and_switch.extend(coalesced_click(brush_position, PointerButton::Primary));
                pointer_app_frame(&mut app, &context, release_and_switch);
                if newer_canvas_click {
                    pointer_app_frame(
                        &mut app,
                        &context,
                        coalesced_click(next_stroke, PointerButton::Primary),
                    );
                }
                for _ in 0..3 {
                    pointer_app_frame(&mut app, &context, vec![]);
                }
                let mut expected = RgbaImage::from_pixel(240, 180, Rgba(WHITE));
                let slot = usize::from(button == PointerButton::Secondary);
                d::styled_shape(
                    &mut expected,
                    Tool::Oval,
                    (20, 20),
                    (180, 140),
                    app.tool_sizes[3],
                    Some((app.colors[slot], PaintStyle::None)),
                    Some((app.colors[1 - slot], PaintStyle::Solid).into()),
                );
                if newer_canvas_click {
                    let mut with_next_stroke = expected.clone();
                    d::stamp(
                        &mut with_next_stroke,
                        (210, 160),
                        app.tool_sizes[0],
                        app.colors[0],
                        app.brush,
                    );
                    assert!(app.rendered == with_next_stroke);
                    app.doc.undo();
                    assert!(app.doc.composite() == expected);
                } else {
                    assert!(
                        app.rendered == expected,
                        "{button:?}: full oval and no brush trail"
                    );
                }
                assert_eq!(app.tool, Tool::Brush);
                assert!(app.gesture.is_none());
                assert!(app.shape_draft.is_none());
                app.doc.undo();
                assert!(app.doc.composite().pixels().all(|pixel| pixel.0 == WHITE));
                assert!(!app.doc.can_undo());
            }
        }
    }

    #[test]
    fn batched_double_click_reopens_text_even_when_repaints_are_slow() {
        for frame_delay in [0.03, 0.7] {
            let context = Context::default();
            let mut app = PaintApp::new_with_context(&context, false);
            app.doc = Document::new(400, 200);
            let index = app.doc.add_object(Object::new(
                ObjectKind::Text {
                    text: "Hello world".into(),
                    format: Default::default(),
                },
                (20, 20),
            ));
            app.doc.mark_saved();
            app.set_tool(Tool::Select);
            for frame in 0..3 {
                pointer_app_frame_at(&mut app, &context, vec![], frame as f64 * frame_delay);
            }
            let position = app.canvas_rect.min + vec2(45.5, 30.5) * app.zoom;
            let mut events = coalesced_click(position, PointerButton::Primary);
            events.extend(coalesced_click(position, PointerButton::Primary));
            pointer_app_frame_at(&mut app, &context, events, 3.0 * frame_delay);
            for frame in 4..7 {
                pointer_app_frame_at(&mut app, &context, vec![], frame as f64 * frame_delay);
            }
            assert!(
                app.text_edit.is_some(),
                "double-click should reopen text with {frame_delay}s frames"
            );
            let state = app.text_edit.as_ref().unwrap();
            assert_eq!(state.index, Some(index));
            assert_eq!(state.text, "Hello world");
            assert!(!app.doc.dirty());
        }
    }

    #[test]
    fn double_clicking_rotated_text_preserves_position_pixels_and_saved_state() {
        for angle in [28.0, 270.0] {
            for selected in [false, true] {
                for cancel in [false, true] {
                    let context = Context::default();
                    let mut app = PaintApp::new_with_context(&context, false);
                    app.doc = Document::new(400, 240);
                    let mut object = Object::new(
                        ObjectKind::Text {
                            text: "Rotated title".into(),
                            format: crate::text::TextFormat {
                                width: 160,
                                ..Default::default()
                            },
                        },
                        (20, 20),
                    );
                    object.angle = angle;
                    let index = app.doc.add_object(object);
                    app.doc.mark_saved();
                    app.set_tool(Tool::Select);
                    if selected {
                        app.select_object(index);
                    }
                    let objects = app.doc.objects.clone();
                    let pixels = app.doc.composite();
                    for frame in 0..3 {
                        pointer_app_frame_at(&mut app, &context, vec![], frame as f64 * 0.03);
                    }
                    let first = app.canvas_rect.min + vec2(45.5, 35.5) * app.zoom;
                    let second = first + vec2(2.0, -1.0);
                    let mut events = coalesced_click(first, PointerButton::Primary);
                    events.extend(coalesced_click(second, PointerButton::Primary));
                    pointer_app_frame_at(&mut app, &context, events, 0.12);
                    for frame in 5..8 {
                        pointer_app_frame_at(&mut app, &context, vec![], frame as f64 * 0.03);
                    }
                    let editor = app.text_edit.as_ref().expect("Double-click opens text");
                    assert_eq!(editor.index, Some(index));
                    assert_eq!(editor.origin, objects[index].pos);
                    let ObjectKind::Text { format, .. } = &objects[index].kind else {
                        unreachable!();
                    };
                    assert!(
                        editor.format == *format,
                        "Opening does not rewrite font assets"
                    );
                    assert!(app.doc.objects == objects);
                    assert!(!app.doc.dirty());

                    let event = Event::Key {
                        key: if cancel { Key::Escape } else { Key::Enter },
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: if cancel {
                            Modifiers::NONE
                        } else {
                            Modifiers::CTRL
                        },
                    };
                    pointer_app_frame_at(&mut app, &context, vec![event], 0.3);
                    assert!(app.text_edit.is_none());
                    assert!(app.doc.objects == objects);
                    assert!(app.doc.composite() == pixels);
                    assert!(!app.doc.dirty());
                    assert!(!app.doc.can_undo());
                }
            }
        }
    }

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
    fn canvas_handle_growth_reveals_objects_and_undoes_in_one_step() {
        let context = Context::default();
        let mut app = PaintApp::new_with_context(&context, false);
        app.doc = Document::from_image(RgbaImage::new(20, 20));
        app.doc.add_object(Object::new(
            ObjectKind::Image(RgbaImage::from_pixel(8, 8, Rgba([255, 0, 0, 255]))),
            (18, 2),
        ));
        let objects = app.doc.objects.clone();
        app.colors[1] = [20, 150, 70, 255];
        app.doc.mark_saved();
        app.doc.begin();
        app.gesture = Some(Gesture::CanvasSize {
            start: (20, 20),
            original: (20, 20),
            axis: 3,
        });
        gesture_frame(&mut app, &context, (30, 25), true);
        let expanded = app.doc.composite();
        assert_eq!(expanded.dimensions(), (30, 25));
        assert_eq!(expanded.get_pixel(0, 0)[3], 0);
        assert_eq!(expanded.get_pixel(25, 5).0, [255, 0, 0, 255]);
        assert_eq!(expanded.get_pixel(29, 24).0, app.colors[1]);
        app.doc.undo();
        assert_eq!(app.doc.image.dimensions(), (20, 20));
        assert!(app.doc.objects == objects);
        assert!(!app.doc.dirty());
        assert!(!app.doc.can_undo());
    }

    #[test]
    fn insertion_growth_selects_the_picture_after_adding_background_below_objects() {
        let context = Context::default();
        let mut app = PaintApp::new_with_context(&context, false);
        app.doc = Document::from_image(RgbaImage::new(20, 20));
        app.doc.add_object(Object::new(
            ObjectKind::Image(RgbaImage::from_pixel(8, 8, Rgba([255, 0, 0, 255]))),
            (18, 2),
        ));
        app.colors[1] = [20, 150, 70, 255];
        app.transparent = true;
        let mut inserted = RgbaImage::new(30, 25);
        inserted.put_pixel(27, 24, Rgba([30, 80, 200, 255]));
        app.insert_image(inserted);
        let selected = &app.doc.objects[app.object.unwrap()];
        assert!(matches!(selected.kind, ObjectKind::Image(_)));
        assert_eq!(selected.render().get_pixel(27, 24).0, [30, 80, 200, 255]);
        let expanded = app.doc.composite();
        assert_eq!(expanded.get_pixel(0, 0)[3], 0);
        assert_eq!(expanded.get_pixel(25, 5).0, [255, 0, 0, 255]);
        assert_eq!(expanded.get_pixel(29, 24).0, app.colors[1]);
    }

    #[test]
    fn transparent_eraser_removes_visible_objects_and_undo_restores_editability() {
        for target in [None, Some(BLACK)] {
            for opacity in [0, 96] {
                let context = Context::default();
                let mut app = PaintApp::new_with_context(&context, false);
                app.doc = Document::new(200, 100);
                app.doc.add_object(Object::new(
                    ObjectKind::Image(RgbaImage::from_pixel(30, 30, Rgba(BLACK))),
                    (10, 10),
                ));
                app.doc.add_object(Object::new(
                    ObjectKind::Text {
                        text: "Editable".into(),
                        format: crate::text::TextFormat {
                            width: 90,
                            ..Default::default()
                        },
                    },
                    (90, 10),
                ));
                let original_objects = app.doc.objects.clone();
                let original_image = app.doc.composite();
                app.doc.mark_saved();
                app.tool = Tool::Eraser;
                app.colors[1] = [255, 255, 255, opacity];
                app.size = 3;
                app.doc.begin();
                app.gesture = Some(Gesture::Paint {
                    start: (20, 20),
                    last: (20, 20),
                    color: BLACK,
                    erase_target: target,
                    first: true,
                });
                gesture_frame(&mut app, &context, (25, 20), true);
                assert_eq!(app.doc.composite().get_pixel(22, 20).0, app.colors[1]);
                assert_eq!(app.doc.composite().get_pixel(15, 15).0, BLACK);
                assert!(app.doc.objects.is_empty());
                assert!(app.doc.dirty());
                app.doc.undo();
                assert!(app.doc.objects == original_objects);
                assert_eq!(app.doc.composite(), original_image);
                assert!(!app.doc.dirty());
                assert!(!app.doc.can_undo());
            }
        }
    }

    #[test]
    fn transparent_color_eraser_preserves_objects_when_the_target_does_not_match() {
        let context = Context::default();
        let mut app = PaintApp::new_with_context(&context, false);
        app.doc = Document::new(200, 100);
        app.doc.add_object(Object::new(
            ObjectKind::Image(RgbaImage::from_pixel(30, 30, Rgba(BLACK))),
            (10, 10),
        ));
        let original = app.doc.objects.clone();
        app.doc.mark_saved();
        app.tool = Tool::Eraser;
        app.colors[1] = [0, 0, 0, 0];
        app.doc.begin();
        app.gesture = Some(Gesture::Paint {
            start: (20, 20),
            last: (20, 20),
            color: BLACK,
            erase_target: Some([255, 0, 0, 255]),
            first: true,
        });
        gesture_frame(&mut app, &context, (25, 20), true);
        assert!(app.doc.objects == original);
        assert!(!app.doc.dirty());
        assert!(!app.doc.can_undo());
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
    fn shift_drag_stamps_cropped_images_without_applying_source_edits_twice() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.doc = Document::new(40, 40);
        app.doc.add_layer().unwrap();
        let mut object = Object::new(
            ObjectKind::Image(RgbaImage::from_pixel(30, 20, Rgba([25, 80, 190, 255]))),
            (0, 0),
        );
        object.image_edits.invert = true;
        let index = app.doc.add_object(object);
        app.doc
            .crop_canvas(Region {
                x: 12,
                y: 0,
                w: 20,
                h: 20,
            })
            .unwrap();
        assert!(app.doc.objects[index].source_clip.is_some());
        let expected = app.doc.objects[index].render();
        let origin = app.doc.objects[index].pos;
        app.doc.begin();
        app.gesture = Some(Gesture::Move {
            start: (0, 0),
            origin,
            index,
            last_stamp: (0, 0),
        });
        let _ = ctx.run(RawInput::default(), |ctx| {
            CentralPanel::default().show(ctx, |ui| {
                app.continue_canvas_gesture(
                    ui,
                    Rect::from_min_size(Pos2::ZERO, vec2(20.0, 20.0)),
                    ctx,
                    CanvasPointer {
                        raw: (5, 0),
                        clamped: (5, 0),
                        shift: true,
                        released: true,
                    },
                );
            });
        });
        assert_eq!(app.doc.objects[index].render(), expected);
        assert!(app.doc.objects[index].source_clip.is_none());
        assert!(app.doc.objects[index + 1].source_clip.is_some());
        app.doc.undo();
        assert_eq!(app.doc.objects.len(), 1);
        assert_eq!(app.doc.objects[index].render(), expected);
    }

    #[test]
    fn shift_stroke_handles_large_outside_pointer_coordinates() {
        let context = Context::default();
        let mut app = PaintApp::new_with_context(&context, false);
        app.doc = Document::new(40, 40);
        app.tool = Tool::Pencil;
        app.size = 1;
        app.doc.begin();
        app.gesture = Some(Gesture::Paint {
            start: (5, 5),
            last: (5, 5),
            color: BLACK,
            erase_target: None,
            first: true,
        });
        let _ = context.run(RawInput::default(), |context| {
            CentralPanel::default().show(context, |ui| {
                app.continue_canvas_gesture(
                    ui,
                    Rect::from_min_size(Pos2::ZERO, vec2(40.0, 40.0)),
                    context,
                    CanvasPointer {
                        raw: (50_000, 50_000),
                        clamped: (39, 39),
                        shift: true,
                        released: true,
                    },
                );
            });
        });
        assert_eq!(app.doc.image.dimensions(), (40, 40));
        assert_eq!(app.doc.image.get_pixel(5, 5).0, BLACK);
        assert_eq!(app.doc.image.get_pixel(39, 39).0, BLACK);
        assert_eq!(app.doc.image.get_pixel(39, 5).0, WHITE);
        app.doc.undo();
        assert!(app.doc.image.pixels().all(|pixel| pixel.0 == WHITE));
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
        let mut expected = Document::new(200, 100).image.clone();
        d::styled_shape(
            &mut expected,
            Tool::Line,
            (20, 20),
            (120, 70),
            app.size,
            Some((BLACK, app.outline)),
            Some((WHITE, app.fill).into()),
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
