//! View commands arranged by what they change: zoom, canvas guides, and panels.

use super::ribbon_controls as controls;
use super::ribbon_layout::Group;
use super::*;

#[cfg(test)]
mod tests;

impl PaintApp {
    pub(in crate::app) fn view_ribbon(&mut self, ui: &mut Ui, origin: Pos2, ctx: &Context) {
        let groups = [
            Group {
                label: "Zoom",
                width: 280.0,
                icon: Icon::ZoomIn,
                keys: "ZZ",
                popup: "view_zoom",
            },
            Group {
                label: "Canvas",
                width: 144.0,
                icon: Icon::Rulers,
                keys: "ZC",
                popup: "view_canvas",
            },
            Group {
                label: "Panels",
                width: 212.0,
                icon: Icon::Layers,
                keys: "ZP",
                popup: "view_panels",
            },
            Group {
                label: "Display",
                width: 76.0,
                icon: Icon::Fullscreen,
                keys: "ZD",
                popup: "view_display",
            },
            Group {
                label: "Measure",
                width: 242.0,
                icon: Icon::Measure,
                keys: "ZM",
                popup: "view_measure",
            },
            Group {
                label: "Appearance",
                width: 154.0,
                icon: Icon::Appearance,
                keys: "ZA",
                popup: "view_appearance",
            },
        ];
        let widths = ribbon_layout::widths(
            &groups,
            ui.max_rect().right() - origin.x,
            &[4, 3, 2, 5, 1, 0],
        );
        let mut x = origin.x;
        for (index, (group, width)) in groups.into_iter().zip(widths).enumerate() {
            ribbon_layout::show(ui, pos2(x, origin.y), width, "view", group, |ui, o| {
                if !controls::current(ui).is_some_and(|scope| scope.name == "view_appearance") {
                    Self::group(ui, o, 0.0, group.width - 1.0, group.label);
                }
                match index {
                    0 => self.view_zoom_group(ui, o, ctx),
                    1 => self.view_canvas_group(ui, o),
                    2 => self.view_panels_group(ui, o, ctx),
                    3 => {
                        if tile(ui, o, "Full screen", Icon::Fullscreen, None, true)
                            .on_hover_text(
                                "View the picture full screen (F11). Esc returns to editing.",
                            )
                            .clicked()
                        {
                            self.show_picture(ctx);
                        }
                    }
                    4 => self.measure_group(ui, o, ctx),
                    _ => self.appearance_group(ui, o, ctx),
                }
            });
            x += width;
        }
    }

    fn appearance_group(&mut self, ui: &mut Ui, o: Pos2, ctx: &Context) {
        let before = (self.appearance, self.canvas_background);
        if controls::current(ui).is_some_and(|scope| scope.name == "view_appearance") {
            ui.scope_builder(
                UiBuilder::new().max_rect(Rect::from_min_size(o, vec2(194.0, 230.0))),
                |ui| {
                    theme::menu(ui);
                    theme::menu_heading(ui, "Theme", 194.0);
                    self.appearance_choices(ui, false);
                    theme::menu_heading(ui, "New canvas", 194.0);
                    self.appearance_choices(ui, true);
                },
            );
            if before != (self.appearance, self.canvas_background) {
                self.save_appearance(ctx);
            }
            return;
        }
        for (row, label, keys, scope, icon) in [
            (0, "Color theme", "A", "appearance_theme", Icon::Appearance),
            (1, "New canvas", "B", "appearance_canvas", Icon::New),
        ] {
            ui.scope_builder(UiBuilder::new().max_rect(Rect::from_min_size(
                o + vec2(4.0, 14.0 + row as f32 * 36.0), vec2(144.0, 28.0),
            )), |ui| {
                ribbon::ribbon_menu_button(ui, label, keys, scope, Some(icon), |ui| {
                    self.appearance_choices(ui, row != 0);
                }).response.on_hover_text(if row == 0 {
                    "Appearance is saved on this device. System follows your desktop or browser theme."
                } else {
                    "Background for new pictures, including saved exports. Existing pictures are unchanged."
                });
            });
        }
        if before != (self.appearance, self.canvas_background) {
            self.save_appearance(ctx);
        }
    }

    fn appearance_choices(&mut self, ui: &mut Ui, canvas: bool) {
        use crate::preferences::{Appearance, CanvasBackground};
        let appearances = [Appearance::System, Appearance::Light, Appearance::Dark];
        let backgrounds = [
            CanvasBackground::MatchTheme,
            CanvasBackground::White,
            CanvasBackground::Dark,
        ];
        let labels = if canvas {
            ["Match theme", "White canvas", "Dark canvas"]
        } else {
            ["System theme", "Light mode", "Dark mode"]
        };
        for (index, label) in labels.into_iter().enumerate() {
            let selected = if canvas {
                self.canvas_background == backgrounds[index]
            } else {
                self.appearance == appearances[index]
            };
            let choice = ui.add(theme::MenuItem::new(label).width(194.0).selected(selected));
            controls::named(ui, &choice, label);
            if choice.clicked() {
                if canvas {
                    self.canvas_background = backgrounds[index];
                } else {
                    self.appearance = appearances[index];
                }
                ui.close_menu();
            }
        }
    }

    fn view_zoom_group(&mut self, ui: &mut Ui, o: Pos2, ctx: &Context) {
        for (column, label, icon, factor, enabled) in [
            (0, "Zoom in", Icon::ZoomIn, 2.0, self.zoom < MAX_ZOOM),
            (1, "Zoom out", Icon::ZoomOut, 0.5, self.zoom > MIN_ZOOM),
        ] {
            if tile(
                ui,
                o + vec2(column as f32 * 68.0, 0.0),
                label,
                icon,
                None,
                enabled,
            )
            .clicked()
            {
                self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
            }
        }
        if tile(
            ui,
            o + vec2(136.0, 0.0),
            "100%",
            Icon::ActualSize,
            None,
            true,
        )
        .on_hover_text("Actual size: one picture pixel per canvas pixel.")
        .clicked()
        {
            self.zoom = 1.0;
        }
        if tile(
            ui,
            o + vec2(204.0, 0.0),
            "Fit to window",
            Icon::FitWindow,
            None,
            true,
        )
        .clicked()
        {
            let viewport = ctx
                .data(|data| data.get_temp::<Rect>(Id::new("paint10_canvas_viewport")))
                .unwrap_or_else(|| ctx.available_rect());
            let margin = 22.0 + if self.rulers { 22.0 } else { 0.0 };
            let space = (viewport.size() - Vec2::splat(margin)).max(Vec2::splat(1.0));
            self.zoom = (space.x / self.doc.image.width() as f32)
                .min(space.y / self.doc.image.height() as f32)
                .clamp(MIN_ZOOM, MAX_ZOOM);
            self.center_canvas_on(
                (
                    self.doc.image.width() as i32 / 2,
                    self.doc.image.height() as i32 / 2,
                ),
                ctx,
            );
        }
    }

    fn view_canvas_group(&mut self, ui: &mut Ui, o: Pos2) {
        if tile(ui, o, "Rulers", Icon::Rulers, Some(self.rulers), true)
            .on_hover_text("Show rulers along the top and left of the canvas.")
            .clicked()
        {
            self.rulers = !self.rulers;
        }
        if tile(
            ui,
            o + vec2(68.0, 0.0),
            "Gridlines",
            Icon::Grid,
            Some(self.grid),
            true,
        )
        .on_hover_text("Show pixel boundaries at 400% zoom and above.")
        .clicked()
        {
            self.grid = !self.grid;
        }
    }

    fn view_panels_group(&mut self, ui: &mut Ui, o: Pos2, ctx: &Context) {
        if tile(
            ui,
            o,
            "Layers",
            Icon::Layers,
            Some(self.layer_ui.open),
            true,
        )
        .clicked()
        {
            self.layer_ui.open = !self.layer_ui.open;
        }
        let thumbnail = Self::thumbnail_enabled(ctx);
        if tile(
            ui,
            o + vec2(68.0, 0.0),
            "Thumbnail",
            Icon::Thumbnail,
            Some(thumbnail),
            self.zoom > 1.0,
        )
        .on_hover_text("Show an overview for navigation when zoomed in beyond 100%.")
        .clicked()
        {
            Self::set_thumbnail_enabled(ctx, !thumbnail);
        }
        if tile(
            ui,
            o + vec2(136.0, 0.0),
            "Status bar",
            Icon::StatusBar,
            Some(self.status_bar),
            true,
        )
        .clicked()
        {
            self.status_bar = !self.status_bar;
        }
    }
}

/// Uniform icon tiles; toggle state is also exposed to assistive technology.
pub(super) fn tile(
    ui: &mut Ui,
    origin: Pos2,
    label: &str,
    icon: Icon,
    selected: Option<bool>,
    enabled: bool,
) -> Response {
    let response = controls::button(
        ui,
        label,
        Rect::from_min_size(origin + vec2(4.0, 5.0), vec2(64.0, 82.0)),
        icon,
        label,
        selected.unwrap_or(false),
        enabled,
    );
    if let Some(selected) = selected {
        response.widget_info(|| {
            WidgetInfo::selected(WidgetType::Button, response.enabled(), selected, label)
        });
    }
    if response.clicked() {
        ui.close_menu();
    }
    response
}
