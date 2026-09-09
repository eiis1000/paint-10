//! Two-dimensional views of the editor's color coordinates.

use super::{from_coordinates, Conversion, Space};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slice {
    First,
    Second,
    Third,
    PaintSpectrum,
}

impl Slice {
    pub fn channel(self) -> usize {
        match self {
            Self::First => 0,
            Self::Second => 1,
            Self::Third | Self::PaintSpectrum => 2,
        }
    }

    pub fn label(self, space: Space) -> &'static str {
        if self == Self::PaintSpectrum {
            "Spectrum"
        } else {
            space.channels()[self.channel()].label
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plane {
    pub space: Space,
    pub slice: Slice,
}

impl Plane {
    pub fn default_slice(space: Space) -> Slice {
        match space {
            Space::PaintHsl => Slice::PaintSpectrum,
            Space::Hsv | Space::Oklab | Space::Oklch => Slice::First,
            _ => Slice::Third,
        }
    }

    pub fn slices(space: Space) -> &'static [Slice] {
        if space == Space::PaintHsl {
            &[
                Slice::PaintSpectrum,
                Slice::First,
                Slice::Second,
                Slice::Third,
            ]
        } else {
            &[Slice::First, Slice::Second, Slice::Third]
        }
    }

    /// Horizontal and upward vertical axes. Hue stays horizontal in OKLCH.
    pub fn axes(self) -> [usize; 2] {
        let axes = match self.slice.channel() {
            0 => [1, 2],
            1 => [0, 2],
            _ => [0, 1],
        };
        if self.space == Space::Oklch && axes[1] == 2 {
            [axes[1], axes[0]]
        } else {
            axes
        }
    }

    pub fn coordinates_at(self, mut values: [f64; 4], x: f64, y: f64) -> [f64; 4] {
        for (axis, fraction) in self.axes().into_iter().zip([x, y]) {
            values[axis] = self.channel_value(axis, fraction);
        }
        values
    }

    pub fn color_at(self, values: [f64; 4], x: f64, y: f64) -> Conversion {
        let mut values = self.coordinates_at(values, x, y);
        if self.slice == Slice::PaintSpectrum {
            // Preserve Paint's familiar spectrum at midpoint luminosity. The
            // luminosity strip still edits the selected color's actual value.
            values[2] = 120.0;
        }
        from_coordinates(self.space, values)
    }

    pub fn strip_coordinates_at(
        self,
        mut values: [f64; 4],
        channel: usize,
        fraction: f64,
    ) -> [f64; 4] {
        values[channel] = self.channel_value(channel, fraction);
        values
    }

    pub fn strip_color_at(self, values: [f64; 4], channel: usize, fraction: f64) -> Conversion {
        let mut values = self.strip_coordinates_at(values, channel, fraction);
        if channel == 0 && matches!(self.space, Space::PaintHsl | Space::Hsl | Space::Hsv) {
            // A hue guide remains useful even when the chosen color is gray.
            values[1] = self.space.channels()[1].max;
            values[2] = if self.space == Space::Hsv {
                100.0
            } else {
                self.space.channels()[2].max / 2.0
            };
        }
        from_coordinates(self.space, values)
    }

    pub fn position(self, values: [f64; 4]) -> [f64; 2] {
        self.axes().map(|axis| self.fraction(axis, values[axis]))
    }

    pub fn fraction(self, channel: usize, value: f64) -> f64 {
        let channel = &self.space.channels()[channel];
        (value - channel.min) / (channel.max - channel.min)
    }

    /// Only fixed coordinates affect the plane image, not its moving marker.
    pub fn fixed_coordinates(self, mut values: [f64; 4]) -> [f64; 4] {
        for axis in self.axes() {
            values[axis] = 0.0;
        }
        if self.slice == Slice::PaintSpectrum {
            values[2] = 120.0;
        }
        values
    }

    fn channel_value(self, index: usize, fraction: f64) -> f64 {
        let channel = &self.space.channels()[index];
        let value = channel.min + fraction.clamp(0.0, 1.0) * (channel.max - channel.min);
        if channel.decimals == 0 {
            value.round()
        } else {
            value
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slices_keep_the_unedited_coordinates_and_round_trip_the_marker() {
        for space in Space::ALL {
            for &slice in Plane::slices(space) {
                let plane = Plane { space, slice };
                let original = super::super::coordinates(space, [61, 129, 203]);
                for [x, y] in [[0.0, 0.0], [0.5, 0.25], [1.0, 1.0]] {
                    let selected = plane.coordinates_at(original, x, y);
                    assert_eq!(selected[slice.channel()], original[slice.channel()]);
                    assert_eq!(selected[3], original[3]);
                    for (actual, expected) in plane.position(selected).into_iter().zip([x, y]) {
                        assert!((actual - expected).abs() <= 1.0 / 239.0);
                    }
                    let moved = plane.strip_coordinates_at(selected, slice.channel(), 0.7);
                    for axis in plane.axes() {
                        assert_eq!(moved[axis], selected[axis]);
                    }
                }
            }
        }
    }

    #[test]
    fn rgb_linear_rgb_and_cmyk_planes_follow_their_actual_channels() {
        let plane = |space| Plane {
            space,
            slice: Slice::Third,
        };
        assert_eq!(
            plane(Space::Rgb)
                .color_at([0.0, 0.0, 32.0, 0.0], 0.5, 1.0)
                .rgb,
            [128, 255, 32]
        );
        assert_eq!(
            plane(Space::LinearRgb)
                .color_at([0.0, 0.0, 0.0, 0.0], 0.5, 1.0)
                .rgb,
            [188, 255, 0]
        );
        let ink = plane(Space::Cmyk);
        assert_eq!(ink.color_at([0.0, 0.0, 0.0, 0.0], 0.0, 0.0).rgb, [255; 3]);
        assert_eq!(
            ink.color_at([0.0, 0.0, 0.0, 0.0], 1.0, 0.0).rgb,
            [0, 255, 255]
        );
        assert_eq!(
            ink.color_at([0.0, 0.0, 0.0, 50.0], 1.0, 0.0).rgb,
            [0, 128, 128]
        );
        assert_eq!(ink.strip_color_at([0.0; 4], 3, 1.0).rgb, [0; 3]);
    }

    #[test]
    fn perceptual_planes_expose_the_srgb_boundary_without_losing_coordinates() {
        for space in [Space::Oklab, Space::Oklch] {
            let plane = Plane {
                space,
                slice: Slice::First,
            };
            let values = [0.7, 0.0, 0.0, 0.0];
            assert!(
                plane
                    .color_at(values, 0.5, if space == Space::Oklab { 0.5 } else { 0.0 })
                    .in_gamut
            );
            let selected = plane.coordinates_at(values, 0.2, 1.0);
            assert!(!plane.color_at(values, 0.2, 1.0).in_gamut);
            assert_eq!(selected[0], 0.7);
            assert_eq!(selected[if space == Space::Oklab { 2 } else { 1 }], 0.4);
        }
    }

    #[test]
    fn paint_spectrum_is_stable_while_true_hsl_slices_track_lightness() {
        let spectrum = Plane {
            space: Space::PaintHsl,
            slice: Slice::PaintSpectrum,
        };
        assert_eq!(spectrum.color_at([0.0; 4], 0.0, 1.0).rgb, [255, 0, 0]);
        assert_eq!(spectrum.coordinates_at([0.0; 4], 0.0, 1.0)[2], 0.0);
        let hsl = Plane {
            space: Space::Hsl,
            slice: Slice::Third,
        };
        assert_eq!(hsl.color_at([0.0; 4], 0.0, 1.0).rgb, [0; 3]);
        assert_eq!(
            hsl.color_at([0.0, 0.0, 50.0, 0.0], 0.0, 1.0).rgb,
            [255, 0, 0]
        );
    }
}
