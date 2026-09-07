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
    Text,
    Size,
    Palette,
    Recent,
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
    Thumbnail,
    Colors,
    Transparent,
    FreeSelect(bool),
    InvertSelection,
    DeleteSelection,
    Angle,
    Quick(crate::preferences::QuickCommand),
    PageSetup,
    Import,
    Wallpaper,
    About,
    RecentFile(usize),
    ColorSlot(usize),
    PaletteColor(Color),
    StrokeSize(u32),
    TextControl(&'static str, bool),
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
            Self::Text => "Text tools",
            Self::Size => "Size",
            Self::Palette => "Colors",
            Self::Recent => "Recent pictures",
        }
    }

    fn entries(self, app: &PaintApp) -> Vec<Entry> {
        use Command::*;
        use Key::*;
        use RibbonKeys as R;

        match self {
            Self::Tabs => {
                let mut entries = vec![
                    (F, "File", Group(R::File)),
                    (H, "Home", Group(R::Home)),
                    (V, "View", Group(R::View)),
                ];
                if app.text_edit.is_some() {
                    entries.push((T, "Text tools", Group(R::Text)));
                }
                entries.extend(
                    app.quick_access
                        .commands
                        .iter()
                        .enumerate()
                        .map(|(index, command)| {
                            (index_key(index), command.name(), Quick(*command))
                        }),
                );
                entries
            }
            Self::File => vec![
                (N, "New", Action(super::Action::New)),
                (O, "Open…", Action(super::Action::Open)),
                (S, "Save", Action(super::Action::Save)),
                (A, "Save as…", Action(super::Action::SaveAs)),
                (P, "Print…", Action(super::Action::Print)),
                (
                    V,
                    "Print preview",
                    Quick(crate::preferences::QuickCommand::PrintPreview),
                ),
                (U, "Page setup…", PageSetup),
                (C, "From scanner or camera…", Import),
                (
                    M,
                    "Send in email…",
                    Quick(crate::preferences::QuickCommand::Email),
                ),
                (B, "Set as desktop background…", Wallpaper),
                (E, "Properties", Action(super::Action::Properties)),
                (I, "About Paint 10", About),
                (R, "Recent pictures", Group(R::Recent)),
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
                (G, "Paste from…", Action(super::Action::PasteFrom)),
                (Z, "Size", Group(R::Size)),
                (Num1, "Color 1", ColorSlot(0)),
                (Num2, "Color 2", ColorSlot(1)),
                (A, "Palette", Group(R::Palette)),
            ],
            Self::View => vec![
                (I, "Zoom in", Zoom(2.)),
                (O, "Zoom out", Zoom(0.5)),
                (Num1, "100%", Zoom(0.)),
                (R, "Rulers", Rulers),
                (G, "Gridlines", Grid),
                (S, "Status bar", Status),
                (F, "Full screen", Picture),
                (T, "Thumbnail", Thumbnail),
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
            Self::Text => vec![
                (F, "Font", TextControl("Font", false)),
                (S, "Font size", TextControl("Font size", false)),
                (B, "Bold", TextControl("Bold", true)),
                (I, "Italic", TextControl("Italic", true)),
                (U, "Underline", TextControl("Underline", true)),
                (K, "Strikeout", TextControl("Strikeout", true)),
                (O, "Opaque", TextControl("Opaque", true)),
                (T, "Transparent", TextControl("Transparent", true)),
                (Num1, "Color 1", ColorSlot(0)),
                (Num2, "Color 2", ColorSlot(1)),
                (C, "Palette", Group(R::Palette)),
                (E, "Edit colors…", Colors),
            ],
            Self::Size => [1, 3, 5, 8, 12, 20, 32, 50]
                .into_iter()
                .enumerate()
                .map(|(index, size)| {
                    (
                        index_key(index),
                        [
                            "1 pixel",
                            "3 pixels",
                            "5 pixels",
                            "8 pixels",
                            "12 pixels",
                            "20 pixels",
                            "32 pixels",
                            "50 pixels",
                        ][index],
                        StrokeSize(size),
                    )
                })
                .collect(),
            Self::Palette => super::ribbon::PALETTE
                .iter()
                .enumerate()
                .map(|(index, rgb)| {
                    (
                        index_key(index),
                        super::ribbon::PALETTE_NAMES[index],
                        PaletteColor([rgb[0], rgb[1], rgb[2], 255]),
                    )
                })
                .collect(),
            Self::Recent => app
                .recent
                .iter()
                .enumerate()
                .map(|(index, _)| (index_key(index), "Recent picture", RecentFile(index)))
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

        for (index, command) in self.quick_access.commands.clone().into_iter().enumerate() {
            if ctx.input_mut(|input| consume_shortcut(input, Modifiers::ALT, index_key(index))) {
                self.ribbon_keys = None;
                self.alt_used = true;
                ctx.input_mut(|input| {
                    input
                        .events
                        .retain(|event| !matches!(event, Event::Text(_)));
                });
                self.quick_action(command, ctx);
                return true;
            }
        }

        for (key, group) in [
            (Key::F, RibbonKeys::File),
            (Key::H, RibbonKeys::Home),
            (Key::V, RibbonKeys::View),
        ] {
            if ctx.input_mut(|i| consume_shortcut(i, Modifiers::ALT, key)) {
                self.ribbon_command(RibbonKeys::Tabs, Command::Group(group), ctx);
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
            ctx.data_mut(|data| data.insert_temp(Id::new("paint10_context_initial_focus"), true));
        }
        let backward = ctx.input_mut(|input| {
            consume_shortcut(input, Modifiers::CTRL | Modifiers::SHIFT, Key::Tab)
        });
        if backward || ctx.input_mut(|input| consume_shortcut(input, Modifiers::CTRL, Key::Tab)) {
            let count = if self.text_edit.is_some() { 3 } else { 2 };
            let current = if self.text_tab {
                2
            } else {
                usize::from(self.view_tab)
            };
            let next = (current + if backward { count - 1 } else { 1 }) % count;
            self.text_tab = next == 2;
            self.view_tab = next == 1;
            if self.collapsed {
                ctx.data_mut(|data| data.insert_temp(Id::new("paint10-ribbon-revealed"), true));
            }
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
        for (key, _, command) in group.entries(self) {
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
                if matches!(next, RibbonKeys::Home | RibbonKeys::View | RibbonKeys::Text) {
                    self.view_tab = next == RibbonKeys::View;
                    self.text_tab = next == RibbonKeys::Text && self.text_edit.is_some();
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
            Command::Thumbnail => {
                if self.zoom > 1.0 {
                    Self::set_thumbnail_enabled(ctx, !Self::thumbnail_enabled(ctx));
                }
            }
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
            Command::Angle => {
                self.angle = self
                    .object
                    .and_then(|index| self.doc.objects.get(index))
                    .map_or(0.0, |object| object.angle);
                self.dialog = Some(Dialog::Rotate);
            }
            Command::Quick(command) => self.quick_action(command, ctx),
            Command::PageSetup => {
                self.finish_editing();
                self.dialog = Some(Dialog::Print);
            }
            Command::Import => {
                if self.job.is_none() {
                    self.dialog = Some(Dialog::Import);
                    self.start_job(ctx, || {
                        JobResult::Devices(crate::integration::enumerate_devices())
                    });
                }
            }
            Command::Wallpaper => {
                self.finish_editing();
                if let Some(size) = ctx.input(|input| input.viewport().monitor_size) {
                    self.wallpaper_size = (size.x as u32, size.y as u32);
                }
                self.dialog = Some(Dialog::Wallpaper);
            }
            Command::About => self.dialog = Some(Dialog::About),
            Command::RecentFile(index) => {
                if let Some(path) = self.recent.get(index).cloned() {
                    self.pending_path = Some(path);
                    self.action(Action::Open, ctx);
                }
            }
            Command::ColorSlot(index) => {
                self.active_color = index;
                self.ribbon_keys = Some(RibbonKeys::Palette);
            }
            Command::PaletteColor(color) => self.colors[self.active_color] = color,
            Command::StrokeSize(size) => self.size = size,
            Command::TextControl(name, activate) => {
                if let Some(id) =
                    ctx.data(|data| data.get_temp::<Id>(Id::new(("paint10_text_control", name))))
                {
                    if let Some(state) = &mut self.text_edit {
                        state.focus = false;
                    }
                    ctx.memory_mut(|memory| memory.request_focus(id));
                    if activate {
                        ctx.input_mut(|input| {
                            input.events.push(Event::Key {
                                key: Key::Enter,
                                physical_key: None,
                                pressed: true,
                                repeat: false,
                                modifiers: Modifiers::NONE,
                            })
                        });
                    } else {
                        let mut state = TextEdit::load_state(ctx, id).unwrap_or_default();
                        state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::two(
                                egui::text::CCursor::new(0),
                                egui::text::CCursor::new(usize::MAX),
                            )));
                        state.store(ctx, id);
                    }
                    ctx.request_repaint();
                }
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
                    ScrollArea::vertical()
                        .max_height((ctx.screen_rect().height() - 145.0).max(100.0))
                        .show(ui, |ui| {
                            Grid::new("key_tip_commands").num_columns(2).show(ui, |ui| {
                                for (key, label, item) in group.entries(self) {
                                    ui.monospace(key.name());
                                    let label = if let Command::RecentFile(index) = item {
                                        self.recent[index]
                                            .file_name()
                                            .unwrap_or_default()
                                            .to_string_lossy()
                                            .into_owned()
                                    } else {
                                        label.to_owned()
                                    };
                                    if ui.button(label).clicked() {
                                        command = Some(item);
                                    }
                                    ui.end_row();
                                }
                            });
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
            Window::new(if self.text_edit.is_some() {
                "Text"
            } else {
                "Selection"
            })
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
        if self.text_edit.is_some() {
            let selected = self
                .text_edit
                .as_ref()
                .is_some_and(|state| !state.selection.is_empty());
            for (label, action, enabled) in [
                ("Cut", Action::Cut, selected),
                ("Copy", Action::Copy, selected),
                ("Paste", Action::Paste, true),
                ("Delete", Action::Clear, selected),
                ("Select all", Action::SelectAll, true),
            ] {
                let response = ui.add_enabled(enabled, Button::new(label));
                self.focus_keyboard_context(ui, &response);
                if response.clicked() {
                    if matches!(action, Action::Clear | Action::SelectAll) {
                        self.text_selection_action(action, ctx);
                    } else {
                        self.action(action, ctx);
                    }
                    self.keyboard_context_menu = false;
                    ui.close_menu();
                }
            }
            return;
        }
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
            let response = ui.add_enabled(enabled, Button::new(label));
            self.focus_keyboard_context(ui, &response);
            if response.clicked() {
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

    fn focus_keyboard_context(&self, ui: &Ui, response: &Response) {
        if self.keyboard_context_menu && response.enabled() && !ui.is_sizing_pass() {
            let initial = ui.ctx().data_mut(|data| {
                std::mem::take(
                    data.get_temp_mut_or_default::<bool>(Id::new("paint10_context_initial_focus")),
                )
            });
            if initial {
                response.request_focus();
                ui.ctx().request_repaint();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_alt_number_uses_custom_toolbar_order_without_opening_keytips() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.quick_access.commands = vec![
            crate::preferences::QuickCommand::Save,
            crate::preferences::QuickCommand::Undo,
        ];
        app.doc.begin();
        app.doc.image.put_pixel(10, 10, Rgba(BLACK));
        app.doc.commit();
        let _ = ctx.run(
            RawInput {
                events: vec![
                    Event::Key {
                        key: Key::Num2,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers::ALT,
                    },
                    Event::Text("2".into()),
                ],
                ..Default::default()
            },
            |ctx| {
                assert!(app.ribbon_keyboard(ctx));
                assert!(!ctx.input(|input| input
                    .events
                    .iter()
                    .any(|event| matches!(event, Event::Text(_)))));
            },
        );
        assert_eq!(*app.doc.image.get_pixel(10, 10), Rgba(WHITE));
        assert!(app.ribbon_keys.is_none());
    }
}
