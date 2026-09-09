use crate::printing::{PageSettings, PrintLayout};
use eframe::egui::{self, Color32, Rect, Sense, Stroke, StrokeKind, TextureHandle, Vec2};
use image::RgbaImage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewAction {
    Print,
    PageSetup,
    Close,
}

/// A snapshot keeps preview pages stable while page settings remain adjustable.
/// Construct it when opening Print Preview, then call `show` in the central panel.
pub struct PrintPreview {
    image: RgbaImage,
    texture: Option<TextureHandle>,
    page: u32,
    zoom: f32,
    fit: bool,
    two_pages: bool,
}

impl PrintPreview {
    pub fn new(image: RgbaImage) -> Self {
        Self {
            image,
            texture: None,
            page: 0,
            zoom: 100.0,
            fit: true,
            two_pages: false,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, settings: &PageSettings) -> Option<PreviewAction> {
        let mut action = None;
        let layout = settings.layout(&self.image);
        ui.horizontal_wrapped(|ui| {
            ui.strong("Print Preview");
            ui.separator();
            if ui.button("Print...").clicked() {
                action = Some(PreviewAction::Print);
            }
            if ui.button("Page setup...").clicked() {
                action = Some(PreviewAction::PageSetup);
            }
            if ui.button("Close print preview").clicked() {
                action = Some(PreviewAction::Close);
            }
        });
        let layout = match layout {
            Ok(layout) => layout,
            Err(error) => {
                ui.colored_label(Color32::DARK_RED, error);
                return action;
            }
        };
        self.page = self.page.min(layout.page_count() - 1);
        let step = if self.two_pages { 2 } else { 1 };
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(self.page > 0, egui::Button::new("Previous page"))
                .clicked()
            {
                self.page = self.page.saturating_sub(step);
            }
            if ui
                .add_enabled(
                    self.page + step < layout.page_count(),
                    egui::Button::new("Next page"),
                )
                .clicked()
            {
                self.page += step;
            }
            ui.label("Page");
            let mut number = self.page + 1;
            if ui
                .add(egui::DragValue::new(&mut number).range(1..=layout.page_count()))
                .changed()
            {
                self.page = number - 1;
            }
            ui.label(format!("of {}", layout.page_count()));
            ui.separator();
            if ui.selectable_label(!self.two_pages, "One page").clicked() {
                self.two_pages = false;
            }
            if ui.selectable_label(self.two_pages, "Two pages").clicked() {
                self.two_pages = true;
            }
            if ui.selectable_label(self.fit, "Fit page").clicked() {
                self.fit = true;
            }
            ui.scope(|ui| {
                ui.spacing_mut().slider_width = 90.0;
                if ui
                    .add(
                        egui::Slider::new(&mut self.zoom, 25.0..=200.0)
                            .suffix("%")
                            .integer(),
                    )
                    .changed()
                {
                    self.fit = false;
                }
            });
        });
        ui.separator();
        if ui.is_enabled() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            action = Some(PreviewAction::Close);
        }
        if ui.is_enabled() && !ui.ctx().wants_keyboard_input() {
            if ui.input(|i| {
                i.key_pressed(egui::Key::PageDown) || i.key_pressed(egui::Key::ArrowRight)
            }) {
                self.page = (self.page + step).min(layout.page_count() - 1);
            }
            if ui.input(|i| i.key_pressed(egui::Key::PageUp) || i.key_pressed(egui::Key::ArrowLeft))
            {
                self.page = self.page.saturating_sub(step);
            }
        }
        let texture = self.texture.get_or_insert_with(|| {
            let max_side = ui.ctx().input(|i| i.max_texture_side).min(4096) as u32;
            let resized;
            let pixels = if self.image.width().max(self.image.height()) > max_side {
                let factor = max_side as f64 / self.image.width().max(self.image.height()) as f64;
                resized = image::imageops::resize(
                    &self.image,
                    (self.image.width() as f64 * factor).round().max(1.0) as u32,
                    (self.image.height() as f64 * factor).round().max(1.0) as u32,
                    image::imageops::FilterType::Triangle,
                );
                &resized
            } else {
                &self.image
            };
            ui.ctx().load_texture(
                "print-preview-image",
                crate::display::image(
                    [pixels.width() as usize, pixels.height() as usize],
                    pixels.as_raw(),
                ),
                egui::TextureOptions::LINEAR,
            )
        });
        let available = ui.available_size();
        let visible = if self.two_pages && self.page + 1 < layout.page_count() {
            2
        } else {
            1
        };
        let gap = 20.0;
        let points = if self.fit {
            ((available.x - gap * (visible as f32 + 1.0)) / (layout.paper_width * visible as f32))
                .min((available.y - gap * 2.0) / layout.paper_height)
                .max(0.05)
        } else {
            // At 100% a printed inch occupies 96 logical screen pixels.
            self.zoom / 100.0 * 96.0 / 72.0
        };
        let paper_size = Vec2::new(layout.paper_width, layout.paper_height) * points;
        egui::Frame::NONE
            .fill(Color32::from_gray(105))
            .show(ui, |ui| {
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .id_salt("print-preview-scroll")
                    .show(ui, |ui| {
                        let content = Vec2::new(
                            paper_size.x * visible as f32 + gap * (visible as f32 + 1.0),
                            paper_size.y + gap * 2.0,
                        );
                        let space = content.max(ui.available_size());
                        let (bounds, _) = ui.allocate_exact_size(space, Sense::hover());
                        let start =
                            bounds.min + Vec2::new((space.x - content.x).max(0.0) / 2.0 + gap, gap);
                        for offset in 0..visible {
                            let page_rect = Rect::from_min_size(
                                start + Vec2::new(offset as f32 * (paper_size.x + gap), 0.0),
                                paper_size,
                            );
                            draw_page(
                                ui.painter(),
                                texture.id(),
                                &layout,
                                self.page + offset,
                                page_rect,
                                points,
                            );
                        }
                    });
            });
        action
    }
}

fn draw_page(
    painter: &egui::Painter,
    texture: egui::TextureId,
    layout: &PrintLayout,
    page: u32,
    paper: Rect,
    scale: f32,
) {
    painter.rect_filled(
        paper.translate(Vec2::splat(4.0)),
        0.0,
        Color32::from_black_alpha(65),
    );
    painter.rect_filled(paper, 0.0, Color32::WHITE);
    painter.rect_stroke(
        paper,
        0.0,
        Stroke::new(1.0_f32, Color32::from_gray(50)),
        StrokeKind::Outside,
    );
    let printable = Rect::from_min_size(
        paper.min + Vec2::new(layout.left, layout.top) * scale,
        Vec2::new(layout.printable_width, layout.printable_height) * scale,
    );
    let (x, y) = layout.image_origin(page);
    let image_rect = Rect::from_min_size(
        paper.min + Vec2::new(x, y) * scale,
        Vec2::new(layout.image_width, layout.image_height) * scale,
    );
    painter
        .with_clip_rect(printable.intersect(painter.clip_rect()))
        .image(
            texture,
            image_rect,
            Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
}
