use super::*;
use crate::text::TextFormat;
use egui::epaint::text::{Glyph, Row, RowVisuals};
use egui::layers::ShapeIdx;
use std::sync::Arc;

#[derive(Clone)]
struct Preview {
    text: String,
    format: TextFormat,
    max_side: usize,
    tiles: Vec<PreviewTile>,
}

#[derive(Clone)]
struct PreviewTile {
    offset: Vec2,
    texture: TextureHandle,
}

/// Use the document renderer for live pixels as well as saved pixels. The
/// editor draws its translucent selection and caret over this same image.
pub(super) fn paint(
    ui: &Ui,
    slot: ShapeIdx,
    position: Pos2,
    text: &str,
    format: &TextFormat,
    zoom: f32,
) {
    let id = Id::new("paint10-live-text-preview");
    let max_side = ui.input(|input| input.max_texture_side).max(1);
    let mut cached = ui.ctx().data(|data| data.get_temp::<Preview>(id));
    if cached.as_ref().is_none_or(|cached| {
        cached.text != text || cached.format != *format || cached.max_side != max_side
    }) {
        let raster = format.render(text);
        let mut tiles = Vec::new();
        for y in (0..raster.height()).step_by(max_side) {
            for x in (0..raster.width()).step_by(max_side) {
                let width = (raster.width() - x).min(max_side as u32);
                let height = (raster.height() - y).min(max_side as u32);
                let tile = image::imageops::crop_imm(&raster, x, y, width, height).to_image();
                let image = ColorImage::from_rgba_unmultiplied(
                    [width as usize, height as usize],
                    tile.as_raw(),
                );
                let texture = if let Some(previous) = cached
                    .as_mut()
                    .and_then(|cached| cached.tiles.get_mut(tiles.len()))
                {
                    previous.texture.set(image, TextureOptions::NEAREST);
                    previous.texture.clone()
                } else {
                    ui.ctx()
                        .load_texture("live-text", image, TextureOptions::NEAREST)
                };
                tiles.push(PreviewTile {
                    offset: vec2(x as f32, y as f32),
                    texture,
                });
            }
        }
        cached = Some(Preview {
            text: text.to_owned(),
            format: format.clone(),
            max_side,
            tiles,
        });
        ui.ctx()
            .data_mut(|data| data.insert_temp(id, cached.clone().unwrap()));
    }
    let preview = cached.expect("initialized preview");
    ui.painter().set(
        slot,
        egui::Shape::Vec(
            preview
                .tiles
                .iter()
                .map(|tile| {
                    egui::Shape::image(
                        tile.texture.id(),
                        Rect::from_min_size(
                            position + tile.offset * zoom,
                            tile.texture.size_vec2() * zoom,
                        ),
                        Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                        Color32::WHITE,
                    )
                })
                .collect(),
        ),
    );
}

/// Adapt the document's character geometry to egui's editing machinery. Glyph
/// meshes stay empty: the shared raster above supplies every visible glyph.
pub(super) fn galley(ui: &Ui, text: &str, format: &TextFormat, zoom: f32) -> Arc<egui::Galley> {
    let layout = format.editor_layout(text);
    let job = egui::text::LayoutJob::simple(
        text.to_owned(),
        FontId::proportional(format.size * zoom),
        Color32::TRANSPARENT,
        layout.width * zoom,
    );
    let rows = layout
        .rows
        .into_iter()
        .map(|row| Row {
            section_index_at_start: 0,
            glyphs: row
                .glyphs
                .into_iter()
                .map(|glyph| Glyph {
                    chr: glyph.character,
                    pos: pos2(glyph.x, glyph.baseline) * zoom,
                    advance_width: glyph.advance * zoom,
                    line_height: glyph.height * zoom,
                    font_ascent: glyph.ascent * zoom,
                    font_height: glyph.height * zoom,
                    font_impl_ascent: glyph.ascent * zoom,
                    font_impl_height: glyph.height * zoom,
                    uv_rect: Default::default(),
                    section_index: 0,
                })
                .collect(),
            rect: Rect::from_min_size(
                pos2(row.left, row.top) * zoom,
                vec2(row.width, row.height) * zoom,
            ),
            visuals: RowVisuals::default(),
            ends_with_newline: row.ends_with_newline,
        })
        .collect();
    Arc::new(egui::Galley {
        job: Arc::new(job),
        rows,
        elided: layout.elided,
        rect: Rect::from_min_size(Pos2::ZERO, vec2(layout.width, layout.height) * zoom),
        mesh_bounds: Rect::NOTHING,
        num_vertices: 0,
        num_indices: 0,
        pixels_per_point: ui.ctx().pixels_per_point(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{points_to_pixels, TextAlignment};

    #[test]
    fn large_preview_tiles_reconstruct_the_exact_raster_at_a_small_gpu_limit() {
        let ctx = Context::default();
        let format = TextFormat {
            width: 1180,
            minimum_height: 1140,
            ..Default::default()
        };
        let expected = format.render("Several lines\nof editable\ntext 😀");
        let output = ctx.run(
            RawInput {
                max_texture_side: Some(1024),
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
                ..Default::default()
            },
            |ctx| {
                CentralPanel::default().show(ctx, |ui| {
                    let slot = ui.painter().add(egui::Shape::Noop);
                    paint(
                        ui,
                        slot,
                        Pos2::ZERO,
                        "Several lines\nof editable\ntext 😀",
                        &format,
                        1.0,
                    );
                });
            },
        );
        let preview = ctx
            .data(|data| data.get_temp::<Preview>(Id::new("paint10-live-text-preview")))
            .unwrap();
        assert!(preview.tiles.len() > 1);
        let mut actual =
            vec![Color32::TRANSPARENT; expected.width() as usize * expected.height() as usize];
        for tile in preview.tiles {
            let delta = output
                .textures_delta
                .set
                .iter()
                .find(|(id, _)| *id == tile.texture.id())
                .unwrap();
            let egui::ImageData::Color(image) = &delta.1.image else {
                panic!("color tile")
            };
            assert!(image.width() <= 1024 && image.height() <= 1024);
            for y in 0..image.height() {
                let target = (tile.offset.y as usize + y) * expected.width() as usize
                    + tile.offset.x as usize;
                actual[target..target + image.width()]
                    .copy_from_slice(&image.pixels[y * image.width()..(y + 1) * image.width()]);
            }
        }
        assert_eq!(
            actual,
            ColorImage::from_rgba_unmultiplied(
                [expected.width() as usize, expected.height() as usize],
                expected.as_raw()
            )
            .pixels
        );
    }

    fn frame(app: &mut PaintApp, ctx: &Context, events: Vec<Event>) -> FullOutput {
        ctx.run(
            RawInput {
                events,
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 800.0))),
                ..Default::default()
            },
            |ctx| {
                if !app.ribbon_keyboard(ctx) {
                    app.shortcut(ctx);
                }
                app.titlebar(ctx);
                app.ribbon(ctx);
                app.status(ctx);
                app.canvas(ctx);
                app.keyboard_menu(ctx);
                app.dialogs(ctx);
            },
        )
    }

    #[test]
    fn live_caption_pixels_match_the_committed_image_in_the_typing_frame() {
        for alignment in [
            TextAlignment::Left,
            TextAlignment::Center,
            TextAlignment::Right,
        ] {
            let ctx = Context::default();
            let mut app = PaintApp::new_with_context(&ctx, false);
            let format = TextFormat {
                width: 740,
                size: points_to_pixels(54.0),
                bold: true,
                italic: true,
                color: WHITE,
                alignment,
                outline_width: 3,
                ..Default::default()
            };
            app.text_tab = true;
            app.colors[0] = WHITE;
            app.text_edit = Some(TextEditState {
                index: None,
                origin: (20, 20),
                text: "Caption".into(),
                format,
                focus: true,
                selection: 0..7,
                insertion_style: None,
                history: Default::default(),
                palette_colors: app.colors,
            });
            frame(&mut app, &ctx, vec![]);
            let output = frame(&mut app, &ctx, vec![Event::Text("Caption test".into())]);
            let state = app.text_edit.as_ref().unwrap();
            assert_eq!(state.text, "Caption test");
            let expected = state.format.render(&state.text);
            let expected = ColorImage::from_rgba_unmultiplied(
                [expected.width() as usize, expected.height() as usize],
                expected.as_raw(),
            );
            let preview = ctx
                .data(|data| data.get_temp::<Preview>(Id::new("paint10-live-text-preview")))
                .unwrap();
            let delta = output
                .textures_delta
                .set
                .iter()
                .find(|(id, _)| *id == preview.tiles[0].texture.id())
                .expect("updated live text texture");
            let egui::ImageData::Color(image) = &delta.1.image else {
                panic!("color preview")
            };
            assert_eq!(
                image.as_ref(),
                &expected,
                "live pixels differ at {alignment:?}"
            );
            app.commit_text();
            let ObjectKind::Text { text, format } = &app.doc.objects[app.object.unwrap()].kind
            else {
                panic!("caption must remain editable");
            };
            let saved = format.render(text);
            assert_eq!(
                ColorImage::from_rgba_unmultiplied(
                    [saved.width() as usize, saved.height() as usize],
                    saved.as_raw()
                ),
                expected
            );
        }
    }

    #[test]
    fn clicking_a_centered_glyph_inserts_at_its_visible_position() {
        for zoom in [1.0, 4.0] {
            let ctx = Context::default();
            let mut app = PaintApp::new_with_context(&ctx, false);
            app.zoom = zoom;
            app.text_tab = true;
            app.text_edit = Some(TextEditState {
                index: None,
                origin: (20, 20),
                text: "Hello world".into(),
                format: TextFormat {
                    width: 240,
                    alignment: TextAlignment::Center,
                    outline_width: 3,
                    ..Default::default()
                },
                focus: true,
                selection: 0..0,
                insertion_style: None,
                history: Default::default(),
                palette_colors: app.colors,
            });
            frame(&mut app, &ctx, vec![]);
            frame(&mut app, &ctx, vec![]);
            let state = app.text_edit.as_ref().unwrap();
            let layout = state.format.editor_layout(&state.text);
            let glyph = &layout.rows[0].glyphs[6];
            let (px, py) = state.format.text_padding();
            let point = app.canvas_rect.min
                + vec2(
                    state.origin.0 as f32 + px as f32 + glyph.x + glyph.advance * 0.2,
                    state.origin.1 as f32 + py as f32 + glyph.baseline - glyph.ascent * 0.5,
                ) * zoom;
            for pressed in [true, false] {
                frame(
                    &mut app,
                    &ctx,
                    vec![
                        Event::PointerMoved(point),
                        Event::PointerButton {
                            pos: point,
                            button: PointerButton::Primary,
                            pressed,
                            modifiers: Modifiers::NONE,
                        },
                    ],
                );
            }
            frame(&mut app, &ctx, vec![Event::Text("!".into())]);
            assert_eq!(
                app.text_edit.as_ref().unwrap().text,
                "Hello !world",
                "zoom {zoom}"
            );
        }
    }
}
