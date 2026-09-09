//! Optional layer controls. A single-layer picture keeps the ordinary Paint layout.

use super::*;

const ROW_HEIGHT: f32 = 54.0;
const SELECTED: Color32 = Color32::from_rgb(204, 232, 255);
const BORDER: Color32 = Color32::from_rgb(190, 195, 201);

#[derive(Default)]
pub(super) struct LayersState {
    pub(super) open: bool,
    rename: Option<RenameState>,
    completed_name: Option<(usize, String)>,
    opacity_transaction: Option<usize>,
    thumbnails: Vec<Option<TextureHandle>>,
    thumbnails_dirty: bool,
    revision: Option<u64>,
    next_preview: f64,
    dragged: Option<usize>,
    keyboard_ids: Vec<Id>,
    menu_open: bool,
    keyboard_menu: Option<usize>,
    keyboard_menu_focus: bool,
}

struct RenameState {
    index: usize,
    original: String,
    value: String,
    focus: bool,
}

#[derive(Clone)]
struct LayerRow {
    index: usize,
    name: String,
    visible: bool,
    locked: bool,
    opacity: u8,
}

enum LayerAction {
    Select(usize),
    Add,
    Delete(usize),
    Duplicate(usize),
    MergeDown(usize),
    Move {
        from: usize,
        to: usize,
    },
    Visibility(usize, bool),
    Lock(usize, bool),
    Rename(usize),
    SetName(usize, String),
    Opacity {
        index: usize,
        value: u8,
        dragging: bool,
    },
}

impl PaintApp {
    pub(super) fn reveal_layers(&mut self) {
        self.layer_ui.open = true;
    }

    pub(super) fn layer_panel_owns_keyboard(&self, ctx: &Context) -> bool {
        let panel_focused = ctx.memory(|memory| {
            memory
                .focused()
                .is_some_and(|id| self.layer_ui.keyboard_ids.contains(&id))
        });
        let edits_selection = ctx.input(|input| {
            input.events.iter().any(|event| match event {
                Event::Copy | Event::Cut | Event::Paste(_) => true,
                Event::Key {
                    key: Key::Delete,
                    pressed: true,
                    ..
                } => true,
                Event::Key {
                    key: Key::A,
                    pressed: true,
                    modifiers,
                    ..
                } => super::shortcuts::shortcut_modifiers_match(*modifiers, Modifiers::CTRL),
                _ => false,
            })
        });
        self.layer_ui.open
            && (self.layer_ui.rename.is_some()
                || self.layer_ui.menu_open
                || (panel_focused && edits_selection))
    }

    /// Called after the ribbon consumes Shift+F10, before its canvas fallback.
    pub(super) fn open_layer_context_menu(&mut self, ctx: &Context) -> bool {
        if !self.layer_ui.open
            || self.layer_controls_blocked()
            || self.layer_ui.rename.is_some()
            || !ctx.memory(|memory| {
                memory
                    .focused()
                    .is_some_and(|id| self.layer_ui.keyboard_ids.contains(&id))
            })
        {
            return false;
        }
        self.layer_ui.keyboard_menu = Some(self.doc.active_layer_index());
        self.layer_ui.keyboard_menu_focus = true;
        ctx.memory_mut(|memory| memory.open_popup(Id::new("paint10_layer_keyboard_menu")));
        true
    }

    /// Drop document-specific UI references after New, Open, Undo, or Redo.
    /// Whether the user wants the pane visible is a view preference.
    pub(super) fn reset_layer_panel_state(&mut self) {
        self.finish_layer_opacity();
        let open = self.layer_ui.open;
        self.layer_ui = LayersState {
            open,
            ..Default::default()
        };
    }

    pub(super) fn finish_layer_opacity(&mut self) {
        if self.layer_ui.opacity_transaction.take().is_some() {
            self.doc.commit();
        }
    }

    fn settle_layer_editing(&mut self) {
        self.finish_layer_opacity();
        self.finish_editing();
        // A menu or keyboard action can interrupt a held canvas gesture.
        // Keep its last displayed result, then prevent its object index from
        // being interpreted in the newly selected layer.
        if self.gesture.take().is_some() {
            self.doc.commit();
        }
        self.clear_selection();
    }

    pub(super) fn switch_layer(&mut self, index: usize, ctx: &Context) {
        if self.layer_controls_blocked() || index == self.doc.active_layer_index() {
            return;
        }
        if index >= self.doc.layer_count() {
            return;
        }
        self.settle_layer_editing();
        if let Err(error) = self.doc.set_active_layer(index) {
            self.message = error;
            return;
        }
        self.layer_ui.rename = None;
        self.layer_selection_message();
        self.refresh = true;
        ctx.request_repaint();
    }

    fn layer_controls_blocked(&self) -> bool {
        self.dialog.is_some() || self.pending.is_some() || self.browser_dialog_open()
    }

    fn layer_selection_message(&mut self) {
        let layer = self.doc.active_layer();
        self.message = if layer.locked {
            format!(
                "{} is locked. Unlock it in Layers to draw on it.",
                layer.name
            )
        } else if !layer.visible {
            format!("{} is hidden. Show it in Layers to draw on it.", layer.name)
        } else {
            format!("Drawing on: {}", layer.name)
        };
    }

    pub(super) fn layers_panel(&mut self, ctx: &Context) {
        if !ctx.input(|input| input.pointer.any_down()) {
            self.finish_layer_opacity();
        }
        if !self.layer_ui.open {
            return;
        }
        if !self.layer_controls_blocked()
            && self.layer_ui.rename.is_none()
            && !self.layer_ui.menu_open
            && ctx.memory(|memory| {
                memory
                    .focused()
                    .is_some_and(|id| self.layer_ui.keyboard_ids.contains(&id))
            })
            && ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Delete))
        {
            self.apply_layer_action(LayerAction::Delete(self.doc.active_layer_index()), ctx);
        }
        self.sync_layer_thumbnails(ctx);
        let rows: Vec<_> = self
            .doc
            .layers()
            .iter()
            .enumerate()
            .map(|(index, layer)| LayerRow {
                index,
                name: layer.name.clone(),
                visible: layer.visible,
                locked: layer.locked,
                opacity: layer.opacity,
            })
            .collect();
        if self.layer_ui.rename.as_ref().is_some_and(|rename| {
            rows.get(rename.index)
                .is_none_or(|row| row.name != rename.original)
        }) {
            self.layer_ui.rename = None;
        }
        let active = self.doc.active_layer_index();
        let enabled = !self.layer_controls_blocked();
        let width = panel_width(ctx.available_rect().width());
        let mut action = None;
        let mut close = false;
        self.layer_ui.keyboard_ids.clear();
        self.layer_ui.menu_open = false;
        SidePanel::right("paint10_layers")
            .resizable(false)
            .exact_width(width)
            .frame(Frame::NONE.fill(RIBBON).inner_margin(Margin::same(7)))
            .show(ctx, |ui| {
                ui.painter().vline(
                    ui.max_rect().left() - 7.0,
                    ui.max_rect().y_range(),
                    Stroke::new(1.0_f32, BORDER),
                );
                ui.horizontal(|ui| {
                    ui.strong("Layers");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let response = ui.add_sized([24.0, 24.0], Button::new("×").frame(false));
                        self.layer_ui.keyboard_ids.push(response.id);
                        response.widget_info(|| {
                            WidgetInfo::labeled(WidgetType::Button, true, "Close Layers")
                        });
                        close = response.on_hover_text("Close Layers").clicked();
                    });
                });
                ui.separator();
                ui.add_enabled_ui(enabled, |ui| {
                    let list_height = (ui.available_height() - 41.0).max(ROW_HEIGHT);
                    ScrollArea::vertical()
                        .id_salt("paint10_layer_list")
                        .auto_shrink([false, false])
                        .max_height(list_height)
                        .min_scrolled_height(ROW_HEIGHT)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 3.0;
                            for row in rows.iter().rev() {
                                self.layer_row(ui, row, active, &mut action);
                            }
                        });
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let add = ui.add_enabled(
                            rows.len() < d::MAX_LAYERS,
                            Button::new("Add").min_size(vec2(45.0, 28.0)),
                        );
                        self.layer_ui.keyboard_ids.push(add.id);
                        if add
                            .on_hover_text("Add a transparent layer above the selected layer")
                            .clicked()
                        {
                            action = Some(LayerAction::Add);
                        }
                        let delete = ui.add_enabled(
                            rows.len() > 1 && !rows[active].locked,
                            Button::new("Delete").min_size(vec2(52.0, 28.0)),
                        );
                        self.layer_ui.keyboard_ids.push(delete.id);
                        if delete
                            .on_hover_text("Delete the selected layer (Undo restores it)")
                            .clicked()
                        {
                            action = Some(LayerAction::Delete(active));
                        }
                        let menu = ui.menu_button("More", |ui| {
                            self.layer_options(ui, &rows[active], &mut action);
                        });
                        self.layer_ui.keyboard_ids.push(menu.response.id);
                        menu.response
                            .on_hover_text("Layer name, order, merging, lock, and opacity");
                    });
                });
            });
        if !ctx.input(|input| input.pointer.primary_down()) {
            self.layer_ui.dragged = None;
        }
        if let Some((index, name)) = self.layer_ui.completed_name.take() {
            self.apply_layer_action(LayerAction::SetName(index, name), ctx);
        }
        if let Some(action) = action {
            if !matches!(action, LayerAction::Opacity { .. })
                && self.layer_ui.keyboard_menu.take().is_some()
            {
                ctx.memory_mut(|memory| memory.close_popup());
            }
            self.apply_layer_action(action, ctx);
        }
        if close {
            self.finish_layer_opacity();
            self.layer_ui.rename = None;
            self.layer_ui.open = false;
            ctx.memory_mut(|memory| memory.request_focus(Id::new("canvas")));
        }
    }

    fn sync_layer_thumbnails(&mut self, ctx: &Context) {
        let revision = self.doc.revision();
        let count = self.doc.layer_count();
        let revision_changed = self.layer_ui.revision != Some(revision);
        if revision_changed || self.refresh || self.layer_ui.thumbnails.len() != count {
            self.layer_ui.thumbnails_dirty = true;
        }
        let now = ctx.input(|input| input.time);
        if self.layer_ui.thumbnails_dirty
            && (self.gesture.is_none() || now >= self.layer_ui.next_preview)
        {
            self.layer_ui.thumbnails.resize_with(count, || None);
            for (index, layer) in self.doc.layers().iter().enumerate() {
                let mut raster = layer.thumbnail(88);
                if self.doc.mono {
                    for pixel in raster.pixels_mut() {
                        let luminance = u32::from(pixel[0]) * 299
                            + u32::from(pixel[1]) * 587
                            + u32::from(pixel[2]) * 114;
                        let value = if luminance >= 128000 { 255 } else { 0 };
                        *pixel = Rgba([value, value, value, pixel[3]]);
                    }
                }
                let image = display::image(
                    [raster.width() as usize, raster.height() as usize],
                    raster.as_raw(),
                );
                if let Some(texture) = &mut self.layer_ui.thumbnails[index] {
                    texture.set(image, TextureOptions::LINEAR);
                } else {
                    self.layer_ui.thumbnails[index] = Some(ctx.load_texture(
                        format!("paint10-layer-{index}"),
                        image,
                        TextureOptions::LINEAR,
                    ));
                }
            }
            self.layer_ui.next_preview = now + 0.15;
            self.layer_ui.thumbnails_dirty = false;
            self.layer_ui.revision = Some(revision);
        }
    }

    fn layer_row(
        &mut self,
        ui: &mut Ui,
        row: &LayerRow,
        active: usize,
        action: &mut Option<LayerAction>,
    ) {
        let (rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
        let selected = row.index == active;
        let select_rect = Rect::from_min_max(rect.min + vec2(27.0, 0.0), rect.max);
        let response = ui.interact(
            select_rect,
            ui.id().with(("layer", row.index)),
            Sense::click_and_drag(),
        );
        self.layer_ui.keyboard_ids.push(response.id);
        response.widget_info(|| {
            WidgetInfo::selected(
                WidgetType::SelectableLabel,
                ui.is_enabled(),
                selected,
                &row.name,
            )
        });
        let background = if selected {
            SELECTED
        } else if response.hovered() || response.has_focus() {
            Color32::from_rgb(229, 243, 255)
        } else {
            Color32::WHITE
        };
        ui.painter().rect(
            rect,
            0.0,
            background,
            Stroke::new(
                1.0_f32,
                if selected || response.has_focus() {
                    BLUE
                } else {
                    BORDER
                },
            ),
            StrokeKind::Inside,
        );
        if response.gained_focus() {
            response.scroll_to_me(None);
        }
        if response.clicked() {
            response.request_focus();
            *action = Some(LayerAction::Select(row.index));
        }
        if response.double_clicked() {
            *action = Some(LayerAction::Rename(row.index));
        }
        if response.has_focus() {
            if ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::F2)) {
                *action = Some(LayerAction::Rename(row.index));
            } else if ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::ArrowUp)) {
                let index = (row.index + 1).min(self.doc.layer_count() - 1);
                ui.memory_mut(|memory| memory.request_focus(ui.id().with(("layer", index))));
                *action = Some(LayerAction::Select(index));
            } else if ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::ArrowDown)) {
                let index = row.index.saturating_sub(1);
                ui.memory_mut(|memory| memory.request_focus(ui.id().with(("layer", index))));
                *action = Some(LayerAction::Select(index));
            }
        }
        if response.drag_started() {
            response.request_focus();
            self.layer_ui.dragged = Some(row.index);
        }
        if let (Some(from), Some(pointer)) = (
            self.layer_ui.dragged,
            ui.input(|input| input.pointer.interact_pos()),
        ) {
            if rect.contains(pointer) {
                let above = pointer.y < rect.center().y;
                let to = drop_destination(from, row.index, above);
                if from != to {
                    let y = if above { rect.top() } else { rect.bottom() };
                    ui.painter()
                        .hline(rect.x_range(), y, Stroke::new(2.0_f32, BLUE));
                    if ui.input(|input| input.pointer.primary_released()) {
                        *action = Some(LayerAction::Move { from, to });
                    }
                }
            }
        }
        let eye_rect =
            Rect::from_center_size(rect.left_center() + vec2(14.0, 0.0), vec2(24.0, 28.0));
        let eye = ui.interact(
            eye_rect,
            ui.id().with(("visible", row.index)),
            Sense::click(),
        );
        self.layer_ui.keyboard_ids.push(eye.id);
        eye.widget_info(|| {
            WidgetInfo::selected(
                WidgetType::Checkbox,
                ui.is_enabled(),
                row.visible,
                format!("Show {}", row.name),
            )
        });
        draw_eye(
            ui.painter(),
            eye_rect,
            row.visible,
            eye.hovered() || eye.has_focus(),
        );
        let eye_clicked = eye.clicked();
        if eye_clicked {
            eye.request_focus();
        }
        if eye
            .on_hover_text(if row.visible {
                "Hide layer"
            } else {
                "Show layer"
            })
            .clicked()
        {
            *action = Some(LayerAction::Visibility(row.index, !row.visible));
        }
        let thumb = Rect::from_center_size(rect.left_center() + vec2(52.0, 0.0), vec2(43.0, 36.0));
        canvas::checkerboard(ui.painter(), thumb, 5.0);
        if let Some(Some(texture)) = self.layer_ui.thumbnails.get(row.index) {
            let size = texture.size_vec2();
            let scale = (thumb.width() / size.x).min(thumb.height() / size.y);
            ui.painter().image(
                texture.id(),
                Rect::from_center_size(thumb.center(), size * scale),
                Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                Color32::from_white_alpha(if row.visible { 255 } else { 110 }),
            );
        }
        ui.painter()
            .rect_stroke(thumb, 0.0, Stroke::new(1.0_f32, BORDER), StrokeKind::Inside);
        let name_rect = Rect::from_min_max(rect.min + vec2(80.0, 5.0), rect.max - vec2(5.0, 5.0));
        if self
            .layer_ui
            .rename
            .as_ref()
            .is_some_and(|rename| rename.index == row.index)
        {
            let rename = self.layer_ui.rename.as_mut().unwrap();
            let mut editor = ui.new_child(
                UiBuilder::new()
                    .id_salt(("layer-name", row.index))
                    .max_rect(Rect::from_min_size(
                        name_rect.min + vec2(0.0, 7.0),
                        vec2(name_rect.width(), 25.0),
                    )),
            );
            let input = editor.add_sized(
                vec2(name_rect.width(), 25.0),
                TextEdit::singleline(&mut rename.value)
                    .id_source("paint10-layer-name")
                    .char_limit(128),
            );
            if rename.focus {
                input.request_focus();
                let mut state = TextEdit::load_state(ui.ctx(), input.id).unwrap_or_default();
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(rename.value.chars().count()),
                    )));
                state.store(ui.ctx(), input.id);
                rename.focus = false;
            }
            if ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
                self.layer_ui.rename = None;
                ui.memory_mut(|memory| memory.request_focus(Id::new("canvas")));
            } else if input.lost_focus() {
                self.layer_ui.completed_name = Some((row.index, rename.value.clone()));
                self.layer_ui.rename = None;
                if ui.input(|input| input.key_pressed(Key::Enter)) {
                    ui.memory_mut(|memory| memory.request_focus(Id::new("canvas")));
                }
            }
        } else {
            let mut job = egui::text::LayoutJob::simple_singleline(
                row.name.clone(),
                FontId::proportional(12.0),
                Color32::from_gray(if row.visible { 32 } else { 115 }),
            );
            job.wrap.max_width = name_rect.width();
            job.wrap.max_rows = 1;
            let galley = ui.painter().layout_job(job);
            ui.painter()
                .galley(name_rect.min + vec2(0.0, 3.0), galley, Color32::BLACK);
            let detail = match (row.locked, row.visible, row.opacity) {
                (true, false, _) => "Locked · Hidden".into(),
                (true, true, _) => "Locked".into(),
                (false, false, _) => "Hidden".into(),
                (false, true, 255) => String::new(),
                (false, true, opacity) => format!("{}% opacity", opacity_percent(opacity)),
            };
            ui.painter().text(
                name_rect.min + vec2(0.0, 26.0),
                Align2::LEFT_TOP,
                detail,
                FontId::proportional(10.0),
                Color32::from_gray(100),
            );
        }
        response.context_menu(|ui| self.layer_options(ui, row, action));
        if self.layer_ui.keyboard_menu == Some(row.index) {
            let popup = Id::new("paint10_layer_keyboard_menu");
            egui::popup::popup_below_widget(
                ui,
                popup,
                &response,
                egui::popup::PopupCloseBehavior::CloseOnClickOutside,
                |ui| self.layer_options(ui, row, action),
            );
            if !ui.memory(|memory| memory.is_popup_open(popup)) {
                self.layer_ui.keyboard_menu = None;
                response.request_focus();
            }
        }
        response.on_hover_text(format!(
            "{}\n{}{}Drag to reorder; double-click to rename.",
            row.name,
            if row.locked { "Locked. " } else { "" },
            if !row.visible { "Hidden. " } else { "" }
        ));
    }

    fn layer_options(&mut self, ui: &mut Ui, row: &LayerRow, action: &mut Option<LayerAction>) {
        self.layer_ui.menu_open = true;
        theme::menu(ui);
        for (label, enabled, next) in [
            ("Rename…", true, LayerAction::Rename(row.index)),
            (
                "Duplicate",
                self.doc.layer_count() < d::MAX_LAYERS,
                LayerAction::Duplicate(row.index),
            ),
            (
                "Merge down",
                row.index > 0
                    && row.visible
                    && !row.locked
                    && self.doc.layers()[row.index - 1].visible
                    && !self.doc.layers()[row.index - 1].locked,
                LayerAction::MergeDown(row.index),
            ),
            (
                "Move up",
                row.index + 1 < self.doc.layer_count(),
                LayerAction::Move {
                    from: row.index,
                    to: row.index + 1,
                },
            ),
            (
                "Move down",
                row.index > 0,
                LayerAction::Move {
                    from: row.index,
                    to: row.index.saturating_sub(1),
                },
            ),
        ] {
            let response = ui.add_enabled(enabled, theme::MenuItem::new(label).width(180.0));
            if self.layer_ui.keyboard_menu_focus && enabled && !ui.is_sizing_pass() {
                response.request_focus();
                self.layer_ui.keyboard_menu_focus = false;
            }
            if response.clicked() {
                *action = Some(next);
                ui.close_menu();
            }
        }
        ui.separator();
        if ui
            .add(
                theme::MenuItem::new("Lock layer")
                    .selected(row.locked)
                    .width(180.0),
            )
            .clicked()
        {
            *action = Some(LayerAction::Lock(row.index, !row.locked));
            ui.close_menu();
        }
        ui.separator();
        theme::restore_widget_chrome(ui);
        ui.label("Opacity");
        let mut value = opacity_percent(row.opacity);
        let response = ui.add(Slider::new(&mut value, 0..=100).suffix("%"));
        #[cfg(test)]
        ui.ctx().data_mut(|data| {
            data.insert_temp(Id::new("paint10-layer-opacity-test-rect"), response.rect)
        });
        if response.changed() {
            *action = Some(LayerAction::Opacity {
                index: row.index,
                value: ((u32::from(value) * 255 + 50) / 100) as u8,
                dragging: ui.input(|input| input.pointer.primary_down()),
            });
        }
    }

    fn apply_layer_action(&mut self, action: LayerAction, ctx: &Context) {
        if self.layer_controls_blocked() {
            return;
        }
        if let LayerAction::Select(index) = action {
            self.switch_layer(index, ctx);
            return;
        }
        if let LayerAction::Opacity {
            index,
            value,
            dragging,
        } = action
        {
            if index >= self.doc.layer_count() || self.doc.layers()[index].opacity == value {
                return;
            }
            if self.layer_ui.opacity_transaction != Some(index) {
                self.settle_layer_editing();
                self.doc.begin();
                self.layer_ui.opacity_transaction = Some(index);
            }
            self.doc.layer_mut(index).unwrap().opacity = value;
            if !dragging {
                self.finish_layer_opacity();
            }
            self.refresh = true;
            return;
        }
        self.settle_layer_editing();
        self.layer_ui.rename = None;
        if let LayerAction::Rename(index) = action {
            if let Some(layer) = self.doc.layers().get(index) {
                self.layer_ui.rename = Some(RenameState {
                    index,
                    original: layer.name.clone(),
                    value: layer.name.clone(),
                    focus: true,
                });
            }
            return;
        }
        self.doc.begin();
        let result = match action {
            LayerAction::Add => self.doc.add_layer().map(|_| ()),
            LayerAction::Delete(index) => self.doc.delete_layer(index),
            LayerAction::Duplicate(index) => self.doc.duplicate_layer(index).map(|_| ()),
            LayerAction::MergeDown(index) => self.doc.merge_layer_down(index),
            LayerAction::Move { from, to } => self.doc.move_layer(from, to),
            LayerAction::Visibility(index, visible) => self
                .doc
                .layer_mut(index)
                .map(|layer| layer.visible = visible)
                .ok_or_else(|| "The layer is no longer available.".to_owned()),
            LayerAction::Lock(index, locked) => self
                .doc
                .layer_mut(index)
                .map(|layer| layer.locked = locked)
                .ok_or_else(|| "The layer is no longer available.".to_owned()),
            LayerAction::SetName(index, name) => {
                let name = name.trim();
                if let Some(layer) = self.doc.layer_mut(index) {
                    if !name.is_empty() {
                        layer.name = bounded_name(name);
                    }
                    Ok(())
                } else {
                    Err("The layer is no longer available.".into())
                }
            }
            LayerAction::Select(_) | LayerAction::Rename(_) | LayerAction::Opacity { .. } => {
                unreachable!()
            }
        };
        match result {
            Ok(()) => {
                self.doc.commit();
                self.layer_selection_message();
            }
            Err(error) => {
                self.doc.cancel();
                self.message = error;
            }
        }
        self.refresh = true;
        self.layer_ui.thumbnails_dirty = true;
        ctx.request_repaint();
    }
}

fn panel_width(available: f32) -> f32 {
    (available * 0.36).clamp(176.0, 216.0)
}

fn opacity_percent(opacity: u8) -> u8 {
    ((u32::from(opacity) * 100 + 127) / 255) as u8
}

fn bounded_name(name: &str) -> String {
    let end = name
        .char_indices()
        .map(|(offset, character)| offset + character.len_utf8())
        .take_while(|end| *end <= 256)
        .last()
        .unwrap_or(0);
    name[..end].to_owned()
}

/// Rows are shown top-to-bottom while the document stores bottom-to-top.
/// An insertion boundary shifts when the dragged row is removed first.
fn drop_destination(from: usize, target: usize, above: bool) -> usize {
    let boundary = target + usize::from(above);
    boundary - usize::from(from < boundary)
}

fn draw_eye(painter: &Painter, rect: Rect, visible: bool, focused: bool) {
    let center = rect.center();
    let ink = Color32::from_gray(if visible { 75 } else { 150 });
    if focused {
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0_f32, BLUE), StrokeKind::Inside);
    }
    painter.add(egui::Shape::closed_line(
        vec![
            center + vec2(-7.0, 0.0),
            center + vec2(-3.0, -4.0),
            center + vec2(3.0, -4.0),
            center + vec2(7.0, 0.0),
            center + vec2(3.0, 4.0),
            center + vec2(-3.0, 4.0),
        ],
        Stroke::new(1.2_f32, ink),
    ));
    painter.circle_filled(center, 2.2, ink);
    if !visible {
        painter.line_segment(
            [center + vec2(-8.0, 7.0), center + vec2(8.0, -7.0)],
            Stroke::new(1.5_f32, ink),
        );
    }
}

#[cfg(test)]
mod tests;
