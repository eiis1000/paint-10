use crate::document;
#[cfg(test)]
use image::{Rgba, RgbaImage};

/// Validate the combined output allocation before changing a document.
pub(in crate::app) struct SkewPlan {
    width: u32,
    height: u32,
}

impl SkewPlan {
    pub(in crate::app) fn new(
        width: u32,
        height: u32,
        x_degrees: f32,
        y_degrees: f32,
    ) -> Result<Self, String> {
        if !x_degrees.is_finite()
            || !y_degrees.is_finite()
            || x_degrees.abs() >= 90.0
            || y_degrees.abs() >= 90.0
        {
            return Err("Skew angles must be between -89 and 89 degrees.".into());
        }
        let x = x_degrees.to_radians().tan();
        let y = y_degrees.to_radians().tan();
        if (1.0 - x * y).abs() < 0.001 {
            return Err(
                "These skew angles collapse the picture into a line. Change one angle.".into(),
            );
        }
        let width_out = (width as f32 + x.abs() * height as f32).ceil() as u32;
        let height_out = (height as f32 + y.abs() * width as f32).ceil() as u32;
        if !document::valid_size(width_out, height_out) {
            return Err("The skewed picture would exceed the 16 megapixel limit. Reduce the angles or dimensions.".into());
        }
        Ok(Self {
            width: width_out,
            height: height_out,
        })
    }

    pub(in crate::app) fn validate_rotation(&self, angle: f32) -> Result<(), String> {
        document::rotation_size(self.width, self.height, angle)
            .map(|_| ())
            .ok_or_else(|| {
                "Enter a finite rotation angle that keeps the result within 16 megapixels.".into()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_skew_is_rejected_before_allocating_or_flattening() {
        assert!(SkewPlan::new(100, 100, 45.0, 45.0).is_err());
        assert!(SkewPlan::new(4000, 4000, 80.0, 0.0).is_err());
        assert!(SkewPlan::new(100, 100, f32::NAN, 0.0).is_err());
    }

    #[test]
    fn zero_skew_preserves_every_pixel() {
        let image = RgbaImage::from_fn(31, 19, |x, y| Rgba([x as u8, y as u8, 150, 255]));
        let mut document = document::Document::from_image(image.clone());
        document.skew_content(0.0, 0.0, [20, 60, 150, 255]).unwrap();
        assert_eq!(document.composite(), image);
    }

    #[test]
    fn skew_uses_the_requested_background_for_exposed_wedges() {
        let image = RgbaImage::from_pixel(12, 8, Rgba([0, 0, 0, 255]));
        let background = [180, 20, 40, 255];
        let mut document = document::Document::from_image(image);
        document.skew_content(30.0, 0.0, background).unwrap();
        let output = document.composite();
        assert_eq!(output.get_pixel(output.width() - 1, 0).0, background);
        assert!(output
            .pixels()
            .all(|pixel| pixel.0 == background || pixel.0 == [0, 0, 0, 255]));
    }
}
