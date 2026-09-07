use super::*;

impl PaintApp {
    pub(in crate::app) fn refresh_texture(&mut self, ctx: &Context) {
        if !self.refresh {
            return;
        }
        self.rendered = self
            .doc
            .composite_without(self.text_edit.as_ref().and_then(|s| s.index));
        let image = ColorImage::from_rgba_unmultiplied(
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
        self.refresh_shape_style();
        self.sync_image_transparency();
        self.refresh_texture(ctx);
        CentralPanel::default()
            .frame(Frame::NONE.fill(Color32::from_rgb(199, 211, 227)))
            .show(ctx, |ui| {
                let wheel = ctx.input(|i| {
                    if i.modifiers.ctrl {
                        i.raw_scroll_delta.y
                    } else {
                        0.
                    }
                });
                if wheel != 0. {
                    self.zoom = (self.zoom * if wheel > 0. { 1.25 } else { 0.8 }).clamp(0.125, 8.);
                }
                ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let ruler = if self.rulers { 22. } else { 0. };
                        let origin = ui.cursor().min + vec2(8. + ruler, 8. + ruler);
                        let size = vec2(
                            self.doc.image.width() as f32,
                            self.doc.image.height() as f32,
                        ) * self.zoom;
                        let rect = Rect::from_min_size(origin, size);
                        self.canvas_rect = rect;
                        ui.allocate_space(size + vec2(22. + ruler, 22. + ruler));
                        ui.painter().rect_filled(
                            rect.translate(vec2(2., 2.)),
                            0.,
                            Color32::from_gray(142),
                        );
                        ui.painter().image(
                            self.texture.as_ref().unwrap().id(),
                            rect,
                            Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                            Color32::WHITE,
                        );
                        let response = ui
                            .interact(rect, Id::new("canvas"), Sense::click_and_drag())
                            .on_hover_cursor(match self.tool {
                                Tool::Text => CursorIcon::Text,
                                Tool::Select if self.object.is_some() => CursorIcon::Move,
                                _ => CursorIcon::Crosshair,
                            });
                        if self.tool == Tool::Select && self.text_edit.is_none() {
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
                        if let Some(r) = self.selected_region().filter(|_| self.text_edit.is_none())
                        {
                            let sr = Rect::from_min_size(
                                rect.min + vec2(r.x as f32, r.y as f32) * self.zoom,
                                vec2(r.w as f32, r.h as f32) * self.zoom,
                            );
                            dashed_rect(ui.painter(), sr);
                            for (handle, pos) in [
                                sr.left_top(),
                                sr.right_top(),
                                sr.left_bottom(),
                                sr.right_bottom(),
                                sr.center_top(),
                                sr.center_bottom(),
                                sr.left_center(),
                                sr.right_center(),
                            ]
                            .into_iter()
                            .enumerate()
                            {
                                ui.painter().rect(
                                    Rect::from_center_size(pos, vec2(5., 5.)),
                                    0.,
                                    Color32::WHITE,
                                    Stroke::new(1.0_f32, BLUE),
                                    StrokeKind::Inside,
                                );
                                ui.interact(
                                    Rect::from_center_size(pos, vec2(9., 9.)),
                                    Id::new(("selection_handle", handle)),
                                    Sense::drag(),
                                )
                                .on_hover_cursor(match handle {
                                    4 | 5 => CursorIcon::ResizeVertical,
                                    6 | 7 => CursorIcon::ResizeHorizontal,
                                    0 | 3 => CursorIcon::ResizeNwSe,
                                    _ => CursorIcon::ResizeNeSw,
                                });
                                let pressed = pointer_press_in(
                                    ui,
                                    ctx,
                                    Rect::from_center_size(pos, vec2(9., 9.)),
                                    true,
                                )
                                .is_some();
                                if pressed
                                    && self.text_edit.is_none()
                                    && self.dialog.is_none()
                                    && self.pending.is_none()
                                {
                                    if let Some(shape) = &self.shape_draft {
                                        self.gesture = Some(Gesture::ResizeShape {
                                            bounds: r,
                                            original: shape.clone(),
                                            handle,
                                        });
                                    } else if let Some(i) = self.lift_selection() {
                                        self.gesture = Some(Gesture::ResizeObject {
                                            index: i,
                                            original: r,
                                            source: self.doc.objects[i].render_unkeyed(),
                                            base: self.doc.objects[i].clone(),
                                            handle,
                                        });
                                    }
                                }
                            }
                        }
                        let blocked = self.dialog.is_some()
                            || self.pending.is_some()
                            || self.text_edit.is_some();
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
                    });
            });
    }

    pub(in crate::app) fn point(&self, p: Pos2, rect: Rect) -> Point {
        (
            ((p.x - rect.left()) / self.zoom).floor() as i32,
            ((p.y - rect.top()) / self.zoom).floor() as i32,
        )
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
