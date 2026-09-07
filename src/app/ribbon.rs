use super::ribbon_controls::{self as controls, Scope};
use super::ribbon_layout::Group;
use super::*;

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

    fn clipboard_group(&mut self, ui: &mut Ui, o: Pos2, ctx: &Context) {
        let has_selection = self.text_edit.as_ref().map_or_else(
            || self.selected_region().is_some(),
            |state| !state.selection.is_empty(),
        );
        Self::group(ui, o, 0., 119., "Clipboard");
        if controls::button(
            ui,
            "paste",
            Rect::from_min_size(o + vec2(3., 4.), vec2(46., 63.)),
            Icon::Paste,
            "Paste",
            false,
            true,
        )
        .on_hover_text("Paste (Ctrl+V)")
        .clicked()
        {
            self.action(Action::Paste, ctx);
        }
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(o + vec2(12., 69.), vec2(32., 20.))),
            |ui| {
                let menu = ribbon_menu_button(ui, "", "ZV", "paste", None, |ui| {
                    if controls::command(ui, "Paste from…").clicked() {
                        ui.close_menu();
                        self.action(Action::PasteFrom, ctx);
                    }
                });
                menu.response
                    .widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "Paste options"));
                ribbon_focus(ui, &menu.response);
            },
        );
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
        if controls::button(
            ui,
            "select",
            Rect::from_min_size(o + vec2(125., 4.), vec2(55., 63.)),
            Icon::Tool(Tool::Select),
            "Select",
            self.tool == Tool::Select,
            true,
        )
        .clicked()
        {
            self.set_tool(Tool::Select);
        }
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(o + vec2(142., 68.), vec2(35., 20.))),
            |ui| {
                let menu = ribbon_menu_button(ui, "", "ZS", "select", None, |ui| {
                    if controls::selectable(ui, !self.free_select, "Rectangular selection")
                        .clicked()
                    {
                        self.free_select = false;
                        self.set_tool(Tool::Select);
                        ui.close_menu();
                    }
                    if controls::selectable(ui, self.free_select, "Free-form selection").clicked() {
                        self.free_select = true;
                        self.set_tool(Tool::Select);
                        ui.close_menu();
                    }
                    ui.separator();
                    if controls::command(ui, "Select all      Ctrl+A").clicked() {
                        self.action(Action::SelectAll, ctx);
                        ui.close_menu();
                    }
                    if controls::command(ui, "Delete selection").clicked() {
                        self.delete_selection();
                        ui.close_menu();
                    }
                    if controls::command(ui, "Invert selection").clicked() {
                        self.invert_selection();
                        ui.close_menu();
                    }
                    controls::checkbox(ui, &mut self.transparent, "Transparent selection");
                });
                menu.response.widget_info(|| {
                    WidgetInfo::labeled(WidgetType::Button, true, "Selection options")
                });
                ribbon_focus(ui, &menu.response);
            },
        );
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
        if controls::button(
            ui,
            "brush",
            Rect::from_min_size(o + vec2(379., 5.), vec2(59., 62.)),
            Icon::Brush(self.brush),
            "Brushes",
            self.tool == Tool::Brush,
            true,
        )
        .clicked()
        {
            self.set_tool(Tool::Brush);
        }
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(o + vec2(395., 69.), vec2(38., 20.))),
            |ui| {
                let menu = ribbon_menu_button(ui, "", "ZB", "brushes", None, |ui| {
                    for brush in Brush::ALL {
                        let choice = ui.add_sized(
                            vec2(262.0, 34.0),
                            Button::new("").selected(self.brush == brush),
                        );
                        let rect = choice.rect;
                        icons::draw(
                            ui.painter(),
                            Rect::from_min_size(rect.min + vec2(5.0, 3.0), vec2(28.0, 28.0)),
                            Icon::Brush(brush),
                        );
                        ui.painter().text(
                            rect.left_center() + vec2(42.0, 0.0),
                            Align2::LEFT_CENTER,
                            brush.name(),
                            FontId::proportional(12.0),
                            Color32::from_gray(35),
                        );
                        let sample = Rect::from_center_size(
                            rect.right_center() - vec2(40.0, 0.0),
                            vec2(66.0, 25.0),
                        );
                        icons::brush_preview(ui.painter(), sample, brush);
                        choice.widget_info(|| {
                            WidgetInfo::selected(
                                WidgetType::Button,
                                true,
                                self.brush == brush,
                                brush.name(),
                            )
                        });
                        controls::named(ui, &choice, brush.name());
                        if choice.clicked() {
                            self.brush = brush;
                            self.set_tool(Tool::Brush);
                            ui.close_menu();
                        }
                    }
                });
                menu.response.widget_info(|| {
                    WidgetInfo::labeled(WidgetType::Button, true, "Choose a brush")
                });
                ribbon_focus(ui, &menu.response);
            },
        );
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
                            if controls::selectable(
                                ui,
                                self.outline == style,
                                if style == PaintStyle::None {
                                    "No outline"
                                } else {
                                    style.name()
                                },
                            )
                            .clicked()
                            {
                                self.outline = style;
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
                        if controls::selectable(
                            ui,
                            self.fill_gradient.is_none() && self.fill == style,
                            if style == PaintStyle::None {
                                "No fill"
                            } else {
                                style.name()
                            },
                        )
                        .clicked()
                        {
                            self.fill = style;
                            self.fill_gradient = None;
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    ui.label("Color 1 to Color 2");
                    for gradient in Gradient::ALL {
                        let (keys, description) = match gradient {
                            Gradient::Vertical => ("V", "Color 1 at the top; Color 2 at the bottom."),
                            Gradient::Horizontal => ("H", "Color 1 on the left; Color 2 on the right."),
                            Gradient::Radial => ("R", "Color 1 at the center; Color 2 at the oval boundary of the shape's bounds."),
                        };
                        let response = ui.selectable_label(
                            self.fill_gradient == Some(gradient),
                            gradient.name(),
                        ).on_hover_text(description);
                        controls::register(ui, &response, keys, keytips::Kind::Button);
                        if response.clicked() {
                            self.fill_gradient = Some(gradient);
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
                let r = Rect::from_min_size(ui.cursor().min, vec2(47., 39.));
                for (j, w) in [1.0_f32, 2., 3., 5.].into_iter().enumerate() {
                    ui.painter().line_segment(
                        [
                            r.min + vec2(4., 4. + j as f32 * 10.),
                            r.min + vec2(41., 4. + j as f32 * 10.),
                        ],
                        Stroke::new(w, Color32::from_gray(40)),
                    );
                }
                ui.add_space(46.);
                let menu = ribbon_menu_button(ui, "Size", "W", "size", None, |ui| {
                    for size in [1, 3, 5, 8, 12, 20, 32, 50] {
                        if controls::selectable(ui, self.size == size, &format!("{size} px"))
                            .clicked()
                        {
                            self.size = size;
                            ui.close_menu();
                        }
                    }
                });
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
                Color32::from_rgba_unmultiplied(
                    self.colors[i][0],
                    self.colors[i][1],
                    self.colors[i][2],
                    self.colors[i][3],
                ),
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
        for i in 0..10 {
            let r = Rect::from_min_size(o + vec2(863. + i as f32 * 20., 54.), vec2(18., 20.));
            let c = self.custom_colors.get(i).copied();
            ui.painter().rect(
                r,
                0.,
                c.map(|c| Color32::from_rgb(c[0], c[1], c[2]))
                    .unwrap_or(RIBBON),
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
        ];
        let widths = ribbon_layout::widths(&groups, ui.max_rect().right() - o.x, &[2, 1, 0]);
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
                    _ => self.display_group(ui, origin - vec2(410.0, 0.0), ctx),
                },
            );
            x += width;
        }
    }

    fn zoom_group(&mut self, ui: &mut Ui, o: Pos2) {
        Self::group(ui, o, 0., 228., "Zoom");
        for (i, label, factor) in [(0, "Zoom in", 2.), (1, "Zoom out", 0.5), (2, "100%", 0.)] {
            if controls::button(
                ui,
                label,
                Rect::from_min_size(o + vec2(7. + i as f32 * 72., 6.), vec2(65., 77.)),
                Icon::Tool(Tool::Magnifier),
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
}
