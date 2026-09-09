use super::*;

pub(super) fn visual_picker(ui: &mut Ui, state: &mut Editor, height: f32) {
    let plane = Plane {
        space: state.space,
        slice: state.slice,
    };
    let axes = plane.axes();
    let channels = state.space.channels();
    let mut values = state.values;
    let previous = values;
    let strip_count = if state.space == Space::Cmyk { 2 } else { 1 };
    let (area, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let rect = Rect::from_min_max(
        area.min + vec2(0.0, 16.0),
        area.max - vec2(strip_count as f32 * 24.0, 14.0),
    );
    let response = ui.interact(rect, ui.id().with("color_plane"), Sense::click_and_drag());
    let label = format!(
        "{} and {}",
        channels[axes[0]].label,
        channels[axes[1]].label.to_ascii_lowercase()
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, ui.is_enabled(), &label));
    if !state.numeric_invalid && (response.clicked() || response.dragged()) {
        if let Some(point) = response.interact_pointer_pos() {
            let x = ((point.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64;
            let y = (1.0 - (point.y - rect.top()) / rect.height()).clamp(0.0, 1.0) as f64;
            values = plane.coordinates_at(values, x, y);
        }
    }
    let texture = plane_texture(ui, rect, plane, values);
    ui.painter().image(
        texture,
        rect,
        Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    border(ui.painter(), rect);
    let [x, y] = plane.position(values);
    let position = vec2(x.clamp(0.0, 1.0) as f32, (1.0 - y.clamp(0.0, 1.0)) as f32);
    let painter = ui.painter().with_clip_rect(rect);
    let position = rect.min + position * rect.size();
    painter.circle_stroke(position, 5.0, Stroke::new(3.0_f32, Color32::WHITE));
    painter.circle_stroke(position, 5.0, Stroke::new(1.0_f32, Color32::BLACK));
    caption(
        ui,
        pos2(rect.left(), area.top()),
        Align2::LEFT_TOP,
        &format!(
            "{} right · {} up",
            abbreviation(channels[axes[0]].label),
            abbreviation(channels[axes[1]].label)
        ),
    );
    let outside_view = !((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y));
    let footer = if outside_view {
        "Outside view".to_owned()
    } else if matches!(state.space, Space::Oklab | Space::Oklch) {
        "Hatching: outside sRGB".to_owned()
    } else {
        format!(
            "{} – {}{}",
            channels[axes[0]].min, channels[axes[0]].max, channels[axes[0]].suffix
        )
    };
    caption(
        ui,
        pos2(rect.left(), rect.bottom() + 2.0),
        Align2::LEFT_TOP,
        &footer,
    );
    response.on_hover_text(format!(
        "{} increases left to right ({}–{}{}).\n{} increases bottom to top ({}–{}{}).\n{}",
        channels[axes[0]].label, channels[axes[0]].min, channels[axes[0]].max, channels[axes[0]].suffix,
        channels[axes[1]].label, channels[axes[1]].min, channels[axes[1]].max, channels[axes[1]].suffix,
        if state.slice == Slice::PaintSpectrum {
            "Paint's spectrum is shown at luminosity 120; the strip edits the actual luminosity."
        } else {
            "The strips set the fixed coordinates. Hatched colors are outside sRGB; numeric entry remains available."
        }
    ));

    for (index, channel) in [plane.slice.channel(), 3]
        .into_iter()
        .take(strip_count)
        .enumerate()
    {
        let strip = Rect::from_min_size(
            pos2(rect.right() + 6.0 + index as f32 * 24.0, rect.top()),
            vec2(18.0, rect.height()),
        );
        let response = ui.interact(
            strip,
            ui.id().with(("color_strip", channel)),
            Sense::click_and_drag(),
        );
        let label = format!("{} color strip", channels[channel].label);
        response.widget_info(|| WidgetInfo::slider(ui.is_enabled(), values[channel], &label));
        if !state.numeric_invalid && (response.clicked() || response.dragged()) {
            if let Some(point) = response.interact_pointer_pos() {
                let fraction =
                    (1.0 - (point.y - strip.top()) / strip.height()).clamp(0.0, 1.0) as f64;
                values = plane.strip_coordinates_at(values, channel, fraction);
            }
        }
        gradient_mesh(ui.painter(), strip, 1, 48, |_, y| {
            rgb32(plane.strip_color_at(values, channel, 1.0 - y).rgb)
        });
        let fraction = plane.fraction(channel, values[channel]).clamp(0.0, 1.0);
        marker(
            ui.painter(),
            strip,
            strip.bottom() - fraction as f32 * strip.height(),
        );
        caption(
            ui,
            pos2(strip.center().x, area.top()),
            Align2::CENTER_TOP,
            abbreviation(channels[channel].label),
        );
        caption(
            ui,
            pos2(strip.center().x, strip.bottom() + 2.0),
            Align2::CENTER_TOP,
            &format!(
                "{:.precision$}",
                values[channel],
                precision = channels[channel].decimals.min(2)
            ),
        );
        response.on_hover_text(format!(
            "{}: {}–{}{}. Drag upward to increase.",
            channels[channel].label,
            channels[channel].min,
            channels[channel].max,
            channels[channel].suffix
        ));
    }
    if values != previous {
        state.apply_coordinates(state.space, values);
    }
}

#[derive(Clone)]
struct PlaneTexture {
    plane: Plane,
    fixed: [f64; 4],
    size: [usize; 2],
    texture: TextureHandle,
}

fn plane_texture(ui: &Ui, rect: Rect, plane: Plane, values: [f64; 4]) -> TextureId {
    let key = ui.id().with("color_plane_texture");
    let fixed = plane.fixed_coordinates(values);
    let size = [
        (rect.width() * ui.ctx().pixels_per_point())
            .ceil()
            .clamp(2.0, 320.0) as usize,
        (rect.height() * ui.ctx().pixels_per_point())
            .ceil()
            .clamp(2.0, 240.0) as usize,
    ];
    let previous = ui.ctx().data(|data| data.get_temp::<PlaneTexture>(key));
    if let Some(cached) = &previous {
        if cached.plane == plane && cached.fixed == fixed && cached.size == size {
            return cached.texture.id();
        }
    }
    let mut pixels = Vec::with_capacity(size[0] * size[1]);
    for row in 0..size[1] {
        for column in 0..size[0] {
            let color = plane.color_at(
                values,
                column as f64 / (size[0] - 1) as f64,
                1.0 - row as f64 / (size[1] - 1) as f64,
            );
            let rgb = if color.in_gamut {
                color.rgb
            } else {
                // Keep the clipped hue recognizable while marking unavailable
                // colors. The requested coordinates remain editable and intact.
                let opacity = if (column + row) % 10 < 3 { 0.65 } else { 0.15 };
                color.rgb.map(|channel| {
                    (channel as f64 * (1.0 - opacity) + 200.0 * opacity).round() as u8
                })
            };
            pixels.push(rgb32(rgb));
        }
    }
    let image = ColorImage { size, pixels };
    let texture = if let Some(mut previous) = previous {
        previous.texture.set(image, TextureOptions::LINEAR);
        previous.texture
    } else {
        ui.ctx()
            .load_texture("color-coordinate-plane", image, TextureOptions::LINEAR)
    };
    let id = texture.id();
    ui.ctx().data_mut(|data| {
        data.insert_temp(
            key,
            PlaneTexture {
                plane,
                fixed,
                size,
                texture,
            },
        )
    });
    id
}

fn abbreviation(label: &str) -> &str {
    match label {
        "Red" => "R",
        "Green" => "G",
        "Blue" => "B",
        "Hue" => "H",
        "Saturation" => "S",
        "Lightness" => "L",
        "Luminosity" => "Lum",
        "Chroma" | "Cyan" => "C",
        "Magenta" => "M",
        "Yellow" => "Y",
        "Black" => "K",
        other => other,
    }
}

fn caption(ui: &Ui, position: Pos2, align: Align2, text: &str) {
    ui.painter().text(
        position,
        align,
        text,
        FontId::proportional(10.0),
        Color32::from_gray(80),
    );
}

fn border(painter: &Painter, rect: Rect) {
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0_f32, Color32::from_gray(140)),
        StrokeKind::Inside,
    );
}

fn rgb32([r, g, b]: [u8; 3]) -> Color32 {
    Color32::from_rgb(r, g, b)
}

pub(super) fn gradient_mesh(
    painter: &Painter,
    rect: Rect,
    columns: u32,
    rows: u32,
    color_at: impl Fn(f64, f64) -> Color32,
) {
    let mut mesh = Mesh::default();
    for row in 0..=rows {
        for column in 0..=columns {
            let x = column as f64 / columns as f64;
            let y = row as f64 / rows as f64;
            mesh.colored_vertex(
                rect.min + vec2(x as f32, y as f32) * rect.size(),
                color_at(x, y),
            );
            if row < rows && column < columns {
                let index = row * (columns + 1) + column;
                mesh.add_triangle(index, index + 1, index + columns + 1);
                mesh.add_triangle(index + 1, index + columns + 2, index + columns + 1);
            }
        }
    }
    painter.add(mesh);
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0_f32, Color32::from_gray(140)),
        StrokeKind::Inside,
    );
}

fn marker(painter: &Painter, rect: Rect, y: f32) {
    let marker = Rect::from_center_size(pos2(rect.center().x, y), vec2(rect.width() + 2.0, 3.0));
    painter.rect_stroke(
        marker,
        0.0,
        Stroke::new(2.0_f32, Color32::WHITE),
        StrokeKind::Middle,
    );
    painter.rect_stroke(
        marker,
        0.0,
        Stroke::new(1.0_f32, Color32::BLACK),
        StrokeKind::Middle,
    );
}
