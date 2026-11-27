use super::*;
use bevy::input::mouse::MouseWheel;
use bevy::ui_widgets::Activate;

const TAB_TOP_SCREEN_MARGIN: f32 = 300.0;
const TAB_BOTTOM_MARGIN: f32 = 24.0;
const TAB_REFERENCE_WINDOW_HEIGHT: f32 = 720.0;
const TAB_REFERENCE_HEIGHT: f32 =
    TAB_REFERENCE_WINDOW_HEIGHT - TAB_TOP_SCREEN_MARGIN - TAB_BOTTOM_MARGIN;

/// Keep editable strings left of the divider and timing-only references on the right. AI_CODE
fn tab_horizontal_bounds(width: f32) -> (f32, f32, f32, f32) {
    (-width / 2.0 + 100.0, -40.0, 40.0, width / 2.0 - 30.0)
}

/// Keep the tab start below editor controls while using the remaining vertical screen space.
///
/// AI_CODE
fn tab_viewport_bounds(window_height: f32) -> (f32, f32) {
    let bottom = -window_height / 2.0 + TAB_BOTTOM_MARGIN;
    let preferred_top = window_height / 2.0 - TAB_TOP_SCREEN_MARGIN;
    (preferred_top.max(bottom + 80.0), bottom)
}

/// A persistent non-note label in the vertical tab view.
///
/// AI_CODE
#[derive(Component)]
pub(crate) struct TabSymbolLabel(usize);

type TabLabelQuery<'world, 'state> = Query<
    'world,
    'state,
    (
        Entity,
        &'static TabSymbolLabel,
        &'static mut Text2d,
        &'static mut Transform,
        &'static mut TextColor,
    ),
>;

/// A note badge drawn in UI space so gizmo staff lines cannot cover its text.
///
/// AI_CODE
#[derive(Component)]
pub(crate) struct TabNoteBadge(usize);

/// Identify continuation symbols in explicit and legacy tie chains. AI_CODE
fn tab_tie_continuation(layout: &crate::notation::NotationLayout, index: usize) -> bool {
    layout.ties.iter().any(|chain| chain.iter().skip(1).any(|symbol| *symbol == index))
}

/// Tie starts retain fret labels; every subsequent symbol is a continuation. AI_CODE
#[test]
fn tab_tie_continuation_marks_only_chain_followers() {
    let chart: Chart = serde_json::from_str(r#"{
        "version":1,"title":"Tab ties","resolution":960,
        "tempo_map":[{"start":0,"bpm":100}],
        "time_signature_map":[{"start":0,"numerator":4,"denominator":4}],
        "tracks":[{"name":"Bass","kind":"strings","notes":[
            {"id":"a","start":0,"length":4,"note":40},
            {"id":"b","start":960,"length":4,"note":40},
            {"id":"c","start":1920,"length":4,"note":40},
            {"id":"d","start":2880,"length":4,"note":40}
        ],"ties":[{"from":"a","to":"b"},{"from":"b","to":"c"}]}]
    }"#).unwrap();
    let layout = layout_track(&chart, &chart.tracks[0]);
    assert!(!tab_tie_continuation(&layout, 0));
    assert!(tab_tie_continuation(&layout, 1));
    assert!(tab_tie_continuation(&layout, 2));
    assert!(!tab_tie_continuation(&layout, 3));
    let mut legacy = chart;
    legacy.tracks[0].ties.clear();
    legacy.tracks[0].notes[0].tied = true;
    let layout = layout_track(&legacy, &legacy.tracks[0]);
    assert!(!tab_tie_continuation(&layout, 0));
    assert!(tab_tie_continuation(&layout, 1));
    assert!(!tab_tie_continuation(&layout, 2));
}

type TabNoteBadgeQuery<'world, 'state> = Query<
    'world,
    'state,
    (
        Entity,
        &'static TabNoteBadge,
        &'static mut Node,
        &'static mut Text,
    ),
>;

/// Map a tick in the active tab window to a top-to-bottom coordinate.
///
/// AI_CODE
fn tab_y(tick: u32, start: u32, span: u32, top: f32, bottom: f32) -> Option<f32> {
    let offset = tick.checked_sub(start)?;
    if offset >= span || span == 0 {
        return None;
    }
    Some(top - (top - bottom) * offset as f32 / span as f32)
}

/// Clip a note's duration endpoint to the bottom of the visible tab window.
///
/// AI_CODE
fn tab_duration_end_y(start: u32, duration: u32, view_start: u32, span: u32, top: f32, bottom: f32) -> Option<f32> {
    tab_y(start, view_start, span, top, bottom)?;
    if duration == 0 {
        return None;
    }
    Some(tab_y(start.saturating_add(duration), view_start, span, top, bottom).unwrap_or(bottom))
}

/// Compute the visible tick span at the selected density for the available vertical space.
///
/// AI_CODE
fn tab_visible_span(resolution: u32, beats_at_reference_height: u8, height: f32) -> u32 {
    let (top, bottom) = tab_viewport_bounds(height);
    let available_height = (top - bottom).max(80.0);
    let ticks = f64::from(resolution.max(1))
        * f64::from(beats_at_reference_height.max(2))
        * f64::from(available_height / TAB_REFERENCE_HEIGHT);
    ticks.round().clamp(1.0, f64::from(u32::MAX)) as u32
}

/// A horizontal tab guide; only meter beats carry left-side tick labels.
///
/// AI_CODE
struct TabBeatGuide {
    tick: u32,
    color: Color,
    label: bool,
}

/// Build meter-aware beat and unlabeled subdivision guides for the visible tab window.
///
/// AI_CODE
fn tab_beat_guides(chart: &Chart, start: u32, span: u32, divisions: u8) -> Vec<TabBeatGuide> {
    let end = start.saturating_add(span);
    let divisions = divisions.clamp(SHEET_BEAT_DIVISIONS_MIN, SHEET_BEAT_DIVISIONS_MAX);
    let mut guides = Vec::new();
    for measure in measures(chart, end) {
        if measure.end_tick <= start {
            continue;
        }
        let beat_ticks = (u64::from(chart.resolution.max(1)) * 4
            / u64::from(measure.denominator.max(1))).max(1);
        for beat_index in 0..measure.numerator.max(1) {
            let beat_start = u64::from(measure.start_tick) + u64::from(beat_index) * beat_ticks;
            if beat_start >= u64::from(measure.end_tick) || beat_start >= u64::from(end) {
                break;
            }
            let beat_start = beat_start as u32;
            if beat_start >= start {
                guides.push(TabBeatGuide {
                    tick: beat_start,
                    color: if beat_index == 0 { Color::srgb(0.78, 0.78, 0.78) }
                        else { Color::srgb(0.38, 0.38, 0.38) },
                    label: true,
                });
            }
            for (index, tick) in super::sheet::beat_subdivision_ticks(
                beat_start, beat_ticks.min(u64::from(u32::MAX)) as u32, divisions,
            ).into_iter().enumerate() {
                if tick >= start && tick < end && tick < measure.end_tick {
                    guides.push(TabBeatGuide {
                        tick,
                        color: super::sheet::subdivision_guide_color(index as u8 + 1, divisions),
                        label: false,
                    });
                }
            }
        }
    }
    guides
}

/// Place a tab event at a selected string and fret, retaining written pitch separately
/// from the optional preferred-string performance hint.
///
/// AI_CODE
fn tab_place_string(
    document: &mut EditorDocument,
    tools: &SheetTools,
    tick: u32,
    string: usize,
    fret: u8,
) -> bool {
    if tools.rest {
        return super::sheet_edit_spelled_pitch_at_tick(document, tick, 0, None, tools);
    }
    let Some(track) = document.chart.tracks.get(document.track_focus) else {
        return false;
    };
    let Some(tuning) = track.tuning.as_ref() else {
        return false;
    };
    let Ok(open_notes) = tuning.open_midi() else {
        return false;
    };
    let Some(open) = open_notes.get(string) else {
        return false;
    };
    let pitch = i16::from(*open) + i16::from(fret) + i16::from(track.capo.unwrap_or(0));
    if !(0..=127).contains(&pitch) {
        return false;
    }
    let added = super::sheet_edit_spelled_pitch_at_tick(document, tick, pitch as u8, None, tools);
    if !added || tools.remove {
        return added;
    }
    let Some(track) = document.chart.tracks.get_mut(document.track_focus) else {
        return false;
    };
    if let Some(event) = track.notes.iter_mut().rev().find(|event| {
        event.start == tick
            && event.voice == tools.voice
            && matches!(event.content, NoteContent::Pitched { note, .. } if note == pitch as u8)
            && event.performance.is_none()
    }) {
        event.performance = Some(PerformanceHints {
            preferred_string: Some(string),
            attack: None,
            transition: None,
            bend: None,
            motion: None,
            percussion_dynamics: None,
            roll: None,
            droll: None,
            percussion_grace: None,
            percussion_technique: None,
            sticking: None,
            hi_hat: None,
            harmonic: None,
            palm_mute: false,
            vibrato: None,
        });
    }
    true
}

/// Consume tie placement before endpoint resizing, deriving the continuation from its lane predecessor. AI_CODE
fn tab_place_tie(document: &mut EditorDocument, tools: &SheetTools, tick: u32, lane: usize) -> bool {
    if !tools.tied || tools.rest || tools.remove { return false; }
    document.tab_resize = None;
    document.tab_fret_string = None;
    document.tab_percussion_lane = None;
    let Some(track) = document.chart.tracks.get(document.track_focus) else { return true };
    let layout = layout_track(&document.chart, track);
    let predecessor = layout.symbols.iter().filter(|symbol| {
        !matches!(symbol.kind, NotationSymbolKind::Rest)
            && symbol.voice == tools.voice
            && symbol.start < tick
            && symbol.start.checked_add(symbol.duration) == Some(tick)
            && tab_lane_for_symbol(track, symbol) == Some(lane)
    }).max_by_key(|symbol| symbol.start);
    let Some(predecessor) = predecessor else {
        document.status = "No preceding note ends here on this lane".into();
        return true;
    };
    let source = predecessor.source;
    let Some(mut continuation) = super::editor_source_event(track, source).cloned() else { return true };
    if layout.symbols.iter().any(|symbol| symbol.start == tick
        && symbol.voice == continuation.voice && symbol.staff == continuation.staff
        && tab_lane_for_symbol(track, symbol) == Some(lane))
    {
        document.status = "A note or rest already occupies this position".into();
        return true;
    }
    continuation.id = None;
    continuation.start = tick;
    continuation.length = tools.duration;
    continuation.dots = tools.dots;
    continuation.tuplet = tools.tuplet;
    continuation.tied = false;
    continuation.chord = None;
    document.chart.tracks[document.track_focus].notes.push(continuation);
    ensure_stable_event_ids(&mut document.chart);
    let track = &mut document.chart.tracks[document.track_focus];
    let from = super::editor_source_event(track, source).unwrap().id.clone().unwrap();
    let to = track.notes.last().unwrap().id.clone().unwrap();
    track.ties.push(EventRelation { from, to });
    document.dirty = true;
    document.status = "Tied continuation placed".into();
    true
}

/// Choose the longest dotted note symbol that fits a snapped duration.
///
/// AI_CODE
fn duration_spelling(duration: u32, resolution: u32) -> (NoteValue, u8) {
    let values = [
        NoteValue::Whole,
        NoteValue::Half,
        NoteValue::Quarter,
        NoteValue::Eighth,
        NoteValue::Sixteenth,
    ];
    values
        .into_iter()
        .flat_map(|value| (0..=2).map(move |dots| (value, dots)))
        .filter_map(|(value, dots)| {
            let mut ticks = value.ticks(resolution);
            let mut addition = ticks;
            for _ in 0..dots {
                addition /= 2;
                ticks = ticks.saturating_add(addition);
            }
            (ticks <= duration).then_some((value, dots, ticks))
        })
        .max_by_key(|(_, _, ticks)| *ticks)
        .map_or((NoteValue::Sixteenth, 0), |(value, dots, _)| (value, dots))
}

/// Resolve the string/lane occupied by a laid-out event for resize collision checks.
///
/// AI_CODE
fn tab_lane_for_symbol(track: &ChartTrack, symbol: &NotationSymbol) -> Option<usize> {
    match &symbol.kind {
        NotationSymbolKind::Percussion {
            lane,
            lane_span: Some(_),
            ..
        } => Some(usize::MAX),
        NotationSymbolKind::Percussion { lane, .. } => *lane,
        NotationSymbolKind::Pitched { .. } if track.kind == InstrumentKind::Voice => Some(0),
        NotationSymbolKind::Pitched { pitch, .. } => {
            let tuning = track.tuning.as_ref()?;
            let open_notes = tuning.open_midi().ok()?;
            let preferred = super::editor_source_event(track, symbol.source)
                .and_then(|event| event.performance.as_ref())
                .and_then(|hint| hint.preferred_string)
                .filter(|string| open_notes.get(*string).is_some_and(|open| open <= pitch));
            preferred.or_else(|| {
                open_notes
                    .iter()
                    .enumerate()
                    .filter(|(_, open)| **open <= *pitch)
                    .min_by_key(|(_, open)| i16::from(*pitch) - i16::from(**open))
                    .map(|(index, _)| index)
            })
        }
        NotationSymbolKind::Rest => Some(0),
    }
}

/// Find the nearest visible event badge on a physical lane without snapping authored time.
///
/// AI_CODE
fn tab_hover_event(
    track: &ChartTrack,
    symbols: &[NotationSymbol],
    lane: usize,
    pointer_y: f32,
    bounds: (u32, u32, f32, f32),
) -> Option<(EventSource, f32)> {
    let (start, span, top, bottom) = bounds;
    symbols.iter()
        .filter(|symbol| tab_lane_for_symbol(track, symbol) == Some(lane))
        .filter_map(|symbol| tab_y(symbol.start, start, span, top, bottom).map(|y| (symbol.source, y)))
        .filter(|(_, y)| (pointer_y - y).abs() <= 18.0)
        .min_by(|(_, left), (_, right)| (pointer_y - left).abs().total_cmp(&(pointer_y - right).abs()))
        .map(|(source, y)| (source, y))
}

/// Resize an authored tab event, capped at the next event on the same physical lane.
///
/// AI_CODE
fn resize_tab_event(
    document: &mut EditorDocument,
    source: EventSource,
    requested_end: u32,
    lane: usize,
    single_symbol_limit: u32,
) -> bool {
    let Some(track) = document.chart.tracks.get(document.track_focus) else {
        return false;
    };
    let layout = layout_track(&document.chart, track);
    let Some(symbol) = layout.symbols.iter().find(|symbol| symbol.source == source) else {
        return false;
    };
    let next_start = layout
        .symbols
        .iter()
        .filter(|candidate| {
            candidate.start > symbol.start && tab_lane_for_symbol(track, candidate) == Some(lane)
        })
        .map(|candidate| candidate.start)
        .min();
    let minimum = document
        .snap_interval
        .ticks(document.chart.resolution.max(1))
        .max(1);
    let end = next_start.map_or(requested_end, |next| requested_end.min(next));
    let duration = end.saturating_sub(symbol.start).max(minimum);
    let (length, dots) = duration_spelling(duration, document.chart.resolution.max(1));
    let tie_required = duration > single_symbol_limit;
    let Some(track) = document.chart.tracks.get_mut(document.track_focus) else {
        return false;
    };
    let Some(event) = super::editor_source_event_mut(track, source) else {
        return false;
    };
    event.length = length;
    event.dots = dots;
    event.tuplet = None;
    event.tied = tie_required;
    document.dirty = true;
    true
}

/// Tab duration changes undo and redo with their notation and automatic tie flag intact. AI_CODE
#[test]
fn editor_history_restores_tab_duration_edits() {
    let mut document = new_chart_document();
    ensure_editor_has_track(&mut document);
    let tools = SheetTools::default();
    assert!(tab_place_string(&mut document, &tools, 0, 0, 3));
    document.checkpoint();
    let original = serde_json::to_value(&document.chart).unwrap();
    let resolution = document.chart.resolution;
    assert!(resize_tab_event(&mut document, EventSource::Track(0), resolution * 2, 0, resolution));
    document.checkpoint();
    let resized = serde_json::to_value(&document.chart).unwrap();
    assert_ne!(resized, original);
    assert!(document.chart.tracks[0].notes[0].tied);
    document.restore_history(false);
    assert_eq!(serde_json::to_value(&document.chart).unwrap(), original);
    document.restore_history(true);
    assert_eq!(serde_json::to_value(&document.chart).unwrap(), resized);
}

/// Apply fret-grid clicks, show each choice's resulting pitch, and close the dialog after use.
///
/// AI_CODE
pub(crate) fn tab_fret_dialog_system(
    mut document: ResMut<EditorDocument>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut choices: Query<(&TabFretChoice, &mut Text, &mut Visibility)>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        document.tab_fret_string = None;
    }
    if document.view_mode != EditorViewMode::Tab || document.metadata_mode
        || document.inspector.is_some() || document.text_entry.is_some() || document.bulk_edit.is_some()
    {
        document.tab_fret_string = None;
    }
    let open = document.view_mode == EditorViewMode::Tab
        && !document.metadata_mode
        && document.tab_fret_string.is_some();
    let Some(string) = document.tab_fret_string else {
        for (_, _, mut visibility) in &mut choices {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let Some(track) = document.chart.tracks.get(document.track_focus) else {
        document.tab_fret_string = None;
        return;
    };
    let Some(tuning) = track.tuning.as_ref() else {
        document.tab_fret_string = None;
        return;
    };
    let Ok(open_notes) = tuning.open_midi() else {
        document.tab_fret_string = None;
        return;
    };
    let Some(open_pitch) = open_notes.get(string).copied() else {
        document.tab_fret_string = None;
        return;
    };
    let capo = track.capo.unwrap_or(0);
    for (choice, mut text, mut visibility) in &mut choices {
        let pitch = i16::from(open_pitch) + i16::from(choice.0) + i16::from(capo);
        text.0 = format!("{}  {}", choice.0, note_name(pitch as u8));
        *visibility = if open && pitch <= 127 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Place a string note from an activated fret-choice button.
///
/// AI_CODE
pub(crate) fn tab_fret_choice_input(
    activate: On<Activate>,
    mut document: ResMut<EditorDocument>,
    tools: Res<SheetTools>,
    choices: Query<&TabFretChoice>,
) {
    if document.view_mode != EditorViewMode::Tab || document.metadata_mode
        || document.inspector.is_some() || document.text_entry.is_some() || document.bulk_edit.is_some()
    {
        return;
    }
    let Some(string) = document.tab_fret_string else {
        return;
    };
    let Ok(choice) = choices.get(activate.entity) else {
        return;
    };
    let tick = document.tab_fret_tick;
    tab_place_string(&mut document, &tools, tick, string, choice.0);
    document.tab_fret_string = None;
}

/// Format MIDI pitch as a compact note name for the fret-choice dialog.
///
/// AI_CODE
fn note_name(midi: u8) -> &'static str {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    NAMES[(midi % 12) as usize]
}

/// Render and edit the focused track as a vertical, top-to-bottom tab view.
///
/// AI_CODE
pub(crate) fn tab_view_system(
    mut commands: Commands,
    mut gizmos: Gizmos,
    mut document: ResMut<EditorDocument>,
    mut tools: ResMut<SheetTools>,
    settings: Res<PersistentSettings>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<MouseWheel>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut labels: TabLabelQuery,
    mut note_badges: TabNoteBadgeQuery,
) {
    if document.view_mode != EditorViewMode::Tab
        || document.metadata_mode
        || document.text_entry.is_some()
        || document.inspector.is_some()
        || document.bulk_edit.is_some()
        || document.tab_fret_string.is_some()
    {
        document.tab_resize = None;
        for (entity, _, _, _, _) in labels.iter() {
            commands.entity(entity).despawn();
        }
        for (entity, _, _, _) in note_badges.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }
    if keyboard.just_pressed(KeyCode::F10) {
        tools.remove = !tools.remove;
    }
    if tools.remove {
        document.tab_resize = None;
    }
    let Ok(window) = windows.single() else { return };
    let resolution = document.chart.resolution.max(1);
    let (top, bottom) = tab_viewport_bounds(window.height());
    let viewport_height = top - bottom;
    let span = tab_visible_span(resolution, document.tab_beats_per_screen, window.height());
    for event in wheel.read() {
        let direction = match event.y.total_cmp(&0.0) {
            std::cmp::Ordering::Greater => -1i64,
            std::cmp::Ordering::Less => 1i64,
            std::cmp::Ordering::Equal => 0,
        };
        let step = i64::from(snap_editor_tick(&document, resolution));
        document.current_tick = (i64::from(document.current_tick) + direction * step)
            .clamp(0, i64::from(u32::MAX)) as u32;
    }
    document.follow_preview(span);
            let view_start = document.current_tick;
            let y_for_tick = |tick| tab_y(tick, view_start, span, top, bottom);
    if keyboard.just_pressed(KeyCode::Equal) || keyboard.just_pressed(KeyCode::NumpadAdd) {
        document.tab_beats_per_screen = document.tab_beats_per_screen.saturating_sub(1).max(2);
    }
    if keyboard.just_pressed(KeyCode::Minus) || keyboard.just_pressed(KeyCode::NumpadSubtract) {
        document.tab_beats_per_screen = document.tab_beats_per_screen.saturating_add(1).min(32);
    }
    let Some(track) = document.chart.tracks.get(document.track_focus).cloned() else {
        return;
    };
    let layout = layout_track(&document.chart, &track);
    let percussion = track.kind == InstrumentKind::Percussion;
    let lanes = if percussion {
        track.kit.as_ref().map_or(0, |kit| kit.lanes)
    } else if track.kind == InstrumentKind::Voice {
        1
    } else {
        track
            .tuning
            .as_ref()
            .map_or(0, |tuning| tuning.strings.len())
    };
    let (left, right, reference_left, reference_right) = tab_horizontal_bounds(window.width());
    if right <= left || reference_right <= reference_left { return; }
    let x_for_lane = |lane: usize| {
        if lanes <= 1 {
            (left + right) / 2.0
        } else {
            left + (right - left) * lane as f32 / (lanes - 1) as f32
        }
    };
    let mut next_labels = Vec::<(String, f32, f32, Color)>::new();
    let mut next_badges = Vec::<(String, f32, f32, bool)>::new();
    let reference_rows = super::references::reference_rows(&document, view_start, span);
    gizmos.line_2d(Vec2::new(0.0, top + 32.0), Vec2::new(0.0, bottom), Color::srgb(0.35, 0.43, 0.45));
    for (index, row) in reference_rows.iter().enumerate() {
        let column_width = (reference_right - reference_left) / reference_rows.len() as f32;
        let x = reference_left + column_width * (index as f32 + 0.5);
        let color = Color::srgb(0.55, 0.70, 0.76);
        gizmos.line_2d(Vec2::new(x, top), Vec2::new(x, bottom), Color::srgb(0.24, 0.31, 0.33));
        let capacity = ((column_width - 8.0) / 11.0).floor().max(1.0) as usize;
        let heading = row.name.chars().collect::<Vec<_>>().chunks(capacity)
            .map(|chunk| chunk.iter().collect::<String>()).collect::<Vec<_>>().join("\n");
        next_labels.push((heading, x, top + 48.0, color));
        for tick in &row.onsets {
            if let Some(y) = y_for_tick(*tick) {
                gizmos.line_2d(Vec2::new(x - 7.0, y), Vec2::new(x + 7.0, y), color);
                gizmos.circle_2d(Vec2::new(x, y), 3.0, color);
            }
        }
    }
    for lane in 0..lanes {
        let x = x_for_lane(lane);
        gizmos.line_2d(
            Vec2::new(x, top),
            Vec2::new(x, bottom),
            Color::srgb(0.55, 0.60, 0.63),
        );
        let name = if percussion {
            format!("L{}", lane + 1)
        } else if track.kind == InstrumentKind::Voice {
            "VOICE".into()
        } else {
            track
                .tuning
                .as_ref()
                .and_then(|tuning| tuning.open_midi().ok())
                .and_then(|notes| notes.get(lane).copied())
                .map_or_else(
                    || format!("S{}", lane + 1),
                    |midi| note_name(midi).to_owned(),
                )
        };
        next_labels.push((name, x, top + 20.0, Color::srgb(0.92, 0.94, 0.88)));
    }
    if percussion {
        let span_x = left - 48.0;
        gizmos.line_2d(
            Vec2::new(span_x, top),
            Vec2::new(span_x, bottom),
            Color::srgb(0.79, 0.63, 0.31),
        );
        next_labels.push((
            "SPAN".into(),
            span_x,
            top + 20.0,
            Color::srgb(0.92, 0.94, 0.88),
        ));
    }
    for guide in tab_beat_guides(&document.chart, view_start, span, settings.sheet_beat_divisions) {
        if let Some(y) = y_for_tick(guide.tick) {
            gizmos.line_2d(
                Vec2::new(left - 30.0, y),
                Vec2::new(reference_right, y),
                guide.color,
            );
            if guide.label {
                next_labels.push((
                    guide.tick.to_string(),
                    left - 68.0,
                    y,
                    Color::srgb(0.92, 0.94, 0.88),
                ));
            }
        }
    }
    let playhead_tick = if document.playback.playing { document.playback.tick } else { document.current_tick };
    if let Some(y) = y_for_tick(playhead_tick) {
        for offset in -1..=1 {
            gizmos.line_2d(
                Vec2::new(left - 30.0, y + offset as f32),
                Vec2::new(reference_right, y + offset as f32),
                Color::srgb(0.95, 0.73, 0.22),
            );
        }
    }
    let selected = super::bulk::selected_sources(&document);
    if let Some(selection) = document.selection.filter(|selection| selection.track == document.track_focus) {
        let start = selection.anchor.min(selection.end).max(view_start);
        let end = selection.anchor.max(selection.end).min(view_start.saturating_add(span).saturating_sub(1));
        if start <= end {
            let start_y = y_for_tick(start).unwrap_or(top);
            let end_y = y_for_tick(end).unwrap_or(bottom);
            gizmos.rect_2d(
                Vec2::new((left + right) / 2.0, (start_y + end_y) / 2.0),
                Vec2::new(right - left + 60.0, (start_y - end_y).max(4.0)),
                Color::srgb(0.95, 0.73, 0.22),
            );
        }
    }
    for (symbol_index, symbol) in layout.symbols.iter().enumerate() {
        let Some(y) = y_for_tick(symbol.start) else {
            continue;
        };
        let lane = tab_lane_for_symbol(&track, symbol);
        let span_piece = matches!(
            symbol.kind,
            NotationSymbolKind::Percussion {
                lane_span: Some(_),
                ..
            }
        );
        let x = if span_piece {
            left - 48.0
        } else {
            let Some(lane) = lane.filter(|lane| *lane < lanes) else {
                continue;
            };
            x_for_lane(lane)
        };
        let text = match &symbol.kind {
            NotationSymbolKind::Pitched { pitch, .. } if track.kind == InstrumentKind::Voice => note_name(*pitch).to_owned(),
            NotationSymbolKind::Pitched { pitch, .. } => {
                let Some(lane) = lane else { continue };
                track
                    .tuning
                    .as_ref()
                    .and_then(|tuning| tuning.open_midi().ok())
                    .and_then(|open| open.get(lane).copied())
                    .map_or_else(
                        || "?".into(),
                        |open| {
                            (i16::from(*pitch)
                                - i16::from(open)
                                - i16::from(track.capo.unwrap_or(0)))
                            .to_string()
                        },
                    )
            }
            NotationSymbolKind::Percussion {
                piece,
                symbol,
                lane_span,
                ..
            } => {
                if lane_span.is_some() {
                    format!("{} {piece}", symbol.as_deref().unwrap_or("x"))
                } else {
                    format!("{} {piece}", symbol.as_deref().unwrap_or("x"))
                }
            }
            NotationSymbolKind::Rest => "REST".into(),
        };
        let continuation = tab_tie_continuation(&layout, symbol_index);
        if selected.contains(&symbol.source) {
            gizmos.circle_2d(Vec2::new(x, y), if continuation { 13.0 } else { 23.0 }, Color::srgb(0.95, 0.73, 0.22));
        }
        if let Some(tick) = super::bulk::move_preview_tick(&document, symbol.source, symbol.start)
            && let Some(ghost_y) = y_for_tick(tick)
        {
            gizmos.circle_2d(Vec2::new(x, ghost_y), if continuation { 13.0 } else { 23.0 }, Color::srgb(0.35, 0.9, 0.95));
        }
        next_badges.push((if continuation { String::new() } else { text }, x, y, continuation));
        if let Some(end_y) = tab_duration_end_y(symbol.start, symbol.duration, view_start, span, top, bottom) {
            for offset in -3..=3 {
                gizmos.line_2d(
                    Vec2::new(x + offset as f32, y),
                    Vec2::new(x + offset as f32, end_y),
                    Color::srgb(0.16, 0.54, 0.44),
                );
            }
            gizmos.line_2d(
                Vec2::new(x - 6.0, end_y),
                Vec2::new(x + 6.0, end_y),
                Color::srgb(0.16, 0.54, 0.44),
            );
        }
    }
    let existing = labels
        .iter_mut()
        .map(|(entity, marker, _, _, _)| (marker.0, entity))
        .collect::<std::collections::HashMap<_, _>>();
    for (index, (text_value, x, y, color)) in next_labels.iter().enumerate() {
        if let Some(entity) = existing.get(&index)
            && let Ok((_, _, mut text, mut transform, mut text_color)) = labels.get_mut(*entity)
        {
            text.0.clone_from(text_value);
            transform.translation = Vec3::new(*x, *y, 10.0);
            text_color.0 = *color;
        } else {
            commands.spawn((
                Text2d::new(text_value.clone()),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(*color),
                Transform::from_xyz(*x, *y, 10.0),
                TabSymbolLabel(index),
            ));
        }
    }
    for (entity, marker, _, _, _) in labels.iter() {
        if marker.0 >= next_labels.len() {
            commands.entity(entity).despawn();
        }
    }
    let existing_badges = note_badges
        .iter_mut()
        .map(|(entity, marker, _, _)| (marker.0, entity))
        .collect::<std::collections::HashMap<_, _>>();
    for (index, (badge_text, x, y, continuation)) in next_badges.iter().enumerate() {
        let badge_width = if *continuation {
            18.0
        } else {
            (badge_text.chars().count() as f32 * 11.0 + 16.0).max(36.0)
        };
        let badge_height = if *continuation { 18.0 } else { 36.0 };
        let left_px = window.width() / 2.0 + *x - badge_width / 2.0;
        let top_px = window.height() / 2.0 - *y - badge_height / 2.0;
        if let Some(entity) = existing_badges.get(&index)
            && let Ok((_, _, mut node, mut text)) = note_badges.get_mut(*entity)
        {
            node.left = Val::Px(left_px);
            node.top = Val::Px(top_px);
            node.width = Val::Px(badge_width);
            node.height = Val::Px(badge_height);
            text.0.clone_from(badge_text);
        } else {
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(left_px),
                    top: Val::Px(top_px),
                    width: Val::Px(badge_width),
                    height: Val::Px(badge_height),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.16, 0.54, 0.44)),
                Text::new(badge_text.clone()),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                TabNoteBadge(index),
            ));
        }
    }
    for (entity, marker, _, _) in note_badges.iter() {
        if marker.0 >= next_badges.len() {
            commands.entity(entity).despawn();
        }
    }

    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let position = Vec2::new(
        cursor.x - window.width() / 2.0,
        window.height() / 2.0 - cursor.y,
    );
    if position.x > right + 30.0 && !document.selection_drag
        && document.selection_move.is_none() && document.tab_resize.is_none()
    { return; }
    if (position.y > top || position.y < bottom) && !document.selection_drag && document.selection_move.is_none() {
        return;
    }
    let tick = document.current_tick.saturating_add(
        ((top - position.y.clamp(bottom, top)) / viewport_height * span as f32) as u32,
    );
    let snapped = super::sheet::snap_sheet_tick(&document.chart, tick, settings.sheet_beat_divisions);
    if document.selection_move.is_some() {
        super::bulk::update_move(&mut document, snapped,
            mouse.just_released(MouseButton::Left) || !mouse.pressed(MouseButton::Left));
        return;
    }
    if document.selection_drag {
        super::bulk::update_range(&mut document, snapped,
            mouse.just_released(MouseButton::Left) || !mouse.pressed(MouseButton::Left));
        return;
    }
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    if !shift && mouse.just_pressed(MouseButton::Left)
        && position.x >= left - 78.0 && position.x <= right + 30.0
        && super::bulk::begin_move(&mut document, snapped)
    { return; }
    if mouse.just_pressed(MouseButton::Left) && (shift || document.selection_mode)
        && position.x >= left - 78.0 && position.x <= right + 30.0
    {
        super::bulk::begin_range(&mut document, snapped, shift);
        return;
    }
    if mouse.just_released(MouseButton::Left)
        && let Some(resize) = document.tab_resize.take()
    {
        let single_symbol_limit = tools.duration.ticks(document.chart.resolution.max(1));
        resize_tab_event(
            &mut document,
            resize.source,
            snapped,
            resize.lane,
            single_symbol_limit,
        );
        return;
    }
    if tools.rest && !tools.remove {
        if mouse.just_pressed(MouseButton::Left) && position.x >= left - 78.0 && position.x <= right + 30.0 {
            super::sheet_edit_spelled_pitch_at_tick(&mut document, snapped, 0, None, &tools);
        }
        return;
    }
    if percussion && position.x >= left - 78.0 && position.x <= left - 20.0 {
        let hit = tab_hover_event(&track, &layout.symbols, usize::MAX, position.y,
            (view_start, span, top, bottom));
        if tools.remove {
            if let Some((source, y)) = hit {
                gizmos.circle_2d(Vec2::new(left - 48.0, y), 24.0, Color::srgb(0.36, 0.83, 0.68));
                if mouse.just_pressed(MouseButton::Left) {
                    super::remove_editor_event(&mut document, source);
                }
            }
            return;
        }
        if let Some(y) = y_for_tick(snapped) {
            gizmos.circle_2d(Vec2::new(left - 48.0, y), 8.0, Color::srgb(0.36, 0.83, 0.68));
        }
        if document.tab_resize.is_some() {
            if let Some(resize) = document.tab_resize {
                if let Some(y) = y_for_tick(resize.start_tick) {
                    gizmos.line_2d(
                        Vec2::new(left - 48.0, y),
                        Vec2::new(left - 48.0, position.y.clamp(bottom, top)),
                        Color::srgb(0.90, 0.75, 0.34),
                    );
                }
            }
            return;
        }
        if mouse.just_pressed(MouseButton::Left) {
            if tab_place_tie(&mut document, &tools, snapped, usize::MAX) { return; }
            if let Some(symbol) = layout.symbols.iter().find(|symbol| {
                matches!(
                    symbol.kind,
                    NotationSymbolKind::Percussion {
                        lane_span: Some(_),
                        ..
                    }
                ) && y_for_tick(symbol.start.saturating_add(symbol.duration))
                    .is_some_and(|end_y| (position.y - end_y).abs() <= 12.0)
            }) {
                document.tab_resize = Some(TabDurationResize {
                    source: symbol.source,
                    start_tick: symbol.start,
                    lane: usize::MAX,
                });
                return;
            }
            let Some(piece) = track
                .kit
                .as_ref()
                .and_then(|kit| kit.pieces.get(tools.piece_index % kit.pieces.len().max(1)))
            else {
                return;
            };
            if piece.lane_span.is_some() {
                super::sheet_edit_percussion_at_tick(&mut document, snapped, 0, &tools);
            } else {
                document.status = "Cycle PIECE to a lane-spanning kit piece".into();
            }
        }
        return;
    }
    let lane = if lanes <= 1 {
        0
    } else {
        ((position.x - left) / (right - left) * (lanes - 1) as f32).round() as usize
    };
    if lane >= lanes || (position.x - x_for_lane(lane)).abs() > 25.0 {
        return;
    }
    if tools.remove {
        if let Some((source, y)) = tab_hover_event(&track, &layout.symbols, lane, position.y,
            (view_start, span, top, bottom))
        {
            gizmos.circle_2d(Vec2::new(x_for_lane(lane), y), 24.0, Color::srgb(0.36, 0.83, 0.68));
            if mouse.just_pressed(MouseButton::Left) {
                super::remove_editor_event(&mut document, source);
            }
        }
        return;
    }
    if let Some(y) = y_for_tick(snapped) {
        gizmos.circle_2d(Vec2::new(x_for_lane(lane), y), 8.0, Color::srgb(0.36, 0.83, 0.68));
    }
    if document.tab_resize.is_some() {
        let start = document
            .tab_resize
            .map_or(snapped, |resize| resize.start_tick);
        if let Some(y) = y_for_tick(start) {
            gizmos.line_2d(
                Vec2::new(x_for_lane(lane), y),
                Vec2::new(x_for_lane(lane), position.y),
                Color::srgb(0.90, 0.75, 0.34),
            );
        }
        return;
    }
    if mouse.just_pressed(MouseButton::Left) {
        if tab_place_tie(&mut document, &tools, snapped, lane) { return; }
        if let Some(symbol) = layout.symbols.iter().find(|symbol| {
            tab_lane_for_symbol(&track, symbol) == Some(lane)
                && symbol.start.saturating_add(symbol.duration) > symbol.start
                && y_for_tick(symbol.start.saturating_add(symbol.duration))
                    .is_some_and(|end_y| (position.y - end_y).abs() <= 12.0)
        }) {
            document.tab_resize = Some(TabDurationResize {
                source: symbol.source,
                start_tick: symbol.start,
                lane,
            });
            return;
        }
        if percussion {
            let Some(kit) = track.kit.as_ref() else {
                return;
            };
            let pieces = kit
                .pieces
                .iter()
                .enumerate()
                .filter(|(_, piece)| piece.lane == Some(lane))
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            if pieces.len() > 1 {
                if document.tab_percussion_lane != Some(lane) {
                    document.tab_percussion_lane = Some(lane);
                    document.status = format!(
                        "Shared lane: choose a matching piece with PIECE, then click lane {} again",
                        lane + 1
                    );
                    return;
                }
                if !pieces.contains(&tools.piece_index) {
                    document.status = "Selected piece is not on this lane; cycle PIECE".into();
                    return;
                }
            }
            document.tab_percussion_lane = None;
            document.current_tick = snapped;
            super::sheet_edit_percussion_at_tick(&mut document, snapped, lane, &tools);
        } else if track.kind == InstrumentKind::Voice {
            document.current_tick = snapped;
            document.event_focus = 0;
        } else {
            document.tab_fret_string = Some(lane);
            document.tab_fret_tick = snapped;
            document.status = format!("Choose fret for string {}", lane + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Focused lanes stay in the left half while reference ticks share the exact vertical axis. AI_CODE
    #[test]
    fn tab_references_use_right_half_and_shared_time_axis() {
        for width in [640.0, 1280.0, 1920.0] {
            let (left, right, reference_left, reference_right) = tab_horizontal_bounds(width);
            assert!(left < right && right < 0.0);
            assert!(reference_left > 0.0 && reference_left < reference_right);
            assert!(right + 30.0 < reference_left);
        }
        let (top, bottom) = tab_viewport_bounds(900.0);
        let span = tab_visible_span(960, 8, 900.0);
        let note_y = tab_y(1920, 960, span, top, bottom).unwrap();
        let reference_y = tab_y(1920, 960, span, top, bottom).unwrap();
        assert_eq!(note_y, reference_y);
    }

    /// Tie endpoint clicks place implied notes without a fret dialog or a resize transaction. AI_CODE
    #[test]
    fn tab_tie_endpoint_places_implied_continuation() {
        let mut document = new_chart_document();
        ensure_editor_has_track(&mut document);
        let mut tools = SheetTools::default();
        assert!(tab_place_string(&mut document, &tools, 0, 0, 5));
        let source = document.chart.tracks[0].notes[0].clone();
        let end = source.duration_ticks(document.chart.resolution);
        tools.tied = true;
        tools.duration = NoteValue::Eighth;
        tools.dots = 1;
        tools.tuplet = Some(Tuplet { actual: 3, normal: 2 });
        document.checkpoint();
        assert!(tab_place_tie(&mut document, &tools, end, 0));
        assert!(document.tab_resize.is_none());
        assert!(document.tab_fret_string.is_none());
        let track = &document.chart.tracks[0];
        assert_eq!(track.notes.len(), 2);
        let continuation = &track.notes[1];
        assert_eq!(continuation.note(), source.note());
        assert_eq!(continuation.performance, source.performance);
        assert_eq!(continuation.start, end);
        assert_eq!(continuation.length, NoteValue::Eighth);
        assert_eq!(continuation.dots, 1);
        assert_eq!(continuation.duration_ticks(document.chart.resolution), 480);
        assert_ne!(continuation.id, source.id);
        assert_eq!(track.ties.len(), 1);
        assert_eq!(layout_track(&document.chart, track).ties.len(), 1);
        assert!(tab_place_tie(&mut document, &tools, end, 0));
        assert_eq!(document.chart.tracks[0].notes.len(), 2);
        document.checkpoint();
        document.restore_history(false);
        assert_eq!(document.chart.tracks[0].notes.len(), 1);
        assert!(document.chart.tracks[0].ties.is_empty());
    }

    /// Tie mode consumes invalid placements instead of opening a fret picker or resizing. AI_CODE
    #[test]
    fn tab_tie_requires_a_lane_predecessor() {
        let mut document = new_chart_document();
        ensure_editor_has_track(&mut document);
        let mut tools = SheetTools::default();
        tab_place_string(&mut document, &tools, 0, 0, 5);
        document.dirty = false;
        tools.tied = true;
        assert!(tab_place_tie(&mut document, &tools, 960, 1));
        assert!(tab_place_tie(&mut document, &tools, 480, 0));
        assert_eq!(document.chart.tracks[0].notes.len(), 1);
        assert!(!document.dirty);
        tools.tied = false;
        assert!(!tab_place_tie(&mut document, &tools, 960, 0));
    }

    /// Tab insertion uses shared lengths, dots, tuplets, ties, and explicit rest tools. AI_CODE
    #[test]
    fn tab_notation_tools_control_inserted_events() {
        let mut document = new_chart_document();
        ensure_editor_has_track(&mut document);
        let mut tools = SheetTools::default();
        tools.duration = NoteValue::Eighth;
        tools.dots = 1;
        tools.tuplet = Some(Tuplet { actual: 3, normal: 2 });
        tools.tied = true;
        assert!(tab_place_string(&mut document, &tools, 0, 0, 3));
        let duration = document.chart.tracks[0].notes[0].duration_ticks(document.chart.resolution);
        assert_eq!(duration, 480);
        assert!(tab_place_string(&mut document, &tools, duration, 0, 3));
        assert_eq!(document.chart.tracks[0].ties.len(), 1);
        assert_eq!(document.chart.tracks[0].notes[1].length, NoteValue::Eighth);
        assert_eq!(document.chart.tracks[0].notes[1].dots, 1);
        assert!(document.chart.tracks[0].notes[1].tuplet.is_some());
        tools.rest = true;
        assert!(tab_place_string(&mut document, &tools, 1920, 0, 0));
        let rest = &document.chart.tracks[0].notes[2];
        assert!(matches!(rest.content, NoteContent::Rest));
        assert!(rest.performance.is_none());
        assert_eq!(rest.duration_ticks(document.chart.resolution), 480);
        let layout = layout_track(&document.chart, &document.chart.tracks[0]);
        let symbol = layout.symbols.iter().find(|symbol| symbol.start == 1920).unwrap();
        assert_eq!(tab_lane_for_symbol(&document.chart.tracks[0], symbol), Some(0));
        assert!(remove_editor_event(&mut document, symbol.source));
        assert_eq!(document.chart.tracks[0].notes.len(), 2);
    }

    /// Untuned vocal phrase notes occupy a visible lane for range selection and pitch editing. AI_CODE
    #[test]
    fn bulk_vocal_tab_symbols_have_a_visible_lane() {
        let mut document = new_chart_document();
        let mut track = new_track(InstrumentKind::Voice);
        track.phrases = serde_json::from_str(r#"[{"start":0,"duration_ticks":960,"text":"la","notes":[{"start":0,"length":4,"note":60}]}]"#).unwrap();
        document.chart.tracks.push(track);
        let layout = layout_track(&document.chart, &document.chart.tracks[0]);
        assert_eq!(tab_lane_for_symbol(&document.chart.tracks[0], &layout.symbols[0]), Some(0));
    }

    /// Duration bars end at the authored tick or clip at the viewport boundary.
    ///
    /// AI_CODE
    #[test]
    fn tab_duration_bars_follow_note_end_and_clip() {
        assert_eq!(tab_duration_end_y(0, 480, 0, 1920, 100.0, -300.0), Some(0.0));
        assert_eq!(tab_duration_end_y(0, 240, 0, 1920, 100.0, -300.0), Some(50.0));
        assert_eq!(tab_duration_end_y(960, 1920, 0, 1920, 100.0, -300.0), Some(-300.0));
        assert_eq!(tab_duration_end_y(0, 1920, 0, 1920, 100.0, -300.0), Some(-300.0));
        assert_eq!(tab_duration_end_y(0, 0, 0, 1920, 100.0, -300.0), None);
    }

    /// Subdivision guides are unlabeled, progressively darker, and follow active meter beats.
    ///
    /// AI_CODE
    #[test]
    fn tab_subdivision_guides_share_meter_and_brightness() {
        let mut document = super::super::new_chart_document();
        let guides = tab_beat_guides(&document.chart, 0, 1920, 4);
        assert_eq!(guides.iter().filter(|guide| guide.label).map(|guide| guide.tick).collect::<Vec<_>>(), vec![0, 960]);
        assert_eq!(guides.iter().filter(|guide| !guide.label).map(|guide| guide.tick).collect::<Vec<_>>(), vec![240, 480, 720, 1200, 1440, 1680]);
        assert!(guides[1].color.to_srgba().red > guides[2].color.to_srgba().red);
        assert!(guides[2].color.to_srgba().red > guides[3].color.to_srgba().red);
        assert!(tab_beat_guides(&document.chart, 0, 1920, 1).iter().all(|guide| guide.label));
        document.chart.time_signature_map[0].numerator = 6;
        document.chart.time_signature_map[0].denominator = 8;
        let guides = tab_beat_guides(&document.chart, 100, 860, 2);
        assert_eq!(guides.iter().map(|guide| guide.tick).collect::<Vec<_>>(), vec![240, 480, 720]);
        assert_eq!(super::super::sheet::snap_sheet_tick(&document.chart, 300, 2), 240);
    }

    /// Shared beat divisions survive tab insertion even when the legacy snap grid is coarser.
    ///
    /// AI_CODE
    #[test]
    fn tab_insertion_preserves_shared_subdivision_tick() {
        let mut document = super::super::new_chart_document();
        document.chart.tracks.push(super::super::new_track(InstrumentKind::Strings));
        let tick = super::super::sheet::snap_sheet_tick(&document.chart, 400, 3);
        assert_eq!(tick, 320);
        assert!(tab_place_string(&mut document, &SheetTools::default(), tick, 0, 5));
        assert_eq!(document.chart.tracks[0].notes[0].start, 320);
    }

    /// Tab hover targets only nearby visible events on the chosen physical string.
    ///
    /// AI_CODE
    #[test]
    fn tab_hover_targets_lane_and_authored_tick_for_removal() {
        let mut document = super::super::new_chart_document();
        document.chart.tracks.push(super::super::new_track(InstrumentKind::Strings));
        let tools = SheetTools::default();
        assert!(tab_place_string(&mut document, &tools, 0, 0, 5));
        assert!(tab_place_string(&mut document, &tools, 960, 1, 3));
        let track = &document.chart.tracks[0];
        let layout = layout_track(&document.chart, track);
        let bounds = (0, 3840, 100.0, -300.0);
        assert_eq!(tab_hover_event(track, &layout.symbols, 1, 100.0, bounds), None);
        assert_eq!(tab_hover_event(track, &layout.symbols, 0, 75.0, bounds), None);
        assert_eq!(tab_hover_event(track, &layout.symbols, 0, 100.0, (960, 3840, 100.0, -300.0)), None);
        let (source, y) = tab_hover_event(track, &layout.symbols, 0, 98.0, bounds).unwrap();
        assert_eq!(y, 100.0);
        assert!(super::super::remove_editor_event(&mut document, source));
        assert_eq!(document.chart.tracks[0].notes.len(), 1);
        assert_eq!(document.chart.tracks[0].notes[0].start, 960);
    }

    /// Activating fret five on an ADGCF bass inserts D1 at the chosen tick.
    ///
    /// AI_CODE
    #[test]
    fn tab_fret_activation_places_low_bass_note() {
        let mut document = super::super::new_chart_document();
        document
            .chart
            .tracks
            .push(super::super::new_track(InstrumentKind::Strings));
        document.chart.tracks[0].tuning = Some(Tuning {
            strings: ["A0", "D1", "G1", "C2", "F2"]
                .map(str::to_owned)
                .to_vec(),
        });
        document.view_mode = EditorViewMode::Tab;
        document.tab_fret_string = Some(0);
        document.tab_fret_tick = 0;
        let mut app = App::new();
        app.insert_resource(document)
            .insert_resource(SheetTools::default())
            .add_observer(tab_fret_choice_input);
        let button = app.world_mut().spawn(TabFretChoice(5)).id();
        app.world_mut().trigger(Activate { entity: button });

        let document = app.world().resource::<EditorDocument>();
        assert_eq!(document.chart.tracks[0].notes.len(), 1);
        let event = &document.chart.tracks[0].notes[0];
        assert_eq!(event.start, 0);
        assert!(matches!(event.content, NoteContent::Pitched { note: 26, .. }));
        assert_eq!(event.performance.as_ref().unwrap().preferred_string, Some(0));
        assert!(document.dirty);
        assert_eq!(document.tab_fret_string, None);
    }

    /// Vertical tab coordinates progress from the top of the window to the bottom in time order.
    ///
    /// AI_CODE
    #[test]
    fn tab_time_maps_top_to_bottom() {
        let bottom = -300.0;
        let top = 250.0;
        assert_eq!(tab_y(100, 100, 1000, top, bottom), Some(top));
        assert_eq!(
            tab_y(600, 100, 1000, top, bottom),
            Some((top + bottom) / 2.0)
        );
        assert_eq!(tab_y(1100, 100, 1000, top, bottom), None);
        assert_eq!(tab_y(99, 100, 1000, top, bottom), None);
    }

    /// Taller windows extend the view to the bottom and show more time at the same density.
    ///
    /// AI_CODE
    #[test]
    fn tab_visible_span_grows_with_window_height() {
        let standard = tab_visible_span(960, 8, 720.0);
        let tall = tab_visible_span(960, 8, 1440.0);
        assert_eq!(standard, 960 * 8);
        assert!(tall > standard);
        let (standard_top, standard_bottom) = tab_viewport_bounds(720.0);
        let (tall_top, tall_bottom) = tab_viewport_bounds(1440.0);
        assert_eq!(standard_top - standard_bottom, 396.0);
        assert_eq!(tall_top - tall_bottom, 1116.0);
        assert_eq!(720.0 / 2.0 - standard_top, TAB_TOP_SCREEN_MARGIN);
        assert_eq!(1440.0 / 2.0 - tall_top, TAB_TOP_SCREEN_MARGIN);
    }

    /// A fret choice resolves through tuning/capo and records its preferred string separately.
    ///
    /// AI_CODE
    #[test]
    fn tab_fret_choice_places_written_pitch_and_string_hint() {
        let mut document = super::super::new_chart_document();
        document
            .chart
            .tracks
            .push(super::super::new_track(InstrumentKind::Strings));
        document.chart.tracks[0].capo = Some(2);
        let open = document.chart.tracks[0]
            .tuning
            .as_ref()
            .unwrap()
            .open_midi()
            .unwrap()[0];
        let tools = SheetTools::default();

        assert!(tab_place_string(&mut document, &tools, 0, 0, 3));
        let event = &document.chart.tracks[0].notes[0];
        assert!(matches!(event.content, NoteContent::Pitched { note, .. } if note == open + 5));
        assert_eq!(
            event
                .performance
                .as_ref()
                .and_then(|hint| hint.preferred_string),
            Some(0)
        );
    }

    /// Duration spelling respects the active snap minimum and resize stops before lane collisions.
    ///
    /// AI_CODE
    #[test]
    fn tab_resize_snaps_and_caps_before_next_note_on_lane() {
        let mut document = super::super::new_chart_document();
        document
            .chart
            .tracks
            .push(super::super::new_track(InstrumentKind::Strings));
        let tools = SheetTools::default();
        let open = document.chart.tracks[0]
            .tuning
            .as_ref()
            .unwrap()
            .open_midi()
            .unwrap()[0];
        assert!(tab_place_string(&mut document, &tools, 0, 0, 3));
        assert!(tab_place_string(&mut document, &tools, 1920, 0, 5));

        let single_symbol_limit = tools.duration.ticks(document.chart.resolution);
        assert!(resize_tab_event(
            &mut document,
            EventSource::Track(0),
            3000,
            0,
            single_symbol_limit,
        ));
        let first = &document.chart.tracks[0].notes[0];
        assert_eq!(first.duration_ticks(document.chart.resolution), 1920);
        assert!(first.tied);
        assert!(matches!(first.content, NoteContent::Pitched { note, .. } if note == open + 3));

        assert_eq!(
            duration_spelling(0, document.chart.resolution),
            (NoteValue::Sixteenth, 0)
        );
        assert_eq!(
            duration_spelling(1920, document.chart.resolution),
            (NoteValue::Half, 0)
        );
        assert!(resize_tab_event(
            &mut document,
            EventSource::Track(0),
            0,
            0,
            single_symbol_limit,
        ));
        assert_eq!(
            document.chart.tracks[0].notes[0].duration_ticks(document.chart.resolution),
            single_symbol_limit
        );
    }
}
