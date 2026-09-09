use super::*;

const DEJAVU: &[u8] = DEFAULT_FONT;
const DEVANAGARI: &[u8] = include_bytes!("../../assets/test-fonts/NotoSansDevanagari.ttf");

fn format(font: &[u8]) -> TextFormat {
    TextFormat {
        font: font.into(),
        size: 64.0,
        width: 700,
        ..Default::default()
    }
}

fn names(format: &TextFormat, text: &str) -> Vec<String> {
    let layout = format.layout(text);
    layout
        .lines
        .iter()
        .flat_map(|line| &line.glyphs)
        .map(|glyph| {
            let style = &layout.styles[glyph.style];
            let face =
                rustybuzz::Face::from_slice(style.font.font_data(), style.font_index).unwrap();
            face.glyph_name(rustybuzz::ttf_parser::GlyphId(glyph.id.0))
                .unwrap_or("unnamed")
                .to_owned()
        })
        .collect()
}

#[test]
fn arabic_uses_joined_forms_and_required_lam_alef_ligature() {
    let format = format(DEJAVU);
    assert_eq!(names(&format, "سلام"), ["uni0645", "uniFEFC", "uniFEB3"]);
    let layout = format.layout("سلام");
    let scale = layout.styles[0]
        .font
        .as_scaled(layout.styles[0].size)
        .h_scale_factor();
    let advances: Vec<_> = layout.lines[0]
        .glyphs
        .iter()
        .map(|glyph| (glyph.advance / scale).round() as i32)
        .collect();
    assert_eq!(
        advances,
        [1268, 1222, 1716],
        "independent hb-shape advances"
    );
    assert!(format.render("سلام").pixels().any(|pixel| pixel[3] > 0));
}

#[test]
fn indic_prebase_vowel_and_conjunct_match_harfbuzz_reference() {
    let format = format(DEVANAGARI);
    assert_eq!(
        names(&format, "किरण क्ष"),
        [
            "uni093F.04",
            "uni0915",
            "uni0930",
            "uni0923",
            "space",
            "uni0915094D0937",
        ]
    );
    let editor = format.editor_layout("किरण क्ष");
    assert_eq!(
        editor.horizontal_cursor(0, true),
        2,
        "the pre-base vowel is part of one grapheme"
    );
    assert_eq!(
        editor.horizontal_cursor(5, true),
        8,
        "the conjunct is one editing grapheme"
    );
    assert_eq!(
        editor.rows[0]
            .glyphs
            .iter()
            .map(|glyph| glyph.character)
            .collect::<String>(),
        "किरण क्ष"
    );
}

#[test]
fn combining_marks_compose_and_receive_real_gpos_offsets() {
    let format = format(DEJAVU);
    assert_eq!(format.render("a\u{301}"), format.render("á"));
    let layout = format.layout("x\u{301}");
    assert_eq!(names(&format, "x\u{301}"), ["x", "acutecomb"]);
    let glyphs = &layout.lines[0].glyphs;
    let scale = layout.styles[0]
        .font
        .as_scaled(layout.styles[0].size)
        .h_scale_factor();
    let mark_offset = (glyphs[1].x - glyphs[0].x - glyphs[0].advance) / scale;
    assert!(
        (mark_offset + 90.0).abs() < 0.01,
        "independent hb-shape mark offset: {mark_offset}"
    );
    let editor = format.editor_layout("x\u{301}");
    assert_eq!(editor.rows[0].visual_carets.len(), 2);
    assert_eq!(editor.horizontal_cursor(0, true), 2);
}

#[test]
fn hebrew_visual_carets_and_selection_keep_logical_scalar_indices() {
    let format = format(DEJAVU);
    let editor = format.editor_layout("שלום");
    assert_eq!(
        editor.rows[0]
            .glyphs
            .iter()
            .map(|glyph| glyph.character)
            .collect::<String>(),
        "שלום"
    );
    assert_eq!(
        editor.rows[0]
            .visual_carets
            .iter()
            .map(|caret| caret.index)
            .collect::<Vec<_>>(),
        [4, 3, 2, 1, 0]
    );
    for index in 0..=4 {
        let caret = editor.caret(index);
        assert_eq!(
            editor.hit_test(caret.x, caret.y + caret.height / 2.0),
            index
        );
    }
    assert_eq!(editor.horizontal_cursor(0, false), 1);
    assert_eq!(editor.horizontal_cursor(4, true), 3);
    let selection = editor.selection_rects(1..3);
    assert_eq!(selection.len(), 1);
    assert!(selection[0].width > 0.0);
    let mixed = format.editor_layout("ab שלום cd");
    assert!(
        mixed.selection_rects(1..5).len() > 1,
        "mixed-direction logical selection has disjoint visual intervals"
    );
}

#[test]
fn complex_wrapping_never_splits_graphemes_and_retains_paragraph_direction() {
    use unicode_segmentation::UnicodeSegmentation;
    for (font, text) in [
        (DEJAVU, "سلام سلام سلام"),
        (DEVANAGARI, "किरण क्ष किरण क्ष"),
        (DEJAVU, "x\u{301}x\u{301}x\u{301}"),
    ] {
        let mut format = format(font);
        format.width = 45;
        let editor = format.editor_layout(text);
        let mut boundaries = vec![0];
        let mut count = 0;
        for grapheme in text.graphemes(true) {
            count += grapheme.chars().count();
            boundaries.push(count);
        }
        assert!(editor.rows.len() > 1);
        for row in &editor.rows {
            assert!(boundaries.contains(&row.source_range.start));
            assert!(boundaries.contains(&row.source_range.end));
        }
        assert_eq!(
            editor
                .rows
                .iter()
                .flat_map(|row| row.glyphs.iter().map(|glyph| glyph.character))
                .collect::<String>(),
            text
        );
    }
}

#[test]
fn default_ignorables_are_invisible_and_do_not_become_question_marks() {
    let format = format(DEJAVU);
    for text in [
        "\u{200d}",
        "\u{fe0f}",
        "\u{2066}\u{2069}",
        "\u{115f}",
        "\u{3164}",
        "\u{1bca0}",
    ] {
        assert!(
            format.render(text).pixels().all(|pixel| pixel[3] == 0),
            "{text:?}"
        );
        assert_eq!(
            format.editor_layout(text).rows[0].glyphs.len(),
            text.chars().count()
        );
    }
}

#[test]
fn shaping_keeps_fallback_fonts_styles_and_serialized_metadata_unchanged() {
    let text = "ABC سلام क्ष";
    let mut format = TextFormat {
        size: 48.0,
        width: 700,
        font_faces: vec![
            EmbeddedFont {
                family: "Arabic fallback".into(),
                data: DEJAVU.into(),
                index: 0,
                bold: false,
                italic: false,
            },
            EmbeddedFont {
                family: "Indic fallback".into(),
                data: DEVANAGARI.into(),
                index: 0,
                bold: false,
                italic: false,
            },
        ],
        ..Default::default()
    };
    format
        .modify_style(4..8, |style| style.color = [200, 20, 80, 255])
        .unwrap();
    let saved = serde_json::to_vec(&format).unwrap();
    let raster = format.render(text);
    assert!(raster.pixels().any(|pixel| pixel[0] == 200 && pixel[3] > 0));
    assert!(names(&format, text).contains(&"uniFEFC".to_owned()));
    assert!(names(&format, text).contains(&"uni0915094D0937".to_owned()));
    assert_eq!(serde_json::to_vec(&format).unwrap(), saved);
}

#[test]
fn every_mixed_direction_stop_is_reachable_without_navigation_loops() {
    for text in ["abc שלום def", "abc (שלום) 123 def", "שלום 123 (abc) סוף"] {
        let format = format(DEJAVU);
        let editor = format.editor_layout(text);
        let stops = &editor.rows[0].visual_carets;
        let mut visited = Vec::new();
        let mut current = stops[0].index;
        for stop in stops {
            assert_eq!(current, stop.index, "{text:?}");
            visited.push(current);
            let hit = editor.hit_test(stop.x, stop.y + stop.height / 2.0);
            assert!((editor.caret(hit).x - stop.x).abs() < 0.001);
            let same_position = stops
                .iter()
                .filter(|other| (other.x - stop.x).abs() < 0.001)
                .count();
            if same_position == 1 {
                assert_eq!(hit, stop.index);
            }
            let next = editor.horizontal_cursor(current, true);
            if next != current {
                assert_eq!(editor.horizontal_cursor(next, false), current);
            }
            current = next;
        }
        visited.sort_unstable();
        assert_eq!(visited, (0..=text.chars().count()).collect::<Vec<_>>());
        let selected = editor.selection_rects(0..text.chars().count());
        assert!(selected.iter().all(|rect| rect.width > 0.0));
        let selected_width: f32 = selected.iter().map(|rect| rect.width).sum();
        assert!((selected_width - editor.rows[0].width).abs() < 1.0);
    }
}

#[test]
fn wrapped_ltr_and_rtl_arrows_cross_shared_boundaries_in_both_directions() {
    for (text, forward) in [("abcdefghij", true), ("שששששששששש", false)] {
        let mut format = format(DEJAVU);
        format.width = 80;
        let editor = format.editor_layout(text);
        assert!(editor.rows.len() > 1);
        let mut current = 0;
        for expected in 1..=text.chars().count() {
            let next = editor.horizontal_cursor(current, forward);
            assert_eq!(next, expected, "{text:?}");
            assert_eq!(editor.horizontal_cursor(next, !forward), current);
            current = next;
        }
        assert_eq!(editor.horizontal_cursor(current, forward), current);
    }
}

#[test]
fn contextual_line_shapes_fit_or_contain_one_indivisible_cluster() {
    let text = "سسسلامسلامسسسلامسلامسسسسسس";
    for width in [35, 48, 61, 83, 125] {
        let mut format = format(DEJAVU);
        format.width = width;
        let layout = format.layout(text);
        for line in &layout.lines {
            let mut clusters: Vec<_> = line
                .cells
                .iter()
                .map(|cell| cell.wrap_cluster.clone())
                .collect();
            clusters.dedup();
            assert!(
                line.width <= format.content_width() as f32 + 0.01 || clusters.len() == 1,
                "width {width}, line width {}, clusters {clusters:?}",
                line.width
            );
        }
    }
}

#[test]
fn shaping_uses_the_saved_font_collection_face_index() {
    // Build a valid two-face collection without depending on host font tools.
    let fonts = [epaint_default_fonts::HACK_REGULAR, DEJAVU];
    let mut collection = b"ttcf\0\x01\0\0\0\0\0\x02".to_vec();
    collection.resize(20, 0);
    for (face_index, font) in fonts.into_iter().enumerate() {
        while !collection.len().is_multiple_of(4) {
            collection.push(0);
        }
        let start = collection.len();
        collection[12 + face_index * 4..16 + face_index * 4]
            .copy_from_slice(&(start as u32).to_be_bytes());
        collection.extend_from_slice(font);
        let tables = u16::from_be_bytes([font[4], font[5]]) as usize;
        for table in 0..tables {
            let offset = start + 12 + table * 16 + 8;
            let original = u32::from_be_bytes(collection[offset..offset + 4].try_into().unwrap());
            collection[offset..offset + 4]
                .copy_from_slice(&(original + start as u32).to_be_bytes());
        }
    }
    let mut actual = format(&collection);
    actual.font_index = 1;
    actual.validate().unwrap();
    assert_eq!(
        actual.render("سلام x\u{301}"),
        format(DEJAVU).render("سلام x\u{301}")
    );
}

#[test]
fn grapheme_fallback_shapes_the_base_and_mark_together() {
    let base = FontRef::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT).unwrap();
    let fallback = FontRef::try_from_slice(DEJAVU).unwrap();
    let mark = (0x300..=0x36f)
        .filter_map(char::from_u32)
        .find(|&character| {
            !font_supports_outline(&base, character) && font_supports_outline(&fallback, character)
        })
        .expect("the fixture has a combining mark absent from Ubuntu");
    let text = format!("A{mark}");
    let actual = TextFormat {
        size: 64.0,
        width: 700,
        font_faces: vec![EmbeddedFont {
            family: "DejaVu fallback".into(),
            data: DEJAVU.into(),
            index: 0,
            bold: false,
            italic: false,
        }],
        ..Default::default()
    };
    assert_eq!(actual.render(&text), format(DEJAVU).render(&text));
}

#[test]
fn missing_font_ink_does_not_erase_the_original_text_direction() {
    let format = TextFormat::default();
    let editor = format.editor_layout("שלום");
    assert!(editor.rows[0].right_to_left);
    assert_eq!(
        editor.rows[0]
            .visual_carets
            .iter()
            .map(|caret| caret.index)
            .collect::<Vec<_>>(),
        [4, 3, 2, 1, 0]
    );
    assert_eq!(editor.horizontal_cursor(0, false), 1);
    assert!(format.render("שלום").pixels().any(|pixel| pixel[3] > 0));
}
