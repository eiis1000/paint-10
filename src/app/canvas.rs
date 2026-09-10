use super::*;

const CANVAS_CENTER_REQUEST: &str = "paint10_canvas_center_request";

#[derive(Clone)]
pub(super) struct CanvasImageSlot {
    pub painter: Painter,
    pub shape: egui::layers::ShapeIdx,
    pub rect: Rect,
    pub pass: u64,
}

pub(super) const CANVAS_IMAGE_SLOT: &str = "paint10-canvas-image-slot";

pub(super) fn checkerboard(painter: &Painter, rect: Rect, cell: f32) {
    let clip = rect.intersect(painter.clip_rect());
    if !clip.is_positive() {
        return;
    }
    painter.rect_filled(clip, 0.0, Color32::WHITE);
    let x_start = ((clip.left() - rect.left()) / cell).floor() as i32;
    let y_start = ((clip.top() - rect.top()) / cell).floor() as i32;
    let x_end = ((clip.right() - rect.left()) / cell).ceil() as i32;
    let y_end = ((clip.bottom() - rect.top()) / cell).ceil() as i32;
    for y in y_start..y_end {
        for x in x_start..x_end {
            if (x + y) % 2 == 0 {
                let tile = Rect::from_min_size(
                    rect.min + vec2(x as f32, y as f32) * cell,
                    Vec2::splat(cell),
                );
                painter.rect_filled(tile.intersect(clip), 0.0, Color32::from_gray(215));
            }
        }
    }
}

impl PaintApp {
    pub(in crate::app) fn center_canvas_on(&self, point: Point, ctx: &Context) {
        ctx.data_mut(|data| data.insert_temp(Id::new(CANVAS_CENTER_REQUEST), point));
        ctx.request_repaint();
    }

    pub(in crate::app) fn magnify_at(&mut self, point: Point, zoom_out: bool, ctx: &Context) {
        self.zoom = (self.zoom * if zoom_out { 0.5 } else { 2.0 }).clamp(MIN_ZOOM, MAX_ZOOM);
        self.center_canvas_on(point, ctx);
    }

    pub(in crate::app) fn refresh_texture(&mut self, ctx: &Context) {
        self.refresh_shape_hover(ctx);
        if !self.refresh {
            return;
        }
        self.render_revision = self.render_revision.wrapping_add(1);
        self.rendered =
            self.shape_display_image(ctx, self.text_edit.as_ref().and_then(|s| s.index));
        self.canvas_alpha = self.rendered.pixels().any(|pixel| pixel[3] != 255);
        let image = display::image(
            [
                self.rendered.width() as usize,
                self.rendered.height() as usize,
            ],
            self.rendered.as_raw(),
        );
        if let Some(texture) = &mut self.texture {
            texture.set(image, TextureOptions::NEAREST);
        } else {
            self.texture = Some(ctx.load_texture("paint-canvas", image, TextureOptions::NEAREST));
        }
        self.refresh = false;
    }

    pub(in crate::app) fn canvas(&mut self, ctx: &Context) {
        self.measure_readout(ctx);
        // Requests created by canvas input are applied next frame, when the
        // canvas dimensions already reflect any change in zoom.
        let center_request =
            ctx.data_mut(|data| data.remove_temp::<Point>(Id::new(CANVAS_CENTER_REQUEST)));
        self.refresh_shape_style();
        self.sync_image_transparency();
        self.refresh_texture(ctx);
        CentralPanel::default()
            .frame(Frame::NONE.fill(Color32::from_rgb(199, 211, 227)))
            .show(ctx, |ui| {
                let wheel = ctx.input(|i| {
                    if i.modifiers.ctrl || i.modifiers.command {
                        i.raw_scroll_delta.y
                    } else {
                        0.
                    }
                });
                if wheel != 0. {
                    self.zoom =
                        (self.zoom * if wheel > 0. { 1.25 } else { 0.8 }).clamp(MIN_ZOOM, MAX_ZOOM);
                }
                let scroll = ScrollArea::both()
                    .auto_shrink([false, false])
                    .animated(center_request.is_none())
                    .show_viewport(ui, |ui, viewport| {
                        let ruler = if self.rulers { 22. } else { 0. };
                        let origin = ui.cursor().min + vec2(8. + ruler, 8. + ruler);
                        let size = vec2(
                            self.doc.image.width() as f32,
                            self.doc.image.height() as f32,
                        ) * self.zoom;
                        let rect = Rect::from_min_size(origin, size);
                        self.canvas_rect = rect;
                        ui.allocate_space(size + vec2(22. + ruler, 22. + ruler));
                        if let Some(point) = center_request {
                            let offset = (vec2(point.0 as f32, point.1 as f32) * self.zoom
                                + Vec2::splat(8.0 + ruler)
                                - viewport.size() / 2.0)
                                .max(Vec2::ZERO);
                            ui.scroll_with_delta(viewport.min.to_vec2() - offset);
                        }
                        ui.painter().rect_filled(
                            rect.translate(vec2(2., 2.)),
                            0.,
                            Color32::from_gray(142),
                        );
                        if self.canvas_alpha {
                            checkerboard(ui.painter(), rect, 12.0);
                        }
                        let shape = ui.painter().image(
                            self.texture.as_ref().unwrap().id(),
                            rect,
                            Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                            Color32::WHITE,
                        );
                        let pass = ctx.cumulative_pass_nr();
                        ctx.data_mut(|data| {
                            data.insert_temp(
                                Id::new(CANVAS_IMAGE_SLOT),
                                CanvasImageSlot {
                                    painter: ui.painter().clone(),
                                    shape,
                                    rect,
                                    pass,
                                },
                            );
                        });
                        let response = ui
                            .interact(rect, Id::new("canvas"), Sense::click_and_drag())
                            .on_hover_cursor(match self.tool {
                                _ if self.measure.enabled => CursorIcon::Crosshair,
                                Tool::Text
                                    if self.active_layer_editable()
                                        && ctx
                                            .input(|input| input.pointer.hover_pos())
                                            .is_some_and(|position| {
                                                self.selected_equation_at(
                                                    self.point(position, rect),
                                                )
                                                .is_some()
                                            }) =>
                                {
                                    CursorIcon::Move
                                }
                                Tool::Text => CursorIcon::Text,
                                Tool::Select if self.object.is_some() => CursorIcon::Move,
                                _ => CursorIcon::Crosshair,
                            });
                        if !self.measure.enabled
                            && matches!(self.tool, Tool::Select | Tool::Text)
                            && self.text_edit.is_none()
                        {
                            response.context_menu(|ui| self.selection_menu(ui, ctx));
                        }
                        if self.grid && self.zoom >= 4. {
                            let clip = rect.intersect(ui.clip_rect());
                            let step = self.zoom;
                            for x in (((clip.left() - rect.left()) / step).max(0.) as u32)
                                ..=(((clip.right() - rect.left()) / step) as u32)
                            {
                                let xx = rect.left() + x as f32 * step;
                                ui.painter().line_segment(
                                    [pos2(xx, clip.top()), pos2(xx, clip.bottom())],
                                    Stroke::new(0.5_f32, Color32::from_black_alpha(65)),
                                );
                            }
                            for y in (((clip.top() - rect.top()) / step).max(0.) as u32)
                                ..=(((clip.bottom() - rect.top()) / step) as u32)
                            {
                                let yy = rect.top() + y as f32 * step;
                                ui.painter().line_segment(
                                    [pos2(clip.left(), yy), pos2(clip.right(), yy)],
                                    Stroke::new(0.5_f32, Color32::from_black_alpha(65)),
                                );
                            }
                        }
                        if self.rulers {
                            self.draw_rulers(ui, rect);
                        }
                        if let Some((position, r)) =
                            self.selection_bounds().filter(|_| self.text_edit.is_none())
                        {
                            let sr = Rect::from_min_size(
                                rect.min + vec2(position.0 as f32, position.1 as f32) * self.zoom,
                                vec2(r.w as f32, r.h as f32) * self.zoom,
                            );
                            dashed_rect(ui.painter(), sr);
                            let endpoints = self
                                .shape_draft
                                .as_ref()
                                .and_then(ShapeDraft::line_endpoints);
                            // A live marquee is not resizable yet. Registering
                            // its handles would capture the next click using
                            // the previous frame's one-pixel selection bounds.
                            let handles = if matches!(self.gesture, Some(Gesture::Select { .. })) {
                                Vec::new()
                            } else if let Some(points) = endpoints {
                                points
                                    .into_iter()
                                    .map(|point| {
                                        rect.min
                                            + vec2(point.0 as f32 + 0.5, point.1 as f32 + 0.5)
                                                * self.zoom
                                    })
                                    .collect::<Vec<_>>()
                            } else {
                                vec![
                                    sr.left_top(),
                                    sr.right_top(),
                                    sr.left_bottom(),
                                    sr.right_bottom(),
                                    sr.center_top(),
                                    sr.center_bottom(),
                                    sr.left_center(),
                                    sr.right_center(),
                                ]
                            };
                            for (handle, pos) in handles.into_iter().enumerate() {
                                ui.painter().rect(
                                    Rect::from_center_size(pos, vec2(5., 5.)),
                                    0.,
                                    Color32::WHITE,
                                    Stroke::new(1.0_f32, BLUE),
                                    StrokeKind::Inside,
                                );
                                if self.measure.enabled {
                                    continue;
                                }
                                ui.interact(
                                    Rect::from_center_size(pos, vec2(9., 9.)),
                                    Id::new(("selection_handle", handle)),
                                    Sense::drag(),
                                )
                                .on_hover_cursor(
                                    if endpoints.is_some() {
                                        CursorIcon::Crosshair
                                    } else {
                                        match handle {
                                            4 | 5 => CursorIcon::ResizeVertical,
                                            6 | 7 => CursorIcon::ResizeHorizontal,
                                            0 | 3 => CursorIcon::ResizeNwSe,
                                            _ => CursorIcon::ResizeNeSw,
                                        }
                                    },
                                );
                                let press = pointer_press_in(
                                    ui,
                                    ctx,
                                    Rect::from_center_size(pos, vec2(9., 9.)),
                                    true,
                                );
                                if let Some(press) = press.filter(|_| {
                                    self.text_edit.is_none()
                                        && !self.measure.enabled
                                        && self.active_layer_editable()
                                        && self.dialog.is_none()
                                        && self.pending.is_none()
                                }) {
                                    if let Some(shape) = &self.shape_draft {
                                        self.gesture = Some(if endpoints.is_some() {
                                            Gesture::LineEndpoint {
                                                start: self.point(press, rect),
                                                original: shape.clone(),
                                                endpoint: handle,
                                            }
                                        } else {
                                            Gesture::ResizeShape {
                                                bounds: r,
                                                original: shape.clone(),
                                                start: self.point(press, rect),
                                                handle,
                                            }
                                        });
                                    } else {
                                        if self.object.is_some() {
                                            self.doc.begin();
                                        }
                                        self.gesture = Some(Gesture::ResizeObject {
                                            index: self.object,
                                            original: r,
                                            start: self.point(press, rect),
                                            base: self.object.map(|index| {
                                                Box::new(self.doc.objects[index].clone())
                                            }),
                                            handle,
                                        });
                                    }
                                }
                            }
                        }
                        if !self.measure.enabled {
                            self.text_geometry_handles(ui, ctx, rect);
                        }
                        let blocked = self.dialog.is_some()
                            || self.pending.is_some()
                            || self.text_edit.is_some();
                        let blocked = blocked || self.measure.enabled;
                        for (axis, pos) in [
                            (1, rect.right_center()),
                            (2, rect.center_bottom()),
                            (3, rect.right_bottom()),
                        ] {
                            let handle = Rect::from_center_size(pos + vec2(2., 2.), vec2(7., 7.));
                            ui.painter().rect(
                                handle,
                                0.,
                                Color32::WHITE,
                                Stroke::new(1.0_f32, Color32::from_gray(96)),
                                StrokeKind::Inside,
                            );
                            if self.measure.enabled {
                                continue;
                            }
                            ui.interact(
                                handle.expand(2.),
                                Id::new(("canvas_handle", axis)),
                                Sense::drag(),
                            )
                            .on_hover_cursor(match axis {
                                1 => CursorIcon::ResizeHorizontal,
                                2 => CursorIcon::ResizeVertical,
                                _ => CursorIcon::ResizeNwSe,
                            });
                            if let Some(press) = pointer_press_in(ui, ctx, handle.expand(2.), true)
                                .filter(|_| {
                                    !blocked
                                        && !matches!(
                                            self.gesture,
                                            Some(
                                                Gesture::ResizeShape { .. }
                                                    | Gesture::ResizeObject { .. }
                                                    | Gesture::LineEndpoint { .. }
                                            )
                                        )
                                })
                            {
                                self.commit_shape();
                                self.doc.begin();
                                self.gesture = Some(Gesture::CanvasSize {
                                    start: self.point(press, rect),
                                    original: self.doc.image.dimensions(),
                                    axis,
                                });
                            }
                        }
                        if !blocked
                            && self.shape_draft.is_some()
                            && self.gesture.is_none()
                            && pointer_press_in(ui, ctx, ui.clip_rect(), true)
                                .is_some_and(|position| !rect.contains(position))
                        {
                            self.commit_shape();
                        }
                        if !blocked {
                            self.canvas_input(ui, &response, rect, ctx);
                        }
                        if self.measure.enabled {
                            self.measure_canvas(ui, &response, rect, ctx);
                        }
                    });
                ctx.data_mut(|data| {
                    data.insert_temp(Id::new("paint10_canvas_viewport"), scroll.inner_rect);
                });
            });
    }

    pub(in crate::app) fn point(&self, p: Pos2, rect: Rect) -> Point {
        (
            ((p.x - rect.left()) / self.zoom).floor() as i32,
            ((p.y - rect.top()) / self.zoom).floor() as i32,
        )
    }

    /// Object handles describe the complete object, including pixels outside the canvas.
    fn selection_bounds(&self) -> Option<(Point, Region)> {
        if let Some(object) = self.object.and_then(|index| self.doc.objects.get(index)) {
            let (w, h) = object.rendered_dimensions()?;
            return Some((object.pos, Region { x: 0, y: 0, w, h }));
        }
        self.selected_region()
            .map(|region| ((region.x as i32, region.y as i32), region))
    }

    fn text_geometry_handles(&mut self, ui: &Ui, ctx: &Context, canvas: Rect) {
        let Some(state) = &self.text_edit else {
            return;
        };
        if self.dialog.is_some() || self.pending.is_some() || keytips::popup_open(ctx) {
            return;
        }
        let Some(editor) =
            ctx.data(|data| data.get_temp::<Rect>(Id::new("paint10_text_editor_rect")))
        else {
            return;
        };
        let border = editor.expand(3.0);
        let handles = [
            border.left_top(),
            border.right_top(),
            border.left_bottom(),
            border.right_bottom(),
            border.center_top(),
            border.center_bottom(),
            border.left_center(),
            border.right_center(),
        ];
        let painter = ctx
            .layer_painter(LayerId::new(Order::Foreground, Id::new("text_box_handles")))
            .with_clip_rect(ui.clip_rect());
        for point in handles {
            painter.rect(
                Rect::from_center_size(point, vec2(5.0, 5.0)),
                0.0,
                Color32::WHITE,
                Stroke::new(1.0_f32, BLUE),
                StrokeKind::Inside,
            );
        }
        if let Some(position) = ctx
            .input(|input| input.pointer.hover_pos())
            .filter(|position| ui.clip_rect().contains(*position))
        {
            if let Some(handle) = handles.iter().position(|point| {
                Rect::from_center_size(*point, vec2(11.0, 11.0)).contains(position)
            }) {
                ctx.set_cursor_icon(match handle {
                    4 | 5 => CursorIcon::ResizeVertical,
                    6 | 7 => CursorIcon::ResizeHorizontal,
                    0 | 3 => CursorIcon::ResizeNwSe,
                    _ => CursorIcon::ResizeNeSw,
                });
            } else if border.expand(5.0).contains(position)
                && !editor.shrink(5.0).contains(position)
            {
                ctx.set_cursor_icon(CursorIcon::Move);
            }
        }
        let events = ctx.input(|input| input.events.clone());
        let press = events.iter().find_map(|event| {
            let Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                ..
            } = event
            else {
                return None;
            };
            let layer = ctx.layer_id_at(*pos);
            let allowed = layer == Some(ui.layer_id())
                || layer == Some(LayerId::new(Order::Foreground, Id::new("inline_text")));
            (allowed
                && ui.clip_rect().contains(*pos)
                && border.expand(5.0).contains(*pos)
                && !editor.shrink(5.0).contains(*pos))
            .then_some(*pos)
        });
        if let Some(press) = press {
            let start = self.point(press, canvas);
            if let Some(handle) = handles
                .iter()
                .position(|point| Rect::from_center_size(*point, vec2(11.0, 11.0)).contains(press))
            {
                self.gesture = Some(Gesture::ResizeText {
                    start,
                    origin: state.origin,
                    width: state.format.width,
                    height: (editor.height() / self.zoom).round().max(1.0) as u32,
                    minimum_height: state.format.minimum_height,
                    handle,
                });
            } else {
                self.gesture = Some(Gesture::MoveText {
                    start,
                    origin: state.origin,
                });
            }
            self.begin_text_geometry_history();
        }
        self.continue_text_geometry_gesture(ui, ctx, canvas);
    }

    pub(in crate::app) fn draw_rulers(&self, ui: &Ui, r: Rect) {
        let top = Rect::from_min_max(r.min - vec2(0., 22.), r.right_top());
        let left = Rect::from_min_max(r.min - vec2(22., 0.), r.left_bottom());
        ui.painter().rect_filled(top, 0., RIBBON);
        ui.painter().rect_filled(left, 0., RIBBON);
        for (vertical, length) in [
            (false, self.doc.image.width()),
            (true, self.doc.image.height()),
        ] {
            let tick = if self.zoom < 0.5 {
                100
            } else if self.zoom < 2. {
                50
            } else {
                10
            };
            for n in (0..=length).step_by(tick) {
                let p = if vertical {
                    r.min + vec2(-1., n as f32 * self.zoom)
                } else {
                    r.min + vec2(n as f32 * self.zoom, -1.)
                };
                ui.painter().line_segment(
                    [
                        p,
                        p + if vertical {
                            vec2(-5., 0.)
                        } else {
                            vec2(0., -5.)
                        },
                    ],
                    Stroke::new(1.0_f32, Color32::from_gray(100)),
                );
                ui.painter().text(
                    p + if vertical {
                        vec2(-9., 0.)
                    } else {
                        vec2(2., -9.)
                    },
                    if vertical {
                        Align2::RIGHT_CENTER
                    } else {
                        Align2::LEFT_BOTTOM
                    },
                    n.to_string(),
                    FontId::proportional(9.),
                    Color32::from_gray(85),
                );
            }
        }
    }
}

pub(in crate::app) fn dashed_rect(p: &Painter, r: Rect) {
    p.rect_stroke(
        r,
        0.,
        Stroke::new(1.0_f32, Color32::WHITE),
        StrokeKind::Outside,
    );
    for (a, b) in [
        (r.left_top(), r.right_top()),
        (r.right_top(), r.right_bottom()),
        (r.right_bottom(), r.left_bottom()),
        (r.left_bottom(), r.left_top()),
    ] {
        let len = a.distance(b);
        let dir = (b - a).normalized();
        for i in (0..len as usize).step_by(8) {
            p.line_segment(
                [a + dir * i as f32, a + dir * (i as f32 + 4.).min(len)],
                Stroke::new(1.0_f32, Color32::from_gray(30)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_texture_composites_translucent_pixels_like_the_document() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let source = RgbaImage::from_fn(2, 1, |x, _| {
            Rgba(if x == 0 {
                [192, 196, 185, 96]
            } else {
                [255, 23, 240, 0]
            })
        });
        app.doc = Document::from_image(source.clone());
        let output = ctx.run(RawInput::default(), |ctx| app.refresh_texture(ctx));
        let id = app.texture.as_ref().unwrap().id();
        let delta = &output
            .textures_delta
            .set
            .iter()
            .find(|(texture, _)| *texture == id)
            .unwrap()
            .1;
        let ImageData::Color(uploaded) = &delta.image else {
            panic!("canvas must upload RGBA pixels");
        };
        assert_eq!(
            app.doc.image, source,
            "upload must leave document RGBA intact"
        );
        assert_eq!(uploaded.pixels[1], Color32::TRANSPARENT);
        for gray in [215, 255] {
            let mut expected = RgbaImage::from_pixel(2, 1, Rgba([gray, gray, gray, 255]));
            d::overlay(&mut expected, &source, 0, 0);
            for (displayed, expected) in uploaded.pixels.iter().zip(expected.pixels()) {
                for channel in 0..3 {
                    let actual = (f64::from(displayed[channel])
                        + f64::from(gray) * (1.0 - f64::from(displayed.a()) / 255.0))
                        .round()
                        .clamp(0.0, 255.0) as u8;
                    assert!(actual.abs_diff(expected[channel]) <= 1);
                }
            }
        }
    }

    fn canvas_frame(app: &mut PaintApp, ctx: &Context) -> Rect {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(640.0, 480.0))),
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            TopBottomPanel::top("test_toolbar")
                .exact_height(53.0)
                .show(ctx, |_| {});
            SidePanel::left("test_sidebar")
                .exact_width(79.0)
                .show(ctx, |_| {});
            app.canvas(ctx);
        });
        ctx.data(|data| data.get_temp::<Rect>(Id::new("paint10_canvas_viewport")))
            .unwrap()
    }

    #[test]
    fn magnifier_centers_the_clicked_detail_in_the_actual_viewport() {
        for rulers in [false, true] {
            let ctx = Context::default();
            let mut app = PaintApp::new_with_context(&ctx, false);
            app.doc = Document::new(1600, 1200);
            app.rulers = rulers;
            canvas_frame(&mut app, &ctx);
            canvas_frame(&mut app, &ctx);
            let point = (350, 280);
            app.magnify_at(point, false, &ctx);
            assert_eq!(app.zoom, 2.0);
            for _ in 0..3 {
                canvas_frame(&mut app, &ctx);
            }
            let viewport = canvas_frame(&mut app, &ctx);
            let detail = app.canvas_rect.min + vec2(point.0 as f32, point.1 as f32) * app.zoom;
            assert!(
                detail.distance(viewport.center()) < 1.0,
                "detail {detail:?}, viewport {viewport:?}"
            );
            app.magnify_at(point, true, &ctx);
            for _ in 0..3 {
                canvas_frame(&mut app, &ctx);
            }
            let viewport = canvas_frame(&mut app, &ctx);
            let detail = app.canvas_rect.min + vec2(point.0 as f32, point.1 as f32) * app.zoom;
            assert_eq!(app.zoom, 1.0);
            assert!(detail.distance(viewport.center()) < 1.0);
        }
    }

    #[test]
    fn centering_a_canvas_corner_clamps_the_scroll_offset() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.doc = Document::new(1600, 1200);
        app.center_canvas_on((0, 0), &ctx);
        for _ in 0..3 {
            canvas_frame(&mut app, &ctx);
        }
        let viewport = canvas_frame(&mut app, &ctx);
        assert!(viewport.contains(app.canvas_rect.min));
        assert!((app.canvas_rect.left() - viewport.left() - 8.0).abs() < 1.0);
        assert!((app.canvas_rect.top() - viewport.top() - 8.0).abs() < 1.0);
    }
}
