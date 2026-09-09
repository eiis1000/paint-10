use super::RasterFormat;
use eframe::egui::{self, Button, Context, Key, Modifiers};

/// The picker itself is shared with headless tests; native window creation is
/// used only by the Windows/macOS helper, never by the Linux GTK save path.
struct FormatPicker {
    selected: RasterFormat,
}

impl FormatPicker {
    fn show(&mut self, ctx: &Context) -> Option<Option<RasterFormat>> {
        let cancel = ctx.input_mut(|input| {
            input.consume_key(Modifiers::MAC_CMD, Key::Q)
                || input.consume_key(Modifiers::MAC_CMD, Key::W)
                || input.consume_key(Modifiers::NONE, Key::Escape)
        });
        if cancel {
            return Some(None);
        }
        let mut result = None;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Choose a file format");
            ui.add_space(8.0);
            egui::Grid::new("save_formats")
                .num_columns(2)
                .spacing([18.0, 6.0])
                .show(ui, |ui| {
                    for (index, format) in RasterFormat::ALL.into_iter().enumerate() {
                        ui.radio_value(&mut self.selected, format, format.label());
                        if index % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let proceed = ui.add(Button::new("Continue").min_size(egui::vec2(90.0, 26.0)));
                let cancel = ui.button("Cancel");
                let enter = ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Enter));
                if cancel.clicked() || (enter && cancel.has_focus()) {
                    result = Some(None);
                } else if proceed.clicked() || enter {
                    result = Some(Some(self.selected));
                }
            });
        });
        result
    }
}

#[cfg(any(not(target_os = "linux"), test))]
pub(super) fn choose(title: &str, initial: RasterFormat) -> Result<Option<RasterFormat>, String> {
    use std::sync::{Arc, Mutex};

    struct PickerWindow {
        picker: FormatPicker,
        result: Arc<Mutex<Option<RasterFormat>>>,
    }
    impl eframe::App for PickerWindow {
        fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
            if let Some(result) = self.picker.show(ctx) {
                *self.result.lock().expect("format picker result") = result;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
    let result = Arc::new(Mutex::new(None));
    let output = result.clone();
    eframe::run_native(
        title,
        crate::native_options::with_viewport(
            egui::ViewportBuilder::default()
                .with_inner_size([520.0, 275.0])
                .with_resizable(false),
        ),
        Box::new(move |_| {
            Ok(Box::new(PickerWindow {
                picker: FormatPicker { selected: initial },
                result: output,
            }))
        }),
    )
    .map_err(|error| format!("Could not choose a file format: {error}"))?;
    let chosen = *result
        .lock()
        .map_err(|_| "Could not read the selected format.")?;
    Ok(chosen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_quit_and_close_cancel_the_picker_without_choosing_a_format() {
        for key in [Key::Q, Key::W] {
            for modifiers in [
                Modifiers::MAC_CMD,
                Modifiers::CTRL | Modifiers::COMMAND,
                Modifiers::NONE,
            ] {
                let ctx = Context::default();
                let mut picker = FormatPicker {
                    selected: RasterFormat::Bmp16,
                };
                let mut result = None;
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    result = picker.show(ctx);
                });
                assert!(result.is_none());
                let _ = ctx.run(
                    egui::RawInput {
                        events: vec![egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers,
                        }],
                        ..Default::default()
                    },
                    |ctx| {
                        result = picker.show(ctx);
                    },
                );
                assert_eq!(result, modifiers.mac_cmd.then_some(None));
                assert_eq!(picker.selected, RasterFormat::Bmp16);
            }
        }
    }

    #[test]
    fn all_formats_and_bitmap_depths_are_accessible_and_cancel_is_distinct() {
        let _native_entry_point = choose;
        for format in RasterFormat::ALL {
            let ctx = Context::default();
            ctx.enable_accesskit();
            let mut picker = FormatPicker {
                selected: RasterFormat::Png,
            };
            let mut result = None;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(520.0, 275.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    result = picker.show(ctx);
                },
            );
            assert!(result.is_none());
            let nodes = output.platform_output.accesskit_update.unwrap().nodes;
            for choice in RasterFormat::ALL {
                assert!(
                    nodes
                        .iter()
                        .any(|(_, node)| node.label() == Some(choice.label())),
                    "missing {}",
                    choice.label()
                );
            }
            let bounds = nodes
                .iter()
                .find(|(_, node)| node.label() == Some(format.label()))
                .unwrap()
                .1
                .bounds()
                .unwrap();
            assert!(
                bounds.x0 >= 0.0 && bounds.x1 <= 520.0 && bounds.y0 >= 0.0 && bounds.y1 <= 275.0
            );
            let position = egui::pos2(
                ((bounds.x0 + bounds.x1) / 2.0) as f32,
                ((bounds.y0 + bounds.y1) / 2.0) as f32,
            );
            for pressed in [true, false] {
                let _ = ctx.run(
                    egui::RawInput {
                        events: vec![
                            egui::Event::PointerMoved(position),
                            egui::Event::PointerButton {
                                pos: position,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: Modifiers::NONE,
                            },
                        ],
                        ..Default::default()
                    },
                    |ctx| {
                        result = picker.show(ctx);
                    },
                );
                assert!(result.is_none());
            }
            assert_eq!(picker.selected, format);
            for (key, expected) in [(Key::Enter, Some(format)), (Key::Escape, None)] {
                let _ = ctx.run(
                    egui::RawInput {
                        events: vec![egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: Modifiers::NONE,
                        }],
                        ..Default::default()
                    },
                    |ctx| {
                        result = picker.show(ctx);
                    },
                );
                assert_eq!(result, Some(expected));
            }
        }
    }

    #[test]
    fn enter_on_focused_cancel_does_not_continue_to_the_native_dialog() {
        let ctx = Context::default();
        ctx.enable_accesskit();
        let mut picker = FormatPicker {
            selected: RasterFormat::Bmp16,
        };
        let mut frame = |key: Option<Key>| {
            let mut result = None;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(520.0, 275.0),
                    )),
                    events: key
                        .into_iter()
                        .map(|key| egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: Modifiers::NONE,
                        })
                        .collect(),
                    ..Default::default()
                },
                |ctx| {
                    result = picker.show(ctx);
                },
            );
            (output, result)
        };
        let (output, _) = frame(None);
        let cancel_id = output
            .platform_output
            .accesskit_update
            .unwrap()
            .nodes
            .into_iter()
            .find(|(_, node)| node.label() == Some("Cancel"))
            .unwrap()
            .0;
        let mut focused_cancel = false;
        for _ in 0..16 {
            let (output, result) = frame(Some(Key::Tab));
            assert!(result.is_none());
            if output.platform_output.accesskit_update.unwrap().focus == cancel_id {
                focused_cancel = true;
                break;
            }
        }
        assert!(focused_cancel, "Cancel must be reachable using Tab");
        assert_eq!(frame(Some(Key::Enter)).1, Some(None));
    }
}
