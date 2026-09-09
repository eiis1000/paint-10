//! A layer owns both retained objects and the raster drawn above them. The
//! document's legacy `image` and `objects` access dereferences to the active
//! layer, so there is no second working copy to synchronize when switching.

use super::*;
use std::ops::{Deref, DerefMut};

pub const MAX_LAYERS: usize = 64;
pub const MAX_LAYER_BYTES: usize = 192 * 1024 * 1024;
pub const MAX_OBJECTS: usize = 1000;
pub const MAX_OBJECT_BYTES: usize = 128 * 1024 * 1024;
const MAX_LAYER_NAME_BYTES: usize = 256;

/// Structural edits and project decoding share this aggregate budget. Font
/// allocations count once across the complete layer stack.
#[derive(Default)]
pub(crate) struct ObjectBudget {
    fonts: FontMemory,
    count: usize,
    pub(crate) bytes: usize,
}

impl ObjectBudget {
    pub(crate) fn add(&mut self, object: &Object) -> Result<(), String> {
        self.count += 1;
        if self.count > MAX_OBJECTS {
            return Err("Project objects exceed the 1,000 object limit across all layers.".into());
        }
        self.bytes = self
            .bytes
            .saturating_add(object.bytes_with_fonts(&mut self.fonts));
        if self.bytes > MAX_OBJECT_BYTES {
            return Err(
                "Project objects exceed the 128 MB allocation limit across all layers.".into(),
            );
        }
        Ok(())
    }
}

impl Object {
    pub(crate) fn bytes_with_fonts(&self, fonts: &mut FontMemory) -> usize {
        self.source_clip
            .as_ref()
            .map_or(0, SourceClip::memory_bytes)
            + match &self.kind {
                ObjectKind::Raster(image) | ObjectKind::Image(image) => image.as_raw().len(),
                ObjectKind::Text { text, format } => {
                    text.len() + format.memory_bytes_with_fonts(fonts)
                }
            }
    }
}

#[derive(Clone, PartialEq)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: u8,
    /// Identifies Paint's paper independently of its name or stack position.
    pub is_background: bool,
    pub image: RgbaImage,
    pub objects: Vec<Object>,
}

impl Layer {
    pub(crate) fn background(image: RgbaImage) -> Self {
        Self {
            name: "Background".into(),
            visible: true,
            locked: false,
            opacity: 255,
            is_background: true,
            image,
            objects: Vec::new(),
        }
    }

    /// Render the contents before the layer's visibility and group opacity.
    pub fn composite(&self) -> RgbaImage {
        self.composite_with_raster(&self.image, None)
    }

    /// Render a small pane preview without allocating full-size image copies or
    /// transformed rasters. Image effects use the same bounded preview path as
    /// their dialog; text still uses its normal layout to preserve wrapping.
    pub fn thumbnail(&self, max_edge: u32) -> RgbaImage {
        let scale = (f64::from(max_edge.clamp(1, 512))
            / f64::from(self.image.width().max(self.image.height())))
        .min(1.0);
        let width = (f64::from(self.image.width()) * scale).round().max(1.0) as u32;
        let height = (f64::from(self.image.height()) * scale).round().max(1.0) as u32;
        let mut out = RgbaImage::new(width, height);
        for object in &self.objects {
            let Some((object_width, object_height)) = object.rendered_dimensions() else {
                continue;
            };
            let source = match &object.kind {
                ObjectKind::Raster(image) => ImageEdits::default().preview_clipped(
                    image,
                    object.color_key,
                    512,
                    object.source_clip.as_ref(),
                ),
                ObjectKind::Image(image) => object.image_edits.preview_clipped(
                    image,
                    object.color_key,
                    512,
                    object.source_clip.as_ref(),
                ),
                ObjectKind::Text { text, format } => {
                    let mut image = format.render(text);
                    if let Some(clip) = &object.source_clip {
                        clip.apply(&mut image, [0.0; 2]);
                    }
                    if let Some(key) = object.color_key {
                        for pixel in image.pixels_mut() {
                            if pixel.0[..3] == key[..3] {
                                pixel[3] = 0;
                            }
                        }
                    }
                    Ok(image)
                }
            };
            let Ok(source) = source else { continue };
            let transform = object.combined_transform();
            let determinant = transform.determinant();
            let (source_width, source_height) = object.cropped_dimensions();
            for y in 0..height {
                for x in 0..width {
                    let local_x = (f64::from(x) + 0.5) / scale - f64::from(object.pos.0);
                    let local_y = (f64::from(y) + 0.5) / scale - f64::from(object.pos.1);
                    if local_x < 0.0
                        || local_y < 0.0
                        || local_x >= f64::from(object_width)
                        || local_y >= f64::from(object_height)
                    {
                        continue;
                    }
                    let centered_x = local_x - f64::from(object_width) / 2.0;
                    let centered_y = local_y - f64::from(object_height) / 2.0;
                    let source_x = (transform.yy * centered_x - transform.xy * centered_y)
                        / determinant
                        + f64::from(source_width) / 2.0;
                    let source_y = (-transform.yx * centered_x + transform.xx * centered_y)
                        / determinant
                        + f64::from(source_height) / 2.0;
                    if source_x < 0.0
                        || source_y < 0.0
                        || source_x >= f64::from(source_width)
                        || source_y >= f64::from(source_height)
                    {
                        continue;
                    }
                    let sample_x = (source_x * f64::from(source.width()) / f64::from(source_width))
                        .floor() as u32;
                    let sample_y = (source_y * f64::from(source.height())
                        / f64::from(source_height))
                    .floor() as u32;
                    blend(
                        &mut out,
                        x as i32,
                        y as i32,
                        source
                            .get_pixel(
                                sample_x.min(source.width() - 1),
                                sample_y.min(source.height() - 1),
                            )
                            .0,
                    );
                }
            }
        }
        for y in 0..height {
            for x in 0..width {
                let source_x =
                    (((f64::from(x) + 0.5) / scale).floor() as u32).min(self.image.width() - 1);
                let source_y =
                    (((f64::from(y) + 0.5) / scale).floor() as u32).min(self.image.height() - 1);
                blend(
                    &mut out,
                    x as i32,
                    y as i32,
                    self.image.get_pixel(source_x, source_y).0,
                );
            }
        }
        out
    }

    pub(super) fn composite_with_raster(
        &self,
        raster: &RgbaImage,
        skip: Option<usize>,
    ) -> RgbaImage {
        let mut out = if self.objects.is_empty() {
            raster.clone()
        } else {
            RgbaImage::new(self.image.width(), self.image.height())
        };
        for (index, object) in self.objects.iter().enumerate() {
            if skip != Some(index) {
                overlay(
                    &mut out,
                    &object.render(),
                    object.pos.0 as i64,
                    object.pos.1 as i64,
                );
            }
        }
        if !self.objects.is_empty() {
            overlay(&mut out, raster, 0, 0);
        }
        out
    }

    pub(super) fn bytes_with_fonts(&self, fonts: &mut FontMemory) -> usize {
        self.image.as_raw().len()
            + self
                .objects
                .iter()
                .map(|object| object.bytes_with_fonts(fonts))
                .sum::<usize>()
    }
}

impl Deref for Document {
    type Target = Layer;

    fn deref(&self) -> &Self::Target {
        self.active_layer()
    }
}

impl DerefMut for Document {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.active_layer_mut()
    }
}

pub(super) fn apply_opacity(image: &mut RgbaImage, opacity: u8) {
    if opacity != 255 {
        for pixel in image.pixels_mut() {
            pixel[3] = ((u16::from(pixel[3]) * u16::from(opacity) + 127) / 255) as u8;
        }
    }
}

pub(crate) fn validate_layers(layers: &[Layer], active: usize) -> Result<(), String> {
    if layers.is_empty() || layers.len() > MAX_LAYERS || active >= layers.len() {
        return Err("A project needs 1–64 layers and a valid active layer.".into());
    }
    let dimensions = layers[0].image.dimensions();
    if !valid_size(dimensions.0, dimensions.1) {
        return Err("The canvas must fit within 16 megapixels.".into());
    }
    if layers.iter().filter(|layer| layer.is_background).count() > 1 {
        return Err("A project can contain only one Background layer.".into());
    }
    let mut objects = ObjectBudget::default();
    let mut raster_bytes = 0usize;
    for layer in layers {
        if layer.name.trim().is_empty() || layer.name.len() > MAX_LAYER_NAME_BYTES {
            return Err("Layer names must contain 1–256 bytes of text.".into());
        }
        if layer.image.dimensions() != dimensions {
            return Err("Every layer must have the same canvas dimensions.".into());
        }
        for object in &layer.objects {
            objects.add(object)?;
        }
        raster_bytes = raster_bytes.saturating_add(layer.image.as_raw().len());
        if raster_bytes.saturating_add(objects.bytes) > MAX_LAYER_BYTES {
            return Err("Layers exceed the 192 MB image allocation limit.".into());
        }
    }
    Ok(())
}

impl Document {
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    pub fn active_layer_index(&self) -> usize {
        self.active_layer
    }

    pub fn active_layer(&self) -> &Layer {
        &self.layers[self.active_layer]
    }

    pub fn active_layer_mut(&mut self) -> &mut Layer {
        &mut self.layers[self.active_layer]
    }

    pub fn layer_mut(&mut self, index: usize) -> Option<&mut Layer> {
        self.layers.get_mut(index)
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn set_active_layer(&mut self, index: usize) -> Result<(), String> {
        if index >= self.layers.len() {
            return Err("That layer no longer exists.".into());
        }
        self.active_layer = index;
        Ok(())
    }

    pub(crate) fn from_layers(layers: Vec<Layer>, active: usize) -> Result<Self, String> {
        validate_layers(&layers, active)?;
        let mut document = Self::from_image(RgbaImage::new(1, 1));
        document.layers = layers;
        document.active_layer = active;
        Ok(document)
    }

    pub fn active_composite(&self) -> RgbaImage {
        self.active_composite_without(None)
    }

    pub fn active_composite_without(&self, skip: Option<usize>) -> RgbaImage {
        let mut out = self.active_layer().composite_with_raster(&self.image, skip);
        if self.mono {
            for pixel in out.pixels_mut() {
                let value = if u32::from(pixel[0]) * 299
                    + u32::from(pixel[1]) * 587
                    + u32::from(pixel[2]) * 114
                    >= 128000
                {
                    255
                } else {
                    0
                };
                *pixel = Rgba([value, value, value, pixel[3]]);
            }
        }
        out
    }

    /// Preview a new or edited retained object at its actual layer depth. A new
    /// object follows the current raster; a replacement keeps its stack slot.
    pub fn composite_with_object(&self, object: &Object, replace: Option<usize>) -> RgbaImage {
        let mut output: Option<RgbaImage> = None;
        for (index, layer) in self.layers.iter().enumerate() {
            if !layer.visible || layer.opacity == 0 {
                continue;
            }
            let mut pixels = if index == self.active_layer {
                let mut pixels = RgbaImage::new(self.image.width(), self.image.height());
                for (object_index, existing) in layer.objects.iter().enumerate() {
                    let next = if replace == Some(object_index) {
                        object
                    } else {
                        existing
                    };
                    overlay(
                        &mut pixels,
                        &next.render(),
                        i64::from(next.pos.0),
                        i64::from(next.pos.1),
                    );
                }
                overlay(&mut pixels, &layer.image, 0, 0);
                if replace.is_none() {
                    overlay(
                        &mut pixels,
                        &object.render(),
                        i64::from(object.pos.0),
                        i64::from(object.pos.1),
                    );
                }
                pixels
            } else {
                layer.composite()
            };
            apply_opacity(&mut pixels, layer.opacity);
            if let Some(out) = &mut output {
                overlay(out, &pixels, 0, 0);
            } else {
                output = Some(pixels);
            }
        }
        let mut out =
            output.unwrap_or_else(|| RgbaImage::new(self.image.width(), self.image.height()));
        if self.mono {
            for pixel in out.pixels_mut() {
                let value = if u32::from(pixel[0]) * 299
                    + u32::from(pixel[1]) * 587
                    + u32::from(pixel[2]) * 114
                    >= 128000
                {
                    255
                } else {
                    0
                };
                *pixel = Rgba([value, value, value, pixel[3]]);
            }
        }
        out
    }

    fn check_addition(&self, raster_bytes: usize, extra_objects: &[Object]) -> Result<(), String> {
        if self.layers.len() >= MAX_LAYERS {
            return Err("A picture can contain at most 64 layers.".into());
        }
        let mut budget = ObjectBudget::default();
        for object in self
            .layers
            .iter()
            .flat_map(|layer| &layer.objects)
            .chain(extra_objects)
        {
            budget.add(object)?;
        }
        let current_raster_bytes: usize = self
            .layers
            .iter()
            .map(|layer| layer.image.as_raw().len())
            .sum();
        if current_raster_bytes
            .saturating_add(raster_bytes)
            .saturating_add(budget.bytes)
            > MAX_LAYER_BYTES
        {
            return Err("Adding this layer would exceed the 192 MB image allocation limit.".into());
        }
        Ok(())
    }

    pub fn add_layer(&mut self) -> Result<usize, String> {
        self.check_addition(self.image.as_raw().len(), &[])?;
        let mut number = 1;
        let name = loop {
            let candidate = format!("Layer {number}");
            if !self.layers.iter().any(|layer| layer.name == candidate) {
                break candidate;
            }
            number += 1;
        };
        let mut layer = Layer::background(RgbaImage::new(self.image.width(), self.image.height()));
        layer.name = name;
        layer.is_background = false;
        let index = self.active_layer + 1;
        self.layers.insert(index, layer);
        self.active_layer = index;
        Ok(index)
    }

    pub fn duplicate_layer(&mut self, index: usize) -> Result<usize, String> {
        let layer = self
            .layers
            .get(index)
            .ok_or("That layer no longer exists.")?;
        self.check_addition(layer.image.as_raw().len(), &layer.objects)?;
        let mut copy = layer.clone();
        copy.name = format!("{} copy", copy.name.chars().take(60).collect::<String>());
        copy.is_background = false;
        copy.locked = false;
        self.layers.insert(index + 1, copy);
        self.active_layer = index + 1;
        Ok(index + 1)
    }

    pub fn delete_layer(&mut self, index: usize) -> Result<(), String> {
        let layer = self
            .layers
            .get(index)
            .ok_or("That layer no longer exists.")?;
        if self.layers.len() == 1 {
            return Err("The last layer cannot be deleted.".into());
        }
        if layer.locked {
            return Err("Unlock the layer before deleting it.".into());
        }
        self.layers.remove(index);
        if self.active_layer >= index {
            self.active_layer = self.active_layer.saturating_sub(1);
        }
        Ok(())
    }

    pub fn move_layer(&mut self, from: usize, to: usize) -> Result<(), String> {
        if from >= self.layers.len() || to >= self.layers.len() {
            return Err("That layer no longer exists.".into());
        }
        if from == to {
            return Ok(());
        }
        let layer = self.layers.remove(from);
        self.layers.insert(to, layer);
        self.active_layer = if self.active_layer == from {
            to
        } else if from < self.active_layer && self.active_layer <= to {
            self.active_layer - 1
        } else if to <= self.active_layer && self.active_layer < from {
            self.active_layer + 1
        } else {
            self.active_layer
        };
        Ok(())
    }

    pub fn merge_layer_down(&mut self, index: usize) -> Result<(), String> {
        if index == 0 || index >= self.layers.len() {
            return Err("Select a layer with another layer beneath it.".into());
        }
        let lower = &self.layers[index - 1];
        let upper = &self.layers[index];
        if lower.locked || upper.locked || !lower.visible || !upper.visible {
            return Err("Show and unlock both layers before merging.".into());
        }
        let mut merged = lower.clone();
        if lower.opacity == 255 && upper.opacity == 255 {
            // At full opacity, preserve editable text, images, and source pixels.
            if merged.image.pixels().any(|pixel| pixel[3] > 0) {
                merged.objects.push(Object::new(
                    ObjectKind::Raster(merged.image.clone()),
                    (0, 0),
                ));
            }
            merged.objects.extend(upper.objects.iter().cloned());
            merged.image = upper.image.clone();
        } else {
            // Group opacity cannot generally be distributed across overlapping
            // objects. Bake the explicit merge once; undo retains both sources.
            let mut image = lower.composite();
            apply_opacity(&mut image, lower.opacity);
            let mut top = upper.composite();
            apply_opacity(&mut top, upper.opacity);
            overlay(&mut image, &top, 0, 0);
            merged.image = image;
            merged.objects.clear();
        }
        merged.opacity = 255;
        merged.is_background |= upper.is_background;
        let mut budget = ObjectBudget::default();
        for object in self
            .layers
            .iter()
            .enumerate()
            .filter(|(layer_index, _)| *layer_index != index && *layer_index != index - 1)
            .flat_map(|(_, layer)| &layer.objects)
            .chain(&merged.objects)
        {
            budget.add(object)?;
        }
        let raster_bytes = self
            .layers
            .iter()
            .enumerate()
            .filter(|(layer_index, _)| *layer_index != index && *layer_index != index - 1)
            .map(|(_, layer)| layer.image.as_raw().len())
            .sum::<usize>()
            + merged.image.as_raw().len();
        if raster_bytes.saturating_add(budget.bytes) > MAX_LAYER_BYTES {
            return Err("Layers exceed the 192 MB image allocation limit.".into());
        }
        self.layers[index - 1] = merged;
        self.layers.remove(index);
        self.active_layer = index - 1;
        Ok(())
    }

    fn transform_layers(
        &mut self,
        mut transform: impl FnMut(&mut Layer) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut layers = self.layers.clone();
        for layer in &mut layers {
            transform(layer)?;
        }
        validate_layers(&layers, self.active_layer)?;
        self.layers = layers;
        Ok(())
    }

    pub fn resize_content(
        &mut self,
        width: u32,
        height: u32,
        sampling: ImageSampling,
    ) -> Result<(), String> {
        self.check_canvas_allocation(width, height)?;
        self.transform_layers(|layer| layer.resize_content(width, height, sampling))
    }

    pub fn crop_canvas(&mut self, crop: Region) -> Result<(), String> {
        self.transform_layers(|layer| layer.crop_canvas(crop))
    }

    pub fn rotate_content(&mut self, angle: f32, background: Color) -> Result<(), String> {
        let (width, height) = rotation_size(self.image.width(), self.image.height(), angle)
            .ok_or("The rotated picture would exceed the image size limit.")?;
        self.check_canvas_allocation(width, height)?;
        self.transform_layers(|layer| {
            let color = if layer.is_background {
                background
            } else {
                [0; 4]
            };
            layer.rotate_content(angle, color)
        })
    }

    pub fn flip_content(&mut self, horizontal: bool) {
        for layer in &mut self.layers {
            layer.flip_content(horizontal);
        }
    }

    pub fn skew_content(
        &mut self,
        x_degrees: f32,
        y_degrees: f32,
        background: Color,
    ) -> Result<(), String> {
        if !x_degrees.is_finite()
            || !y_degrees.is_finite()
            || x_degrees.abs() >= 90.0
            || y_degrees.abs() >= 90.0
        {
            return Err("Skew angles must be between -89 and 89 degrees.".into());
        }
        if x_degrees == 0.0 && y_degrees == 0.0 {
            return Ok(());
        }
        let skew = LinearTransform {
            xy: f64::from(x_degrees).to_radians().tan(),
            yx: f64::from(y_degrees).to_radians().tan(),
            ..Default::default()
        };
        if skew.determinant().abs() < 0.001 {
            return Err("These skew angles collapse the picture into a line.".into());
        }
        let (old_width, old_height) = self.image.dimensions();
        let (width, height) = skew
            .output_size(old_width, old_height)
            .ok_or("The skewed picture exceeds the image size limit.")?;
        self.check_canvas_allocation(width, height)?;
        self.transform_layers(|layer| {
            if layer.image.pixels().any(|pixel| pixel[3] > 0) {
                layer
                    .objects
                    .push(Object::new(ObjectKind::Raster(layer.image.clone()), (0, 0)));
            }
            for object in &mut layer.objects {
                object.clip_to_canvas([0.0, 0.0, f64::from(old_width), f64::from(old_height)])?;
                let (before_width, before_height) = object
                    .rendered_dimensions()
                    .ok_or("Invalid object dimensions.")?;
                let center_x = f64::from(object.pos.0) + f64::from(before_width) / 2.0
                    - f64::from(old_width) / 2.0;
                let center_y = f64::from(object.pos.1) + f64::from(before_height) / 2.0
                    - f64::from(old_height) / 2.0;
                object.skew_rendered(x_degrees, y_degrees)?;
                let (object_width, object_height) = object
                    .rendered_dimensions()
                    .ok_or("Invalid skewed object dimensions.")?;
                object.pos = (
                    (center_x
                        + skew.xy * center_y
                        + (f64::from(width) - f64::from(object_width)) / 2.0)
                        .round() as i32,
                    (skew.yx * center_x
                        + center_y
                        + (f64::from(height) - f64::from(object_height)) / 2.0)
                        .round() as i32,
                );
            }
            layer.image = RgbaImage::new(width, height);
            if layer.is_background && background[3] > 0 {
                let determinant = skew.determinant();
                for (x, y, pixel) in layer.image.enumerate_pixels_mut() {
                    let tx = f64::from(x) + 0.5 - f64::from(width) / 2.0;
                    let ty = f64::from(y) + 0.5 - f64::from(height) / 2.0;
                    let sx = (tx - skew.xy * ty) / determinant + f64::from(old_width) / 2.0;
                    let sy = (ty - skew.yx * tx) / determinant + f64::from(old_height) / 2.0;
                    if sx < 0.0
                        || sy < 0.0
                        || sx >= f64::from(old_width)
                        || sy >= f64::from(old_height)
                    {
                        *pixel = Rgba(background);
                    }
                }
            }
            Ok(())
        })
    }

    pub fn resize_canvas(
        &mut self,
        width: u32,
        height: u32,
        background: Color,
    ) -> Result<(), String> {
        self.check_canvas_allocation(width, height)?;
        self.transform_layers(|layer| {
            let color = if layer.is_background {
                background
            } else {
                [0; 4]
            };
            layer.resize_canvas(width, height, color)
        })
    }

    fn check_canvas_allocation(&self, width: u32, height: u32) -> Result<(), String> {
        if !valid_size(width, height)
            || u64::from(width) * u64::from(height) * 4 * self.layers.len() as u64
                > MAX_LAYER_BYTES as u64
        {
            return Err(
                "The canvas must fit within 16 megapixels and all layers within 192 MB.".into(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
