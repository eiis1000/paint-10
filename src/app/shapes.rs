use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum StylePreview {
    Outline(PaintStyle),
    Fill(PaintStyle),
    Gradient(Gradient),
    Size(u32),
}

#[derive(Clone, Copy)]
struct HoverPreview {
    pass: u64,
    style: StylePreview,
}

const HOVER_PREVIEW: &str = "paint10-shape-style-hover";
const DISPLAYED_PREVIEW: &str = "paint10-shape-displayed-hover";
const ACCEPTED_PREVIEW: &str = "paint10-shape-accepted-hover";

#[derive(Clone)]
pub(in crate::app) enum ShapeGeometry {
    Primitive {
        tool: Tool,
        start: Point,
        end: Point,
    },
    Curve {
        start: Point,
        end: Point,
        controls: [Point; 2],
    },
    Polygon(Vec<Point>),
}

#[derive(Clone, Copy)]
pub(in crate::app) struct CurveBend {
    pub start: Point,
    pub end: Point,
    pub first: Option<Point>,
    pub color_slot: usize,
    pub dragging: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ShapeStyle {
    colors: [Color; 2],
    outline: PaintStyle,
    fill: PaintStyle,
    fill_gradient: Option<Gradient>,
    size: u32,
}

impl ShapeStyle {
    fn resolved_fill(self, color_slot: usize) -> ShapeFill {
        if let Some(direction) = self.fill_gradient {
            ShapeFill::Gradient {
                direction,
                colors: self.colors,
            }
        } else {
            ShapeFill::Paint(self.colors[1 - color_slot], self.fill)
        }
    }
}

#[derive(Clone)]
pub(in crate::app) struct ShapeDraft {
    geometry: ShapeGeometry,
    color_slot: usize,
    style: ShapeStyle,
}

impl ShapeGeometry {
    fn points(&self) -> Vec<Point> {
        match self {
            Self::Primitive { tool, start, end } => {
                if matches!(tool, Tool::Line | Tool::Curve) {
                    return vec![*start, *end];
                }
                let min = (start.0.min(end.0), start.1.min(end.1));
                let size = ((end.0 - start.0).abs(), (end.1 - start.1).abs());
                d::shape_points(*tool)
                    .into_iter()
                    .map(|(x, y)| {
                        (
                            min.0 + (x * size.0 as f32).round() as i32,
                            min.1 + (y * size.1 as f32).round() as i32,
                        )
                    })
                    .collect()
            }
            Self::Curve {
                start,
                end,
                controls,
            } => vec![*start, *end, controls[0], controls[1]],
            Self::Polygon(points) => points.clone(),
        }
    }

    fn bounds(&self) -> (Point, Point) {
        if let Self::Curve {
            start,
            end,
            controls,
        } = self
        {
            return d::cubic_bounds(*start, *end, *controls);
        }
        let points = self.points();
        let first = points.first().copied().unwrap_or((0, 0));
        points.iter().fold((first, first), |(min, max), point| {
            (
                (min.0.min(point.0), min.1.min(point.1)),
                (max.0.max(point.0), max.1.max(point.1)),
            )
        })
    }

    fn map_points(&mut self, map: impl Fn(Point) -> Point) {
        match self {
            Self::Primitive { start, end, .. } => {
                *start = map(*start);
                *end = map(*end);
            }
            Self::Curve {
                start,
                end,
                controls,
            } => {
                *start = map(*start);
                *end = map(*end);
                controls.iter_mut().for_each(|point| *point = map(*point));
            }
            Self::Polygon(points) => points.iter_mut().for_each(|point| *point = map(*point)),
        }
    }
}

impl ShapeDraft {
    pub(in crate::app) fn line_endpoints(&self) -> Option<[Point; 2]> {
        match self.geometry {
            ShapeGeometry::Primitive {
                tool: Tool::Line,
                start,
                end,
            } => Some([start, end]),
            _ => None,
        }
    }

    fn with_endpoint_delta(&self, endpoint: usize, delta: Point, constrained: bool) -> Self {
        let mut draft = self.clone();
        let ShapeGeometry::Primitive {
            tool: Tool::Line,
            start,
            end,
        } = &mut draft.geometry
        else {
            return draft;
        };
        let (moving, fixed) = if endpoint == 0 {
            (start, *end)
        } else {
            (end, *start)
        };
        if delta == (0, 0) {
            return draft;
        }
        let mut target = (moving.0 + delta.0, moving.1 + delta.1);
        if constrained {
            let dx = (target.0 - fixed.0) as f64;
            let dy = (target.1 - fixed.1) as f64;
            let angle =
                (dy.atan2(dx) / std::f64::consts::FRAC_PI_4).round() * std::f64::consts::FRAC_PI_4;
            let distance = dx.hypot(dy);
            target = (
                fixed.0 + (angle.cos() * distance).round() as i32,
                fixed.1 + (angle.sin() * distance).round() as i32,
            );
        }
        *moving = target;
        draft
    }

    pub(in crate::app) fn bounds(&self, image: &RgbaImage) -> Option<Region> {
        let (min, max) = self.geometry.bounds();
        let width = if self.style.outline == PaintStyle::None {
            1
        } else {
            self.style.size.clamp(1, 1024) as i32
        };
        // Even brush widths place one more pixel before the path than after it.
        let before = width / 2;
        let after = (width - 1) / 2;
        let left = (i64::from(min.0) - i64::from(before)).max(0);
        let top = (i64::from(min.1) - i64::from(before)).max(0);
        let right = (i64::from(max.0) + i64::from(after) + 1).min(i64::from(image.width()));
        let bottom = (i64::from(max.1) + i64::from(after) + 1).min(i64::from(image.height()));
        (right > left && bottom > top).then(|| Region {
            x: left as u32,
            y: top as u32,
            w: (right - left) as u32,
            h: (bottom - top) as u32,
        })
    }

    pub(in crate::app) fn translated(&self, delta: Point) -> Self {
        let mut draft = self.clone();
        draft
            .geometry
            .map_points(|point| (point.0 + delta.0, point.1 + delta.1));
        draft
    }

    pub(in crate::app) fn resized(&self, original: Region, new_min: Point, new_max: Point) -> Self {
        if new_min == (original.x as i32, original.y as i32)
            && new_max
                == (
                    (original.x + original.w - 1) as i32,
                    (original.y + original.h - 1) as i32,
                )
        {
            return self.clone();
        }
        let mut draft = self.clone();
        let (min, max) = self.geometry.bounds();
        // Apply handle movement to the path's edges, keeping outline thickness
        // fixed. This also avoids a jump when the canvas clips part of a stroke.
        let target_min = (
            min.0 + new_min.0 - original.x as i32,
            min.1 + new_min.1 - original.y as i32,
        );
        let target_max = (
            max.0 + new_max.0 - (original.x + original.w - 1) as i32,
            max.1 + new_max.1 - (original.y + original.h - 1) as i32,
        );
        let map_axis = |point: i32, min: i32, max: i32, target_min: i32, target_max: i32| {
            if min == max {
                (target_min + target_max) / 2
            } else {
                let target_max = target_max.max(target_min);
                target_min
                    + ((point - min) as f64 / (max - min) as f64 * (target_max - target_min) as f64)
                        .round() as i32
            }
        };
        draft.geometry.map_points(|point| {
            (
                map_axis(point.0, min.0, max.0, target_min.0, target_max.0),
                map_axis(point.1, min.1, max.1, target_min.1, target_max.1),
            )
        });
        draft
    }

    fn render(&self, image: &mut RgbaImage) {
        let foreground = self.style.colors[self.color_slot];
        let outline = Some((foreground, self.style.outline));
        let fill = Some(self.style.resolved_fill(self.color_slot));
        match &self.geometry {
            ShapeGeometry::Primitive { tool, start, end } => {
                d::styled_shape(image, *tool, *start, *end, self.style.size, outline, fill);
            }
            ShapeGeometry::Curve {
                start,
                end,
                controls,
            } => {
                d::styled_cubic(
                    image,
                    *start,
                    *end,
                    *controls,
                    self.style.size,
                    foreground,
                    self.style.outline,
                );
            }
            ShapeGeometry::Polygon(points) => {
                d::styled_polygon(image, points, outline, fill, self.style.size)
            }
        }
    }
}

impl PaintApp {
    pub(in crate::app) fn preview_shape_style(&self, response: &Response, style: StylePreview) {
        let pass = response.ctx.cumulative_pass_nr();
        if self.shape_draft.is_some()
            && response.enabled()
            && response.hovered()
            && !response.clicked()
            && !response
                .ctx
                .data(|data| data.get_temp::<u64>(Id::new(ACCEPTED_PREVIEW)) == Some(pass))
        {
            response.ctx.data_mut(|data| {
                data.insert_temp(Id::new(HOVER_PREVIEW), HoverPreview { pass, style });
            });
        }
    }

    pub(in crate::app) fn accept_shape_style(&self, ctx: &Context) {
        let pass = ctx.cumulative_pass_nr();
        ctx.data_mut(|data| {
            data.remove::<HoverPreview>(Id::new(HOVER_PREVIEW));
            data.insert_temp(Id::new(ACCEPTED_PREVIEW), pass);
        });
    }

    fn hovered_shape_style(&self, ctx: &Context) -> Option<StylePreview> {
        self.shape_draft.as_ref()?;
        ctx.data(|data| data.get_temp::<HoverPreview>(Id::new(HOVER_PREVIEW)))
            .filter(|preview| preview.pass == ctx.cumulative_pass_nr())
            .map(|preview| preview.style)
    }

    pub(in crate::app) fn refresh_shape_hover(&mut self, ctx: &Context) {
        let current = self.hovered_shape_style(ctx);
        let previous = ctx.data_mut(|data| {
            let id = Id::new(DISPLAYED_PREVIEW);
            let previous = data.get_temp::<Option<StylePreview>>(id).flatten();
            data.insert_temp(id, current);
            previous
        });
        self.refresh |= current != previous;
    }

    pub(in crate::app) fn shape_display_image(
        &self,
        ctx: &Context,
        skip: Option<usize>,
    ) -> RgbaImage {
        let Some((mut draft, preview)) =
            self.shape_draft.clone().zip(self.hovered_shape_style(ctx))
        else {
            return self.doc.composite_without(skip);
        };
        draft.style = self.shape_style();
        match preview {
            StylePreview::Outline(style) => draft.style.outline = style,
            StylePreview::Fill(style) => {
                draft.style.fill = style;
                draft.style.fill_gradient = None;
            }
            StylePreview::Gradient(gradient) => draft.style.fill_gradient = Some(gradient),
            StylePreview::Size(size) => draft.style.size = size,
        }
        let mut raster = self.doc.preview_raster().clone();
        draft.render(&mut raster);
        self.doc.composite_with_raster(&raster, skip)
    }

    pub(in crate::app) fn start_shape_draft(&mut self, geometry: ShapeGeometry, color_slot: usize) {
        self.clear_selection();
        self.shape_draft = Some(ShapeDraft {
            geometry,
            color_slot,
            style: self.shape_style(),
        });
        self.redraw_shape();
        self.message = if self.shape_draft.as_ref().and_then(ShapeDraft::line_endpoints).is_some() {
            "Drag to move the line; drag either endpoint to change its length or angle. Hold Shift to snap to 45° increments."
        } else {
            "Drag to move the shape; drag its handles to resize. Colors, Outline, Fill, and Size remain adjustable."
        }.into();
    }

    fn shape_style(&self) -> ShapeStyle {
        ShapeStyle {
            colors: self.colors,
            outline: self.outline,
            fill: self.fill,
            fill_gradient: self.fill_gradient,
            size: self.size,
        }
    }

    pub(in crate::app) fn shape_fill(&self, color_slot: usize) -> ShapeFill {
        self.shape_style().resolved_fill(color_slot)
    }

    pub(in crate::app) fn refresh_shape_style(&mut self) {
        let style = self.shape_style();
        if let Some(draft) = &mut self.shape_draft {
            if draft.style != style {
                draft.style = style;
                self.redraw_shape();
            }
        }
    }

    pub(in crate::app) fn redraw_shape(&mut self) {
        if let Some(draft) = &self.shape_draft {
            self.doc.restore_preview();
            draft.render(&mut self.doc.image);
            self.refresh = true;
        }
    }

    pub(in crate::app) fn commit_shape(&mut self) -> Option<Region> {
        self.refresh_shape_style();
        let draft = self.shape_draft.take()?;
        let bounds = draft.bounds(&self.doc.image);
        self.doc.commit();
        self.refresh = true;
        bounds
    }

    pub(in crate::app) fn begin_shape_gesture(&mut self, point: Point) -> bool {
        let Some(draft) = &self.shape_draft else {
            return false;
        };
        if draft
            .bounds(&self.doc.image)
            .is_some_and(|bounds| bounds.contains(point))
        {
            self.gesture = Some(Gesture::MoveShape {
                start: point,
                original: draft.clone(),
            });
            true
        } else {
            self.commit_shape();
            false
        }
    }

    pub(in crate::app) fn resize_shape(
        &mut self,
        original: &ShapeDraft,
        bounds: Region,
        handle: usize,
        delta: Point,
        aspect: bool,
    ) {
        if delta == (0, 0) {
            self.shape_draft = Some(original.clone());
            self.redraw_shape();
            return;
        }
        let mut min = (bounds.x as i32, bounds.y as i32);
        let mut max = (
            (bounds.x + bounds.w - 1) as i32,
            (bounds.y + bounds.h - 1) as i32,
        );
        if matches!(handle, 0 | 2 | 6) {
            min.0 = (min.0 + delta.0).min(max.0);
        }
        if matches!(handle, 1 | 3 | 7) {
            max.0 = (max.0 + delta.0).max(min.0);
        }
        if matches!(handle, 0 | 1 | 4) {
            min.1 = (min.1 + delta.1).min(max.1);
        }
        if matches!(handle, 2 | 3 | 5) {
            max.1 = (max.1 + delta.1).max(min.1);
        }
        if aspect {
            if matches!(handle, 4 | 5) {
                let width = (((max.1 - min.1 + 1) as f32 * bounds.w as f32 / bounds.h as f32)
                    .round() as i32)
                    .max(1);
                min.0 = bounds.x as i32 + (bounds.w as i32 - width) / 2;
                max.0 = min.0 + width - 1;
            } else {
                let height = (((max.0 - min.0 + 1) as f32 * bounds.h as f32 / bounds.w as f32)
                    .round() as i32)
                    .max(1);
                if matches!(handle, 0 | 1) {
                    min.1 = max.1 - height + 1;
                } else {
                    max.1 = min.1 + height - 1;
                }
            }
        }
        if d::valid_size((max.0 - min.0 + 1) as u32, (max.1 - min.1 + 1) as u32) {
            self.shape_draft = Some(original.resized(bounds, min, max));
            self.redraw_shape();
        }
    }

    pub(in crate::app) fn drag_line_endpoint(
        &mut self,
        original: &ShapeDraft,
        endpoint: usize,
        delta: Point,
        constrained: bool,
    ) {
        let draft = original.with_endpoint_delta(endpoint, delta, constrained);
        let (min, max) = draft.geometry.bounds();
        if d::valid_size((max.0 - min.0 + 1) as u32, (max.1 - min.1 + 1) as u32) {
            self.shape_draft = Some(draft);
            self.redraw_shape();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas_frame(app: &mut PaintApp, context: &Context, events: Vec<Event>) {
        let _ = context.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(700.0, 500.0))),
                events,
                ..Default::default()
            },
            |context| app.canvas(context),
        );
    }

    fn pointer_button(position: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos: position,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn gradient_shape_and_polygon_drafts_recolor_resize_save_and_undo() {
        for tool in [Tool::Rectangle, Tool::Polygon] {
            for gradient in Gradient::ALL {
                let context = Context::default();
                let mut app = PaintApp::new_with_context(&context, false);
                app.doc = Document::from_image(RgbaImage::new(180, 140));
                app.set_tool(tool);
                app.outline = PaintStyle::None;
                app.fill_gradient = Some(gradient);
                app.colors = [WHITE, [0, 0, 0, 0]];
                for _ in 0..2 {
                    canvas_frame(&mut app, &context, vec![]);
                }
                let origin = app.canvas_rect.min;
                let start = origin + vec2(20.5, 20.5);
                let end = origin + vec2(100.5, if tool == Tool::Polygon { 20.5 } else { 100.5 });
                canvas_frame(
                    &mut app,
                    &context,
                    vec![
                        Event::PointerMoved(start),
                        pointer_button(start, true),
                        Event::PointerMoved(end),
                        pointer_button(end, false),
                    ],
                );
                if tool == Tool::Polygon {
                    let third = origin + vec2(60.5, 100.5);
                    canvas_frame(
                        &mut app,
                        &context,
                        vec![
                            Event::PointerMoved(third),
                            pointer_button(third, true),
                            pointer_button(third, false),
                        ],
                    );
                    canvas_frame(
                        &mut app,
                        &context,
                        vec![Event::Key {
                            key: Key::Enter,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: Modifiers::NONE,
                        }],
                    );
                }
                assert!(app.shape_draft.is_some());
                assert!(app.polygon.is_empty());
                let white = app.doc.image.clone();
                app.colors[0] = [210, 180, 60, 255];
                canvas_frame(&mut app, &context, vec![]);
                assert_ne!(app.doc.image, white);
                assert!(app
                    .doc
                    .image
                    .pixels()
                    .filter(|pixel| pixel[3] > 0)
                    .all(|pixel| pixel.0[..3] == app.colors[0][..3]));
                let colored = app.doc.image.clone();
                let handle = origin + vec2(101.0, 101.0);
                canvas_frame(
                    &mut app,
                    &context,
                    vec![
                        Event::PointerMoved(handle),
                        pointer_button(handle, true),
                        pointer_button(handle, false),
                    ],
                );
                assert_eq!(
                    app.doc.image, colored,
                    "merely touching a handle must not change a gradient"
                );
                canvas_frame(&mut app, &context, vec![pointer_button(handle, true)]);
                let resized = handle + vec2(40.0, 20.0);
                canvas_frame(
                    &mut app,
                    &context,
                    vec![Event::PointerMoved(resized), pointer_button(resized, false)],
                );
                let draft = app.shape_draft.as_ref().unwrap();
                assert_eq!(draft.geometry.bounds(), ((20, 20), (140, 120)));
                let bounds = draft.bounds(&app.doc.image).unwrap();
                assert!(app
                    .doc
                    .image
                    .enumerate_pixels()
                    .filter(|(_, _, pixel)| pixel[3] > 0)
                    .all(|(x, y, _)| bounds.contains((x as i32, y as i32))));
                let final_pixels = app.doc.image.clone();
                assert_ne!(final_pixels, colored);
                app.commit_shape();
                let mut png = std::io::Cursor::new(Vec::new());
                app.doc
                    .composite()
                    .write_to(&mut png, image::ImageFormat::Png)
                    .unwrap();
                assert_eq!(
                    image::load_from_memory(png.get_ref()).unwrap().to_rgba8(),
                    final_pixels
                );
                app.doc.undo();
                assert!(app.doc.image.pixels().all(|pixel| pixel[3] == 0));
                assert!(!app.doc.can_undo());
                app.doc.redo();
                assert_eq!(app.doc.image, final_pixels);
            }
        }
    }

    #[test]
    fn curve_bounds_follow_extrema_with_the_complete_stroke() {
        let mut shape = rectangle();
        shape.geometry = ShapeGeometry::Curve {
            start: (40, 40),
            end: (140, 40),
            controls: [(40, 140), (140, 140)],
        };
        shape.style.size = 8;
        let mut image = RgbaImage::new(200, 200);
        shape.render(&mut image);
        let bounds = shape.bounds(&image).unwrap();
        assert_eq!(
            bounds,
            Region {
                x: 36,
                y: 36,
                w: 108,
                h: 83
            }
        );
        assert!(image
            .enumerate_pixels()
            .filter(|(_, _, pixel)| pixel[3] > 0)
            .all(|(x, y, _)| bounds.contains((x as i32, y as i32))));
        let unchanged = shape.resized(bounds, (36, 36), (143, 118));
        let mut after = RgbaImage::new(200, 200);
        unchanged.render(&mut after);
        assert_eq!(image, after);
    }

    #[test]
    fn off_canvas_shape_does_not_copy_a_phantom_edge_pixel() {
        let context = Context::default();
        let mut app = PaintApp::new_with_context(&context, false);
        app.doc = Document::new(100, 100);
        let shape = rectangle();
        app.shape_draft = Some(shape.translated((-60, 0)));
        assert!(app.selected_region().is_none());
        assert!(app.selected_image().is_none());
        app.shape_draft = Some(shape.translated((100, 0)));
        assert!(app.selected_region().is_none());
        app.shape_draft = Some(shape.translated((0, -60)));
        assert!(app.selected_region().is_none());
        app.shape_draft = Some(shape.translated((0, 100)));
        assert!(app.selected_region().is_none());
        app.shape_draft = Some(shape.translated((-20, 0)));
        assert_eq!(
            app.selected_region(),
            Some(Region {
                x: 0,
                y: 10,
                w: 11,
                h: 21
            })
        );
        assert_eq!(app.selected_image().unwrap().dimensions(), (11, 21));
    }

    #[test]
    fn line_endpoint_drag_changes_the_perpendicular_axis_and_snaps_with_shift() {
        let mut shape = rectangle();
        shape.geometry = ShapeGeometry::Primitive {
            tool: Tool::Line,
            start: (30, 60),
            end: (130, 60),
        };
        shape.style.size = 8;
        let moved = shape.with_endpoint_delta(1, (0, 60), false);
        assert_eq!(moved.line_endpoints(), Some([(30, 60), (130, 120)]));
        assert_eq!(moved.style.size, 8);
        let snapped = shape.with_endpoint_delta(1, (-35, 60), true);
        let [fixed, moved] = snapped.line_endpoints().unwrap();
        assert_eq!(fixed, (30, 60));
        assert_eq!(moved.0 - fixed.0, moved.1 - fixed.1);
        shape.geometry = ShapeGeometry::Primitive {
            tool: Tool::Line,
            start: (60, 30),
            end: (60, 130),
        };
        assert_eq!(
            shape
                .with_endpoint_delta(0, (50, 0), false)
                .line_endpoints(),
            Some([(110, 30), (60, 130)])
        );
    }

    #[test]
    fn line_handle_press_offset_is_a_no_op_and_drag_is_one_undo_step() {
        let context = Context::default();
        let mut app = PaintApp::new_with_context(&context, false);
        app.doc = Document::new(240, 180);
        app.doc.begin();
        app.start_shape_draft(
            ShapeGeometry::Primitive {
                tool: Tool::Line,
                start: (30, 50),
                end: (130, 50),
            },
            0,
        );
        canvas_frame(&mut app, &context, vec![]);
        let original = app.doc.image.clone();
        let press = app.canvas_rect.min + vec2(133.5, 52.5);
        canvas_frame(
            &mut app,
            &context,
            vec![Event::PointerMoved(press), pointer_button(press, true)],
        );
        assert!(matches!(
            app.gesture,
            Some(Gesture::LineEndpoint { endpoint: 1, .. })
        ));
        canvas_frame(&mut app, &context, vec![pointer_button(press, false)]);
        assert_eq!(app.doc.image, original);
        canvas_frame(&mut app, &context, vec![pointer_button(press, true)]);
        let end = press + vec2(0.0, 60.0);
        canvas_frame(&mut app, &context, vec![Event::PointerMoved(end)]);
        canvas_frame(&mut app, &context, vec![pointer_button(end, false)]);
        assert_eq!(
            app.shape_draft.as_ref().unwrap().line_endpoints(),
            Some([(30, 50), (130, 110)])
        );
        assert_ne!(app.doc.image, original);
        app.commit_shape();
        app.doc.undo();
        assert!(app.doc.image.pixels().all(|pixel| pixel.0 == WHITE));
        assert!(!app.doc.can_undo());
    }

    #[test]
    fn ordinary_shape_handle_press_offset_does_not_change_its_geometry() {
        let context = Context::default();
        let mut app = PaintApp::new_with_context(&context, false);
        app.doc.begin();
        app.start_shape_draft(
            ShapeGeometry::Primitive {
                tool: Tool::Heart,
                start: (40, 40),
                end: (180, 150),
            },
            0,
        );
        canvas_frame(&mut app, &context, vec![]);
        let original = app.doc.image.clone();
        let bounds = app
            .shape_draft
            .as_ref()
            .unwrap()
            .bounds(&app.doc.image)
            .unwrap();
        let press = app.canvas_rect.min
            + vec2(
                (bounds.x + bounds.w) as f32 + 3.0,
                (bounds.y + bounds.h) as f32 - 2.0,
            );
        canvas_frame(
            &mut app,
            &context,
            vec![Event::PointerMoved(press), pointer_button(press, true)],
        );
        assert!(matches!(app.gesture, Some(Gesture::ResizeShape { .. })));
        canvas_frame(&mut app, &context, vec![pointer_button(press, false)]);
        assert_eq!(app.doc.image, original);
    }

    fn rectangle() -> ShapeDraft {
        ShapeDraft {
            geometry: ShapeGeometry::Primitive {
                tool: Tool::Rectangle,
                start: (10, 10),
                end: (30, 30),
            },
            color_slot: 0,
            style: ShapeStyle {
                colors: [BLACK, WHITE],
                outline: PaintStyle::Solid,
                fill: PaintStyle::None,
                fill_gradient: None,
                size: 1,
            },
        }
    }

    #[test]
    fn changing_shape_style_replaces_preview_and_keeps_one_undo_step() {
        let mut document = Document::new(80, 60);
        document.begin();
        let mut shape = rectangle();
        shape.render(&mut document.image);
        shape.style.colors[0] = [255, 0, 0, 255];
        shape = shape.translated((20, 0));
        document.restore_preview();
        shape.render(&mut document.image);
        assert_eq!(document.image.get_pixel(10, 10).0, WHITE);
        assert_eq!(document.image.get_pixel(30, 10).0, [255, 0, 0, 255]);
        document.commit();
        document.undo();
        assert!(document.image.pixels().all(|pixel| pixel.0 == WHITE));
        assert!(!document.can_undo());
    }

    #[test]
    fn resizing_keeps_opposite_corner_and_maps_geometry() {
        let image = RgbaImage::new(100, 100);
        let shape = rectangle();
        let resized = shape.resized(shape.bounds(&image).unwrap(), (10, 10), (50, 40));
        let bounds = resized.bounds(&image).unwrap();
        assert_eq!((bounds.x, bounds.y, bounds.w, bounds.h), (10, 10, 41, 31));
    }

    #[test]
    fn selection_bounds_include_every_visible_shape_pixel() {
        let mut geometries: Vec<_> = Tool::SHAPES
            .into_iter()
            .map(|tool| ShapeGeometry::Primitive {
                tool,
                start: (40, 40),
                end: (140, 140),
            })
            .collect();
        geometries.extend([
            ShapeGeometry::Primitive {
                tool: Tool::Line,
                start: (40, 60),
                end: (140, 60),
            },
            ShapeGeometry::Primitive {
                tool: Tool::Line,
                start: (60, 40),
                end: (60, 140),
            },
            ShapeGeometry::Curve {
                start: (40, 40),
                end: (140, 140),
                controls: [(10, 180), (180, 10)],
            },
            ShapeGeometry::Curve {
                start: (30, 100),
                end: (170, 100),
                controls: [(30, -180), (170, 380)],
            },
            ShapeGeometry::Curve {
                start: (60, 60),
                end: (60, 60),
                controls: [(60, 60), (60, 60)],
            },
            ShapeGeometry::Polygon(vec![(30, 90), (100, 35), (155, 150)]),
        ]);
        for geometry in geometries {
            for width in [1, 3, 8, 50] {
                let mut shape = rectangle();
                shape.geometry = geometry.clone();
                shape.style.size = width;
                shape.style.fill = PaintStyle::Solid;
                let mut image = RgbaImage::new(200, 200);
                shape.render(&mut image);
                let region = shape.bounds(&image).unwrap();
                let painted = image.pixels().filter(|pixel| pixel[3] > 0).count();
                let copied = region.extract(&image);
                assert!(painted > 0);
                assert_eq!(
                    painted,
                    copied.pixels().filter(|pixel| pixel[3] > 0).count(),
                    "Copy clipped a {width}-pixel shape outline"
                );
            }
        }
    }

    #[test]
    fn resizing_thick_shape_preserves_its_fixed_visible_corner() {
        let image = RgbaImage::new(100, 100);
        let mut shape = rectangle();
        shape.style.size = 8;
        let original = shape.bounds(&image).unwrap();
        assert_eq!(
            original,
            Region {
                x: 6,
                y: 6,
                w: 28,
                h: 28
            }
        );
        let resized = shape.resized(original, (6, 6), (60, 50));
        assert_eq!(
            resized.bounds(&image).unwrap(),
            Region {
                x: 6,
                y: 6,
                w: 55,
                h: 45
            }
        );
        assert_eq!(resized.style.size, 8);
    }

    #[test]
    fn resizing_clipped_outline_does_not_move_the_shape_without_a_drag() {
        let image = RgbaImage::new(100, 100);
        let mut shape = rectangle().translated((-10, -10));
        shape.style.size = 8;
        let original = shape.bounds(&image).unwrap();
        let resized = shape.resized(
            original,
            (original.x as i32, original.y as i32),
            (
                (original.x + original.w - 1) as i32,
                (original.y + original.h - 1) as i32,
            ),
        );
        let mut before = image.clone();
        let mut after = image.clone();
        shape.render(&mut before);
        resized.render(&mut after);
        assert_eq!(before, after);
    }

    #[test]
    fn shift_resize_uses_vertical_middle_handle_movement() {
        let context = Context::default();
        let mut app = PaintApp::new_with_context(&context, false);
        let shape = rectangle().translated((40, 40));
        let original = shape.bounds(&app.doc.image).unwrap();
        app.doc.begin();
        app.resize_shape(&shape, original, 5, (0, 20), true);
        let bounds = app
            .shape_draft
            .as_ref()
            .unwrap()
            .bounds(&app.doc.image)
            .unwrap();
        assert_eq!(
            bounds,
            Region {
                x: 40,
                y: 50,
                w: 41,
                h: 41
            }
        );
    }
}
