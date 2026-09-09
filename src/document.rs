use crate::text::FontMemory;
use image::{imageops, Rgba, RgbaImage};
use std::collections::VecDeque;

pub type Color = [u8; 4];
pub type Point = (i32, i32);
pub const WHITE: Color = [255, 255, 255, 255];
pub const BLACK: Color = [0, 0, 0, 255];
pub const MAX_PIXELS: u64 = 16_777_216;
pub const DEFAULT_CANVAS_SIZE: (u32, u32) = (900, 600);
const HISTORY_BYTES: usize = 128 * 1024 * 1024;

#[cfg(test)]
mod history_tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Pencil,
    Fill,
    Text,
    Eraser,
    Picker,
    Magnifier,
    Brush,
    Select,
    Line,
    Curve,
    Oval,
    Rectangle,
    RoundedRect,
    Triangle,
    RightTriangle,
    Diamond,
    Pentagon,
    Hexagon,
    RightArrow,
    LeftArrow,
    UpArrow,
    DownArrow,
    Star4,
    Star5,
    Star6,
    Heart,
    Lightning,
    Callout,
    Polygon,
    OvalCallout,
    CloudCallout,
}

impl Tool {
    pub const SHAPES: [Self; 23] = [
        Self::Line,
        Self::Curve,
        Self::Oval,
        Self::Rectangle,
        Self::RoundedRect,
        Self::Polygon,
        Self::Triangle,
        Self::RightTriangle,
        Self::Diamond,
        Self::Pentagon,
        Self::Hexagon,
        Self::RightArrow,
        Self::LeftArrow,
        Self::UpArrow,
        Self::DownArrow,
        Self::Star4,
        Self::Star5,
        Self::Star6,
        Self::Callout,
        Self::OvalCallout,
        Self::CloudCallout,
        Self::Heart,
        Self::Lightning,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Pencil => "Pencil",
            Self::Fill => "Fill with color",
            Self::Text => "Text",
            Self::Eraser => "Eraser",
            Self::Picker => "Color picker",
            Self::Magnifier => "Magnifier",
            Self::Brush => "Brush",
            Self::Select => "Select",
            Self::Line => "Line",
            Self::Curve => "Curve",
            Self::Oval => "Oval",
            Self::Rectangle => "Rectangle",
            Self::RoundedRect => "Rounded rectangle",
            Self::Triangle => "Triangle",
            Self::RightTriangle => "Right triangle",
            Self::Diamond => "Diamond",
            Self::Pentagon => "Pentagon",
            Self::Hexagon => "Hexagon",
            Self::RightArrow => "Right arrow",
            Self::LeftArrow => "Left arrow",
            Self::UpArrow => "Up arrow",
            Self::DownArrow => "Down arrow",
            Self::Star4 => "Four-point star",
            Self::Star5 => "Five-point star",
            Self::Star6 => "Six-point star",
            Self::Heart => "Heart",
            Self::Lightning => "Lightning",
            Self::Callout => "Rounded rectangular callout",
            Self::Polygon => "Polygon",
            Self::OvalCallout => "Oval callout",
            Self::CloudCallout => "Cloud callout",
        }
    }
    pub fn is_shape(self) -> bool {
        Self::SHAPES.contains(&self)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Brush {
    Round,
    Calligraphy,
    Calligraphy2,
    Airbrush,
    Oil,
    Marker,
    Crayon,
    Pencil,
    Watercolor,
}
impl Brush {
    pub const ALL: [Self; 9] = [
        Self::Round,
        Self::Calligraphy,
        Self::Calligraphy2,
        Self::Airbrush,
        Self::Oil,
        Self::Crayon,
        Self::Marker,
        Self::Pencil,
        Self::Watercolor,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Round => "Brush",
            Self::Calligraphy => "Calligraphy brush",
            Self::Airbrush => "Airbrush",
            Self::Marker => "Marker",
            Self::Crayon => "Crayon",
            Self::Watercolor => "Watercolor brush",
            Self::Calligraphy2 => "Calligraphy brush 2",
            Self::Oil => "Oil brush",
            Self::Pencil => "Natural pencil",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaintStyle {
    None,
    Solid,
    Crayon,
    Marker,
    Oil,
    Pencil,
    Watercolor,
}
impl PaintStyle {
    pub const ALL: [Self; 7] = [
        Self::None,
        Self::Solid,
        Self::Crayon,
        Self::Marker,
        Self::Oil,
        Self::Pencil,
        Self::Watercolor,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Solid => "Solid color",
            Self::Crayon => "Crayon",
            Self::Marker => "Marker",
            Self::Oil => "Oil",
            Self::Pencil => "Natural pencil",
            Self::Watercolor => "Watercolor",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gradient {
    Vertical,
    Horizontal,
    Radial,
}

impl Gradient {
    pub const ALL: [Self; 3] = [Self::Vertical, Self::Horizontal, Self::Radial];

    pub fn name(self) -> &'static str {
        match self {
            Self::Vertical => "Vertical gradient",
            Self::Horizontal => "Horizontal gradient",
            Self::Radial => "Radial gradient",
        }
    }
}

/// A fill is distinct from an outline texture. Gradients run from the first
/// color to the second across the complete, unclipped polygon bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeFill {
    Paint(Color, PaintStyle),
    Gradient {
        direction: Gradient,
        colors: [Color; 2],
    },
}

impl From<(Color, PaintStyle)> for ShapeFill {
    fn from((color, style): (Color, PaintStyle)) -> Self {
        Self::Paint(color, style)
    }
}

impl ShapeFill {
    fn color_at(self, point: Point, bounds: (Point, Point)) -> Color {
        let (direction, colors) = match self {
            Self::Paint(color, style) => return texture_color(color, style, point.0, point.1),
            Self::Gradient { direction, colors } => (direction, colors),
        };
        let ((left, top), (right, bottom)) = bounds;
        let fraction = |position: i32, min: i32, max: i32| {
            if min == max {
                0.0
            } else {
                ((position as f64 - min as f64) / (max as f64 - min as f64)).clamp(0.0, 1.0)
            }
        };
        let t = match direction {
            Gradient::Vertical => fraction(point.1, top, bottom),
            Gradient::Horizontal => fraction(point.0, left, right),
            Gradient::Radial => {
                let x = if left == right {
                    0.0
                } else {
                    fraction(point.0, left, right) * 2.0 - 1.0
                };
                let y = if top == bottom {
                    0.0
                } else {
                    fraction(point.1, top, bottom) * 2.0 - 1.0
                };
                x.hypot(y).min(1.0)
            }
        };
        let [start, end] = colors;
        let start_alpha = start[3] as f64 * (1.0 - t);
        let end_alpha = end[3] as f64 * t;
        let alpha = start_alpha + end_alpha;
        if alpha <= 0.0 {
            return [0, 0, 0, 0];
        }
        // Interpolate premultiplied RGB so a transparent endpoint's hidden
        // color cannot introduce a dark or colored halo.
        let mut color = [0; 4];
        for channel in 0..3 {
            color[channel] =
                ((start[channel] as f64 * start_alpha + end[channel] as f64 * end_alpha) / alpha)
                    .round() as u8;
        }
        color[3] = alpha.round() as u8;
        color
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}
impl Region {
    pub fn between(a: Point, b: Point, img: &RgbaImage) -> Self {
        let x = a.0.min(b.0).clamp(0, img.width() as i32 - 1) as u32;
        let y = a.1.min(b.1).clamp(0, img.height() as i32 - 1) as u32;
        Self {
            x,
            y,
            w: (a.0.max(b.0).clamp(0, img.width() as i32 - 1) as u32 - x + 1),
            h: (a.1.max(b.1).clamp(0, img.height() as i32 - 1) as u32 - y + 1),
        }
    }
    pub fn contains(self, p: Point) -> bool {
        p.0 >= self.x as i32
            && p.1 >= self.y as i32
            && p.0 < (self.x + self.w) as i32
            && p.1 < (self.y + self.h) as i32
    }
    pub fn extract(self, img: &RgbaImage) -> RgbaImage {
        imageops::crop_imm(img, self.x, self.y, self.w, self.h).to_image()
    }
    pub fn clear(self, img: &mut RgbaImage, color: Color) {
        for y in self.y..(self.y + self.h).min(img.height()) {
            for x in self.x..(self.x + self.w).min(img.width()) {
                img.put_pixel(x, y, Rgba(color));
            }
        }
    }
}

#[derive(Clone)]
struct Snapshot {
    image: RgbaImage,
    objects: Vec<Object>,
    mono: bool,
    resolution: crate::metadata::Resolution,
    revision: u64,
}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ObjectKind {
    Raster(#[serde(with = "crate::project::pixels")] RgbaImage),
    Image(#[serde(with = "crate::project::pixels")] RgbaImage),
    Text {
        text: String,
        format: crate::text::TextFormat,
    },
}
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Object {
    pub kind: ObjectKind,
    pub pos: Point,
    pub angle: f32,
    pub scale: f32,
    #[serde(default)]
    pub color_key: Option<Color>,
    #[serde(default)]
    pub transform: LinearTransform,
}

/// A linear transform in canvas coordinates, applied after legacy rotation/scale.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LinearTransform {
    pub xx: f64,
    pub xy: f64,
    pub yx: f64,
    pub yy: f64,
}

impl Default for LinearTransform {
    fn default() -> Self {
        Self {
            xx: 1.0,
            xy: 0.0,
            yx: 0.0,
            yy: 1.0,
        }
    }
}

impl LinearTransform {
    fn scale(x: f64, y: f64) -> Self {
        Self {
            xx: x,
            yy: y,
            ..Default::default()
        }
    }

    fn rotation(angle: f32) -> Self {
        let angle = angle.rem_euclid(360.0) as f64;
        let (sine, cosine) = if (angle / 90.0 - (angle / 90.0).round()).abs() < 0.000001 {
            match (angle / 90.0).round() as i32 % 4 {
                0 => (0.0, 1.0),
                1 => (1.0, 0.0),
                2 => (0.0, -1.0),
                _ => (-1.0, 0.0),
            }
        } else {
            angle.to_radians().sin_cos()
        };
        Self {
            xx: cosine,
            xy: -sine,
            yx: sine,
            yy: cosine,
        }
    }

    fn then(self, next: Self) -> Self {
        Self {
            xx: next.xx * self.xx + next.xy * self.yx,
            xy: next.xx * self.xy + next.xy * self.yy,
            yx: next.yx * self.xx + next.yy * self.yx,
            yy: next.yx * self.xy + next.yy * self.yy,
        }
    }

    fn determinant(self) -> f64 {
        self.xx * self.yy - self.xy * self.yx
    }

    pub fn valid(self) -> bool {
        [self.xx, self.xy, self.yx, self.yy]
            .iter()
            .all(|value| value.is_finite() && value.abs() <= 1_000_000.0)
            && self.determinant().abs() > 1e-12
    }

    fn output_size(self, width: u32, height: u32) -> Option<(u32, u32)> {
        if !self.valid() {
            return None;
        }
        let width_out = (self.xx.abs() * width as f64 + self.xy.abs() * height as f64 - 1e-9)
            .ceil()
            .max(1.0) as u32;
        let height_out = (self.yx.abs() * width as f64 + self.yy.abs() * height as f64 - 1e-9)
            .ceil()
            .max(1.0) as u32;
        valid_size(width_out, height_out).then_some((width_out, height_out))
    }
}

impl Object {
    pub fn new(kind: ObjectKind, pos: Point) -> Self {
        Self {
            kind,
            pos,
            angle: 0.0,
            scale: 1.0,
            color_key: None,
            transform: Default::default(),
        }
    }

    pub fn editable(&self) -> bool {
        !matches!(self.kind, ObjectKind::Raster(_))
    }
    pub fn render(&self) -> RgbaImage {
        self.render_with_key(self.color_key)
    }

    pub fn render_unkeyed(&self) -> RgbaImage {
        self.render_with_key(None)
    }

    fn combined_transform(&self) -> LinearTransform {
        LinearTransform::scale(self.scale as f64, self.scale as f64)
            .then(LinearTransform::rotation(self.angle))
            .then(self.transform)
    }

    pub fn rendered_dimensions(&self) -> Option<(u32, u32)> {
        if !self.angle.is_finite() || !self.scale.is_finite() || self.scale <= 0.0 {
            return None;
        }
        let (width, height) = match &self.kind {
            ObjectKind::Raster(image) | ObjectKind::Image(image) => image.dimensions(),
            ObjectKind::Text { text, format } => format.dimensions(text),
        };
        if self.transform == LinearTransform::default() {
            let scaled_width = (width as f64 * self.scale as f64).round().max(1.0) as u32;
            let scaled_height = (height as f64 * self.scale as f64).round().max(1.0) as u32;
            rotation_size(scaled_width, scaled_height, self.angle)
        } else {
            self.combined_transform().output_size(width, height)
        }
    }

    /// Resize the displayed bounds while retaining the original text/image data.
    pub fn resize_rendered(&mut self, width: u32, height: u32) -> Result<(), String> {
        if !valid_size(width, height) {
            return Err("The resized object would exceed the 16 megapixel limit.".into());
        }
        let (old_width, old_height) = self
            .rendered_dimensions()
            .ok_or("Invalid object transform.")?;
        if (width, height) == (old_width, old_height) {
            return Ok(());
        }
        let previous = self.transform;
        let (source_width, source_height) = match &self.kind {
            ObjectKind::Image(image) | ObjectKind::Raster(image) => image.dimensions(),
            ObjectKind::Text { text, format } => format.dimensions(text),
        };
        let current = self.combined_transform();
        let extent_width =
            current.xx.abs() * source_width as f64 + current.xy.abs() * source_height as f64;
        let extent_height =
            current.yx.abs() * source_width as f64 + current.yy.abs() * source_height as f64;
        self.transform = self.transform.then(LinearTransform::scale(
            width as f64 / extent_width,
            height as f64 / extent_height,
        ));
        if self.rendered_dimensions().is_none() {
            self.transform = previous;
            return Err("The resized object would exceed the supported transform range.".into());
        }
        Ok(())
    }

    /// Rotate the current appearance, preserving earlier nonuniform resizes.
    pub fn rotate_to(&mut self, angle: f32) -> Result<(), String> {
        if !angle.is_finite() {
            return Err("Invalid rotation angle.".into());
        }
        let previous = (self.angle, self.transform);
        let delta = angle.rem_euclid(360.0) - self.angle.rem_euclid(360.0);
        if self.transform != LinearTransform::default() {
            self.transform = LinearTransform::rotation(-delta)
                .then(self.transform)
                .then(LinearTransform::rotation(delta));
        }
        self.angle = angle.rem_euclid(360.0);
        if self.rendered_dimensions().is_none() {
            (self.angle, self.transform) = previous;
            return Err("The rotated object would exceed the 16 megapixel limit.".into());
        }
        Ok(())
    }

    fn render_with_key(&self, color_key: Option<Color>) -> RgbaImage {
        let mut raw = match &self.kind {
            ObjectKind::Raster(img) | ObjectKind::Image(img) => img.clone(),
            ObjectKind::Text { text, format } => format.render(text),
        };
        if let Some(key) = color_key {
            for pixel in raw.pixels_mut() {
                if pixel.0[..3] == key[..3] {
                    pixel[3] = 0;
                }
            }
        }
        if self.transform != LinearTransform::default() {
            return transform_image(&raw, self.combined_transform()).unwrap_or(raw);
        }
        let raw = if self.scale.is_finite() && (self.scale - 1.).abs() > 0.001 {
            let (w, h) = scaled_size(raw.width(), raw.height(), self.scale);
            imageops::resize(&raw, w, h, imageops::FilterType::CatmullRom)
        } else {
            raw
        };
        if self.angle.abs() > 0.01 {
            rotate(&raw, self.angle, [0, 0, 0, 0])
        } else {
            raw
        }
    }
}

fn transform_image(image: &RgbaImage, transform: LinearTransform) -> Option<RgbaImage> {
    let (width, height) = transform.output_size(image.width(), image.height())?;
    let determinant = transform.determinant();
    let mut output = RgbaImage::new(width, height);
    for (x, y, pixel) in output.enumerate_pixels_mut() {
        let dx = x as f64 - (width as f64 - 1.0) / 2.0;
        let dy = y as f64 - (height as f64 - 1.0) / 2.0;
        let source_x = ((transform.yy * dx - transform.xy * dy) / determinant
            + (image.width() as f64 - 1.0) / 2.0)
            .round() as i64;
        let source_y = ((transform.xx * dy - transform.yx * dx) / determinant
            + (image.height() as f64 - 1.0) / 2.0)
            .round() as i64;
        if source_x >= 0
            && source_y >= 0
            && source_x < image.width() as i64
            && source_y < image.height() as i64
        {
            *pixel = *image.get_pixel(source_x as u32, source_y as u32);
        }
    }
    Some(output)
}

pub struct Document {
    pub image: RgbaImage,
    pub mono: bool,
    pub resolution: crate::metadata::Resolution,
    pub objects: Vec<Object>,
    undo: VecDeque<Snapshot>,
    redo: Vec<Snapshot>,
    before: Option<Snapshot>,
    revision: u64,
    next_revision: u64,
    saved_revision: u64,
}
impl Document {
    pub fn new(w: u32, h: u32) -> Self {
        Self::from_image(RgbaImage::from_pixel(w, h, Rgba(WHITE)))
    }
    pub fn from_image(image: RgbaImage) -> Self {
        Self {
            image,
            mono: false,
            resolution: Default::default(),
            objects: vec![],
            undo: VecDeque::new(),
            redo: vec![],
            before: None,
            revision: 0,
            next_revision: 1,
            saved_revision: 0,
        }
    }
    pub fn dirty(&self) -> bool {
        self.revision != self.saved_revision
            || self.before.as_ref().is_some_and(|s| {
                s.image != self.image
                    || s.objects != self.objects
                    || s.mono != self.mono
                    || s.resolution != self.resolution
            })
    }
    pub fn mark_saved(&mut self) {
        self.saved_revision = self.revision;
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn begin(&mut self) {
        if self.before.is_none() {
            self.before = Some(Snapshot {
                image: self.image.clone(),
                objects: self.objects.clone(),
                mono: self.mono,
                resolution: self.resolution,
                revision: self.revision,
            });
        }
    }
    pub fn restore_preview(&mut self) {
        if let Some(s) = &self.before {
            self.image.clone_from(&s.image);
            self.objects.clone_from(&s.objects);
            self.mono = s.mono;
            self.resolution = s.resolution;
        }
    }
    pub fn cancel(&mut self) {
        if let Some(s) = self.before.take() {
            self.image = s.image;
            self.objects = s.objects;
            self.mono = s.mono;
            self.resolution = s.resolution;
            self.revision = s.revision;
        }
    }
    pub fn commit(&mut self) {
        if let Some(s) = self.before.take() {
            if s.image == self.image
                && s.objects == self.objects
                && s.mono == self.mono
                && s.resolution == self.resolution
            {
                return;
            }
            self.undo.push_back(s);
            self.redo.clear();
            self.revision = self.next_revision;
            self.next_revision += 1;
            let mut fonts = FontMemory::default();
            let mut bytes = 0usize;
            for index in (0..self.undo.len()).rev() {
                bytes = bytes.saturating_add(self.undo[index].bytes_with_fonts(&mut fonts));
                if bytes > HISTORY_BYTES && index + 1 < self.undo.len() {
                    self.undo.drain(..=index);
                    break;
                }
            }
        }
    }
    #[cfg(test)]
    pub fn edit(&mut self, edit: impl FnOnce(&mut RgbaImage)) {
        self.begin();
        edit(&mut self.image);
        self.commit();
    }
    pub fn undo(&mut self) {
        self.cancel();
        if let Some(s) = self.undo.pop_back() {
            self.redo.push(Snapshot {
                image: std::mem::replace(&mut self.image, s.image),
                objects: std::mem::replace(&mut self.objects, s.objects),
                mono: std::mem::replace(&mut self.mono, s.mono),
                resolution: std::mem::replace(&mut self.resolution, s.resolution),
                revision: self.revision,
            });
            self.revision = s.revision;
        }
    }
    pub fn redo(&mut self) {
        self.cancel();
        if let Some(s) = self.redo.pop() {
            self.undo.push_back(Snapshot {
                image: std::mem::replace(&mut self.image, s.image),
                objects: std::mem::replace(&mut self.objects, s.objects),
                mono: std::mem::replace(&mut self.mono, s.mono),
                resolution: std::mem::replace(&mut self.resolution, s.resolution),
                revision: self.revision,
            });
            self.revision = s.revision;
        }
    }
    pub fn composite(&self) -> RgbaImage {
        self.composite_without(None)
    }
    pub fn composite_without(&self, skip: Option<usize>) -> RgbaImage {
        self.composite_with_raster(&self.image, skip)
    }

    /// Raster beneath an active drawing transaction, without changing its state.
    pub fn preview_raster(&self) -> &RgbaImage {
        self.before
            .as_ref()
            .map_or(&self.image, |before| &before.image)
    }

    /// Composite a display-only raster replacement over the retained objects.
    pub fn composite_with_raster(&self, raster: &RgbaImage, skip: Option<usize>) -> RgbaImage {
        debug_assert_eq!(raster.dimensions(), self.image.dimensions());
        let mut out = if self.objects.is_empty() {
            raster.clone()
        } else {
            RgbaImage::new(self.image.width(), self.image.height())
        };
        for (i, obj) in self.objects.iter().enumerate() {
            if skip != Some(i) {
                overlay(&mut out, &obj.render(), obj.pos.0 as i64, obj.pos.1 as i64);
            }
        }
        if !self.objects.is_empty() {
            overlay(&mut out, raster, 0, 0);
        }
        if self.mono {
            for p in out.pixels_mut() {
                let v = if p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114 >= 128000 {
                    255
                } else {
                    0
                };
                *p = Rgba([v, v, v, p[3]]);
            }
        }
        out
    }
    pub fn flatten(&mut self) {
        self.image = self.composite();
        self.objects.clear();
    }

    /// Change canvas bounds, copying existing pixels exactly and placing newly
    /// exposed background beneath retained objects. A new bottom raster can
    /// shift object indices; callers must clear or re-establish their selection.
    pub fn resize_canvas(
        &mut self,
        width: u32,
        height: u32,
        background: Color,
    ) -> Result<(), String> {
        if !valid_size(width, height) {
            return Err("The canvas must fit within 16 megapixels.".into());
        }
        let previous = self.image.dimensions();
        if previous == (width, height) {
            return Ok(());
        }
        if self.objects.is_empty() {
            self.image = resize_canvas(&self.image, width, height, background);
            return Ok(());
        }
        if background[3] > 0 && (width > previous.0 || height > previous.1) {
            let bottom = self.objects.first_mut().and_then(|object| {
                if width >= previous.0
                    && height >= previous.1
                    && object.pos == (0, 0)
                    && object.angle == 0.0
                    && object.scale == 1.0
                    && object.color_key.is_none()
                    && object.transform == LinearTransform::default()
                {
                    if let ObjectKind::Raster(raster) = &mut object.kind {
                        return (raster.dimensions() == previous).then_some(raster);
                    }
                }
                None
            });
            if let Some(bottom) = bottom {
                *bottom = resize_canvas(bottom, width, height, background);
            } else {
                let mut padding = RgbaImage::new(width, height);
                for (x, y, pixel) in padding.enumerate_pixels_mut() {
                    if x >= previous.0 || y >= previous.1 {
                        *pixel = Rgba(background);
                    }
                }
                self.objects
                    .insert(0, Object::new(ObjectKind::Raster(padding), (0, 0)));
            }
        }
        self.image = resize_canvas(&self.image, width, height, [0, 0, 0, 0]);
        Ok(())
    }

    pub fn add_object(&mut self, obj: Object) -> usize {
        if self.image.pixels().any(|p| p[3] > 0) {
            let mut bounds = (self.image.width(), self.image.height(), 0, 0);
            for (x, y, p) in self.image.enumerate_pixels() {
                if p[3] > 0 {
                    bounds.0 = bounds.0.min(x);
                    bounds.1 = bounds.1.min(y);
                    bounds.2 = bounds.2.max(x);
                    bounds.3 = bounds.3.max(y);
                }
            }
            let raster = imageops::crop_imm(
                &self.image,
                bounds.0,
                bounds.1,
                bounds.2 - bounds.0 + 1,
                bounds.3 - bounds.1 + 1,
            )
            .to_image();
            self.objects.push(Object {
                kind: ObjectKind::Raster(raster),
                pos: (bounds.0 as i32, bounds.1 as i32),
                angle: 0.,
                scale: 1.,
                color_key: None,
                transform: Default::default(),
            });
            self.image = RgbaImage::new(self.image.width(), self.image.height());
        }
        self.objects.push(obj);
        self.objects.len() - 1
    }
}

impl Snapshot {
    fn bytes_with_fonts(&self, fonts: &mut FontMemory) -> usize {
        self.image.as_raw().len()
            + self
                .objects
                .iter()
                .map(|o| match &o.kind {
                    ObjectKind::Raster(img) | ObjectKind::Image(img) => img.as_raw().len(),
                    ObjectKind::Text { text, format } => {
                        text.len() + format.memory_bytes_with_fonts(fonts)
                    }
                })
                .sum::<usize>()
    }
}

pub fn valid_size(w: u32, h: u32) -> bool {
    w > 0 && h > 0 && w <= 16384 && h <= 16384 && w as u64 * h as u64 <= MAX_PIXELS
}

/// Keep both dimensions and their product within the document allocation budget.
pub fn scaled_size(w: u32, h: u32, scale: f32) -> (u32, u32) {
    let w = w.max(1) as f64;
    let h = h.max(1) as f64;
    let requested = if scale.is_finite() {
        (scale as f64).max(0.)
    } else {
        1.
    };
    let limit = (16384. / w)
        .min(16384. / h)
        .min((MAX_PIXELS as f64 / (w * h)).sqrt());
    let scale = requested.min(limit);
    let mut width = (w * scale).round().max(1.) as u32;
    let mut height = (h * scale).round().max(1.) as u32;
    // Rounding at the allocation limit can add one extra row or column.
    if width as u64 * height as u64 > MAX_PIXELS {
        if width >= height {
            width = (MAX_PIXELS / height as u64) as u32;
        } else {
            height = (MAX_PIXELS / width as u64) as u32;
        }
    }
    (width, height)
}

pub fn blend(img: &mut RgbaImage, x: i32, y: i32, color: Color) {
    if x < 0 || y < 0 || x >= img.width() as i32 || y >= img.height() as i32 {
        return;
    }
    let p = img.get_pixel_mut(x as u32, y as u32);
    let alpha = color[3] as u32;
    if alpha == 0 {
        return;
    }
    if alpha == 255 {
        *p = Rgba(color);
        return;
    }
    // Keep alpha in 1/255 units until RGB has been normalized. Rounding it
    // first can make the channel quotient exceed 255 and wrap back to black.
    let background_alpha = p[3] as u32 * (255 - alpha);
    let combined_alpha = alpha * 255 + background_alpha;
    for i in 0..3 {
        let premultiplied = color[i] as u32 * alpha * 255 + p[i] as u32 * background_alpha;
        p[i] = ((premultiplied + combined_alpha / 2) / combined_alpha) as u8;
    }
    p[3] = ((combined_alpha + 127) / 255) as u8;
}

/// Composite straight-alpha pixels without introducing floating-point opacity loss.
pub fn overlay(image: &mut RgbaImage, source: &RgbaImage, x: i64, y: i64) {
    let start_x = x.clamp(0, image.width() as i64) as u32;
    let start_y = y.clamp(0, image.height() as i64) as u32;
    let end_x = x
        .saturating_add(source.width() as i64)
        .clamp(0, image.width() as i64) as u32;
    let end_y = y
        .saturating_add(source.height() as i64)
        .clamp(0, image.height() as i64) as u32;
    for destination_y in start_y..end_y {
        for destination_x in start_x..end_x {
            let pixel = source.get_pixel(
                (destination_x as i64 - x) as u32,
                (destination_y as i64 - y) as u32,
            );
            blend(image, destination_x as i32, destination_y as i32, pixel.0);
        }
    }
}

fn noise_at(x: i32, y: i32, seed: u32) -> u32 {
    let mut n = (x as u32).wrapping_mul(374761393)
        ^ (y as u32).wrapping_mul(668265263)
        ^ seed.wrapping_mul(2246822519);
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    n ^ (n >> 16)
}

pub fn stamp(img: &mut RgbaImage, p: Point, width: u32, color: Color, brush: Brush) {
    stamp_seeded(img, p, width, color, brush, 0);
}

/// A changing seed lets a held airbrush accumulate new droplets each tick.
pub fn airbrush_stamp(img: &mut RgbaImage, p: Point, width: u32, color: Color, seed: u32) {
    stamp_seeded(img, p, width, color, Brush::Airbrush, seed);
}

fn stamp_seeded(img: &mut RgbaImage, p: Point, width: u32, color: Color, brush: Brush, seed: u32) {
    let width = width.clamp(1, 1024) as i32;
    let radius = width as f32 / 2.;
    let start = -(width / 2);
    let end = start + width;
    let offset = if width % 2 == 0 { 0.5 } else { 0. };
    for dy in start..end {
        for dx in start..end {
            let d = (dx as f32 + offset).powi(2) + (dy as f32 + offset).powi(2);
            if d > radius * radius {
                continue;
            }
            let noise = noise_at(p.0.wrapping_add(dx), p.1.wrapping_add(dy), seed);
            let mut c = color;
            match brush {
                Brush::Calligraphy if (dx + dy).abs() > (width / 6).max(1) => continue,
                Brush::Calligraphy2 if (dx - dy).abs() > (width / 6).max(1) => continue,
                Brush::Airbrush if noise % 1000 > ((1. - d / (radius * radius)) * 120.) as u32 => {
                    continue
                }
                Brush::Crayon if noise % 5 < 2 => continue,
                Brush::Pencil => {
                    c[3] = ((c[3] as u32 * if noise % 7 < 2 { 35 } else { 150 }) / 255) as u8
                }
                Brush::Oil => {
                    let k = (noise % 30) as u8;
                    for v in &mut c[..3] {
                        *v = v.saturating_add(k);
                    }
                    c[3] = (c[3] as u32 * 180 / 255) as u8;
                }
                Brush::Marker => c[3] = (c[3] as u32 * 70 / 255) as u8,
                Brush::Watercolor => {
                    c[3] = (c[3] as f32 * ((1.0 - d / (radius * radius + 1.0)) * 22.0 + 3.) / 255.)
                        as u8
                }
                _ => {}
            }
            blend(img, p.0 + dx, p.1 + dy, c);
        }
    }
}

pub fn line(img: &mut RgbaImage, a: Point, b: Point, width: u32, color: Color, brush: Brush) {
    let steps = (b.0 - a.0).abs().max((b.1 - a.1).abs());
    if steps == 0 {
        stamp(img, a, width, color, brush);
        return;
    }
    let seed = if brush == Brush::Airbrush {
        noise_at(a.0, a.1, 0)
    } else {
        0
    };
    stamp_seeded(img, a, width, color, brush, seed);
    line_from_previous(img, a, b, width, color, brush);
}

/// Continue a stroke whose previous endpoint has already been painted.
/// Repeated stationary events do nothing; the initial press needs `stamp`.
pub fn line_from_previous(
    img: &mut RgbaImage,
    a: Point,
    b: Point,
    width: u32,
    color: Color,
    brush: Brush,
) {
    let steps = (b.0 - a.0).abs().max((b.1 - a.1).abs());
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let p = (
            (a.0 as f32 + (b.0 - a.0) as f32 * t).round() as i32,
            (a.1 as f32 + (b.1 - a.1) as f32 * t).round() as i32,
        );
        let seed = if brush == Brush::Airbrush {
            noise_at(a.0, a.1, i as u32)
        } else {
            0
        };
        stamp_seeded(img, p, width, color, brush, seed);
    }
}

/// Paint's square eraser; a target restricts replacement to Color 1 (right drag).
pub fn erase_line(
    img: &mut RgbaImage,
    a: Point,
    b: Point,
    width: u32,
    replacement: Color,
    target: Option<Color>,
) {
    let width = width.clamp(1, 1024) as i32;
    let steps = (b.0 - a.0).abs().max((b.1 - a.1).abs());
    for i in 0..=steps {
        let t = if steps == 0 {
            0.
        } else {
            i as f32 / steps as f32
        };
        let x = (a.0 as f32 + (b.0 - a.0) as f32 * t).round() as i32 - width / 2;
        let y = (a.1 as f32 + (b.1 - a.1) as f32 * t).round() as i32 - width / 2;
        for yy in y.max(0)..(y + width).min(img.height() as i32) {
            for xx in x.max(0)..(x + width).min(img.width() as i32) {
                let pixel = img.get_pixel_mut(xx as u32, yy as u32);
                if target.is_none_or(|color| pixel.0[..3] == color[..3] && pixel[3] > 0) {
                    *pixel = Rgba(replacement);
                }
            }
        }
    }
}

pub fn flood_fill(img: &mut RgbaImage, p: Point, color: Color) {
    if p.0 < 0 || p.1 < 0 || p.0 >= img.width() as i32 || p.1 >= img.height() as i32 {
        return;
    }
    let target = *img.get_pixel(p.0 as u32, p.1 as u32);
    let color = Rgba(color);
    if target == color {
        return;
    }
    let mut stack = vec![p];
    while let Some((x, y)) = stack.pop() {
        if *img.get_pixel(x as u32, y as u32) != target {
            continue;
        }
        let mut left = x;
        while left > 0 && *img.get_pixel((left - 1) as u32, y as u32) == target {
            left -= 1;
        }
        let mut right = x;
        while right + 1 < img.width() as i32
            && *img.get_pixel((right + 1) as u32, y as u32) == target
        {
            right += 1;
        }
        for xx in left..=right {
            img.put_pixel(xx as u32, y as u32, color);
        }
        for yy in [y - 1, y + 1] {
            if yy < 0 || yy >= img.height() as i32 {
                continue;
            }
            let mut in_run = false;
            for xx in left..=right {
                let matches = *img.get_pixel(xx as u32, yy as u32) == target;
                if matches && !in_run {
                    stack.push((xx, yy));
                }
                in_run = matches;
            }
        }
    }
}

fn texture_color(mut color: Color, style: PaintStyle, x: i32, y: i32) -> Color {
    let noise = noise_at(x, y, 0);
    let alpha = match style {
        PaintStyle::None => 0,
        PaintStyle::Solid => 255,
        PaintStyle::Crayon => {
            if noise % 7 < 2 {
                0
            } else {
                (110 + noise % 146) as u8
            }
        }
        PaintStyle::Marker => 128,
        PaintStyle::Oil => {
            let streak = (noise_at(x / 3, y / 18, 1) % 30) as u8;
            for v in &mut color[..3] {
                *v = v.saturating_add(streak);
            }
            220
        }
        PaintStyle::Pencil => (35 + noise % 160) as u8,
        PaintStyle::Watercolor => (35 + noise_at(x / 8, y / 8, 2) % 65 + noise % 20) as u8,
    };
    color[3] = (color[3] as u32 * alpha as u32 / 255) as u8;
    color
}

fn styled_path(
    img: &mut RgbaImage,
    pts: &[Point],
    closed: bool,
    width: u32,
    color: Color,
    style: PaintStyle,
) {
    if pts.is_empty() || style == PaintStyle::None {
        return;
    }
    let pad = (width.clamp(1, 1024) / 2 + 1) as i32;
    let x0 = (pts.iter().map(|p| p.0).min().unwrap() - pad).clamp(0, img.width() as i32);
    let y0 = (pts.iter().map(|p| p.1).min().unwrap() - pad).clamp(0, img.height() as i32);
    let x1 = (pts.iter().map(|p| p.0).max().unwrap() + pad + 1).clamp(0, img.width() as i32);
    let y1 = (pts.iter().map(|p| p.1).max().unwrap() + pad + 1).clamp(0, img.height() as i32);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    // Union the stroke coverage first, so marker and watercolor outlines do not
    // become opaque at polygon corners or where adjacent line stamps overlap.
    let mut mask = RgbaImage::new((x1 - x0) as u32, (y1 - y0) as u32);
    let offset = |p: Point| (p.0 - x0, p.1 - y0);
    for pair in pts.windows(2) {
        line(
            &mut mask,
            offset(pair[0]),
            offset(pair[1]),
            width,
            WHITE,
            Brush::Round,
        );
    }
    if closed || pts.len() == 1 {
        line(
            &mut mask,
            offset(*pts.last().unwrap()),
            offset(pts[0]),
            width,
            WHITE,
            Brush::Round,
        );
    }
    for (x, y, p) in mask.enumerate_pixels() {
        if p[3] > 0 {
            let x = x as i32 + x0;
            let y = y as i32 + y0;
            blend(img, x, y, texture_color(color, style, x, y));
        }
    }
}

pub fn styled_polygon(
    img: &mut RgbaImage,
    pts: &[Point],
    outline: Option<(Color, PaintStyle)>,
    fill: Option<ShapeFill>,
    width: u32,
) {
    if pts.len() < 2 {
        return;
    }
    if let Some(fill) = fill.filter(|fill| !matches!(fill, ShapeFill::Paint(_, PaintStyle::None))) {
        let bounds = pts.iter().fold((pts[0], pts[0]), |(min, max), point| {
            (
                (min.0.min(point.0), min.1.min(point.1)),
                (max.0.max(point.0), max.1.max(point.1)),
            )
        });
        let min_y = bounds.0 .1.max(0);
        let max_y = bounds.1 .1.min(img.height() as i32 - 1);
        for y in min_y..=max_y {
            // Paint's existing texture coverage is unchanged. Gradient fills
            // include their final edge so the second color reaches the bottom.
            let include_bottom = matches!(fill, ShapeFill::Gradient { .. }) && y == bounds.1 .1;
            let mut intersections = vec![];
            for (a, b) in pts.iter().zip(pts.iter().cycle().skip(1)).take(pts.len()) {
                let crosses = if include_bottom {
                    (a.1 < y && b.1 >= y) || (b.1 < y && a.1 >= y)
                } else {
                    (a.1 <= y && b.1 > y) || (b.1 <= y && a.1 > y)
                };
                if crosses {
                    let intersection = if matches!(fill, ShapeFill::Gradient { .. }) {
                        a.0 as f64
                            + (y as f64 - a.1 as f64) * (b.0 as f64 - a.0 as f64)
                                / (b.1 as f64 - a.1 as f64)
                    } else {
                        // Preserve the established raster coverage of the
                        // original solid and textured Paint fill choices.
                        (a.0 as f32 + (y - a.1) as f32 * (b.0 - a.0) as f32 / (b.1 - a.1) as f32)
                            as f64
                    };
                    intersections.push(intersection);
                }
            }
            intersections.sort_by(f64::total_cmp);
            for pair in intersections.as_chunks::<2>().0 {
                for x in (pair[0].ceil() as i32).max(0)
                    ..=(pair[1].floor() as i32).min(img.width() as i32 - 1)
                {
                    blend(img, x, y, fill.color_at((x, y), bounds));
                }
            }
        }
    }
    if let Some((color, style)) = outline {
        styled_path(img, pts, true, width, color, style);
    }
}

pub fn shape_points(tool: Tool) -> Vec<(f32, f32)> {
    use std::f32::consts::{PI, TAU};
    let regular = |n: usize, star: bool| -> Vec<(f32, f32)> {
        (0..n)
            .map(|i| {
                let angle = i as f32 * TAU / n as f32 - PI / 2.0;
                let r = if star && i % 2 == 1 { 0.22 } else { 0.5 };
                (0.5 + angle.cos() * r, 0.5 + angle.sin() * r)
            })
            .collect()
    };
    match tool {
        Tool::Oval => regular(128, false),
        Tool::RoundedRect => (0..4)
            .flat_map(|corner| {
                (0..12).map(move |i| {
                    let angle = (corner as f32 + i as f32 / 11.0) * PI / 2.0;
                    let center = [(0.82, 0.82), (0.18, 0.82), (0.18, 0.18), (0.82, 0.18)][corner];
                    (center.0 + 0.18 * angle.cos(), center.1 + 0.18 * angle.sin())
                })
            })
            .collect(),
        Tool::Triangle => vec![(0.5, 0.), (1., 1.), (0., 1.)],
        Tool::RightTriangle => vec![(0., 0.), (1., 1.), (0., 1.)],
        Tool::Diamond => vec![(0.5, 0.), (1., 0.5), (0.5, 1.), (0., 0.5)],
        Tool::Pentagon => regular(5, false),
        Tool::Hexagon => regular(6, false),
        Tool::Star4 => regular(8, true),
        Tool::Star5 => regular(10, true),
        Tool::Star6 => regular(12, true),
        Tool::RightArrow | Tool::LeftArrow | Tool::UpArrow | Tool::DownArrow => {
            let pts = vec![
                (0., 0.28),
                (0.6, 0.28),
                (0.6, 0.),
                (1., 0.5),
                (0.6, 1.),
                (0.6, 0.72),
                (0., 0.72),
            ];
            pts.into_iter()
                .map(|(x, y)| match tool {
                    Tool::LeftArrow => (1. - x, y),
                    Tool::UpArrow => (y, 1. - x),
                    Tool::DownArrow => (y, x),
                    _ => (x, y),
                })
                .collect()
        }
        Tool::Heart => {
            let mut points: Vec<_> = (0..100)
                .map(|i| {
                    let t = i as f32 * TAU / 100.0;
                    (
                        t.sin().powi(3),
                        -(13.0 * t.cos()
                            - 5.0 * (2.0 * t).cos()
                            - 2.0 * (3.0 * t).cos()
                            - (4.0 * t).cos()),
                    )
                })
                .collect();
            let (min, max) = points.iter().fold(
                (
                    (f32::INFINITY, f32::INFINITY),
                    (f32::NEG_INFINITY, f32::NEG_INFINITY),
                ),
                |(min, max), &(x, y)| ((min.0.min(x), min.1.min(y)), (max.0.max(x), max.1.max(y))),
            );
            for point in &mut points {
                point.0 = (point.0 - min.0) / (max.0 - min.0);
                point.1 = (point.1 - min.1) / (max.1 - min.1);
            }
            points
        }
        Tool::Lightning => vec![
            (0.55, 0.),
            (0., 0.55),
            (0.4, 0.55),
            (0.3, 1.),
            (1., 0.3),
            (0.55, 0.3),
        ],
        Tool::Callout => {
            let mut points = Vec::new();
            for (corner, center) in [(0.88, 0.12), (0.88, 0.63), (0.12, 0.63), (0.12, 0.12)]
                .into_iter()
                .enumerate()
            {
                for step in 0..=8 {
                    let angle = (corner as f32 - 1.0 + step as f32 / 8.0) * PI / 2.0;
                    points.push((center.0 + 0.12 * angle.cos(), center.1 + 0.12 * angle.sin()));
                }
                if corner == 1 {
                    points.extend([(0.45, 0.75), (0.2, 1.0), (0.25, 0.75)]);
                }
            }
            points
        }
        Tool::Polygon => vec![(0., 0.8), (0.2, 0.), (0.6, 0.35), (1., 0.1), (0.85, 1.)],
        Tool::OvalCallout => {
            let mut pts = regular(64, false);
            for p in &mut pts {
                p.1 *= 0.8;
            }
            pts.splice(39..41, [(0.2, 1.)]);
            pts
        }
        Tool::CloudCallout => {
            let mut points: Vec<_> = (0..128)
                .map(|i| {
                    let t = i as f32 * TAU / 128.;
                    let r = 0.43 + 0.07 * (t * 9.).cos();
                    (0.5 + r * t.cos(), 0.43 + r * t.sin() * 0.8)
                })
                .collect();
            // Replace part of the lower-left edge with the tail. Appending the
            // tail after the complete perimeter cuts a diagonal through the body.
            points.splice(39..44, [(0.15, 1.0)]);
            points
        }
        _ => vec![(0., 0.), (1., 0.), (1., 1.), (0., 1.)],
    }
}

pub fn styled_shape(
    img: &mut RgbaImage,
    tool: Tool,
    a: Point,
    b: Point,
    width: u32,
    outline: Option<(Color, PaintStyle)>,
    fill: Option<ShapeFill>,
) {
    if matches!(tool, Tool::Line | Tool::Curve) {
        if let Some((color, style)) = outline {
            styled_path(img, &[a, b], false, width, color, style);
        }
        return;
    }
    let x = a.0.min(b.0);
    let y = a.1.min(b.1);
    let w = (b.0 - a.0).abs();
    let h = (b.1 - a.1).abs();
    let pts: Vec<Point> = shape_points(tool)
        .iter()
        .map(|(px, py)| {
            (
                x + (px * w as f32).round() as i32,
                y + (py * h as f32).round() as i32,
            )
        })
        .collect();
    styled_polygon(img, &pts, outline, fill, width);
}

pub fn styled_curve(
    img: &mut RgbaImage,
    a: Point,
    b: Point,
    control: Point,
    width: u32,
    color: Color,
    style: PaintStyle,
) {
    let n =
        ((a.0 - b.0).abs() + (a.1 - b.1).abs() + (control.0 - a.0).abs() + (control.1 - a.1).abs())
            .clamp(32, 8192);
    let mut pts = vec![a];
    for i in 1..=n {
        let t = i as f32 / n as f32;
        let q = |a: i32, b: i32, c: i32| {
            ((1. - t).powi(2) * a as f32 + 2. * (1. - t) * t * c as f32 + t * t * b as f32).round()
                as i32
        };
        let p = (q(a.0, b.0, control.0), q(a.1, b.1, control.1));
        pts.push(p);
    }
    styled_path(img, &pts, false, width, color, style);
}

/// Integer bounds enclosing the curve itself, excluding its stroke thickness.
pub fn cubic_bounds(start: Point, end: Point, controls: [Point; 2]) -> (Point, Point) {
    let axis = |points: [i32; 4]| {
        let [p0, p1, p2, p3] = points.map(f64::from);
        let evaluate = |t: f64| {
            let s = 1.0 - t;
            s * s * s * p0 + 3.0 * s * s * t * p1 + 3.0 * s * t * t * p2 + t * t * t * p3
        };
        // The derivative divided by three is a*t² + b*t + c.
        let a = -p0 + 3.0 * p1 - 3.0 * p2 + p3;
        let b = 2.0 * (p0 - 2.0 * p1 + p2);
        let c = p1 - p0;
        let mut extrema = [None, None];
        if a.abs() < 1e-12 {
            if b.abs() >= 1e-12 {
                extrema[0] = Some(-c / b);
            }
        } else {
            let discriminant = b * b - 4.0 * a * c;
            if discriminant >= 0.0 {
                let q = -0.5 * (b + discriminant.sqrt().copysign(b));
                if q.abs() < 1e-12 {
                    extrema[0] = Some(-b / (2.0 * a));
                } else {
                    extrema = [Some(q / a), Some(c / q)];
                }
            }
        }
        let mut min = p0.min(p3);
        let mut max = p0.max(p3);
        for t in extrema
            .into_iter()
            .flatten()
            .filter(|t| *t > 0.0 && *t < 1.0)
        {
            let value = evaluate(t);
            min = min.min(value);
            max = max.max(value);
        }
        (min.floor() as i32, max.ceil() as i32)
    };
    let x = axis([start.0, controls[0].0, controls[1].0, end.0]);
    let y = axis([start.1, controls[0].1, controls[1].1, end.1]);
    ((x.0, y.0), (x.1, y.1))
}

pub fn styled_cubic(
    img: &mut RgbaImage,
    a: Point,
    b: Point,
    controls: [Point; 2],
    width: u32,
    color: Color,
    style: PaintStyle,
) {
    let [c1, c2] = controls;
    let n = ((a.0 - b.0).abs() + (a.1 - b.1).abs() + (c1.0 - c2.0).abs() + (c1.1 - c2.1).abs())
        .clamp(64, 8192);
    let mut pts = vec![a];
    for i in 1..=n {
        let t = i as f32 / n as f32;
        let s = 1. - t;
        let q = |a: i32, b: i32, c: i32, d: i32| {
            (s * s * s * a as f32
                + 3. * s * s * t * c as f32
                + 3. * s * t * t * d as f32
                + t * t * t * b as f32)
                .round() as i32
        };
        let p = (q(a.0, b.0, c1.0, c2.0), q(a.1, b.1, c1.1, c2.1));
        pts.push(p);
    }
    styled_path(img, &pts, false, width, color, style);
}

pub fn resize_canvas(img: &RgbaImage, w: u32, h: u32, bg: Color) -> RgbaImage {
    let mut out = RgbaImage::from_pixel(w, h, Rgba(bg));
    imageops::replace(&mut out, img, 0, 0);
    out
}

pub fn rotate(img: &RgbaImage, angle: f32, bg: Color) -> RgbaImage {
    let Some((w, h)) = rotation_size(img.width(), img.height(), angle) else {
        return img.clone();
    };
    let angle = angle.rem_euclid(360.);
    if angle.abs() < 0.0001 || 360. - angle < 0.0001 {
        return img.clone();
    }
    if (angle - 90.).abs() < 0.0001 {
        return imageops::rotate90(img);
    }
    if (angle - 180.).abs() < 0.0001 {
        return imageops::rotate180(img);
    }
    if (angle - 270.).abs() < 0.0001 {
        return imageops::rotate270(img);
    }
    let theta = (angle as f64).to_radians();
    let (s, c) = theta.sin_cos();
    let mut out = RgbaImage::from_pixel(w, h, Rgba(bg));
    for (x, y, p) in out.enumerate_pixels_mut() {
        let dx = x as f64 - (w as f64 - 1.) / 2.;
        let dy = y as f64 - (h as f64 - 1.) / 2.;
        let sx = (dx * c + dy * s + (img.width() as f64 - 1.) / 2.).round() as i32;
        let sy = (-dx * s + dy * c + (img.height() as f64 - 1.) / 2.).round() as i32;
        if sx >= 0 && sy >= 0 && sx < img.width() as i32 && sy < img.height() as i32 {
            *p = *img.get_pixel(sx as u32, sy as u32);
        }
    }
    out
}

/// Preflight a rotation before changing a document or an editable object.
pub fn rotation_size(w: u32, h: u32, angle: f32) -> Option<(u32, u32)> {
    if !angle.is_finite() || !valid_size(w, h) {
        return None;
    }
    let angle = angle.rem_euclid(360.);
    if angle < 0.0001 || (angle - 180.).abs() < 0.0001 || 360. - angle < 0.0001 {
        return Some((w, h));
    }
    if (angle - 90.).abs() < 0.0001 || (angle - 270.).abs() < 0.0001 {
        return Some((h, w));
    }
    let (s, c) = (angle as f64).to_radians().sin_cos();
    let width = (w as f64 * c.abs() + h as f64 * s.abs()).ceil() as u32;
    let height = (w as f64 * s.abs() + h as f64 * c.abs()).ceil() as u32;
    valid_size(width, height).then_some((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradient_fills_reach_their_endpoints_and_ignore_drag_direction() {
        let colors = [[240, 40, 20, 255], [20, 80, 220, 255]];
        for direction in Gradient::ALL {
            let render = |a, b| {
                let mut image = RgbaImage::new(32, 32);
                styled_shape(
                    &mut image,
                    Tool::Rectangle,
                    a,
                    b,
                    1,
                    None,
                    Some(ShapeFill::Gradient { direction, colors }),
                );
                image
            };
            let image = render((4, 6), (24, 26));
            assert_eq!(image, render((24, 26), (4, 6)));
            let (start, middle, end) = match direction {
                Gradient::Vertical => ((14, 6), (14, 16), (14, 26)),
                Gradient::Horizontal => ((4, 16), (14, 16), (24, 16)),
                Gradient::Radial => ((14, 16), (19, 16), (24, 16)),
            };
            assert_eq!(image.get_pixel(start.0, start.1).0, colors[0]);
            assert_eq!(image.get_pixel(middle.0, middle.1).0, [130, 60, 120, 255]);
            assert_eq!(image.get_pixel(end.0, end.1).0, colors[1]);
            assert!(image
                .enumerate_pixels()
                .filter(|(_, _, pixel)| pixel[3] > 0)
                .all(|(x, y, _)| (4..=24).contains(&x) && (6..=26).contains(&y)));
        }
    }

    #[test]
    fn gradient_alpha_is_premultiplied_and_has_no_transparent_color_halo() {
        let render = |colors| {
            let mut image = RgbaImage::new(25, 25);
            styled_shape(
                &mut image,
                Tool::Rectangle,
                (2, 2),
                (22, 22),
                1,
                None,
                Some(ShapeFill::Gradient {
                    direction: Gradient::Radial,
                    colors,
                }),
            );
            image
        };
        let white_light = render([WHITE, [0, 0, 0, 0]]);
        assert_eq!(white_light.get_pixel(12, 12).0, WHITE);
        assert_eq!(white_light.get_pixel(17, 12).0, [255, 255, 255, 128]);
        assert_eq!(white_light.get_pixel(22, 12)[3], 0);
        assert_eq!(white_light, render([WHITE, [255, 0, 160, 0]]));
        let partial = render([[200, 40, 100, 64], [20, 160, 220, 192]]);
        assert_eq!(partial.get_pixel(17, 12).0, [65, 130, 190, 128]);
        let mut background = RgbaImage::from_pixel(25, 25, Rgba([0, 0, 255, 255]));
        overlay(&mut background, &white_light, 0, 0);
        assert_eq!(background.get_pixel(17, 12).0, [128, 128, 255, 255]);
        assert_eq!(background.get_pixel(22, 12).0, [0, 0, 255, 255]);
    }

    #[test]
    fn polygon_gradient_clipping_preserves_its_original_bounds() {
        let mut image = RgbaImage::new(24, 24);
        styled_polygon(
            &mut image,
            &[(-10, -10), (10, -10), (10, 10), (-10, 10)],
            None,
            Some(ShapeFill::Gradient {
                direction: Gradient::Horizontal,
                colors: [BLACK, WHITE],
            }),
            1,
        );
        assert_eq!(image.get_pixel(0, 0).0, [128, 128, 128, 255]);
        assert_eq!(image.get_pixel(10, 0).0, WHITE);
        assert_eq!(image.get_pixel(11, 0)[3], 0);
        let before = image.clone();
        styled_polygon(
            &mut image,
            &[(-40, -40), (-20, -40), (-20, -20)],
            None,
            Some(ShapeFill::Gradient {
                direction: Gradient::Radial,
                colors: [WHITE, BLACK],
            }),
            1,
        );
        assert_eq!(image, before);
        let mut triangle = RgbaImage::new(24, 24);
        styled_polygon(
            &mut triangle,
            &[(2, 2), (18, 2), (10, 18)],
            None,
            Some(ShapeFill::Gradient {
                direction: Gradient::Vertical,
                colors: [BLACK, WHITE],
            }),
            1,
        );
        assert_eq!(triangle.get_pixel(10, 2).0, BLACK);
        assert_eq!(triangle.get_pixel(10, 18).0, WHITE);
        assert_eq!(triangle.get_pixel(2, 18)[3], 0);
        assert_eq!(triangle.get_pixel(18, 18)[3], 0);
        for x in [i32::MIN, i32::MAX] {
            let mut empty = RgbaImage::new(4, 4);
            styled_polygon(
                &mut empty,
                &[(x, 0), (x, 3)],
                None,
                Some(ShapeFill::Gradient {
                    direction: Gradient::Horizontal,
                    colors: [BLACK, WHITE],
                }),
                1,
            );
            assert!(empty.pixels().all(|pixel| pixel[3] == 0));
        }
    }

    #[test]
    fn canvas_growth_copies_rgba_and_places_background_below_revealed_objects() {
        let green = [20, 150, 70, 255];
        let mut original = RgbaImage::new(20, 20);
        original.put_pixel(5, 5, Rgba([80, 30, 200, 127]));
        let expanded = resize_canvas(&original, 30, 25, green);
        assert_eq!(expanded.get_pixel(5, 5), original.get_pixel(5, 5));
        assert_eq!(expanded.get_pixel(0, 0)[3], 0);
        assert_eq!(expanded.get_pixel(29, 24).0, green);

        let mut document = Document::from_image(original);
        document.add_object(Object::new(
            ObjectKind::Image(RgbaImage::from_pixel(8, 8, Rgba([255, 0, 0, 255]))),
            (18, 2),
        ));
        let old_objects = document.objects.clone();
        let before = document.composite();
        document.begin();
        document.resize_canvas(30, 25, green).unwrap();
        document.commit();
        let expanded = document.composite();
        assert_eq!(expanded.get_pixel(5, 5).0, [80, 30, 200, 127]);
        assert_eq!(expanded.get_pixel(0, 0)[3], 0);
        assert_eq!(expanded.get_pixel(25, 5).0, [255, 0, 0, 255]);
        assert_eq!(expanded.get_pixel(29, 24).0, green);
        document.undo();
        assert!(document.objects == old_objects);
        assert_eq!(document.composite(), before);
        document.redo();
        assert_eq!(document.composite(), expanded);
        let count = document.objects.len();
        document.resize_canvas(40, 30, green).unwrap();
        assert_eq!(document.objects.len(), count);
        assert_eq!(document.composite().get_pixel(0, 0)[3], 0);
    }

    #[test]
    fn canvas_resize_noops_and_transparent_growth_keep_object_indices_stable() {
        let mut document = Document::from_image(RgbaImage::new(20, 20));
        document.add_object(Object::new(
            ObjectKind::Image(RgbaImage::from_pixel(8, 8, Rgba(BLACK))),
            (18, 2),
        ));
        let objects = document.objects.clone();
        document.mark_saved();
        document.begin();
        document.resize_canvas(20, 20, WHITE).unwrap();
        assert!(document.resize_canvas(0, 20, WHITE).is_err());
        document.commit();
        assert!(!document.dirty());
        assert!(!document.can_undo());
        assert!(document.objects == objects);
        document.resize_canvas(30, 30, [0, 0, 0, 0]).unwrap();
        assert!(document.objects == objects);
        let expanded = document.composite();
        assert_eq!(expanded.get_pixel(29, 29)[3], 0);
        assert_eq!(expanded.get_pixel(25, 5).0, BLACK);
    }

    #[test]
    fn mixed_axis_canvas_resize_keeps_retained_raster_pixels_outside_the_canvas() {
        let mut document = Document::new(20, 20);
        document.image.put_pixel(18, 5, Rgba([10, 40, 200, 255]));
        document.add_object(Object::new(
            ObjectKind::Image(RgbaImage::from_pixel(1, 1, Rgba(BLACK))),
            (1, 1),
        ));
        document.resize_canvas(10, 30, WHITE).unwrap();
        document.resize_canvas(20, 30, WHITE).unwrap();
        assert_eq!(document.composite().get_pixel(18, 5).0, [10, 40, 200, 255]);
    }

    #[test]
    fn editable_objects_keep_the_original_canvas_opacity_and_undo_state() {
        for background in [WHITE, [0, 0, 0, 0]] {
            let original = RgbaImage::from_pixel(180, 100, Rgba(background));
            let mut document = Document::from_image(original.clone());
            document.begin();
            document.add_object(Object::new(
                ObjectKind::Text {
                    text: "Caption".into(),
                    format: crate::text::TextFormat {
                        width: 120,
                        ..Default::default()
                    },
                },
                (10, 10),
            ));
            document.add_object(Object::new(
                ObjectKind::Image(RgbaImage::from_pixel(10, 10, Rgba([255, 0, 0, 128]))),
                (140, 70),
            ));
            document.commit();
            let composed = document.composite();
            assert_eq!(composed.get_pixel(179, 99).0, background);
            assert_eq!(
                composed.get_pixel(140, 70)[3],
                if background[3] == 0 { 128 } else { 255 }
            );
            assert!(composed.pixels().any(|pixel| pixel.0 == BLACK));
            document.undo();
            assert!(document.objects.is_empty());
            assert_eq!(document.image, original);
            document.redo();
            assert_eq!(document.composite(), composed);
        }
    }

    #[test]
    fn translucent_layers_preserve_their_color_at_every_alpha_pair() {
        let mut image = RgbaImage::new(1, 1);
        for rgb in [[255, 255, 255], [240, 73, 19]] {
            for foreground in 0..=255u32 {
                for background in 0..=255u32 {
                    image.put_pixel(0, 0, Rgba([rgb[0], rgb[1], rgb[2], background as u8]));
                    blend(&mut image, 0, 0, [rgb[0], rgb[1], rgb[2], foreground as u8]);
                    let expected_alpha =
                        (foreground * 255 + background * (255 - foreground) + 127) / 255;
                    let actual = image.get_pixel(0, 0).0;
                    assert_eq!(actual[3], expected_alpha as u8);
                    assert_eq!(actual[..3], rgb, "alphas {foreground}, {background}");
                }
            }
        }
    }

    #[test]
    fn translucent_source_over_mixes_distinct_colors_without_darkening() {
        let mut image = RgbaImage::from_pixel(1, 1, Rgba([0, 0, 255, 128]));
        blend(&mut image, 0, 0, [255, 0, 0, 128]);
        assert_eq!(image.get_pixel(0, 0).0, [170, 0, 85, 192]);
        blend(&mut image, 0, 0, [255, 255, 0, 0]);
        assert_eq!(image.get_pixel(0, 0).0, [170, 0, 85, 192]);
        blend(&mut image, 0, 0, [10, 20, 30, 255]);
        assert_eq!(image.get_pixel(0, 0).0, [10, 20, 30, 255]);
    }

    #[test]
    fn integer_overlay_clips_pixels_and_extreme_offsets_without_opacity_loss() {
        let mut image = RgbaImage::from_pixel(4, 4, Rgba(WHITE));
        let source = RgbaImage::from_pixel(3, 3, Rgba([255, 0, 0, 128]));
        overlay(&mut image, &source, -2, -1);
        assert_eq!(image.get_pixel(0, 0).0, [255, 127, 127, 255]);
        assert_eq!(image.get_pixel(0, 1).0, [255, 127, 127, 255]);
        assert_eq!(image.get_pixel(1, 0).0, WHITE);
        assert_eq!(image.get_pixel(0, 2).0, WHITE);
        let expected = image.clone();
        for coordinate in [i64::MIN, i64::MAX] {
            overlay(&mut image, &source, coordinate, 0);
            overlay(&mut image, &source, 0, coordinate);
        }
        assert_eq!(image, expected);
    }

    #[test]
    fn cubic_bounds_handle_internal_extrema_and_degenerate_axes() {
        assert_eq!(
            cubic_bounds((40, 40), (140, 40), [(40, 140), (140, 140)]),
            ((40, 40), (140, 115))
        );
        assert_eq!(
            cubic_bounds((50, 150), (250, 150), [(50, -150), (250, 450)]),
            ((50, 63), (250, 237))
        );
        assert_eq!(
            cubic_bounds((30, 10), (30, 90), [(30, 30), (30, 70)]),
            ((30, 10), (30, 90))
        );
        assert_eq!(
            cubic_bounds((4, 7), (4, 7), [(4, 7), (4, 7)]),
            ((4, 7), (4, 7))
        );
    }

    #[test]
    fn heart_path_fits_its_drag_rectangle_in_both_directions() {
        let points = shape_points(Tool::Heart);
        assert!(points
            .iter()
            .all(|&(x, y)| (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)));
        for axis in [0, 1] {
            let values: Vec<_> = points
                .iter()
                .map(|point| if axis == 0 { point.0 } else { point.1 })
                .collect();
            assert_eq!(values.iter().copied().reduce(f32::min), Some(0.0));
            assert_eq!(values.iter().copied().reduce(f32::max), Some(1.0));
        }
        let mut image = RgbaImage::new(220, 200);
        styled_shape(
            &mut image,
            Tool::Heart,
            (180, 150),
            (30, 25),
            1,
            Some((BLACK, PaintStyle::Solid)),
            Some(([180, 50, 70, 255], PaintStyle::Solid).into()),
        );
        let painted: Vec<_> = image
            .enumerate_pixels()
            .filter(|(_, _, pixel)| pixel[3] > 0)
            .map(|(x, y, _)| (x, y))
            .collect();
        assert_eq!(painted.iter().map(|p| p.0).min(), Some(30));
        assert_eq!(painted.iter().map(|p| p.0).max(), Some(180));
        assert_eq!(painted.iter().map(|p| p.1).min(), Some(25));
        assert_eq!(painted.iter().map(|p| p.1).max(), Some(150));
    }
    #[test]
    fn fill_stays_inside_outline_and_handles_edges() {
        let mut img = Document::new(64, 64).image;
        styled_shape(
            &mut img,
            Tool::Rectangle,
            (10, 10),
            (50, 50),
            1,
            Some((BLACK, PaintStyle::Solid)),
            None,
        );
        flood_fill(&mut img, (20, 20), [255, 0, 0, 255]);
        assert_eq!(img.get_pixel(20, 20).0, [255, 0, 0, 255]);
        assert_eq!(img.get_pixel(0, 0).0, WHITE);
        flood_fill(&mut img, (0, 0), [0, 0, 255, 255]);
        assert_eq!(img.get_pixel(63, 63).0, [0, 0, 255, 255]);
        assert_eq!(img.get_pixel(10, 10).0, BLACK);
    }
    #[test]
    fn history_tracks_saved_state_and_branches() {
        let mut doc = Document::new(8, 8);
        doc.edit(|img| stamp(img, (2, 2), 1, BLACK, Brush::Round));
        doc.mark_saved();
        doc.edit(|img| stamp(img, (4, 4), 1, BLACK, Brush::Round));
        assert!(doc.dirty());
        doc.undo();
        assert!(!doc.dirty());
        doc.redo();
        assert!(doc.dirty());
        doc.undo();
        doc.edit(|img| stamp(img, (5, 5), 1, BLACK, Brush::Round));
        assert!(!doc.can_redo());
    }
    #[test]
    fn canceled_preview_does_not_enter_history() {
        let mut doc = Document::new(8, 8);
        doc.begin();
        stamp(&mut doc.image, (2, 2), 2, BLACK, Brush::Round);
        doc.cancel();
        assert!(!doc.dirty());
        assert!(!doc.can_undo());
        doc.begin();
        doc.commit();
        assert!(!doc.can_undo());
    }
    #[test]
    fn one_pixel_selection_and_clipped_paste() {
        let mut img = Document::new(8, 8).image;
        let r = Region::between((7, 7), (7, 7), &img);
        assert_eq!((r.w, r.h), (1, 1));
        let source = RgbaImage::from_pixel(4, 4, Rgba(BLACK));
        let mut doc = Document::from_image(img);
        doc.add_object(Object {
            kind: ObjectKind::Image(source),
            pos: (6, 6),
            angle: 0.0,
            scale: 1.0,
            color_key: None,
            transform: Default::default(),
        });
        img = doc.composite();
        assert_eq!(img.get_pixel(7, 7).0, BLACK);
        assert_eq!(img.get_pixel(5, 5).0, WHITE);
    }
    #[test]
    fn affine_resize_and_rotation_preserve_operation_order() {
        let source = RgbaImage::from_fn(7, 5, |x, y| Rgba([x as u8 * 30, y as u8 * 40, 0, 255]));
        let mut object = Object::new(ObjectKind::Image(source), (0, 0));
        object.rotate_to(37.0).unwrap();
        object.resize_rendered(36, 17).unwrap();
        let resized = object.render();
        assert_eq!(resized.dimensions(), (36, 17));
        object.rotate_to(127.0).unwrap();
        assert_eq!(object.render(), imageops::rotate90(&resized));
        object.resize_rendered(1000, 7).unwrap();
        assert_eq!(object.render().dimensions(), (1000, 7));
        let previous = object.clone();
        assert!(object.resize_rendered(16384, 16384).is_err());
        assert!(object == previous);
    }

    #[test]
    fn resize_and_rotate_preserve_pixels() {
        let mut img = Document::new(2, 3).image;
        img.put_pixel(0, 0, Rgba(BLACK));
        let rotated = rotate(&img, 90., WHITE);
        assert_eq!(rotated.dimensions(), (3, 2));
        assert_eq!(rotated.get_pixel(2, 0).0, BLACK);
        let resized = resize_canvas(&img, 5, 5, WHITE);
        assert_eq!(resized.get_pixel(0, 0).0, BLACK);
        assert_eq!(resized.get_pixel(4, 4).0, WHITE);
    }
    #[test]
    fn png_roundtrip_and_text_render() {
        let mut img = Document::new(128, 64).image;
        let rendered = crate::text::TextFormat {
            size: 24.,
            width: 100,
            ..Default::default()
        }
        .render("Paint");
        imageops::overlay(&mut img, &rendered, 5, 5);
        assert!(img.pixels().any(|p| p.0 != WHITE));
        let mut bytes = std::io::Cursor::new(vec![]);
        img.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        assert_eq!(
            image::load_from_memory(bytes.get_ref()).unwrap().to_rgba8(),
            img
        );
    }

    #[test]
    fn rotations_at_right_angles_are_exact_and_sizes_are_bounded() {
        let image = RgbaImage::from_fn(7, 3, |x, y| Rgba([x as u8, y as u8, 0, 255]));
        assert_eq!(rotate(&image, 90., WHITE), imageops::rotate90(&image));
        assert_eq!(rotate(&image, 180., WHITE), imageops::rotate180(&image));
        assert_eq!(rotate(&image, -90., WHITE), imageops::rotate270(&image));
        assert_eq!(rotate(&image, 360., WHITE), image);
        assert_eq!(rotation_size(4096, 4096, 45.), None);
        assert_eq!(rotation_size(20, 30, f32::NAN), None);
        for (w, h, scale) in [
            (4096, 4096, 16.),
            (16384, 1024, 16.),
            (1, 100, 16.),
            (900, 600, f32::INFINITY),
        ] {
            let (w, h) = scaled_size(w, h, scale);
            assert!(valid_size(w, h));
        }
    }

    #[test]
    fn eraser_has_square_size_and_replaces_only_the_target_color() {
        let mut image = RgbaImage::from_pixel(12, 12, Rgba(BLACK));
        erase_line(&mut image, (6, 6), (6, 6), 4, WHITE, None);
        assert_eq!(image.pixels().filter(|p| p.0 == WHITE).count(), 16);
        assert_eq!(image.get_pixel(4, 4).0, WHITE);
        assert_eq!(image.get_pixel(7, 7).0, WHITE);
        image.put_pixel(5, 5, Rgba([255, 0, 0, 255]));
        erase_line(
            &mut image,
            (0, 0),
            (11, 11),
            4,
            [0, 0, 255, 255],
            Some(BLACK),
        );
        assert_eq!(image.get_pixel(5, 5).0, [255, 0, 0, 255]);
        assert_eq!(image.get_pixel(4, 4).0, WHITE);
        assert_eq!(image.get_pixel(1, 1).0, [0, 0, 255, 255]);
    }

    #[test]
    fn brush_presets_are_distinct_and_airbrush_accumulates() {
        let renders: Vec<_> = Brush::ALL
            .iter()
            .map(|&brush| {
                let mut image = RgbaImage::new(50, 50);
                stamp(&mut image, (25, 25), 32, BLACK, brush);
                image
            })
            .collect();
        for (i, a) in renders.iter().enumerate() {
            assert!(a.pixels().any(|p| p[3] > 0));
            for b in &renders[i + 1..] {
                assert_ne!(a, b);
            }
        }
        let mut image = RgbaImage::new(50, 50);
        airbrush_stamp(&mut image, (25, 25), 32, BLACK, 0);
        let first = image.pixels().filter(|p| p[3] > 0).count();
        for seed in 1..20 {
            airbrush_stamp(&mut image, (25, 25), 32, BLACK, seed);
        }
        assert!(image.pixels().filter(|p| p[3] > 0).count() > first * 2);
    }

    #[test]
    fn continuing_stationary_brush_does_not_darken_its_stamp() {
        for brush in Brush::ALL {
            let mut image = RgbaImage::new(60, 40);
            stamp(&mut image, (20, 20), 8, BLACK, brush);
            let original = image.clone();
            for _ in 0..30 {
                line_from_previous(&mut image, (20, 20), (20, 20), 8, BLACK, brush);
            }
            assert_eq!(image, original, "{brush:?} replayed a stationary endpoint");
        }
    }

    #[test]
    fn brush_continuations_preserve_opacity_across_event_batches() {
        for brush in Brush::ALL {
            // Airbrush randomness deliberately varies with its time/seed input.
            if brush == Brush::Airbrush {
                continue;
            }
            let mut continuous = RgbaImage::new(80, 60);
            line(&mut continuous, (10, 10), (50, 50), 8, BLACK, brush);
            let mut batched = RgbaImage::new(80, 60);
            stamp(&mut batched, (10, 10), 8, BLACK, brush);
            for coordinate in 11..=50 {
                line_from_previous(
                    &mut batched,
                    (coordinate - 1, coordinate - 1),
                    (coordinate, coordinate),
                    8,
                    BLACK,
                    brush,
                );
            }
            assert_eq!(continuous, batched, "{brush:?} repainted shared endpoints");
        }
    }

    #[test]
    fn cloud_callout_outline_does_not_cross_its_body() {
        let points = shape_points(Tool::CloudCallout);
        let cross = |a: (f32, f32), b: (f32, f32), c: (f32, f32)| {
            (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
        };
        for first in 0..points.len() {
            let a = points[first];
            let b = points[(first + 1) % points.len()];
            for second in first + 2..points.len() {
                let c = points[second];
                let d = points[(second + 1) % points.len()];
                assert!(
                    cross(a, b, c) * cross(a, b, d) >= 0.0
                        || cross(c, d, a) * cross(c, d, b) >= 0.0,
                    "Cloud perimeter edges {first} and {second} intersect"
                );
            }
        }
    }

    #[test]
    fn rounded_callout_cuts_out_corners_and_keeps_the_tail() {
        let mut image = RgbaImage::new(140, 140);
        styled_shape(
            &mut image,
            Tool::Callout,
            (10, 10),
            (110, 110),
            1,
            Some((BLACK, PaintStyle::Solid)),
            Some((WHITE, PaintStyle::Solid).into()),
        );
        assert_eq!(image.get_pixel(10, 10)[3], 0);
        assert_eq!(image.get_pixel(110, 10)[3], 0);
        assert_eq!(image.get_pixel(60, 40).0, WHITE);
        assert_eq!(image.get_pixel(30, 110).0, BLACK);
    }

    #[test]
    fn textured_shapes_respect_no_fill_and_uniform_marker_corners() {
        let mut image = RgbaImage::new(64, 64);
        styled_shape(
            &mut image,
            Tool::Rectangle,
            (10, 10),
            (50, 50),
            5,
            Some((BLACK, PaintStyle::Marker)),
            None,
        );
        assert_eq!(image.get_pixel(10, 10)[3], 128);
        assert_eq!(image.get_pixel(30, 10)[3], 128);
        assert_eq!(image.get_pixel(30, 30)[3], 0);
        let renders: Vec<_> = PaintStyle::ALL
            .iter()
            .map(|&style| {
                let mut image = RgbaImage::new(64, 64);
                styled_shape(
                    &mut image,
                    Tool::Rectangle,
                    (10, 10),
                    (50, 50),
                    1,
                    None,
                    Some((BLACK, style).into()),
                );
                image
            })
            .collect();
        for (i, a) in renders.iter().enumerate() {
            for b in &renders[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn object_edits_and_monochrome_are_undoable_without_losing_text() {
        let mut doc = Document::new(100, 80);
        doc.begin();
        let index = doc.add_object(Object {
            kind: ObjectKind::Text {
                text: "Editable".into(),
                format: crate::text::TextFormat {
                    width: 100,
                    color: [255, 0, 0, 255],
                    ..Default::default()
                },
            },
            pos: (0, 0),
            scale: 1.,
            angle: 0.,
            color_key: None,
            transform: Default::default(),
        });
        doc.commit();
        doc.mark_saved();
        let original = doc.composite();
        doc.begin();
        doc.objects[index].pos = (10, 5);
        doc.objects[index].angle = 30.;
        doc.mono = true;
        doc.commit();
        assert!(doc.dirty());
        assert_ne!(doc.composite(), original);
        doc.undo();
        assert_eq!(doc.composite(), original);
        assert!(!doc.mono);
        assert!(!doc.dirty());
        doc.redo();
        assert!(doc.mono);
        assert!(matches!(doc.objects[index].kind, ObjectKind::Text { .. }));
    }

    #[test]
    fn transparency_keys_preserve_source_pixels_and_can_be_toggled() {
        let source = RgbaImage::from_fn(4, 2, |x, _| Rgba(if x < 2 { WHITE } else { BLACK }));
        let mut object = Object::new(ObjectKind::Image(source.clone()), (0, 0));
        object.color_key = Some(WHITE);
        assert_eq!(object.render().get_pixel(0, 0)[3], 0);
        assert_eq!(object.render().get_pixel(3, 0)[3], 255);
        assert_eq!(object.render_unkeyed(), source);
        object.color_key = Some(BLACK);
        assert_eq!(object.render().get_pixel(0, 0)[3], 255);
        assert_eq!(object.render().get_pixel(3, 0)[3], 0);
        object.color_key = None;
        assert_eq!(object.render(), source);
        assert!(matches!(object.kind, ObjectKind::Image(ref pixels) if *pixels == source));
    }

    #[test]
    fn resolution_changes_participate_in_history_and_saved_state() {
        let mut document = Document::new(20, 30);
        document.begin();
        document.resolution = crate::metadata::Resolution { x: 300.0, y: 150.0 };
        assert!(document.dirty());
        document.commit();
        document.mark_saved();
        document.undo();
        assert_eq!(document.resolution, crate::metadata::Resolution::default());
        assert!(document.dirty());
        document.redo();
        assert_eq!(document.resolution.x, 300.0);
        assert!(!document.dirty());
    }
}
