use crate::document::{self, Color};
use image::{Rgba, RgbaImage};

/// Validate the output allocation and inverse map before changing a document.
pub(in crate::app) struct SkewPlan {
    x: f32,
    y: f32,
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
            x,
            y,
            width: width_out,
            height: height_out,
        })
    }

    pub(in crate::app) fn apply(&self, image: &RgbaImage, background: Color) -> RgbaImage {
        let mut output = RgbaImage::from_pixel(self.width, self.height, Rgba(background));
        let determinant = 1.0 - self.x * self.y;
        for (x, y, pixel) in output.enumerate_pixels_mut() {
            let translated_x = x as f32 + self.x.min(0.0) * image.height() as f32;
            let translated_y = y as f32 + self.y.min(0.0) * image.width() as f32;
            let source_x = ((translated_x - self.x * translated_y) / determinant).round() as i32;
            let source_y = ((translated_y - self.y * translated_x) / determinant).round() as i32;
            if source_x >= 0
                && source_y >= 0
                && source_x < image.width() as i32
                && source_y < image.height() as i32
            {
                *pixel = *image.get_pixel(source_x as u32, source_y as u32);
            }
        }
        output
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
        assert_eq!(
            SkewPlan::new(31, 19, 0.0, 0.0)
                .unwrap()
                .apply(&image, [20, 60, 150, 255]),
            image
        );
    }

    #[test]
    fn skew_uses_the_requested_background_for_exposed_wedges() {
        let image = RgbaImage::from_pixel(12, 8, Rgba([0, 0, 0, 255]));
        let background = [180, 20, 40, 255];
        let output = SkewPlan::new(12, 8, 30.0, 0.0)
            .unwrap()
            .apply(&image, background);
        assert_eq!(output.get_pixel(output.width() - 1, 0).0, background);
        assert!(output
            .pixels()
            .all(|pixel| pixel.0 == background || pixel.0 == [0, 0, 0, 255]));
    }
}
