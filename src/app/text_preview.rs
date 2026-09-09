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

#[derive(Clone)]
struct LayeredPreview {
    object: Object,
    replace: Option<usize>,
    revision: u64,
    max_side: usize,
    tiles: Vec<PreviewTile>,
}

/// Replace the canvas image in its original paint slot. Painting a caption as
/// a foreground overlay would put it above higher layers and blend opacity twice.
pub(super) fn paint_layered(
    ui: &Ui,
    document: &Document,
    object: Object,
    replace: Option<usize>,
    revision: u64,
) {
    let Some(slot) = ui
        .ctx()
        .data(|data| data.get_temp::<canvas::CanvasImageSlot>(Id::new(canvas::CANVAS_IMAGE_SLOT)))
        .filter(|slot| slot.pass == ui.ctx().cumulative_pass_nr())
    else {
        return;
    };
    let id = Id::new("paint10-layered-text-preview");
    let max_side = ui.input(|input| input.max_texture_side).max(1);
    let mut cached = ui.ctx().data(|data| data.get_temp::<LayeredPreview>(id));
    if cached.as_ref().is_none_or(|preview| {
        preview.object != object
            || preview.replace != replace
            || preview.revision != revision
            || preview.max_side != max_side
    }) {
        let raster = document.composite_with_object(&object, replace);
        let mut tiles = Vec::new();
        for y in (0..raster.height()).step_by(max_side) {
            for x in (0..raster.width()).step_by(max_side) {
                let width = (raster.width() - x).min(max_side as u32);
                let height = (raster.height() - y).min(max_side as u32);
                let tile = image::imageops::crop_imm(&raster, x, y, width, height).to_image();
                let image = display::image([width as usize, height as usize], tile.as_raw());
                let texture = if let Some(previous) = cached
                    .as_mut()
                    .and_then(|preview| preview.tiles.get_mut(tiles.len()))
                {
                    previous.texture.set(image, TextureOptions::NEAREST);
                    previous.texture.clone()
                } else {
                    ui.ctx()
                        .load_texture("layered-live-text", image, TextureOptions::NEAREST)
                };
                tiles.push(PreviewTile {
                    offset: vec2(x as f32, y as f32),
                    texture,
                });
            }
        }
        cached = Some(LayeredPreview {
            object,
            replace,
            revision,
            max_side,
            tiles,
        });
        ui.ctx()
            .data_mut(|data| data.insert_temp(id, cached.clone().unwrap()));
    }
    let preview = cached.expect("initialized layered preview");
    let zoom = slot.rect.width() / document.image.width() as f32;
    slot.painter.set(
        slot.shape,
        egui::Shape::Vec(
            preview
                .tiles
                .iter()
                .map(|tile| {
                    egui::Shape::image(
                        tile.texture.id(),
                        Rect::from_min_size(
                            slot.rect.min + tile.offset * zoom,
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

pub(super) struct Interaction {
    cursor: egui::text_selection::TextCursorState,
    selection_color: Color32,
    cursor_style: egui::style::TextCursorStyle,
}

/// egui keeps logical scalar indices for editing, but its own selection painter
/// assumes increasing x coordinates and ASCII word boundaries. Unicode text
/// uses this bridge while retaining the same logical editing machinery.
pub(super) fn begin_interaction(
    ui: &mut Ui,
    text: &str,
    _format: &TextFormat,
) -> Option<Interaction> {
    let incoming_unicode = ui.input(|input| {
        input.events.iter().any(|event| match event {
            Event::Text(text)
            | Event::Paste(text)
            | Event::Ime(egui::ImeEvent::Preedit(text) | egui::ImeEvent::Commit(text)) => {
                !text.is_ascii()
            }
            _ => false,
        })
    });
    if text.is_ascii() && !incoming_unicode {
        return None;
    }
    let interaction = Interaction {
        cursor: TextEdit::load_state(ui.ctx(), Id::new("text_input"))
            .unwrap_or_default()
            .cursor,
        selection_color: ui.visuals().selection.bg_fill,
        cursor_style: ui.visuals().text_cursor.clone(),
    };
    ui.visuals_mut().selection.bg_fill = Color32::TRANSPARENT;
    ui.visuals_mut().text_cursor.stroke = Stroke::NONE;
    ui.visuals_mut().text_cursor.preview = false;
    Some(interaction)
}

pub(super) fn finish_interaction(
    ui: &mut Ui,
    output: &mut egui::text_edit::TextEditOutput,
    text: &str,
    format: &TextFormat,
    zoom: f32,
    interaction: Option<Interaction>,
) {
    let Some(mut interaction) = interaction else {
        return;
    };
    ui.visuals_mut().selection.bg_fill = interaction.selection_color;
    ui.visuals_mut().text_cursor = interaction.cursor_style;
    let layout = format.editor_layout(text);
    if let Some(pointer) = ui.ctx().pointer_interact_pos() {
        let local = (pointer - output.galley_pos) / zoom;
        let index = layout.hit_test(local.x, local.y);
        let cursor = output.galley.from_ccursor(egui::text::CCursor::new(index));
        if interaction.cursor.pointer_interaction(
            ui,
            &output.response,
            cursor,
            &output.galley,
            ui.ctx().is_being_dragged(output.response.id),
        ) {
            if output.response.double_clicked() {
                // A click near a glyph's trailing edge may resolve to the next
                // caret. Word selection belongs to the actual cell under the
                // pointer, including when its advance runs right-to-left.
                let character = layout
                    .rows
                    .iter()
                    .filter(|row| local.y >= row.top && local.y <= row.top + row.height)
                    .flat_map(|row| &row.glyphs)
                    .filter(|glyph| glyph.advance != 0.0)
                    .min_by(|a, b| {
                        let distance = |glyph: &crate::text::EditorGlyph| {
                            let left = glyph.x.min(glyph.x + glyph.advance);
                            let right = glyph.x.max(glyph.x + glyph.advance);
                            (left - local.x).max(0.0) + (local.x - right).max(0.0)
                        };
                        distance(a).total_cmp(&distance(b))
                    })
                    .map_or(index, |glyph| glyph.character_index);
                let range = text_editing::word_selection(text, character);
                interaction
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(range.start),
                        egui::text::CCursor::new(range.end),
                    )));
            }
            output.state.cursor = interaction.cursor;
            output.cursor_range = output.state.cursor.range(&output.galley);
            output.state.clone().store(ui.ctx(), output.response.id);
        }
    }
    if !output.response.has_focus() {
        return;
    }
    let Some(cursor) = output.cursor_range else {
        return;
    };
    let painter = ui.painter().with_clip_rect(output.text_clip_rect);
    let range = cursor.as_sorted_char_range();
    for selection in layout.selection_rects(range) {
        painter.rect_filled(
            Rect::from_min_size(
                output.galley_pos + vec2(selection.x, selection.y) * zoom,
                vec2(selection.width, selection.height) * zoom,
            ),
            0.0,
            interaction.selection_color,
        );
    }
    let caret = layout.caret(cursor.primary.ccursor.index);
    let rect = Rect::from_min_size(
        output.galley_pos + vec2(caret.x, caret.y) * zoom,
        vec2(0.0, caret.height * zoom),
    );
    let clock_id = output.response.id.with("visual-caret-clock");
    let now = ui.input(|input| input.time);
    let previous = ui
        .ctx()
        .data(|data| data.get_temp::<(usize, f64)>(clock_id));
    let changed =
        output.response.changed() || previous.is_none_or(|(index, _)| index != caret.index);
    let since = if changed { now } else { previous.unwrap().1 };
    ui.ctx()
        .data_mut(|data| data.insert_temp(clock_id, (caret.index, since)));
    if ui.input(|input| input.focused) {
        egui::text_selection::visuals::paint_text_cursor(ui, &painter, rect, now - since);
    }
    let transform = ui
        .ctx()
        .layer_transform_to_global(ui.layer_id())
        .unwrap_or_default();
    ui.output_mut(|output| {
        if let Some(ime) = &mut output.ime {
            ime.cursor_rect = transform * rect;
        }
    });
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
                let image = display::image([width as usize, height as usize], tile.as_raw());
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
            display::image(
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
    fn editing_transformed_layer_text_keeps_the_preview_aligned_with_the_caret() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.doc = Document::new(260, 180);
        app.doc.add_layer().unwrap();
        let mut object = Object::new(
            ObjectKind::Text {
                text: "Caption".into(),
                format: TextFormat {
                    width: 180,
                    ..Default::default()
                },
            },
            (20, 20),
        );
        object.angle = 27.0;
        object.scale = 1.4;
        let index = app.doc.add_object(object.clone());
        app.edit_text_object(index);
        frame(&mut app, &ctx, vec![]);
        let preview = ctx
            .data(|data| data.get_temp::<LayeredPreview>(Id::new("paint10-layered-text-preview")))
            .unwrap();
        assert_eq!(preview.object.angle, 0.0);
        assert_eq!(preview.object.scale, 1.0);
        assert_eq!(preview.object.pos, object.pos);
        assert_eq!(preview.object.transform, d::LinearTransform::default());
        app.commit_text();
        assert!(app.doc.objects[index] == object);
    }

    #[test]
    fn layered_live_text_matches_commit_and_stays_below_the_upper_layer() {
        for opacity in [255, 128] {
            let ctx = Context::default();
            let mut app = PaintApp::new_with_context(&ctx, false);
            app.doc = Document::new(240, 120);
            app.doc.add_layer().unwrap();
            app.doc.active_layer_mut().opacity = opacity;
            app.doc.add_layer().unwrap();
            let blue = [25, 80, 210, 255];
            for y in 0..120 {
                for x in 50..100 {
                    app.doc.image.put_pixel(x, y, Rgba(blue));
                }
            }
            app.doc.set_active_layer(1).unwrap();
            app.text_tab = true;
            app.text_edit = Some(TextEditState {
                index: None,
                origin: (10, 20),
                text: "Caption".into(),
                format: TextFormat {
                    width: 200,
                    size: 40.0,
                    color: BLACK,
                    ..Default::default()
                },
                focus: true,
                selection: 0..7,
                insertion_style: None,
                history: Default::default(),
                palette_colors: app.colors,
            });
            frame(&mut app, &ctx, vec![]);
            let output = frame(&mut app, &ctx, vec![Event::Text("Layer text".into())]);
            assert_eq!(app.text_edit.as_ref().unwrap().text, "Layer text");
            let preview = ctx
                .data(|data| {
                    data.get_temp::<LayeredPreview>(Id::new("paint10-layered-text-preview"))
                })
                .expect("live text must be composited at its layer depth");
            assert_eq!(preview.tiles.len(), 1);
            let texture = preview.tiles[0].texture.id();
            let delta = output
                .textures_delta
                .set
                .iter()
                .find(|(id, _)| *id == texture)
                .expect("typing must update the layer preview immediately");
            let egui::ImageData::Color(actual) = &delta.1.image else {
                panic!("color preview");
            };
            for y in 0..120 {
                for x in 50..100 {
                    assert_eq!(actual.pixels[y * 240 + x], Color32::from_rgb(25, 80, 210));
                }
            }
            fn uses_texture(shape: &egui::Shape, texture: TextureId) -> bool {
                match shape {
                    egui::Shape::Mesh(mesh) => mesh.texture_id == texture,
                    egui::Shape::Vec(shapes) => {
                        shapes.iter().any(|shape| uses_texture(shape, texture))
                    }
                    _ => false,
                }
            }
            assert_eq!(
                output
                    .shapes
                    .iter()
                    .filter(|shape| uses_texture(&shape.shape, texture))
                    .count(),
                1
            );
            app.commit_text();
            let committed = app.doc.composite();
            assert_eq!(
                actual.as_ref(),
                &display::image([240, 120], committed.as_raw())
            );

            app.edit_text_object(app.object.unwrap());
            frame(&mut app, &ctx, vec![]);
            let state = app.text_edit.as_mut().unwrap();
            state.text = "Edited".into();
            let output = frame(&mut app, &ctx, vec![]);
            let preview = ctx
                .data(|data| {
                    data.get_temp::<LayeredPreview>(Id::new("paint10-layered-text-preview"))
                })
                .unwrap();
            assert_eq!(preview.replace, Some(0));
            let delta = output
                .textures_delta
                .set
                .iter()
                .find(|(id, _)| *id == preview.tiles[0].texture.id())
                .unwrap();
            let egui::ImageData::Color(actual) = &delta.1.image else {
                panic!("color preview");
            };
            app.commit_text();
            assert_eq!(
                actual.as_ref(),
                &display::image([240, 120], app.doc.composite().as_raw())
            );
        }
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
            let expected = display::image(
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
                display::image(
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
