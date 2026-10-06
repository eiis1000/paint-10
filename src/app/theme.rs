//! Shared desktop chrome: restrained borders, consistent focus, and readable menus.

use super::*;

use crate::preferences::{Appearance, CanvasBackground};

#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub text: Color32,
    pub muted: Color32,
    pub border: Color32,
    pub separator: Color32,
    pub title: Color32,
    pub ribbon: Color32,
    pub window: Color32,
    pub field: Color32,
    pub control: Color32,
    pub hover: Color32,
    pub selected: Color32,
    pub accent: Color32,
    pub workspace: Color32,
}

impl Palette {
    fn new(dark: bool) -> Self {
        if dark {
            Self {
                text: Color32::from_rgb(225, 229, 235),
                muted: Color32::from_rgb(164, 174, 186),
                border: Color32::from_rgb(82, 92, 105),
                separator: Color32::from_rgb(59, 66, 77),
                title: Color32::from_rgb(30, 34, 40),
                ribbon: Color32::from_rgb(39, 44, 52),
                window: Color32::from_rgb(43, 48, 57),
                field: Color32::from_rgb(29, 33, 40),
                control: Color32::from_rgb(48, 54, 64),
                hover: Color32::from_rgb(49, 67, 85),
                selected: Color32::from_rgb(47, 77, 103),
                accent: Color32::from_rgb(113, 179, 231),
                workspace: Color32::from_rgb(23, 26, 31),
            }
        } else {
            Self {
                text: Color32::from_rgb(32, 35, 39),
                muted: Color32::from_gray(103),
                border: Color32::from_rgb(171, 177, 184),
                separator: Color32::from_gray(220),
                title: Color32::WHITE,
                ribbon: Color32::from_rgb(245, 246, 247),
                window: Color32::from_rgb(250, 250, 250),
                field: Color32::WHITE,
                control: Color32::from_rgb(248, 249, 250),
                hover: Color32::from_rgb(229, 243, 255),
                selected: Color32::from_rgb(204, 232, 255),
                accent: BLUE,
                workspace: Color32::from_rgb(199, 211, 227),
            }
        }
    }
}

pub(super) fn palette(ctx: &Context) -> Palette {
    Palette::new(ctx.theme() == Theme::Dark)
}

pub(super) fn set_appearance(ctx: &Context, appearance: Appearance) {
    ctx.set_theme(match appearance {
        Appearance::System => ThemePreference::System,
        Appearance::Light => ThemePreference::Light,
        Appearance::Dark => ThemePreference::Dark,
    });
}

pub(super) const DARK_PAPER: Color = [38, 42, 49, 255];
const LIGHT_PAINT: Color = [232, 235, 240, 255];

impl PaintApp {
    pub(super) fn new_canvas(&mut self, ctx: &Context) {
        let dark = match self.canvas_background {
            CanvasBackground::MatchTheme => ctx.theme() == Theme::Dark,
            CanvasBackground::White => false,
            CanvasBackground::Dark => true,
        };
        let background = if dark { DARK_PAPER } else { WHITE };
        self.doc = Document::from_image(RgbaImage::from_pixel(900, 600, Rgba(background)));
        self.colors = [if dark { LIGHT_PAINT } else { BLACK }, background];
        self.refresh = true;
    }

    pub(super) fn save_appearance(&mut self, ctx: &Context) {
        set_appearance(ctx, self.appearance);
        ctx.request_repaint();
        if self.persist_preferences {
            if let Err(error) =
                crate::preferences::save_appearance(self.appearance, self.canvas_background)
            {
                self.message = format!("Could not save appearance: {error}");
            }
        }
    }
}

pub(super) fn install(ctx: &Context) {
    ctx.options_mut(|options| options.fallback_theme = Theme::Light);
    // Customize both slots so late browser/desktop theme events keep our style.
    for theme in [Theme::Light, Theme::Dark] {
        ctx.set_style_of(theme, style(theme == Theme::Dark));
    }
    ctx.set_theme(Theme::Light);
}

fn style(dark: bool) -> Style {
    let colors = Palette::new(dark);
    let mut style = Style {
        visuals: if dark {
            Visuals::dark()
        } else {
            Visuals::light()
        },
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
    style.visuals.window_fill = colors.window;
    style.visuals.panel_fill = colors.ribbon;
    style.visuals.window_stroke = Stroke::new(1.0_f32, colors.border);
    style.visuals.extreme_bg_color = colors.field;
    style.visuals.faint_bg_color = colors.control;
    style.visuals.selection.bg_fill = colors.selected;
    style.visuals.selection.stroke = Stroke::new(1.0_f32, colors.text);
    style.visuals.hyperlink_color = colors.accent;
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
        widget.fg_stroke = Stroke::new(1.0_f32, colors.text);
        widget.expansion = 0.0;
    }
    widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, colors.separator);
    widgets.inactive.weak_bg_fill = colors.control;
    widgets.inactive.bg_fill = colors.field;
    widgets.inactive.bg_stroke = Stroke::new(1.0_f32, colors.border);
    widgets.hovered.weak_bg_fill = colors.hover;
    widgets.hovered.bg_fill = colors.hover;
    widgets.hovered.bg_stroke = Stroke::new(1.0_f32, colors.accent);
    widgets.active.weak_bg_fill = colors.selected;
    widgets.active.bg_fill = colors.selected;
    widgets.active.bg_stroke = Stroke::new(1.0_f32, colors.accent);
    widgets.open.weak_bg_fill = colors.selected;
    widgets.open.bg_fill = colors.selected;
    widgets.open.bg_stroke = Stroke::new(1.0_f32, colors.accent);
    style
}

/// Called inside a popup, after egui applies its compact menu defaults.
pub(super) fn menu(ui: &mut Ui) {
    ui.spacing_mut().item_spacing = vec2(8.0, 1.0);
    ui.spacing_mut().button_padding = vec2(9.0, 5.0);
    ui.spacing_mut().interact_size.y = 28.0;
    let accent = palette(ui.ctx()).accent;
    ui.style_mut().visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, accent);
    ui.style_mut().visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, accent);
}

/// Egui strips idle control borders in menus. A collapsed ribbon is a panel
/// of inputs and buttons, so keep the same visible controls as the full ribbon.
pub(super) fn restore_widget_chrome(ui: &mut Ui) {
    let widgets = ui.ctx().style().visuals.widgets.clone();
    ui.visuals_mut().widgets = widgets;
}

/// Ribbon commands share a quiet idle state; hover, focus, and open menus
/// retain the same visible feedback as other controls.
pub(super) fn toolbar_button(ui: &mut Ui) {
    ui.spacing_mut().button_padding = Vec2::ZERO;
    let inactive = &mut ui.visuals_mut().widgets.inactive;
    inactive.weak_bg_fill = Color32::TRANSPARENT;
    inactive.bg_stroke = Stroke::NONE;
}

pub(super) fn menu_heading(ui: &mut Ui, label: &str, width: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 23.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, 0.0, palette(ui.ctx()).ribbon);
    ui.painter().text(
        rect.left_center() + vec2(8.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(11.0),
        palette(ui.ctx()).muted,
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
        let colors = palette(ui.ctx());
        let font = FontId::proportional(13.0);
        let shortcut =
            ui.painter()
                .layout_no_wrap(self.shortcut.to_owned(), font.clone(), colors.muted);
        let available = ui.ctx().screen_rect().width() - 24.0;
        let natural = ui
            .painter()
            .layout_no_wrap(self.label.to_owned(), font.clone(), colors.text);
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
                    if self.selected {
                        colors.selected
                    } else {
                        colors.hover
                    },
                    Stroke::new(1.0_f32, colors.accent),
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
                    Stroke::new(1.6_f32, colors.text),
                );
                painter.line_segment(
                    [center + vec2(-1.0, 3.0), center + vec2(5.0, -4.0)],
                    Stroke::new(1.6_f32, colors.text),
                );
            }
            let mut job =
                egui::text::LayoutJob::simple_singleline(self.label.to_owned(), font, colors.text);
            job.wrap.max_width = (width - shortcut.size().x - 52.0).max(20.0);
            job.wrap.max_rows = 1;
            let label = painter.layout_job(job);
            painter.galley(
                rect.left_center() + vec2(32.0, -label.size().y / 2.0),
                label,
                colors.text,
            );
            painter.galley(
                rect.right_center() - vec2(shortcut.size().x + 10.0, shortcut.size().y / 2.0),
                shortcut,
                colors.muted,
            );
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_theme_changes_keep_both_custom_styles_and_explicit_override() {
        let ctx = Context::default();
        install(&ctx);
        set_appearance(&ctx, Appearance::System);
        for theme in [Theme::Dark, Theme::Light, Theme::Dark] {
            let _ = ctx.run(
                RawInput {
                    system_theme: Some(theme),
                    ..Default::default()
                },
                |_| {},
            );
            assert_eq!(ctx.theme(), theme);
            assert_eq!(
                ctx.style().visuals.panel_fill,
                Palette::new(theme == Theme::Dark).ribbon
            );
            assert_eq!(
                ctx.style().spacing.interact_size.y,
                style(false).spacing.interact_size.y
            );
        }
        set_appearance(&ctx, Appearance::Light);
        let _ = ctx.run(
            RawInput {
                system_theme: Some(Theme::Dark),
                ..Default::default()
            },
            |_| {},
        );
        assert_eq!(ctx.theme(), Theme::Light);
    }

    #[test]
    fn theme_switch_keeps_picture_pixels_and_new_canvas_defaults_survive_project_export() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let original = app.doc.composite();
        app.appearance = Appearance::Dark;
        app.save_appearance(&ctx);
        assert_eq!(app.doc.composite(), original);
        assert!(!app.doc.dirty());
        for (theme, background, expected) in [
            (Appearance::Dark, CanvasBackground::MatchTheme, DARK_PAPER),
            (Appearance::Light, CanvasBackground::MatchTheme, WHITE),
            (Appearance::Dark, CanvasBackground::White, WHITE),
            (Appearance::Light, CanvasBackground::Dark, DARK_PAPER),
        ] {
            app.appearance = theme;
            app.canvas_background = background;
            app.save_appearance(&ctx);
            app.execute(Action::New, &ctx);
            assert!(app
                .doc
                .composite()
                .pixels()
                .all(|pixel| pixel.0 == expected));
            assert_eq!(app.editing_background(), expected);
            assert_ne!(app.colors[0], expected);
            assert!(!app.doc.dirty());
            assert!(!app.doc.can_undo());
            let loaded =
                crate::project::decode(&crate::project::encode(&app.doc).unwrap()).unwrap();
            assert_eq!(loaded.composite(), app.doc.composite());
        }
    }

    #[test]
    fn dark_text_and_selected_states_have_readable_contrast() {
        fn luminance(color: Color32) -> f32 {
            let [r, g, b, _] = color.to_array();
            let linear = |v: u8| {
                let v = v as f32 / 255.0;
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
        }
        let colors = Palette::new(true);
        for foreground in [colors.text, colors.muted] {
            for background in [
                colors.ribbon,
                colors.window,
                colors.field,
                colors.selected,
                colors.hover,
            ] {
                assert!((luminance(foreground) + 0.05) / (luminance(background) + 0.05) >= 3.0);
            }
        }
        assert!((luminance(colors.text) + 0.05) / (luminance(colors.selected) + 0.05) >= 4.5);
    }

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
