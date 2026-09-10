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
        ];
        let widths =
            ribbon_layout::widths(&groups, ui.max_rect().right() - origin.x, &[4, 3, 2, 1, 0]);
        let mut x = origin.x;
        for (index, (group, width)) in groups.into_iter().zip(widths).enumerate() {
            ribbon_layout::show(ui, pos2(x, origin.y), width, "view", group, |ui, o| {
                Self::group(ui, o, 0.0, group.width - 1.0, group.label);
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
                    _ => self.measure_group(ui, o, ctx),
                }
            });
            x += width;
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
