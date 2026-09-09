use super::*;

fn near(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} differs from {expected}"
    );
}

#[test]
fn reference_primaries_and_gray_have_expected_coordinates() {
    assert_eq!(
        coordinates(Space::Hsl, [255, 0, 0]),
        [0.0, 100.0, 50.0, 0.0]
    );
    assert_eq!(
        coordinates(Space::Hsv, [0, 128, 0]),
        [120.0, 100.0, 12800.0 / 255.0, 0.0]
    );
    assert_eq!(
        coordinates(Space::Cmyk, [255, 0, 0]),
        [0.0, 100.0, 100.0, 0.0]
    );
    assert_eq!(coordinates(Space::Cmyk, [0, 0, 0]), [0.0, 0.0, 0.0, 100.0]);
    assert_eq!(
        coordinates(Space::PaintHsl, [255; 3]),
        [160.0, 0.0, 240.0, 0.0]
    );
    near(
        coordinates(Space::LinearRgb, [128; 3])[0],
        0.2158605001,
        1e-9,
    );
    let lab = coordinates(Space::Oklab, [255, 0, 0]);
    near(lab[0], 0.62795536, 1e-8);
    near(lab[1], 0.22486306, 1e-8);
    near(lab[2], 0.12584630, 1e-8);
    let white = coordinates(Space::Oklab, [255; 3]);
    near(white[0], 1.0, 1e-7);
    near(white[1], 0.0, 1e-7);
    near(white[2], 0.0, 1e-7);
    assert_eq!(coordinates(Space::Oklch, [128; 3])[1..3], [0.0, 0.0]);
}

#[test]
fn all_spaces_roundtrip_a_representative_rgb_cube() {
    for space in Space::ALL {
        for red in (0..=255).step_by(17) {
            for green in (0..=255).step_by(17) {
                for blue in (0..=255).step_by(17) {
                    let original = [red, green, blue];
                    let converted = from_coordinates(space, coordinates(space, original));
                    assert!(converted.in_gamut, "{space:?}, {original:?}");
                    let tolerance = if space == Space::PaintHsl { 4 } else { 1 };
                    for (actual, expected) in converted.rgb.into_iter().zip(original) {
                        assert!(
                            actual.abs_diff(expected) <= tolerance,
                            "{space:?}: {original:?} -> {:?}",
                            converted.rgb
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn paint_hue_wraps_at_red_and_preserves_the_legacy_primary_coordinates() {
    for (rgb, values) in [
        ([255, 0, 0], [0.0, 240.0, 120.0, 0.0]),
        ([255, 255, 0], [40.0, 240.0, 120.0, 0.0]),
        ([0, 255, 0], [80.0, 240.0, 120.0, 0.0]),
        ([0, 255, 255], [120.0, 240.0, 120.0, 0.0]),
        ([0, 0, 255], [160.0, 240.0, 120.0, 0.0]),
        ([255, 0, 255], [200.0, 240.0, 120.0, 0.0]),
    ] {
        assert_eq!(coordinates(Space::PaintHsl, rgb), values);
        assert_eq!(from_coordinates(Space::PaintHsl, values).rgb, rgb);
    }
    for rgb in [[255, 0, 1], [235, 0, 1], [255, 1, 0]] {
        let values = coordinates(Space::PaintHsl, rgb);
        assert_eq!(values[0], 0.0);
        let converted = from_coordinates(Space::PaintHsl, values);
        for (actual, expected) in converted.rgb.into_iter().zip(rgb) {
            assert!(actual.abs_diff(expected) <= 1);
        }
    }
}

#[test]
fn transfer_functions_use_the_srgb_linear_segment_and_extended_sign() {
    near(srgb_to_linear(0.04045), 0.04045 / 12.92, 1e-12);
    near(linear_to_srgb(0.0031308), 0.0031308 * 12.92, 1e-12);
    for value in [-0.5, -0.01, 0.0, 0.02, 0.5, 1.0, 1.2] {
        near(linear_to_srgb(srgb_to_linear(value)), value, 1e-12);
    }
}

#[test]
fn gamut_fit_preserves_lightness_and_hue_while_reducing_chroma() {
    for hue in (0..360).step_by(15) {
        let requested = [0.7, 0.4, f64::from(hue)];
        assert!(!from_coordinates(Space::Oklch, [0.7, 0.4, f64::from(hue), 0.0]).in_gamut);
        let fitted = fit_oklch(requested);
        assert_eq!(fitted[0], requested[0]);
        assert_eq!(fitted[2], requested[2]);
        assert!(fitted[1] < requested[1]);
        assert!(from_coordinates(Space::Oklch, [fitted[0], fitted[1], fitted[2], 0.0]).in_gamut);
    }
    let inside = [0.6, 0.05, 45.0];
    assert_eq!(fit_oklch(inside), inside);
    for lightness in [0.0, 1.0] {
        let fitted = fit_oklch([lightness, 0.4, -90.0]);
        assert_eq!(fitted[2], 270.0);
        assert!(fitted[1] < 0.001);
    }
    assert!(!from_coordinates(Space::Oklab, [f64::NAN, 0.0, 0.0, 0.0]).in_gamut);
    assert_eq!(fit_oklch([f64::INFINITY, 0.0, 0.0]), [0.0; 3]);
}

#[test]
fn hex_and_literal_css_colors_preserve_explicit_alpha_metadata() {
    for (text, expected, alpha) in [
        ("#f80", [255, 136, 0, 255], false),
        ("#f808", [255, 136, 0, 136], true),
        (" 1a2B3c ", [26, 43, 60, 255], false),
        ("#1a2b3c80", [26, 43, 60, 128], true),
        ("rgb(255, 128, 0)", [255, 128, 0, 255], false),
        ("rgba(255, 128, 0, .5)", [255, 128, 0, 128], true),
        ("RGB(100% 50% 0% / 25%)", [255, 128, 0, 64], true),
        ("hsl(120deg 100% 50%)", [0, 255, 0, 255], false),
        ("hsla(.5turn, 100%, 50%, 50%)", [0, 255, 255, 128], true),
        ("hsl(200grad 100 50)", [0, 255, 255, 255], false),
        (
            "hsl(3.141592653589793rad 100% 50%)",
            [0, 255, 255, 255],
            false,
        ),
        (
            "oklab(0.62795536 0.22486306 0.12584630 / .5)",
            [255, 0, 0, 128],
            true,
        ),
        ("oklab(50% 0% 0%)", [99, 99, 99, 255], false),
        ("oklch(50% 0% 200)", [99, 99, 99, 255], false),
        ("color(srgb 1 .5 0 / .5)", [255, 128, 0, 128], true),
        (
            "color(srgb-linear 0.2158605 0.2158605 0.2158605)",
            [128, 128, 128, 255],
            false,
        ),
        ("transparent", [0; 4], true),
    ] {
        assert_eq!(parse_color(text).unwrap(), expected, "{text}");
        assert_eq!(
            parse_color_with_alpha(text).unwrap().explicit_alpha,
            alpha,
            "{text}"
        );
    }
    assert_eq!(format_hex([26, 43, 60, 128], true), "#1A2B3C80");
    assert_eq!(format_hex([26, 43, 60, 128], false), "#1A2B3C");
}

#[test]
fn color_parser_rejects_malformed_or_nonfinite_literals() {
    for text in [
        "#12",
        "#GG0000",
        "#ff00ff trailing",
        "rgb(1 2)",
        "rgb(1, 2,)",
        "rgb(1,2,3 / .5)",
        "rgb(1 2 3 / / 1)",
        "rgb(1 2 3 / )",
        "rgb(1%, 2, 3)",
        "hsl(120, 1, 0.5)",
        "oklab(.5, 0, 0)",
        "rgb(NaN 0 0)",
        "rgb(inf 0 0)",
        "rgb(1e999 0 0)",
        "rgb(1 2 3))",
        "rgb(1 2 3)junk",
        "rgb (1 2 3)",
        "rgb(calc(2) 3 4)",
        "oklch(.5 .1 50%)",
        "color(display-p3 1 0 0)",
    ] {
        assert!(
            parse_color(text).is_err(),
            "Accepted malformed color: {text}"
        );
    }
}

#[test]
fn parsed_perceptual_coordinates_retain_the_requested_out_of_gamut_color() {
    let parsed = parse_color_with_alpha("oklch(70% 100% 390deg / 25%)").unwrap();
    assert_eq!(
        parsed.coordinates,
        Some((Space::Oklch, [0.7, 0.4, 30.0, 0.0]))
    );
    assert!(!parsed.in_gamut);
    assert!(parsed.explicit_alpha);
    assert_eq!(parsed.rgba[3], 64);
    let lab = parse_color_with_alpha("oklab(50% -50% 25%)").unwrap();
    assert_eq!(lab.coordinates, Some((Space::Oklab, [0.5, -0.2, 0.1, 0.0])));
    let negative_chroma = parse_color_with_alpha("oklch(50% -25% 30)").unwrap();
    assert_eq!(
        negative_chroma.coordinates,
        Some((Space::Oklch, [0.5, 0.0, 30.0, 0.0]))
    );
    assert_eq!(negative_chroma.rgba, [99, 99, 99, 255]);
}
