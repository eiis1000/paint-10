use super::*;

fn state(font: FontBytes) -> TextEditState {
    let format = crate::text::TextFormat {
        font_name: "Retained caption font".into(),
        font,
        ..Default::default()
    };
    TextEditState {
        index: None,
        origin: (0, 0),
        text: "0".into(),
        insertion_style: Some(format.default_style()),
        format,
        focus: false,
        selection: 0..0,
        history: TextHistory::default(),
        palette_colors: [BLACK, WHITE],
    }
}

fn padded_font(bytes: usize, identity: u8) -> FontBytes {
    let mut font = epaint_default_fonts::HACK_REGULAR.to_vec();
    font.resize(bytes, identity);
    font.into()
}

#[test]
fn text_history_retains_repeated_caption_edits_with_one_shared_font() {
    let font = padded_font(8 * 1024 * 1024, 0);
    let mut state = state(font.clone());
    for step in 1..=12 {
        let before = TextSnapshot::capture(&state);
        state.text = step.to_string();
        state.origin.0 = step;
        state.history.record(before, step as f64, false);
    }
    assert_eq!(state.history.undo.len(), 12);
    for step in (0..12).rev() {
        let previous = state.history.undo.pop().unwrap();
        previous.restore(&mut state);
        assert_eq!(state.text, step.to_string());
        assert_eq!(state.origin.0, step);
        assert!(state.format.font.shares_storage_with(&font));
        assert!(state
            .insertion_style
            .as_ref()
            .unwrap()
            .font
            .shares_storage_with(&font));
        assert_eq!(state.format.font_name, "Retained caption font");
    }
}

#[test]
fn text_history_budget_still_counts_distinct_font_payloads() {
    let mut state = state(FontBytes::default());
    for step in 0..7 {
        state.text = step.to_string();
        state.format.font = padded_font(6 * 1024 * 1024, step);
        state.insertion_style = Some(state.format.default_style());
        let before = TextSnapshot::capture(&state);
        state.history.record(before, step as f64, false);
    }
    assert_eq!(state.history.undo.len(), 5);
    assert_eq!(state.history.undo.first().unwrap().text, "2");
    assert_eq!(state.history.undo.last().unwrap().text, "6");
    let mut fonts = FontMemory::default();
    let bytes: usize = state
        .history
        .undo
        .iter()
        .map(|snapshot| snapshot.bytes_with_fonts(&mut fonts))
        .sum();
    assert!((30 * 1024 * 1024..32 * 1024 * 1024).contains(&bytes));
}
