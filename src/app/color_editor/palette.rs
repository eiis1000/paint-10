use super::*;

// ChooseColor-compatible RGB values; see the period-reference audit and
// Wine's comdlg32/colordlg.c predefcolors table. This is not sampled video RGB.
const BASIC_COLORS: [[u8; 3]; 48] = [
    [255, 128, 128],
    [255, 255, 128],
    [128, 255, 128],
    [0, 255, 128],
    [128, 255, 255],
    [0, 128, 255],
    [255, 128, 192],
    [255, 128, 255],
    [255, 0, 0],
    [255, 255, 0],
    [128, 255, 0],
    [0, 255, 64],
    [0, 255, 255],
    [0, 128, 192],
    [128, 128, 192],
    [255, 0, 255],
    [128, 64, 64],
    [255, 128, 64],
    [0, 255, 0],
    [0, 128, 128],
    [0, 64, 128],
    [128, 128, 255],
    [128, 0, 64],
    [255, 0, 128],
    [128, 0, 0],
    [255, 128, 0],
    [0, 128, 0],
    [0, 128, 64],
    [0, 0, 255],
    [0, 0, 160],
    [128, 0, 128],
    [128, 0, 255],
    [64, 0, 0],
    [128, 64, 0],
    [0, 64, 0],
    [0, 64, 64],
    [0, 0, 128],
    [0, 0, 64],
    [64, 0, 64],
    [64, 0, 128],
    [0, 0, 0],
    [128, 128, 0],
    [128, 128, 64],
    [128, 128, 128],
    [64, 128, 128],
    [192, 192, 192],
    [64, 0, 64],
    [255, 255, 255],
];

pub(super) fn palette_controls(ui: &mut Ui, state: &mut Editor, custom: &[Color]) -> Option<Color> {
    let mut recalled = None;
    ui.columns(2, |columns| {
        let ui = &mut columns[0];
        ui.label(RichText::new("Basic colors").small());
        Grid::new("dialog_basic_colors")
            .num_columns(8)
            .min_col_width(18.0)
            .spacing(vec2(4.0, 3.0))
            .show(ui, |ui| {
                for (index, [r, g, b]) in BASIC_COLORS.into_iter().enumerate() {
                    let label = format!("Basic color {}: #{r:02X}{g:02X}{b:02X}", index + 1);
                    if swatch(ui, [r, g, b, 255], vec2(18.0, 18.0), &label).clicked() {
                        state.set_rgb([r, g, b]);
                    }
                    if index % 8 == 7 {
                        ui.end_row();
                    }
                }
            });

        let ui = &mut columns[1];
        ui.label(RichText::new("Custom colors").small());
        Grid::new("dialog_custom_colors")
            .num_columns(8)
            .min_col_width(18.0)
            .spacing(vec2(4.0, 3.0))
            .show(ui, |ui| {
                for index in 0..crate::preferences::CUSTOM_COLOR_COUNT {
                    let color = custom.get(index).copied().unwrap_or(WHITE);
                    let label = format!(
                        "Custom color {}: {}",
                        index + 1,
                        color::format_hex(color, color[3] != 255)
                    );
                    let response = swatch(ui, color, vec2(18.0, 18.0), &label);
                    let selected = index == state.custom_slot;
                    response.widget_info(|| {
                        WidgetInfo::selected(WidgetType::Button, true, selected, &label)
                    });
                    if selected {
                        ui.painter().rect_stroke(
                            response.rect.expand(1.0),
                            0.0,
                            Stroke::new(2.0_f32, BLUE),
                            StrokeKind::Outside,
                        );
                    }
                    if response.clicked() {
                        state.custom_slot = index;
                        state.set_color(color);
                        recalled = Some(color);
                    }
                    if index % 8 == 7 {
                        ui.end_row();
                    }
                }
            });
        ui.add_space(6.0);
        ui.horizontal_top(|ui| {
            for original in [true, false] {
                ui.vertical(|ui| {
                    ui.label(if original { "Current" } else { "New" });
                    let rgba = if original { state.original } else { state.rgba };
                    let label = if original {
                        "Restore the original color"
                    } else {
                        "New color"
                    };
                    if swatch(ui, rgba, vec2(76.0, 53.0), label).clicked() && original {
                        state.set_color(state.original);
                    }
                });
            }
        });
    });
    recalled
}
