//! View and image commands use literal geometry rather than unrelated tool
//! artwork. Keep the silhouettes distinct at both menu and ribbon sizes.

use super::*;

pub(super) fn draw(canvas: &Canvas<'_>, icon: Icon) {
    match icon {
        Icon::ActualSize => actual_size(canvas),
        Icon::FitWindow => fit_window(canvas),
        Icon::Rulers => rulers(canvas),
        Icon::Grid => grid(canvas),
        Icon::StatusBar => status_bar(canvas),
        Icon::Layers => layers(canvas),
        Icon::Thumbnail => thumbnail(canvas),
        Icon::Fullscreen => fullscreen(canvas),
        Icon::Measure => measure(canvas),
        Icon::ResetMeasurement => reset_measurement(canvas),
        Icon::Rotate | Icon::RotateRight => rotate(canvas, false),
        Icon::RotateLeft => rotate(canvas, true),
        Icon::FlipHorizontal => flip(canvas, false),
        Icon::FlipVertical => flip(canvas, true),
        Icon::Adjustments => adjustments(canvas),
        Icon::Grayscale => grayscale(canvas),
        Icon::Invert => invert(canvas),
        Icon::ResetImage => reset_image(canvas),
        Icon::CropImage => crop(canvas, true),
        Icon::CropCanvas => crop(canvas, false),
        _ => unreachable!("only view and image commands use this painter"),
    }
}

fn actual_size(canvas: &Canvas<'_>) {
    // Eleven columns leave breathing room at 16 px. Integer physical pixels
    // keep both ones and the two colon dots legible at fractional UI scales.
    let pixels_per_point = canvas.painter.pixels_per_point();
    let cell = ((canvas.rect.width() * pixels_per_point - 2.0) / 11.0)
        .floor()
        .max(1.0);
    let size = vec2(11.0, 7.0) * cell;
    let origin = (canvas.rect.center() * pixels_per_point - size / 2.0).round();
    let mut mesh = Mesh::default();
    for (row, bits) in [0b010_u8, 0b110, 0b010, 0b010, 0b010, 0b010, 0b111]
        .into_iter()
        .enumerate()
    {
        for column in 0..3 {
            if bits & (1 << (2 - column)) != 0 {
                for offset in [0, 8] {
                    pixel_cell(canvas, &mut mesh, origin, cell, column + offset, row);
                }
            }
        }
    }
    for row in [2, 5] {
        pixel_cell(canvas, &mut mesh, origin, cell, 5, row);
    }
    canvas.painter.add(mesh);
}

fn pixel_cell(
    canvas: &Canvas<'_>,
    mesh: &mut Mesh,
    origin: Pos2,
    size: f32,
    column: usize,
    row: usize,
) {
    let pixels_per_point = canvas.painter.pixels_per_point();
    let min = origin + vec2(column as f32, row as f32) * size;
    // A mesh deliberately avoids rectangle feathering, which can erase a
    // one-pixel block completely at the smallest menu size.
    mesh.add_colored_rect(
        Rect::from_min_size(min / pixels_per_point, Vec2::splat(size / pixels_per_point)),
        INK,
    );
}

fn fit_window(canvas: &Canvas<'_>) {
    canvas.rect((8.0, 8.0), (16.0, 16.0), LIGHT_BLUE, BLUE);
    for (x, y) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
        let point = |a: f32, b: f32| (12.0 + x * a, 12.0 + y * b);
        canvas.line(point(10.0, 10.0), point(6.0, 6.0), INK, 1.5);
        canvas.line(point(10.0, 6.0), point(6.0, 6.0), INK, 1.5);
        canvas.line(point(6.0, 10.0), point(6.0, 6.0), INK, 1.5);
    }
}

fn rulers(canvas: &Canvas<'_>) {
    canvas.rect((3.0, 3.0), (21.0, 8.0), GOLD, WOOD);
    canvas.rect((3.0, 8.0), (8.0, 21.0), GOLD, WOOD);
    for (position, length) in [(7.0, 2.0), (11.0, 3.0), (15.0, 2.0), (19.0, 3.0)] {
        canvas.line((position, 3.0), (position, 3.0 + length), INK, 1.0);
    }
    for (position, length) in [(11.0, 3.0), (15.0, 2.0), (19.0, 3.0)] {
        canvas.line((3.0, position), (3.0 + length, position), INK, 1.0);
    }
}

fn grid(canvas: &Canvas<'_>) {
    canvas.rect((3.0, 3.0), (21.0, 21.0), Color32::WHITE, INK);
    canvas.rect((9.0, 9.0), (15.0, 15.0), LIGHT_BLUE, CLEAR);
    for position in [9.0, 15.0] {
        canvas.line((position, 3.0), (position, 21.0), BLUE, 1.0);
        canvas.line((3.0, position), (21.0, position), BLUE, 1.0);
    }
}

fn status_bar(canvas: &Canvas<'_>) {
    canvas.rect((2.0, 4.0), (22.0, 20.0), Color32::WHITE, INK);
    canvas.rect((2.0, 15.0), (22.0, 20.0), BLUE, BLUE);
    canvas.line((5.0, 17.5), (10.0, 17.5), Color32::WHITE, 1.0);
    canvas.line((15.0, 17.5), (19.0, 17.5), Color32::WHITE, 1.0);
}

fn layers(canvas: &Canvas<'_>) {
    for (top, fill) in [(12.0, SILVER), (7.0, LIGHT_BLUE), (2.0, Color32::WHITE)] {
        canvas.polygon(
            &[
                (12.0, top),
                (22.0, top + 5.0),
                (12.0, top + 10.0),
                (2.0, top + 5.0),
            ],
            fill,
            BLUE,
        );
    }
}

fn thumbnail(canvas: &Canvas<'_>) {
    canvas.rect((2.0, 3.0), (22.0, 20.0), Color32::WHITE, INK);
    canvas.line((2.0, 7.0), (22.0, 7.0), INK, 1.0);
    canvas.rect((11.0, 11.0), (23.0, 22.0), LIGHT_BLUE, BLUE);
    canvas.circle((19.5, 13.5), 1.0, GOLD, CLEAR);
    canvas.polygon(&[(13.0, 20.0), (16.0, 15.0), (21.0, 20.0)], BLUE, CLEAR);
}

fn fullscreen(canvas: &Canvas<'_>) {
    for (x, y) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
        let point = |a: f32, b: f32| (12.0 + x * a, 12.0 + y * b);
        canvas.line(point(3.0, 3.0), point(9.0, 9.0), BLUE, 1.5);
        canvas.line(point(4.0, 9.0), point(9.0, 9.0), INK, 1.5);
        canvas.line(point(9.0, 4.0), point(9.0, 9.0), INK, 1.5);
    }
}

fn measure(canvas: &Canvas<'_>) {
    canvas.line((3.0, 6.0), (21.0, 6.0), BLUE, 1.3);
    canvas.path(&[(6.0, 3.0), (3.0, 6.0), (6.0, 9.0)], BLUE, 1.3, false);
    canvas.path(&[(18.0, 3.0), (21.0, 6.0), (18.0, 9.0)], BLUE, 1.3, false);
    canvas.rect((3.0, 13.0), (21.0, 20.0), GOLD, WOOD);
    for (position, length) in [(6.0, 3.0), (10.0, 4.0), (14.0, 3.0), (18.0, 4.0)] {
        canvas.line((position, 13.0), (position, 13.0 + length), INK, 1.0);
    }
}

fn reset_measurement(canvas: &Canvas<'_>) {
    canvas.rect((2.0, 4.0), (21.0, 10.0), GOLD, WOOD);
    for position in [5.0, 9.0, 13.0, 17.0] {
        canvas.line((position, 4.0), (position, 7.0), INK, 1.0);
    }
    canvas.curve(
        [(19.0, 13.0), (26.0, 24.0), (10.0, 25.0), (10.0, 15.0)],
        BLUE,
        1.8,
    );
    canvas.polygon(&[(6.0, 17.0), (10.0, 12.0), (14.0, 17.0)], BLUE, CLEAR);
}

fn rotate(canvas: &Canvas<'_>, left: bool) {
    let point = |x: f32, y: f32| (if left { 24.0 - x } else { x }, y);
    canvas.rect((8.0, 10.0), (16.0, 18.0), LIGHT_BLUE, BLUE);
    canvas.curve(
        [
            point(5.0, 18.0),
            point(-1.0, 7.0),
            point(6.0, 1.0),
            point(13.0, 3.0),
        ],
        BLUE,
        1.8,
    );
    canvas.curve(
        [
            point(13.0, 3.0),
            point(19.0, 3.0),
            point(22.0, 7.0),
            point(21.0, 12.0),
        ],
        BLUE,
        1.8,
    );
    canvas.polygon(
        &[point(17.0, 10.0), point(21.0, 16.0), point(23.0, 9.0)],
        BLUE,
        CLEAR,
    );
}

fn flip(canvas: &Canvas<'_>, vertical: bool) {
    let point = |x, y| if vertical { (y, x) } else { (x, y) };
    for y in [3.0, 8.0, 13.0, 18.0] {
        canvas.line(point(12.0, y), point(12.0, y + 3.0), INK, 1.0);
    }
    canvas.polygon(
        &[point(3.0, 5.0), point(9.0, 12.0), point(3.0, 19.0)],
        LIGHT_BLUE,
        BLUE,
    );
    canvas.polygon(
        &[point(21.0, 5.0), point(15.0, 12.0), point(21.0, 19.0)],
        BLUE,
        BLUE,
    );
}

fn adjustments(canvas: &Canvas<'_>) {
    for (x, y) in [(5.0, 8.0), (12.0, 16.0), (19.0, 10.0)] {
        canvas.line((x, 3.0), (x, 21.0), INK, 1.3);
        canvas.rect((x - 2.5, y - 2.0), (x + 2.5, y + 2.0), LIGHT_BLUE, BLUE);
    }
}

fn grayscale(canvas: &Canvas<'_>) {
    for (index, shade) in [230, 145, 55].into_iter().enumerate() {
        let left = 3.0 + index as f32 * 6.0;
        canvas.rect(
            (left, 4.0),
            (left + 6.0, 20.0),
            Color32::from_gray(shade),
            CLEAR,
        );
    }
    canvas.rect((3.0, 4.0), (21.0, 20.0), CLEAR, INK);
}

fn invert(canvas: &Canvas<'_>) {
    canvas.rect((3.0, 3.0), (12.0, 21.0), INK, CLEAR);
    canvas.rect((12.0, 3.0), (21.0, 21.0), Color32::WHITE, CLEAR);
    canvas.rect((3.0, 3.0), (21.0, 21.0), CLEAR, INK);
    // Matching arrows exchange the dark and light halves of the swatch.
    canvas.line((6.0, 8.0), (12.0, 8.0), Color32::WHITE, 1.5);
    canvas.line((12.0, 8.0), (18.0, 8.0), INK, 1.5);
    canvas.path(&[(15.0, 5.0), (18.0, 8.0), (15.0, 11.0)], INK, 1.5, false);
    canvas.line((6.0, 16.0), (12.0, 16.0), Color32::WHITE, 1.5);
    canvas.line((12.0, 16.0), (18.0, 16.0), INK, 1.5);
    canvas.path(
        &[(9.0, 13.0), (6.0, 16.0), (9.0, 19.0)],
        Color32::WHITE,
        1.5,
        false,
    );
}

fn reset_image(canvas: &Canvas<'_>) {
    canvas.rect((8.0, 9.0), (18.0, 18.0), LIGHT_BLUE, BLUE);
    canvas.polygon(&[(9.0, 17.0), (12.0, 12.0), (17.0, 17.0)], BLUE, CLEAR);
    canvas.curve(
        [(5.0, 7.0), (16.0, -4.0), (29.0, 12.0), (19.0, 20.0)],
        BLUE,
        1.8,
    );
    canvas.curve(
        [(19.0, 20.0), (14.0, 24.0), (5.0, 21.0), (4.0, 15.0)],
        BLUE,
        1.8,
    );
    canvas.polygon(&[(3.0, 3.0), (3.0, 10.0), (10.0, 9.0)], BLUE, CLEAR);
}

fn crop(canvas: &Canvas<'_>, image: bool) {
    canvas.rect((7.0, 7.0), (17.0, 17.0), LIGHT_BLUE, CLEAR);
    if image {
        canvas.circle((14.5, 9.5), 1.2, GOLD, CLEAR);
        canvas.polygon(&[(8.0, 16.0), (11.5, 10.0), (16.0, 16.0)], BLUE, CLEAR);
    } else {
        canvas.rect((3.0, 3.0), (21.0, 21.0), CLEAR, Color32::from_gray(165));
    }
    canvas.line((7.0, 2.0), (7.0, 17.0), INK, 1.8);
    canvas.line((7.0, 17.0), (22.0, 17.0), INK, 1.8);
    canvas.line((2.0, 7.0), (17.0, 7.0), INK, 1.8);
    canvas.line((17.0, 7.0), (17.0, 22.0), INK, 1.8);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_size_remains_solid_and_pixel_aligned_at_menu_and_ribbon_scales() {
        for pixels_per_point in [1.0, 1.25, 2.0] {
            for side in [16.0, 28.0] {
                let ctx = Context::default();
                ctx.set_pixels_per_point(pixels_per_point);
                let rect = Rect::from_min_size(pos2(10.3, 9.7), Vec2::splat(side));
                let output = ctx.run(
                    RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(100.0))),
                        ..Default::default()
                    },
                    |ctx| {
                        let painter = ctx.layer_painter(LayerId::background());
                        actual_size(&Canvas::new(&painter, rect));
                    },
                );
                let mesh = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        Shape::Mesh(mesh) => Some(mesh),
                        _ => None,
                    })
                    .expect("the digits must use an unfeathered mesh");

                assert!(!mesh.indices.is_empty());
                for vertex in &mesh.vertices {
                    assert_eq!(vertex.color, INK);
                    assert!(rect.contains(vertex.pos));
                    for coordinate in [vertex.pos.x, vertex.pos.y] {
                        let physical = coordinate * ctx.pixels_per_point();
                        assert!((physical - physical.round()).abs() < 0.001);
                    }
                }
            }
        }
    }
}
