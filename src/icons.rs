//! Small vector artwork for the ribbon, independent of installed symbol fonts.

mod artwork;
mod brushes;

use crate::document::{Brush, Tool};
use eframe::egui::{self, *};

#[derive(Clone, Copy, Debug)]
pub enum Icon {
    Tool(Tool),
    Brush(Brush),
    New,
    Open,
    Print,
    PrintPreview,
    Email,
    Save,
    Undo,
    Redo,
    Paste,
    Cut,
    Copy,
    Crop,
    Resize,
    Rotate,
    Outline,
    Fill,
    Colors,
    ChevronDown,
    ChevronUp,
}

impl Icon {
    pub fn name(self) -> &'static str {
        match self {
            Self::Tool(tool) => tool.name(),
            Self::Brush(brush) => brush.name(),
            Self::New => "New",
            Self::Open => "Open",
            Self::Print => "Print",
            Self::PrintPreview => "Print preview",
            Self::Email => "Send in email",
            Self::Save => "Save",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::Paste => "Paste",
            Self::Cut => "Cut",
            Self::Copy => "Copy",
            Self::Crop => "Crop",
            Self::Resize => "Resize",
            Self::Rotate => "Rotate",
            Self::Outline => "Outline",
            Self::Fill => "Fill",
            Self::Colors => "Edit colors",
            Self::ChevronDown => "Expand",
            Self::ChevronUp => "Collapse",
        }
    }
}

pub fn draw(painter: &Painter, rect: Rect, icon: Icon) {
    if !rect.is_positive() || !painter.is_visible() {
        return;
    }
    let canvas = Canvas::new(painter, rect);
    match icon {
        Icon::Tool(tool) => artwork::tool(&canvas, tool),
        Icon::Brush(brush) => brushes::draw(&canvas, brush),
        _ => artwork::command(&canvas, icon),
    }
}

/// A stroke sample for brush galleries, drawn at the supplied menu-row size.
pub fn brush_preview(painter: &Painter, rect: Rect, brush: Brush) {
    brushes::preview(painter, rect, brush);
}

pub fn button(
    ui: &mut Ui,
    id: impl std::hash::Hash,
    rect: Rect,
    icon: Icon,
    label: &str,
    selected: bool,
    enabled: bool,
) -> Response {
    let enabled = enabled && ui.is_enabled();
    let id = ui.id().with(id);
    // Use the same child scope in both states so enabling Undo, for example,
    // cannot change the automatic IDs of the menu buttons that follow it.
    let mut builder = UiBuilder::new().id_salt(id).max_rect(rect);
    if !enabled {
        builder = builder.disabled();
    }
    let response = ui.new_child(builder).interact(rect, id, Sense::click());
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
    if selected || (response.hovered() && enabled) {
        let pressed = enabled && response.is_pointer_button_down_on();
        ui.painter().rect(
            rect,
            0.0,
            if pressed {
                Color32::from_rgb(183, 218, 247)
            } else if selected {
                Color32::from_rgb(206, 231, 252)
            } else {
                Color32::from_rgb(229, 243, 255)
            },
            Stroke::new(1.0_f32, Color32::from_rgb(125, 181, 224)),
            StrokeKind::Inside,
        );
    }
    let icon_rect = if label.is_empty() {
        // The 18-point shape cells need 14-point artwork. A four-point inset
        // on both sides reduced arrows and callouts to almost unreadable marks.
        rect.shrink(if rect.width() <= 20.0 { 2.0 } else { 3.0 })
    } else {
        Rect::from_center_size(pos2(rect.center().x, rect.top() + 23.0), vec2(32.0, 32.0))
    };
    let mut painter = ui.painter().clone();
    if !enabled && ui.is_enabled() {
        painter.set_opacity(0.4);
    }
    draw(&painter, icon_rect, icon);
    if !label.is_empty() {
        painter.text(
            pos2(rect.center().x, rect.bottom() - 10.0),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(12.0),
            Color32::from_gray(35),
        );
    }
    if response.has_focus() && enabled {
        focus_outline(ui.painter(), rect.shrink(2.0));
    }
    let name = if label.is_empty() { icon.name() } else { label };
    response.widget_info(|| {
        if matches!(icon, Icon::Tool(_) | Icon::Brush(_)) {
            WidgetInfo::selected(WidgetType::Button, enabled, selected, name)
        } else {
            WidgetInfo::labeled(WidgetType::Button, enabled, name)
        }
    });
    response
}

fn focus_outline(painter: &Painter, rect: Rect) {
    let pixel = 1.0 / painter.pixels_per_point();
    let stroke = Stroke::new(pixel, Color32::from_rgb(25, 67, 101));
    for [start, end] in [
        [rect.left_top(), rect.right_top()],
        [rect.right_top(), rect.right_bottom()],
        [rect.right_bottom(), rect.left_bottom()],
        [rect.left_bottom(), rect.left_top()],
    ] {
        let length = start.distance(end);
        let direction = (end - start).normalized();
        let mut offset = 0.0;
        while offset < length {
            painter.line_segment(
                [
                    start + direction * offset,
                    start + direction * (offset + 2.0 * pixel).min(length),
                ],
                stroke,
            );
            offset += 4.0 * pixel;
        }
    }
}

const INK: Color32 = Color32::from_rgb(64, 76, 86);
const BLUE: Color32 = Color32::from_rgb(38, 113, 177);
const LIGHT_BLUE: Color32 = Color32::from_rgb(195, 225, 244);
const SILVER: Color32 = Color32::from_rgb(223, 231, 235);
const GOLD: Color32 = Color32::from_rgb(238, 181, 56);
const WOOD: Color32 = Color32::from_rgb(185, 111, 46);
const CLEAR: Color32 = Color32::TRANSPARENT;

/// Coordinates use a 24-unit grid. Stroke widths are rounded to physical
/// pixels, while curves retain their subpixel coordinates for smooth edges.
struct Canvas<'a> {
    painter: &'a Painter,
    rect: Rect,
    scale: f32,
}

impl<'a> Canvas<'a> {
    fn new(painter: &'a Painter, rect: Rect) -> Self {
        let side = rect.width().min(rect.height());
        Self {
            painter,
            rect: Rect::from_center_size(rect.center(), Vec2::splat(side)),
            scale: side / 24.0,
        }
    }

    fn point(&self, point: (f32, f32)) -> Pos2 {
        self.rect.min + vec2(point.0, point.1) * self.scale
    }

    fn width(&self, width: f32) -> f32 {
        let pixels_per_point = self.painter.pixels_per_point();
        (width * self.scale * pixels_per_point).round().max(1.0) / pixels_per_point
    }

    fn line(&self, start: (f32, f32), end: (f32, f32), color: Color32, width: f32) {
        let width = self.width(width);
        let mut points = [self.point(start), self.point(end)];
        // Axis-aligned strokes occupy full physical pixels. Diagonal paths
        // and curves retain their ordinary antialiasing.
        let pixels_per_point = self.painter.pixels_per_point();
        let snap = |coordinate: f32| {
            ((coordinate * pixels_per_point - width * pixels_per_point / 2.0).round()
                + width * pixels_per_point / 2.0)
                / pixels_per_point
        };
        if start.0 == end.0 {
            points.iter_mut().for_each(|point| point.x = snap(point.x));
        }
        if start.1 == end.1 {
            points.iter_mut().for_each(|point| point.y = snap(point.y));
        }
        self.painter.line_segment(points, Stroke::new(width, color));
    }

    fn path(&self, points: &[(f32, f32)], color: Color32, width: f32, closed: bool) {
        let points = points.iter().map(|&point| self.point(point)).collect();
        let stroke = Stroke::new(self.width(width), color);
        self.painter.add(if closed {
            Shape::closed_line(points, stroke)
        } else {
            Shape::line(points, stroke)
        });
    }

    fn polygon(&self, points: &[(f32, f32)], fill: Color32, outline: Color32) {
        let stroke = if outline == CLEAR {
            Stroke::NONE
        } else {
            Stroke::new(self.width(1.0), outline)
        };
        self.painter.add(Shape::convex_polygon(
            points.iter().map(|&point| self.point(point)).collect(),
            fill,
            stroke,
        ));
    }

    fn rect(&self, min: (f32, f32), max: (f32, f32), fill: Color32, outline: Color32) {
        let rect = Rect::from_min_max(self.point(min), self.point(max));
        if fill != CLEAR {
            self.painter.rect_filled(rect, 0.0, fill);
        }
        if outline != CLEAR {
            self.line(min, (max.0, min.1), outline, 1.0);
            self.line((max.0, min.1), max, outline, 1.0);
            self.line(max, (min.0, max.1), outline, 1.0);
            self.line((min.0, max.1), min, outline, 1.0);
        }
    }

    fn circle(&self, center: (f32, f32), radius: f32, fill: Color32, outline: Color32) {
        self.painter.circle(
            self.point(center),
            radius * self.scale,
            fill,
            if outline == CLEAR {
                Stroke::NONE
            } else {
                Stroke::new(self.width(1.2), outline)
            },
        );
    }

    fn curve(&self, points: [(f32, f32); 4], color: Color32, width: f32) {
        self.painter
            .add(egui::epaint::CubicBezierShape::from_points_stroke(
                points.map(|point| self.point(point)),
                false,
                CLEAR,
                Stroke::new(self.width(width), color),
            ));
    }
}
