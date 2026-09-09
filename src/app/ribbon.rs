use super::ribbon_controls::{self as controls, Scope};
use super::ribbon_layout::Group;
use super::*;

#[cfg(test)]
mod gallery_scroll_tests;

pub(in crate::app) const PALETTE: [[u8; 3]; 20] = [
    [0, 0, 0],
    [127, 127, 127],
    [136, 0, 21],
    [237, 28, 36],
    [255, 127, 39],
    [255, 242, 0],
    [34, 177, 76],
    [0, 162, 232],
    [63, 72, 204],
    [163, 73, 164],
    [255, 255, 255],
    [195, 195, 195],
    [185, 122, 87],
    [255, 174, 201],
    [255, 201, 14],
    [239, 228, 176],
    [181, 230, 29],
    [153, 217, 234],
    [112, 146, 190],
    [200, 191, 231],
];

pub(in crate::app) const PALETTE_NAMES: [&str; 20] = [
    "Black",
    "Gray",
    "Dark red",
    "Red",
    "Orange",
    "Yellow",
    "Green",
    "Turquoise",
    "Indigo",
    "Purple",
    "White",
    "Light gray",
    "Brown",
    "Rose",
    "Gold",
    "Light yellow",
    "Lime",
    "Light turquoise",
    "Blue gray",
    "Lavender",
];

fn ribbon_focus(ui: &Ui, response: &Response) {
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.shrink(1.0),
            0.0,
            Stroke::new(2.0_f32, Color32::from_rgb(0, 80, 160)),
            StrokeKind::Inside,
        );
    }
}

fn paint_style_choice(
    ui: &mut Ui,
    selected: bool,
    style: PaintStyle,
    label: &str,
    color: Color,
    fill: bool,
) -> Response {
    let response = ui.add(theme::MenuItem::new(label).selected(selected).width(250.0));
    controls::named(ui, &response, label);
    let sample = Rect::from_center_size(
        response.rect.right_center() - vec2(42.0, 0.0),
        vec2(62.0, 20.0),
    );
    if style != PaintStyle::None {
        // The swatch uses the same rasterizer and color as the picture. Cache
        // one texture per choice, replacing it when the palette changes.
        let id = Id::new(("paint10-style-sample", style as u8, fill));
        let cached = ui
            .ctx()
            .data(|data| data.get_temp::<(Color, TextureHandle)>(id));
        let texture = if let Some((_, texture)) = cached.filter(|(saved, _)| *saved == color) {
            texture
        } else {
            let mut pixels = RgbaImage::new(62, 20);
            if fill {
                d::styled_shape(
                    &mut pixels,
                    Tool::Rectangle,
                    (0, 0),
                    (61, 20),
                    1,
                    None,
                    Some((color, style).into()),
                );
            } else {
                d::styled_cubic(
                    &mut pixels,
                    (2, 12),
                    (59, 9),
                    [(20, 1), (40, 19)],
                    5,
                    color,
                    style,
                );
            }
            let texture = ui.ctx().load_texture(
                "Paint style sample",
                display::image([62, 20], pixels.as_raw()),
                TextureOptions::NEAREST,
            );
            ui.ctx()
                .data_mut(|data| data.insert_temp(id, (color, texture.clone())));
            texture
        };
        canvas::checkerboard(ui.painter(), sample, 5.0);
        ui.painter().image(
            texture.id(),
            sample,
            Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        ui.painter().rect_stroke(
            sample.shrink2(vec2(13.0, 3.0)),
            0.0,
            Stroke::new(1.0_f32, Color32::from_gray(180)),
            StrokeKind::Inside,
        );
        ui.painter().line_segment(
            [
                sample.center() + vec2(-11.0, 5.0),
                sample.center() + vec2(11.0, -5.0),
            ],
            Stroke::new(1.0_f32, Color32::from_gray(145)),
        );
    }
    response
}

fn gradient_preview(painter: &Painter, rect: Rect, gradient: Gradient, colors: [Color; 2]) {
    for row in 0..4 {
        for column in 0..12 {
            let cell = Rect::from_min_size(
                rect.min
                    + vec2(
                        column as f32 * rect.width() / 12.0,
                        row as f32 * rect.height() / 4.0,
                    ),
                vec2(rect.width() / 12.0, rect.height() / 4.0),
            );
            painter.rect_filled(
                cell,
                0.0,
                Color32::from_gray(if (row + column) % 2 == 0 { 255 } else { 220 }),
            );
        }
    }
    let colors = colors.map(display::color);
    let mut mesh = egui::Mesh::default();
    match gradient {
        Gradient::Vertical | Gradient::Horizontal => {
            for (point, color) in [
                (rect.left_top(), colors[0]),
                (
                    rect.right_top(),
                    colors[usize::from(gradient == Gradient::Horizontal)],
                ),
                (rect.right_bottom(), colors[1]),
                (
                    rect.left_bottom(),
                    colors[usize::from(gradient == Gradient::Vertical)],
                ),
            ] {
                mesh.colored_vertex(point, color);
            }
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
        }
        Gradient::Radial => {
            painter.rect_filled(rect, 0.0, colors[1]);
            mesh.colored_vertex(rect.center(), colors[0]);
            for index in 0..=32 {
                let angle = index as f32 * std::f32::consts::TAU / 32.0;
                mesh.colored_vertex(
                    rect.center()
                        + vec2(angle.cos() * rect.width(), angle.sin() * rect.height()) / 2.0,
                    colors[1],
                );
                if index > 0 {
                    mesh.add_triangle(0, index, index + 1);
                }
            }
        }
    }
    painter.add(mesh);
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0_f32, Color32::from_gray(190)),
        StrokeKind::Inside,
    );
}

pub(super) fn ribbon_menu_button<R>(
    ui: &mut Ui,
    label: &str,
    keys: &str,
    popup_scope: &'static str,
    icon: Option<Icon>,
    contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<Option<R>> {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::proportional(12.0),
        Color32::from_gray(35),
    );
    let icon_width = if icon.is_some() { 20.0 } else { 0.0 };
    let size = vec2((galley.size().x + icon_width + 24.0).max(22.0), 22.0);
    let group = controls::current(ui).map_or("Menu", |scope| scope.group);
    let menu = egui::menu::menu_custom_button(ui, Button::new("").min_size(size), |ui| {
        theme::menu(ui);
        controls::scope(ui, Scope::new(popup_scope, group), contents)
    });
    controls::register(
        ui,
        &menu.response,
        keys,
        keytips::Kind::Menu { scope: popup_scope },
    );
    let rect = menu.response.rect;
    if let Some(icon) = icon {
        icons::draw(
            ui.painter(),
            Rect::from_center_size(rect.left_center() + vec2(12.0, 0.0), vec2(16.0, 16.0)),
            icon,
        );
    }
    ui.painter().galley(
        pos2(
            rect.left() + 5.0 + icon_width,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        Color32::from_gray(35),
    );
    icons::draw(
        ui.painter(),
        Rect::from_center_size(
            if label.is_empty() && icon.is_none() {
                rect.center()
            } else {
                rect.right_center() - vec2(9.0, 0.0)
            },
            vec2(12.0, 12.0),
        ),
        Icon::ChevronDown,
    );
    menu.response
        .widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), label));
    menu
}

impl PaintApp {
    pub(in crate::app) fn ribbon(&mut self, ctx: &Context) {
        self.text_tab &= self.text_edit.is_some();
        let reveal_id = Id::new("paint10-ribbon-revealed");
        let mut revealed = ctx
            .data(|data| data.get_temp::<bool>(reveal_id))
            .unwrap_or(false);
        if !self.collapsed {
            revealed = false;
        }
        let mut tab_activated = false;
        let tabs = TopBottomPanel::top("tabs")
            .exact_height(27.)
            .frame(Frame::NONE.fill(Color32::WHITE))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.scope(|ui| {
                        ui.visuals_mut().widgets.inactive.bg_fill = BLUE;
                        ui.visuals_mut().widgets.inactive.weak_bg_fill = BLUE;
                        ui.visuals_mut().override_text_color = Some(Color32::WHITE);
                        let file =
                            egui::menu::menu_custom_button(ui, Button::new("  File  "), |ui| {
                                self.file_menu(ui, ctx);
                            });
                        keytips::register(
                            ui,
                            &file.response,
                            "tabs",
                            "Tabs",
                            "F",
                            keytips::Kind::Menu { scope: "file" },
                        );
                        if file.response.clicked() {
                            ribbon_layout::close_groups(ctx);
                        }
                    });
                    for (index, label) in [(0, "   Home   "), (1, "   View   "), (2, "   Text   ")]
                    {
                        if index == 2 && self.text_edit.is_none() {
                            continue;
                        }
                        let selected = match index {
                            0 => !self.view_tab && !self.text_tab,
                            1 => self.view_tab && !self.text_tab,
                            _ => self.text_tab,
                        };
                        let response = controls::selectable(ui, selected, label);
                        let (keys, scope) = match index {
                            0 => ("H", "home"),
                            1 => ("V", "view"),
                            _ => ("T", "text"),
                        };
                        keytips::register(
                            ui,
                            &response,
                            "tabs",
                            "Tabs",
                            keys,
                            keytips::Kind::Tab { scope },
                        );
                        response.widget_info(|| {
                            WidgetInfo::selected(
                                WidgetType::SelectableLabel,
                                true,
                                selected,
                                label.trim(),
                            )
                        });
                        if response.clicked() {
                            ribbon_layout::close_groups(ctx);
                            self.view_tab = index == 1;
                            self.text_tab = index == 2;
                            revealed = self.collapsed;
                            tab_activated = true;
                        }
                        if response.double_clicked() {
                            self.collapsed = !self.collapsed;
                            revealed = false;
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let help = ui
                            .button(RichText::new("?").color(BLUE))
                            .on_hover_text("Help (F1)");
                        help.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "Help"));
                        if help.clicked() {
                            self.dialog = Some(Dialog::About);
                        }
                        let collapse_label = if self.collapsed {
                            "Expand the ribbon"
                        } else {
                            "Minimize the ribbon"
                        };
                        let collapse = ui
                            .add_sized(vec2(20.0, 18.0), Button::new(""))
                            .on_hover_text(format!("{collapse_label} (Ctrl+F1)"));
                        icons::draw(
                            ui.painter(),
                            collapse.rect.shrink(3.0),
                            if self.collapsed {
                                Icon::ChevronDown
                            } else {
                                Icon::ChevronUp
                            },
                        );
                        collapse.widget_info(|| {
                            WidgetInfo::labeled(WidgetType::Button, true, collapse_label)
                        });
                        if collapse.clicked() {
                            self.collapsed = !self.collapsed;
                            revealed = false;
                        }
                    });
                });
            });
        if self.collapsed && !revealed {
            ctx.data_mut(|data| data.insert_temp(reveal_id, false));
            return;
        }
        let frame = Frame::NONE
            .fill(RIBBON)
            .stroke(Stroke::new(1.0_f32, Color32::from_gray(218)));
        let ribbon_rect = if self.collapsed {
            Area::new(Id::new("temporary_ribbon"))
                .order(Order::Foreground)
                .fixed_pos(tabs.response.rect.left_bottom())
                .movable(false)
                .show(ctx, |ui| {
                    ui.set_width(tabs.response.rect.width() - 2.0);
                    frame.show(ui, |ui| self.ribbon_contents(ui, ctx));
                })
                .response
                .rect
        } else {
            TopBottomPanel::top("ribbon")
                .exact_height(116.0)
                .frame(frame)
                .show(ctx, |ui| self.ribbon_contents(ui, ctx))
                .response
                .rect
        };
        if self.collapsed && !tab_activated && !keytips::popup_open(ctx) {
            let outside = ctx.input(|input| {
                input.pointer.any_pressed()
                    && input
                        .pointer
                        .interact_pos()
                        .is_some_and(|point| !ribbon_rect.union(tabs.response.rect).contains(point))
            });
            if outside || ctx.input(|input| input.key_pressed(Key::Escape)) {
                revealed = false;
            }
        }
        if self.dialog.is_some() || self.pending.is_some() {
            revealed = false;
        }
        ctx.data_mut(|data| data.insert_temp(reveal_id, revealed));
    }

    fn ribbon_contents(&mut self, ui: &mut Ui, ctx: &Context) {
        let origin = ui.cursor().min;
        ui.allocate_space(vec2(ui.available_width(), 112.0));
        if self.text_tab && self.text_edit.is_some() {
            self.text_ribbon(ui, origin, ctx);
        } else if self.view_tab {
            self.view_ribbon(ui, origin, ctx);
        } else {
            self.home_ribbon(ui, origin, ctx);
        }
    }

    pub(in crate::app) fn group(ui: &Ui, origin: Pos2, x: f32, w: f32, label: &str) {
        ui.painter().line_segment(
            [origin + vec2(x + w, 5.), origin + vec2(x + w, 106.)],
            Stroke::new(1.0_f32, Color32::from_gray(220)),
        );
        ui.painter().text(
            origin + vec2(x + w / 2., 102.),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(11.),
            Color32::from_gray(112),
        );
    }

    pub(in crate::app) fn small_action(
        &mut self,
        ui: &mut Ui,
        position: Pos2,
        icon: Icon,
        label: &str,
        act: Action,
        enabled: bool,
    ) {
        let r = Rect::from_min_size(position, vec2(76., 25.));
        let response = ui.interact(
            r,
            ui.id().with(label),
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        if response.hovered() && enabled {
            ui.painter()
                .rect_filled(r, 0., Color32::from_rgb(224, 240, 253));
        }
        let mut painter = ui.painter().clone();
        if !enabled {
            painter.set_opacity(0.35);
        }
        icons::draw(
            &painter,
            Rect::from_min_size(r.min + vec2(2., 4.), vec2(17., 17.)),
            icon,
        );
        painter.text(
            r.min + vec2(24., 13.),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(12.),
            Color32::from_gray(30),
        );
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, label));
        controls::named(ui, &response, label);
        ribbon_focus(ui, &response);
        if response.clicked() {
            self.action(act, ui.ctx());
            ui.close_menu();
        }
    }

    pub(in crate::app) fn home_ribbon(&mut self, ui: &mut Ui, o: Pos2, ctx: &Context) {
        let groups = [
            Group {
                label: "Clipboard",
                width: 120.0,
                icon: Icon::Paste,
                keys: "ZC",
                popup: "home_clipboard",
            },
            Group {
                label: "Image",
                width: 161.0,
                icon: Icon::Resize,
                keys: "ZI",
                popup: "home_image",
            },
            Group {
                label: "Tools",
                width: 92.0,
                icon: Icon::Tool(Tool::Pencil),
                keys: "ZT",
                popup: "home_tools",
            },
            Group {
                label: "Brushes",
                width: 71.0,
                icon: Icon::Brush(self.brush),
                keys: "ZB",
                popup: "home_brushes",
            },
            Group {
                label: "Shapes",
                width: 252.0,
                icon: Icon::Tool(Tool::Rectangle),
                keys: "ZH",
                popup: "home_shapes",
            },
            Group {
                label: "Size",
                width: 65.0,
                icon: Icon::Outline,
                keys: "ZZ",
                popup: "home_size",
            },
            Group {
                label: "Colors",
                width: 347.0,
                icon: Icon::Colors,
                keys: "ZK",
                popup: "home_colors",
            },
        ];
        let widths =
            ribbon_layout::widths(&groups, ui.max_rect().right() - o.x, &[6, 4, 1, 0, 2, 3]);
        let mut x = o.x;
        for (index, (group, width)) in groups.into_iter().zip(widths).enumerate() {
            ribbon_layout::show(ui, pos2(x, o.y), width, "home", group, |ui, origin| {
                let legacy_x = [0.0, 120.0, 281.0, 373.0, 444.0, 696.0, 761.0][index];
                let origin = origin - vec2(legacy_x, 0.0);
                match index {
                    0 => self.clipboard_group(ui, origin, ctx),
                    1 => self.image_group(ui, origin, ctx),
                    2 => self.tools_group(ui, origin),
                    3 => self.brushes_group(ui, origin),
                    4 => self.shapes_group(ui, origin),
                    5 => self.size_group(ui, origin),
                    _ => self.colors_group(ui, origin),
                }
            });
            x += width;
        }
    }

    pub(in crate::app) fn clipboard_group(&mut self, ui: &mut Ui, o: Pos2, ctx: &Context) {
        let has_selection = self.text_edit.as_ref().map_or_else(
            || self.selected_region().is_some(),
            |state| !state.selection.is_empty(),
        );
        Self::group(ui, o, 0., 119., "Clipboard");
        let paste = controls::split_button(
            ui,
            controls::SplitButton {
                id: "paste",
                rect: Rect::from_min_size(o + vec2(3.0, 4.0), vec2(46.0, 83.0)),
                icon: Icon::Paste,
                label: "Paste",
                selected: false,
                menu: controls::SplitMenu {
                    label: "Paste options",
                    keys: "ZV",
                    scope: "paste",
                },
            },
            |ui| {
                if controls::command(ui, "Paste from…").clicked() {
                    ui.close_menu();
                    self.action(Action::PasteFrom, ctx);
                }
                #[cfg(target_arch = "wasm32")]
                if ui
                    .add_enabled(self.copied.is_some(), Button::new("Paste copied selection"))
                    .on_hover_text("Paste the last image copied within this Paint 10 tab, without reading the system clipboard.")
                    .clicked()
                {
                    if let Some(image) = self.copied.clone() {
                        self.insert_image(image);
                    }
                    ui.close_menu();
                }
            },
        );
        if paste.on_hover_text("Paste (Ctrl+V)").clicked() {
            self.action(Action::Paste, ctx);
        }
        self.small_action(
            ui,
            o + vec2(49., 7.),
            Icon::Cut,
            "Cut",
            Action::Cut,
            has_selection,
        );
        self.small_action(
            ui,
            o + vec2(49., 34.),
            Icon::Copy,
            "Copy",
            Action::Copy,
            has_selection,
        );
    }

    fn image_group(&mut self, ui: &mut Ui, o: Pos2, ctx: &Context) {
        let has_selection = self.selected_region().is_some();
        Self::group(ui, o, 120., 160., "Image");
        let select = controls::split_button(
            ui,
            controls::SplitButton {
                id: "select",
                rect: Rect::from_min_size(o + vec2(125.0, 4.0), vec2(55.0, 83.0)),
                icon: Icon::Tool(Tool::Select),
                label: "Select",
                selected: self.tool == Tool::Select,
                menu: controls::SplitMenu {
                    label: "Selection options",
                    keys: "ZS",
                    scope: "select",
                },
            },
            |ui| {
                theme::menu_heading(ui, "Selection shapes", 245.0);
                for (free, label, key) in [
                    (false, "Rectangular selection", "1"),
                    (true, "Free-form selection", "2"),
                ] {
                    let choice = ui.add(
                        theme::MenuItem::new(label)
                            .selected(self.free_select == free)
                            .width(245.0),
                    );
                    controls::register(ui, &choice, key, keytips::Kind::Button);
                    if choice.clicked() {
                        self.free_select = free;
                        self.set_tool(Tool::Select);
                        ui.close_menu();
                    }
                }
                theme::menu_heading(ui, "Selection options", 245.0);
                let all = ui.add(
                    theme::MenuItem::new("Select all")
                        .shortcut("Ctrl+A")
                        .width(245.0),
                );
                controls::register(ui, &all, "3", keytips::Kind::Button);
                if all.clicked() {
                    self.action(Action::SelectAll, ctx);
                    ui.close_menu();
                }
                let invert = ui.add_enabled(
                    has_selection,
                    theme::MenuItem::new("Invert selection").width(245.0),
                );
                controls::register(ui, &invert, "4", keytips::Kind::Button);
                if invert.clicked() {
                    self.invert_selection();
                    ui.close_menu();
                }
                let delete = ui.add_enabled(
                    has_selection,
                    theme::MenuItem::new("Delete").shortcut("Del").width(245.0),
                );
                controls::register(ui, &delete, "5", keytips::Kind::Button);
                if delete.clicked() {
                    self.delete_selection();
                    ui.close_menu();
                }
                let transparent = ui.add(
                    theme::MenuItem::new("Transparent selection")
                        .selected(self.transparent)
                        .width(245.0),
                );
                controls::register(ui, &transparent, "6", keytips::Kind::Button);
                if transparent.clicked() {
                    self.transparent = !self.transparent;
                    ui.close_menu();
                }
            },
        );
        if select.on_hover_text("Select part of the picture").clicked() {
            self.set_tool(Tool::Select);
        }
        self.small_action(
            ui,
            o + vec2(187., 5.),
            Icon::Crop,
            "Crop",
            Action::Crop,
            has_selection,
        );
        self.small_action(
            ui,
            o + vec2(187., 32.),
            Icon::Resize,
            "Resize",
            Action::Resize,
            true,
        );
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(o + vec2(187., 61.), vec2(85., 25.))),
            |ui| {
                let menu =
                    ribbon_menu_button(ui, "Rotate", "RO", "rotate", Some(Icon::Rotate), |ui| {
                        for (label, act) in [
                            ("Rotate right 90°", Action::Rotate(90.)),
                            ("Rotate left 90°", Action::Rotate(270.)),
                            ("Rotate 180°", Action::Rotate(180.)),
                            ("Flip vertical", Action::Flip(false)),
                            ("Flip horizontal", Action::Flip(true)),
                        ] {
                            if controls::command(ui, label).clicked() {
                                self.action(act, ctx);
                                ui.close_menu();
                            }
                        }
                        ui.separator();
                        if controls::command(ui, "Custom angle…").clicked() {
                            self.angle =
                                self.object.map(|i| self.doc.objects[i].angle).unwrap_or(0.);
                            self.dialog = Some(Dialog::Rotate);
                            ui.close_menu();
                        }
                    });
                menu.response.widget_info(|| {
                    WidgetInfo::labeled(WidgetType::Button, true, "Rotate and flip")
                });
                ribbon_focus(ui, &menu.response);
            },
        );
    }

    fn tools_group(&mut self, ui: &mut Ui, o: Pos2) {
        Self::group(ui, o, 281., 91., "Tools");
        for (i, tool) in [
            Tool::Pencil,
            Tool::Fill,
            Tool::Text,
            Tool::Eraser,
            Tool::Picker,
            Tool::Magnifier,
        ]
        .into_iter()
        .enumerate()
        {
            let r = Rect::from_min_size(
                o + vec2(286. + (i % 3) as f32 * 27., 9. + (i / 3) as f32 * 32.),
                vec2(26., 29.),
            );
            if controls::button(
                ui,
                tool.name(),
                r,
                Icon::Tool(tool),
                "",
                self.tool == tool,
                true,
            )
            .on_hover_text(tool.name())
            .clicked()
            {
                self.set_tool(tool);
            }
        }
    }

    fn brushes_group(&mut self, ui: &mut Ui, o: Pos2) {
        Self::group(ui, o, 373., 70., " ");
        let brush = controls::split_button(
            ui,
            controls::SplitButton {
                id: "brush",
                rect: Rect::from_min_size(o + vec2(379.0, 5.0), vec2(59.0, 82.0)),
                icon: Icon::Brush(self.brush),
                label: "Brushes",
                selected: self.tool == Tool::Brush,
                menu: controls::SplitMenu {
                    label: "Choose a brush",
                    keys: "ZB",
                    scope: "brushes",
                },
            },
            |ui| {
                let mut preview = self.brush;
                ui.set_min_width(184.0);
                egui::Grid::new("brush_gallery")
                    .num_columns(4)
                    .spacing(vec2(2.0, 2.0))
                    .show(ui, |ui| {
                        for (index, brush) in Brush::ALL.into_iter().enumerate() {
                            let choice = ui.add_sized(
                                vec2(44.0, 44.0),
                                Button::new("").selected(self.brush == brush),
                            );
                            icons::draw(ui.painter(), choice.rect.shrink(5.0), Icon::Brush(brush));
                            choice.widget_info(|| {
                                WidgetInfo::selected(
                                    WidgetType::Button,
                                    true,
                                    self.brush == brush,
                                    brush.name(),
                                )
                            });
                            controls::named(ui, &choice, brush.name());
                            if choice.hovered() || choice.has_focus() {
                                preview = brush;
                            }
                            if choice.clicked() {
                                self.brush = brush;
                                self.set_tool(Tool::Brush);
                                ui.close_menu();
                            }
                            choice.on_hover_text(brush.name());
                            if index % 4 == 3 {
                                ui.end_row();
                            }
                        }
                    });
                ui.separator();
                ui.label(preview.name());
                let (sample, _) = ui.allocate_exact_size(vec2(184.0, 30.0), Sense::hover());
                icons::brush_preview(ui.painter(), sample.shrink2(vec2(8.0, 2.0)), preview);
            },
        );
        if brush.on_hover_text(self.brush.name()).clicked() {
            self.set_tool(Tool::Brush);
        }
    }

    fn shapes_group(&mut self, ui: &mut Ui, o: Pos2) {
        Self::group(ui, o, 444., 251., "Shapes");
        ui.painter().rect_filled(
            Rect::from_min_size(o + vec2(450., 7.), vec2(149., 80.)),
            0.,
            Color32::WHITE,
        );
        let offset_id = ui.id().with("shape_gallery_offset");
        let mut offset = ui
            .ctx()
            .data(|data| data.get_temp::<f32>(offset_id).unwrap_or(0.0));
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(o + vec2(451.0, 9.0), vec2(126.0, 75.0))),
            |ui| {
                let gallery = ScrollArea::vertical()
                    .id_salt("shape_gallery")
                    .max_height(75.0)
                    .auto_shrink([false, false])
                    // Shape presses select tools; scrolling uses the wheel or arrows.
                    .drag_to_scroll(false)
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .vertical_scroll_offset(offset)
                    .show(ui, |ui| {
                        let (rect, _) = ui.allocate_exact_size(vec2(126.0, 100.0), Sense::hover());
                        self.shape_gallery_buttons(ui, rect.min, vec2(18.0, 25.0))
                    });
                offset = gallery.state.offset.y;
                // The inner vertical scroll area consumes scroll requests on
                // both axes. Forward focus to the ribbon's horizontal scroller.
                if let Some(rect) = gallery.inner.1 {
                    ui.scroll_to_rect(rect, None);
                }
            },
        );
        for (row, icon, name, enabled, direction) in [
            (0, Icon::ChevronUp, "Scroll shapes up", offset > 0.0, -1.0),
            (
                1,
                Icon::ChevronDown,
                "Scroll shapes down",
                offset < 25.0,
                1.0,
            ),
        ] {
            let response = controls::button(
                ui,
                name,
                Rect::from_min_size(o + vec2(580.0, 9.0 + row as f32 * 25.0), vec2(17.0, 25.0)),
                icon,
                "",
                false,
                enabled,
            )
            .on_hover_text(name);
            response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, name));
            controls::named(ui, &response, name);
            if response.clicked() {
                offset = (offset + direction * 25.0).clamp(0.0, 25.0);
            }
        }
        ui.ctx()
            .data_mut(|data| data.insert_temp(offset_id, offset));
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(o + vec2(580.0, 59.0), vec2(17.0, 25.0))),
            |ui| {
                let more = egui::menu::menu_custom_button(
                    ui,
                    Button::new("").min_size(vec2(17.0, 25.0)),
                    |ui| {
                        controls::scope(ui, Scope::new("shapes", "Shapes"), |ui| {
                            let (rect, _) =
                                ui.allocate_exact_size(vec2(154.0, 100.0), Sense::hover());
                            if self.shape_gallery_buttons(ui, rect.min, vec2(22.0, 25.0)).0 {
                                ui.close_menu();
                            }
                        });
                    },
                );
                icons::draw(
                    ui.painter(),
                    more.response.rect.shrink(3.0),
                    Icon::ChevronDown,
                );
                more.response
                    .widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "More shapes"));
                controls::register(
                    ui,
                    &more.response,
                    "GM",
                    keytips::Kind::Menu { scope: "shapes" },
                );
                more.response.on_hover_text("More shapes");
            },
        );
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(o + vec2(605., 11.), vec2(88., 76.))),
            |ui| {
                let outline =
                    ribbon_menu_button(ui, "Outline", "O", "outline", Some(Icon::Outline), |ui| {
                        for style in PaintStyle::ALL {
                            let choice = paint_style_choice(
                                ui,
                                self.outline == style,
                                style,
                                if style == PaintStyle::None {
                                    "No outline"
                                } else {
                                    style.name()
                                },
                                self.colors[0],
                                false,
                            );
                            self.preview_shape_style(&choice, shapes::StylePreview::Outline(style));
                            if choice.clicked() {
                                self.outline = style;
                                self.accept_shape_style(ui.ctx());
                                ui.close_menu();
                            }
                        }
                    });
                outline.response.widget_info(|| {
                    WidgetInfo::labeled(WidgetType::Button, true, "Shape outline style")
                });
                ribbon_focus(ui, &outline.response);
                ui.add_space(7.);
                let fill = ribbon_menu_button(ui, "Fill", "L", "fill", Some(Icon::Fill), |ui| {
                    for style in PaintStyle::ALL {
                        let choice = paint_style_choice(
                            ui,
                            self.fill_gradient.is_none() && self.fill == style,
                            style,
                            if style == PaintStyle::None {
                                "No fill"
                            } else {
                                style.name()
                            },
                            self.colors[1],
                            true,
                        );
                        self.preview_shape_style(&choice, shapes::StylePreview::Fill(style));
                        if choice.clicked() {
                            self.fill = style;
                            self.fill_gradient = None;
                            self.accept_shape_style(ui.ctx());
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    theme::menu_heading(ui, "Color 1 to Color 2", 250.0);
                    for gradient in Gradient::ALL {
                        let (keys, description) = match gradient {
                            Gradient::Vertical => ("V", "Color 1 at the top; Color 2 at the bottom."),
                            Gradient::Horizontal => ("H", "Color 1 on the left; Color 2 on the right."),
                            Gradient::Radial => ("R", "Color 1 at the center; Color 2 at the oval boundary of the shape's bounds."),
                        };
                        let response = ui.add(theme::MenuItem::new(gradient.name())
                            .selected(self.fill_gradient == Some(gradient)).width(250.0))
                            .on_hover_text(description);
                        gradient_preview(
                            ui.painter(),
                            Rect::from_center_size(response.rect.right_center() - vec2(41.0, 0.0), vec2(60.0, 16.0)),
                            gradient,
                            self.colors,
                        );
                        controls::register(ui, &response, keys, keytips::Kind::Button);
                        self.preview_shape_style(&response, shapes::StylePreview::Gradient(gradient));
                        if response.clicked() {
                            self.fill_gradient = Some(gradient);
                            self.accept_shape_style(ui.ctx());
                            ui.close_menu();
                        }
                    }
                });
                fill.response.widget_info(|| {
                    WidgetInfo::labeled(WidgetType::Button, true, "Shape fill style")
                });
                ribbon_focus(ui, &fill.response);
            },
        );
    }

    fn size_group(&mut self, ui: &mut Ui, o: Pos2) {
        Self::group(ui, o, 696., 64., " ");
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(o + vec2(702., 8.), vec2(53., 85.))),
            |ui| {
                let menu = egui::menu::menu_custom_button(
                    ui,
                    Button::new("").min_size(vec2(53.0, 84.0)),
                    |ui| {
                        theme::menu(ui);
                        controls::scope(ui, Scope::new("size", "Size"), |ui| {
                            let sizes = match self.tool {
                                Tool::Pencil => [1, 2, 3, 4],
                                Tool::Eraser => [4, 6, 8, 10],
                                _ => [1, 3, 5, 8],
                            };
                            for size in sizes {
                                let label = format!("{size} px");
                                let choice = ui.add(
                                    theme::MenuItem::new(&label)
                                        .selected(self.size == size)
                                        .width(220.0),
                                );
                                controls::named(ui, &choice, &label);
                                let right = choice.rect.right_center() - vec2(15.0, 0.0);
                                ui.painter().line_segment(
                                    [right - vec2(100.0, 0.0), right],
                                    Stroke::new(size as f32, Color32::from_gray(42)),
                                );
                                self.preview_shape_style(&choice, shapes::StylePreview::Size(size));
                                if choice.clicked() {
                                    self.size = size;
                                    self.accept_shape_style(ui.ctx());
                                    ui.close_menu();
                                }
                            }
                            ui.separator();
                            ui.horizontal(|ui| {
                                theme::restore_widget_chrome(ui);
                                ui.label("Custom size");
                                let size = ui.add(
                                    DragValue::new(&mut self.size)
                                        .range(1..=500)
                                        .update_while_editing(false)
                                        .suffix(" px"),
                                );
                                size.widget_info(|| {
                                    WidgetInfo::labeled(
                                        WidgetType::DragValue,
                                        ui.is_enabled(),
                                        "Custom size",
                                    )
                                });
                                controls::register(ui, &size, "C", keytips::Kind::NumericInput);
                                if size.changed() {
                                    self.accept_shape_style(ui.ctx());
                                }
                            });
                        });
                    },
                );
                controls::register(
                    ui,
                    &menu.response,
                    "W",
                    keytips::Kind::Menu { scope: "size" },
                );
                let rect = menu.response.rect;
                for (row, width) in [1.0_f32, 2.0, 3.0, 5.0].into_iter().enumerate() {
                    let y = rect.top() + 8.0 + row as f32 * 10.0;
                    ui.painter().line_segment(
                        [pos2(rect.left() + 8.0, y), pos2(rect.right() - 8.0, y)],
                        Stroke::new(width, Color32::from_gray(40)),
                    );
                }
                ui.painter().text(
                    rect.center_top() + vec2(0.0, 59.0),
                    Align2::CENTER_CENTER,
                    "Size",
                    FontId::proportional(12.0),
                    Color32::from_gray(35),
                );
                icons::draw(
                    ui.painter(),
                    Rect::from_center_size(rect.center_bottom() - vec2(0.0, 8.0), vec2(10.0, 10.0)),
                    Icon::ChevronDown,
                );
                menu.response.widget_info(|| {
                    WidgetInfo::labeled(
                        WidgetType::Button,
                        true,
                        format!("Brush and outline size, {} pixels", self.size),
                    )
                });
                ribbon_focus(ui, &menu.response);
            },
        );
    }

    fn shape_gallery_buttons(
        &mut self,
        ui: &mut Ui,
        origin: Pos2,
        cell: Vec2,
    ) -> (bool, Option<Rect>) {
        let mut selected = false;
        let mut focused = None;
        for (index, tool) in Tool::SHAPES.into_iter().enumerate() {
            let rect = Rect::from_min_size(
                origin + vec2((index % 7) as f32 * cell.x, (index / 7) as f32 * cell.y),
                cell,
            );
            let response = controls::button(
                ui,
                tool.name(),
                rect,
                Icon::Tool(tool),
                "",
                self.tool == tool,
                true,
            )
            .on_hover_text(tool.name());
            if response.gained_focus() {
                focused = Some(rect);
            }
            if response.clicked() {
                self.set_tool(tool);
                selected = true;
            }
        }
        (selected, focused)
    }

    fn colors_group(&mut self, ui: &mut Ui, o: Pos2) {
        self.colors_group_at(ui, o, 761.0);
    }

    pub(in crate::app) fn colors_group_at(&mut self, ui: &mut Ui, origin: Pos2, x: f32) {
        let o = origin + vec2(x - 761.0, 0.0);
        Self::group(ui, o, 761., 346., "Colors");
        for i in 0..2 {
            let r = Rect::from_min_size(o + vec2(767. + i as f32 * 47., 5.), vec2(44., 81.));
            let response = ui.interact(r, ui.id().with(("color", i)), Sense::click());
            if self.active_color == i || response.hovered() {
                ui.painter().rect(
                    r,
                    0.,
                    Color32::from_rgb(222, 237, 250),
                    Stroke::new(1.0_f32, Color32::from_rgb(166, 202, 233)),
                    StrokeKind::Inside,
                );
            }
            let swatch = Rect::from_center_size(
                r.center_top() + vec2(0., 24.),
                vec2(
                    if i == 0 { 32. } else { 25. },
                    if i == 0 { 32. } else { 25. },
                ),
            );
            if self.colors[i][3] < 255 {
                canvas::checkerboard(ui.painter(), swatch, 5.0);
            }
            ui.painter().rect(
                swatch,
                0.,
                display::color(self.colors[i]),
                Stroke::new(1.0_f32, Color32::from_gray(125)),
                StrokeKind::Inside,
            );
            ui.painter().text(
                r.center_bottom() - vec2(0., 20.),
                Align2::CENTER_CENTER,
                format!("Color {}", i + 1),
                FontId::proportional(12.),
                Color32::from_gray(35),
            );
            if response.clicked() {
                self.active_color = i;
            }
            response.widget_info(|| {
                WidgetInfo::selected(
                    WidgetType::RadioButton,
                    true,
                    self.active_color == i,
                    format!(
                        "Color {}, {}, red {}, green {}, blue {}",
                        i + 1,
                        if i == 0 { "foreground" } else { "background" },
                        self.colors[i][0],
                        self.colors[i][1],
                        self.colors[i][2]
                    ),
                )
            });
            controls::register(ui, &response, &(i + 1).to_string(), keytips::Kind::Button);
            ribbon_focus(ui, &response);
            response.on_hover_text(if i == 0 {
                "Foreground (left mouse button)"
            } else {
                "Background (right mouse button)"
            });
        }
        for (i, rgb) in PALETTE.into_iter().enumerate() {
            let r = Rect::from_min_size(
                o + vec2(863. + (i % 10) as f32 * 20., 8. + (i / 10) as f32 * 23.),
                vec2(18., 20.),
            );
            let response = ui.interact(r, ui.id().with(("swatch", i)), Sense::click());
            ui.painter().rect(
                r,
                0.,
                Color32::WHITE,
                Stroke::new(
                    1.0_f32,
                    if response.hovered() {
                        BLUE
                    } else {
                        Color32::from_gray(178)
                    },
                ),
                StrokeKind::Inside,
            );
            ui.painter()
                .rect_filled(r.shrink(2.), 0., Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
            if response.clicked() || response.secondary_clicked() {
                self.colors[if response.secondary_clicked() {
                    1
                } else {
                    self.active_color
                }] = [rgb[0], rgb[1], rgb[2], 255];
                if controls::current(ui).is_some_and(|scope| scope.name.ends_with("_colors")) {
                    ui.close_menu();
                }
            }
            let name = format!(
                "{}, red {}, green {}, blue {}",
                PALETTE_NAMES[i], rgb[0], rgb[1], rgb[2]
            );
            response.widget_info(|| {
                WidgetInfo::selected(
                    WidgetType::RadioButton,
                    true,
                    self.colors[self.active_color][..3] == rgb,
                    &name,
                )
            });
            controls::register(
                ui,
                &response,
                &controls::palette_key(i),
                keytips::Kind::Button,
            );
            ribbon_focus(ui, &response);
            response.on_hover_text(name);
        }
        let mut recalled_custom = None;
        for i in 0..crate::preferences::RECENT_CUSTOM_COLOR_COUNT {
            let r = Rect::from_min_size(o + vec2(863. + i as f32 * 20., 54.), vec2(18., 20.));
            let c = self.recent_custom_colors.get(i).copied();
            if c.is_some_and(|color| color[3] < 255) {
                canvas::checkerboard(ui.painter(), r.shrink(1.0), 4.0);
            }
            ui.painter().rect(
                r,
                0.,
                c.map(display::color).unwrap_or(RIBBON),
                Stroke::new(1.0_f32, Color32::from_gray(210)),
                StrokeKind::Inside,
            );
            let response = ui.interact(
                r,
                ui.id().with(("custom", i)),
                if c.is_some() {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            );
            if response.clicked() || response.secondary_clicked() {
                if let Some(c) = c {
                    recalled_custom = Some(c);
                    self.colors[if response.secondary_clicked() {
                        1
                    } else {
                        self.active_color
                    }] = c;
                    if controls::current(ui).is_some_and(|scope| scope.name.ends_with("_colors")) {
                        ui.close_menu();
                    }
                }
            }
            let name = c.map_or_else(
                || format!("Empty custom color {}", i + 1),
                |color| {
                    format!(
                        "Custom color {}, red {}, green {}, blue {}",
                        i + 1,
                        color[0],
                        color[1],
                        color[2]
                    )
                },
            );
            response.widget_info(|| {
                WidgetInfo::selected(
                    WidgetType::RadioButton,
                    c.is_some(),
                    c == Some(self.colors[self.active_color]),
                    &name,
                )
            });
            controls::register(
                ui,
                &response,
                &controls::palette_key(i + 20),
                keytips::Kind::Button,
            );
            ribbon_focus(ui, &response);
            response.on_hover_text(name);
        }
        if let Some(color) = recalled_custom {
            self.remember_custom_color(color);
            ui.ctx().request_repaint();
        }
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(o + vec2(863.0, 77.0), vec2(196.0, 19.0))),
            |ui| {
                let mut transparent = self.colors[1][3] == 0;
                let response = ui.checkbox(&mut transparent, "Transparent Color 2");
                controls::register(ui, &response, "Q", keytips::Kind::Button);
                response.clone().on_hover_text("Erase, clear, and extend the canvas with transparency. PNG, WebP, TIFF, and Paint 10 projects retain it.");
                if response.changed() {
                    self.colors[1][3] = if transparent { 0 } else { 255 };
                }
            },
        );
        if controls::button(
            ui,
            "edit_colors",
            Rect::from_min_size(o + vec2(1064., 4.), vec2(43., 80.)),
            Icon::Colors,
            "Edit colors",
            false,
            true,
        )
        .clicked()
        {
            self.hex = format!(
                "{:02X}{:02X}{:02X}",
                self.colors[self.active_color][0],
                self.colors[self.active_color][1],
                self.colors[self.active_color][2]
            );
            self.dialog = Some(Dialog::Colors);
        }
    }

    pub(in crate::app) fn view_ribbon(&mut self, ui: &mut Ui, o: Pos2, ctx: &Context) {
        let groups = [
            Group {
                label: "Zoom",
                width: 229.0,
                icon: Icon::Tool(Tool::Magnifier),
                keys: "ZZ",
                popup: "view_zoom",
            },
            Group {
                label: "Show or hide",
                width: 181.0,
                icon: Icon::Tool(Tool::Rectangle),
                keys: "ZS",
                popup: "view_show",
            },
            Group {
                label: "Display",
                width: 228.0,
                icon: Icon::Tool(Tool::Select),
                keys: "ZD",
                popup: "view_display",
            },
            Group {
                label: "Measure",
                width: 190.0,
                icon: Icon::Tool(Tool::Line),
                keys: "ZM",
                popup: "view_measure",
            },
        ];
        let widths = ribbon_layout::widths(&groups, ui.max_rect().right() - o.x, &[3, 2, 1, 0]);
        let mut x = o.x;
        for (index, (group, width)) in groups.into_iter().zip(widths).enumerate() {
            ribbon_layout::show(
                ui,
                pos2(x, o.y),
                width,
                "view",
                group,
                |ui, origin| match index {
                    0 => self.zoom_group(ui, origin),
                    1 => self.visibility_group(ui, origin - vec2(229.0, 0.0)),
                    2 => self.display_group(ui, origin - vec2(410.0, 0.0), ctx),
                    _ => self.measure_group(ui, origin, ctx),
                },
            );
            x += width;
        }
    }

    fn zoom_group(&mut self, ui: &mut Ui, o: Pos2) {
        Self::group(ui, o, 0., 228., "Zoom");
        for (i, label, factor, icon) in [
            (0, "Zoom in", 2.0, Icon::ZoomIn),
            (1, "Zoom out", 0.5, Icon::ZoomOut),
            (2, "100%", 0.0, Icon::ActualSize),
        ] {
            if controls::button(
                ui,
                label,
                Rect::from_min_size(o + vec2(7. + i as f32 * 72., 6.), vec2(65., 77.)),
                icon,
                label,
                false,
                true,
            )
            .clicked()
            {
                self.zoom = if factor == 0. {
                    1.
                } else {
                    (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM)
                };
            }
        }
    }

    fn visibility_group(&mut self, ui: &mut Ui, o: Pos2) {
        Self::group(ui, o, 229., 180., "Show or hide");
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(o + vec2(242., 10.), vec2(155., 80.))),
            |ui| {
                controls::checkbox(ui, &mut self.rulers, "Rulers");
                controls::checkbox(ui, &mut self.grid, "Gridlines");
                controls::checkbox(ui, &mut self.status_bar, "Status bar");
            },
        );
    }

    fn display_group(&mut self, ui: &mut Ui, o: Pos2, ctx: &Context) {
        Self::group(ui, o, 410., 227., "Display");
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(o + vec2(424., 10.), vec2(200., 80.))),
            |ui| {
                if controls::command(ui, "Full screen     F11").clicked() {
                    self.show_picture(ctx);
                }
                let mut thumbnail = Self::thumbnail_enabled(ctx);
                let response =
                    ui.add_enabled(self.zoom > 1.0, Checkbox::new(&mut thumbnail, "Thumbnail"));
                controls::named(ui, &response, "Thumbnail");
                if response.changed() {
                    Self::set_thumbnail_enabled(ctx, thumbnail);
                }
                if controls::command(ui, "Fit to window").clicked() {
                    let space = ctx.available_rect().size() - vec2(30., 40.);
                    self.zoom = (space.x / self.doc.image.width() as f32)
                        .min(space.y / self.doc.image.height() as f32)
                        .clamp(MIN_ZOOM, MAX_ZOOM);
                }
            },
        );
    }
}

#[cfg(test)]
mod gallery_tests {
    use super::*;

    fn frame(app: &mut PaintApp, ctx: &Context, events: Vec<Event>) -> FullOutput {
        ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 400.0))),
                events,
                time: Some(ctx.cumulative_pass_nr() as f64 / 10.0),
                ..Default::default()
            },
            |ctx| app.ribbon(ctx),
        )
    }

    fn node(output: &FullOutput, label: &str) -> (egui::accesskit::NodeId, egui::accesskit::Node) {
        output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some(label))
            .cloned()
            .unwrap_or_else(|| panic!("missing accessible shape: {label}"))
    }

    #[test]
    fn expanded_gallery_scrolls_and_activates_shapes_through_keyboard_focus() {
        let ctx = Context::default();
        ctx.enable_accesskit();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let mut output = frame(&mut app, &ctx, vec![]);
        for tool in Tool::SHAPES {
            let (_, item) = node(&output, tool.name());
            assert!(!item.is_disabled());
        }
        for tool in [Tool::CloudCallout, Tool::Lightning, Tool::Line] {
            let (id, _) = node(&output, tool.name());
            let _ = frame(
                &mut app,
                &ctx,
                vec![Event::AccessKitActionRequest(
                    egui::accesskit::ActionRequest {
                        action: egui::accesskit::Action::Focus,
                        target: id,
                        data: None,
                    },
                )],
            );
            for _ in 0..8 {
                output = frame(&mut app, &ctx, vec![]);
            }
            let (_, item) = node(&output, tool.name());
            let bounds = item.bounds().unwrap();
            assert!(
                bounds.x0 >= 0.0 && bounds.x1 <= 1200.0,
                "{} is outside expanded ribbon: {bounds:?}",
                tool.name()
            );
            assert!(
                bounds.y0 >= 27.0 && bounds.y1 <= 114.0,
                "{} is outside gallery viewport: {bounds:?}",
                tool.name()
            );
            output = frame(
                &mut app,
                &ctx,
                vec![Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }],
            );
            assert_eq!(app.tool, tool);
        }
        let (inline_lightning, _) = node(&output, Tool::Lightning.name());
        let (more, _) = node(&output, "More shapes");
        let _ = frame(
            &mut app,
            &ctx,
            vec![Event::AccessKitActionRequest(
                egui::accesskit::ActionRequest {
                    action: egui::accesskit::Action::Focus,
                    target: more,
                    data: None,
                },
            )],
        );
        output = frame(
            &mut app,
            &ctx,
            vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert!(keytips::popup_open(&ctx));
        let (last_shape, _) = output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .find(|(id, node)| {
                *id != inline_lightning && node.label() == Some(Tool::Lightning.name())
            })
            .unwrap();
        let _ = frame(
            &mut app,
            &ctx,
            vec![Event::AccessKitActionRequest(
                egui::accesskit::ActionRequest {
                    action: egui::accesskit::Action::Focus,
                    target: *last_shape,
                    data: None,
                },
            )],
        );
        let _ = frame(
            &mut app,
            &ctx,
            vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert_eq!(app.tool, Tool::Lightning);
        assert!(!keytips::popup_open(&ctx));
    }
}

#[cfg(test)]
mod gradient_tests {
    use super::*;

    fn frame(app: &mut PaintApp, ctx: &Context, events: Vec<Event>) -> FullOutput {
        let mut input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 850.0))),
            time: Some(ctx.cumulative_pass_nr() as f64 / 30.0),
            events,
            ..Default::default()
        };
        eframe::App::raw_input_hook(app, ctx, &mut input);
        ctx.run(input, |ctx| {
            if !app.ribbon_keyboard(ctx) {
                app.shortcut(ctx);
            }
            app.titlebar(ctx);
            app.ribbon(ctx);
            app.quick_access_below(ctx);
            app.status(ctx);
            app.canvas(ctx);
            app.keyboard_menu(ctx);
            app.dialogs(ctx);
        })
    }

    fn keys(app: &mut PaintApp, ctx: &Context, keys: &[Key]) {
        for &key in keys {
            frame(
                app,
                ctx,
                vec![Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }],
            );
        }
    }

    fn settle(app: &mut PaintApp, ctx: &Context) -> FullOutput {
        for _ in 0..2 {
            frame(app, ctx, vec![]);
        }
        frame(app, ctx, vec![])
    }

    fn has_label(output: &FullOutput, label: &str) -> bool {
        output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some(label))
    }

    fn center(output: &FullOutput, label: &str) -> Pos2 {
        let bounds = output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some(label))
            .unwrap_or_else(|| panic!("Missing {label}"))
            .1
            .bounds()
            .unwrap();
        pos2(
            ((bounds.x0 + bounds.x1) / 2.0) as f32,
            ((bounds.y0 + bounds.y1) / 2.0) as f32,
        )
    }

    fn click(app: &mut PaintApp, ctx: &Context, position: Pos2) {
        frame(
            app,
            ctx,
            vec![
                Event::PointerMoved(position),
                Event::PointerButton {
                    pos: position,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
                Event::PointerButton {
                    pos: position,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }

    fn field_center(output: &FullOutput, label: &str) -> Pos2 {
        let nodes = &output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes;
        let labels: Vec<_> = nodes
            .iter()
            .filter(|(_, node)| node.label() == Some(label) || node.value() == Some(label))
            .map(|(id, _)| *id)
            .collect();
        let field = nodes
            .iter()
            .find(|(_, node)| node.labelled_by().iter().any(|id| labels.contains(id)))
            .unwrap_or_else(|| panic!("Missing field {label}"))
            .1
            .bounds()
            .unwrap();
        pos2(
            ((field.x0 + field.x1) / 2.0) as f32,
            ((field.y0 + field.y1) / 2.0) as f32,
        )
    }

    #[test]
    fn shape_style_menu_hover_changes_only_display_and_restores_on_exit_or_escape() {
        for (menu, label) in [
            (Key::O, "Watercolor"),
            (Key::L, "Oil"),
            (Key::L, "Vertical gradient"),
            (Key::W, "8 px"),
        ] {
            let context = Context::default();
            context.enable_accesskit();
            let mut app = PaintApp::new_with_context(&context, false);
            app.doc = Document::new(160, 120);
            app.doc.add_object(Object::new(
                ObjectKind::Raster(RgbaImage::from_pixel(14, 14, Rgba([40, 180, 80, 255]))),
                (0, 0),
            ));
            app.set_tool(Tool::Rectangle);
            app.size = 3;
            app.colors = [[20, 70, 190, 255], [230, 160, 70, 180]];
            app.doc.begin();
            app.start_shape_draft(
                shapes::ShapeGeometry::Primitive {
                    tool: Tool::Rectangle,
                    start: (30, 30),
                    end: (120, 90),
                },
                0,
            );
            settle(&mut app, &context);
            let pixels = app.doc.image.clone();
            let objects = app.doc.objects.clone();
            let baseline = app.rendered.clone();
            let dirty = app.doc.dirty();
            let undo = app.doc.can_undo();
            let redo = app.doc.can_redo();
            keys(&mut app, &context, &[Key::F10, Key::H, menu]);
            let popup = settle(&mut app, &context);
            let position = center(&popup, label);
            frame(&mut app, &context, vec![Event::PointerMoved(position)]);
            assert!(
                app.rendered != baseline,
                "{label} must preview on the actual canvas"
            );
            assert_eq!(
                app.rendered.get_pixel(2, 2),
                baseline.get_pixel(2, 2),
                "retained objects stay visible"
            );
            assert!(
                app.doc.image == pixels,
                "hover does not change document pixels"
            );
            assert!(app.doc.objects == objects);
            assert_eq!(
                (app.doc.dirty(), app.doc.can_undo(), app.doc.can_redo()),
                (dirty, undo, redo)
            );
            assert_eq!(
                (app.outline, app.fill, app.fill_gradient, app.size),
                (PaintStyle::Solid, PaintStyle::None, None, 3)
            );
            frame(
                &mut app,
                &context,
                vec![Event::PointerMoved(pos2(1190.0, 700.0))],
            );
            assert!(
                app.rendered == baseline,
                "pointer exit restores chosen style"
            );
            frame(&mut app, &context, vec![Event::PointerMoved(position)]);
            assert!(app.rendered != baseline);
            keys(&mut app, &context, &[Key::Escape]);
            settle(&mut app, &context);
            assert!(app.rendered == baseline, "Escape restores chosen style");
            assert!(app.doc.image == pixels);
            assert!(app.shape_draft.is_some());
            app.finish_editing();
            assert!(
                app.doc.composite() == baseline,
                "save/commit never includes a canceled hover"
            );
            app.doc.undo();
            assert!(app.doc.image.pixels().all(|pixel| pixel[3] == 0));
            assert!(!app.doc.can_undo(), "hover adds no undo history");
        }
    }

    #[test]
    fn choosing_a_hovered_style_commits_that_style_and_keeps_the_draft_adjustable() {
        let context = Context::default();
        context.enable_accesskit();
        let mut app = PaintApp::new_with_context(&context, false);
        app.doc = Document::new(160, 120);
        app.set_tool(Tool::Rectangle);
        app.colors[1] = [200, 60, 80, 255];
        app.doc.begin();
        app.start_shape_draft(
            shapes::ShapeGeometry::Primitive {
                tool: Tool::Rectangle,
                start: (30, 30),
                end: (120, 90),
            },
            0,
        );
        settle(&mut app, &context);
        keys(&mut app, &context, &[Key::F10, Key::H, Key::L]);
        let popup = settle(&mut app, &context);
        let position = center(&popup, "Oil");
        frame(&mut app, &context, vec![Event::PointerMoved(position)]);
        let preview = app.rendered.clone();
        keys(&mut app, &context, &[Key::Num2]);
        assert_eq!(app.fill, PaintStyle::Solid);
        assert!(
            app.rendered == app.doc.composite(),
            "a keyboard choice immediately clears a different hovered row"
        );
        settle(&mut app, &context);
        keys(&mut app, &context, &[Key::F10, Key::H, Key::L]);
        let popup = settle(&mut app, &context);
        let position = center(&popup, "Oil");
        frame(&mut app, &context, vec![Event::PointerMoved(position)]);
        click(&mut app, &context, position);
        settle(&mut app, &context);
        assert_eq!(app.fill, PaintStyle::Oil);
        assert!(app.shape_draft.is_some());
        assert!(app.rendered == preview);
        assert!(app.doc.composite() == preview);
        assert!(!app.doc.can_undo());
        keys(&mut app, &context, &[Key::F10, Key::H, Key::W]);
        let popup = settle(&mut app, &context);
        let position = center(&popup, "8 px");
        frame(&mut app, &context, vec![Event::PointerMoved(position)]);
        keys(&mut app, &context, &[Key::C]);
        frame(&mut app, &context, vec![Event::Text("13".into())]);
        keys(&mut app, &context, &[Key::Enter]);
        assert_eq!(app.size, 13);
        assert!(
            app.rendered == app.doc.composite(),
            "custom size immediately clears a different hovered preset"
        );
        app.finish_editing();
        app.doc.undo();
        assert!(app.doc.image.pixels().all(|pixel| pixel.0 == WHITE));
        assert!(!app.doc.can_undo());
    }

    #[test]
    fn size_menu_uses_tool_presets_and_accepts_exact_custom_values() {
        for os in [
            egui::os::OperatingSystem::Windows,
            egui::os::OperatingSystem::Nix,
            egui::os::OperatingSystem::Mac,
        ] {
            let context = Context::default();
            context.set_os(os);
            context.enable_accesskit();
            let mut app = PaintApp::new_with_context(&context, false);
            for (tool, presets) in [
                (Tool::Pencil, [1, 2, 3, 4]),
                (Tool::Rectangle, [1, 3, 5, 8]),
                (Tool::Eraser, [4, 6, 8, 10]),
            ] {
                app.set_tool(tool);
                settle(&mut app, &context);
                keys(&mut app, &context, &[Key::F10, Key::H, Key::W]);
                let popup = settle(&mut app, &context);
                for size in presets {
                    assert!(has_label(&popup, &format!("{size} px")));
                }
                assert!(!has_label(&popup, "50 px"));
                keys(&mut app, &context, &[Key::Num2]);
                settle(&mut app, &context);
                assert_eq!(app.size, presets[1]);
            }
            for (text, expected, modifiers, cancel) in [
                ("137", 137, Modifiers::CTRL, false),
                ("0", 1, Modifiers::CTRL, false),
                ("900", 500, Modifiers::MAC_CMD, false),
                ("75", 500, Modifiers::CTRL, true),
            ] {
                keys(&mut app, &context, &[Key::F10, Key::H, Key::W, Key::C]);
                settle(&mut app, &context);
                frame(
                    &mut app,
                    &context,
                    vec![Event::Key {
                        key: Key::A,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        // Match the command flag supplied by the native and web backends.
                        modifiers: modifiers | Modifiers::COMMAND,
                    }],
                );
                assert!(
                    keytips::popup_open(&context),
                    "Select all keeps the numeric editor open"
                );
                frame(&mut app, &context, vec![Event::Text(text.into())]);
                keys(
                    &mut app,
                    &context,
                    &[if cancel { Key::Escape } else { Key::Enter }],
                );
                settle(&mut app, &context);
                assert_eq!(app.size, expected);
                assert!(app.selection.is_none(), "Ctrl+A never selects the picture");
                keys(&mut app, &context, &[Key::Escape, Key::Escape, Key::Escape]);
                settle(&mut app, &context);
            }
        }
    }

    #[test]
    fn clicking_the_size_stroke_sample_opens_its_menu() {
        let context = Context::default();
        context.enable_accesskit();
        let mut app = PaintApp::new_with_context(&context, false);
        let output = settle(&mut app, &context);
        let node = output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .find(|(_, node)| {
                node.label()
                    .is_some_and(|label| label.starts_with("Brush and outline size,"))
            })
            .unwrap();
        let bounds = node.1.bounds().unwrap();
        let stroke_sample = pos2(
            ((bounds.x0 + bounds.x1) / 2.0) as f32,
            bounds.y0 as f32 + 10.0,
        );
        click(&mut app, &context, stroke_sample);
        let popup = settle(&mut app, &context);
        assert!(keytips::popup_open(&context));
        assert!(has_label(&popup, "Custom size"));
        assert!(has_label(&popup, "8 px"));
        assert!(!app.doc.dirty());
    }

    #[test]
    fn fill_gradient_keytips_preserve_all_paint_choices_and_outline_menu() {
        let context = Context::default();
        context.enable_accesskit();
        let mut app = PaintApp::new_with_context(&context, false);
        app.outline = PaintStyle::Marker;
        settle(&mut app, &context);
        for (gradient, key) in [
            (Gradient::Vertical, Key::V),
            (Gradient::Horizontal, Key::H),
            (Gradient::Radial, Key::R),
        ] {
            keys(&mut app, &context, &[Key::F10, Key::H, Key::L]);
            let menu = settle(&mut app, &context);
            for style in PaintStyle::ALL {
                assert!(has_label(
                    &menu,
                    if style == PaintStyle::None {
                        "No fill"
                    } else {
                        style.name()
                    }
                ));
            }
            for gradient in Gradient::ALL {
                assert!(has_label(&menu, gradient.name()));
            }
            keys(&mut app, &context, &[key]);
            settle(&mut app, &context);
            assert_eq!(app.fill_gradient, Some(gradient));
            assert_eq!(app.outline, PaintStyle::Marker);
            assert!(!keytips::active(&context));
            assert!(!keytips::popup_open(&context));
        }
        for (style, key) in PaintStyle::ALL.into_iter().zip([
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
        ]) {
            app.fill_gradient = Some(Gradient::Radial);
            keys(&mut app, &context, &[Key::F10, Key::H, Key::L]);
            settle(&mut app, &context);
            keys(&mut app, &context, &[key]);
            settle(&mut app, &context);
            assert_eq!(app.fill, style);
            assert_eq!(app.fill_gradient, None);
        }
        keys(&mut app, &context, &[Key::F10, Key::H, Key::O]);
        let outline = settle(&mut app, &context);
        for gradient in Gradient::ALL {
            assert!(!has_label(&outline, gradient.name()));
        }
        assert!(has_label(&outline, "No outline"));
    }

    #[test]
    fn color_dialog_closes_ribbon_menus_and_preserves_typing_before_ok() {
        let context = Context::default();
        context.enable_accesskit();
        let mut app = PaintApp::new_with_context(&context, false);
        app.set_tool(Tool::Rectangle);
        settle(&mut app, &context);
        for open_menu_by_mouse in [false, true] {
            keys(&mut app, &context, &[Key::F10, Key::H, Key::L, Key::V]);
            let mut ribbon = settle(&mut app, &context);
            assert_eq!(app.fill_gradient, Some(Gradient::Vertical));
            if open_menu_by_mouse {
                click(&mut app, &context, center(&ribbon, "Shape fill style"));
                ribbon = settle(&mut app, &context);
                assert!(keytips::popup_open(&context));
            }
            let position = center(&ribbon, "Edit colors");
            click(&mut app, &context, position);
            let dialog = settle(&mut app, &context);
            assert!(app.dialog == Some(Dialog::Colors));
            assert!(!keytips::popup_open(&context));
            assert!(!has_label(&dialog, "Vertical gradient"));

            click(&mut app, &context, center(&dialog, "Color coordinates"));
            settle(&mut app, &context);
            assert!(keytips::popup_open(&context));
            keys(&mut app, &context, &[Key::Escape]);
            let dialog = settle(&mut app, &context);
            assert!(!keytips::popup_open(&context));
            assert!(app.dialog == Some(Dialog::Colors));

            click(&mut app, &context, field_center(&dialog, "Color text"));
            settle(&mut app, &context);
            frame(
                &mut app,
                &context,
                vec![Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                }],
            );
            frame(&mut app, &context, vec![Event::Text("607C97F".into())]);
            let dialog = settle(&mut app, &context);
            let ok = center(&dialog, "OK");
            frame(
                &mut app,
                &context,
                vec![
                    Event::Text("F".into()),
                    Event::PointerMoved(ok),
                    Event::PointerButton {
                        pos: ok,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers: Modifiers::NONE,
                    },
                    Event::PointerButton {
                        pos: ok,
                        button: PointerButton::Primary,
                        pressed: false,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
            settle(&mut app, &context);
            assert!(app.dialog.is_none());
            assert_eq!(app.colors[0], [96, 124, 151, 255]);
        }
    }

    #[test]
    fn alt_fill_gradient_keytips_then_color_click_handle_batched_input() {
        for already_selected in [true, false] {
            for batched in [false, true] {
                let context = Context::default();
                context.enable_accesskit();
                let mut app = PaintApp::new_with_context(&context, false);
                app.set_tool(Tool::Rectangle);
                app.fill_gradient = already_selected.then_some(Gradient::Vertical);
                let ribbon = settle(&mut app, &context);
                let edit_colors = center(&ribbon, "Edit colors");
                let mut events = Vec::new();
                for (key, modifiers) in [
                    (Key::Escape, Modifiers::NONE),
                    (Key::Escape, Modifiers::NONE),
                    (Key::H, Modifiers::ALT),
                    (Key::L, Modifiers::NONE),
                    (Key::V, Modifiers::NONE),
                ] {
                    for pressed in [true, false] {
                        events.push(Event::Key {
                            key,
                            physical_key: None,
                            pressed,
                            repeat: false,
                            modifiers,
                        });
                        if pressed && key != Key::Escape {
                            events.push(Event::Text(key.name().to_ascii_lowercase()));
                        }
                    }
                }
                events.push(Event::PointerMoved(edit_colors));
                for pressed in [true, false] {
                    events.push(Event::PointerButton {
                        pos: edit_colors,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    });
                }
                if batched {
                    frame(&mut app, &context, events);
                } else {
                    for event in events {
                        frame(&mut app, &context, vec![event]);
                    }
                }
                for _ in 0..12 {
                    frame(&mut app, &context, vec![]);
                }
                assert_eq!(
                    app.fill_gradient,
                    Some(Gradient::Vertical),
                    "already_selected={already_selected}; batched={batched}"
                );
                assert!(app.dialog == Some(Dialog::Colors));
                assert!(!keytips::active(&context));
                assert!(!keytips::popup_open(&context));
            }
        }
    }

    #[test]
    fn color_dialog_keytip_preserves_following_click_and_typing() {
        let context = Context::default();
        context.enable_accesskit();
        let mut app = PaintApp::new_with_context(&context, false);
        settle(&mut app, &context);
        keys(&mut app, &context, &[Key::F10, Key::H, Key::D]);
        let dialog = settle(&mut app, &context);
        let field = field_center(&dialog, "Color text");
        let ok = center(&dialog, "OK");
        keys(&mut app, &context, &[Key::Escape]);
        settle(&mut app, &context);
        assert!(app.dialog.is_none());

        let mut events = Vec::new();
        for key in [Key::F10, Key::H, Key::D] {
            events.push(Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            });
        }
        for position in [field, ok] {
            events.push(Event::PointerMoved(position));
            for pressed in [true, false] {
                events.push(Event::PointerButton {
                    pos: position,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                });
            }
            if position == field {
                events.push(Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                });
                events.push(Event::Text("607C97FF".into()));
            }
        }
        frame(&mut app, &context, events);
        for _ in 0..20 {
            frame(&mut app, &context, vec![]);
        }
        assert!(app.dialog.is_none());
        assert_eq!(app.colors[0], [96, 124, 151, 255]);
    }
}
