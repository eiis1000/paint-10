use flate2::{write::ZlibEncoder, Compression};
use image::RgbaImage;
use std::io::Write;

const POINTS_PER_MM: f32 = 72.0 / 25.4;
pub const MAX_PAGES: u32 = 100;
/// Acrobat's interoperable PDF 1.4 page limit: 14,400 points, or 200 inches.
/// See PDF Reference 1.4, Appendix C, page 707.
pub const MAX_PAPER_MM: f32 = 5080.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum PaperSize {
    #[default]
    Letter,
    Legal,
    Tabloid,
    Executive,
    Statement,
    A0,
    A1,
    A2,
    A3,
    A4,
    A5,
    A6,
    B4,
    B5,
    Photo4x6,
    Photo5x7,
    Envelope10,
    EnvelopeDl,
    EnvelopeC5,
    Custom {
        width_mm: f32,
        height_mm: f32,
    },
}

impl PaperSize {
    pub const PRESETS: [Self; 19] = [
        Self::Letter,
        Self::Legal,
        Self::Tabloid,
        Self::Executive,
        Self::Statement,
        Self::A0,
        Self::A1,
        Self::A2,
        Self::A3,
        Self::A4,
        Self::A5,
        Self::A6,
        Self::B4,
        Self::B5,
        Self::Photo4x6,
        Self::Photo5x7,
        Self::Envelope10,
        Self::EnvelopeDl,
        Self::EnvelopeC5,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Letter => "Letter",
            Self::Legal => "Legal",
            Self::Tabloid => "Tabloid / Ledger",
            Self::Executive => "Executive",
            Self::Statement => "Statement",
            Self::A0 => "A0",
            Self::A1 => "A1",
            Self::A2 => "A2",
            Self::A3 => "A3",
            Self::A4 => "A4",
            Self::A5 => "A5",
            Self::A6 => "A6",
            Self::B4 => "B4 (ISO)",
            Self::B5 => "B5 (ISO)",
            Self::Photo4x6 => "Photo 4 × 6 in",
            Self::Photo5x7 => "Photo 5 × 7 in",
            Self::Envelope10 => "Envelope #10",
            Self::EnvelopeDl => "Envelope DL",
            Self::EnvelopeC5 => "Envelope C5",
            Self::Custom { .. } => "Custom",
        }
    }

    /// Base sheet dimensions before the separate Landscape option is applied.
    pub fn dimensions_mm(self) -> (f32, f32) {
        match self {
            Self::Letter => (215.9, 279.4),
            Self::Legal => (215.9, 355.6),
            Self::Tabloid => (279.4, 431.8),
            Self::Executive => (184.15, 266.7),
            Self::Statement => (139.7, 215.9),
            Self::A0 => (841.0, 1189.0),
            Self::A1 => (594.0, 841.0),
            Self::A2 => (420.0, 594.0),
            Self::A3 => (297.0, 420.0),
            Self::A4 => (210.0, 297.0),
            Self::A5 => (148.0, 210.0),
            Self::A6 => (105.0, 148.0),
            Self::B4 => (250.0, 353.0),
            Self::B5 => (176.0, 250.0),
            Self::Photo4x6 => (101.6, 152.4),
            Self::Photo5x7 => (127.0, 177.8),
            Self::Envelope10 => (104.775, 241.3),
            Self::EnvelopeDl => (110.0, 220.0),
            Self::EnvelopeC5 => (162.0, 229.0),
            Self::Custom {
                width_mm,
                height_mm,
            } => (width_mm, height_mm),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PageSettings {
    pub landscape: bool,
    pub paper: PaperSize,
    pub margin_left_mm: f32,
    pub margin_right_mm: f32,
    pub margin_top_mm: f32,
    pub margin_bottom_mm: f32,
    pub fit: bool,
    pub fit_across: u32,
    pub fit_down: u32,
    pub scale: f32,
    pub dpi_x: f32,
    pub dpi_y: f32,
    pub center_h: bool,
    pub center_v: bool,
}

impl Default for PageSettings {
    fn default() -> Self {
        Self {
            landscape: false,
            paper: PaperSize::Letter,
            margin_left_mm: 12.7,
            margin_right_mm: 12.7,
            margin_top_mm: 12.7,
            margin_bottom_mm: 12.7,
            fit: true,
            fit_across: 1,
            fit_down: 1,
            scale: 100.0,
            dpi_x: 96.0,
            dpi_y: 96.0,
            center_h: true,
            center_v: true,
        }
    }
}

/// All distances are PDF points (1/72 inch), measured from the upper left.
/// PDF output and the interactive preview use this same layout.
#[derive(Clone, Copy, Debug)]
pub struct PrintLayout {
    pub paper_width: f32,
    pub paper_height: f32,
    pub left: f32,
    pub top: f32,
    pub printable_width: f32,
    pub printable_height: f32,
    pub image_width: f32,
    pub image_height: f32,
    pub columns: u32,
    pub rows: u32,
    offset_x: f32,
    offset_y: f32,
}

impl PrintLayout {
    pub fn page_count(&self) -> u32 {
        self.columns * self.rows
    }

    /// Image origin on a numbered page, ordered left-to-right then top-to-bottom.
    pub fn image_origin(&self, page: u32) -> (f32, f32) {
        (
            self.left + self.offset_x - (page % self.columns) as f32 * self.printable_width,
            self.top + self.offset_y - (page / self.columns) as f32 * self.printable_height,
        )
    }
}

impl PageSettings {
    pub fn set_resolution(&mut self, x: f32, y: f32) {
        self.dpi_x = x;
        self.dpi_y = y;
    }

    pub fn paper_points(&self) -> Result<(f32, f32), String> {
        let (width_mm, height_mm) = self.paper.dimensions_mm();
        if [width_mm, height_mm]
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return Err("Paper width and height must be positive, finite numbers.".into());
        }
        if width_mm > MAX_PAPER_MM || height_mm > MAX_PAPER_MM {
            return Err("Paper width and height cannot exceed 5080 mm (200 inches).".into());
        }
        let (w, h) = (width_mm * POINTS_PER_MM, height_mm * POINTS_PER_MM);
        Ok(if self.landscape { (h, w) } else { (w, h) })
    }

    pub fn layout(&self, img: &RgbaImage) -> Result<PrintLayout, String> {
        if img.width() == 0 || img.height() == 0 {
            return Err("The image is empty.".into());
        }
        if !self.dpi_x.is_finite()
            || !self.dpi_y.is_finite()
            || self.dpi_x <= 0.0
            || self.dpi_y <= 0.0
        {
            return Err("Image resolution must be a positive DPI value.".into());
        }
        let natural_width = img.width() as f32 * 72.0 / self.dpi_x;
        let natural_height = img.height() as f32 * 72.0 / self.dpi_y;
        if !natural_width.is_finite() || !natural_height.is_finite() {
            return Err("Image resolution produces an unrepresentable print size.".into());
        }
        let (paper_width, paper_height) = self.paper_points()?;
        let margins = [
            self.margin_left_mm,
            self.margin_right_mm,
            self.margin_top_mm,
            self.margin_bottom_mm,
        ];
        if margins.iter().any(|v| !v.is_finite() || *v < 0.0) {
            return Err("Margins must be non-negative numbers.".into());
        }
        let [left, right, top, bottom] = margins.map(|v| v * POINTS_PER_MM);
        let printable_width = paper_width - left - right;
        let printable_height = paper_height - top - bottom;
        if printable_width < 1.0 || printable_height < 1.0 {
            return Err("The margins leave no printable area. Reduce the margins.".into());
        }
        let factor = if self.fit {
            if self.fit_across == 0
                || self.fit_down == 0
                || self.fit_across > MAX_PAGES
                || self.fit_down > MAX_PAGES
            {
                return Err("Fit-to-page counts must be between 1 and 100.".into());
            }
            (printable_width * self.fit_across as f32 / natural_width)
                .min(printable_height * self.fit_down as f32 / natural_height)
        } else {
            if !self.scale.is_finite() || self.scale <= 0.0 {
                return Err("Print scale must be greater than zero.".into());
            }
            self.scale / 100.0
        };
        let image_width = natural_width * factor;
        let image_height = natural_height * factor;
        if !image_width.is_finite()
            || !image_height.is_finite()
            || image_width <= 0.0
            || image_height <= 0.0
        {
            return Err(
                "The scaled image has an invalid print size. Adjust the print scale.".into(),
            );
        }
        // Floating-point error at an exact page boundary must not create a blank page.
        let columns = ((image_width / printable_width - 0.00001).ceil().max(1.0)) as u32;
        let rows = ((image_height / printable_height - 0.00001).ceil().max(1.0)) as u32;
        if columns
            .checked_mul(rows)
            .is_none_or(|count| count > MAX_PAGES)
        {
            return Err(
                "The print layout exceeds 100 pages. Reduce the scale or page counts.".into(),
            );
        }
        Ok(PrintLayout {
            paper_width,
            paper_height,
            left,
            top,
            printable_width,
            printable_height,
            image_width,
            image_height,
            columns,
            rows,
            offset_x: if self.center_h {
                (columns as f32 * printable_width - image_width).max(0.0) / 2.0
            } else {
                0.0
            },
            offset_y: if self.center_v {
                (rows as f32 * printable_height - image_height).max(0.0) / 2.0
            } else {
                0.0
            },
        })
    }
}

/// A lossless RGB XObject is shared across tiled pages. Transparent pixels are
/// composited over paper white, preserving fine pixel art without JPEG artifacts.
pub fn pdf(img: &RgbaImage, settings: &PageSettings) -> Result<Vec<u8>, String> {
    let layout = settings.layout(img)?;
    let mut rgb = ZlibEncoder::new(Vec::new(), Compression::default());
    let mut row = vec![0; img.width() as usize * 3];
    for pixels in img.rows() {
        for (pixel, output) in pixels.zip(row.as_chunks_mut::<3>().0) {
            let alpha = u32::from(pixel[3]);
            for (value, result) in pixel.0[..3].iter().zip(output) {
                *result = ((u32::from(*value) * alpha + 255 * (255 - alpha) + 127) / 255) as u8;
            }
        }
        rgb.write_all(&row).map_err(|e| e.to_string())?;
    }
    let compressed = rgb.finish().map_err(|e| e.to_string())?;
    let mut objects: Vec<Vec<u8>> = vec![b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()];
    let kids = (0..layout.page_count())
        .map(|n| format!("{} 0 R", 4 + n * 2))
        .collect::<Vec<_>>()
        .join(" ");
    objects.push(
        format!(
            "<< /Type /Pages /Count {} /Kids [{kids}] >>",
            layout.page_count()
        )
        .into_bytes(),
    );
    let mut image = format!("<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream\n", img.width(), img.height(), compressed.len()).into_bytes();
    image.extend(compressed);
    image.extend(b"\nendstream");
    objects.push(image);
    for page in 0..layout.page_count() {
        let page_id = objects.len() + 1;
        objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.5} {:.5}] /Resources << /XObject << /Im0 3 0 R >> >> /Contents {} 0 R >>", layout.paper_width, layout.paper_height, page_id + 1).into_bytes());
        let (x, top) = layout.image_origin(page);
        // PDF uses a lower-left origin; the shared layout uses an upper-left origin.
        let y = layout.paper_height - top - layout.image_height;
        let clip_y = layout.paper_height - layout.top - layout.printable_height;
        let stream = format!(
            "q {:.5} {clip_y:.5} {:.5} {:.5} re W n {:.5} 0 0 {:.5} {x:.5} {y:.5} cm /Im0 Do Q",
            layout.left,
            layout.printable_width,
            layout.printable_height,
            layout.image_width,
            layout.image_height
        );
        objects.push(
            format!(
                "<< /Length {} >>\nstream\n{stream}\nendstream",
                stream.len()
            )
            .into_bytes(),
        );
    }
    let mut out = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = vec![0];
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(out.len());
        writeln!(&mut out, "{} 0 obj", i + 1).unwrap();
        out.extend(obj);
        out.extend(b"\nendobj\n");
    }
    let xref = out.len();
    write!(&mut out, "xref\n0 {}\n0000000000 65535 f \n", offsets.len()).unwrap();
    for offset in offsets.iter().skip(1) {
        writeln!(&mut out, "{offset:010} 00000 n ").unwrap();
    }
    write!(
        &mut out,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        offsets.len()
    )
    .unwrap();
    Ok(out)
}

#[cfg(target_os = "linux")]
pub fn print(img: &RgbaImage, settings: &PageSettings) -> Result<(), String> {
    use std::io::Seek;

    let bytes = pdf(img, settings)?;
    let mut file = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    file.as_file_mut().rewind().map_err(|e| e.to_string())?;
    pollster::block_on(async {
        let proxy = ashpd::desktop::print::PrintProxy::new()
            .await
            .map_err(|e| e.to_string())?;
        proxy
            .print(None, "Print - Paint 10", file.as_file(), None, true)
            .await
            .map_err(|e| e.to_string())?
            .response()
            .map_err(|e| e.to_string())
    })
}

#[cfg(all(not(target_os = "linux"), not(target_arch = "wasm32")))]
pub fn print(img: &RgbaImage, settings: &PageSettings) -> Result<(), String> {
    let bytes = pdf(img, settings)?;
    let path = rfd::FileDialog::new()
        .set_title("Save printable PDF — Paint 10")
        .add_filter("PDF document", &["pdf"])
        .set_file_name("Paint 10.pdf")
        .save_file()
        .ok_or("Print canceled")?;
    crate::project::atomic_write(&path, &bytes)
}

/// Browser printing uses the same paginated PDF as the native preview.
#[cfg(target_arch = "wasm32")]
pub fn print(img: &RgbaImage, settings: &PageSettings) -> Result<(), String> {
    let bytes = pdf(img, settings)?;
    crate::web::download("Paint10.pdf", "application/pdf", &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> RgbaImage {
        RgbaImage::from_pixel(900, 600, image::Rgba([255; 4]))
    }

    #[test]
    fn actual_size_uses_the_images_physical_resolution() {
        let image = RgbaImage::new(300, 600);
        let mut settings = PageSettings {
            fit: false,
            ..Default::default()
        };
        settings.set_resolution(300.0, 600.0);
        let layout = settings.layout(&image).unwrap();
        assert_eq!((layout.image_width, layout.image_height), (72.0, 72.0));
    }

    #[test]
    fn common_paper_sizes_have_correct_physical_dimensions_in_both_orientations() {
        for (paper, expected) in [
            (PaperSize::Legal, (612.0, 1008.0)),
            (PaperSize::Tabloid, (792.0, 1224.0)),
            (PaperSize::Photo4x6, (288.0, 432.0)),
            (PaperSize::A3, (841.8898, 1190.5512)),
        ] {
            for landscape in [false, true] {
                let settings = PageSettings {
                    paper,
                    landscape,
                    ..Default::default()
                };
                let layout = settings.layout(&sample()).unwrap();
                let (width, height) = if landscape {
                    (expected.1, expected.0)
                } else {
                    expected
                };
                assert!((layout.paper_width - width).abs() < 0.001);
                assert!((layout.paper_height - height).abs() < 0.001);
            }
        }
    }

    #[test]
    fn legal_paper_changes_tiling_without_changing_actual_image_size() {
        let image = RgbaImage::new(1200, 1200);
        let mut settings = PageSettings {
            fit: false,
            dpi_x: 100.0,
            dpi_y: 100.0,
            ..Default::default()
        };
        let letter = settings.layout(&image).unwrap();
        assert_eq!((letter.columns, letter.rows), (2, 2));
        settings.paper = PaperSize::Legal;
        let legal = settings.layout(&image).unwrap();
        assert_eq!((legal.columns, legal.rows), (2, 1));
        assert_eq!((legal.image_width, legal.image_height), (864.0, 864.0));
    }

    #[test]
    fn custom_paper_preserves_asymmetric_margins_centering_and_axis_dpi() {
        let image = RgbaImage::new(200, 400);
        let mut settings = PageSettings {
            paper: PaperSize::Custom {
                width_mm: 100.0,
                height_mm: 160.0,
            },
            margin_left_mm: 5.0,
            margin_right_mm: 15.0,
            margin_top_mm: 20.0,
            margin_bottom_mm: 10.0,
            dpi_x: 254.0,
            dpi_y: 508.0,
            fit: false,
            scale: 300.0,
            ..Default::default()
        };
        for (landscape, origin_mm) in [(false, (15.0, 55.0)), (true, (45.0, 25.0))] {
            settings.landscape = landscape;
            let layout = settings.layout(&image).unwrap();
            assert_eq!(layout.page_count(), 1);
            assert!((layout.image_width / POINTS_PER_MM - 60.0).abs() < 0.001);
            assert!((layout.image_height / POINTS_PER_MM - 60.0).abs() < 0.001);
            let origin = layout.image_origin(0);
            assert!((origin.0 / POINTS_PER_MM - origin_mm.0).abs() < 0.001);
            assert!((origin.1 / POINTS_PER_MM - origin_mm.1).abs() < 0.001);
        }
    }

    #[test]
    fn custom_paper_pdf_uses_selected_media_box_on_every_tiled_page() {
        let settings = PageSettings {
            paper: PaperSize::Custom {
                width_mm: 70.0,
                height_mm: 100.0,
            },
            landscape: true,
            fit: false,
            dpi_x: 300.0,
            dpi_y: 300.0,
            margin_left_mm: 5.0,
            margin_right_mm: 5.0,
            margin_top_mm: 5.0,
            margin_bottom_mm: 5.0,
            ..Default::default()
        };
        let image = RgbaImage::new(2000, 2000);
        let layout = settings.layout(&image).unwrap();
        assert_eq!((layout.columns, layout.rows), (2, 3));
        let bytes = pdf(&image, &settings).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        let boxes: Vec<_> = text.split("/MediaBox [").skip(1).collect();
        assert_eq!(boxes.len(), 6);
        for value in boxes {
            let coordinates: Vec<f64> = value
                .split(']')
                .next()
                .unwrap()
                .split_whitespace()
                .map(|number| number.parse().unwrap())
                .collect();
            assert_eq!(&coordinates[..2], &[0.0, 0.0]);
            assert!((coordinates[2] - 100.0 * 72.0 / 25.4).abs() < 0.001);
            assert!((coordinates[3] - 70.0 * 72.0 / 25.4).abs() < 0.001);
        }
    }

    #[test]
    fn invalid_custom_paper_is_rejected_before_pdf_encoding() {
        let image = RgbaImage::new(1, 1);
        for invalid in [0.0, -1.0, f32::NAN, f32::INFINITY, MAX_PAPER_MM + 1.0] {
            for (width_mm, height_mm) in [(invalid, 100.0), (100.0, invalid)] {
                let settings = PageSettings {
                    paper: PaperSize::Custom {
                        width_mm,
                        height_mm,
                    },
                    ..Default::default()
                };
                assert!(settings.paper_points().is_err());
                assert!(pdf(&image, &settings).unwrap_err().starts_with("Paper"));
            }
        }
        let settings = PageSettings {
            paper: PaperSize::Custom {
                width_mm: 20.0,
                height_mm: 30.0,
            },
            ..Default::default()
        };
        assert!(settings.layout(&image).unwrap_err().contains("margins"));
    }

    #[test]
    fn fit_does_not_add_blank_pages_at_exact_boundaries() {
        for paper in PaperSize::PRESETS {
            for landscape in [false, true] {
                let settings = PageSettings {
                    paper,
                    landscape,
                    ..Default::default()
                };
                let layout = settings.layout(&sample()).unwrap();
                assert_eq!((layout.columns, layout.rows), (1, 1));
                assert!(layout.image_width <= layout.printable_width + 0.001);
                assert!(layout.image_height <= layout.printable_height + 0.001);
            }
        }
    }

    #[test]
    fn asymmetric_margins_and_page_offsets_match() {
        let settings = PageSettings {
            fit: false,
            scale: 200.0,
            margin_left_mm: 10.0,
            margin_right_mm: 20.0,
            margin_top_mm: 30.0,
            margin_bottom_mm: 40.0,
            center_h: false,
            center_v: false,
            ..Default::default()
        };
        let layout = settings.layout(&sample()).unwrap();
        assert_eq!((layout.columns, layout.rows), (3, 2));
        let (x0, y0) = layout.image_origin(0);
        let (x1, y1) = layout.image_origin(1);
        let (x3, y3) = layout.image_origin(3);
        assert!((x0 - 10.0 * POINTS_PER_MM).abs() < 0.001);
        assert!((y0 - 30.0 * POINTS_PER_MM).abs() < 0.001);
        assert!((x1 + layout.printable_width - x0).abs() < 0.001);
        assert!((y3 + layout.printable_height - y0).abs() < 0.001);
        assert_eq!(x0, x3);
        assert_eq!(y0, y1);
    }

    #[test]
    fn fit_across_and_down_preserves_aspect_ratio() {
        let settings = PageSettings {
            fit_across: 2,
            fit_down: 3,
            ..Default::default()
        };
        let layout = settings.layout(&sample()).unwrap();
        assert_eq!((layout.columns, layout.rows), (2, 1));
        assert!((layout.image_width / layout.image_height - 1.5).abs() < 0.0001);
    }

    #[test]
    fn invalid_settings_fail_before_pdf_encoding() {
        for settings in [
            PageSettings {
                margin_left_mm: 300.0,
                ..Default::default()
            },
            PageSettings {
                margin_top_mm: f32::NAN,
                ..Default::default()
            },
            PageSettings {
                fit_across: 0,
                ..Default::default()
            },
            PageSettings {
                fit: false,
                scale: f32::INFINITY,
                ..Default::default()
            },
            PageSettings {
                fit: false,
                scale: 1e20,
                ..Default::default()
            },
        ] {
            assert!(pdf(&sample(), &settings).is_err());
        }
    }

    #[test]
    fn pdf_page_count_and_cross_references_are_valid() {
        let bytes = pdf(
            &sample(),
            &PageSettings {
                fit: false,
                scale: 200.0,
                ..Default::default()
            },
        )
        .unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/Count 6"));
        assert!(text.contains("/Filter /FlateDecode"));
        assert!(text.ends_with("%%EOF\n"));
        let xref_offset = text
            .split("startxref\n")
            .nth(1)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .parse::<usize>()
            .unwrap();
        assert_eq!(&bytes[xref_offset..xref_offset + 4], b"xref");
        let xref_text = std::str::from_utf8(&bytes[xref_offset..]).unwrap();
        for (object, line) in xref_text
            .lines()
            .skip(3)
            .take_while(|line| line.ends_with(" n "))
            .enumerate()
        {
            let offset = line[..10].parse::<usize>().unwrap();
            assert!(bytes[offset..].starts_with(format!("{} 0 obj\n", object + 1).as_bytes()));
        }
    }

    #[test]
    fn pdf_flattens_alpha_to_white_without_loss() {
        use std::io::Read;
        let img = RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 0, 0]).unwrap();
        let bytes = pdf(&img, &PageSettings::default()).unwrap();
        let stream = bytes.windows(7).position(|s| s == b"stream\n").unwrap() + 7;
        let mut rgb = vec![];
        flate2::read::ZlibDecoder::new(&bytes[stream..])
            .read_to_end(&mut rgb)
            .unwrap();
        assert_eq!(rgb, [255, 0, 0, 255, 255, 255]);
    }
}
