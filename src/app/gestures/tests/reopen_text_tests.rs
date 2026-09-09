use super::*;
use crate::text::TextFormat;

fn editor_is_visible(app: &PaintApp, ctx: &Context, output: &FullOutput, latex: bool) -> bool {
    if latex {
        app.dialog == Some(Dialog::Latex)
            && ctx.memory(|memory| memory.has_focus(Id::new("paint10-latex-source")))
            && output.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "Source")
            })
    } else {
        app.text_edit.is_some() && ctx.memory(|memory| memory.has_focus(Id::new("text_input")))
    }
}

#[test]
fn loaded_text_and_equations_reopen_using_only_their_requested_repaints() {
    for latex in [false, true] {
        for batched in [false, true] {
            let ctx = Context::default();
            ctx.enable_accesskit();
            let source = if latex {
                r"x = \frac{-b \pm \sqrt{b^2-4ac}}{2a}"
            } else {
                "Retained text"
            };
            let mut original = Document::new(600, 400);
            let index = original.add_object(Object::new(
                ObjectKind::Text {
                    text: source.into(),
                    format: TextFormat {
                        latex,
                        ..Default::default()
                    },
                },
                (40, 80),
            ));
            let mut app = PaintApp::new_with_context(&ctx, false);
            app.doc = crate::project::decode(&crate::project::encode(&original).unwrap()).unwrap();
            app.set_tool(Tool::Select);
            for frame in 0..4 {
                pointer_app_frame_at(&mut app, &ctx, vec![], frame as f64 / 60.0);
            }
            let size = app.doc.objects[index].render().dimensions();
            let position = app.canvas_rect.min
                + vec2(40.0 + size.0 as f32 / 2.0, 80.0 + size.1 as f32 / 2.0) * app.zoom;
            let first = coalesced_click(position, PointerButton::Primary);
            let second = coalesced_click(position, PointerButton::Primary);
            let mut time = 0.2;
            let mut output = if batched {
                let mut events = first;
                events.extend(second);
                pointer_app_frame_at(&mut app, &ctx, events, time)
            } else {
                pointer_app_frame_at(&mut app, &ctx, first, time);
                time += 0.1;
                pointer_app_frame_at(&mut app, &ctx, second, time)
            };

            // Do not hide a missing repaint by sending Enter, moving the pointer,
            // or unconditionally rendering settling frames after the double-click.
            for _ in 0..8 {
                if editor_is_visible(&app, &ctx, &output, latex) {
                    break;
                }
                let delay = output.viewport_output[&ViewportId::ROOT].repaint_delay;
                assert!(
                    delay <= std::time::Duration::from_millis(20),
                    "Editor needs another frame but requested {delay:?}; latex={latex}, batched={batched}"
                );
                time += delay.as_secs_f64().max(1.0 / 60.0);
                output = pointer_app_frame_at(&mut app, &ctx, vec![], time);
            }
            assert!(
                editor_is_visible(&app, &ctx, &output, latex),
                "Double-click must display and focus the editor; latex={latex}, batched={batched}"
            );
            assert_eq!(app.object, Some(index));
            assert!(!app.doc.dirty());
            assert!(
                matches!(&app.doc.objects[index].kind, ObjectKind::Text { text, .. } if text == source)
            );
            if latex {
                let source_node = output
                    .platform_output
                    .accesskit_update
                    .as_ref()
                    .unwrap()
                    .nodes
                    .iter()
                    .find(|(_, node)| node.label() == Some("LaTeX source"))
                    .expect("Equation source field is present");
                assert_eq!(source_node.1.value(), Some(source));
            } else {
                assert_eq!(app.text_edit.as_ref().unwrap().text, source);
            }
        }
    }
}
