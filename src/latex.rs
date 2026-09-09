//! Bounded, offline math notation rendering shared by desktop and WebAssembly.
//!
//! LaTeX source stays in the ordinary retained text object. RaTeX supplies the
//! math layout and bundled KaTeX fonts; Paint owns text-box geometry, outlines,
//! backgrounds, validation, and the small preview cache.

use std::{cell::RefCell, collections::VecDeque, io::Write, sync::Arc};

use image::{Rgba, RgbaImage};
use ratex_layout::{layout, to_display_list, LayoutOptions};
use ratex_types::{display_item::DisplayList, math_style::MathStyle};

use crate::{
    document::{overlay, valid_size, Color},
    text::{paint_text_outline, TextAlignment, TextFormat},
};

pub const MAX_SOURCE_BYTES: usize = 8192;
const MAX_AST_BYTES: usize = 1024 * 1024;
const MAX_NODES: usize = 4096;
const MAX_DISPLAY_ITEMS: usize = 32768;
const MAX_CACHE_ENTRIES: usize = 16;
const MAX_CACHE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, PartialEq)]
struct Key {
    source: String,
    size: f32,
    color: Color,
    background: Option<Color>,
    width: u32,
    minimum_height: u32,
    alignment: TextAlignment,
    outline_width: u32,
    outline_color: Color,
    bold: bool,
    italic: bool,
    underline: bool,
    strikeout: bool,
}

impl Key {
    fn new(source: &str, format: &TextFormat) -> Self {
        Self {
            source: source.into(),
            size: format.size,
            color: format.color,
            background: format.background,
            width: format.width,
            minimum_height: format.minimum_height,
            alignment: format.alignment,
            outline_width: format.outline_width,
            outline_color: format.outline_color,
            bold: format.bold,
            italic: format.italic,
            underline: format.underline,
            strikeout: format.strikeout,
        }
    }
}

struct Prepared {
    display: DisplayList,
    padding: f32,
    output_size: (u32, u32),
    x: i32,
}

struct Cached {
    key: Key,
    prepared: Result<Arc<Prepared>, String>,
    image: Option<Arc<RgbaImage>>,
}

thread_local! {
    static CACHE: RefCell<VecDeque<Cached>> = const { RefCell::new(VecDeque::new()) };
}

/// Validate and measure without allocating a full-size raster image.
pub fn dimensions(source: &str, format: &TextFormat) -> Result<(u32, u32), String> {
    Ok(prepare(source, format)?.output_size)
}

/// Compile editable math source to straight-alpha sRGB pixels.
///
/// An invalid draft returns its error to the source editor; it never substitutes
/// ordinary text or a partial expression. Projects validate through this same
/// layout path before accepting a retained formula.
pub fn render(source: &str, format: &TextFormat) -> Result<RgbaImage, String> {
    let prepared = prepare(source, format)?;
    let key = Key::new(source, format);
    if let Some(image) = CACHE.with(|cache| {
        cache
            .borrow()
            .iter()
            .find(|entry| entry.key == key)
            .and_then(|entry| entry.image.clone())
    }) {
        return Ok((*image).clone());
    }

    let options = ratex_render::RenderOptions {
        font_size: format.size,
        padding: prepared.padding,
        background_color: ratex_types::color::Color::new(0.0, 0.0, 0.0, 0.0),
        ..Default::default()
    };
    let png = ratex_render::render_to_png(&prepared.display, &options)?;
    let formula = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
        .map_err(|error| format!("Could not rasterize the equation: {error}"))?
        .to_rgba8();
    let (width, height) = prepared.output_size;
    let mut foreground = RgbaImage::new(width, height);
    overlay(&mut foreground, &formula, i64::from(prepared.x), 0);
    let mut output = RgbaImage::from_pixel(
        width,
        height,
        Rgba(format.background.unwrap_or([0, 0, 0, 0])),
    );
    if format.outline_width > 0 {
        paint_text_outline(
            &mut output,
            &foreground,
            format.outline_width,
            format.outline_color,
        );
    }
    overlay(&mut output, &foreground, 0, 0);

    if output.as_raw().len() <= MAX_CACHE_BYTES {
        CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            let mut bytes: usize = cache
                .iter()
                .filter_map(|entry| entry.image.as_ref())
                .map(|image| image.as_raw().len())
                .sum();
            for entry in cache.iter_mut().rev() {
                if bytes + output.as_raw().len() <= MAX_CACHE_BYTES {
                    break;
                }
                if let Some(image) = entry.image.take() {
                    bytes -= image.as_raw().len();
                }
            }
            if let Some(entry) = cache.iter_mut().find(|entry| entry.key == key) {
                entry.image = Some(Arc::new(output.clone()));
            }
        });
    }
    Ok(output)
}

fn prepare(source: &str, format: &TextFormat) -> Result<Arc<Prepared>, String> {
    // Check before copying source into the cache or invoking the parser.
    if source.len() > MAX_SOURCE_BYTES {
        return Err("The equation exceeds the 8,192-byte source limit.".into());
    }
    format.validate()?;
    if !format.spans.is_empty() {
        return Err("LaTeX expressions use one text style; remove rich-text spans first.".into());
    }
    let key = Key::new(source, format);
    if let Some(cached) = CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let index = cache.iter().position(|entry| entry.key == key)?;
        let entry = cache.remove(index)?;
        let result = entry.prepared.clone();
        cache.push_front(entry);
        Some(result)
    }) {
        return cached;
    }
    let prepared = compile(source, format).map(Arc::new);
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.push_front(Cached {
            key,
            prepared: prepared.clone(),
            image: None,
        });
        cache.truncate(MAX_CACHE_ENTRIES);
    });
    prepared
}

fn compile(source: &str, format: &TextFormat) -> Result<Prepared, String> {
    validate_source(source)?;
    let mut expression = source.trim().to_owned();
    // The parser also accepts outer math delimiters. Remove them before adding
    // formatting wrappers, so `$x$` and raw `x` behave identically.
    for (open, close) in [("$$", "$$"), ("\\[", "\\]"), ("\\(", "\\)"), ("$", "$")] {
        if let Some(inner) = expression
            .strip_prefix(open)
            .and_then(|s| s.strip_suffix(close))
        {
            expression = inner.to_owned();
            break;
        }
    }
    if format.bold {
        expression = format!("\\boldsymbol{{{expression}}}");
    }
    if format.italic {
        expression = format!("\\mathit{{{expression}}}");
    }
    if format.underline {
        expression = format!("\\underline{{{expression}}}");
    }
    if format.strikeout {
        expression = format!("\\sout{{{expression}}}");
    }
    let ast = ratex_parser::parser::parse(&expression).map_err(|error| error.to_string())?;
    if ast.is_empty() {
        return Err("Enter a LaTeX math expression.".into());
    }
    validate_ast(&ast)?;
    let [r, g, b, a] = format.color;
    let color = ratex_types::color::Color::new(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    );
    let options = LayoutOptions::default()
        .with_style(MathStyle::Display)
        .with_color(color);
    let tree = layout(&ast, &options);
    let padding = 4.0 + format.outline_width as f32;
    // Validate nominal bounds before conversion can expand dashed array rules.
    raster_dimensions(tree.width, tree.height + tree.depth, format.size, padding)?;
    let display = to_display_list(&tree);
    if display.items.len() > MAX_DISPLAY_ITEMS {
        return Err("The equation is too complex to render.".into());
    }
    for item in &display.items {
        if let ratex_types::display_item::DisplayItem::GlyphPath {
            font, char_code, ..
        } = item
        {
            let available = ratex_font::FontId::parse(font)
                .and_then(|font| ratex_font::get_char_metrics(font, *char_code))
                .is_some();
            if !available {
                let character = char::from_u32(*char_code).unwrap_or('\u{fffd}');
                return Err(format!(
                    "The math fonts do not contain {character:?}. Use a regular text box for this text."
                ));
            }
        }
    }
    let raster_size = raster_dimensions(
        display.width,
        display.height + display.depth,
        format.size,
        padding,
    )?;
    let output_size = (
        format.width.max(raster_size.0),
        format.minimum_height.max(raster_size.1),
    );
    if !valid_size(output_size.0, output_size.1) {
        return Err("The equation exceeds the canvas allocation limit.".into());
    }
    let extra = output_size.0 - raster_size.0;
    let x = match format.alignment {
        TextAlignment::Left => 0,
        TextAlignment::Center => extra / 2,
        TextAlignment::Right => extra,
    } as i32;
    Ok(Prepared {
        display,
        padding,
        output_size,
        x,
    })
}

fn raster_dimensions(
    width: f64,
    height: f64,
    size: f32,
    padding: f32,
) -> Result<(u32, u32), String> {
    if !width.is_finite() || !height.is_finite() || !size.is_finite() || !padding.is_finite() {
        return Err("The equation contains invalid dimensions.".into());
    }
    let width = (width as f32 * size + 2.0 * padding).ceil().max(1.0);
    let height = (height as f32 * size + 2.0 * padding).ceil().max(1.0);
    if !width.is_finite() || !height.is_finite() || !valid_size(width as u32, height as u32) {
        return Err("The equation exceeds the canvas allocation limit.".into());
    }
    Ok((width as u32, height as u32))
}

fn validate_source(source: &str) -> Result<(), String> {
    if source.trim().is_empty() {
        return Err("Enter a LaTeX math expression.".into());
    }
    let mut characters = source.char_indices().peekable();
    while let Some((_, character)) = characters.next() {
        match character {
            '%' => {
                for (_, character) in characters.by_ref() {
                    if character == '\n' {
                        break;
                    }
                }
            }
            '\\' => {
                let start = characters.peek().map_or(source.len(), |&(index, _)| index);
                let mut end = start;
                while let Some(&(index, character)) = characters.peek() {
                    if !character.is_ascii_alphabetic() && character != '@' {
                        break;
                    }
                    end = index + character.len_utf8();
                    characters.next();
                }
                if start == end {
                    characters.next();
                    continue;
                }
                let command = &source[start..end];
                if command == "begin" {
                    let environment = literal_argument(&mut characters)?;
                    if matches!(environment.trim(), "alignat" | "alignat*" | "alignedat") {
                        let count = literal_argument(&mut characters)?;
                        if count
                            .trim()
                            .parse::<usize>()
                            .ok()
                            .is_none_or(|count| count == 0 || count > 64)
                        {
                            return Err("An alignat environment must use 1–64 column pairs.".into());
                        }
                    }
                }
                // User-defined expansion, file/HTML commands, and custom array
                // spacing are outside a drawing's math expression. Reject them
                // before expansion; upstream separately caps depth at 32 and
                // built-in macro expansion at 1,000 operations.
                if matches!(
                    command,
                    "def"
                        | "gdef"
                        | "edef"
                        | "xdef"
                        | "let"
                        | "futurelet"
                        | "newcommand"
                        | "renewcommand"
                        | "providecommand"
                        | "global"
                        | "csname"
                        | "endcsname"
                        | "catcode"
                        | "expandafter"
                        | "input"
                        | "include"
                        | "includegraphics"
                        | "htmlClass"
                        | "htmlId"
                        | "htmlStyle"
                        | "htmlData"
                        | "html@mathml"
                ) {
                    return Err(format!("\\{command} is not supported in a math text box."));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn literal_argument(
    characters: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
) -> Result<String, String> {
    loop {
        match characters.peek().map(|&(_, character)| character) {
            Some(character) if character.is_whitespace() => {
                characters.next();
            }
            Some('%') => {
                for (_, character) in characters.by_ref() {
                    if character == '\n' {
                        break;
                    }
                }
            }
            _ => break,
        }
    }
    if !matches!(characters.next(), Some((_, '{'))) {
        return Err(
            "An environment needs a literal name in braces, such as \\begin{matrix}.".into(),
        );
    }
    let mut argument = String::new();
    for (_, character) in characters.by_ref() {
        match character {
            '}' => return Ok(argument),
            '{' | '\\' | '%' => break,
            _ => argument.push(character),
        }
    }
    Err("Use literal environment names and column counts inside matching braces.".into())
}

fn validate_ast(ast: &[ratex_parser::parse_node::ParseNode]) -> Result<(), String> {
    // RaTeX exposes its entire AST as serde data. Walking that public structure
    // keeps resource checks exhaustive without duplicating dozens of variants.
    struct Bounded(Vec<u8>);
    impl Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.0.len().saturating_add(bytes.len()) > MAX_AST_BYTES {
                return Err(std::io::Error::other("The equation is too complex."));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut encoded = Bounded(Vec::new());
    serde_json::to_writer(&mut encoded, ast).map_err(|error| error.to_string())?;
    let value: serde_json::Value =
        serde_json::from_slice(&encoded.0).map_err(|error| error.to_string())?;
    let mut pending = vec![&value];
    let mut nodes = 0;
    let mut array_cells = 0usize;
    while let Some(value) = pending.pop() {
        match value {
            serde_json::Value::Array(values) => pending.extend(values),
            serde_json::Value::Object(fields) => {
                if fields.get("type").and_then(|value| value.as_str()) == Some("array") {
                    if let Some(rows) = fields.get("body").and_then(|value| value.as_array()) {
                        let columns = rows
                            .iter()
                            .filter_map(|row| row.as_array().map(Vec::len))
                            .max()
                            .unwrap_or(0)
                            .max(
                                fields
                                    .get("cols")
                                    .and_then(|value| value.as_array())
                                    .map_or(0, Vec::len),
                            );
                        // The layout engine pads ragged rows to a rectangle.
                        // Bound the implicit empty cells as well as AST nodes.
                        array_cells =
                            array_cells.saturating_add(rows.len().saturating_mul(columns));
                        if array_cells > MAX_NODES {
                            return Err(
                                "Matrices and aligned equations exceed the 4,096-cell limit."
                                    .into(),
                            );
                        }
                    }
                }
                if fields.contains_key("type") {
                    nodes += 1;
                    if nodes > MAX_NODES {
                        return Err("The equation exceeds the 4,096-node complexity limit.".into());
                    }
                }
                if fields.contains_key("unit") {
                    let number = fields.get("number").and_then(|value| value.as_f64());
                    if number.is_none_or(|value| !value.is_finite() || value.abs() > 512.0) {
                        return Err("An equation spacing or rule dimension is too large.".into());
                    }
                }
                pending.extend(fields.values());
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
