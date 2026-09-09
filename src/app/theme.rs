//! Shared desktop chrome: restrained borders, consistent focus, and readable menus.

use super::*;

const INK: Color32 = Color32::from_rgb(32, 35, 39);
const BORDER: Color32 = Color32::from_rgb(171, 177, 184);
const HOVER: Color32 = Color32::from_rgb(229, 243, 255);
const SELECTED: Color32 = Color32::from_rgb(204, 232, 255);

pub(super) fn install(ctx: &Context) {
    // The web runner reports the system theme after creating the application.
    // Select our intended theme first so that late event cannot switch away
    // from the customized style to egui's other, untouched style slot.
    ctx.set_theme(Theme::Light);
    let mut style = Style {
        visuals: Visuals::light(),
        ..Default::default()
    };
    for (role, size) in [
        (TextStyle::Body, 13.0),
        (TextStyle::Button, 13.0),
        (TextStyle::Small, 11.0),
        (TextStyle::Heading, 18.0),
    ] {
        style.text_styles.insert(role, FontId::proportional(size));
    }
    style.spacing.item_spacing = vec2(6.0, 4.0);
    style.spacing.button_padding = vec2(8.0, 4.0);
    style.spacing.window_margin = Margin::same(14);
    style.spacing.menu_margin = Margin::symmetric(4, 4);
    style.visuals.window_corner_radius = CornerRadius::ZERO;
    style.visuals.menu_corner_radius = CornerRadius::ZERO;
    style.visuals.window_fill = Color32::from_rgb(250, 250, 250);
    style.visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    style.visuals.extreme_bg_color = Color32::WHITE;
    style.visuals.selection.bg_fill = SELECTED;
    style.visuals.selection.stroke = Stroke::new(1.0_f32, INK);
    style.visuals.popup_shadow = egui::epaint::Shadow {
        offset: [2, 3],
        blur: 7,
        spread: 0,
        color: Color32::from_black_alpha(42),
    };
    style.visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 5],
        blur: 20,
        spread: 0,
        color: Color32::from_black_alpha(48),
    };
    let widgets = &mut style.visuals.widgets;
    for widget in [
        &mut widgets.noninteractive,
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        widget.corner_radius = CornerRadius::ZERO;
        widget.fg_stroke = Stroke::new(1.0_f32, INK);
        widget.expansion = 0.0;
    }
    widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_gray(220));
    widgets.inactive.weak_bg_fill = Color32::from_rgb(248, 249, 250);
    widgets.inactive.bg_fill = Color32::WHITE;
    widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    widgets.hovered.weak_bg_fill = HOVER;
    widgets.hovered.bg_fill = HOVER;
    widgets.hovered.bg_stroke = Stroke::new(1.0_f32, BLUE);
    widgets.active.weak_bg_fill = SELECTED;
    widgets.active.bg_fill = SELECTED;
    widgets.active.bg_stroke = Stroke::new(1.0_f32, BLUE);
    widgets.open.weak_bg_fill = SELECTED;
    widgets.open.bg_stroke = Stroke::new(1.0_f32, BLUE);
    ctx.set_style(style);
}

/// Called inside a popup, after egui applies its compact menu defaults.
pub(super) fn menu(ui: &mut Ui) {
    ui.spacing_mut().item_spacing = vec2(8.0, 1.0);
    ui.spacing_mut().button_padding = vec2(9.0, 5.0);
    ui.spacing_mut().interact_size.y = 28.0;
    ui.style_mut().visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, BLUE);
    ui.style_mut().visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, BLUE);
}

/// Egui strips idle control borders in menus. A collapsed ribbon is a panel
/// of inputs and buttons, so keep the same visible controls as the full ribbon.
pub(super) fn restore_widget_chrome(ui: &mut Ui) {
    let widgets = ui.ctx().style().visuals.widgets.clone();
    ui.visuals_mut().widgets = widgets;
}

pub(super) fn menu_heading(ui: &mut Ui, label: &str, width: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 23.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, Color32::from_gray(239));
    ui.painter().text(
        rect.left_center() + vec2(8.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(11.0),
        Color32::from_gray(86),
    );
}

pub(super) struct MenuItem<'a> {
    label: &'a str,
    shortcut: &'a str,
    selected: bool,
    icon: Option<Icon>,
    width: f32,
}

impl<'a> MenuItem<'a> {
    pub(super) fn new(label: &'a str) -> Self {
        Self {
            label,
            shortcut: "",
            selected: false,
            icon: None,
            width: 180.0,
        }
    }

    pub(super) fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub(super) fn shortcut(mut self, shortcut: &'a str) -> Self {
        self.shortcut = shortcut;
        self
    }

    pub(super) fn icon(mut self, icon: Option<Icon>) -> Self {
        self.icon = icon;
        self
    }

    pub(super) fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

impl Widget for MenuItem<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let font = FontId::proportional(13.0);
        let shortcut = ui.painter().layout_no_wrap(
            self.shortcut.to_owned(),
            font.clone(),
            Color32::from_gray(103),
        );
        let available = ui.ctx().screen_rect().width() - 24.0;
        let natural = ui
            .painter()
            .layout_no_wrap(self.label.to_owned(), font.clone(), INK);
        let width = self
            .width
            .max(natural.size().x + shortcut.size().x + 58.0)
            .min(available);
        let (rect, response) = ui.allocate_exact_size(vec2(width, 28.0), Sense::click());
        keytips::set_badge_anchor(ui, &response, rect.left_center() + vec2(14.0, 0.0));
        response.widget_info(|| {
            WidgetInfo::selected(
                WidgetType::Button,
                ui.is_enabled(),
                self.selected,
                self.label,
            )
        });
        if response.gained_focus() {
            response.scroll_to_me(None);
        }
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let active = response.hovered() || response.has_focus();
            if self.selected || active {
                painter.rect(
                    rect,
                    0.0,
                    if self.selected { SELECTED } else { HOVER },
                    Stroke::new(1.0_f32, Color32::from_rgb(145, 195, 233)),
                    StrokeKind::Inside,
                );
            }
            let center = rect.left_center() + vec2(14.0, 0.0);
            if let Some(icon) = self.icon {
                icons::draw(
                    painter,
                    Rect::from_center_size(center, vec2(18.0, 18.0)),
                    icon,
                );
            } else if self.selected {
                painter.line_segment(
                    [center + vec2(-4.0, 0.0), center + vec2(-1.0, 3.0)],
                    Stroke::new(1.6_f32, INK),
                );
                painter.line_segment(
                    [center + vec2(-1.0, 3.0), center + vec2(5.0, -4.0)],
                    Stroke::new(1.6_f32, INK),
                );
            }
            let mut job =
                egui::text::LayoutJob::simple_singleline(self.label.to_owned(), font, INK);
            job.wrap.max_width = (width - shortcut.size().x - 52.0).max(20.0);
            job.wrap.max_rows = 1;
            let label = painter.layout_job(job);
            painter.galley(
                rect.left_center() + vec2(32.0, -label.size().y / 2.0),
                label,
                INK,
            );
            painter.galley(
                rect.right_center() - vec2(shortcut.size().x + 10.0, shortcut.size().y / 2.0),
                shortcut,
                INK,
            );
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_system_theme_events_preserve_the_installed_menu_style() {
        let ctx = Context::default();
        install(&ctx);

        // Match WebRunner: create the app before the first frame reports the
        // browser theme, then deliver a later operating-system theme change.
        for system_theme in [Theme::Light, Theme::Dark, Theme::Light] {
            let mut button_rect = Rect::NOTHING;
            let output = ctx.run(
                RawInput {
                    system_theme: Some(system_theme),
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(500.0, 400.0))),
                    ..Default::default()
                },
                |ctx| {
                    CentralPanel::default().show(ctx, |ui| {
                        button_rect = ui.button("Menu appearance").rect;
                    });
                },
            );
            let button = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect) if rect.rect == button_rect => Some(rect),
                    _ => None,
                })
                .expect("the real button should paint its background");
            assert_eq!(button.corner_radius, CornerRadius::ZERO);
            assert_eq!(button.fill, Color32::from_rgb(248, 249, 250));
            assert_eq!(ctx.theme(), Theme::Light);
        }
    }
}
