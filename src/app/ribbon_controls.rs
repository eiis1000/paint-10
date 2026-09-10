//! Shared registration for the actual ribbon widgets, including those reused
//! inside collapsed groups. Scope follows the synchronous UI construction stack.

use super::*;

#[cfg(test)]
mod split_tests;

#[derive(Clone, Copy)]
pub(super) struct Scope {
    pub name: &'static str,
    pub group: &'static str,
}

impl Scope {
    pub const fn new(name: &'static str, group: &'static str) -> Self {
        Self { name, group }
    }
}

pub(super) fn scope<R>(ui: &mut Ui, current: Scope, contents: impl FnOnce(&mut Ui) -> R) -> R {
    let id = Id::new("paint10-ribbon-widget-scope");
    let previous = ui.ctx().data_mut(|data| {
        let previous = data.get_temp::<Scope>(id);
        data.insert_temp(id, current);
        previous
    });
    let result = contents(ui);
    ui.ctx().data_mut(|data| {
        if let Some(previous) = previous {
            data.insert_temp(id, previous);
        } else {
            data.remove::<Scope>(id);
        }
    });
    result
}

pub(super) fn current(ui: &Ui) -> Option<Scope> {
    ui.ctx()
        .data(|data| data.get_temp(Id::new("paint10-ribbon-widget-scope")))
}

pub(super) fn register(ui: &Ui, response: &Response, keys: &str, kind: keytips::Kind) {
    if let Some(scope) = current(ui) {
        keytips::register(ui, response, scope.name, scope.group, keys, kind);
    }
}

pub(super) fn named(ui: &Ui, response: &Response, label: &str) {
    let Some(scope) = current(ui) else { return };
    if matches!(
        scope.name,
        "paste" | "select" | "rotate" | "outline" | "fill" | "size"
    ) {
        register(ui, response, &next_choice(ui, scope), keytips::Kind::Button);
        return;
    }
    let key = match label {
        "Paste" => "V",
        "Cut" => "X",
        "Copy" => "C",
        "Select" => "S",
        "Crop" => "H",
        "Resize" => "RE",
        "Pencil" => "P",
        "Fill with color" => "F",
        "Text" => "T",
        "Eraser" => "E",
        "Color picker" => "I",
        "Magnifier" => "M",
        "Brushes" => "B",
        "Edit colors" => {
            if scope.name.starts_with("text") {
                "EC"
            } else {
                "D"
            }
        }
        "Scroll shapes up" => "GU",
        "Scroll shapes down" => "GD",
        "Zoom in" => "I",
        "Zoom out" => "O",
        "100%" => "1",
        "Rulers" => "R",
        "Gridlines" => "G",
        "Status bar" => "S",
        "Layers" => "L",
        "Full screen" => "F",
        "Measure distance" => "M",
        "Thumbnail" => "T",
        "Fit to window" => "W",
        "Font" => "FF",
        "Font list" => "FL",
        "Font size" => "FS",
        "Bold" => "B",
        "Italic" => "I",
        "Underline" => "U",
        "Strikeout" => "K",
        "Opaque" => "O",
        "Transparent" => "T",
        "Done" => "D",
        "Cancel" => "X",
        _ => "",
    };
    let key = if !key.is_empty() {
        key.to_owned()
    } else if let Some(index) = Tool::SHAPES.iter().position(|tool| tool.name() == label) {
        format!("J{}", char::from(b'A' + index as u8))
    } else if let Some(index) = Brush::ALL.iter().position(|brush| brush.name() == label) {
        (index + 1).to_string()
    } else {
        next_choice(ui, scope)
    };
    let kind = match label {
        "Font" => keytips::Kind::TextInput,
        "Font size" => keytips::Kind::NumericInput,
        _ => keytips::Kind::Button,
    };
    register(ui, response, &key, kind);
}

pub(super) fn palette_key(index: usize) -> String {
    let suffix = if index < 26 {
        char::from(b'A' + index as u8)
    } else {
        char::from(b'0' + (index - 26) as u8)
    };
    format!("A{suffix}")
}

fn next_choice(ui: &Ui, scope: Scope) -> String {
    let id = ui.id().with(("keytip-choice", scope.name));
    let pass = ui.ctx().cumulative_pass_nr();
    let index = ui.ctx().data_mut(|data| {
        let (previous, count) = data.get_temp::<(u64, usize)>(id).unwrap_or((pass, 0));
        let index = if previous == pass { count + 1 } else { 1 };
        data.insert_temp(id, (pass, index));
        index
    });
    index.to_string()
}

pub(super) fn button(
    ui: &mut Ui,
    id: impl std::hash::Hash,
    rect: Rect,
    icon: Icon,
    label: &str,
    selected: bool,
    enabled: bool,
) -> Response {
    let response = icons::button(ui, id, rect, icon, label, selected, enabled);
    named(
        ui,
        &response,
        if label.is_empty() { icon.name() } else { label },
    );
    if response.clicked() && current(ui).is_some_and(|scope| scope.name.starts_with("home_")) {
        ui.close_menu();
    }
    response
}

pub(super) struct SplitMenu<'a> {
    pub label: &'a str,
    pub keys: &'static str,
    pub scope: &'static str,
}

pub(super) struct SplitButton<'a> {
    pub id: &'static str,
    pub rect: Rect,
    pub icon: Icon,
    pub label: &'a str,
    pub selected: bool,
    pub menu: SplitMenu<'a>,
}

/// A connected ribbon tile with independent main-action and dropdown targets.
/// Both regions keep their ordinary widget responses and keytip registration.
pub(super) fn split_button(
    ui: &mut Ui,
    split: SplitButton<'_>,
    contents: impl FnOnce(&mut Ui),
) -> Response {
    let divider = split.rect.bottom() - 20.0;
    let main_rect = Rect::from_min_max(split.rect.min, pos2(split.rect.right(), divider));
    let menu_rect = Rect::from_min_max(pos2(split.rect.left(), divider), split.rect.max);
    let background = ui.painter().add(egui::Shape::Noop);
    let main = button(
        ui,
        split.id,
        main_rect,
        split.icon,
        split.label,
        split.selected,
        true,
    );
    let mut menu_ui = ui.new_child(
        UiBuilder::new()
            .id_salt((split.id, "split-menu"))
            .max_rect(menu_rect),
    );
    menu_ui.spacing_mut().button_padding = Vec2::ZERO;
    menu_ui.spacing_mut().interact_size.y = menu_rect.height();
    let group = current(ui).map_or("Menu", |scope| scope.group);
    let menu = egui::menu::menu_custom_button(
        &mut menu_ui,
        Button::new("").frame(false).min_size(menu_rect.size()),
        |ui| {
            theme::menu(ui);
            scope(ui, Scope::new(split.menu.scope, group), contents);
        },
    );
    register(
        &menu_ui,
        &menu.response,
        split.menu.keys,
        keytips::Kind::Menu {
            scope: split.menu.scope,
        },
    );
    menu.response
        .widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), split.menu.label));
    icons::draw(
        ui.painter(),
        Rect::from_center_size(menu.response.rect.center(), Vec2::splat(12.0)),
        Icon::ChevronDown,
    );
    if split.selected
        || main.hovered()
        || main.has_focus()
        || menu.response.hovered()
        || menu.response.has_focus()
        || menu.inner.is_some()
    {
        let border = Stroke::new(1.0_f32, Color32::from_rgb(125, 181, 224));
        let fill = if split.selected || menu.inner.is_some() {
            Color32::from_rgb(206, 231, 252)
        } else {
            Color32::from_rgb(229, 243, 255)
        };
        ui.painter()
            .set(background, egui::Shape::rect_filled(split.rect, 0.0, fill));
        ui.painter()
            .rect_stroke(split.rect, 0.0, border, StrokeKind::Inside);
        ui.painter().hline(split.rect.x_range(), divider, border);
    }
    menu.response.on_hover_text(split.menu.label);
    main
}

pub(super) fn command(ui: &mut Ui, label: &str) -> Response {
    let popup =
        current(ui).is_some_and(|scope| matches!(scope.name, "paste" | "select" | "rotate"));
    let response = if popup {
        ui.add(theme::MenuItem::new(label).width(245.0))
    } else {
        ui.button(label)
    };
    named(ui, &response, label);
    response
}

/// A compact ribbon action, sharing its gutters with ribbon dropdowns.
pub(super) fn icon_row(
    ui: &mut Ui,
    rect: Rect,
    icon: Icon,
    label: &str,
    selected: Option<bool>,
    enabled: bool,
) -> Response {
    let mut builder = UiBuilder::new().id_salt(label).max_rect(rect);
    if !enabled {
        builder = builder.disabled();
    }
    let mut child = ui.new_child(builder);
    let response = child.add_sized(
        rect.size(),
        Button::new("")
            .frame(false)
            .selected(selected.unwrap_or(false)),
    );
    // The disabled child already fades widget chrome. Paint the custom artwork
    // from the parent so applying opacity does not fade icons and labels twice.
    let mut painter = ui.painter().clone();
    if !response.enabled() && ui.is_enabled() {
        painter.set_opacity(0.4);
    }
    icons::draw(
        &painter,
        Rect::from_min_size(rect.min + vec2(4.0, 4.0), vec2(18.0, 18.0)),
        icon,
    );
    painter.text(
        rect.min + vec2(28.0, 13.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        Color32::from_gray(35),
    );
    if response.has_focus() {
        painter.rect_stroke(
            rect.shrink(1.0),
            0.0,
            Stroke::new(1.0_f32, BLUE),
            StrokeKind::Inside,
        );
    }
    response.widget_info(|| match selected {
        Some(selected) => {
            WidgetInfo::selected(WidgetType::Button, response.enabled(), selected, label)
        }
        None => WidgetInfo::labeled(WidgetType::Button, response.enabled(), label),
    });
    keytips::set_badge_anchor(ui, &response, rect.left_center() + vec2(14.0, 0.0));
    response
}

pub(super) fn selectable(ui: &mut Ui, selected: bool, label: &str) -> Response {
    let response = ui.selectable_label(selected, label);
    named(ui, &response, label);
    response
}
