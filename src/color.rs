//! Color coordinates for editing an sRGB document without changing its profile.
//!
//! Transfer functions and CSS units follow <https://www.w3.org/TR/css-color-4/>.
//! Oklab uses Björn Ottosson's public-domain, January 2021 matrices:
//! <https://bottosson.github.io/posts/oklab/>.

mod named;
mod parse;

pub use parse::{parse_color, parse_color_with_alpha, ParsedColor};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    PaintHsl,
    Rgb,
    Hsl,
    Hsv,
    LinearRgb,
    Cmyk,
    Oklab,
    Oklch,
}

#[derive(Clone, Copy, Debug)]
pub struct Channel {
    pub label: &'static str,
    pub min: f64,
    pub max: f64,
    pub decimals: usize,
    pub suffix: &'static str,
}

impl Channel {
    const fn new(
        label: &'static str,
        min: f64,
        max: f64,
        decimals: usize,
        suffix: &'static str,
    ) -> Self {
        Self {
            label,
            min,
            max,
            decimals,
            suffix,
        }
    }
}

impl Space {
    pub const ALL: [Self; 8] = [
        Self::PaintHsl,
        Self::Rgb,
        Self::Hsl,
        Self::Hsv,
        Self::LinearRgb,
        Self::Cmyk,
        Self::Oklab,
        Self::Oklch,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::PaintHsl => "Paint HSL",
            Self::Rgb => "RGB",
            Self::Hsl => "HSL",
            Self::Hsv => "HSV",
            Self::LinearRgb => "Linear RGB",
            Self::Cmyk => "CMYK (approximate)",
            Self::Oklab => "OKLab",
            Self::Oklch => "OKLCH",
        }
    }

    pub const fn channels(self) -> &'static [Channel] {
        match self {
            Self::PaintHsl => {
                const {
                    &[
                        Channel::new("Hue", 0.0, 239.0, 0, ""),
                        Channel::new("Saturation", 0.0, 240.0, 0, ""),
                        Channel::new("Luminosity", 0.0, 240.0, 0, ""),
                    ]
                }
            }
            Self::Rgb => {
                const {
                    &[
                        Channel::new("Red", 0.0, 255.0, 0, ""),
                        Channel::new("Green", 0.0, 255.0, 0, ""),
                        Channel::new("Blue", 0.0, 255.0, 0, ""),
                    ]
                }
            }
            Self::Hsl => {
                const {
                    &[
                        Channel::new("Hue", 0.0, 360.0, 2, "°"),
                        Channel::new("Saturation", 0.0, 100.0, 2, "%"),
                        Channel::new("Lightness", 0.0, 100.0, 2, "%"),
                    ]
                }
            }
            Self::Hsv => {
                const {
                    &[
                        Channel::new("Hue", 0.0, 360.0, 2, "°"),
                        Channel::new("Saturation", 0.0, 100.0, 2, "%"),
                        Channel::new("Value", 0.0, 100.0, 2, "%"),
                    ]
                }
            }
            Self::LinearRgb => {
                const {
                    &[
                        Channel::new("Red", 0.0, 1.0, 5, ""),
                        Channel::new("Green", 0.0, 1.0, 5, ""),
                        Channel::new("Blue", 0.0, 1.0, 5, ""),
                    ]
                }
            }
            Self::Cmyk => {
                const {
                    &[
                        Channel::new("Cyan", 0.0, 100.0, 2, "%"),
                        Channel::new("Magenta", 0.0, 100.0, 2, "%"),
                        Channel::new("Yellow", 0.0, 100.0, 2, "%"),
                        Channel::new("Black", 0.0, 100.0, 2, "%"),
                    ]
                }
            }
            Self::Oklab => {
                const {
                    &[
                        Channel::new("Lightness", 0.0, 1.0, 5, ""),
                        Channel::new("a", -0.4, 0.4, 5, ""),
                        Channel::new("b", -0.4, 0.4, 5, ""),
                    ]
                }
            }
            Self::Oklch => {
                const {
                    &[
                        Channel::new("Lightness", 0.0, 1.0, 5, ""),
                        Channel::new("Chroma", 0.0, 0.4, 5, ""),
                        Channel::new("Hue", 0.0, 360.0, 2, "°"),
                    ]
                }
            }
        }
    }
}

/// The preview is clipped to sRGB; `in_gamut` describes the requested color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Conversion {
    pub rgb: [u8; 3],
    pub in_gamut: bool,
}

/// Return full-precision coordinates. Display rounding must not mutate the color.
pub fn coordinates(space: Space, rgb: [u8; 3]) -> [f64; 4] {
    let channels = rgb.map(|v| f64::from(v) / 255.0);
    let [red, green, blue] = channels;
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let delta = max - min;
    let hue = if delta == 0.0 {
        0.0
    } else if max == red {
        (60.0 * (green - blue) / delta).rem_euclid(360.0)
    } else if max == green {
        60.0 * ((blue - red) / delta + 2.0)
    } else {
        60.0 * ((red - green) / delta + 4.0)
    };
    let triple = match space {
        Space::Rgb => rgb.map(f64::from),
        Space::LinearRgb => channels.map(srgb_to_linear),
        Space::Hsl | Space::PaintHsl => {
            let lightness = (max + min) / 2.0;
            let saturation = if delta == 0.0 {
                0.0
            } else {
                delta / (1.0 - (2.0 * lightness - 1.0).abs())
            };
            if space == Space::PaintHsl {
                [
                    if delta == 0.0 {
                        160.0
                    } else {
                        (hue * 240.0 / 360.0).round().rem_euclid(240.0)
                    },
                    (saturation * 240.0).round(),
                    (lightness * 240.0).round(),
                ]
            } else {
                [hue, saturation * 100.0, lightness * 100.0]
            }
        }
        Space::Hsv => [
            hue,
            if max == 0.0 { 0.0 } else { 100.0 * delta / max },
            max * 100.0,
        ],
        Space::Cmyk => {
            // Unprofiled subtractive approximation, not a print/ICC conversion.
            if max == 0.0 {
                return [0.0, 0.0, 0.0, 100.0];
            }
            return [
                (1.0 - red / max) * 100.0,
                (1.0 - green / max) * 100.0,
                (1.0 - blue / max) * 100.0,
                (1.0 - max) * 100.0,
            ];
        }
        Space::Oklab => linear_to_oklab(channels.map(srgb_to_linear)),
        Space::Oklch => {
            let [lightness, a, b] = linear_to_oklab(channels.map(srgb_to_linear));
            let chroma = a.hypot(b);
            if chroma < 0.000_004 {
                [lightness, 0.0, 0.0]
            } else {
                [lightness, chroma, b.atan2(a).to_degrees().rem_euclid(360.0)]
            }
        }
    };
    [triple[0], triple[1], triple[2], 0.0]
}

pub fn from_coordinates(space: Space, values: [f64; 4]) -> Conversion {
    checked_from_coordinates(space, values).unwrap_or(Conversion {
        rgb: [0; 3],
        in_gamut: false,
    })
}

// Literal entry must distinguish conversion overflow from a valid clipped color.
fn checked_from_coordinates(space: Space, values: [f64; 4]) -> Option<Conversion> {
    if !values[..space.channels().len()]
        .iter()
        .all(|v| v.is_finite())
    {
        return None;
    }
    let [a, b, c, d] = values;
    let rgb = match space {
        Space::Rgb => [a / 255.0, b / 255.0, c / 255.0],
        Space::LinearRgb => [a, b, c].map(linear_to_srgb),
        Space::Hsl => hsl_to_srgb(a, b / 100.0, c / 100.0),
        Space::PaintHsl => hsl_to_srgb(a * 360.0 / 240.0, b / 240.0, c / 240.0),
        Space::Hsv => {
            let saturation = (b / 100.0).clamp(0.0, 1.0);
            let value = (c / 100.0).clamp(0.0, 1.0);
            hue_rgb(a, saturation * value, value * (1.0 - saturation))
        }
        Space::Cmyk => [a, b, c]
            .map(|v| (1.0 - v.clamp(0.0, 100.0) / 100.0) * (1.0 - d.clamp(0.0, 100.0) / 100.0)),
        Space::Oklab => oklab_to_linear([a, b, c]).map(linear_to_srgb),
        Space::Oklch => oklab_to_linear(oklch_to_oklab([a, b, c])).map(linear_to_srgb),
    };
    if !rgb.iter().all(|channel| channel.is_finite()) {
        return None;
    }
    Some(Conversion {
        rgb: rgb.map(byte),
        in_gamut: in_unit_gamut(rgb.map(srgb_to_linear)),
    })
}

pub fn format_hex([red, green, blue, alpha]: [u8; 4], include_alpha: bool) -> String {
    if include_alpha {
        format!("#{red:02X}{green:02X}{blue:02X}{alpha:02X}")
    } else {
        format!("#{red:02X}{green:02X}{blue:02X}")
    }
}

/// Reduce chroma into sRGB while preserving lightness and hue.
///
/// This is a conservative chroma search, not CSS's perceptual gamut mapper.
/// Lightness outside 0..1 is brought to the nearest endpoint first.
pub fn fit_oklch([lightness, chroma, hue]: [f64; 3]) -> [f64; 3] {
    if ![lightness, chroma, hue].iter().all(|v| v.is_finite()) {
        return [0.0; 3];
    }
    let lightness = lightness.clamp(0.0, 1.0);
    let hue = hue.rem_euclid(360.0);
    if lightness == 0.0 || lightness == 1.0 {
        return [lightness, 0.0, hue];
    }
    let chroma = chroma.max(0.0);
    let inside = |c| in_unit_gamut(oklab_to_linear(oklch_to_oklab([lightness, c, hue])));
    if inside(chroma) {
        return [lightness, chroma, hue];
    }
    let mut low = 0.0;
    let mut high = chroma.min(1.0);
    for _ in 0..32 {
        let middle = (low + high) / 2.0;
        if inside(middle) {
            low = middle;
        } else {
            high = middle;
        }
    }
    [lightness, low, hue]
}

fn in_unit_gamut(rgb: [f64; 3]) -> bool {
    // Matrix coefficients are rounded; tolerate sub-byte floating-point noise.
    rgb.iter()
        .all(|v| v.is_finite() && (-1e-6..=1.0 + 1e-6).contains(v))
}

fn byte(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn srgb_to_linear(value: f64) -> f64 {
    if value.abs() <= 0.04045 {
        value / 12.92
    } else {
        value.signum() * ((value.abs() + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(value: f64) -> f64 {
    if value.abs() <= 0.0031308 {
        value * 12.92
    } else {
        value.signum() * (1.055 * value.abs().powf(1.0 / 2.4) - 0.055)
    }
}

fn hsl_to_srgb(hue: f64, saturation: f64, lightness: f64) -> [f64; 3] {
    let saturation = saturation.clamp(0.0, 1.0);
    let lightness = lightness.clamp(0.0, 1.0);
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    hue_rgb(hue, chroma, lightness - chroma / 2.0)
}

fn hue_rgb(hue: f64, chroma: f64, offset: f64) -> [f64; 3] {
    let sector = hue.rem_euclid(360.0) / 60.0;
    let second = chroma * (1.0 - (sector.rem_euclid(2.0) - 1.0).abs());
    let rgb = match sector as u8 {
        0 => [chroma, second, 0.0],
        1 => [second, chroma, 0.0],
        2 => [0.0, chroma, second],
        3 => [0.0, second, chroma],
        4 => [second, 0.0, chroma],
        _ => [chroma, 0.0, second],
    };
    rgb.map(|v| v + offset)
}

fn linear_to_oklab([red, green, blue]: [f64; 3]) -> [f64; 3] {
    let l = (0.4122214708 * red + 0.5363325363 * green + 0.0514459929 * blue).cbrt();
    let m = (0.2119034982 * red + 0.6806995451 * green + 0.1073969566 * blue).cbrt();
    let s = (0.0883024619 * red + 0.2817188376 * green + 0.6299787005 * blue).cbrt();
    [
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    ]
}

fn oklab_to_linear([lightness, a, b]: [f64; 3]) -> [f64; 3] {
    let l = (lightness + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m = (lightness - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s = (lightness - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    [
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    ]
}

fn oklch_to_oklab([lightness, chroma, hue]: [f64; 3]) -> [f64; 3] {
    let angle = hue.rem_euclid(360.0).to_radians();
    [
        lightness,
        chroma.max(0.0) * angle.cos(),
        chroma.max(0.0) * angle.sin(),
    ]
}

#[cfg(test)]
mod tests;
