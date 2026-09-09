use super::*;

const ENABLED: &str = "paint10_thumbnail_enabled";
const IMAGE_RECT: &str = "paint10_thumbnail_image_rect";

#[derive(Clone)]
struct TextPreview {
    text: String,
    format: crate::text::TextFormat,
    texture: TextureHandle,
}

impl PaintApp {
    pub(in crate::app) fn thumbnail_enabled(ctx: &Context) -> bool {
        ctx.data(|data| data.get_temp::<bool>(Id::new(ENABLED)).unwrap_or(false))
    }

    pub(in crate::app) fn set_thumbnail_enabled(ctx: &Context, enabled: bool) {
        ctx.data_mut(|data| data.insert_temp(Id::new(ENABLED), enabled));
    }

    pub(in crate::app) fn thumbnail(&mut self, ctx: &Context) {
        ctx.data_mut(|data| data.remove::<Rect>(Id::new(IMAGE_RECT)));
        if self.zoom <= 1.0 || !Self::thumbnail_enabled(ctx) {
            return;
        }
        let Some(texture) = &self.texture else {
            return;
        };
        let texture = texture.id();
        let text_preview = self.thumbnail_text(ctx);
        let dimensions = vec2(
            self.doc.image.width() as f32,
            self.doc.image.height() as f32,
        );
        let mut open = true;
        Window::new("Thumbnail")
            .id(Id::new("paint10_thumbnail"))
            .order(Order::Foreground)
            .open(&mut open)
            .collapsible(false)
            .default_size(vec2(240.0, 160.0))
            .min_size(vec2(120.0, 80.0))
            .default_pos(ctx.available_rect().right_top() + vec2(-270.0, 20.0))
            .show(ctx, |ui| {
                let available = ui.available_size().max(vec2(100.0, 60.0));
                let (frame, _) = ui.allocate_exact_size(available, Sense::hover());
                let scale = (frame.width() / dimensions.x)
                    .min(frame.height() / dimensions.y)
                    .min(1.0);
                let picture = Rect::from_center_size(frame.center(), dimensions * scale);
                let response = ui.interact(picture, Id::new(IMAGE_RECT), Sense::click_and_drag());
                response.widget_info(|| {
                    WidgetInfo::labeled(
                        WidgetType::ImageButton,
                        response.enabled(),
                        "Navigate picture",
                    )
                });
                ctx.data_mut(|data| data.insert_temp(Id::new(IMAGE_RECT), picture));
                ui.painter().rect_filled(frame, 0.0, RIBBON);
                ui.painter().image(
                    texture,
                    picture,
                    Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                if let (Some(state), Some(preview)) = (&self.text_edit, &text_preview) {
                    let position =
                        picture.min + vec2(state.origin.0 as f32, state.origin.1 as f32) * scale;
                    let rect = Rect::from_min_size(position, preview.texture.size_vec2() * scale);
                    ui.painter().with_clip_rect(picture).image(
                        preview.texture.id(),
                        rect,
                        Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                if let Some(viewport) =
                    ctx.data(|data| data.get_temp::<Rect>(Id::new("paint10_canvas_viewport")))
                {
                    let visible = viewport.intersect(self.canvas_rect);
                    if visible.is_positive() {
                        let factor = scale / self.zoom;
                        let rect = Rect::from_min_max(
                            picture.min + (visible.min - self.canvas_rect.min) * factor,
                            picture.min + (visible.max - self.canvas_rect.min) * factor,
                        );
                        ui.painter().rect_stroke(
                            rect,
                            0.0,
                            Stroke::new(1.5_f32, BLUE),
                            StrokeKind::Inside,
                        );
                    }
                }
                if response.clicked() || response.dragged() {
                    if let Some(position) = response.interact_pointer_pos() {
                        let point = (position - picture.min) / scale;
                        self.center_canvas_on(
                            (
                                point.x.round().clamp(0.0, dimensions.x - 1.0) as i32,
                                point.y.round().clamp(0.0, dimensions.y - 1.0) as i32,
                            ),
                            ctx,
                        );
                    }
                }
                response.on_hover_text("Click or drag to move the visible part of the picture.");
            });
        if !open {
            Self::set_thumbnail_enabled(ctx, false);
        }
    }

    fn thumbnail_text(&self, ctx: &Context) -> Option<TextPreview> {
        let id = Id::new("paint10_thumbnail_text");
        let Some(state) = &self.text_edit else {
            ctx.data_mut(|data| data.remove::<TextPreview>(id));
            return None;
        };
        let cached = ctx.data(|data| data.get_temp::<TextPreview>(id));
        if let Some(cached) = cached {
            if cached.text == state.text && cached.format == state.format {
                return Some(cached);
            }
        }
        // Cache only the editable text. The main canvas texture already tracks
        // all committed objects and raster changes without another full image.
        let raster = state.format.render(&state.text);
        let image = display::image(
            [raster.width() as usize, raster.height() as usize],
            raster.as_raw(),
        );
        let preview = TextPreview {
            text: state.text.clone(),
            format: state.format.clone(),
            texture: ctx.load_texture("thumbnail-text", image, TextureOptions::LINEAR),
        };
        ctx.data_mut(|data| data.insert_temp(id, preview.clone()));
        Some(preview)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(app: &mut PaintApp, ctx: &Context, events: Vec<Event>) {
        let _ = ctx.run(
            RawInput {
                events,
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(640.0, 480.0))),
                ..Default::default()
            },
            |ctx| {
                app.canvas(ctx);
                app.thumbnail(ctx);
                app.dialogs(ctx);
            },
        );
    }

    #[test]
    fn thumbnail_requires_magnification_and_click_navigates_the_real_canvas() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        PaintApp::set_thumbnail_enabled(&ctx, true);
        frame(&mut app, &ctx, vec![]);
        assert!(ctx
            .data(|data| data.get_temp::<Rect>(Id::new(IMAGE_RECT)))
            .is_none());
        app.zoom = 4.0;
        for _ in 0..3 {
            frame(&mut app, &ctx, vec![]);
        }
        let thumbnail = ctx
            .data(|data| data.get_temp::<Rect>(Id::new(IMAGE_RECT)))
            .unwrap();
        let target = thumbnail.min + thumbnail.size() * vec2(0.75, 0.65);
        for pressed in [true, false] {
            frame(
                &mut app,
                &ctx,
                vec![
                    Event::PointerMoved(target),
                    Event::PointerButton {
                        pos: target,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        for _ in 0..3 {
            frame(&mut app, &ctx, vec![]);
        }
        let viewport = ctx
            .data(|data| data.get_temp::<Rect>(Id::new("paint10_canvas_viewport")))
            .unwrap();
        let source_center = (viewport.center() - app.canvas_rect.min) / app.zoom;
        assert!((source_center.x - 675.0).abs() < 3.0, "{source_center:?}");
        assert!((source_center.y - 390.0).abs() < 3.0, "{source_center:?}");
        app.zoom = 1.0;
        frame(&mut app, &ctx, vec![]);
        assert!(ctx
            .data(|data| data.get_temp::<Rect>(Id::new(IMAGE_RECT)))
            .is_none());
        assert!(PaintApp::thumbnail_enabled(&ctx));
    }

    #[test]
    fn navigating_the_thumbnail_preserves_the_active_text_editor() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.zoom = 2.0;
        app.text_edit = Some(TextEditState {
            index: None,
            origin: (10, 150),
            text: "Editable".into(),
            format: crate::text::TextFormat::default(),
            focus: true,
            selection: 0..0,
            insertion_style: None,
            history: text_editing::TextHistory::default(),
            palette_colors: app.colors,
        });
        PaintApp::set_thumbnail_enabled(&ctx, true);
        for _ in 0..3 {
            frame(&mut app, &ctx, vec![]);
        }
        let thumbnail = ctx
            .data(|data| data.get_temp::<Rect>(Id::new(IMAGE_RECT)))
            .unwrap();
        let target = thumbnail.center();
        for pressed in [true, false] {
            frame(
                &mut app,
                &ctx,
                vec![
                    Event::PointerMoved(target),
                    Event::PointerButton {
                        pos: target,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        assert_eq!(app.text_edit.as_ref().unwrap().text, "Editable");
        assert!(app.doc.objects.is_empty());
        let cached = ctx
            .data(|data| data.get_temp::<TextPreview>(Id::new("paint10_thumbnail_text")))
            .unwrap();
        assert_eq!(cached.text, "Editable");
        app.text_edit.as_mut().unwrap().text = "Updated".into();
        frame(&mut app, &ctx, vec![]);
        let updated = ctx
            .data(|data| data.get_temp::<TextPreview>(Id::new("paint10_thumbnail_text")))
            .unwrap();
        assert_eq!(updated.text, "Updated");
        assert_ne!(updated.texture.id(), cached.texture.id());
    }
}
