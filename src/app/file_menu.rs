use super::*;
use crate::raster_io::RasterFormat;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FilePane {
    #[default]
    Recent,
    SaveAs,
}

#[derive(Clone, Default)]
struct FileMenuState {
    pane: FilePane,
    last_pass: u64,
}

fn pane_id() -> Id {
    Id::new("paint10_file_menu_pane")
}

fn set_pane(ctx: &Context, pane: FilePane) {
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<FileMenuState>(pane_id())
            .pane = pane
    });
}

fn pane(ctx: &Context) -> FilePane {
    ctx.data(|data| {
        data.get_temp::<FileMenuState>(pane_id())
            .unwrap_or_default()
            .pane
    })
}

/// File commands use larger icons and wrapped labels than ordinary ribbon menus.
struct FileItem<'a> {
    label: &'a str,
    description: Option<&'a str>,
    icon: Option<Icon>,
    compact: bool,
    selected: bool,
    width: f32,
}

impl Widget for FileItem<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let text_inset = if self.compact && self.icon.is_none() {
            8.0
        } else {
            39.0
        };
        let text_width = (self.width - text_inset - 9.0).max(80.0);
        let mut label_job = egui::text::LayoutJob::simple(
            self.label.to_owned(),
            FontId::proportional(13.0),
            ui.visuals().text_color(),
            text_width,
        );
        if self.compact {
            label_job.wrap.max_rows = 1;
        }
        let label = ui.painter().layout_job(label_job);
        let description = self.description.map(|text| {
            ui.painter().layout(
                text.to_owned(),
                FontId::proportional(12.0),
                ui.visuals().weak_text_color(),
                text_width,
            )
        });
        let text_height =
            label.size().y + description.as_ref().map_or(0.0, |text| 3.0 + text.size().y);
        let minimum = if self.compact || matches!(self.label, "Print preview" | "Page setup…") {
            28.0
        } else {
            40.0
        };
        let height = (text_height + 14.0).max(minimum);
        let (rect, response) = ui.allocate_exact_size(vec2(self.width, height), Sense::click());
        keytips::set_badge_anchor(
            ui,
            &response,
            rect.left_center() + vec2(if self.compact { 13.0 } else { 19.0 }, 0.0),
        );
        response
            .widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), self.label));
        if response.gained_focus() {
            response.scroll_to_me(None);
        }
        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact_selectable(&response, self.selected);
            if self.selected || response.hovered() || response.has_focus() {
                ui.painter().rect(
                    rect,
                    0.0,
                    visuals.weak_bg_fill,
                    visuals.bg_stroke,
                    StrokeKind::Inside,
                );
            }
            if let Some(icon) = self.icon {
                icons::draw(
                    ui.painter(),
                    Rect::from_center_size(rect.left_center() + vec2(19.0, 0.0), Vec2::splat(26.0)),
                    icon,
                );
            }
            let top = rect.center().y - text_height / 2.0;
            let label_height = label.size().y;
            ui.painter().galley(
                pos2(rect.left() + text_inset, top),
                label,
                visuals.text_color(),
            );
            if let Some(description) = description {
                ui.painter().galley(
                    pos2(rect.left() + text_inset, top + label_height + 3.0),
                    description,
                    ui.visuals().weak_text_color(),
                );
            }
        }
        response
    }
}

fn command(ui: &mut Ui, label: &str, shortcut: &str, key: &str, enabled: bool) -> Response {
    let icon = match label {
        "New" => Some(Icon::New),
        "Open" => Some(Icon::Open),
        "Save" | "Save as…" | "Save a copy…" | "Save selection as…" => Some(Icon::Save),
        "Print…" | "Page setup…" => Some(Icon::Print),
        "Print preview" => Some(Icon::PrintPreview),
        "Send in email…" => Some(Icon::Email),
        "From scanner or camera…" => Some(Icon::Scanner),
        "Set as desktop background…" => Some(Icon::Wallpaper),
        "Properties" => Some(Icon::Properties),
        "About Paint 10" => Some(Icon::About),
        "Exit" => Some(Icon::Exit),
        _ => None,
    };
    let response = ui.add_enabled(
        enabled,
        FileItem {
            label,
            description: None,
            icon,
            compact: false,
            selected: label == "Save as…" && pane(ui.ctx()) == FilePane::SaveAs,
            width: ui.available_width(),
        },
    );
    if response.hovered() || response.gained_focus() {
        set_pane(
            ui.ctx(),
            if label == "Save as…" {
                FilePane::SaveAs
            } else {
                FilePane::Recent
            },
        );
    }
    keytips::register(ui, &response, "file", "File", key, keytips::Kind::Button);
    if shortcut.is_empty() {
        response
    } else {
        response.on_hover_text(shortcut)
    }
}

impl PaintApp {
    pub(in crate::app) fn file_menu(&mut self, ui: &mut Ui, ctx: &Context) {
        theme::menu(ui);
        let width = 480.0_f32.min((ctx.screen_rect().width() - 20.0).max(300.0));
        let height = (ctx.screen_rect().height() - 90.0).clamp(220.0, 516.0);
        let left_width = 200.0_f32.min(width * 0.43);
        let right_width = width - left_width - 9.0;
        let pass = ctx.cumulative_pass_nr();
        ctx.data_mut(|data| {
            let state = data.get_temp_mut_or_default::<FileMenuState>(pane_id());
            if state.last_pass + 1 < pass {
                state.pane = FilePane::Recent;
            }
            state.last_pass = pass;
        });
        ui.set_width(width);
        ui.spacing_mut().item_spacing.x = 9.0;
        ui.horizontal_top(|ui| {
            let left = ui.allocate_ui_with_layout(
                vec2(left_width, height),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(left_width);
                    ScrollArea::vertical()
                        .id_salt("file_menu_scroll")
                        .drag_to_scroll(false)
                        .max_height(height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.file_menu_contents(ui, ctx));
                },
            );
            ui.painter().vline(
                left.response.rect.right() + 4.0,
                left.response.rect.y_range(),
                Stroke::new(1.0_f32, ui.visuals().widgets.noninteractive.bg_stroke.color),
            );
            ui.allocate_ui_with_layout(
                vec2(right_width, height),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(right_width);
                    ScrollArea::vertical()
                        .id_salt("file_menu_details")
                        .drag_to_scroll(false)
                        .max_height(height - 83.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| match pane(ctx) {
                            FilePane::Recent => self.recent_pictures(ui, ctx),
                            FilePane::SaveAs => self.save_formats(ui, ctx),
                        });
                    ui.separator();
                    self.file_export_commands(ui, ctx);
                },
            );
        });
    }

    fn file_menu_contents(&mut self, ui: &mut Ui, ctx: &Context) {
        for (label, shortcut, key, action) in [
            ("New", "Ctrl+N", "N", Action::New),
            ("Open", "Ctrl+O", "O", Action::Open),
            ("Save", "Ctrl+S", "S", Action::Save),
        ] {
            if command(ui, label, shortcut, key, true).clicked() {
                ui.close_menu();
                self.action(action, ctx);
            }
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let width = ui.available_width() - 22.0;
            ui.allocate_ui_with_layout(vec2(width, 40.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(width);
                if command(ui, "Save as…", "F12", "A", true).clicked() {
                    ui.close_menu();
                    self.action(Action::SaveAs, ctx);
                }
            });
            let response = ui.add_sized([22.0, 40.0], Button::new(""));
            response.widget_info(|| {
                WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), "Save as formats")
            });
            let center = response.rect.center();
            ui.painter().add(egui::Shape::convex_polygon(
                vec![
                    center + vec2(-2.0, -4.0),
                    center + vec2(3.0, 0.0),
                    center + vec2(-2.0, 4.0),
                ],
                ui.visuals().text_color(),
                Stroke::NONE,
            ));
            if response.hovered() || response.clicked() || response.gained_focus() {
                set_pane(ctx, FilePane::SaveAs);
            }
        });
        ui.separator();
        if command(ui, "Print…", "Ctrl+P", "P", true).clicked() {
            self.action(Action::Print, ctx);
            ui.close_menu();
        }
        if command(ui, "Print preview", "", "V", true).clicked() {
            self.finish_editing();
            self.print_preview = Some(crate::print_preview::PrintPreview::new(
                self.doc.composite(),
            ));
            ui.close_menu();
        }
        if command(ui, "Page setup…", "", "U", true).clicked() {
            self.finish_editing();
            self.dialog = Some(Dialog::Print);
            ui.close_menu();
        }
        let scanner_tip = concat!(
            "Scanner and camera drivers require the native app. ",
            "In a browser, import a captured picture with Open or Paste from."
        );
        if command(
            ui,
            "From scanner or camera…",
            "",
            "C",
            self.job.is_none() && !cfg!(target_arch = "wasm32"),
        )
        .on_hover_text(scanner_tip)
        .clicked()
        {
            self.dialog = Some(Dialog::Import);
            self.start_job(ctx, || {
                JobResult::Devices(crate::integration::enumerate_devices())
            });
            ui.close_menu();
        }
        if command(
            ui,
            "Send in email…",
            "",
            "E",
            self.job.is_none() && !cfg!(target_arch = "wasm32"),
        )
        .on_hover_text("Download the picture and attach it in your email app.")
        .clicked()
        {
            self.finish_editing();
            let image = self.doc.composite();
            self.start_job(ctx, move || {
                JobResult::Status(
                    crate::integration::compose_email(&image).map(|_| "Email draft opened".into()),
                )
            });
            ui.close_menu();
        }
        if command(
            ui,
            "Set as desktop background…",
            "",
            "B",
            !cfg!(target_arch = "wasm32"),
        )
        .on_hover_text("Desktop wallpaper access requires the native app.")
        .clicked()
        {
            self.finish_editing();
            if let Some(size) = ctx.input(|input| input.viewport().monitor_size) {
                self.wallpaper_size = (size.x as u32, size.y as u32);
            }
            self.dialog = Some(Dialog::Wallpaper);
            ui.close_menu();
        }
        if command(ui, "Properties", "Ctrl+E", "R", true).clicked() {
            self.action(Action::Properties, ctx);
            ui.close_menu();
        }
        if command(ui, "About Paint 10", "", "T", true).clicked() {
            self.dialog = Some(Dialog::About);
            ui.close_menu();
        }
        ui.separator();
        if command(ui, "Exit", "", "X", true).clicked() {
            self.action(Action::Close, ctx);
            ui.close_menu();
        }
    }

    fn recent_pictures(&mut self, ui: &mut Ui, ctx: &Context) {
        ui.label(RichText::new("Recent pictures").strong());
        ui.separator();
        if self.recent.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new("Pictures you open or save appear here.").weak());
        }
        for (index, path) in self.recent.clone().into_iter().enumerate() {
            let key = ((index + 1) % 10).to_string();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let label = format!("{}  {name}", index + 1);
            let response = ui.add(FileItem {
                label: &label,
                description: None,
                icon: None,
                compact: true,
                selected: false,
                width: ui.available_width(),
            });
            keytips::register(ui, &response, "file", "File", &key, keytips::Kind::Button);
            if response.on_hover_text(path.display().to_string()).clicked() {
                self.pending_path = Some(path);
                self.action(Action::Open, ctx);
                ui.close_menu();
            }
        }
    }

    fn save_formats(&mut self, ui: &mut Ui, ctx: &Context) {
        ui.label(RichText::new("Save as").strong());
        ui.separator();
        for (label, description, key, format, icon) in [
            (
                "PNG picture",
                "Sharp detail and full transparency.",
                "G",
                Some(RasterFormat::Png),
                Icon::Png,
            ),
            (
                "JPEG picture",
                "Smaller files for photographs.",
                "J",
                Some(RasterFormat::Jpeg),
                Icon::Jpeg,
            ),
            (
                "BMP picture",
                "An uncompressed Windows bitmap.",
                "M",
                Some(RasterFormat::Bmp24),
                Icon::Bitmap,
            ),
            (
                "GIF picture",
                "Up to 256 colors for simple drawings.",
                "I",
                Some(RasterFormat::Gif),
                Icon::Gif,
            ),
            (
                "WebP picture",
                "Lossless color and transparency.",
                "W",
                Some(RasterFormat::WebP),
                Icon::Png,
            ),
            (
                "TIFF picture",
                "Full-quality pictures for publishing.",
                "D",
                Some(RasterFormat::Tiff),
                Icon::OtherFormats,
            ),
            (
                "Paint 10 project",
                "Keep layers, text, and images editable.",
                "Q",
                Some(RasterFormat::Project),
                Icon::Save,
            ),
            (
                "Other formats",
                "Choose Windows icons or bitmap color depth.",
                "F",
                None,
                Icon::OtherFormats,
            ),
        ] {
            let response = ui.add(FileItem {
                label,
                description: Some(description),
                icon: Some(icon),
                compact: false,
                selected: false,
                width: ui.available_width(),
            });
            keytips::register(ui, &response, "file", "File", key, keytips::Kind::Button);
            keytips::return_left_to(ui, &response, "A");
            if response.clicked() {
                ui.close_menu();
                if let Some(format) = format {
                    self.save_as_format(format);
                } else {
                    self.action(Action::SaveAs, ctx);
                }
            }
        }
    }

    fn file_export_commands(&mut self, ui: &mut Ui, ctx: &Context) {
        let has_selection = self.selected_region().is_some()
            || self
                .object
                .and_then(|index| self.doc.objects.get(index))
                .is_some_and(|object| object.rendered_dimensions().is_some())
            || self
                .text_edit
                .as_ref()
                .is_some_and(|edit| !edit.text.is_empty());
        for (label, key, selection, enabled) in [
            ("Save a copy…", "Y", false, true),
            ("Save selection as…", "L", true, has_selection),
        ] {
            let response = ui.add_enabled(
                enabled,
                theme::MenuItem::new(label)
                    .icon(Some(Icon::Save))
                    .width(ui.available_width()),
            );
            keytips::register(ui, &response, "file", "File", key, keytips::Kind::Button);
            if response.clicked() {
                ui.close_menu();
                self.export_copy(ctx, selection);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod picker_scroll_tests;

    fn frame(app: &mut PaintApp, ctx: &Context, key: Option<Key>) -> FullOutput {
        frame_at_size(app, ctx, key, vec2(640.0, 600.0))
    }

    fn frame_at_size(
        app: &mut PaintApp,
        ctx: &Context,
        key: Option<Key>,
        size: Vec2,
    ) -> FullOutput {
        frame_events(
            app,
            ctx,
            key.map(|key| Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            })
            .into_iter()
            .collect(),
            size,
        )
    }

    fn frame_events(
        app: &mut PaintApp,
        ctx: &Context,
        events: Vec<Event>,
        size: Vec2,
    ) -> FullOutput {
        let mut input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            events,
            ..Default::default()
        };
        keytips::raw_input(ctx, &mut input);
        ctx.run(input, |ctx| {
            keytips::begin_frame(ctx);
            keytips::keyboard(ctx, Id::new("canvas"));
            TopBottomPanel::top("file_tabs").show(ctx, |ui| {
                let menu = egui::menu::menu_custom_button(ui, Button::new("File"), |ui| {
                    app.file_menu(ui, ctx);
                });
                keytips::register(
                    ui,
                    &menu.response,
                    "tabs",
                    "Tabs",
                    "F",
                    keytips::Kind::Menu { scope: "file" },
                );
            });
            keytips::finish_frame(ctx);
        })
    }

    fn bounds(output: &FullOutput, label: &str) -> Rect {
        let node = &output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some(label) || node.value() == Some(label))
            .unwrap_or_else(|| panic!("Missing control: {label}"))
            .1;
        let rect = node.bounds().unwrap();
        Rect::from_min_max(
            pos2(rect.x0 as f32, rect.y0 as f32),
            pos2(rect.x1 as f32, rect.y1 as f32),
        )
    }

    #[test]
    fn hovering_save_as_switches_the_right_pane_and_other_commands_restore_recent() {
        let ctx = Context::default();
        ctx.enable_accesskit();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.recent = vec![PathBuf::from("/tmp/recent.png")];
        for key in [None, Some(Key::F10), Some(Key::F), None] {
            frame(&mut app, &ctx, key);
        }
        let output = frame(&mut app, &ctx, None);
        let new = bounds(&output, "New");
        let save_as = bounds(&output, "Save as…");
        assert!(bounds(&output, "Recent pictures").left() > new.right());
        let output = frame_events(
            &mut app,
            &ctx,
            vec![Event::PointerMoved(save_as.center())],
            vec2(640.0, 600.0),
        );
        assert_eq!(pane(&ctx), FilePane::SaveAs);
        for label in [
            "PNG picture",
            "JPEG picture",
            "BMP picture",
            "GIF picture",
            "Other formats",
        ] {
            assert!(bounds(&output, label).left() > save_as.right());
        }
        let png = bounds(&output, "PNG picture");
        frame_events(
            &mut app,
            &ctx,
            vec![Event::PointerMoved(png.center())],
            vec2(640.0, 600.0),
        );
        assert_eq!(
            pane(&ctx),
            FilePane::SaveAs,
            "moving into the format pane must keep it open"
        );
        let output = frame_events(
            &mut app,
            &ctx,
            vec![Event::PointerMoved(new.center())],
            vec2(640.0, 600.0),
        );
        assert_eq!(pane(&ctx), FilePane::Recent);
        assert!(bounds(&output, "1  recent.png").left() > new.right());
        assert!(!app.doc.dirty());
    }

    #[test]
    fn file_columns_and_export_controls_fit_a_500_pixel_window() {
        assert_file_columns_fit(vec2(500.0, 400.0));
    }

    #[test]
    fn file_columns_stay_compact_in_a_wide_window() {
        assert_file_columns_fit(vec2(1440.0, 900.0));
    }

    fn assert_file_columns_fit(size: Vec2) {
        let ctx = Context::default();
        ctx.enable_accesskit();
        let mut app = PaintApp::new_with_context(&ctx, false);
        for key in [None, Some(Key::F10), Some(Key::F), None] {
            frame_at_size(&mut app, &ctx, key, size);
        }
        let output = frame_at_size(&mut app, &ctx, None, size);
        let left = bounds(&output, "New");
        for label in ["Recent pictures", "Save a copy…", "Save selection as…"] {
            let rect = bounds(&output, label);
            assert!(rect.left() >= left.right(), "columns overlap: {label}");
            assert!(
                rect.right() <= size.x && rect.bottom() <= size.y,
                "control outside window: {label} {rect:?}"
            );
            assert!(
                rect.right() <= 500.0,
                "menu expands with the window: {label}"
            );
        }
        let save_as = bounds(&output, "Save as…");
        let output = frame_events(
            &mut app,
            &ctx,
            vec![Event::PointerMoved(save_as.center())],
            size,
        );
        for label in ["PNG picture", "JPEG picture", "Other formats"] {
            assert!(
                bounds(&output, label).right() <= size.x,
                "format row overflows: {label}"
            );
        }
    }

    #[test]
    fn arrow_focus_reveals_save_formats_without_saving() {
        let ctx = Context::default();
        ctx.enable_accesskit();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let mut focused_labels = Vec::new();
        for key in [
            None,
            Some(Key::F10),
            Some(Key::F),
            Some(Key::ArrowDown),
            Some(Key::ArrowDown),
            Some(Key::ArrowDown),
        ] {
            frame(&mut app, &ctx, key);
            // Focus requests repaint the real app before the next key press.
            let output = frame(&mut app, &ctx, None);
            let focus = ctx.memory(|memory| memory.focused());
            let label = output
                .platform_output
                .accesskit_update
                .as_ref()
                .and_then(|update| {
                    update
                        .nodes
                        .iter()
                        .find(|(id, _)| Some(id.0) == focus.map(|id| id.value()))
                })
                .map(|(_, node)| node.label().unwrap_or_default().to_owned());
            focused_labels.push((key, label));
        }
        assert_eq!(pane(&ctx), FilePane::SaveAs, "focus: {focused_labels:?}");
        assert!(keytips::active(&ctx));
        assert!(app.file.is_none());
        assert!(!app.doc.dirty());
        frame(&mut app, &ctx, Some(Key::ArrowRight));
        let output = frame(&mut app, &ctx, None);
        let focus = ctx.memory(|memory| memory.focused()).unwrap();
        let node = output
            .platform_output
            .accesskit_update
            .unwrap()
            .nodes
            .into_iter()
            .find(|(id, _)| *id == eframe::egui::accesskit::NodeId(focus.value()))
            .map(|(_, node)| node.label().unwrap_or_default().to_owned());
        assert!(
            matches!(
                node.as_deref(),
                Some(
                    "PNG picture"
                        | "JPEG picture"
                        | "BMP picture"
                        | "GIF picture"
                        | "Other formats"
                )
            ),
            "right arrow did not enter the format pane: {node:?}"
        );
        frame(&mut app, &ctx, Some(Key::ArrowLeft));
        let output = frame(&mut app, &ctx, None);
        let focus = ctx.memory(|memory| memory.focused()).unwrap();
        let node = output
            .platform_output
            .accesskit_update
            .unwrap()
            .nodes
            .into_iter()
            .find(|(id, _)| *id == eframe::egui::accesskit::NodeId(focus.value()))
            .map(|(_, node)| node.label().unwrap_or_default().to_owned());
        assert_eq!(node.as_deref(), Some("Save as…"));
        assert!(keytips::popup_open(&ctx));
        assert!(!app.doc.dirty());
    }

    #[test]
    fn file_keytips_use_real_menu_actions_and_recent_file_numbers() {
        let ctx = Context::default();
        ctx.enable_accesskit();
        let mut app = PaintApp::new_with_context(&ctx, false);
        frame(&mut app, &ctx, None);
        frame(&mut app, &ctx, Some(Key::F10));
        frame(&mut app, &ctx, Some(Key::F));
        frame(&mut app, &ctx, Some(Key::V));
        assert!(app.print_preview.is_some());
        assert!(!keytips::active(&ctx));

        app.print_preview = None;
        app.doc.begin();
        d::stamp(&mut app.doc.image, (10, 10), 1, BLACK, Brush::Round);
        app.doc.commit();
        let path = PathBuf::from("/tmp/paint10-recent-keytip-test.png");
        app.recent = vec![path.clone()];
        frame(&mut app, &ctx, None);
        frame(&mut app, &ctx, Some(Key::F10));
        frame(&mut app, &ctx, Some(Key::F));
        frame(&mut app, &ctx, Some(Key::Num1));
        assert_eq!(app.pending_path, Some(path));
        assert!(matches!(app.pending, Some(Action::Open)));
        assert_eq!(app.doc.image.get_pixel(10, 10).0, BLACK);
    }

    #[test]
    fn file_export_choices_expose_selection_availability() {
        let ctx = Context::default();
        ctx.enable_accesskit();
        let mut app = PaintApp::new_with_context(&ctx, false);
        frame(&mut app, &ctx, None);
        frame(&mut app, &ctx, Some(Key::F10));
        frame(&mut app, &ctx, Some(Key::F));
        let output = frame(&mut app, &ctx, Some(Key::L));
        assert!(keytips::active(&ctx));
        let nodes = &output.platform_output.accesskit_update.unwrap().nodes;
        for (label, disabled) in [("Save a copy…", false), ("Save selection as…", true)] {
            let node = &nodes
                .iter()
                .find(|(_, node)| node.label() == Some(label))
                .unwrap()
                .1;
            assert_eq!(node.is_disabled(), disabled, "{label}");
        }
        app.selection = Some(Region {
            x: 0,
            y: 0,
            w: 8,
            h: 8,
        });
        let output = frame(&mut app, &ctx, None);
        assert!(output
            .platform_output
            .accesskit_update
            .unwrap()
            .nodes
            .iter()
            .any(|(_, node)| {
                node.label() == Some("Save selection as…") && !node.is_disabled()
            }));
        app.clear_selection();
        let index = app.doc.add_object(Object::new(
            ObjectKind::Image(RgbaImage::from_pixel(3, 2, Rgba(BLACK))),
            (2000, 2000),
        ));
        app.select_object(index);
        let output = frame(&mut app, &ctx, None);
        assert!(output
            .platform_output
            .accesskit_update
            .unwrap()
            .nodes
            .iter()
            .any(|(_, node)| {
                node.label() == Some("Save selection as…") && !node.is_disabled()
            }));
    }

    #[test]
    fn short_window_can_activate_the_last_recent_file() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.doc.begin();
        d::stamp(&mut app.doc.image, (0, 0), 1, BLACK, Brush::Round);
        app.doc.commit();
        app.recent = (1..=10)
            .map(|index| PathBuf::from(format!("/tmp/recent-picture-{index}.png")))
            .collect();
        for key in [None, Some(Key::F10), Some(Key::F), Some(Key::Num0)] {
            frame_at_size(&mut app, &ctx, key, vec2(500.0, 400.0));
        }
        assert_eq!(app.pending_path, app.recent.last().cloned());
        assert!(matches!(app.pending, Some(Action::Open)));
        assert!(app.doc.dirty());
    }
}
