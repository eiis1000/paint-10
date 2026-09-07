//! Raster file formats and bounded decoding, including Paint's indexed BMPs.

use crate::metadata::{self, Resolution};
use image::{DynamicImage, ImageDecoder, ImageFormat, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::{Cursor, Read},
    path::Path,
};

const MAX_PIXELS: u64 = 16_777_216;
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RasterFormat {
    #[default]
    Png,
    Jpeg,
    Gif,
    Tiff,
    BmpMono,
    Bmp16,
    Bmp256,
    Bmp24,
    WebP,
    Icon,
    Project,
}

impl RasterFormat {
    pub const ALL: [Self; 11] = [
        Self::Png,
        Self::Jpeg,
        Self::BmpMono,
        Self::Bmp16,
        Self::Bmp256,
        Self::Bmp24,
        Self::Gif,
        Self::Tiff,
        Self::WebP,
        Self::Icon,
        Self::Project,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Png => "PNG picture",
            Self::Jpeg => "JPEG picture",
            Self::Gif => "GIF picture",
            Self::Tiff => "TIFF picture",
            Self::BmpMono => "Monochrome bitmap",
            Self::Bmp16 => "16 color bitmap",
            Self::Bmp256 => "256 color bitmap",
            Self::Bmp24 => "24-bit bitmap",
            Self::WebP => "WebP picture",
            Self::Icon => "Windows icon",
            Self::Project => "Paint 10 editable project",
        }
    }

    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Png => &["png"],
            Self::Jpeg => &["jpg", "jpeg", "jpe", "jfif"],
            Self::Gif => &["gif"],
            Self::Tiff => &["tif", "tiff"],
            Self::BmpMono | Self::Bmp16 | Self::Bmp256 | Self::Bmp24 => &["bmp", "dib"],
            Self::WebP => &["webp"],
            Self::Icon => &["ico"],
            Self::Project => &["p10"],
        }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        // A filename alone cannot distinguish indexed BMP variants.
        if matches!(extension.as_str(), "bmp" | "dib") {
            return Some(Self::Bmp24);
        }
        Self::ALL
            .into_iter()
            .find(|format| format.extensions().contains(&extension.as_str()))
    }

    pub fn matches_path(self, path: &Path) -> bool {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                self.extensions()
                    .iter()
                    .any(|known| extension.eq_ignore_ascii_case(known))
            })
    }

    pub fn bmp_depth(self) -> Option<u16> {
        match self {
            Self::BmpMono => Some(1),
            Self::Bmp16 => Some(4),
            Self::Bmp256 => Some(8),
            Self::Bmp24 => Some(24),
            _ => None,
        }
    }

    fn image_format(self) -> Option<ImageFormat> {
        Some(match self {
            Self::Png => ImageFormat::Png,
            Self::Jpeg => ImageFormat::Jpeg,
            Self::Gif => ImageFormat::Gif,
            Self::Tiff => ImageFormat::Tiff,
            Self::BmpMono | Self::Bmp16 | Self::Bmp256 | Self::Bmp24 => ImageFormat::Bmp,
            Self::WebP => ImageFormat::WebP,
            Self::Icon => ImageFormat::Ico,
            Self::Project => return None,
        })
    }
}

/// Keep the bit depth of an existing BMP when using Save instead of Save As.
pub fn detect_format(path: &Path) -> Option<RasterFormat> {
    let fallback = RasterFormat::from_path(path);
    let mut header = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(32)
        .read_to_end(&mut header)
        .ok()?;
    let start = if header.starts_with(b"BM") {
        14
    } else if dib_header_size(&header).is_some() {
        0
    } else {
        return fallback;
    };
    let dib_size = u32::from_le_bytes(header.get(start..start + 4)?.try_into().ok()?);
    let offset = start + if dib_size == 12 { 10 } else { 14 };
    Some(
        match u16::from_le_bytes(header.get(offset..offset + 2)?.try_into().ok()?) {
            1 => RasterFormat::BmpMono,
            4 => RasterFormat::Bmp16,
            8 => RasterFormat::Bmp256,
            _ => RasterFormat::Bmp24,
        },
    )
}

fn dib_header_size(bytes: &[u8]) -> Option<u32> {
    let size = u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?);
    matches!(size, 12 | 40 | 52 | 56 | 108 | 124).then_some(size)
}

pub fn decode(path: &Path) -> Result<RgbaImage, String> {
    decode_with_resolution(path).map(|(image, _)| image)
}

pub fn decode_with_resolution(path: &Path) -> Result<(RgbaImage, Resolution), String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err("Picture files must be no larger than 256 MiB.".into());
    }
    let image = decode_bytes(&bytes)?;
    let resolution = metadata::read_resolution(&bytes)
        .or_else(|| {
            if dib_header_size(&bytes)? < 40 || bytes.len() < 40 {
                return None;
            }
            // DIB stores the same resolution fields without BMP's 14-byte file header.
            let mut header = [0_u8; 54];
            header[..2].copy_from_slice(b"BM");
            header[14..].copy_from_slice(&bytes[..40]);
            metadata::read_resolution(&header)
        })
        .unwrap_or_default();
    Ok((image, resolution))
}

fn decode_bytes(bytes: &[u8]) -> Result<RgbaImage, String> {
    if dib_header_size(bytes).is_some() {
        let mut decoder =
            image::codecs::bmp::BmpDecoder::new_without_file_header(Cursor::new(bytes))
                .map_err(|error| format!("Could not open DIB picture: {error}"))?;
        decoder
            .set_limits(decode_limits())
            .map_err(|error| error.to_string())?;
        return decode_decoder(decoder);
    }
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| error.to_string())?;
    decode_reader(reader)
}

fn decode_reader<R: std::io::BufRead + std::io::Seek>(
    mut reader: image::ImageReader<R>,
) -> Result<RgbaImage, String> {
    reader.limits(decode_limits());
    let decoder = reader
        .into_decoder()
        .map_err(|error| format!("Could not open picture: {error}"))?;
    decode_decoder(decoder)
}

fn decode_limits() -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(128 * 1024 * 1024);
    limits
}

fn decode_decoder(decoder: impl ImageDecoder) -> Result<RgbaImage, String> {
    let (width, height) = decoder.dimensions();
    validate_size(width, height)?;
    let image = DynamicImage::from_decoder(decoder)
        .map_err(|error| format!("Could not open picture: {error}"))?;
    Ok(image.to_rgba8())
}

fn validate_size(width: u32, height: u32) -> Result<(), String> {
    if width == 0
        || height == 0
        || width > 16384
        || height > 16384
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        Err("The picture exceeds the 16 megapixel canvas limit.".into())
    } else {
        Ok(())
    }
}

pub fn encode(image: &RgbaImage, format: RasterFormat) -> Result<Vec<u8>, String> {
    validate_size(image.width(), image.height())?;
    if let Some(depth) = format.bmp_depth() {
        return encode_bmp(image, depth);
    }
    if format == RasterFormat::Tiff {
        return encode_tiff(image).map_err(|error| error.to_string());
    }
    if format == RasterFormat::Icon && (image.width() > 256 || image.height() > 256) {
        return Err("Windows icons must be no larger than 256 × 256 pixels.".into());
    }
    let image_format = format
        .image_format()
        .ok_or("Use the project writer to preserve editable objects.")?;
    let raster = if format == RasterFormat::Jpeg {
        DynamicImage::ImageRgba8(opaque(image)).to_rgb8().into()
    } else {
        DynamicImage::ImageRgba8(image.clone())
    };
    let mut bytes = Cursor::new(Vec::new());
    raster
        .write_to(&mut bytes, image_format)
        .map_err(|error| error.to_string())?;
    Ok(bytes.into_inner())
}

fn encode_tiff(image: &RgbaImage) -> tiff::TiffResult<Vec<u8>> {
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut encoder = tiff::encoder::TiffEncoder::new(&mut bytes)?;
        let mut picture =
            encoder.new_image::<tiff::encoder::colortype::RGBA8>(image.width(), image.height())?;
        // tiff 0.9's RGBA color type does not describe its fourth channel.
        // Our RGB values are unpremultiplied, so TIFF requires straight alpha.
        picture
            .encoder()
            .write_tag(tiff::tags::Tag::ExtraSamples, &[2_u16][..])?;
        picture.write_data(image.as_raw())?;
    }
    Ok(bytes.into_inner())
}

pub fn encode_with_resolution(
    image: &RgbaImage,
    format: RasterFormat,
    resolution: Resolution,
) -> Result<Vec<u8>, String> {
    let bytes = encode(image, format)?;
    metadata::write_resolution(
        bytes,
        format.image_format().expect("encoded raster format"),
        resolution,
    )
}

fn opaque(image: &RgbaImage) -> RgbaImage {
    let mut result = RgbaImage::from_pixel(image.width(), image.height(), Rgba([255; 4]));
    image::imageops::overlay(&mut result, image, 0, 0);
    result
}

fn indexed_pixels(image: &RgbaImage, colors: usize) -> (Vec<[u8; 3]>, Vec<u8>) {
    let mut palette = Vec::new();
    let mut lookup = HashMap::new();
    // Preserve exact colors for pixel art that already fits the selected depth.
    for pixel in image.pixels() {
        let rgb = [pixel[0], pixel[1], pixel[2]];
        if let std::collections::hash_map::Entry::Vacant(entry) = lookup.entry(rgb) {
            if palette.len() == colors {
                break;
            }
            entry.insert(palette.len() as u8);
            palette.push(rgb);
        }
    }
    if image
        .pixels()
        .all(|pixel| lookup.contains_key(&[pixel[0], pixel[1], pixel[2]]))
    {
        let indices = image
            .pixels()
            .map(|pixel| lookup[&[pixel[0], pixel[1], pixel[2]]])
            .collect();
        palette.resize(colors, [0; 3]);
        return (palette, indices);
    }
    let quantizer = color_quant::NeuQuant::new(10, colors, image.as_raw());
    let palette = quantizer
        .color_map_rgb()
        .chunks_exact(3)
        .map(|rgb| [rgb[0], rgb[1], rgb[2]])
        .collect();
    let indices = image
        .pixels()
        .map(|pixel| quantizer.index_of(&pixel.0) as u8)
        .collect();
    (palette, indices)
}

/// Windows BITMAPINFOHEADER, uncompressed, bottom-up rows padded to four bytes.
fn encode_bmp(image: &RgbaImage, depth: u16) -> Result<Vec<u8>, String> {
    let image = opaque(image);
    let (width, height) = image.dimensions();
    let row_bytes = (u64::from(width) * u64::from(depth)).div_ceil(32) * 4;
    let pixel_bytes = row_bytes * u64::from(height);
    let (palette, indices) = match depth {
        1 => (
            vec![[0, 0, 0], [255, 255, 255]],
            image
                .pixels()
                .map(|pixel| {
                    u8::from(
                        299 * u32::from(pixel[0])
                            + 587 * u32::from(pixel[1])
                            + 114 * u32::from(pixel[2])
                            >= 128000,
                    )
                })
                .collect::<Vec<_>>(),
        ),
        4 | 8 => indexed_pixels(&image, 1_usize << depth),
        24 => (vec![], vec![]),
        _ => return Err("Unsupported bitmap depth.".into()),
    };
    let offset = 14 + 40 + palette.len() as u32 * 4;
    let length = u64::from(offset) + pixel_bytes;
    let mut out = Vec::with_capacity(length as usize);
    out.extend(b"BM");
    out.extend((length as u32).to_le_bytes());
    out.extend([0; 4]);
    out.extend(offset.to_le_bytes());
    out.extend(40_u32.to_le_bytes());
    out.extend((width as i32).to_le_bytes());
    out.extend((height as i32).to_le_bytes());
    out.extend(1_u16.to_le_bytes());
    out.extend(depth.to_le_bytes());
    out.extend(0_u32.to_le_bytes());
    out.extend((pixel_bytes as u32).to_le_bytes());
    out.extend(3780_i32.to_le_bytes()); // 96 DPI, rounded to pixels per meter.
    out.extend(3780_i32.to_le_bytes());
    out.extend((palette.len() as u32).to_le_bytes());
    out.extend(0_u32.to_le_bytes());
    for [red, green, blue] in palette {
        out.extend([blue, green, red, 0]);
    }
    let mut row = vec![0_u8; row_bytes as usize];
    for y in (0..height).rev() {
        row.fill(0);
        for x in 0..width {
            let index = (y as usize * width as usize) + x as usize;
            match depth {
                1 => row[x as usize / 8] |= indices[index] << (7 - x % 8),
                4 => row[x as usize / 2] |= indices[index] << if x % 2 == 0 { 4 } else { 0 },
                8 => row[x as usize] = indices[index],
                24 => {
                    let pixel = image.get_pixel(x, y);
                    let start = x as usize * 3;
                    row[start..start + 3].copy_from_slice(&[pixel[2], pixel[1], pixel[0]]);
                }
                _ => unreachable!(),
            }
        }
        out.extend(&row);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmap_depths_padding_and_orientation_roundtrip() {
        for width in [1, 3, 9, 17] {
            let image = RgbaImage::from_fn(width, 5, |x, y| {
                if (x + y) % 2 == 0 {
                    Rgba([0, 0, 0, 255])
                } else {
                    Rgba([255; 4])
                }
            });
            for format in [
                RasterFormat::BmpMono,
                RasterFormat::Bmp16,
                RasterFormat::Bmp256,
                RasterFormat::Bmp24,
            ] {
                let bytes = encode(&image, format).unwrap();
                assert_eq!(&bytes[..2], b"BM");
                assert_eq!(
                    u16::from_le_bytes(bytes[28..30].try_into().unwrap()),
                    format.bmp_depth().unwrap()
                );
                assert_eq!(
                    u32::from_le_bytes(bytes[2..6].try_into().unwrap()) as usize,
                    bytes.len()
                );
                assert_eq!(decode_bytes(&bytes).unwrap(), image);
            }
        }
    }

    #[test]
    fn indexed_bitmaps_keep_existing_colors_and_reduce_complex_images() {
        let image = RgbaImage::from_fn(16, 4, |x, _| Rgba([(x * 17) as u8, 23, 45, 255]));
        assert_eq!(
            decode_bytes(&encode(&image, RasterFormat::Bmp16).unwrap()).unwrap(),
            image
        );
        let image = RgbaImage::from_fn(40, 30, |x, y| {
            Rgba([(x * 6) as u8, (y * 8) as u8, ((x + y) * 3) as u8, 255])
        });
        for (format, count) in [(RasterFormat::Bmp16, 16), (RasterFormat::Bmp256, 256)] {
            let decoded = decode_bytes(&encode(&image, format).unwrap()).unwrap();
            let colors: std::collections::HashSet<_> =
                decoded.pixels().map(|pixel| pixel.0).collect();
            assert!(colors.len() <= count);
            assert_eq!(decoded.dimensions(), image.dimensions());
        }
    }

    #[test]
    fn aliases_and_lossless_formats_work() {
        assert_eq!(
            RasterFormat::from_path(Path::new("Picture.DIB")),
            Some(RasterFormat::Bmp24)
        );
        assert_eq!(
            RasterFormat::from_path(Path::new("Picture.JPE")),
            Some(RasterFormat::Jpeg)
        );
        let image = RgbaImage::from_fn(11, 7, |x, y| Rgba([x as u8, y as u8, 42, 255]));
        for format in [
            RasterFormat::Png,
            RasterFormat::Tiff,
            RasterFormat::WebP,
            RasterFormat::Icon,
        ] {
            assert_eq!(
                decode_bytes(&encode(&image, format).unwrap()).unwrap(),
                image
            );
        }
        let jpeg = encode(&image, RasterFormat::Jpeg).unwrap();
        assert_eq!(&jpeg[..2], &[0xff, 0xd8]);
        assert_eq!(
            decode_bytes(&jpeg).unwrap().dimensions(),
            image.dimensions()
        );
        let gif = encode(&image, RasterFormat::Gif).unwrap();
        assert!(gif.starts_with(b"GIF"));
        assert_eq!(decode_bytes(&gif).unwrap().dimensions(), image.dimensions());
    }

    #[test]
    fn existing_bitmap_depth_is_detected_from_header() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("example.dib");
        let image = RgbaImage::from_pixel(3, 3, Rgba([255; 4]));
        for format in [
            RasterFormat::BmpMono,
            RasterFormat::Bmp16,
            RasterFormat::Bmp256,
            RasterFormat::Bmp24,
        ] {
            std::fs::write(&path, encode(&image, format).unwrap()).unwrap();
            assert_eq!(detect_format(&path), Some(format));
        }
    }

    #[test]
    fn raw_dib_files_decode_with_their_depth_and_resolution() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("example.dib");
        let image = RgbaImage::from_fn(3, 2, |x, y| {
            if x == y {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255; 4])
            }
        });
        let resolution = Resolution { x: 300.0, y: 150.0 };
        for format in [
            RasterFormat::BmpMono,
            RasterFormat::Bmp16,
            RasterFormat::Bmp256,
            RasterFormat::Bmp24,
        ] {
            let bmp = encode_with_resolution(&image, format, resolution).unwrap();
            std::fs::write(&path, &bmp[14..]).unwrap();
            let (decoded, actual) = decode_with_resolution(&path).unwrap();
            assert_eq!(decoded, image);
            assert_eq!(detect_format(&path), Some(format));
            assert!((actual.x - resolution.x).abs() < 0.1);
            assert!((actual.y - resolution.y).abs() < 0.1);
        }
    }

    #[test]
    fn oversized_dimensions_are_rejected_before_decoding_pixels() {
        let image = RgbaImage::from_pixel(1, 1, Rgba([255; 4]));
        let mut bytes = encode(&image, RasterFormat::Bmp24).unwrap();
        bytes[18..22].copy_from_slice(&8192_i32.to_le_bytes());
        bytes[22..26].copy_from_slice(&8192_i32.to_le_bytes());
        assert!(decode_bytes(&bytes).unwrap_err().contains("16 megapixel"));
    }

    #[test]
    fn saving_and_loading_keep_physical_resolution() {
        let directory = tempfile::tempdir().unwrap();
        let image = RgbaImage::from_pixel(3, 2, Rgba([50, 90, 180, 255]));
        let resolution = Resolution { x: 300.0, y: 150.0 };
        for format in [
            RasterFormat::Png,
            RasterFormat::Jpeg,
            RasterFormat::Tiff,
            RasterFormat::Bmp24,
            RasterFormat::Bmp16,
        ] {
            let path = directory
                .path()
                .join(format!("picture.{}", format.extensions()[0]));
            std::fs::write(
                &path,
                encode_with_resolution(&image, format, resolution).unwrap(),
            )
            .unwrap();
            let (decoded, actual) = decode_with_resolution(&path).unwrap();
            assert_eq!(decoded.dimensions(), image.dimensions());
            assert!(
                (actual.x - resolution.x).abs() < 0.1,
                "{format:?}: {actual:?}"
            );
            assert!(
                (actual.y - resolution.y).abs() < 0.1,
                "{format:?}: {actual:?}"
            );
        }
    }

    #[test]
    fn tiff_identifies_straight_alpha_and_keeps_it_when_dpi_changes() {
        let image = RgbaImage::from_fn(16, 16, |x, y| {
            Rgba([x as u8 * 17, y as u8 * 17, 91, (x + y * 16) as u8])
        });
        let resolution = Resolution {
            x: 300.125,
            y: 150.5,
        };
        for bytes in [
            encode(&image, RasterFormat::Tiff).unwrap(),
            encode_with_resolution(&image, RasterFormat::Tiff, resolution).unwrap(),
        ] {
            let mut decoder = tiff::decoder::Decoder::new(Cursor::new(&bytes)).unwrap();
            assert_eq!(
                decoder
                    .get_tag_u32_vec(tiff::tags::Tag::ExtraSamples)
                    .unwrap(),
                [2]
            );
            assert_eq!(decode_bytes(&bytes).unwrap(), image);
            let updated = metadata::write_resolution(bytes, ImageFormat::Tiff, resolution).unwrap();
            assert_eq!(metadata::read_resolution(&updated), Some(resolution));
            assert_eq!(decode_bytes(&updated).unwrap(), image);
            let mut decoder = tiff::decoder::Decoder::new(Cursor::new(updated)).unwrap();
            assert_eq!(
                decoder
                    .get_tag_u32_vec(tiff::tags::Tag::ExtraSamples)
                    .unwrap(),
                [2]
            );
        }
    }

    #[test]
    fn lossless_exports_preserve_pixel_art_rgba() {
        let image = RgbaImage::from_fn(16, 8, |x, y| {
            Rgba([
                x as u8 * 17,
                y as u8 * 31,
                91,
                [0, 64, 128, 255][(x % 4) as usize],
            ])
        });
        for format in [
            RasterFormat::Png,
            RasterFormat::Tiff,
            RasterFormat::WebP,
            RasterFormat::Icon,
        ] {
            let bytes = encode_with_resolution(&image, format, Resolution::default()).unwrap();
            let decoded = decode_bytes(&bytes).unwrap();
            assert_eq!(decoded, image, "{format:?} altered pixel colors or alpha");
        }
    }

    #[test]
    fn indexed_exports_preserve_exact_palette_colors_and_gif_transparency() {
        for (format, count) in [
            (RasterFormat::Bmp16, 16),
            (RasterFormat::Bmp256, 256),
            (RasterFormat::Gif, 256),
        ] {
            let image = RgbaImage::from_fn(count, 3, |x, y| {
                let color = (x + y) % count;
                Rgba([color as u8, (color * 31) as u8, (color * 73) as u8, 255])
            });
            let decoded = decode_bytes(&encode(&image, format).unwrap()).unwrap();
            assert_eq!(
                decoded, image,
                "{format:?} changed an existing palette color"
            );
        }
        let image = RgbaImage::from_fn(12, 5, |x, y| {
            if (x + y) % 4 == 0 {
                Rgba([0; 4])
            } else {
                Rgba([x as u8 * 20, y as u8 * 40, 80, 255])
            }
        });
        let decoded = decode_bytes(&encode(&image, RasterFormat::Gif).unwrap()).unwrap();
        assert_eq!(decoded, image);
    }

    #[test]
    fn opaque_exports_composite_transparency_over_white() {
        let image = RgbaImage::from_fn(32, 16, |x, _| {
            if x < 16 {
                Rgba([0, 0, 0, 0])
            } else {
                Rgba([0, 0, 0, 255])
            }
        });
        for format in [
            RasterFormat::BmpMono,
            RasterFormat::Bmp16,
            RasterFormat::Bmp256,
            RasterFormat::Bmp24,
            RasterFormat::Jpeg,
        ] {
            let decoded = decode_bytes(&encode(&image, format).unwrap()).unwrap();
            assert_eq!(decoded.dimensions(), image.dimensions());
            for channel in 0..3 {
                assert!(
                    decoded.get_pixel(4, 8)[channel] >= 252,
                    "{format:?} lost the white matte"
                );
                assert!(
                    decoded.get_pixel(24, 8)[channel] <= 3,
                    "{format:?} altered opaque black"
                );
            }
            assert!(decoded.pixels().all(|pixel| pixel[3] == 255));
        }
    }
}
