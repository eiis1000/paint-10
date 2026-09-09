//! Continuous pigment textures for shape fills and outlines.
//!
//! Sample in canvas coordinates so clipping and redraws cannot change the
//! texture. Interpolating between noise samples avoids visible rectangular
//! cells; fine paper grain is deliberately much weaker than the broad wash.

use super::{noise_at, Color};

fn smooth_noise(x: f32, y: f32, seed: u32) -> f32 {
    let left = x.floor() as i32;
    let top = y.floor() as i32;
    let ease = |t: f32| t * t * (3.0 - 2.0 * t);
    let u = ease(x - left as f32);
    let v = ease(y - top as f32);
    let sample = |dx, dy| (noise_at(left + dx, top + dy, seed) & 0xffff) as f32 / 65535.0;
    let upper = sample(0, 0) * (1.0 - u) + sample(1, 0) * u;
    let lower = sample(0, 1) * (1.0 - u) + sample(1, 1) * u;
    upper * (1.0 - v) + lower * v
}

pub(super) fn watercolor(x: i32, y: i32) -> u8 {
    let broad = smooth_noise(x as f32 / 37.0, y as f32 / 37.0, 2);
    let pigment = smooth_noise(x as f32 / 11.0, y as f32 / 11.0, 17);
    let paper = (noise_at(x, y, 31) & 0xff) as f32 / 255.0;
    (48.0 + 78.0 * broad + 24.0 * pigment + 7.0 * paper).round() as u8
}

pub(super) fn oil(color: &mut Color, x: i32, y: i32) -> u8 {
    // Long, softly varying bristle ridges, without hard horizontal seams.
    let bristles = smooth_noise(x as f32 / 3.5, y as f32 / 57.0, 1);
    let body = smooth_noise(x as f32 / 13.0, y as f32 / 83.0, 23);
    let highlight = (bristles * body * 10.0).round() as u8;
    for channel in &mut color[..3] {
        *channel = channel.saturating_add(highlight);
    }
    (218.0 + 24.0 * bristles + 13.0 * body).round() as u8
}

#[cfg(test)]
mod tests {
    use super::super::{styled_shape, texture_color, PaintStyle, Tool};
    use image::RgbaImage;

    #[test]
    fn pigment_has_no_hard_grid_boundaries() {
        for style in [PaintStyle::Watercolor, PaintStyle::Oil] {
            let mut minimum = 255;
            let mut maximum = 0;
            for y in -64..128 {
                for x in -64..128 {
                    let pixel = texture_color([30, 90, 150, 255], style, x, y);
                    minimum = minimum.min(pixel[3]);
                    maximum = maximum.max(pixel[3]);
                    for (dx, dy) in [(1, 0), (0, 1)] {
                        let neighbor = texture_color([30, 90, 150, 255], style, x + dx, y + dy);
                        for channel in 0..4 {
                            assert!(
                                pixel[channel].abs_diff(neighbor[channel]) <= 18,
                                "hard {style:?} boundary at ({x}, {y}), channel {channel}"
                            );
                        }
                    }
                }
            }
            assert!(
                maximum - minimum > 12,
                "texture must retain visible variation"
            );
        }
    }

    #[test]
    fn all_textured_fills_stay_inside_the_shape_and_respect_authored_alpha() {
        for tool in Tool::SHAPES {
            let render = |style, alpha| {
                let mut image = RgbaImage::new(96, 80);
                styled_shape(
                    &mut image,
                    tool,
                    (12, 9),
                    (81, 66),
                    1,
                    None,
                    Some(([237, 28, 36, alpha], style).into()),
                );
                image
            };
            let coverage = render(PaintStyle::Solid, 255);
            for style in PaintStyle::ALL {
                let image = render(style, 127);
                assert_eq!(
                    image,
                    render(style, 127),
                    "redraw changed {tool:?}/{style:?}"
                );
                for (pixel, solid) in image.pixels().zip(coverage.pixels()) {
                    assert!(pixel[3] <= 127, "opacity overflow in {tool:?}/{style:?}");
                    assert!(
                        solid[3] > 0 || pixel[3] == 0,
                        "fill escaped {tool:?}/{style:?}"
                    );
                }
            }
        }
    }
}
