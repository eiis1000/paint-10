//! Convert Paint's straight-alpha sRGB pixels for the pinned egui glow renderer.
//!
//! Paint composites encoded sRGB channels. egui_glow 0.31.1 also blends encoded
//! channels (`ONE`, `ONE_MINUS_SRC_ALPHA`), but ecolor's unmultiplied constructor
//! premultiplies in linear light. Passing that result to this renderer makes
//! translucent light colors too bright. Premultiply the encoded bytes here,
//! only at the display boundary; document pixels and their hidden RGB stay intact.

use crate::document::Color;
use eframe::egui::{Color32, ColorImage};

/// A display color for egui_glow's premultiplied sRGB blending.
#[inline]
pub fn color([red, green, blue, alpha]: Color) -> Color32 {
    if alpha == 255 {
        return Color32::from_rgb(red, green, blue);
    }
    if alpha == 0 {
        return Color32::TRANSPARENT;
    }
    let premultiply = |channel: u8| ((u32::from(channel) * u32::from(alpha) + 127) / 255) as u8;
    Color32::from_rgba_premultiplied(
        premultiply(red),
        premultiply(green),
        premultiply(blue),
        alpha,
    )
}

/// A display texture from row-major, straight-alpha sRGB bytes.
pub fn image(size: [usize; 2], rgba: &[u8]) -> ColorImage {
    assert_eq!(
        size[0]
            .checked_mul(size[1])
            .and_then(|pixels| pixels.checked_mul(4)),
        Some(rgba.len()),
        "RGBA byte count must match the image dimensions",
    );
    ColorImage {
        size,
        pixels: rgba
            .chunks_exact(4)
            .map(|pixel| color([pixel[0], pixel[1], pixel[2], pixel[3]]))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document;
    use image::{Rgba, RgbaImage};

    // Model the pinned renderer's ONE / ONE_MINUS_SRC_ALPHA equation after
    // sampling a solid texture pixel or mesh color. Quantization can add one
    // channel level relative to Paint's single-rounding document compositor.
    fn display_over(source: Color32, background: Color) -> Color {
        let mut result = background;
        for channel in 0..3 {
            result[channel] = (f64::from(source[channel])
                + f64::from(background[channel]) * (1.0 - f64::from(source.a()) / 255.0))
                .round()
                .clamp(0.0, 255.0) as u8;
        }
        result
    }

    fn assert_matches_document(source: Color, displayed: Color32, background: Color) {
        let mut expected = RgbaImage::from_pixel(1, 1, Rgba(background));
        document::blend(&mut expected, 0, 0, source);
        let actual = display_over(displayed, background);
        for (actual, expected) in actual.into_iter().zip(expected.get_pixel(0, 0).0) {
            assert!(
                actual.abs_diff(expected) <= 1,
                "{source:?} over {background:?}"
            );
        }
    }

    #[test]
    fn translucent_display_matches_document_compositing_at_every_alpha() {
        for rgb in [
            [192, 196, 185],
            [255, 255, 255],
            [8, 27, 94],
            [255, 80, 0],
            [0, 0, 0],
        ] {
            for alpha in 0..=255 {
                let source = [rgb[0], rgb[1], rgb[2], alpha];
                for background in [
                    [215, 215, 215, 255],
                    [255; 4],
                    [0, 0, 0, 255],
                    [17, 91, 205, 255],
                ] {
                    assert_matches_document(source, color(source), background);
                }
            }
        }
    }

    #[test]
    fn light_alpha_fixture_retains_checkerboard_contrast_instead_of_clipping_white() {
        let source = [192, 196, 185, 96];
        let dark = display_over(color(source), [215, 215, 215, 255]);
        let light = display_over(color(source), [255; 4]);
        assert_eq!(dark, [206, 208, 204, 255]);
        assert_eq!(light, [231, 233, 229, 255]);
        assert_matches_document(source, color(source), [215, 215, 215, 255]);
        assert_matches_document(source, color(source), [255; 4]);
    }

    #[test]
    fn textures_share_color_compositing_without_mutating_source_or_leaking_hidden_rgb() {
        let source = [
            [192, 196, 185, 96],
            [255, 23, 240, 0],
            [255, 255, 255, 1],
            [17, 91, 205, 255],
        ];
        let bytes = source.concat();
        let original = bytes.clone();
        let texture = image([2, 2], &bytes);
        assert_eq!(texture.size, [2, 2]);
        assert_eq!(texture.pixels[1], Color32::TRANSPARENT);
        assert_eq!(texture.pixels[3].to_array(), source[3]);
        assert_eq!(bytes, original, "display must preserve hidden source RGB");
        for (source, displayed) in source.into_iter().zip(texture.pixels) {
            for background in [[215, 215, 215, 255], [255; 4], [17, 91, 205, 255]] {
                assert_matches_document(source, displayed, background);
            }
        }
    }
}
