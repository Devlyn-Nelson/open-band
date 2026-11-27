use super::*;

const REFERENCE_PAGE_SIZE: usize = 3;

/// Stack reference rows immediately above the focused score, independent of window height. AI_CODE
pub(super) fn reference_row_y(focused_top: f32, index: usize, count: usize) -> f32 {
    focused_top + 24.0 + count.saturating_sub(index + 1) as f32 * 24.0
}

/// Reference pages keep the same staff gap even with fewer rows or a taller viewport. AI_CODE
#[test]
fn reference_rows_stay_adjacent_to_focused_score() {
    for focused_top in [-100.0, 18.0, 300.0] {
        for count in 1..=3 {
            let bottom = reference_row_y(focused_top, count - 1, count);
            assert_eq!(bottom - focused_top, 24.0);
            for index in 1..count {
                assert_eq!(reference_row_y(focused_top, index - 1, count)
                    - reference_row_y(focused_top, index, count), 24.0);
            }
        }
    }
}

/// A timing-only reference row; pitch, fret, and editable event sources are omitted. AI_CODE
pub(super) struct ReferenceRow {
    pub(super) name: String,
    pub(super) onsets: Vec<u32>,
}

/// Build bounded reference pages in track order using the focused view's exact tick interval. AI_CODE
pub(super) fn reference_rows(document: &EditorDocument, start: u32, span: u32) -> Vec<ReferenceRow> {
    let others = document.chart.tracks.iter().enumerate()
        .filter(|(index, _)| *index != document.track_focus).collect::<Vec<_>>();
    let pages = others.len().div_ceil(REFERENCE_PAGE_SIZE).max(1);
    let page = document.reference_page % pages;
    others.into_iter().skip(page * REFERENCE_PAGE_SIZE).take(REFERENCE_PAGE_SIZE)
        .map(|(index, track)| {
            let mut onsets = all_track_events(track)
                .filter(|event| !matches!(event.content, NoteContent::Rest)
                    && event.start >= start && u64::from(event.start) < u64::from(start) + u64::from(span))
                .map(|event| event.start).collect::<Vec<_>>();
            onsets.sort_unstable();
            onsets.dedup();
            ReferenceRow {
                name: format!("{}: {}  [{}/{}]", index + 1, track.name.chars().take(24).collect::<String>(), page + 1, pages),
                onsets,
            }
        }).collect()
}

/// Cycle only the timing-reference page without changing focus, selection, or shared position. AI_CODE
pub(super) fn cycle_reference_page(document: &mut EditorDocument, direction: i32) {
    let pages = document.chart.tracks.len().saturating_sub(1).div_ceil(REFERENCE_PAGE_SIZE).max(1);
    document.reference_page = (document.reference_page as i64 + i64::from(direction)).rem_euclid(pages as i64) as usize;
}

/// Reference pages exclude the focused track and rests, include nested notes, and share tick clipping. AI_CODE
#[test]
fn reference_rows_follow_focus_time_and_pages() {
    let mut document = new_chart_document();
    for index in 0..6 {
        let mut track = new_track(InstrumentKind::Voice);
        track.name = format!("Track {index}");
        track.notes = serde_json::from_str(r#"[
            {"start":0,"length":4,"note":60},
            {"start":960,"length":4,"note":60},
            {"start":960,"length":4,"note":64},
            {"start":1200,"length":8,"rest":true},
            {"start":1920,"length":4,"note":67}
        ]"#).unwrap();
        track.phrases = serde_json::from_str(r#"[{"start":0,"duration_ticks":1920,"text":"la","notes":[{"start":1440,"length":8,"note":62}]}]"#).unwrap();
        document.chart.tracks.push(track);
    }
    document.track_focus = 1;
    document.current_tick = 960;
    document.dirty = false;
    document.checkpoint();
    let rows = reference_rows(&document, 960, 960);
    assert_eq!(rows.len(), 3);
    assert!(rows[0].name.contains("Track 0"));
    assert!(rows[1].name.contains("Track 2"));
    assert!(rows.iter().all(|row| row.onsets == vec![960, 1440]));
    cycle_reference_page(&mut document, 1);
    assert_eq!(reference_rows(&document, 960, 960).len(), 2);
    cycle_reference_page(&mut document, 1);
    assert_eq!(document.reference_page, 0);
    cycle_reference_page(&mut document, -1);
    assert_eq!(document.reference_page, 1);
    document.checkpoint();
    assert_eq!(document.current_tick, 960);
    assert_eq!(document.track_focus, 1);
    assert!(!document.dirty);
    assert!(document.history.undo.is_empty());
    document.chart.tracks.truncate(1);
    document.track_focus = 0;
    assert!(reference_rows(&document, 0, 960).is_empty());
}

/// Reference shortcuts page context instead of switching the editable track in either view. AI_CODE
#[test]
fn reference_keyboard_paging_preserves_editor_context() {
    for view in [EditorViewMode::Sheet, EditorViewMode::Tab] {
        let mut document = new_chart_document();
        document.chart.tracks = (0..5).map(|_| new_track(InstrumentKind::Voice)).collect();
        document.view_mode = view;
        document.track_focus = 1;
        document.current_tick = 1920;
        document.selection = Some(super::bulk::TabSelection { track: 1, anchor: 1920, end: 2880 });
        document.dirty = false;
        let mut app = App::new();
        app.add_message::<KeyboardInput>()
            .insert_resource(document)
            .insert_resource(EditorLibraries { tunings: Vec::new(), kits: Vec::new() })
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, editor_tracks_input);
        for (key, page) in [(KeyCode::PageDown, 1), (KeyCode::PageUp, 0)] {
            let mut keys = ButtonInput::default();
            keys.press(KeyCode::ControlLeft);
            keys.press(key);
            app.insert_resource(keys);
            app.update();
            let document = app.world().resource::<EditorDocument>();
            assert_eq!(document.reference_page, page);
            assert_eq!(document.track_focus, 1);
            assert_eq!(document.current_tick, 1920);
            assert_eq!(document.selection.unwrap().anchor, 1920);
            assert_eq!(document.view_mode, view);
            assert!(!document.dirty);
        }
    }
}