//! Physical pixel dimensions in PNG pHYs, JPEG JFIF/Exif, BMP, and TIFF 6.0.
//! These helpers change resolution tags without recompressing raster data.
use image::ImageFormat;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Resolution {
    pub x: f32,
    pub y: f32,
}

impl Default for Resolution {
    fn default() -> Self {
        Self { x: 96.0, y: 96.0 }
    }
}

impl Resolution {
    pub fn new(x: f32, y: f32) -> Option<Self> {
        let resolution = Self { x, y };
        resolution.validate().ok().map(|()| resolution)
    }

    pub fn validate(self) -> Result<(), String> {
        if [self.x, self.y]
            .iter()
            .all(|value| value.is_finite() && (0.01..=65535.0).contains(value))
        {
            Ok(())
        } else {
            Err("Image resolution must be between 0.01 and 65,535 DPI.".into())
        }
    }

    pub fn pixels_per_unit(self, unit: u8) -> (f64, f64) {
        match unit {
            1 => (self.x as f64, self.y as f64),
            2 => (self.x as f64 / 2.54, self.y as f64 / 2.54),
            _ => (1.0, 1.0),
        }
    }
}

pub fn read_resolution(bytes: &[u8]) -> Option<Resolution> {
    match image::guess_format(bytes).ok()? {
        ImageFormat::Png => read_png(bytes),
        ImageFormat::Jpeg => read_jpeg(bytes),
        ImageFormat::Bmp => read_bmp(bytes),
        ImageFormat::Tiff => read_tiff(bytes),
        _ => None,
    }
}

pub fn write_resolution(
    bytes: Vec<u8>,
    format: ImageFormat,
    resolution: Resolution,
) -> Result<Vec<u8>, String> {
    resolution.validate()?;
    match format {
        ImageFormat::Png => write_png(&bytes, resolution),
        ImageFormat::Jpeg => write_jpeg(&bytes, resolution),
        ImageFormat::Bmp => write_bmp(bytes, resolution),
        ImageFormat::Tiff => write_tiff(&bytes, resolution),
        // GIF and ICO do not have absolute physical resolution fields.
        _ => Ok(bytes),
    }
}

fn read_png(bytes: &[u8]) -> Option<Resolution> {
    let mut offset = 8;
    let mut exif = None;
    while offset + 12 <= bytes.len() {
        let length = u32::from_be_bytes(bytes.get(offset..offset + 4)?.try_into().ok()?) as usize;
        let end = offset.checked_add(length)?.checked_add(12)?;
        let data = bytes.get(offset + 8..end - 4)?;
        match bytes.get(offset + 4..offset + 8)? {
            b"pHYs" if data.len() == 9 && data[8] == 1 => {
                let x = u32::from_be_bytes(data[..4].try_into().ok()?);
                let y = u32::from_be_bytes(data[4..8].try_into().ok()?);
                if let Some(resolution) = Resolution::new(x as f32 * 0.0254, y as f32 * 0.0254) {
                    return Some(resolution);
                }
            }
            b"eXIf" => exif = read_tiff(data),
            b"IEND" => break,
            _ => {}
        }
        offset = end;
    }
    exif
}

fn write_png(bytes: &[u8], resolution: Resolution) -> Result<Vec<u8>, String> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("Invalid PNG signature.".into());
    }
    let mut result = bytes[..8].to_vec();
    let mut offset = 8;
    let mut inserted = false;
    while offset + 12 <= bytes.len() {
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let end = offset
            .checked_add(length)
            .and_then(|value| value.checked_add(12))
            .filter(|&end| end <= bytes.len())
            .ok_or("Truncated PNG chunk.")?;
        let kind = &bytes[offset + 4..offset + 8];
        if kind == b"eXIf" {
            let exif = write_tiff(&bytes[offset + 8..end - 4], resolution)?;
            png_chunk(&mut result, b"eXIf", &exif);
        } else if kind != b"pHYs" {
            result.extend_from_slice(&bytes[offset..end]);
        }
        if kind == b"IHDR" {
            let mut data = Vec::with_capacity(9);
            data.extend_from_slice(&((resolution.x as f64 / 0.0254).round() as u32).to_be_bytes());
            data.extend_from_slice(&((resolution.y as f64 / 0.0254).round() as u32).to_be_bytes());
            data.push(1);
            png_chunk(&mut result, b"pHYs", &data);
            inserted = true;
        }
        offset = end;
        if kind == b"IEND" {
            return if inserted {
                Ok(result)
            } else {
                Err("PNG image header is missing.".into())
            };
        }
    }
    Err("PNG image is incomplete.".into())
}

fn png_chunk(output: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    output.extend_from_slice(&(data.len() as u32).to_be_bytes());
    output.extend_from_slice(kind);
    output.extend_from_slice(data);
    let mut crc = u32::MAX;
    for &byte in kind.iter().chain(data) {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    output.extend_from_slice(&(!crc).to_be_bytes());
}

fn read_bmp(bytes: &[u8]) -> Option<Resolution> {
    let size = u32::from_le_bytes(bytes.get(14..18)?.try_into().ok()?);
    if size < 40 {
        return None;
    }
    let x = i32::from_le_bytes(bytes.get(38..42)?.try_into().ok()?);
    let y = i32::from_le_bytes(bytes.get(42..46)?.try_into().ok()?);
    Resolution::new(x as f32 * 0.0254, y as f32 * 0.0254)
}

fn write_bmp(mut bytes: Vec<u8>, resolution: Resolution) -> Result<Vec<u8>, String> {
    if bytes.len() < 54
        || !bytes.starts_with(b"BM")
        || u32::from_le_bytes(bytes[14..18].try_into().unwrap()) < 40
    {
        return Err("This BMP header cannot store physical resolution.".into());
    }
    bytes[38..42].copy_from_slice(&((resolution.x as f64 / 0.0254).round() as i32).to_le_bytes());
    bytes[42..46].copy_from_slice(&((resolution.y as f64 / 0.0254).round() as i32).to_le_bytes());
    Ok(bytes)
}

struct JpegSegment<'a> {
    marker: u8,
    data: &'a [u8],
}

fn jpeg_segments(bytes: &[u8]) -> Option<(Vec<JpegSegment<'_>>, usize)> {
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return None;
    }
    let mut offset = 2;
    let mut segments = vec![];
    while offset < bytes.len() {
        let start = offset;
        if bytes[offset] != 0xff {
            return None;
        }
        while bytes.get(offset) == Some(&0xff) {
            offset += 1;
        }
        let marker = *bytes.get(offset)?;
        offset += 1;
        if marker == 0xda || marker == 0xd9 {
            return Some((segments, start));
        }
        if marker == 0x01 || (0xd0..=0xd8).contains(&marker) {
            continue;
        }
        let length = u16::from_be_bytes(bytes.get(offset..offset + 2)?.try_into().ok()?) as usize;
        if length < 2 {
            return None;
        }
        let end = offset.checked_add(length)?;
        segments.push(JpegSegment {
            marker,
            data: bytes.get(offset + 2..end)?,
        });
        offset = end;
    }
    None
}

fn read_jpeg(bytes: &[u8]) -> Option<Resolution> {
    let (segments, _) = jpeg_segments(bytes)?;
    let mut jfif = None;
    for segment in segments {
        if segment.marker == 0xe1 && segment.data.starts_with(b"Exif\0\0") {
            if let Some(resolution) = read_tiff(&segment.data[6..]) {
                return Some(resolution);
            }
        }
        if segment.marker == 0xe0 && segment.data.starts_with(b"JFIF\0") && segment.data.len() >= 14
        {
            let scale = match segment.data[7] {
                1 => 1.0,
                2 => 2.54,
                _ => continue,
            };
            let x = u16::from_be_bytes(segment.data[8..10].try_into().ok()?);
            let y = u16::from_be_bytes(segment.data[10..12].try_into().ok()?);
            jfif = Resolution::new(x as f32 * scale, y as f32 * scale);
        }
    }
    jfif
}

fn write_jpeg(bytes: &[u8], resolution: Resolution) -> Result<Vec<u8>, String> {
    let (segments, scan) = jpeg_segments(bytes).ok_or("Invalid JPEG segments.")?;
    let is_jfif =
        |segment: &JpegSegment<'_>| segment.marker == 0xe0 && segment.data.starts_with(b"JFIF\0");
    let is_exif =
        |segment: &JpegSegment<'_>| segment.marker == 0xe1 && segment.data.starts_with(b"Exif\0\0");
    let mut jfif = segments
        .iter()
        .find(|segment| is_jfif(segment))
        .map_or_else(
            || b"JFIF\0\x01\x02\x01\0\x60\0\x60\0\0".to_vec(),
            |segment| segment.data.to_vec(),
        );
    if jfif.len() < 14 {
        return Err("Invalid JFIF density fields.".into());
    }
    jfif[7] = 1;
    jfif[8..10].copy_from_slice(&(resolution.x.round().max(1.0) as u16).to_be_bytes());
    jfif[10..12].copy_from_slice(&(resolution.y.round().max(1.0) as u16).to_be_bytes());
    let mut result = vec![0xff, 0xd8];
    jpeg_segment(&mut result, 0xe0, &jfif)?;
    if !segments.iter().any(is_exif) {
        let mut exif = b"Exif\0\0".to_vec();
        exif.extend(write_tiff(b"II\x2a\0\x08\0\0\0\0\0\0\0\0\0", resolution)?);
        jpeg_segment(&mut result, 0xe1, &exif)?;
    }
    for segment in segments {
        if is_jfif(&segment) {
            continue;
        }
        if is_exif(&segment) {
            let mut exif = b"Exif\0\0".to_vec();
            exif.extend(write_tiff(&segment.data[6..], resolution)?);
            jpeg_segment(&mut result, segment.marker, &exif)?;
        } else {
            jpeg_segment(&mut result, segment.marker, segment.data)?;
        }
    }
    result.extend_from_slice(&bytes[scan..]);
    Ok(result)
}

fn jpeg_segment(output: &mut Vec<u8>, marker: u8, data: &[u8]) -> Result<(), String> {
    let length =
        u16::try_from(data.len() + 2).map_err(|_| "JPEG metadata segment is too large.")?;
    output.extend_from_slice(&[0xff, marker]);
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(data);
    Ok(())
}

#[derive(Clone, Copy)]
enum Endian {
    Little,
    Big,
}

impl Endian {
    fn u16(self, bytes: &[u8], offset: usize) -> Option<u16> {
        let value = bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?;
        Some(match self {
            Self::Little => u16::from_le_bytes(value),
            Self::Big => u16::from_be_bytes(value),
        })
    }
    fn u32(self, bytes: &[u8], offset: usize) -> Option<u32> {
        let value = bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?;
        Some(match self {
            Self::Little => u32::from_le_bytes(value),
            Self::Big => u32::from_be_bytes(value),
        })
    }
    fn bytes16(self, value: u16) -> [u8; 2] {
        match self {
            Self::Little => value.to_le_bytes(),
            Self::Big => value.to_be_bytes(),
        }
    }
    fn bytes32(self, value: u32) -> [u8; 4] {
        match self {
            Self::Little => value.to_le_bytes(),
            Self::Big => value.to_be_bytes(),
        }
    }
}

struct TiffDirectory {
    endian: Endian,
    entries: Vec<[u8; 12]>,
    next: u32,
}

fn tiff_directory(bytes: &[u8]) -> Option<TiffDirectory> {
    let endian = match bytes.get(..2)? {
        b"II" => Endian::Little,
        b"MM" => Endian::Big,
        _ => return None,
    };
    if endian.u16(bytes, 2)? != 42 {
        return None;
    }
    let offset = endian.u32(bytes, 4)? as usize;
    let count = endian.u16(bytes, offset)? as usize;
    let table = bytes.get(offset.checked_add(2)?..offset.checked_add(2 + count * 12)?)?;
    let entries = table.as_chunks::<12>().0.to_vec();
    Some(TiffDirectory {
        endian,
        entries,
        next: endian.u32(bytes, offset + 2 + count * 12)?,
    })
}

fn read_tiff(bytes: &[u8]) -> Option<Resolution> {
    let directory = tiff_directory(bytes)?;
    let endian = directory.endian;
    let mut unit = 2;
    let mut x = None;
    let mut y = None;
    for entry in directory.entries {
        let tag = endian.u16(&entry, 0)?;
        let kind = endian.u16(&entry, 2)?;
        let count = endian.u32(&entry, 4)?;
        if count != 1 {
            continue;
        }
        if tag == 296 && kind == 3 {
            unit = endian.u16(&entry, 8)?;
        }
        if matches!(tag, 282 | 283) && kind == 5 {
            let offset = endian.u32(&entry, 8)? as usize;
            let numerator = endian.u32(bytes, offset)? as f64;
            let denominator = endian.u32(bytes, offset.checked_add(4)?)? as f64;
            if denominator == 0.0 {
                return None;
            }
            if tag == 282 {
                x = Some((numerator / denominator) as f32);
            } else {
                y = Some((numerator / denominator) as f32);
            }
        }
    }
    let scale = match unit {
        2 => 1.0,
        3 => 2.54,
        _ => return None,
    };
    Resolution::new(x? * scale, y? * scale)
}

fn write_tiff(bytes: &[u8], resolution: Resolution) -> Result<Vec<u8>, String> {
    let directory =
        tiff_directory(bytes).ok_or("Unsupported or invalid TIFF metadata directory.")?;
    let endian = directory.endian;
    let mut entries: Vec<_> = directory
        .entries
        .into_iter()
        .filter(|entry| !matches!(endian.u16(entry, 0), Some(282 | 283 | 296)))
        .collect();
    let count = u16::try_from(entries.len() + 3).map_err(|_| "Too many TIFF metadata entries.")?;
    let mut result = bytes.to_vec();
    if !result.len().is_multiple_of(2) {
        result.push(0);
    }
    let offset = u32::try_from(result.len()).map_err(|_| "TIFF metadata offset is too large.")?;
    let rational_offset = offset
        .checked_add(2 + count as u32 * 12 + 4)
        .ok_or("TIFF directory is too large.")?;
    for (tag, kind, value) in [
        (282_u16, 5_u16, rational_offset),
        (283, 5, rational_offset + 8),
        (296, 3, 2),
    ] {
        let mut entry = [0_u8; 12];
        entry[..2].copy_from_slice(&endian.bytes16(tag));
        entry[2..4].copy_from_slice(&endian.bytes16(kind));
        entry[4..8].copy_from_slice(&endian.bytes32(1));
        if kind == 3 {
            entry[8..10].copy_from_slice(&endian.bytes16(value as u16));
        } else {
            entry[8..12].copy_from_slice(&endian.bytes32(value));
        }
        entries.push(entry);
    }
    entries.sort_by_key(|entry| endian.u16(entry, 0));
    result.extend_from_slice(&endian.bytes16(count));
    for entry in entries {
        result.extend_from_slice(&entry);
    }
    result.extend_from_slice(&endian.bytes32(directory.next));
    for dpi in [resolution.x, resolution.y] {
        result.extend_from_slice(&endian.bytes32((dpi as f64 * 10000.0).round() as u32));
        result.extend_from_slice(&endian.bytes32(10000));
    }
    result[4..8].copy_from_slice(&endian.bytes32(offset));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn dpi_roundtrips_without_changing_image_pixels() {
        let image = RgbaImage::from_fn(7, 5, |x, y| Rgba([x as u8 * 30, y as u8 * 40, 50, 255]));
        let expected = Resolution {
            x: 300.125,
            y: 150.5,
        };
        for format in [
            ImageFormat::Png,
            ImageFormat::Jpeg,
            ImageFormat::Bmp,
            ImageFormat::Tiff,
        ] {
            let mut encoded = std::io::Cursor::new(vec![]);
            let source = image::DynamicImage::ImageRgba8(image.clone());
            let source = if format == ImageFormat::Jpeg {
                image::DynamicImage::ImageRgb8(source.to_rgb8())
            } else {
                source
            };
            source.write_to(&mut encoded, format).unwrap();
            let original = image::load_from_memory(encoded.get_ref())
                .unwrap()
                .to_rgba8();
            let bytes = write_resolution(encoded.into_inner(), format, expected).unwrap();
            let actual = read_resolution(&bytes).unwrap();
            assert!(
                (actual.x - expected.x).abs() < 0.013,
                "{format:?}: {actual:?}"
            );
            assert!(
                (actual.y - expected.y).abs() < 0.013,
                "{format:?}: {actual:?}"
            );
            assert_eq!(
                image::load_from_memory(&bytes).unwrap().to_rgba8(),
                original
            );
            let updated = write_resolution(bytes, format, Resolution::default()).unwrap();
            assert!((read_resolution(&updated).unwrap().x - 96.0).abs() < 0.013);
            assert_eq!(
                image::load_from_memory(&updated).unwrap().to_rgba8(),
                original
            );
        }
    }

    #[test]
    fn jfif_centimeter_units_and_unknown_units_are_distinct() {
        let bytes = [
            0xff, 0xd8, 0xff, 0xe0, 0, 16, b'J', b'F', b'I', b'F', 0, 1, 2, 2, 0, 100, 0, 50, 0, 0,
            0xff, 0xd9,
        ];
        assert_eq!(
            read_resolution(&bytes),
            Some(Resolution { x: 254.0, y: 127.0 })
        );
        let mut unknown = bytes;
        unknown[13] = 0;
        assert_eq!(read_resolution(&unknown), None);
    }

    #[test]
    fn big_endian_tiff_resolution_and_invalid_input_are_safe() {
        let bytes = write_tiff(
            b"MM\0\x2a\0\0\0\x08\0\0\0\0\0\0",
            Resolution { x: 600.0, y: 300.0 },
        )
        .unwrap();
        assert_eq!(
            read_resolution(&bytes),
            Some(Resolution { x: 600.0, y: 300.0 })
        );
        for end in 0..bytes.len() {
            let _ = read_resolution(&bytes[..end]);
        }
        assert!(Resolution::new(0.0, 96.0).is_none());
        assert!(Resolution::new(f32::NAN, 96.0).is_none());
        assert!(write_resolution(vec![0; 20], ImageFormat::Png, Resolution::default()).is_err());
    }
}
