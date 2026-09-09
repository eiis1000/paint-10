use super::*;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Picker {
    Paint,
    Hsv,
}

impl Picker {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Paint => "Paint spectrum",
            Self::Hsv => "HSV picker",
        }
    }
}

pub(super) fn visual_picker(ui: &mut Ui, state: &mut Editor, height: f32) {
    let mut values = match state.picker {
        Picker::Paint => state.paint,
        Picker::Hsv => state.hsv,
    };
    let previous = values;
    let size = vec2(ui.available_width() - 24.0, height);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let label = if state.picker == Picker::Paint {
        "Hue and saturation"
    } else {
        "Saturation and value"
    };
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, label));
    if response.clicked() || response.dragged() {
        if let Some(point) = response.interact_pointer_pos() {
            let x = ((point.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64;
            let y = (1.0 - (point.y - rect.top()) / rect.height()).clamp(0.0, 1.0) as f64;
            if state.picker == Picker::Paint {
                values[0] = (x * 239.0).round();
                values[1] = (y * 240.0).round();
            } else {
                values[1] = x * 100.0;
                values[2] = y * 100.0;
            }
        }
    }
    gradient_mesh(ui.painter(), rect, 24, 8, |x, y| {
        let (space, channels) = match state.picker {
            Picker::Paint => (Space::PaintHsl, [x * 239.0, (1.0 - y) * 240.0, 120.0, 0.0]),
            Picker::Hsv => (Space::Hsv, [values[0], x * 100.0, (1.0 - y) * 100.0, 0.0]),
        };
        rgb32(color::from_coordinates(space, channels).rgb)
    });
    let position = match state.picker {
        Picker::Paint => vec2((values[0] / 239.0) as f32, (1.0 - values[1] / 240.0) as f32),
        Picker::Hsv => vec2((values[1] / 100.0) as f32, (1.0 - values[2] / 100.0) as f32),
    };
    let painter = ui.painter().with_clip_rect(rect);
    let position = rect.min + position * rect.size();
    painter.circle_stroke(position, 5.0, Stroke::new(3.0_f32, Color32::WHITE));
    painter.circle_stroke(position, 5.0, Stroke::new(1.0_f32, Color32::BLACK));
    response.on_hover_text(
        "Drag to choose a color, or use the numeric coordinates for keyboard entry.",
    );

    let (strip, response) = ui.allocate_exact_size(vec2(18.0, height), Sense::click_and_drag());
    let paint = state.picker == Picker::Paint;
    response.widget_info(|| {
        WidgetInfo::labeled(
            WidgetType::Other,
            true,
            if paint { "Luminosity" } else { "Picker hue" },
        )
    });
    if response.clicked() || response.dragged() {
        if let Some(point) = response.interact_pointer_pos() {
            let fraction = ((point.y - strip.top()) / strip.height()).clamp(0.0, 1.0) as f64;
            if paint {
                values[2] = ((1.0 - fraction) * 240.0).round();
            } else {
                values[0] = fraction * 360.0;
            }
        }
    }
    gradient_mesh(ui.painter(), strip, 1, 24, |_, y| {
        let (space, channels) = if paint {
            (
                Space::PaintHsl,
                [values[0], values[1], (1.0 - y) * 240.0, 0.0],
            )
        } else {
            (Space::Hsv, [y * 360.0, 100.0, 100.0, 0.0])
        };
        rgb32(color::from_coordinates(space, channels).rgb)
    });
    let fraction = if paint {
        1.0 - values[2] / 240.0
    } else {
        values[0] / 360.0
    };
    let y = strip.top() + fraction as f32 * strip.height();
    marker(ui.painter(), strip, y);
    if values != previous {
        state.apply_coordinates(if paint { Space::PaintHsl } else { Space::Hsv }, values);
    }
}

fn rgb32([r, g, b]: [u8; 3]) -> Color32 {
    Color32::from_rgb(r, g, b)
}

pub(super) fn gradient_mesh(
    painter: &Painter,
    rect: Rect,
    columns: u32,
    rows: u32,
    color_at: impl Fn(f64, f64) -> Color32,
) {
    let mut mesh = Mesh::default();
    for row in 0..=rows {
        for column in 0..=columns {
            let x = column as f64 / columns as f64;
            let y = row as f64 / rows as f64;
            mesh.colored_vertex(
                rect.min + vec2(x as f32, y as f32) * rect.size(),
                color_at(x, y),
            );
            if row < rows && column < columns {
                let index = row * (columns + 1) + column;
                mesh.add_triangle(index, index + 1, index + columns + 1);
                mesh.add_triangle(index + 1, index + columns + 2, index + columns + 1);
            }
        }
    }
    painter.add(mesh);
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0_f32, Color32::from_gray(140)),
        StrokeKind::Inside,
    );
}

fn marker(painter: &Painter, rect: Rect, y: f32) {
    let marker = Rect::from_center_size(pos2(rect.center().x, y), vec2(rect.width() + 2.0, 3.0));
    painter.rect_stroke(
        marker,
        0.0,
        Stroke::new(2.0_f32, Color32::WHITE),
        StrokeKind::Middle,
    );
    painter.rect_stroke(
        marker,
        0.0,
        Stroke::new(1.0_f32, Color32::BLACK),
        StrokeKind::Middle,
    );
}
