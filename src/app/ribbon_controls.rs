//! Shared registration for the actual ribbon widgets, including those reused
//! inside collapsed groups. Scope follows the synchronous UI construction stack.

use super::*;

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
        "Full screen     F11" => "F",
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

pub(super) fn selectable(ui: &mut Ui, selected: bool, label: &str) -> Response {
    let response = ui.selectable_label(selected, label);
    named(ui, &response, label);
    response
}

pub(super) fn checkbox(ui: &mut Ui, value: &mut bool, label: &str) -> Response {
    let response = ui.checkbox(value, label);
    named(ui, &response, label);
    response
}
