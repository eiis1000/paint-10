//! Convex clipping in original image coordinates. Canvas operations can hide
//! source pixels permanently without replacing the original image asset.

use super::{LinearTransform, Object, ObjectKind};
use image::RgbaImage;

const EPSILON: f64 = 1e-7;
const MAX_VERTICES: usize = 4096;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SourceClip {
    #[serde(deserialize_with = "deserialize_vertices")]
    vertices: Vec<[f64; 2]>,
}

impl SourceClip {
    pub fn validate(&self) -> Result<(), String> {
        if self.vertices.len() > MAX_VERTICES
            || (!self.vertices.is_empty() && self.vertices.len() < 3)
            || (!self.vertices.is_empty() && area(&self.vertices) <= EPSILON)
            || self
                .vertices
                .iter()
                .flatten()
                .any(|value| !value.is_finite() || value.abs() > 1_000_000.0)
        {
            return Err("Invalid image source clipping boundary.".into());
        }
        Ok(())
    }

    pub fn memory_bytes(&self) -> usize {
        self.vertices.len() * std::mem::size_of::<[f64; 2]>()
    }

    pub(super) fn contains(&self, point: [f64; 2]) -> bool {
        contains(&self.vertices, point)
    }

    pub(super) fn apply(&self, image: &mut RgbaImage, offset: [f64; 2]) {
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            if !self.contains([x as f64 + offset[0], y as f64 + offset[1]]) {
                pixel[3] = 0;
            }
        }
    }
}

fn cross(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

fn area(polygon: &[[f64; 2]]) -> f64 {
    polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .take(polygon.len())
        .map(|(a, b)| a[0] * b[1] - a[1] * b[0])
        .sum::<f64>()
        * 0.5
}

fn contains(polygon: &[[f64; 2]], point: [f64; 2]) -> bool {
    polygon.len() >= 3
        && polygon
            .iter()
            .zip(polygon.iter().cycle().skip(1))
            .take(polygon.len())
            .all(|(&a, &b)| cross(a, b, point) >= -EPSILON)
}

fn rectangle(left: f64, top: f64, right: f64, bottom: f64) -> Vec<[f64; 2]> {
    vec![[left, top], [right, top], [right, bottom], [left, bottom]]
}

fn intersection(mut subject: Vec<[f64; 2]>, boundary: &[[f64; 2]]) -> Vec<[f64; 2]> {
    for (&a, &b) in boundary
        .iter()
        .zip(boundary.iter().cycle().skip(1))
        .take(boundary.len())
    {
        let Some(&last) = subject.last() else { break };
        let mut previous = last;
        let mut previous_distance = cross(a, b, previous);
        let mut output = Vec::new();
        for current in subject {
            let distance = cross(a, b, current);
            if (distance >= -EPSILON) != (previous_distance >= -EPSILON) {
                let t = (previous_distance / (previous_distance - distance)).clamp(0.0, 1.0);
                output.push([
                    previous[0] + t * (current[0] - previous[0]),
                    previous[1] + t * (current[1] - previous[1]),
                ]);
            }
            if distance >= -EPSILON {
                output.push(current);
            }
            previous = current;
            previous_distance = distance;
        }
        subject = output;
    }
    subject
}

impl Object {
    pub(super) fn source_crop_offset(&self) -> [f64; 2] {
        if matches!(self.kind, ObjectKind::Image(_)) {
            self.image_edits
                .crop
                .map(|crop| [crop.x as f64, crop.y as f64])
                .unwrap_or([0.0; 2])
        } else {
            [0.0; 2]
        }
    }

    /// Intersect the object's existing source clip with a canvas rectangle.
    pub(super) fn clip_to_canvas(&mut self, bounds: [f64; 4]) -> Result<(), String> {
        let (width, height) = self
            .rendered_dimensions()
            .ok_or("Invalid object dimensions.")?;
        let (source_width, source_height) = self.cropped_dimensions();
        let transform = if self.transform == LinearTransform::default() {
            let scaled_width = (source_width as f64 * self.scale as f64).round().max(1.0);
            let scaled_height = (source_height as f64 * self.scale as f64).round().max(1.0);
            LinearTransform::scale(
                scaled_width / source_width as f64,
                scaled_height / source_height as f64,
            )
            .then(LinearTransform::rotation(self.angle))
        } else {
            self.combined_transform()
        };
        let offset = self.source_crop_offset();
        let mut boundary = rectangle(
            bounds[0] - 0.5,
            bounds[1] - 0.5,
            bounds[2] - 0.5,
            bounds[3] - 0.5,
        );
        for point in &mut boundary {
            let x = point[0] - self.pos.0 as f64 - (width as f64 - 1.0) / 2.0;
            let y = point[1] - self.pos.1 as f64 - (height as f64 - 1.0) / 2.0;
            *point = [
                (transform.yy * x - transform.xy * y) / transform.determinant()
                    + (source_width as f64 - 1.0) / 2.0
                    + offset[0],
                (transform.xx * y - transform.yx * x) / transform.determinant()
                    + (source_height as f64 - 1.0) / 2.0
                    + offset[1],
            ];
        }
        if transform.determinant() < 0.0 {
            boundary.reverse();
        }
        let (original_width, original_height) = self.source_dimensions();
        let previous = self
            .source_clip
            .as_ref()
            .map(|clip| clip.vertices.clone())
            .unwrap_or_else(|| {
                rectangle(
                    -0.5,
                    -0.5,
                    original_width as f64 - 0.5,
                    original_height as f64 - 0.5,
                )
            });
        if previous.iter().all(|&point| contains(&boundary, point)) {
            return Ok(());
        }
        let mut vertices = intersection(previous, &boundary);
        if vertices.len() < 3 || area(&vertices) <= EPSILON {
            vertices.clear();
        }
        let clip = SourceClip { vertices };
        clip.validate()?;
        self.source_clip = Some(clip);
        Ok(())
    }
}

fn deserialize_vertices<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<[f64; 2]>, D::Error> {
    struct Vertices;
    impl<'de> serde::de::Visitor<'de> for Vertices {
        type Value = Vec<[f64; 2]>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a bounded image clipping boundary")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut sequence: A,
        ) -> Result<Self::Value, A::Error> {
            let mut vertices = Vec::new();
            while let Some(vertex) = sequence.next_element::<[f64; 2]>()? {
                if vertices.len() == MAX_VERTICES {
                    return Err(serde::de::Error::custom(
                        "The image clipping boundary has too many vertices.",
                    ));
                }
                vertices.push(vertex);
            }
            Ok(vertices)
        }
    }
    deserializer.deserialize_seq(Vertices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Document, ImageSampling, Region};
    use image::{imageops, Rgba};

    const HIDDEN: Rgba<u8> = Rgba([240, 15, 40, 255]);
    const VISIBLE: Rgba<u8> = Rgba([20, 90, 220, 255]);

    fn source() -> RgbaImage {
        RgbaImage::from_fn(48, 40, |x, y| {
            if (12..32).contains(&x) && (10..28).contains(&y) {
                VISIBLE
            } else {
                HIDDEN
            }
        })
    }

    #[test]
    fn cropped_pixels_stay_hidden_through_smooth_resize_flip_and_transparent_rotation() {
        let original = source();
        let mut document = Document::from_image(original.clone());
        document
            .crop_canvas(Region {
                x: 12,
                y: 10,
                w: 20,
                h: 18,
            })
            .unwrap();
        document
            .resize_content(11, 9, ImageSampling::Smooth)
            .unwrap();
        document.flip_content(true);
        document.rotate_content(37.0, [0, 0, 0, 0]).unwrap();
        let visible = document.composite();
        assert!(visible.pixels().any(|pixel| pixel[3] > 0));
        for pixel in visible.pixels().filter(|pixel| pixel[3] > 0) {
            assert_eq!(&pixel.0[..3], &VISIBLE.0[..3]);
        }
        assert!(
            matches!(&document.objects[0].kind, ObjectKind::Raster(pixels) if *pixels == original)
        );
        let loaded = crate::project::decode(&crate::project::encode(&document).unwrap()).unwrap();
        assert_eq!(loaded.composite(), visible);
        let png = crate::raster_io::encode(&visible, crate::raster_io::RasterFormat::Png).unwrap();
        assert_eq!(image::load_from_memory(&png).unwrap().into_rgba8(), visible);
    }

    #[test]
    fn off_canvas_object_pixels_cannot_tint_transparent_rotation_corners() {
        let mut document = Document::from_image(RgbaImage::new(20, 18));
        let index = document.add_object(Object::new(ObjectKind::Image(source()), (-12, -10)));
        document.rotate_content(45.0, [0, 0, 0, 0]).unwrap();
        for pixel in document.composite().pixels().filter(|pixel| pixel[3] > 0) {
            assert_eq!(&pixel.0[..3], &VISIBLE.0[..3]);
        }
        assert!(document.objects[index].source_clip.is_some());
    }

    #[test]
    fn crop_keeps_layer_order_and_exact_visible_pixels() {
        let mut document = Document::new(20, 18);
        document.add_object(Object::new(
            ObjectKind::Image(RgbaImage::from_pixel(12, 9, Rgba([40, 120, 200, 128]))),
            (2, 3),
        ));
        document.image.put_pixel(5, 6, Rgba([10, 220, 70, 200]));
        let before = document.composite();
        let crop = Region {
            x: 3,
            y: 4,
            w: 9,
            h: 7,
        };
        document.begin();
        document.crop_canvas(crop).unwrap();
        document.commit();
        assert_eq!(document.composite(), crop.extract(&before));
        let cropped = document.composite();
        document.rotate_content(90.0, [0; 4]).unwrap();
        assert_eq!(document.composite(), imageops::rotate90(&cropped));
        document.undo();
        assert_eq!(document.composite(), before);
    }

    #[test]
    fn hidden_source_colors_do_not_bleed_into_later_blur_adjustments() {
        let mut document = Document::from_image(RgbaImage::new(48, 40));
        let index = document.add_object(Object::new(ObjectKind::Image(source()), (0, 0)));
        document
            .crop_canvas(Region {
                x: 12,
                y: 10,
                w: 20,
                h: 18,
            })
            .unwrap();
        document.objects[index].image_edits.blur = 4.0;
        document.objects[index].image_edits.matte = Some([255; 4]);
        for pixel in document.composite().pixels().filter(|pixel| pixel[3] > 0) {
            assert_eq!(&pixel.0[..3], &VISIBLE.0[..3]);
        }
        let object = &document.objects[index];
        let preview = object.preview_image_edits(object.image_edits, 360).unwrap();
        for pixel in preview.pixels().filter(|pixel| pixel[3] > 0) {
            assert_eq!(&pixel.0[..3], &VISIBLE.0[..3]);
        }
    }

    #[test]
    fn malformed_and_oversized_project_clip_boundaries_are_rejected() {
        assert!(serde_json::from_value::<SourceClip>(serde_json::json!({
            "vertices": vec![[0.0, 0.0]; MAX_VERTICES + 1]
        }))
        .is_err());
        for vertices in [vec![[0.0, 0.0]], vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]]] {
            assert!(SourceClip { vertices }.validate().is_err());
        }
    }
}
