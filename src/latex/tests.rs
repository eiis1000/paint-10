use super::*;
use crate::document::{Document, Object, ObjectKind};

fn math_format() -> TextFormat {
    TextFormat {
        latex: true,
        size: 40.0,
        width: 80,
        ..Default::default()
    }
}

#[test]
fn standard_math_renders_inside_measured_bounds_without_edge_clipping() {
    let format = math_format();
    for source in [
        r"\frac{-b\pm\sqrt{b^2-4ac}}{2a}",
        r"\sum_{n=1}^{\infty}\frac{1}{n^2}=\frac{\pi^2}{6}",
        r"\alpha+\beta=\theta\quad\Gamma\Delta\Omega",
        r"\begin{pmatrix}a&b\\c&d\end{pmatrix}",
        r"\begin{cases}x^2&x\ge0\\-x&x<0\end{cases}",
        r"\begin{aligned}a&=b+c\\d&=e+f\end{aligned}",
    ] {
        let image = render(source, &format).unwrap();
        assert_eq!(image.dimensions(), dimensions(source, &format).unwrap());
        assert!(image.pixels().any(|pixel| pixel[3] > 0), "{source}");
        for (x, y, pixel) in image.enumerate_pixels() {
            if x == 0 || y == 0 || x + 1 == image.width() || y + 1 == image.height() {
                assert_eq!(pixel[3], 0, "clipped ink at {x},{y} for {source}");
            }
        }
        assert_eq!(image, format.render(source));
    }
}

#[test]
fn invalid_or_excessive_source_is_rejected_before_becoming_an_object() {
    let format = math_format();
    for source in [
        "".into(),
        "% only a comment".into(),
        r"\frac{1}".into(),
        r"\unknowncommand{x}".into(),
        r"\left(x".into(),
        r"\begin{matrix}1&2".into(),
        r"\def\x{\x}\x".into(),
        r"\includegraphics{example.png}".into(),
        r"\rule{999999em}{1em}".into(),
        r"\text{你好}".into(),
        r"\begin{alignat}{999999999}a&=b\end{alignat}".into(),
        format!("{}x{}", "{".repeat(40), "}".repeat(40)),
        "x".repeat(MAX_SOURCE_BYTES + 1),
    ] {
        assert!(dimensions(&source, &format).is_err(), "{source}");
        assert!(format.validate_for_text(&source).is_err(), "{source}");
    }
    let error = dimensions(r"x+\unknown", &format).unwrap_err();
    assert!(error.contains("position"));
    assert!(error.contains("unknown"));
    assert!(raster_dimensions(f64::NAN, 1.0, 40.0, 4.0).is_err());
    assert!(raster_dimensions(1.0, f64::INFINITY, 40.0, 4.0).is_err());
    let ragged_matrix = format!(
        r"\begin{{matrix}}{}{}\end{{matrix}}",
        "x&".repeat(100),
        r"\\x".repeat(100),
    );
    let error = dimensions(&ragged_matrix, &format).unwrap_err();
    assert!(error.contains("4,096-cell"), "{error}");
}

#[test]
fn equation_geometry_color_outline_and_background_follow_text_format() {
    let mut format = math_format();
    format.color = [230, 20, 60, 128];
    let natural = render(r"\frac{a}{b}", &format).unwrap();
    assert!(natural.pixels().all(|pixel| pixel[3] <= 128));
    assert!(natural
        .pixels()
        .any(|pixel| pixel[3] == 128 && pixel[0] > 200));

    format.width = natural.width() + 80;
    format.minimum_height = natural.height() + 20;
    format.alignment = TextAlignment::Right;
    let aligned = render(r"\frac{a}{b}", &format).unwrap();
    assert_eq!(aligned.dimensions(), (format.width, format.minimum_height));
    assert!(aligned
        .enumerate_pixels()
        .all(|(x, _, pixel)| x >= 80 || pixel[3] == 0));

    format.outline_width = 2;
    format.outline_color = [10, 100, 250, 255];
    let outlined = render(r"\frac{a}{b}", &format).unwrap();
    assert!(outlined
        .pixels()
        .any(|pixel| pixel[2] > 180 && pixel[1] > 50));
    format.background = Some([240, 230, 210, 255]);
    let opaque = render(r"\frac{a}{b}", &format).unwrap();
    assert_eq!(opaque.get_pixel(0, 0).0, [240, 230, 210, 255]);
    assert!(opaque.pixels().all(|pixel| pixel[3] == 255));

    format.width = 10;
    format.outline_width = 0;
    assert!(dimensions(r"\sum_{n=0}^{10} n^2", &format).unwrap().0 > 10);
}

#[test]
fn style_wrappers_accept_outer_delimiters_and_keep_sources_editable() {
    let mut format = math_format();
    for field in ["bold", "italic", "underline", "strikeout"] {
        match field {
            "bold" => format.bold = true,
            "italic" => format.italic = true,
            "underline" => format.underline = true,
            "strikeout" => format.strikeout = true,
            _ => unreachable!(),
        }
        assert_eq!(
            render("x^2", &format).unwrap(),
            render("$x^2$", &format).unwrap()
        );
    }
}

#[test]
fn version_five_preserves_formula_source_layers_styles_and_transforms() {
    let mut document = Document::new(480, 240);
    document.add_layer().unwrap();
    let source = r"\begin{pmatrix}\alpha&\beta\\1&2\end{pmatrix}";
    let mut format = math_format();
    format.color = [100, 20, 180, 170];
    format.outline_width = 1;
    let mut object = Object::new(
        ObjectKind::Text {
            text: source.into(),
            format,
        },
        (14, 20),
    );
    object.rotate_to(17.0).unwrap();
    object.resize_rendered(220, 130).unwrap();
    document.objects.push(object);
    let bytes = crate::project::encode(&document).unwrap();
    let mut json: serde_json::Value =
        serde_json::from_reader(flate2::read::ZlibDecoder::new(&bytes[8..])).unwrap();
    assert_eq!(json["version"], 5);
    let reopened = crate::project::decode(&bytes).unwrap();
    assert!(reopened.layers() == document.layers());
    assert_eq!(reopened.composite(), document.composite());

    json["version"] = 4.into();
    let mut encoded =
        flate2::write::ZlibEncoder::new(b"PAINT10\0".to_vec(), flate2::Compression::default());
    serde_json::to_writer(&mut encoded, &json).unwrap();
    assert!(crate::project::decode(&encoded.finish().unwrap()).is_err());
}
