use super::shortcuts::consume_shortcut;
use super::*;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum RibbonKeys {
    Tabs,
    File,
    Home,
    View,
    Brushes,
    Shapes,
    Outline,
    Fill,
    Select,
    Rotate,
}

#[derive(Clone, Copy)]
enum Command {
    Group(RibbonKeys),
    Action(Action),
    Tool(Tool),
    Brush(Brush),
    Style(PaintStyle),
    Zoom(f32),
    Grid,
    Rulers,
    Status,
    Picture,
    Colors,
    Transparent,
    FreeSelect(bool),
    InvertSelection,
    DeleteSelection,
    Angle,
    TextTab,
}

type Entry = (Key, &'static str, Command);

impl RibbonKeys {
    fn title(self) -> &'static str {
        match self {
            Self::Tabs => "Ribbon",
            Self::File => "File",
            Self::Home => "Home",
            Self::View => "View",
            Self::Brushes => "Brushes",
            Self::Shapes => "Shapes",
            Self::Outline => "Outline",
            Self::Fill => "Fill",
            Self::Select => "Select",
            Self::Rotate => "Rotate",
        }
    }

    fn entries(self) -> Vec<Entry> {
        use Command::*;
        use Key::*;
        use RibbonKeys as R;

        match self {
            Self::Tabs => vec![
                (F, "File", Group(R::File)),
                (H, "Home", Group(R::Home)),
                (V, "View", Group(R::View)),
                (T, "Text tools", TextTab),
                (Num1, "Save", Action(super::Action::Save)),
                (Num2, "Undo", Action(super::Action::Undo)),
                (Num3, "Redo", Action(super::Action::Redo)),
            ],
            Self::File => vec![
                (N, "New", Action(super::Action::New)),
                (O, "Open…", Action(super::Action::Open)),
                (S, "Save", Action(super::Action::Save)),
                (A, "Save as…", Action(super::Action::SaveAs)),
                (P, "Print…", Action(super::Action::Print)),
                (E, "Properties", Action(super::Action::Properties)),
                (X, "Exit", Action(super::Action::Close)),
            ],
            Self::Home => vec![
                (V, "Paste", Action(super::Action::Paste)),
                (X, "Cut", Action(super::Action::Cut)),
                (C, "Copy", Action(super::Action::Copy)),
                (S, "Select", Group(R::Select)),
                (R, "Crop", Action(super::Action::Crop)),
                (W, "Resize and skew…", Action(super::Action::Resize)),
                (O, "Rotate", Group(R::Rotate)),
                (P, "Pencil", Tool(super::Tool::Pencil)),
                (F, "Fill with color", Tool(super::Tool::Fill)),
                (T, "Text", Tool(super::Tool::Text)),
                (E, "Eraser", Tool(super::Tool::Eraser)),
                (K, "Color picker", Tool(super::Tool::Picker)),
                (M, "Magnifier", Tool(super::Tool::Magnifier)),
                (B, "Brushes", Group(R::Brushes)),
                (H, "Shapes", Group(R::Shapes)),
                (L, "Outline", Group(R::Outline)),
                (I, "Shape fill", Group(R::Fill)),
                (D, "Edit colors…", Colors),
            ],
            Self::View => vec![
                (I, "Zoom in", Zoom(2.)),
                (O, "Zoom out", Zoom(0.5)),
                (Num1, "100%", Zoom(0.)),
                (R, "Rulers", Rulers),
                (G, "Gridlines", Grid),
                (S, "Status bar", Status),
                (F, "Full screen", Picture),
            ],
            Self::Select => vec![
                (R, "Rectangular selection", FreeSelect(false)),
                (F, "Free-form selection", FreeSelect(true)),
                (A, "Select all", Action(super::Action::SelectAll)),
                (I, "Invert selection", InvertSelection),
                (D, "Delete selection", DeleteSelection),
                (T, "Transparent selection", Transparent),
            ],
            Self::Rotate => vec![
                (R, "Rotate right 90°", Action(super::Action::Rotate(90.))),
                (L, "Rotate left 90°", Action(super::Action::Rotate(270.))),
                (Num2, "Rotate 180°", Action(super::Action::Rotate(180.))),
                (H, "Flip horizontal", Action(super::Action::Flip(true))),
                (V, "Flip vertical", Action(super::Action::Flip(false))),
                (A, "Custom angle…", Angle),
            ],
            Self::Brushes => super::Brush::ALL
                .iter()
                .enumerate()
                .map(|(i, brush)| (index_key(i), brush.name(), Brush(*brush)))
                .collect(),
            Self::Shapes => super::Tool::SHAPES
                .iter()
                .enumerate()
                .map(|(i, tool)| (index_key(i), tool.name(), Tool(*tool)))
                .collect(),
            Self::Outline | Self::Fill => PaintStyle::ALL
                .iter()
                .enumerate()
                .map(|(i, style)| (index_key(i), style.name(), Style(*style)))
                .collect(),
        }
    }
}

fn index_key(index: usize) -> Key {
    use Key::*;
    [
        Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9, Num0, A, B, C, D, E, F, G, H, I, J,
        K, L, M,
    ][index]
}

impl PaintApp {
    pub(in crate::app) fn show_picture(&mut self, ctx: &Context) {
        self.commit_text();
        self.finish_polygon();
        self.commit_shape();
        if self.curve.take().is_some() {
            self.doc.commit();
        }
        self.preview = true;
        self.fullscreen = true;
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(true));
    }

    pub(in crate::app) fn hide_picture(&mut self, ctx: &Context) {
        self.preview = false;
        self.fullscreen = false;
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
    }

    pub(in crate::app) fn ribbon_keyboard(&mut self, ctx: &Context) -> bool {
        if self.dialog.is_some() || self.pending.is_some() {
            self.ribbon_keys = None;
            self.keyboard_context_menu = false;
            return false;
        }
        let (alt, other_key) = ctx.input(|i| {
            (
                i.modifiers.alt,
                i.events
                    .iter()
                    .any(|event| matches!(event, Event::Key { pressed: true, .. })),
            )
        });
        if alt && !self.alt_was_down {
            self.alt_used = false;
        }
        self.alt_used |= alt && other_key;
        let released_alt = self.alt_was_down && !alt && !self.alt_used;
        self.alt_was_down = alt;

        for (key, group) in [
            (Key::F, RibbonKeys::File),
            (Key::H, RibbonKeys::Home),
            (Key::V, RibbonKeys::View),
        ] {
            if ctx.input_mut(|i| consume_shortcut(i, Modifiers::ALT, key)) {
                self.ribbon_keys = Some(group);
                ctx.input_mut(|input| {
                    input
                        .events
                        .retain(|event| !matches!(event, Event::Text(_)))
                });
            }
        }

        if released_alt || ctx.input_mut(|i| consume_shortcut(i, Modifiers::NONE, Key::F10)) {
            self.ribbon_keys = if self.ribbon_keys.is_some() {
                None
            } else {
                Some(RibbonKeys::Tabs)
            };
        }
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::SHIFT, Key::F10)) {
            self.keyboard_context_menu = true;
            self.ribbon_keys = None;
        }
        if ctx.input_mut(|i| {
            consume_shortcut(i, Modifiers::CTRL, Key::Tab)
                || consume_shortcut(i, Modifiers::CTRL | Modifiers::SHIFT, Key::Tab)
        }) {
            self.text_tab = false;
            self.view_tab = !self.view_tab;
            self.collapsed = false;
        }
        if ctx.input_mut(|i| {
            consume_shortcut(i, Modifiers::NONE, Key::F6)
                || consume_shortcut(i, Modifiers::SHIFT, Key::F6)
        }) {
            if ctx.memory(|memory| memory.has_focus(Id::new("canvas"))) {
                self.ribbon_keys = Some(RibbonKeys::Tabs);
            } else {
                ctx.memory_mut(|memory| memory.request_focus(Id::new("canvas")));
                self.ribbon_keys = None;
            }
        }
        let Some(group) = self.ribbon_keys else {
            return self.keyboard_context_menu;
        };
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::NONE, Key::Escape)) {
            self.ribbon_keys = (group != RibbonKeys::Tabs).then_some(RibbonKeys::Tabs);
            return true;
        }
        for (key, _, command) in group.entries() {
            if ctx.input_mut(|i| {
                consume_shortcut(i, Modifiers::NONE, key)
                    || consume_shortcut(i, Modifiers::ALT, key)
            }) {
                // Key tips are commands, so their text events must not reach an
                // inline text box that remains active while using the ribbon.
                ctx.input_mut(|input| {
                    input
                        .events
                        .retain(|event| !matches!(event, Event::Text(_)))
                });
                self.ribbon_command(group, command, ctx);
                break;
            }
        }
        true
    }

    fn ribbon_command(&mut self, group: RibbonKeys, command: Command, ctx: &Context) {
        self.ribbon_keys = None;
        match command {
            Command::Group(next) => {
                self.ribbon_keys = Some(next);
                if matches!(next, RibbonKeys::Home | RibbonKeys::View) {
                    self.view_tab = next == RibbonKeys::View;
                    self.text_tab = false;
                    self.collapsed = false;
                }
            }
            Command::Action(action) => self.action(action, ctx),
            Command::Tool(tool) => self.set_tool(tool),
            Command::Brush(brush) => {
                self.brush = brush;
                self.set_tool(Tool::Brush);
            }
            Command::Style(style) => {
                if group == RibbonKeys::Outline {
                    self.outline = style;
                } else {
                    self.fill = style;
                }
            }
            Command::Zoom(factor) => {
                self.zoom = if factor == 0. {
                    1.
                } else {
                    (self.zoom * factor).clamp(0.125, 8.)
                }
            }
            Command::Grid => self.grid = !self.grid,
            Command::Rulers => self.rulers = !self.rulers,
            Command::Status => self.status_bar = !self.status_bar,
            Command::Picture => self.show_picture(ctx),
            Command::Colors => {
                let color = self.colors[self.active_color];
                self.hex = format!("{:02X}{:02X}{:02X}", color[0], color[1], color[2]);
                self.dialog = Some(Dialog::Colors);
            }
            Command::Transparent => self.transparent = !self.transparent,
            Command::FreeSelect(free) => {
                self.free_select = free;
                self.set_tool(Tool::Select);
            }
            Command::InvertSelection => self.invert_selection(),
            Command::DeleteSelection => self.delete_selection(),
            Command::Angle => self.dialog = Some(Dialog::Rotate),
            Command::TextTab => {
                self.text_tab = self.text_edit.is_some();
                self.collapsed = false;
            }
        }
    }

    pub(in crate::app) fn keyboard_menu(&mut self, ctx: &Context) {
        if let Some(group) = self.ribbon_keys {
            let mut command = None;
            let mut open = true;
            Window::new(format!("{} · Key tips", group.title()))
                .id(Id::new("ribbon_key_tips"))
                .order(Order::Foreground)
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .fixed_pos(pos2(8., 58.))
                .show(ctx, |ui| {
                    ui.label("Press a letter, or choose a command. Esc goes back.");
                    Grid::new("key_tip_commands").num_columns(2).show(ui, |ui| {
                        for (key, label, item) in group.entries() {
                            ui.monospace(key.name());
                            if ui.button(label).clicked() {
                                command = Some(item);
                            }
                            ui.end_row();
                        }
                    });
                });
            if !open {
                self.ribbon_keys = None;
            }
            if let Some(command) = command {
                self.ribbon_command(group, command, ctx);
            }
        }
        if self.keyboard_context_menu {
            let mut open = true;
            Window::new("Selection")
                .id(Id::new("keyboard_context_menu"))
                .order(Order::Foreground)
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_pos(self.canvas_rect.min + vec2(20., 20.))
                .show(ctx, |ui| self.selection_menu(ui, ctx));
            if !open || ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.keyboard_context_menu = false;
            }
        }
    }

    pub(in crate::app) fn selection_menu(&mut self, ui: &mut Ui, ctx: &Context) {
        let selected = self.selected_region().is_some();
        for (label, action, enabled) in [
            ("Cut", Action::Cut, selected),
            ("Copy", Action::Copy, selected),
            ("Paste", Action::Paste, true),
            ("Crop", Action::Crop, selected),
            ("Delete", Action::Clear, selected),
            ("Select all", Action::SelectAll, true),
            ("Invert colors", Action::Invert, true),
            ("Resize and skew…", Action::Resize, true),
        ] {
            if ui.add_enabled(enabled, Button::new(label)).clicked() {
                self.action(action, ctx);
                self.keyboard_context_menu = false;
                ui.close_menu();
            }
        }
        if selected && ui.button("Invert selection").clicked() {
            self.invert_selection();
            self.keyboard_context_menu = false;
            ui.close_menu();
        }
    }
}
