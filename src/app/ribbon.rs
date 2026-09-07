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

fn ribbon_menu_button<R>(
    ui: &mut Ui,
    label: &str,
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
    let menu = egui::menu::menu_custom_button(ui, Button::new("").min_size(size), contents);
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
                        ui.menu_button("  File  ", |ui| {
                            self.file_menu(ui, ctx);
                        });
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
                        let response = ui.selectable_label(selected, label);
                        response.widget_info(|| {
                            WidgetInfo::selected(
                                WidgetType::SelectableLabel,
                                true,
                                selected,
                                label.trim(),
                            )
                        });
                        if response.clicked() {
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
        if self.collapsed && !tab_activated && !ctx.memory(|memory| memory.any_popup_open()) {
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
        ScrollArea::horizontal()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let origin = ui.cursor().min;
                let text = self.text_tab && self.text_edit.is_some();
                ui.allocate_space(vec2(
                    if text {
                        1000.0
                    } else if self.view_tab {
                        650.0
                    } else {
                        1110.0
                    },
                    112.0,
                ));
                if text {
                    self.text_ribbon(ui, origin, ctx);
                } else if self.view_tab {
                    self.view_ribbon(ui, origin, ctx);
                } else {
                    self.home_ribbon(ui, origin, ctx);
                }
            });
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
        ribbon_focus(ui, &response);
        if response.clicked() {
            self.action(act, ui.ctx());
        }
    }

    pub(in crate::app) fn home_ribbon(&mut self, ui: &mut Ui, o: Pos2, ctx: &Context) {
        self.clipboard_group(ui, o, ctx);
        self.image_group(ui, o, ctx);
        self.tools_group(ui, o);
        self.brushes_group(ui, o);
        self.shapes_group(ui, o);
        self.size_group(ui, o);
        self.colors_group(ui, o);
    }

    fn clipboard_group(&mut self, ui: &mut Ui, o: Pos2, ctx: &Context) {
        let has_selection = self.text_edit.as_ref().map_or_else(
            || self.selected_region().is_some(),
            |state| !state.selection.is_empty(),
        );
        Self::group(ui, o, 0., 119., "Clipboard");
        if icons::button(
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
                let menu = ribbon_menu_button(ui, "", None, |ui| {
                    if ui.button("Paste from…").clicked() {
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
        if icons::button(
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
                let menu = ribbon_menu_button(ui, "", None, |ui| {
                    if ui
                        .selectable_label(!self.free_select, "Rectangular selection")
                        .clicked()
                    {
                        self.free_select = false;
                        self.set_tool(Tool::Select);
                        ui.close_menu();
                    }
                    if ui
                        .selectable_label(self.free_select, "Free-form selection")
                        .clicked()
                    {
                        self.free_select = true;
                        self.set_tool(Tool::Select);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Select all      Ctrl+A").clicked() {
                        self.action(Action::SelectAll, ctx);
                        ui.close_menu();
                    }
                    if ui.button("Delete selection").clicked() {
                        self.delete_selection();
                        ui.close_menu();
                    }
                    if ui.button("Invert selection").clicked() {
                        self.invert_selection();
                        ui.close_menu();
                    }
                    ui.checkbox(&mut self.transparent, "Transparent selection");
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
                let menu = ribbon_menu_button(ui, "Rotate", Some(Icon::Rotate), |ui| {
                    for (label, act) in [
                        ("Rotate right 90°", Action::Rotate(90.)),
                        ("Rotate left 90°", Action::Rotate(270.)),
                        ("Rotate 180°", Action::Rotate(180.)),
                        ("Flip vertical", Action::Flip(false)),
                        ("Flip horizontal", Action::Flip(true)),
                    ] {
                        if ui.button(label).clicked() {
                            self.action(act, ctx);
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    if ui.button("Custom angle…").clicked() {
                        self.angle = self.object.map(|i| self.doc.objects[i].angle).unwrap_or(0.);
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
            if icons::button(
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
        if icons::button(
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
                let menu = ribbon_menu_button(ui, "", None, |ui| {
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
            let response = icons::button(
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
            if response.clicked() {
                offset = (offset + direction * 25.0).clamp(0.0, 25.0);
            }
        }
        ui.ctx()
            .data_mut(|data| data.insert_temp(offset_id, offset));
        let more = icons::button(
            ui,
            "More shapes",
            Rect::from_min_size(o + vec2(580.0, 59.0), vec2(17.0, 25.0)),
            Icon::ChevronDown,
            "",
            false,
            true,
        )
        .on_hover_text("More shapes");
        more.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "More shapes"));
        let popup = ui.id().with("all_shapes");
        if more.clicked() {
            ui.memory_mut(|memory| memory.toggle_popup(popup));
        }
        egui::popup::popup_below_widget(
            ui,
            popup,
            &more,
            egui::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(154.0, 100.0), Sense::hover());
                if self.shape_gallery_buttons(ui, rect.min, vec2(22.0, 25.0)).0 {
                    ui.memory_mut(|memory| memory.close_popup());
                }
            },
        );
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(o + vec2(605., 11.), vec2(88., 76.))),
            |ui| {
                let outline = ribbon_menu_button(ui, "Outline", Some(Icon::Outline), |ui| {
                    for style in PaintStyle::ALL {
                        if ui
                            .selectable_label(
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
                let fill = ribbon_menu_button(ui, "Fill", Some(Icon::Fill), |ui| {
                    for style in PaintStyle::ALL {
                        if ui
                            .selectable_label(
                                self.fill == style,
                                if style == PaintStyle::None {
                                    "No fill"
                                } else {
                                    style.name()
                                },
                            )
                            .clicked()
                        {
                            self.fill = style;
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
                let menu = ribbon_menu_button(ui, "Size", None, |ui| {
                    for size in [1, 3, 5, 8, 12, 20, 32, 50] {
                        if ui
                            .selectable_label(self.size == size, format!("{size} px"))
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
            let response = icons::button(
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
            ui.painter().rect(
                swatch,
                0.,
                Color32::from_rgb(self.colors[i][0], self.colors[i][1], self.colors[i][2]),
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
            }
            let name = format!("Red {}, green {}, blue {}", rgb[0], rgb[1], rgb[2]);
            response.widget_info(|| {
                WidgetInfo::selected(
                    WidgetType::RadioButton,
                    true,
                    self.colors[self.active_color][..3] == rgb,
                    &name,
                )
            });
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
            ribbon_focus(ui, &response);
            response.on_hover_text(name);
        }
        if icons::button(
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
        Self::group(ui, o, 0., 228., "Zoom");
        for (i, label, factor) in [(0, "Zoom in", 2.), (1, "Zoom out", 0.5), (2, "100%", 0.)] {
            if icons::button(
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
                    (self.zoom * factor).clamp(0.125, 8.)
                };
            }
        }
        Self::group(ui, o, 229., 180., "Show or hide");
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(o + vec2(242., 10.), vec2(155., 80.))),
            |ui| {
                ui.checkbox(&mut self.rulers, "Rulers");
                ui.checkbox(&mut self.grid, "Gridlines");
                ui.checkbox(&mut self.status_bar, "Status bar");
            },
        );
        Self::group(ui, o, 410., 227., "Display");
        ui.scope_builder(
            UiBuilder::new().max_rect(Rect::from_min_size(o + vec2(424., 10.), vec2(200., 80.))),
            |ui| {
                if ui.button("Full screen     F11").clicked() {
                    self.show_picture(ctx);
                }
                let mut thumbnail = Self::thumbnail_enabled(ctx);
                if ui
                    .add_enabled(self.zoom > 1.0, Checkbox::new(&mut thumbnail, "Thumbnail"))
                    .changed()
                {
                    Self::set_thumbnail_enabled(ctx, thumbnail);
                }
                if ui.button("Fit to window").clicked() {
                    let space = ctx.available_rect().size() - vec2(30., 40.);
                    self.zoom = (space.x / self.doc.image.width() as f32)
                        .min(space.y / self.doc.image.height() as f32)
                        .clamp(0.125, 8.);
                }
            },
        );
    }

    fn file_menu(&mut self, ui: &mut Ui, ctx: &Context) {
        ui.set_min_width(232.);
        for (label, act) in [
            ("New                         Ctrl+N", Action::New),
            ("Open                        Ctrl+O", Action::Open),
            ("Save                         Ctrl+S", Action::Save),
            ("Save as…                      F12", Action::SaveAs),
        ] {
            if ui.button(label).clicked() {
                ui.close_menu();
                self.action(act, ctx);
            }
        }
        ui.separator();
        if ui.button("Print…                         Ctrl+P").clicked() {
            self.action(Action::Print, ctx);
            ui.close_menu();
        }
        if ui.button("Print preview").clicked() {
            self.finish_editing();
            self.print_preview = Some(crate::print_preview::PrintPreview::new(
                self.doc.composite(),
            ));
            ui.close_menu();
        }
        if ui.button("Page setup…").clicked() {
            self.finish_editing();
            self.dialog = Some(Dialog::Print);
            ui.close_menu();
        }
        if ui
            .add_enabled(self.job.is_none(), Button::new("From scanner or camera…"))
            .clicked()
        {
            self.dialog = Some(Dialog::Import);
            self.start_job(ctx, || {
                JobResult::Devices(crate::integration::enumerate_devices())
            });
            ui.close_menu();
        }
        if ui
            .add_enabled(self.job.is_none(), Button::new("Send in email…"))
            .clicked()
        {
            self.finish_editing();
            let img = self.doc.composite();
            self.start_job(ctx, move || {
                JobResult::Status(
                    crate::integration::compose_email(&img).map(|_| "Email draft opened".into()),
                )
            });
            ui.close_menu();
        }
        if ui.button("Set as desktop background…").clicked() {
            self.finish_editing();
            if let Some(size) = ctx.input(|i| i.viewport().monitor_size) {
                self.wallpaper_size = (size.x as u32, size.y as u32);
            }
            self.dialog = Some(Dialog::Wallpaper);
            ui.close_menu();
        }
        if ui.button("Properties                    Ctrl+E").clicked() {
            self.action(Action::Properties, ctx);
            ui.close_menu();
        }
        if ui.button("About Paint 10").clicked() {
            self.dialog = Some(Dialog::About);
            ui.close_menu();
        }
        if !self.recent.is_empty() {
            ui.separator();
            ui.label("Recent pictures");
            for path in self.recent.clone() {
                if ui
                    .button(path.file_name().unwrap_or_default().to_string_lossy())
                    .clicked()
                {
                    self.pending_path = Some(path);
                    self.action(Action::Open, ctx);
                    ui.close_menu();
                }
            }
        }
        ui.separator();
        if ui.button("Exit").clicked() {
            self.action(Action::Close, ctx);
            ui.close_menu();
        }
    }
}

#[cfg(test)]
mod gallery_tests {
    use super::*;

    fn frame(app: &mut PaintApp, ctx: &Context, events: Vec<Event>) -> FullOutput {
        ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(500.0, 400.0))),
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
    fn narrow_ribbon_reveals_and_activates_shapes_through_keyboard_focus() {
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
                bounds.x0 >= 0.0 && bounds.x1 <= 500.0,
                "{} is outside narrow ribbon: {bounds:?}",
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
        assert!(ctx.memory(|memory| memory.any_popup_open()));
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
        assert!(!ctx.memory(|memory| memory.any_popup_open()));
    }
}
