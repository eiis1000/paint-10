use super::{byte, checked_from_coordinates, named, Space};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParsedColor {
    pub rgba: [u8; 4],
    pub explicit_alpha: bool,
    pub in_gamut: bool,
    /// Authored perceptual coordinates, retained for gamut inspection and fitting.
    pub coordinates: Option<(Space, [f64; 4])>,
}

pub fn parse_color(input: &str) -> Result<[u8; 4], String> {
    parse_color_with_alpha(input).map(|color| color.rgba)
}

/// Parse fixed CSS names, hex and common color functions without a CSS environment.
/// Expressions, relative colors, variables and named system colors are rejected.
pub fn parse_color_with_alpha(input: &str) -> Result<ParsedColor, String> {
    let input = input.trim();
    if input.eq_ignore_ascii_case("transparent") {
        return Ok(ParsedColor {
            rgba: [0; 4],
            explicit_alpha: true,
            in_gamut: true,
            coordinates: None,
        });
    }
    if let Some([red, green, blue]) = named::lookup(input) {
        return Ok(ParsedColor {
            rgba: [red, green, blue, 255],
            explicit_alpha: false,
            in_gamut: true,
            coordinates: None,
        });
    }
    let hex = input.strip_prefix('#').unwrap_or(input);
    if hex.is_ascii()
        && matches!(hex.len(), 3 | 4 | 6 | 8)
        && hex.bytes().all(|b| b.is_ascii_hexdigit())
    {
        let mut rgba = [0, 0, 0, 255];
        let short = hex.len() <= 4;
        let width = if short { 1 } else { 2 };
        for (index, channel) in rgba.iter_mut().take(hex.len() / width).enumerate() {
            let start = index * width;
            let value = u8::from_str_radix(&hex[start..start + width], 16).unwrap();
            *channel = if short { value * 17 } else { value };
        }
        return Ok(ParsedColor {
            rgba,
            explicit_alpha: matches!(hex.len(), 4 | 8),
            in_gamut: true,
            coordinates: None,
        });
    }

    let (name, body) = input.split_once('(').ok_or_else(|| {
        "Use hex digits, a CSS color name, or a literal rgb(), hsl(), oklab() or oklch() color."
            .to_owned()
    })?;
    let name = name.to_ascii_lowercase();
    let body = body
        .strip_suffix(')')
        .ok_or("The color function needs a closing parenthesis.")?;
    if body.contains(['(', ')']) {
        return Err("Use numeric color components; expressions are not supported.".into());
    }
    let legacy_allowed = matches!(name.as_str(), "rgb" | "rgba" | "hsl" | "hsla");
    let legacy = body.contains(',');
    let (components, alpha) = split_components(body, legacy_allowed)?;
    if legacy && components.iter().copied().chain(alpha).any(is_none) {
        return Err("The none keyword requires space-separated color components.".into());
    }
    let alpha = alpha
        .map(|token| number_or_percent(token, 1.0))
        .transpose()?;
    let alpha_byte = byte(alpha.unwrap_or(1.0));
    let (space, values) = match name.as_str() {
        "rgb" | "rgba" => {
            require_count(&components, 3)?;
            if legacy
                && components.iter().any(|v| v.ends_with('%'))
                && !components.iter().all(|v| v.ends_with('%'))
            {
                return Err("Comma-separated RGB components must use the same units.".into());
            }
            let rgb = [
                number_or_percent(components[0], 255.0)?,
                number_or_percent(components[1], 255.0)?,
                number_or_percent(components[2], 255.0)?,
                0.0,
            ];
            (Space::Rgb, rgb)
        }
        "hsl" | "hsla" => {
            require_count(&components, 3)?;
            if legacy && !components[1..].iter().all(|v| v.ends_with('%')) {
                return Err(
                    "Comma-separated HSL needs saturation and lightness percentages.".into(),
                );
            }
            (
                Space::Hsl,
                [
                    angle(components[0])?,
                    number_or_percent(components[1], 100.0)?,
                    number_or_percent(components[2], 100.0)?,
                    0.0,
                ],
            )
        }
        "oklab" | "oklch" => {
            require_count(&components, 3)?;
            let lightness = number_or_percent(components[0], 1.0)?.clamp(0.0, 1.0);
            let second = number_or_percent(components[1], 0.4)?;
            let second = if name == "oklch" {
                second.max(0.0)
            } else {
                second
            };
            let third = if name == "oklch" {
                angle(components[2])?
            } else {
                number_or_percent(components[2], 0.4)?
            };
            (
                if name == "oklch" {
                    Space::Oklch
                } else {
                    Space::Oklab
                },
                [lightness, second, third, 0.0],
            )
        }
        "color" => {
            require_count(&components, 4)?;
            let space = match components[0].to_ascii_lowercase().as_str() {
                "srgb" => Space::Rgb,
                "srgb-linear" => Space::LinearRgb,
                _ => return Err("color() currently supports srgb and srgb-linear.".into()),
            };
            let factor = if space == Space::Rgb { 255.0 } else { 1.0 };
            (
                space,
                [
                    number_or_percent(components[1], 1.0)? * factor,
                    number_or_percent(components[2], 1.0)? * factor,
                    number_or_percent(components[3], 1.0)? * factor,
                    0.0,
                ],
            )
        }
        _ => {
            return Err(
                "Supported functions: rgb, hsl, oklab, oklch and color(srgb / srgb-linear).".into(),
            )
        }
    };
    if !values.iter().all(|v| v.is_finite()) {
        return Err("Color components are too large to represent.".into());
    }
    let color = checked_from_coordinates(space, values)
        .ok_or("Color components are too large to convert to sRGB.")?;
    Ok(ParsedColor {
        rgba: [color.rgb[0], color.rgb[1], color.rgb[2], alpha_byte],
        explicit_alpha: alpha.is_some(),
        in_gamut: color.in_gamut,
        coordinates: matches!(space, Space::Oklab | Space::Oklch).then_some((space, values)),
    })
}

fn split_components(body: &str, legacy_allowed: bool) -> Result<(Vec<&str>, Option<&str>), String> {
    if body.contains(',') {
        if !legacy_allowed || body.contains('/') {
            return Err("Use spaces and an optional / alpha for this color function.".into());
        }
        let mut components: Vec<_> = body.split(',').map(str::trim).collect();
        if components
            .iter()
            .any(|part| part.is_empty() || part.split_whitespace().count() != 1)
        {
            return Err("Each comma-separated component must contain one number.".into());
        }
        let alpha = if components.len() == 4 {
            components.pop()
        } else {
            None
        };
        return Ok((components, alpha));
    }
    let mut parts = body.split('/');
    let components = parts
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect();
    let alpha = parts.next().map(str::trim);
    if parts.next().is_some() || alpha.is_some_and(|value| value.split_whitespace().count() != 1) {
        return Err("Use one alpha value after the slash.".into());
    }
    Ok((components, alpha))
}

fn require_count(components: &[&str], count: usize) -> Result<(), String> {
    if components.len() == count {
        Ok(())
    } else {
        Err(format!("Expected {count} color components."))
    }
}

fn number_or_percent(token: &str, reference: f64) -> Result<f64, String> {
    if is_none(token) {
        // Standalone colors resolve missing components to zero, including alpha.
        // https://www.w3.org/TR/css-color-4/#missing
        Ok(0.0)
    } else if let Some(value) = token.strip_suffix('%') {
        Ok(number(value)? / 100.0 * reference)
    } else {
        number(token)
    }
}

fn number(token: &str) -> Result<f64, String> {
    if !is_css_number(token) {
        return Err(format!("Invalid numeric component: {token}"));
    }
    token
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("Invalid or non-finite numeric component: {token}"))
}

fn is_none(token: &str) -> bool {
    token.eq_ignore_ascii_case("none")
}

fn is_css_number(token: &str) -> bool {
    // Consume exactly one CSS number, rather than Rust-only float spellings.
    // https://www.w3.org/TR/css-syntax-3/#consume-number
    let mut bytes = token.bytes().peekable();
    if matches!(bytes.peek(), Some(b'+' | b'-')) {
        bytes.next();
    }

    let mut digits = 0;
    while bytes.peek().is_some_and(u8::is_ascii_digit) {
        bytes.next();
        digits += 1;
    }
    if bytes.peek() == Some(&b'.') {
        bytes.next();
        let mut fraction_digits = 0;
        while bytes.peek().is_some_and(u8::is_ascii_digit) {
            bytes.next();
            fraction_digits += 1;
        }
        if fraction_digits == 0 {
            return false;
        }
        digits += fraction_digits;
    }
    if digits == 0 {
        return false;
    }

    if matches!(bytes.peek(), Some(b'e' | b'E')) {
        bytes.next();
        if matches!(bytes.peek(), Some(b'+' | b'-')) {
            bytes.next();
        }
        let mut exponent_digits = 0;
        while bytes.peek().is_some_and(u8::is_ascii_digit) {
            bytes.next();
            exponent_digits += 1;
        }
        if exponent_digits == 0 {
            return false;
        }
    }
    bytes.next().is_none()
}

fn angle(token: &str) -> Result<f64, String> {
    if is_none(token) {
        return Ok(0.0);
    }
    let token = token.to_ascii_lowercase();
    let degrees = if let Some(value) = token.strip_suffix("grad") {
        number(value)? * 0.9
    } else if let Some(value) = token.strip_suffix("deg") {
        number(value)?
    } else if let Some(value) = token.strip_suffix("turn") {
        number(value)?.rem_euclid(1.0) * 360.0
    } else if let Some(value) = token.strip_suffix("rad") {
        number(value)?
            .rem_euclid(std::f64::consts::TAU)
            .to_degrees()
    } else {
        number(&token)?
    };
    Ok(degrees.rem_euclid(360.0))
}

#[cfg(test)]
mod tests;
