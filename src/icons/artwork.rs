use super::*;
use crate::document::shape_points;

pub(super) fn tool(canvas: &Canvas<'_>, tool: Tool) {
    if tool.is_shape() {
        shape(canvas, tool);
        return;
    }
    match tool {
        Tool::Pencil => pencil(canvas),
        Tool::Brush => brushes::draw(canvas, Brush::Round),
        Tool::Fill => bucket(canvas),
        Tool::Text => {
            // Paint's serif A is artwork, not a font-dependent text glyph.
            canvas.line((5.0, 20.0), (11.5, 3.0), INK, 1.7);
            canvas.line((11.5, 3.0), (18.5, 20.0), INK, 3.0);
            canvas.line((8.0, 14.0), (15.5, 14.0), INK, 1.5);
            canvas.line((2.5, 21.0), (8.0, 21.0), INK, 1.5);
            canvas.line((15.0, 21.0), (22.0, 21.0), INK, 1.5);
        }
        Tool::Eraser => {
            canvas.polygon(
                &[(2.0, 15.0), (13.0, 3.0), (22.0, 11.0), (11.0, 23.0)],
                Color32::from_rgb(239, 153, 177),
                INK,
            );
            canvas.polygon(
                &[(2.0, 15.0), (7.0, 9.5), (16.0, 17.5), (11.0, 23.0)],
                Color32::from_rgb(255, 219, 230),
                INK,
            );
            canvas.line(
                (14.0, 5.0),
                (20.0, 10.5),
                Color32::from_rgb(255, 200, 216),
                1.0,
            );
        }
        Tool::Picker => {
            canvas.polygon(
                &[(3.0, 17.0), (13.0, 7.0), (17.0, 11.0), (7.0, 21.0)],
                SILVER,
                INK,
            );
            canvas.line((3.0, 21.0), (7.0, 17.0), BLUE, 2.0);
            canvas.line((8.0, 15.0), (12.0, 11.0), Color32::WHITE, 1.0);
            canvas.polygon(
                &[(13.0, 5.0), (17.0, 1.0), (23.0, 7.0), (19.0, 11.0)],
                INK,
                INK,
            );
            canvas.line((11.0, 5.0), (19.0, 13.0), INK, 3.0);
        }
        Tool::Magnifier => {
            canvas.line((15.0, 15.0), (22.0, 22.0), INK, 4.0);
            canvas.circle((9.5, 9.5), 7.0, LIGHT_BLUE, BLUE);
            canvas.curve(
                [(4.5, 10.0), (4.5, 5.0), (7.0, 4.0), (10.0, 4.0)],
                Color32::WHITE,
                1.2,
            );
        }
        Tool::Select => {
            for step in (2..21).step_by(4) {
                let start = step as f32;
                let end = (start + 2.0).min(22.0);
                canvas.line((start, 2.0), (end, 2.0), INK, 1.0);
                canvas.line((start, 22.0), (end, 22.0), INK, 1.0);
                canvas.line((2.0, start), (2.0, end), INK, 1.0);
                canvas.line((22.0, start), (22.0, end), INK, 1.0);
            }
        }
        _ => unreachable!("all shape tools are handled above"),
    }
}

fn shape(canvas: &Canvas<'_>, tool: Tool) {
    match tool {
        Tool::Line => canvas.line((2.0, 21.0), (22.0, 3.0), BLUE, 1.4),
        Tool::Curve => canvas.curve(
            [(2.0, 18.0), (3.0, -2.0), (21.0, 27.0), (22.0, 5.0)],
            BLUE,
            1.4,
        ),
        Tool::Rectangle => canvas.rect((2.0, 4.0), (22.0, 20.0), CLEAR, BLUE),
        Tool::RoundedRect => {
            canvas.painter.rect_stroke(
                Rect::from_min_max(canvas.point((2.0, 4.0)), canvas.point((22.0, 20.0))),
                (4.0 * canvas.scale).round() as u8,
                Stroke::new(canvas.width(1.3), BLUE),
                StrokeKind::Middle,
            );
        }
        Tool::Heart => {
            // Separate Bezier segments avoid long miter spikes at the cleft
            // and point when this symbol is only fourteen points high.
            for points in [
                [(12.0, 7.0), (8.0, -1.0), (1.0, 1.0), (2.0, 8.0)],
                [(2.0, 8.0), (3.0, 13.0), (10.0, 19.0), (12.0, 22.0)],
                [(12.0, 22.0), (14.0, 19.0), (21.0, 13.0), (22.0, 8.0)],
                [(22.0, 8.0), (23.0, 1.0), (16.0, -1.0), (12.0, 7.0)],
            ] {
                canvas.curve(points, BLUE, 1.3);
            }
        }
        Tool::OvalCallout => {
            canvas.curve(
                [(7.0, 16.0), (0.0, 14.0), (0.0, 3.0), (12.0, 3.0)],
                BLUE,
                1.3,
            );
            canvas.curve(
                [(12.0, 3.0), (25.0, 3.0), (25.0, 17.0), (11.0, 17.0)],
                BLUE,
                1.3,
            );
            canvas.line((11.0, 17.0), (6.0, 22.0), BLUE, 1.3);
            canvas.line((6.0, 22.0), (7.0, 16.0), BLUE, 1.3);
        }
        Tool::CloudCallout => {
            for points in [
                [(4.0, 16.0), (-1.0, 16.0), (0.0, 9.0), (3.0, 8.0)],
                [(3.0, 8.0), (0.0, 3.0), (8.0, 1.0), (10.0, 4.0)],
                [(10.0, 4.0), (12.0, -1.0), (19.0, 0.0), (19.0, 5.0)],
                [(19.0, 5.0), (25.0, 4.0), (25.0, 11.0), (21.0, 12.0)],
                [(21.0, 12.0), (25.0, 16.0), (19.0, 20.0), (16.0, 17.0)],
                [(16.0, 17.0), (13.0, 21.0), (9.0, 19.0), (9.0, 17.0)],
            ] {
                canvas.curve(points, BLUE, 1.3);
            }
            canvas.line((9.0, 17.0), (4.0, 22.0), BLUE, 1.3);
            canvas.line((4.0, 22.0), (4.0, 16.0), BLUE, 1.3);
        }
        _ => {
            let points: Vec<_> = shape_points(tool)
                .into_iter()
                .map(|(x, y)| {
                    let (top, height) = if tool == Tool::Oval {
                        (4.0, 16.0)
                    } else {
                        (2.0, 20.0)
                    };
                    (2.0 + x * 20.0, top + y * height)
                })
                .collect();
            canvas.path(&points, BLUE, 1.3, true);
        }
    }
}

fn pencil(canvas: &Canvas<'_>) {
    canvas.polygon(
        &[(4.0, 16.0), (16.0, 3.0), (21.0, 8.0), (9.0, 21.0)],
        GOLD,
        INK,
    );
    canvas.line(
        (7.0, 16.0),
        (16.0, 6.0),
        Color32::from_rgb(255, 227, 131),
        1.0,
    );
    canvas.polygon(
        &[(4.0, 16.0), (9.0, 21.0), (2.0, 23.0)],
        Color32::from_rgb(243, 220, 178),
        INK,
    );
    canvas.polygon(&[(2.0, 23.0), (3.0, 19.5), (5.5, 22.0)], INK, CLEAR);
    canvas.polygon(
        &[(15.0, 4.0), (17.0, 2.0), (22.0, 7.0), (20.0, 9.0)],
        Color32::from_rgb(238, 158, 159),
        INK,
    );
}

fn bucket(canvas: &Canvas<'_>) {
    canvas.curve(
        [(5.0, 10.0), (3.0, 0.0), (12.0, -1.0), (14.0, 6.0)],
        INK,
        1.2,
    );
    canvas.polygon(
        &[(2.0, 11.0), (10.0, 3.0), (20.0, 13.0), (12.0, 21.0)],
        SILVER,
        INK,
    );
    canvas.polygon(&[(3.0, 12.0), (18.0, 12.0), (12.0, 19.0)], BLUE, CLEAR);
    canvas.line((4.0, 10.0), (11.0, 4.0), Color32::WHITE, 1.2);
    canvas.polygon(&[(21.0, 14.0), (18.0, 20.0), (23.0, 20.0)], BLUE, CLEAR);
    canvas.circle((20.5, 20.0), 2.2, BLUE, CLEAR);
}

pub(super) fn command(canvas: &Canvas<'_>, icon: Icon) {
    match icon {
        Icon::New => document(canvas),
        Icon::Open => {
            canvas.rect((2.0, 5.0), (10.0, 12.0), GOLD, WOOD);
            canvas.rect((2.0, 8.0), (21.0, 21.0), GOLD, WOOD);
            canvas.polygon(
                &[(5.0, 11.0), (23.0, 11.0), (19.0, 21.0), (1.0, 21.0)],
                Color32::from_rgb(255, 219, 115),
                WOOD,
            );
        }
        Icon::Print => {
            canvas.rect((6.0, 1.0), (18.0, 11.0), Color32::WHITE, INK);
            canvas.rect((2.0, 8.0), (22.0, 18.0), SILVER, INK);
            canvas.line((4.0, 11.0), (20.0, 11.0), Color32::WHITE, 1.0);
            canvas.rect((6.0, 15.0), (18.0, 23.0), Color32::WHITE, INK);
            canvas.line((9.0, 18.0), (15.0, 18.0), BLUE, 1.0);
            canvas.line((9.0, 20.0), (15.0, 20.0), BLUE, 1.0);
            canvas.circle((19.0, 13.0), 0.8, Color32::from_rgb(69, 152, 70), CLEAR);
        }
        Icon::PrintPreview => {
            document(canvas);
            canvas.line((16.0, 17.0), (22.0, 23.0), INK, 3.0);
            canvas.circle((13.0, 14.0), 5.5, LIGHT_BLUE, BLUE);
            canvas.line((10.0, 12.0), (14.0, 12.0), Color32::WHITE, 1.0);
        }
        Icon::Email => {
            canvas.rect((1.0, 5.0), (23.0, 20.0), Color32::WHITE, BLUE);
            canvas.polygon(&[(1.0, 20.0), (12.0, 10.0), (23.0, 20.0)], LIGHT_BLUE, BLUE);
            canvas.polygon(&[(1.0, 5.0), (23.0, 5.0), (12.0, 15.0)], SILVER, BLUE);
        }
        Icon::Scanner => {
            canvas.polygon(
                &[(3.0, 13.0), (7.0, 3.0), (21.0, 5.0), (21.0, 14.0)],
                SILVER,
                INK,
            );
            canvas.polygon(
                &[(6.0, 12.0), (9.0, 6.0), (18.0, 7.0), (19.0, 13.0)],
                Color32::WHITE,
                CLEAR,
            );
            canvas.rect((2.0, 14.0), (22.0, 21.0), SILVER, INK);
            canvas.line((4.0, 16.0), (20.0, 16.0), LIGHT_BLUE, 1.5);
            canvas.line((4.0, 19.0), (14.0, 19.0), INK, 1.0);
            canvas.circle((19.0, 19.0), 0.9, BLUE, CLEAR);
        }
        Icon::Wallpaper => {
            canvas.rect((1.0, 3.0), (23.0, 18.0), SILVER, INK);
            canvas.rect((3.0, 5.0), (21.0, 16.0), LIGHT_BLUE, CLEAR);
            canvas.circle((17.0, 8.0), 2.0, GOLD, CLEAR);
            canvas.polygon(&[(3.0, 16.0), (9.0, 8.0), (15.0, 16.0)], BLUE, CLEAR);
            canvas.polygon(
                &[(10.0, 16.0), (16.0, 11.0), (21.0, 16.0)],
                Color32::from_rgb(67, 145, 122),
                CLEAR,
            );
            canvas.line((12.0, 19.0), (12.0, 22.0), INK, 2.0);
            canvas.line((7.0, 22.0), (17.0, 22.0), INK, 1.5);
        }
        Icon::Properties => {
            document(canvas);
            for (y, thumb) in [(11.0, 10.0), (15.0, 15.0), (19.0, 11.0)] {
                canvas.line((7.0, y), (17.0, y), BLUE, 1.0);
                canvas.rect((thumb - 1.0, y - 1.5), (thumb + 1.0, y + 1.5), GOLD, INK);
            }
        }
        Icon::About => {
            canvas.circle((12.0, 12.0), 10.0, BLUE, INK);
            canvas.circle((12.0, 6.5), 1.3, Color32::WHITE, CLEAR);
            canvas.line((12.0, 10.0), (12.0, 18.0), Color32::WHITE, 2.0);
            canvas.line((9.0, 18.0), (15.0, 18.0), Color32::WHITE, 1.5);
            canvas.line((9.5, 10.0), (12.0, 10.0), Color32::WHITE, 1.5);
        }
        Icon::Exit => {
            canvas.rect((3.0, 2.0), (15.0, 22.0), SILVER, INK);
            canvas.polygon(
                &[(4.0, 3.0), (11.0, 6.0), (11.0, 20.0), (4.0, 22.0)],
                GOLD,
                WOOD,
            );
            canvas.circle((9.0, 13.0), 0.9, WOOD, CLEAR);
            canvas.line((14.0, 12.0), (22.0, 12.0), BLUE, 2.0);
            canvas.path(&[(18.0, 8.0), (22.0, 12.0), (18.0, 16.0)], BLUE, 2.0, false);
        }
        Icon::Save => {
            canvas.polygon(
                &[
                    (3.0, 2.0),
                    (19.0, 2.0),
                    (22.0, 5.0),
                    (22.0, 22.0),
                    (3.0, 22.0),
                ],
                BLUE,
                INK,
            );
            canvas.rect((6.0, 2.0), (17.0, 9.0), SILVER, INK);
            canvas.rect((13.0, 3.0), (16.0, 8.0), INK, CLEAR);
            canvas.rect((6.0, 13.0), (19.0, 22.0), Color32::WHITE, INK);
            canvas.line((8.0, 16.0), (17.0, 16.0), LIGHT_BLUE, 1.0);
            canvas.line((8.0, 19.0), (17.0, 19.0), LIGHT_BLUE, 1.0);
        }
        Icon::Png | Icon::Jpeg | Icon::Bitmap | Icon::Gif | Icon::OtherFormats => {
            picture_format(canvas, icon);
        }
        Icon::Undo | Icon::Redo => {
            let flip = |x| {
                if matches!(icon, Icon::Redo) {
                    24.0 - x
                } else {
                    x
                }
            };
            canvas.curve(
                [
                    (flip(6.0), 8.0),
                    (flip(24.0), 3.0),
                    (flip(24.0), 23.0),
                    (flip(10.0), 20.0),
                ],
                BLUE,
                2.5,
            );
            canvas.polygon(
                &[(flip(2.0), 9.0), (flip(9.0), 2.0), (flip(9.0), 14.0)],
                BLUE,
                CLEAR,
            );
        }
        Icon::Paste => {
            canvas.rect((3.0, 4.0), (18.0, 22.0), GOLD, INK);
            canvas.rect(
                (5.0, 6.0),
                (16.0, 20.0),
                Color32::from_rgb(255, 243, 212),
                CLEAR,
            );
            canvas.rect((7.0, 2.0), (14.0, 6.0), SILVER, INK);
            canvas.rect((10.0, 10.0), (22.0, 23.0), Color32::WHITE, BLUE);
            for y in [13.0, 16.0, 19.0] {
                canvas.line((12.0, y), (20.0, y), BLUE, 1.0);
            }
        }
        Icon::Copy => {
            canvas.rect((2.0, 2.0), (15.0, 18.0), Color32::WHITE, BLUE);
            canvas.rect((8.0, 7.0), (22.0, 23.0), Color32::WHITE, BLUE);
            for y in [11.0, 15.0, 19.0] {
                canvas.line((11.0, y), (19.0, y), LIGHT_BLUE, 1.0);
            }
        }
        Icon::Cut => {
            canvas.line((7.0, 17.0), (19.0, 2.0), INK, 1.7);
            canvas.line((17.0, 17.0), (5.0, 2.0), INK, 1.7);
            canvas.circle((6.0, 19.0), 3.5, CLEAR, BLUE);
            canvas.circle((18.0, 19.0), 3.5, CLEAR, BLUE);
            canvas.circle((12.0, 11.0), 1.2, SILVER, INK);
        }
        Icon::Crop => {
            canvas.line((3.0, 22.0), (21.0, 3.0), LIGHT_BLUE, 1.0);
            canvas.path(&[(7.0, 2.0), (7.0, 17.0), (23.0, 17.0)], INK, 2.2, false);
            canvas.path(&[(2.0, 7.0), (17.0, 7.0), (17.0, 23.0)], INK, 2.2, false);
        }
        Icon::Resize => {
            canvas.rect((2.0, 3.0), (22.0, 21.0), CLEAR, INK);
            canvas.rect((2.0, 12.0), (12.0, 21.0), LIGHT_BLUE, BLUE);
            canvas.line((12.0, 12.0), (20.0, 5.0), BLUE, 1.7);
            canvas.path(&[(14.0, 5.0), (20.0, 5.0), (20.0, 11.0)], BLUE, 1.7, false);
        }
        Icon::Rotate
        | Icon::RotateLeft
        | Icon::RotateRight
        | Icon::FlipHorizontal
        | Icon::FlipVertical
        | Icon::Adjustments
        | Icon::Grayscale
        | Icon::Invert
        | Icon::ResetImage
        | Icon::CropImage
        | Icon::CropCanvas
        | Icon::ActualSize
        | Icon::FitWindow
        | Icon::Rulers
        | Icon::Grid
        | Icon::StatusBar
        | Icon::Layers
        | Icon::Thumbnail
        | Icon::Fullscreen
        | Icon::Measure
        | Icon::ResetMeasurement => view_image::draw(canvas, icon),
        Icon::Outline => {
            canvas.rect((3.0, 4.0), (20.0, 19.0), CLEAR, BLUE);
            canvas.line((3.0, 21.0), (22.0, 21.0), INK, 2.7);
            canvas.line((13.0, 12.0), (22.0, 2.0), GOLD, 3.0);
            canvas.line((11.0, 14.0), (14.0, 11.0), INK, 1.2);
        }
        Icon::Fill => bucket(canvas),
        Icon::Colors => colors(canvas),
        Icon::ZoomIn | Icon::ZoomOut => {
            canvas.circle((9.0, 9.0), 6.5, LIGHT_BLUE, BLUE);
            canvas.line((14.0, 14.0), (22.0, 22.0), INK, 3.0);
            canvas.line((5.5, 9.0), (12.5, 9.0), INK, 1.5);
            if matches!(icon, Icon::ZoomIn) {
                canvas.line((9.0, 5.5), (9.0, 12.5), INK, 1.5);
            }
        }
        Icon::ChevronDown => canvas.path(&[(5.0, 8.0), (12.0, 15.0), (19.0, 8.0)], INK, 1.8, false),
        Icon::ChevronUp => canvas.path(&[(5.0, 16.0), (12.0, 9.0), (19.0, 16.0)], INK, 1.8, false),
        Icon::Tool(_) | Icon::Brush(_) => {
            unreachable!("tool and brush artwork is dispatched separately")
        }
    }
}

/// Small original picture thumbnails distinguish the file-format rows without
/// relying on installed fonts or introducing another raster asset system.
fn picture_format(canvas: &Canvas<'_>, icon: Icon) {
    if matches!(icon, Icon::OtherFormats) {
        canvas.rect((1.0, 1.0), (19.0, 18.0), SILVER, INK);
        canvas.rect((4.0, 4.0), (22.0, 21.0), Color32::WHITE, INK);
        canvas.rect((6.0, 6.0), (20.0, 17.0), LIGHT_BLUE, CLEAR);
        canvas.polygon(&[(6.0, 17.0), (12.0, 9.0), (20.0, 17.0)], BLUE, CLEAR);
        for x in [10.0, 14.0, 18.0] {
            canvas.circle((x, 20.0), 0.6, INK, CLEAR);
        }
        return;
    }

    canvas.rect((1.0, 3.0), (23.0, 22.0), Color32::WHITE, INK);
    match icon {
        Icon::Png => {
            for row in 0..4 {
                for column in 0..5 {
                    let x = 2.0 + column as f32 * 4.0;
                    let y = 4.0 + row as f32 * 4.0;
                    let shade = if (row + column) % 2 == 0 { 235 } else { 255 };
                    canvas.rect((x, y), (x + 4.0, y + 4.0), Color32::from_gray(shade), CLEAR);
                }
            }
            canvas.line(
                (12.0, 11.0),
                (12.0, 20.0),
                Color32::from_rgb(54, 137, 73),
                1.5,
            );
            for center in [(9.0, 8.0), (14.0, 7.0), (16.0, 12.0), (10.0, 13.0)] {
                canvas.circle(center, 3.2, Color32::from_rgb(226, 72, 78), CLEAR);
            }
            canvas.circle((12.5, 10.0), 2.1, GOLD, CLEAR);
        }
        Icon::Jpeg => {
            for row in 0..8 {
                let color = Color32::from_rgb(112 + row * 12, 173 + row * 7, 231 + row * 2);
                let y = 4.0 + f32::from(row) * 2.0;
                canvas.rect((2.0, y), (22.0, y + 2.0), color, CLEAR);
            }
            canvas.circle((17.0, 8.0), 2.4, Color32::from_rgb(255, 224, 132), CLEAR);
            canvas.polygon(
                &[(2.0, 20.0), (8.0, 9.0), (17.0, 20.0)],
                Color32::from_rgb(75, 139, 120),
                CLEAR,
            );
            canvas.polygon(
                &[(8.0, 20.0), (15.0, 12.0), (22.0, 17.0), (22.0, 20.0)],
                Color32::from_rgb(43, 105, 86),
                CLEAR,
            );
        }
        Icon::Bitmap => {
            let colors = [BLUE, LIGHT_BLUE, Color32::from_rgb(68, 153, 101), GOLD];
            for row in 0..4 {
                for column in 0..5 {
                    let color = colors[match (row, column) {
                        (0, 3) => 3,
                        (0..=1, _) => 1,
                        (_, 0..=1) => 0,
                        _ => 2,
                    }];
                    let x = 2.0 + column as f32 * 4.0;
                    let y = 4.0 + row as f32 * 4.0;
                    canvas.rect((x, y), (x + 3.5, y + 3.5), color, CLEAR);
                }
            }
        }
        Icon::Gif => {
            canvas.rect(
                (2.0, 4.0),
                (22.0, 20.0),
                Color32::from_rgb(255, 241, 160),
                CLEAR,
            );
            canvas.line(
                (12.0, 11.0),
                (12.0, 20.0),
                Color32::from_rgb(45, 141, 88),
                1.8,
            );
            for center in [(8.0, 8.0), (15.0, 8.0), (8.0, 14.0), (15.0, 14.0)] {
                canvas.circle(center, 3.3, BLUE, CLEAR);
            }
            canvas.circle((11.5, 11.0), 2.2, GOLD, CLEAR);
        }
        _ => unreachable!("only picture-format icons use this painter"),
    }
}

fn document(canvas: &Canvas<'_>) {
    canvas.polygon(
        &[
            (4.0, 1.0),
            (14.0, 1.0),
            (20.0, 7.0),
            (20.0, 23.0),
            (4.0, 23.0),
        ],
        Color32::WHITE,
        INK,
    );
    canvas.polygon(&[(14.0, 1.0), (14.0, 7.0), (20.0, 7.0)], SILVER, INK);
}

fn colors(canvas: &Canvas<'_>) {
    let colors = [
        [238, 64, 64],
        [248, 167, 48],
        [242, 223, 39],
        [72, 184, 76],
        [36, 167, 222],
        [135, 82, 199],
    ];
    for row in 0..4 {
        for (column, rgb) in colors.into_iter().enumerate() {
            let shade = |channel: u8| match row {
                0 => channel.saturating_add((255 - channel) / 2),
                1 => channel,
                2 => (channel as f32 * 0.8) as u8,
                _ => (channel as f32 * 0.55) as u8,
            };
            canvas.rect(
                (2.0 + column as f32 * 3.4, 2.0 + row as f32 * 5.0),
                (
                    2.0 + (column + 1) as f32 * 3.4,
                    2.0 + (row + 1) as f32 * 5.0,
                ),
                Color32::from_rgb(shade(rgb[0]), shade(rgb[1]), shade(rgb[2])),
                CLEAR,
            );
        }
    }
}
