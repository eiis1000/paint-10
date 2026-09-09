use super::*;

struct Fixture {
    ctx: Context,
    app: PaintApp,
    height: f32,
    textures: std::collections::HashMap<TextureId, ColorImage>,
}

impl Fixture {
    fn new(color: Color) -> Self {
        let ctx = Context::default();
        ctx.enable_accesskit();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.colors[0] = color;
        app.dialog = Some(Dialog::Colors);
        Self {
            ctx,
            app,
            height: 500.0,
            textures: Default::default(),
        }
    }

    fn frame(&mut self, events: Vec<Event>) -> FullOutput {
        let mut input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(500.0, self.height))),
            time: Some(self.ctx.cumulative_pass_nr() as f64 / 30.0),
            events,
            ..Default::default()
        };
        eframe::App::raw_input_hook(&mut self.app, &self.ctx, &mut input);
        let output = self.ctx.run(input, |ctx| self.app.dialogs(ctx));
        for (id, delta) in &output.textures_delta.set {
            if let egui::ImageData::Color(image) = &delta.image {
                if delta.pos.is_none() {
                    self.textures.insert(*id, (**image).clone());
                }
            }
        }
        for id in &output.textures_delta.free {
            self.textures.remove(id);
        }
        output
    }

    fn settle(&mut self) -> FullOutput {
        for _ in 0..8 {
            self.frame(vec![]);
        }
        self.frame(vec![])
    }

    fn click(&mut self, point: Pos2) {
        self.frame(vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
            Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
        ]);
    }

    fn choose_space(&mut self, space: Space) -> FullOutput {
        let current = self
            .ctx
            .data(|data| data.get_temp::<Editor>(key()).unwrap().space);
        if current != space {
            let output = self.settle();
            self.click(bounds(&output, "Color coordinates").center());
            let output = self.settle();
            self.click(bounds(&output, space.label()).center());
        }
        let output = self.settle();
        let actual = self
            .ctx
            .data(|data| data.get_temp::<Editor>(key()).unwrap().space);
        assert_eq!(
            actual, space,
            "The real coordinates menu must activate its selected row"
        );
        output
    }

    fn replace_field(&mut self, label: &str, text: &str) -> FullOutput {
        let output = self.settle();
        self.click(field_bounds(&output, label).center());
        self.settle();
        self.frame(vec![
            key_event(Key::A, Modifiers::COMMAND),
            Event::Paste(text.to_owned()),
        ]);
        self.settle()
    }

    fn state(&self) -> Editor {
        self.ctx
            .data(|data| data.get_temp::<Editor>(key()).unwrap())
    }

    fn choose_slice(&mut self, slice: Slice) -> FullOutput {
        let state = self.state();
        if state.slice != slice {
            let output = self.settle();
            self.click(bounds(&output, "Color plane slice").center());
            let output = self.settle();
            let option = nodes(&output)
                .iter()
                .find(|(_, node)| {
                    node.role() == egui::accesskit::Role::Button
                        && node.label() == Some(slice.label(state.space))
                })
                .unwrap();
            self.click(rect(&option.1).center());
        }
        let output = self.settle();
        assert_eq!(self.state().slice, slice);
        output
    }

    fn plane_bounds(&self, output: &FullOutput) -> Rect {
        let state = self.state();
        let axes = Plane {
            space: state.space,
            slice: state.slice,
        }
        .axes();
        bounds(
            output,
            &format!(
                "{} and {}",
                state.space.channels()[axes[0]].label,
                state.space.channels()[axes[1]].label.to_ascii_lowercase()
            ),
        )
    }

    fn plane_image(&self, output: &FullOutput) -> ColorImage {
        let rect = self.plane_bounds(output);
        let id = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                Shape::Mesh(mesh)
                    if mesh.calc_bounds() == rect && mesh.texture_id != TextureId::Managed(0) =>
                {
                    Some(mesh.texture_id)
                }
                _ => None,
            })
            .expect("The actual color plane must be painted with its coordinate texture");
        self.textures.get(&id).unwrap().clone()
    }
}

fn key_event(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn nodes(output: &FullOutput) -> &[(egui::accesskit::NodeId, egui::accesskit::Node)] {
    &output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
}

fn rect(node: &egui::accesskit::Node) -> Rect {
    let rect = node.bounds().unwrap();
    Rect::from_min_max(
        pos2(rect.x0 as f32, rect.y0 as f32),
        pos2(rect.x1 as f32, rect.y1 as f32),
    )
}

fn bounds(output: &FullOutput, label: &str) -> Rect {
    let node = nodes(output)
        .iter()
        .rev()
        .find(|(_, node)| node.label() == Some(label) || node.value() == Some(label))
        .unwrap_or_else(|| panic!("Missing {label}"));
    rect(&node.1)
}

fn field_bounds(output: &FullOutput, label: &str) -> Rect {
    let labels: Vec<_> = nodes(output)
        .iter()
        .filter(|(_, node)| node.label() == Some(label) || node.value() == Some(label))
        .map(|(id, _)| *id)
        .collect();
    let node = nodes(output)
        .iter()
        .find(|(_, node)| node.labelled_by().iter().any(|id| labels.contains(id)))
        .unwrap_or_else(|| panic!("Missing field {label}"));
    rect(&node.1)
}

#[test]
fn every_color_space_fits_500_pixels_without_changing_rgba() {
    let original = [40, 80, 120, 64];
    let mut fixture = Fixture::new(original);
    fixture.settle();
    for space in Space::ALL {
        let output = fixture.choose_space(space);
        assert_eq!(
            fixture.app.colors[0], original,
            "Switching to {space:?} must be a no-op"
        );
        assert!(Rect::from_min_size(Pos2::ZERO, vec2(500.0, 500.0))
            .contains_rect(fixture.plane_bounds(&output)));
        for label in [
            "Basic colors",
            "Custom colors",
            "New color",
            "Alpha transparency",
            "OK",
            "Cancel",
        ] {
            let rect = bounds(&output, label);
            assert!(
                Rect::from_min_size(Pos2::ZERO, vec2(500.0, 500.0)).contains_rect(rect),
                "{space:?}: {label} is outside the viewport: {rect:?}"
            );
        }
        for channel in space.channels() {
            let rect = field_bounds(&output, channel.label);
            assert!(
                Rect::from_min_size(Pos2::ZERO, vec2(500.0, 500.0)).contains_rect(rect),
                "{space:?}: {} {rect:?}",
                channel.label
            );
        }
    }
    assert!(!fixture.app.doc.dirty() && !fixture.app.doc.can_undo());
}

#[test]
fn paint_rgb_typing_tabs_and_alpha_stay_synchronized() {
    let mut fixture = Fixture::new([0, 0, 0, 128]);
    fixture.settle();
    fixture.frame(vec![
        Event::Text("128".into()),
        key_event(Key::Tab, Modifiers::NONE),
        Event::Text("64".into()),
        key_event(Key::Tab, Modifiers::NONE),
        Event::Text("32".into()),
    ]);
    fixture.settle();
    assert_eq!(fixture.app.colors[0], [128, 64, 32, 128]);
    assert_eq!(fixture.app.hex, "80402080");
    fixture.replace_field("Alpha", "17");
    assert_eq!(fixture.app.colors[0], [128, 64, 32, 17]);
    assert_eq!(fixture.app.hex, "80402011");
    fixture.choose_space(Space::LinearRgb);
    fixture.replace_field("Red", "0.5");
    assert_eq!(fixture.app.colors[0], [188, 64, 32, 17]);
}

#[test]
fn css_and_short_hex_preserve_implicit_alpha_and_restore_exactly() {
    let original = [12, 34, 56, 78];
    let mut fixture = Fixture::new(original);
    fixture.settle();
    for (text, expected) in [
        ("#f00", [255, 0, 0, 78]),
        ("#0f08", [0, 255, 0, 136]),
        ("rgb(30 60 90)", [30, 60, 90, 136]),
        ("hsl(240 100% 50% / 25%)", [0, 0, 255, 64]),
        ("123456AA", [18, 52, 86, 170]),
    ] {
        fixture.replace_field("Color text", text);
        assert_eq!(fixture.app.colors[0], expected, "{text}");
    }
    let output = fixture.settle();
    fixture.click(bounds(&output, "Restore the original color").center());
    fixture.settle();
    assert_eq!(fixture.app.colors[0], original);
    assert_eq!(fixture.app.hex, "0C22384E");
}

#[test]
fn invalid_color_text_keeps_last_preview_and_cancel_restores_rgba() {
    for cancel_with_key in [false, true] {
        let original = [12, 34, 56, 78];
        let mut fixture = Fixture::new(original);
        fixture.settle();
        fixture.replace_field("Color text", "#abcdef80");
        fixture.replace_field("Color text", "oklch(unfinished");
        assert_eq!(fixture.app.colors[0], [171, 205, 239, 128]);
        fixture.frame(vec![key_event(Key::Enter, Modifiers::NONE)]);
        let output = fixture.settle();
        assert!(fixture.app.dialog.is_some());
        if cancel_with_key {
            fixture.frame(vec![key_event(Key::Escape, Modifiers::NONE)]);
        } else {
            fixture.click(bounds(&output, "Cancel").center());
        }
        fixture.settle();
        assert!(fixture.app.dialog.is_none());
        assert_eq!(fixture.app.colors[0], original);
        assert!(!fixture.app.doc.dirty() && !fixture.app.doc.can_undo());
    }
}

#[test]
fn out_of_gamut_oklch_offers_a_real_fit_action() {
    let mut fixture = Fixture::new([128, 64, 32, 99]);
    fixture.settle();
    fixture.choose_space(Space::Oklch);
    fixture.replace_field("Lightness", "0.7");
    fixture.replace_field("Chroma", "0.4");
    let output = fixture.replace_field("Hue", "30");
    assert!(!fixture
        .ctx
        .data(|data| data.get_temp::<Editor>(key()).unwrap().in_gamut));
    fixture.click(bounds(&output, "Fit to sRGB").center());
    fixture.settle();
    let state = fixture
        .ctx
        .data(|data| data.get_temp::<Editor>(key()).unwrap());
    assert!(state.in_gamut);
    assert!(state.values[1] < 0.4);
    assert_eq!(fixture.app.colors[0][3], 99);
}

#[test]
fn pasted_oklch_keeps_authored_coordinates_until_the_user_fits_it() {
    let mut fixture = Fixture::new([40, 80, 120, 99]);
    fixture.settle();
    let output = fixture.replace_field("Color text", "oklch(70% 0.4 30)");
    let state = fixture
        .ctx
        .data(|data| data.get_temp::<Editor>(key()).unwrap());
    assert_eq!(state.space, Space::Oklch);
    assert_eq!(state.values, [0.7, 0.4, 30.0, 0.0]);
    assert!(!state.in_gamut);
    assert_eq!(state.rgba[3], 99);
    fixture.click(bounds(&output, "Fit to sRGB").center());
    fixture.settle();
    let state = fixture
        .ctx
        .data(|data| data.get_temp::<Editor>(key()).unwrap());
    assert!(state.in_gamut);
    assert!(state.values[1] < 0.4);
    assert_eq!(state.rgba[3], 99);
}

#[test]
fn invalid_alpha_stays_editable_and_enter_cannot_accept_it() {
    let original = [40, 80, 120, 64];
    let mut fixture = Fixture::new(original);
    fixture.settle();
    fixture.replace_field("Alpha", "invalid");
    fixture.frame(vec![key_event(Key::Enter, Modifiers::NONE)]);
    let output = fixture.settle();
    assert!(fixture.app.dialog.is_some());
    assert_eq!(fixture.app.colors[0], original);
    assert!(fixture
        .ctx
        .data(|data| data.get_temp::<Editor>(key()).unwrap().numeric_invalid));
    assert!(fixture.app.dialog_error.is_none());
    assert!(nodes(&output)
        .iter()
        .any(|(_, node)| node.value() == Some("invalid")));
    fixture.click(bounds(&output, "Cancel").center());
    fixture.settle();
    assert!(fixture.app.dialog.is_none());
    assert_eq!(fixture.app.colors[0], original);
}

#[test]
fn extended_srgb_text_offers_clipping_instead_of_perceptual_fitting() {
    let mut fixture = Fixture::new([40, 80, 120, 64]);
    fixture.settle();
    let output = fixture.replace_field("Color text", "color(srgb 1.5 -0.2 0.5)");
    let state = fixture
        .ctx
        .data(|data| data.get_temp::<Editor>(key()).unwrap());
    assert!(!state.in_gamut && state.authored.is_none());
    assert!(!nodes(&output)
        .iter()
        .any(|(_, node)| node.label() == Some("Fit to sRGB")));
    fixture.click(bounds(&output, "Use clipped color").center());
    fixture.settle();
    let state = fixture
        .ctx
        .data(|data| data.get_temp::<Editor>(key()).unwrap());
    assert!(state.in_gamut);
    assert_eq!(fixture.app.colors[0], [255, 0, 128, 64]);
}

#[test]
fn pasted_perceptual_coordinates_survive_conventional_ranges_and_mode_switches() {
    for (text, space, values) in [
        ("oklch(70% 0.5 30)", Space::Oklch, [0.7, 0.5, 30.0, 0.0]),
        ("oklab(0.7 0.6 -0.5)", Space::Oklab, [0.7, 0.6, -0.5, 0.0]),
    ] {
        let mut fixture = Fixture::new([40, 80, 120, 99]);
        fixture.settle();
        fixture.replace_field("Color text", text);
        let original_preview = fixture.app.colors[0];
        fixture.choose_space(Space::Rgb);
        assert_eq!(fixture.app.colors[0], original_preview);
        let output = fixture.choose_space(space);
        let state = fixture
            .ctx
            .data(|data| data.get_temp::<Editor>(key()).unwrap());
        assert_eq!(state.values, values);
        assert_eq!(state.authored, Some((space, values)));
        assert!(!state.in_gamut);
        fixture.click(bounds(&output, "Fit to sRGB").center());
        fixture.settle();
        assert!(fixture
            .ctx
            .data(|data| data.get_temp::<Editor>(key()).unwrap().in_gamut));
        assert_eq!(fixture.app.colors[0][3], 99);
    }
}

#[test]
fn hsv_picker_alpha_slider_and_custom_swatches_preserve_rgba_semantics() {
    let original = [40, 80, 120, 99];
    let mut fixture = Fixture::new(original);
    fixture.app.custom_colors = vec![[18, 52, 86, 170]];
    fixture.settle();
    let output = fixture.choose_space(Space::Hsv);
    assert_eq!(fixture.app.colors[0], original);
    fixture.click(bounds(&output, "Saturation and value").center());
    let output = fixture.settle();
    assert_ne!(fixture.app.colors[0], original);
    assert_eq!(fixture.app.colors[0][3], 99);

    let slider = bounds(&output, "Alpha transparency");
    fixture.click(pos2(
        slider.left() + slider.width() / 4.0,
        slider.center().y,
    ));
    let output = fixture.settle();
    assert_eq!(fixture.app.colors[0][3], 64);
    fixture.click(bounds(&output, "Custom color 1: #123456AA").center());
    let output = fixture.settle();
    assert_eq!(fixture.app.colors[0], [18, 52, 86, 170]);
    fixture.click(bounds(&output, "Restore the original color").center());
    fixture.settle();
    assert_eq!(fixture.app.colors[0], original);
}

#[test]
fn closing_the_color_window_restores_the_original_rgba() {
    let original = [12, 34, 56, 78];
    let mut fixture = Fixture::new(original);
    fixture.settle();
    let output = fixture.replace_field("Color text", "#abcdef80");
    fixture.click(bounds(&output, "Close window").center());
    fixture.settle();
    assert!(fixture.app.dialog.is_none());
    assert_eq!(fixture.app.colors[0], original);
    assert_eq!(fixture.app.hex, "0C22384E");
}

#[test]
fn short_color_window_keeps_exact_entry_and_acceptance_reachable() {
    let mut fixture = Fixture::new([40, 80, 120, 99]);
    fixture.height = 400.0;
    let output = fixture.settle();
    let viewport = Rect::from_min_size(Pos2::ZERO, vec2(500.0, 400.0));
    for label in ["OK", "Cancel", "Color text"] {
        let control = bounds(&output, label);
        assert!(
            viewport.contains_rect(control),
            "{label} must stay reachable: {control:?}"
        );
    }
    let control = field_bounds(&output, "Color text");
    assert!(
        viewport.contains_rect(control),
        "Exact color entry must stay reachable: {control:?}"
    );
    fixture.replace_field("Color text", "#12345680");
    let output = fixture.settle();
    fixture.click(bounds(&output, "OK").center());
    fixture.settle();
    assert!(fixture.app.dialog.is_none());
    assert_eq!(fixture.app.colors[0], [18, 52, 86, 128]);
}

#[test]
fn all_palette_slots_are_visible_and_add_replaces_the_chosen_slot() {
    let mut fixture = Fixture::new([40, 80, 120, 99]);
    let output = fixture.settle();
    let basic: Vec<_> = nodes(&output)
        .iter()
        .filter(|(_, node)| {
            node.label()
                .is_some_and(|name| name.starts_with("Basic color "))
        })
        .collect();
    let custom: Vec<_> = nodes(&output)
        .iter()
        .filter(|(_, node)| {
            node.label()
                .is_some_and(|name| name.starts_with("Custom color "))
        })
        .collect();
    assert_eq!(basic.len(), 48);
    assert_eq!(custom.len(), crate::preferences::CUSTOM_COLOR_COUNT);
    let viewport = Rect::from_min_size(Pos2::ZERO, vec2(500.0, 500.0));
    for (_, node) in basic.into_iter().chain(custom) {
        assert!(
            viewport.contains_rect(rect(node)),
            "Palette cell is outside the viewport"
        );
    }

    let mut expected = vec![WHITE; crate::preferences::CUSTOM_COLOR_COUNT];
    for step in 0..crate::preferences::CUSTOM_COLOR_COUNT {
        let color = [step as u8, 40, 80, 100 + step as u8];
        let slot = step / 2 + (step % 2) * 8;
        expected[slot] = color;
        let output = fixture.replace_field("Color text", &color::format_hex(color, true));
        fixture.click(bounds(&output, "Add to custom colors").center());
        fixture.settle();
        assert_eq!(fixture.app.custom_colors, expected, "Add at step {step}");
        assert_eq!(fixture.app.recent_custom_colors[0], color);
    }

    let output = fixture.settle();
    let label = format!("Custom color 6: {}", color::format_hex(expected[5], true));
    fixture.click(bounds(&output, &label).center());
    let output = fixture.replace_field("Color text", "#12345680");
    fixture.click(bounds(&output, "Add to custom colors").center());
    let output = fixture.settle();
    expected[5] = [18, 52, 86, 128];
    assert_eq!(fixture.app.custom_colors, expected);
    assert_eq!(fixture.app.recent_custom_colors[0], expected[5]);
    assert_eq!(
        fixture.app.recent_custom_colors.len(),
        crate::preferences::RECENT_CUSTOM_COLOR_COUNT
    );
    fixture.click(bounds(&output, "Cancel").center());
    fixture.settle();

    let mut output = None;
    for _ in 0..5 {
        output = Some(fixture.ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1180.0, 800.0))),
                ..Default::default()
            },
            |ctx| fixture.app.ribbon(ctx),
        ));
    }
    let output = output.unwrap();
    let visible = bounds(&output, "Custom color 1, red 18, green 52, blue 86");
    assert!(Rect::from_min_size(Pos2::ZERO, vec2(1180.0, 800.0)).contains_rect(visible));
    assert_eq!(
        fixture.app.custom_colors, expected,
        "Home recents must not reorder stored slots"
    );
}

#[test]
fn coordinate_choice_changes_the_actual_plane_image_without_changing_literal_or_rgba() {
    let original = [40, 80, 120, 99];
    let literal = "rgb(40 80 120 / 38.823529%)";
    let mut fixture = Fixture::new(original);
    fixture.settle();
    fixture.replace_field("Color text", literal);
    let mut images: Vec<ColorImage> = Vec::new();
    for space in Space::ALL {
        let output = fixture.choose_space(space);
        let image = fixture.plane_image(&output);
        assert!(
            images
                .iter()
                .all(|previous| previous.size != image.size || previous.pixels != image.pixels),
            "Selecting {space:?} must render its own coordinate plane"
        );
        images.push(image);
        assert_eq!(fixture.app.colors[0], original);
        assert_eq!(fixture.app.hex, literal);

        for &slice in Plane::slices(space) {
            let output = fixture.choose_slice(slice);
            assert_eq!(fixture.app.colors[0], original, "{space:?}: {slice:?}");
            assert_eq!(fixture.app.hex, literal);
            assert!(fixture.plane_bounds(&output).is_positive());
        }
    }
    assert!(!fixture.app.doc.dirty() && !fixture.app.doc.can_undo());
}

#[test]
fn plane_and_strip_pointer_edits_use_every_selected_space_and_keep_alpha() {
    for space in Space::ALL {
        let mut fixture = Fixture::new([40, 80, 120, 99]);
        fixture.settle();
        let output = fixture.choose_space(space);
        let before = fixture.state();
        let plane = Plane {
            space,
            slice: before.slice,
        };
        let rect = fixture.plane_bounds(&output);
        fixture.click(pos2(
            rect.left() + rect.width() * 0.75,
            rect.top() + rect.height() * 0.25,
        ));
        let output = fixture.settle();
        let state = fixture.state();
        let expected = plane.coordinates_at(before.values, 0.75, 0.75);
        for (actual, expected) in state.values.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() < 0.001,
                "{space:?}: {actual} vs {expected}"
            );
        }
        assert_eq!(state.rgba[3], 99);
        assert_eq!(
            &state.rgba[..3],
            &color::from_coordinates(space, expected).rgb
        );

        let channel = plane.slice.channel();
        let strip = bounds(
            &output,
            &format!("{} color strip", space.channels()[channel].label),
        );
        fixture.click(pos2(strip.center().x, strip.top() + strip.height() * 0.4));
        fixture.settle();
        let moved = fixture.state();
        for axis in plane.axes() {
            assert_eq!(
                moved.values[axis], state.values[axis],
                "Strip must preserve the plane axes in {space:?}"
            );
        }
        assert!((plane.fraction(channel, moved.values[channel]) - 0.6).abs() < 0.005);
        assert_eq!(moved.rgba[3], 99);
    }
}

#[test]
fn rgb_and_linear_plane_pixels_have_the_expected_transfer_function() {
    let mut fixture = Fixture::new([40, 80, 0, 99]);
    fixture.settle();
    for (space, middle_red) in [(Space::Rgb, 128_u8), (Space::LinearRgb, 188_u8)] {
        let output = fixture.choose_space(space);
        let image = fixture.plane_image(&output);
        let top_left = image.pixels[0];
        let top_right = image.pixels[image.size[0] - 1];
        assert_eq!(top_left, Color32::from_rgb(0, 255, 0));
        assert_eq!(top_right, Color32::from_rgb(255, 255, 0));
        let middle = image.pixels[image.size[0] / 2];
        assert!((middle.r() as i16 - middle_red as i16).abs() <= 1);
        assert_eq!((middle.g(), middle.b()), (255, 0));
    }
}

#[test]
fn cmyk_black_strip_changes_the_plane_and_perceptual_pointer_edits_can_be_fitted() {
    let mut fixture = Fixture::new([40, 80, 120, 99]);
    fixture.settle();
    let output = fixture.choose_space(Space::Cmyk);
    let before = fixture.plane_image(&output);
    let inks = fixture.state().values;
    let black = bounds(&output, "Black color strip");
    fixture.click(pos2(black.center().x, black.top() + black.height() / 4.0));
    let output = fixture.settle();
    let after = fixture.state();
    assert_eq!(&after.values[..3], &inks[..3]);
    assert!((after.values[3] - 75.0).abs() < 0.001);
    assert_ne!(fixture.plane_image(&output).pixels, before.pixels);
    assert_eq!(after.rgba[3], 99);

    fixture.choose_space(Space::Oklab);
    fixture.replace_field("Lightness", "0.7");
    let output = fixture.settle();
    let plane = fixture.plane_bounds(&output);
    fixture.click(pos2(
        plane.left() + plane.width() * 0.95,
        plane.top() + plane.height() * 0.05,
    ));
    let output = fixture.settle();
    let requested = fixture.state();
    assert!(!requested.in_gamut);
    assert_eq!(requested.authored, Some((Space::Oklab, requested.values)));
    assert!((requested.values[1] - 0.36).abs() < 0.001);
    assert!((requested.values[2] - 0.36).abs() < 0.001);
    let image = fixture.plane_image(&output);
    let raw = Plane {
        space: Space::Oklab,
        slice: Slice::First,
    }
    .color_at(requested.values, 0.0, 1.0);
    assert!(!raw.in_gamut);
    assert_ne!(
        image.pixels[0],
        Color32::from_rgb(raw.rgb[0], raw.rgb[1], raw.rgb[2]),
        "Unavailable coordinates are visibly hatched, not presented as ordinary in-gamut colors"
    );
    fixture.click(bounds(&output, "Fit to sRGB").center());
    fixture.settle();
    assert!(fixture.state().in_gamut);
    assert_eq!(fixture.app.colors[0][3], 99);
}
