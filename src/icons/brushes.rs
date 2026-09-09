use super::*;

pub(super) fn draw(canvas: &Canvas<'_>, brush: Brush) {
    match brush {
        Brush::Round | Brush::Oil | Brush::Watercolor => paintbrush(canvas, brush),
        Brush::Calligraphy | Brush::Calligraphy2 => pen(canvas, brush),
        Brush::Airbrush => airbrush(canvas),
        Brush::Marker => {
            canvas.polygon(
                &[(5.0, 15.0), (16.0, 2.0), (22.0, 7.0), (11.0, 20.0)],
                Color32::from_rgb(58, 137, 205),
                INK,
            );
            canvas.line((9.0, 14.0), (17.0, 5.0), LIGHT_BLUE, 1.2);
            canvas.polygon(
                &[(4.0, 17.0), (7.0, 13.0), (12.0, 18.0), (8.0, 21.0)],
                SILVER,
                INK,
            );
            canvas.polygon(&[(4.0, 17.0), (8.0, 21.0), (2.0, 23.0)], INK, CLEAR);
        }
        Brush::Crayon => {
            let red = Color32::from_rgb(204, 69, 66);
            canvas.polygon(
                &[(4.0, 17.0), (16.0, 3.0), (22.0, 8.0), (10.0, 22.0)],
                red,
                INK,
            );
            canvas.polygon(
                &[(8.0, 12.0), (13.0, 6.0), (19.0, 11.0), (14.0, 17.0)],
                Color32::from_rgb(247, 177, 151),
                INK,
            );
            canvas.polygon(&[(4.0, 17.0), (10.0, 22.0), (2.0, 23.0)], red, INK);
            canvas.line((10.0, 11.0), (15.0, 15.0), red, 1.0);
        }
        Brush::Pencil => {
            canvas.polygon(
                &[(4.0, 17.0), (17.0, 2.0), (22.0, 6.0), (9.0, 21.0)],
                Color32::from_rgb(205, 158, 91),
                INK,
            );
            canvas.line(
                (7.0, 17.0),
                (18.0, 5.0),
                Color32::from_rgb(245, 214, 165),
                1.0,
            );
            canvas.polygon(&[(4.0, 17.0), (9.0, 21.0), (2.0, 23.0)], SILVER, INK);
            canvas.polygon(&[(2.0, 23.0), (4.0, 18.0), (8.0, 21.0)], INK, CLEAR);
        }
    }
}

fn paintbrush(canvas: &Canvas<'_>, brush: Brush) {
    let handle = if brush == Brush::Watercolor {
        Color32::from_rgb(49, 153, 155)
    } else {
        WOOD
    };
    canvas.polygon(
        &[(9.0, 13.0), (18.0, 2.0), (22.0, 5.0), (14.0, 17.0)],
        handle,
        INK,
    );
    canvas.line(
        (13.0, 12.0),
        (19.0, 4.0),
        crate::display::color([255, 255, 255, 120]),
        1.0,
    );
    canvas.polygon(
        &[(8.0, 12.0), (15.0, 17.0), (12.0, 20.0), (5.0, 15.0)],
        SILVER,
        INK,
    );
    if brush == Brush::Watercolor {
        canvas.polygon(
            &[(5.0, 15.0), (12.0, 20.0), (6.0, 22.0), (1.0, 23.0)],
            Color32::from_rgb(100, 66, 45),
            INK,
        );
        canvas.line((4.0, 21.0), (8.0, 19.0), GOLD, 1.0);
    } else {
        let bristle = if brush == Brush::Oil {
            Color32::from_rgb(244, 220, 166)
        } else {
            GOLD
        };
        canvas.polygon(
            &[(5.0, 15.0), (12.0, 20.0), (9.0, 23.0), (1.0, 21.0)],
            bristle,
            INK,
        );
        canvas.line((4.0, 20.0), (6.0, 17.0), WOOD, 1.0);
        canvas.line((7.0, 21.0), (9.0, 19.0), WOOD, 1.0);
        if brush == Brush::Oil {
            canvas.line((1.0, 22.0), (8.0, 24.0), BLUE, 1.7);
        }
    }
}

fn pen(canvas: &Canvas<'_>, brush: Brush) {
    canvas.polygon(
        &[(8.0, 14.0), (18.0, 2.0), (22.0, 6.0), (12.0, 18.0)],
        Color32::from_rgb(45, 63, 78),
        INK,
    );
    canvas.line(
        (12.0, 12.0),
        (19.0, 4.0),
        Color32::from_rgb(125, 151, 170),
        1.0,
    );
    let nib = if brush == Brush::Calligraphy {
        [(8.0, 13.0), (13.0, 18.0), (6.0, 23.0), (1.0, 19.0)]
    } else {
        [(8.0, 13.0), (13.0, 18.0), (4.0, 22.0), (4.0, 16.0)]
    };
    canvas.polygon(&nib, GOLD, INK);
    canvas.line((5.0, 20.0), (9.0, 16.0), INK, 1.0);
    canvas.circle((9.0, 16.0), 0.8, INK, CLEAR);
}

fn airbrush(canvas: &Canvas<'_>) {
    canvas.rect((4.0, 10.0), (16.0, 23.0), LIGHT_BLUE, INK);
    canvas.rect((6.0, 6.0), (14.0, 10.0), SILVER, INK);
    canvas.rect((8.0, 3.0), (14.0, 6.0), INK, CLEAR);
    canvas.line((6.0, 13.0), (6.0, 21.0), Color32::WHITE, 1.0);
    for point in [
        (17.0, 4.0),
        (20.0, 2.0),
        (21.0, 6.0),
        (23.0, 3.5),
        (23.0, 8.0),
    ] {
        canvas.circle(point, 0.8, BLUE, CLEAR);
    }
}

pub(super) fn preview(painter: &Painter, rect: Rect, brush: Brush) {
    if !rect.is_positive() {
        return;
    }
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let point = |t: f32, offset: f32| {
        pos2(
            egui::lerp(rect.left() + 4.0..=rect.right() - 4.0, t),
            rect.center().y + (t * std::f32::consts::TAU).sin() * rect.height() * 0.15 + offset,
        )
    };
    let path: Vec<_> = (0..40)
        .map(|index| point(index as f32 / 39.0, 0.0))
        .collect();
    let width = rect.height().clamp(6.0, 22.0) * 0.25;
    match brush {
        Brush::Round => {
            painter.add(Shape::line(path, Stroke::new(width, BLUE)));
        }
        Brush::Calligraphy | Brush::Calligraphy2 => {
            let slope = if brush == Brush::Calligraphy {
                1.0
            } else {
                -1.0
            };
            let mut mesh = epaint::Mesh::default();
            let nib = vec2(width, width * slope);
            let feather = nib.normalized() * 0.75;
            for point in &path {
                for (position, color) in [
                    (*point - nib - feather, CLEAR),
                    (*point - nib, BLUE),
                    (*point + nib, BLUE),
                    (*point + nib + feather, CLEAR),
                ] {
                    mesh.vertices.push(epaint::Vertex {
                        pos: position,
                        uv: epaint::WHITE_UV,
                        color,
                    });
                }
            }
            for index in 0..path.len() as u32 - 1 {
                for strip in 0..3 {
                    let base = index * 4 + strip;
                    mesh.indices.extend_from_slice(&[
                        base,
                        base + 1,
                        base + 4,
                        base + 4,
                        base + 1,
                        base + 5,
                    ]);
                }
            }
            painter.add(Shape::mesh(mesh));
        }
        Brush::Airbrush | Brush::Crayon | Brush::Pencil => {
            if brush == Brush::Crayon {
                painter.add(Shape::line(
                    path.clone(),
                    Stroke::new(width * 1.6, BLUE.gamma_multiply(0.25)),
                ));
            } else if brush == Brush::Pencil {
                painter.add(Shape::line(
                    path.clone(),
                    Stroke::new(0.7_f32, BLUE.gamma_multiply(0.7)),
                ));
            }
            let count = if brush == Brush::Crayon { 300 } else { 140 };
            let spread = if brush == Brush::Pencil { 0.6 } else { width };
            // Fixed random coordinates avoid flicker and diagonal stipple bands.
            let mut seed = 0x6d2b_79f5_u32;
            for _ in 0..count {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let t = (seed & 0xffff) as f32 / 65535.0;
                let noise = (seed >> 16) as f32 / 65535.0 - 0.5;
                let center = point(t, noise * spread * 2.0);
                painter.circle_filled(center, 0.65, BLUE.gamma_multiply(0.7));
            }
        }
        Brush::Oil => {
            painter.add(Shape::line(path, Stroke::new(width * 1.5, BLUE)));
            for offset in [-1.5, 0.0, 1.5] {
                let points = (0..40)
                    .map(|index| point(index as f32 / 39.0, offset))
                    .collect();
                painter.add(Shape::line(
                    points,
                    Stroke::new(0.6_f32, LIGHT_BLUE.gamma_multiply(0.5)),
                ));
            }
        }
        Brush::Marker | Brush::Watercolor => {
            let color = BLUE.gamma_multiply(if brush == Brush::Marker { 0.6 } else { 0.3 });
            painter.add(Shape::line(path, Stroke::new(width * 1.8, color)));
            if brush == Brush::Watercolor {
                let points = (0..40)
                    .map(|index| point(index as f32 / 39.0, -1.0))
                    .collect();
                painter.add(Shape::line(points, Stroke::new(width, color)));
            }
        }
    }
}
