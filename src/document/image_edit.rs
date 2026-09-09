//! Reversible image adjustments. Source pixels stay in `ObjectKind::Image`;
//! these settings are applied only when the object is rendered or exported.

use super::{Color, Region};
use image::{imageops, Rgba, RgbaImage};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ImageSampling {
    #[default]
    Nearest,
    Smooth,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ImageEdits {
    pub crop: Option<Region>,
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub temperature: f32,
    pub hue: f32,
    pub gamma: f32,
    pub opacity: f32,
    pub blur: f32,
    pub sharpen: f32,
    pub grayscale: bool,
    pub invert: bool,
    pub sampling: ImageSampling,
    /// Paint's opaque selection background, separate from the source alpha.
    pub matte: Option<Color>,
}

impl Default for ImageEdits {
    fn default() -> Self {
        Self {
            crop: None,
            brightness: 0.0,
            contrast: 0.0,
            saturation: 0.0,
            temperature: 0.0,
            hue: 0.0,
            gamma: 1.0,
            opacity: 100.0,
            blur: 0.0,
            sharpen: 0.0,
            grayscale: false,
            invert: false,
            sampling: ImageSampling::Nearest,
            matte: None,
        }
    }
}

impl ImageEdits {
    pub fn validate(self, dimensions: (u32, u32)) -> Result<(), String> {
        if !super::valid_size(dimensions.0, dimensions.1) {
            return Err("The source image dimensions are outside the supported range.".into());
        }
        for (value, minimum, maximum) in [
            (self.brightness, -100.0, 100.0),
            (self.contrast, -100.0, 100.0),
            (self.saturation, -100.0, 100.0),
            (self.temperature, -100.0, 100.0),
            (self.hue, -180.0, 180.0),
            (self.gamma, 0.1, 5.0),
            (self.opacity, 0.0, 100.0),
            (self.blur, 0.0, 10.0),
            (self.sharpen, 0.0, 5.0),
        ] {
            if !value.is_finite() || !(minimum..=maximum).contains(&value) {
                return Err("An image adjustment is outside its supported range.".into());
            }
        }
        if let Some(crop) = self.crop {
            if crop.w == 0
                || crop.h == 0
                || u64::from(crop.x) + u64::from(crop.w) > u64::from(dimensions.0)
                || u64::from(crop.y) + u64::from(crop.h) > u64::from(dimensions.1)
            {
                return Err("The crop must fit inside the original image.".into());
            }
        }
        Ok(())
    }

    pub fn dimensions(self, original: (u32, u32)) -> (u32, u32) {
        self.crop.map(|crop| (crop.w, crop.h)).unwrap_or(original)
    }

    /// A bounded thumbnail for interactive controls. Sampling happens before
    /// grading and convolution, so preview work does not grow with source size.
    /// It approximates full-resolution grading; Apply and export use `apply`.
    pub fn preview(
        self,
        source: &RgbaImage,
        color_key: Option<Color>,
        max_edge: u32,
    ) -> Result<RgbaImage, String> {
        self.preview_clipped(source, color_key, max_edge, None)
    }

    pub(super) fn preview_clipped(
        self,
        source: &RgbaImage,
        color_key: Option<Color>,
        max_edge: u32,
        clip: Option<&super::SourceClip>,
    ) -> Result<RgbaImage, String> {
        self.validate(source.dimensions())?;
        let crop = self.crop.unwrap_or(Region {
            x: 0,
            y: 0,
            w: source.width(),
            h: source.height(),
        });
        let scale = (max_edge.clamp(1, 1024) as f64 / crop.w.max(crop.h) as f64).min(1.0);
        let width = (crop.w as f64 * scale).round().max(1.0) as u32;
        let height = (crop.h as f64 * scale).round().max(1.0) as u32;
        let samples = if self.sampling == ImageSampling::Smooth && scale < 1.0 {
            4
        } else {
            1
        };
        let thumbnail = RgbaImage::from_fn(width, height, |x, y| {
            let mut sum = [0.0f64; 4];
            for row in 0..samples {
                for column in 0..samples {
                    let source_x = crop.x
                        + ((x as f64 + (column as f64 + 0.5) / samples as f64) * crop.w as f64
                            / width as f64)
                            .floor() as u32;
                    let source_y = crop.y
                        + ((y as f64 + (row as f64 + 0.5) / samples as f64) * crop.h as f64
                            / height as f64)
                            .floor() as u32;
                    let pixel = if clip
                        .is_some_and(|clip| !clip.contains([source_x as f64, source_y as f64]))
                    {
                        Rgba([0; 4])
                    } else {
                        self.prepare_pixel(*source.get_pixel(source_x, source_y), color_key)
                    };
                    if samples == 1 {
                        return pixel;
                    }
                    for channel in 0..3 {
                        sum[channel] += pixel[channel] as f64 * pixel[3] as f64;
                    }
                    sum[3] += pixel[3] as f64;
                }
            }
            if sum[3] <= 0.0 {
                Rgba([0, 0, 0, 0])
            } else {
                Rgba([
                    (sum[0] / sum[3]).round() as u8,
                    (sum[1] / sum[3]).round() as u8,
                    (sum[2] / sum[3]).round() as u8,
                    (sum[3] / (samples * samples) as f64).round() as u8,
                ])
            }
        });
        let thumbnail_edits = Self {
            crop: None,
            matte: None,
            ..self
        };
        let mut result = thumbnail_edits.apply_at_scale(&thumbnail, None, scale as f32, None);
        if let Some(clip) = clip {
            for (x, y, pixel) in result.enumerate_pixels_mut() {
                let source_point = [
                    crop.x as f64 + (x as f64 + 0.5) * crop.w as f64 / width as f64 - 0.5,
                    crop.y as f64 + (y as f64 + 0.5) * crop.h as f64 / height as f64 - 0.5,
                ];
                if !clip.contains(source_point) {
                    pixel[3] = 0;
                }
            }
        }
        Ok(result)
    }

    pub(super) fn apply(self, source: &RgbaImage, color_key: Option<Color>) -> RgbaImage {
        self.apply_at_scale(source, color_key, 1.0, None)
    }

    pub(super) fn apply_clipped(
        self,
        source: &RgbaImage,
        color_key: Option<Color>,
        clip: Option<&super::SourceClip>,
    ) -> RgbaImage {
        self.apply_at_scale(source, color_key, 1.0, clip)
    }

    fn apply_at_scale(
        self,
        source: &RgbaImage,
        color_key: Option<Color>,
        scale: f32,
        clip: Option<&super::SourceClip>,
    ) -> RgbaImage {
        let mut image = match self.crop {
            Some(crop) => crop.extract(source),
            None => source.clone(),
        };
        if self.matte.is_some() || color_key.is_some() {
            for pixel in image.pixels_mut() {
                *pixel = self.prepare_pixel(*pixel, color_key);
            }
        }
        if let Some(clip) = clip {
            let offset = self
                .crop
                .map(|crop| [crop.x as f64, crop.y as f64])
                .unwrap_or([0.0; 2]);
            clip.apply(&mut image, offset);
        }

        let color_edits = self.brightness != 0.0
            || self.contrast != 0.0
            || self.saturation != 0.0
            || self.temperature != 0.0
            || self.hue != 0.0
            || self.gamma != 1.0
            || self.opacity != 100.0
            || self.grayscale
            || self.invert;
        if color_edits {
            for pixel in image.pixels_mut() {
                *pixel = self.adjust_pixel(*pixel);
            }
        }
        if self.blur > 0.0 {
            image = blur_with_alpha(&image, self.blur * scale);
        }
        if self.sharpen > 0.0 {
            let blurred = blur_with_alpha(&image, scale);
            for (pixel, soft) in image.pixels_mut().zip(blurred.pixels()) {
                for channel in 0..3 {
                    pixel[channel] = (pixel[channel] as f32
                        + self.sharpen * (pixel[channel] as f32 - soft[channel] as f32))
                        .round()
                        .clamp(0.0, 255.0) as u8;
                }
            }
        }
        image
    }

    fn prepare_pixel(self, mut pixel: Rgba<u8>, color_key: Option<Color>) -> Rgba<u8> {
        if let Some(matte) = self.matte {
            let mut background = Rgba(matte);
            image::Pixel::blend(&mut background, &pixel);
            pixel = background;
        }
        if let Some(key) = color_key {
            // The selection's color key follows Invert in the palette. Match
            // that key against the retained, uninverted source before grading.
            let key = if self.invert {
                [255 - key[0], 255 - key[1], 255 - key[2], key[3]]
            } else {
                key
            };
            if pixel.0[..3] == key[..3] {
                pixel[3] = 0;
            }
        }
        pixel
    }

    fn adjust_pixel(self, pixel: Rgba<u8>) -> Rgba<u8> {
        let mut rgb = [pixel[0], pixel[1], pixel[2]].map(|value| value as f32 / 255.0);
        // Rotate hue in HSV so a 120-degree turn maps red precisely to green.
        let maximum = rgb.into_iter().fold(0.0f32, f32::max);
        let minimum = rgb.into_iter().fold(1.0f32, f32::min);
        let chroma = maximum - minimum;
        if self.hue != 0.0 && chroma > 0.0 {
            let hue = if maximum == rgb[0] {
                (rgb[1] - rgb[2]) / chroma
            } else if maximum == rgb[1] {
                (rgb[2] - rgb[0]) / chroma + 2.0
            } else {
                (rgb[0] - rgb[1]) / chroma + 4.0
            };
            let sector = (hue + self.hue / 60.0).rem_euclid(6.0);
            let middle = chroma * (1.0 - (sector.rem_euclid(2.0) - 1.0).abs());
            rgb = match sector as u32 {
                0 => [chroma, middle, 0.0],
                1 => [middle, chroma, 0.0],
                2 => [0.0, chroma, middle],
                3 => [0.0, middle, chroma],
                4 => [middle, 0.0, chroma],
                _ => [chroma, 0.0, middle],
            }
            .map(|value| value + minimum);
        }
        let luminance = rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
        let saturation = if self.grayscale {
            0.0
        } else {
            1.0 + self.saturation / 100.0
        };
        let contrast = (1.0 + self.contrast / 100.0).powi(2);
        for (channel, value) in rgb.iter_mut().enumerate() {
            *value = luminance + (*value - luminance) * saturation;
            if !self.grayscale {
                *value += match channel {
                    0 => self.temperature / 500.0,
                    2 => -self.temperature / 500.0,
                    _ => 0.0,
                };
            }
            *value = ((*value - 0.5) * contrast + 0.5 + self.brightness / 100.0)
                .clamp(0.0, 1.0)
                .powf(1.0 / self.gamma);
            if self.invert {
                *value = 1.0 - *value;
            }
        }
        Rgba([
            (rgb[0] * 255.0).round() as u8,
            (rgb[1] * 255.0).round() as u8,
            (rgb[2] * 255.0).round() as u8,
            (pixel[3] as f32 * self.opacity / 100.0).round() as u8,
        ])
    }
}

fn blur_with_alpha(image: &RgbaImage, sigma: f32) -> RgbaImage {
    let premultiplied = image::ImageBuffer::<Rgba<f32>, Vec<f32>>::from_fn(
        image.width(),
        image.height(),
        |x, y| {
            let pixel = image.get_pixel(x, y);
            let alpha = pixel[3] as f32 / 255.0;
            Rgba([
                pixel[0] as f32 / 255.0 * alpha,
                pixel[1] as f32 / 255.0 * alpha,
                pixel[2] as f32 / 255.0 * alpha,
                alpha,
            ])
        },
    );
    let blurred = imageops::blur(&premultiplied, sigma);
    RgbaImage::from_fn(image.width(), image.height(), |x, y| {
        let pixel = blurred.get_pixel(x, y);
        if pixel[3] <= 0.0 {
            return Rgba([0, 0, 0, 0]);
        }
        Rgba([
            (pixel[0] / pixel[3] * 255.0).round().clamp(0.0, 255.0) as u8,
            (pixel[1] / pixel[3] * 255.0).round().clamp(0.0, 255.0) as u8,
            (pixel[2] / pixel[3] * 255.0).round().clamp(0.0, 255.0) as u8,
            (pixel[3] * 255.0).round().clamp(0.0, 255.0) as u8,
        ])
    })
}

/// Interpolate premultiplied channels so transparent pixels cannot produce
/// dark fringes. Edge extension avoids fading the outermost resized pixels.
pub(super) fn sample_smooth(image: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
    if x < -0.5 || y < -0.5 || x > image.width() as f64 - 0.5 || y > image.height() as f64 - 0.5 {
        return Rgba([0, 0, 0, 0]);
    }
    let x = x.clamp(0.0, image.width() as f64 - 1.0);
    let y = y.clamp(0.0, image.height() as f64 - 1.0);
    let left = x.floor() as u32;
    let top = y.floor() as u32;
    let right = (left + 1).min(image.width() - 1);
    let bottom = (top + 1).min(image.height() - 1);
    let dx = x.fract();
    let dy = y.fract();
    let mut channels = [0.0; 4];
    for (px, py, weight) in [
        (left, top, (1.0 - dx) * (1.0 - dy)),
        (right, top, dx * (1.0 - dy)),
        (left, bottom, (1.0 - dx) * dy),
        (right, bottom, dx * dy),
    ] {
        let pixel = image.get_pixel(px, py);
        let alpha = pixel[3] as f64 * weight;
        for channel in 0..3 {
            channels[channel] += pixel[channel] as f64 * alpha;
        }
        channels[3] += alpha;
    }
    if channels[3] <= 0.0 {
        return Rgba([0, 0, 0, 0]);
    }
    Rgba([
        (channels[0] / channels[3]).round() as u8,
        (channels[1] / channels[3]).round() as u8,
        (channels[2] / channels[3]).round() as u8,
        channels[3].round() as u8,
    ])
}

pub(super) fn resize_smooth(image: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let premultiplied = image::ImageBuffer::<Rgba<f32>, Vec<f32>>::from_fn(
        image.width(),
        image.height(),
        |x, y| {
            let pixel = image.get_pixel(x, y);
            let alpha = pixel[3] as f32 / 255.0;
            Rgba([
                pixel[0] as f32 / 255.0 * alpha,
                pixel[1] as f32 / 255.0 * alpha,
                pixel[2] as f32 / 255.0 * alpha,
                alpha,
            ])
        },
    );
    let resized = imageops::resize(
        &premultiplied,
        width,
        height,
        imageops::FilterType::CatmullRom,
    );
    RgbaImage::from_fn(width, height, |x, y| {
        let pixel = resized.get_pixel(x, y);
        if pixel[3] <= 0.0 {
            return Rgba([0, 0, 0, 0]);
        }
        Rgba([
            (pixel[0] / pixel[3] * 255.0).round().clamp(0.0, 255.0) as u8,
            (pixel[1] / pixel[3] * 255.0).round().clamp(0.0, 255.0) as u8,
            (pixel[2] / pixel[3] * 255.0).round().clamp(0.0, 255.0) as u8,
            (pixel[3] * 255.0).round().clamp(0.0, 255.0) as u8,
        ])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Document, Object, ObjectKind};

    fn detailed_source() -> RgbaImage {
        RgbaImage::from_fn(31, 19, |x, y| {
            Rgba([
                (x * 37 % 256) as u8,
                (y * 71 % 256) as u8,
                ((x + y) * 23 % 256) as u8,
                255,
            ])
        })
    }

    #[test]
    fn repeated_resize_returns_every_original_pixel_for_both_sampling_modes() {
        for sampling in [ImageSampling::Nearest, ImageSampling::Smooth] {
            let source = detailed_source();
            let mut object = Object::new(ObjectKind::Image(source.clone()), (8, 12));
            object.image_edits.sampling = sampling;
            object.resize_rendered(3, 2).unwrap();
            assert_eq!(object.render().dimensions(), (3, 2));
            object.resize_rendered(31, 19).unwrap();
            assert_eq!(object.render(), source);
            assert!(matches!(object.kind, ObjectKind::Image(ref original) if *original == source));
        }
    }

    #[test]
    fn legacy_scale_honors_nearest_for_pixels_and_preserves_text_filtering() {
        let source = RgbaImage::from_fn(4, 3, |x, y| {
            if (x + y) % 2 == 0 {
                Rgba([255, 20, 50, 255])
            } else {
                Rgba([0, 80, 250, 64])
            }
        });
        for kind in [
            ObjectKind::Image(source.clone()),
            ObjectKind::Raster(source.clone()),
        ] {
            let mut object = Object::new(kind, (0, 0));
            object.scale = 3.0;
            let result = object.render();
            for (x, y, pixel) in result.enumerate_pixels() {
                assert_eq!(pixel, source.get_pixel(x / 3, y / 3));
            }
        }
        let mut text = Object::new(
            ObjectKind::Text {
                text: "Scale".into(),
                format: Default::default(),
            },
            (0, 0),
        );
        let original = text.render();
        text.scale = 2.0;
        assert_eq!(
            text.render(),
            imageops::resize(
                &original,
                original.width() * 2,
                original.height() * 2,
                imageops::FilterType::CatmullRom
            )
        );
    }

    #[test]
    fn preview_keeps_original_crop_coordinates_without_modifying_the_source() {
        let source = RgbaImage::from_fn(1024, 512, |x, y| Rgba([x as u8, y as u8, 95, 255]));
        let edits = ImageEdits {
            crop: Some(Region {
                x: 512,
                y: 128,
                w: 400,
                h: 200,
            }),
            brightness: 5.0,
            gamma: 1.1,
            ..Default::default()
        };
        let before = source.clone();
        let preview = edits.preview(&source, None, 20).unwrap();
        assert_eq!(preview.dimensions(), (20, 10));
        for (x, y, pixel) in preview.enumerate_pixels() {
            let expected =
                edits.adjust_pixel(*source.get_pixel(512 + x * 20 + 10, 128 + y * 20 + 10));
            assert_eq!(*pixel, expected);
        }
        assert_eq!(source, before);
    }

    #[test]
    fn native_size_preview_matches_apply_including_keyed_alpha_and_spatial_effects() {
        let source = RgbaImage::from_fn(19, 13, |x, y| {
            if (x + y) % 4 == 0 {
                Rgba([255; 4])
            } else {
                Rgba([90, 110, 160, 180])
            }
        });
        let edits = ImageEdits {
            crop: Some(Region {
                x: 3,
                y: 2,
                w: 11,
                h: 7,
            }),
            blur: 2.0,
            sharpen: 1.5,
            invert: true,
            opacity: 75.0,
            matte: Some([200, 200, 200, 255]),
            sampling: ImageSampling::Smooth,
            ..Default::default()
        };
        let key = Some([0, 0, 0, 255]);
        assert_eq!(
            edits.preview(&source, key, 360).unwrap(),
            edits.apply(&source, key)
        );
    }

    #[test]
    fn large_source_preview_bounds_convolution_and_scales_blur_radius() {
        let source = RgbaImage::from_fn(4096, 4096, |x, _| {
            if x < 2048 {
                Rgba([220, 80, 20, 255])
            } else {
                Rgba([0, 255, 255, 0])
            }
        });
        let edits = ImageEdits {
            blur: 10.0,
            sharpen: 2.0,
            sampling: ImageSampling::Smooth,
            ..Default::default()
        };
        let started = std::time::Instant::now();
        let preview = edits.preview(&source, None, 360).unwrap();
        eprintln!("16 MP preview at 360 px: {:?}", started.elapsed());
        assert_eq!(preview.dimensions(), (360, 360));
        let mut thumbnail = RgbaImage::new(360, 360);
        for (x, _, pixel) in thumbnail.enumerate_pixels_mut() {
            if x < 180 {
                *pixel = Rgba([220, 80, 20, 255]);
            }
        }
        assert_eq!(
            preview,
            edits.apply_at_scale(&thumbnail, None, 360.0 / 4096.0, None)
        );
        assert_eq!(source.dimensions(), (4096, 4096));
    }

    #[test]
    fn crop_adjustments_and_alpha_can_be_reset_after_project_round_trip() {
        let source = detailed_source();
        let mut object = Object::new(ObjectKind::Image(source.clone()), (0, 0));
        let crop = Region {
            x: 5,
            y: 4,
            w: 13,
            h: 9,
        };
        object
            .set_image_edits(ImageEdits {
                crop: Some(crop),
                brightness: 12.0,
                contrast: -24.0,
                saturation: -45.0,
                hue: 30.0,
                gamma: 1.4,
                opacity: 64.0,
                ..Default::default()
            })
            .unwrap();
        object.resize_rendered(4, 3).unwrap();
        let before = object.render();
        let mut document = Document::new(31, 19);
        let index = document.add_object(object);
        let bytes = crate::project::encode(&document).unwrap();
        let mut restored = crate::project::decode(&bytes).unwrap();
        let object = &mut restored.objects[index];
        assert_eq!(object.render(), before);
        object
            .set_image_edits(ImageEdits {
                crop: Some(crop),
                ..Default::default()
            })
            .unwrap();
        object.resize_rendered(crop.w, crop.h).unwrap();
        assert_eq!(object.render(), crop.extract(&source));
        object.set_image_edits(ImageEdits::default()).unwrap();
        assert_eq!(object.render(), source);
    }

    #[test]
    fn legacy_object_without_image_edits_uses_unmodified_source() {
        let source = detailed_source();
        let object = Object::new(ObjectKind::Image(source.clone()), (0, 0));
        let mut wire = serde_json::to_value(object).unwrap();
        wire.as_object_mut().unwrap().remove("image_edits");
        let loaded: Object = serde_json::from_value(wire).unwrap();
        assert_eq!(loaded.render(), source);
        assert_eq!(loaded.image_edits, ImageEdits::default());
    }

    #[test]
    fn invalid_edits_are_rejected_without_changing_the_object() {
        let mut object = Object::new(ObjectKind::Image(detailed_source()), (0, 0));
        let before = object.clone();
        for edits in [
            ImageEdits {
                crop: Some(Region {
                    x: u32::MAX,
                    y: 0,
                    w: 2,
                    h: 2,
                }),
                ..Default::default()
            },
            ImageEdits {
                crop: Some(Region {
                    x: 0,
                    y: 0,
                    w: 0,
                    h: 1,
                }),
                ..Default::default()
            },
            ImageEdits {
                gamma: 0.0,
                ..Default::default()
            },
            ImageEdits {
                brightness: f32::NAN,
                ..Default::default()
            },
        ] {
            assert!(object.set_image_edits(edits).is_err());
            assert!(object == before);
        }
    }

    #[test]
    fn color_adjustments_have_expected_endpoints_and_preserve_alpha() {
        let red = Rgba([255, 0, 0, 128]);
        assert_eq!(
            ImageEdits {
                hue: 120.0,
                ..Default::default()
            }
            .adjust_pixel(red),
            Rgba([0, 255, 0, 128])
        );
        assert_eq!(
            ImageEdits {
                invert: true,
                ..Default::default()
            }
            .adjust_pixel(red),
            Rgba([0, 255, 255, 128])
        );
        assert_eq!(
            ImageEdits {
                brightness: 100.0,
                ..Default::default()
            }
            .adjust_pixel(red),
            Rgba([255, 255, 255, 128])
        );
        assert_eq!(
            ImageEdits {
                brightness: -100.0,
                ..Default::default()
            }
            .adjust_pixel(red),
            Rgba([0, 0, 0, 128])
        );
        let gray = ImageEdits {
            grayscale: true,
            opacity: 50.0,
            ..Default::default()
        }
        .adjust_pixel(red);
        assert_eq!(gray, Rgba([54, 54, 54, 64]));
        assert!(
            ImageEdits {
                gamma: 2.0,
                ..Default::default()
            }
            .adjust_pixel(Rgba([64, 64, 64, 255]))[0]
                > 64
        );
    }

    #[test]
    fn smooth_resize_and_blur_do_not_mix_hidden_transparent_rgb_into_edges() {
        let source = RgbaImage::from_fn(2, 1, |x, _| {
            if x == 0 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 255, 255, 0])
            }
        });
        for output in [resize_smooth(&source, 15, 1), blur_with_alpha(&source, 1.0)] {
            for pixel in output.pixels().filter(|pixel| pixel[3] > 0) {
                assert_eq!(&pixel.0[..3], &[255, 0, 0]);
            }
        }
    }

    #[test]
    fn whole_picture_resize_and_crop_keep_original_assets_and_history() {
        let source = detailed_source();
        let mut document = Document::from_image(source.clone());
        document.begin();
        document
            .resize_content(3, 2, ImageSampling::Smooth)
            .unwrap();
        document.commit();
        document.begin();
        document
            .resize_content(31, 19, ImageSampling::Smooth)
            .unwrap();
        document.commit();
        assert_eq!(document.composite(), source);
        let crop = Region {
            x: 4,
            y: 3,
            w: 10,
            h: 8,
        };
        document.begin();
        document.crop_canvas(crop).unwrap();
        document.commit();
        assert_eq!(document.composite(), crop.extract(&source));
        assert!(
            matches!(&document.objects[0].kind, ObjectKind::Raster(original) if *original == source)
        );
        document.undo();
        assert_eq!(document.composite(), source);
    }

    #[test]
    fn whole_picture_rotation_and_flips_preserve_original_image_objects() {
        let source = detailed_source();
        let mut document = Document::new(50, 40);
        let index = document.add_object(Object::new(ObjectKind::Image(source.clone()), (5, 8)));
        let before = document.composite();
        document.rotate_content(90.0, [255; 4]).unwrap();
        assert_eq!(document.composite(), imageops::rotate90(&before));
        document.rotate_content(-90.0, [255; 4]).unwrap();
        assert_eq!(document.composite(), before);
        for horizontal in [true, false] {
            document.flip_content(horizontal);
            assert_eq!(
                document.composite(),
                if horizontal {
                    imageops::flip_horizontal(&before)
                } else {
                    imageops::flip_vertical(&before)
                }
            );
            document.flip_content(horizontal);
            assert_eq!(document.composite(), before);
        }
        document.objects[index].resize_rendered(3, 2).unwrap();
        document.rotate_content(90.0, [255; 4]).unwrap();
        document.objects[index].resize_rendered(19, 31).unwrap();
        assert_eq!(
            document.objects[index].render(),
            imageops::rotate90(&source)
        );
        assert!(
            matches!(&document.objects[index].kind, ObjectKind::Image(original) if *original == source)
        );
    }
}
