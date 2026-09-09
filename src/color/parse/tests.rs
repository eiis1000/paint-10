use super::*;

#[test]
fn named_colors_include_all_standard_names_and_case_insensitive_aliases() {
    assert_eq!(named::COLORS.len(), 148);
    for pair in named::COLORS.windows(2) {
        assert!(pair[0].0 < pair[1].0);
    }
    for (name, _) in named::COLORS {
        let color = parse_color_with_alpha(name).unwrap();
        assert!(color.in_gamut, "{name}");
        assert!(!color.explicit_alpha, "{name}");
        assert!(color.coordinates.is_none(), "{name}");
        assert_eq!(color.rgba[3], 255, "{name}");
        assert_eq!(parse_color(&name.to_ascii_uppercase()).unwrap(), color.rgba);
    }
    // Normative examples include CSS/X11 differences and non-primary channels.
    for (name, rgb) in [
        (" ALICEBLUE ", [240, 248, 255]),
        ("RebeccaPurple", [102, 51, 153]),
        ("green", [0, 128, 0]),
        ("lime", [0, 255, 0]),
        ("gray", [128, 128, 128]),
        ("lightgoldenrodyellow", [250, 250, 210]),
        ("papayawhip", [255, 239, 213]),
        ("yellowgreen", [154, 205, 50]),
    ] {
        assert_eq!(parse_color(name).unwrap(), [rgb[0], rgb[1], rgb[2], 255]);
    }
    for (first, second) in [
        ("aqua", "cyan"),
        ("fuchsia", "magenta"),
        ("gray", "grey"),
        ("darkgray", "darkgrey"),
        ("darkslategray", "darkslategrey"),
        ("dimgray", "dimgrey"),
        ("lightgray", "lightgrey"),
        ("lightslategray", "lightslategrey"),
        ("slategray", "slategrey"),
    ] {
        assert_eq!(parse_color(first), parse_color(second));
    }
}

#[test]
fn color_names_do_not_accept_css_environment_values_or_near_misses() {
    for input in [
        "currentcolor",
        "Canvas",
        "CanvasText",
        "AccentColor",
        "ButtonFace",
        "inherit",
        "initial",
        "unset",
        "revert",
        "light goldenrod yellow",
        "rebeccapurple;",
        "red blue",
        "none",
        "var(--brand)",
    ] {
        assert!(parse_color(input).is_err(), "{input}");
    }
    let transparent = parse_color_with_alpha("TRANSPARENT").unwrap();
    assert_eq!(transparent.rgba, [0; 4]);
    assert!(transparent.explicit_alpha);
    assert_eq!(parse_color("abc").unwrap(), [170, 187, 204, 255]);
}

#[test]
fn missing_components_resolve_to_zero_for_standalone_modern_colors() {
    for (input, expected) in [
        ("rgb(none 128 none)", [0, 128, 0, 255]),
        ("rgba(10 none 30 / NONE)", [10, 0, 30, 0]),
        ("hsl(none 100% 50%)", [255, 0, 0, 255]),
        ("hsla(120 none 50% / none)", [128, 128, 128, 0]),
        ("oklab(50% none none)", [99, 99, 99, 255]),
        ("oklch(50% none none / none)", [99, 99, 99, 0]),
        ("color(srgb none .5 none)", [0, 128, 0, 255]),
        ("color(srgb-linear none 0.2158605 none)", [0, 128, 0, 255]),
    ] {
        let color = parse_color_with_alpha(input).unwrap();
        assert_eq!(color.rgba, expected, "{input}");
        assert_eq!(color.explicit_alpha, input.contains('/'), "{input}");
    }
    let color = parse_color_with_alpha("oklch(50% none none)").unwrap();
    assert_eq!(
        color.coordinates,
        Some((Space::Oklch, [0.5, 0.0, 0.0, 0.0]))
    );
    for input in [
        "rgb(none, 0, 0)",
        "rgba(0, 0, 0, none)",
        "hsl(none, 100%, 50%)",
        "hsla(120, 100%, 50%, none)",
        "rgb(none% 0 0)",
        "hsl(nonedeg 100% 50%)",
        "color(none 0 0 0)",
        "oklch(50% none none%)",
    ] {
        assert!(parse_color(input).is_err(), "{input}");
    }
}

#[test]
fn css_numbers_require_digits_after_decimal_points_and_exponents() {
    for (number, value) in [
        ("0", 0.0),
        ("-0", 0.0),
        ("+12", 12.0),
        (".5", 0.5),
        ("-.5", -0.5),
        ("+.5", 0.5),
        ("12.50", 12.5),
        ("1e2", 100.0),
        ("1E+2", 100.0),
        ("1.2e-2", 0.012),
        (".5E1", 5.0),
    ] {
        assert_eq!(super::number(number).unwrap(), value, "{number}");
    }
    for token in [
        "", "+", "-", ".", "1.", "1.e2", ".e2", "1e", "1e+", "1e-", "1e2.0", "1_000", "0x10",
        "++1", "1+2", "NaN", "inf", "1e999", "１２", "−1",
    ] {
        assert!(super::number(token).is_err(), "{token}");
    }
    for input in [
        "rgb(1. 0 0)",
        "rgb(1.e2 0 0)",
        "rgb(1.% 0 0)",
        "hsl(1.deg 100% 50%)",
        "oklab(50% 0 0 / 1.)",
    ] {
        assert!(parse_color(input).is_err(), "{input}");
    }
    assert_eq!(
        parse_color("rgb(1e2 +.5e2 0 / 5e1%)").unwrap(),
        [100, 50, 0, 128]
    );
    assert_eq!(
        parse_color("hsl(1.2e2deg 100% 50%)").unwrap(),
        [0, 255, 0, 255]
    );
}

#[test]
fn unrepresentable_color_conversions_fail_without_rejecting_gamut_clipping() {
    for input in [
        "oklab(50% 1e308 1e308)",
        "oklab(50% -1e308 1e308)",
        "oklch(50% 1e308 30)",
    ] {
        let error = parse_color_with_alpha(input).unwrap_err();
        assert!(error.contains("too large"), "{input}: {error}");
    }
    let outside = parse_color_with_alpha("oklch(70% .4 30)").unwrap();
    assert!(!outside.in_gamut);
    assert_eq!(
        outside.coordinates,
        Some((Space::Oklch, [0.7, 0.4, 30.0, 0.0]))
    );
    let clipped = parse_color_with_alpha("rgb(300 -5 0 / 150%)").unwrap();
    assert_eq!(clipped.rgba, [255, 0, 0, 255]);
    assert!(!clipped.in_gamut);
}
