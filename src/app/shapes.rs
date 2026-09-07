use super::*;

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
    size: u32,
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
    pub(in crate::app) fn bounds(&self, image: &RgbaImage) -> Region {
        let (min, max) = self.geometry.bounds();
        let width = if self.style.outline == PaintStyle::None {
            1
        } else {
            self.style.size.clamp(1, 1024) as i32
        };
        // Even brush widths place one more pixel before the path than after it.
        let before = width / 2;
        let after = (width - 1) / 2;
        Region::between(
            (min.0 - before, min.1 - before),
            (max.0 + after, max.1 + after),
            image,
        )
    }

    pub(in crate::app) fn translated(&self, delta: Point) -> Self {
        let mut draft = self.clone();
        draft
            .geometry
            .map_points(|point| (point.0 + delta.0, point.1 + delta.1));
        draft
    }

    pub(in crate::app) fn resized(&self, original: Region, new_min: Point, new_max: Point) -> Self {
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
        let fill = Some((self.style.colors[1 - self.color_slot], self.style.fill));
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
    pub(in crate::app) fn start_shape_draft(&mut self, geometry: ShapeGeometry, color_slot: usize) {
        self.clear_selection();
        self.shape_draft = Some(ShapeDraft {
            geometry,
            color_slot,
            style: self.shape_style(),
        });
        self.redraw_shape();
        self.message = "Drag to move the shape; drag its handles to resize. Colors, Outline, Fill, and Size remain adjustable.".into();
    }

    fn shape_style(&self) -> ShapeStyle {
        ShapeStyle {
            colors: self.colors,
            outline: self.outline,
            fill: self.fill,
            size: self.size,
        }
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
        Some(bounds)
    }

    pub(in crate::app) fn begin_shape_gesture(&mut self, point: Point) -> bool {
        let Some(draft) = &self.shape_draft else {
            return false;
        };
        if draft.bounds(&self.doc.image).contains(point) {
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
        point: Point,
        aspect: bool,
    ) {
        let mut min = (bounds.x as i32, bounds.y as i32);
        let mut max = (
            (bounds.x + bounds.w - 1) as i32,
            (bounds.y + bounds.h - 1) as i32,
        );
        if matches!(handle, 0 | 2 | 6) {
            min.0 = point.0.min(max.0);
        }
        if matches!(handle, 1 | 3 | 7) {
            max.0 = point.0.max(min.0);
        }
        if matches!(handle, 0 | 1 | 4) {
            min.1 = point.1.min(max.1);
        }
        if matches!(handle, 2 | 3 | 5) {
            max.1 = point.1.max(min.1);
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let resized = shape.resized(shape.bounds(&image), (10, 10), (50, 40));
        let bounds = resized.bounds(&image);
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
                let region = shape.bounds(&image);
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
        let original = shape.bounds(&image);
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
            resized.bounds(&image),
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
        let original = shape.bounds(&image);
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
        let original = shape.bounds(&app.doc.image);
        app.doc.begin();
        app.resize_shape(&shape, original, 5, (60, 90), true);
        let bounds = app.shape_draft.as_ref().unwrap().bounds(&app.doc.image);
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
