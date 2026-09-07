use image::{imageops, Rgba, RgbaImage};
use std::collections::VecDeque;

pub type Color = [u8; 4];
pub type Point = (i32, i32);
pub const WHITE: Color = [255, 255, 255, 255];
pub const BLACK: Color = [0, 0, 0, 255];
pub const MAX_PIXELS: u64 = 16_777_216;
pub const DEFAULT_CANVAS_SIZE: (u32, u32) = (900, 600);
const HISTORY_BYTES: usize = 128 * 1024 * 1024;

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
        Self::Heart,
        Self::Lightning,
        Self::Callout,
        Self::Polygon,
        Self::OvalCallout,
        Self::CloudCallout,
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
            Self::Callout => "Speech bubble",
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
}
impl Object {
    pub fn new(kind: ObjectKind, pos: Point) -> Self {
        Self {
            kind,
            pos,
            angle: 0.0,
            scale: 1.0,
            color_key: None,
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
            let mut bytes: usize = self.undo.iter().map(Snapshot::bytes).sum();
            while bytes > HISTORY_BYTES && self.undo.len() > 1 {
                bytes -= self.undo.pop_front().unwrap().bytes();
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
        let mut out = if self.objects.is_empty() {
            self.image.clone()
        } else {
            RgbaImage::from_pixel(self.image.width(), self.image.height(), Rgba(WHITE))
        };
        for (i, obj) in self.objects.iter().enumerate() {
            if skip != Some(i) {
                imageops::overlay(&mut out, &obj.render(), obj.pos.0 as i64, obj.pos.1 as i64);
            }
        }
        if !self.objects.is_empty() {
            imageops::overlay(&mut out, &self.image, 0, 0);
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
            });
            self.image = RgbaImage::new(self.image.width(), self.image.height());
        }
        self.objects.push(obj);
        self.objects.len() - 1
    }
}

impl Snapshot {
    fn bytes(&self) -> usize {
        self.image.as_raw().len()
            + self
                .objects
                .iter()
                .map(|o| match &o.kind {
                    ObjectKind::Raster(img) | ObjectKind::Image(img) => img.as_raw().len(),
                    ObjectKind::Text { text, format } => text.len() + format.memory_bytes(),
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
    let out_alpha = alpha + p[3] as u32 * (255 - alpha) / 255;
    if out_alpha == 0 {
        return;
    }
    for i in 0..3 {
        p[i] = ((color[i] as u32 * alpha + p[i] as u32 * p[3] as u32 * (255 - alpha) / 255)
            / out_alpha) as u8;
    }
    p[3] = out_alpha as u8;
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
    for i in 0..=steps {
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
    fill: Option<(Color, PaintStyle)>,
    width: u32,
) {
    if pts.len() < 2 {
        return;
    }
    if let Some((color, style)) = fill.filter(|(_, style)| *style != PaintStyle::None) {
        let min_y = pts.iter().map(|p| p.1).min().unwrap().max(0);
        let max_y = pts
            .iter()
            .map(|p| p.1)
            .max()
            .unwrap()
            .min(img.height() as i32 - 1);
        for y in min_y..=max_y {
            let mut intersections = vec![];
            for (a, b) in pts.iter().zip(pts.iter().cycle().skip(1)).take(pts.len()) {
                if (a.1 <= y && b.1 > y) || (b.1 <= y && a.1 > y) {
                    intersections.push(
                        a.0 as f32 + (y - a.1) as f32 * (b.0 - a.0) as f32 / (b.1 - a.1) as f32,
                    );
                }
            }
            intersections.sort_by(f32::total_cmp);
            for pair in intersections.chunks_exact(2) {
                for x in (pair[0].ceil() as i32).max(0)
                    ..=(pair[1].floor() as i32).min(img.width() as i32 - 1)
                {
                    blend(img, x, y, texture_color(color, style, x, y));
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
        Tool::Heart => (0..100)
            .map(|i| {
                let t = i as f32 * TAU / 100.;
                (
                    0.5 + t.sin().powi(3) * 0.5,
                    0.5 - (13. * t.cos()
                        - 5. * (2. * t).cos()
                        - 2. * (3. * t).cos()
                        - (4. * t).cos())
                        / 32.,
                )
            })
            .collect(),
        Tool::Lightning => vec![
            (0.55, 0.),
            (0., 0.55),
            (0.4, 0.55),
            (0.3, 1.),
            (1., 0.3),
            (0.55, 0.3),
        ],
        Tool::Callout => vec![
            (0., 0.),
            (1., 0.),
            (1., 0.75),
            (0.45, 0.75),
            (0.2, 1.),
            (0.25, 0.75),
            (0., 0.75),
        ],
        Tool::Polygon => vec![(0., 0.8), (0.2, 0.), (0.6, 0.35), (1., 0.1), (0.85, 1.)],
        Tool::OvalCallout => {
            let mut pts = regular(64, false);
            for p in &mut pts {
                p.1 *= 0.8;
            }
            pts.splice(39..41, [(0.2, 1.)]);
            pts
        }
        Tool::CloudCallout => (0..128)
            .map(|i| {
                let t = i as f32 * TAU / 128.;
                let r = 0.43 + 0.07 * (t * 9.).cos();
                (0.5 + r * t.cos(), 0.43 + r * t.sin() * 0.8)
            })
            .chain([(0.25, 0.87), (0.15, 1.), (0.16, 0.78)])
            .collect(),
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
    fill: Option<(Color, PaintStyle)>,
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
    imageops::overlay(&mut out, img, 0, 0);
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
        });
        img = doc.composite();
        assert_eq!(img.get_pixel(7, 7).0, BLACK);
        assert_eq!(img.get_pixel(5, 5).0, WHITE);
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
                    Some((BLACK, style)),
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
