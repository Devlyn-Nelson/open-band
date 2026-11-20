use crate::*;
use bevy::input::keyboard::KeyboardInput;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Component)]
pub(crate) struct EditorCamera;

#[derive(Component)]
pub(crate) struct EditorText;

/// AI_CODE
pub(crate) struct EditorChartEntry {
    pub(crate) title: String,
    pub(crate) path: PathBuf,
}

#[derive(Resource, Default)]
/// The chart-picker shown before a document is open (see `EditorDocument`).
///
/// AI_CODE
pub(crate) struct EditorBrowser {
    pub(crate) entries: Vec<EditorChartEntry>,
    pub(crate) selected: usize,
}

#[derive(Resource)]
/// Tuning/kit libraries loaded once per editor session, shared by the track list's
/// tuning/kit cycling.
///
/// AI_CODE
pub(crate) struct EditorLibraries {
    pub(crate) tunings: Vec<NamedTuning>,
    pub(crate) kits: Vec<NamedKit>,
}

#[derive(Resource)]
/// The chart currently open for editing. Its presence (vs. `EditorBrowser` alone) is what
/// switches the Editor state between the chart picker and the track list.
///
/// AI_CODE
pub(crate) struct EditorDocument {
    pub(crate) chart: Chart,
    pub(crate) path: Option<PathBuf>,
    pub(crate) dirty: bool,
    pub(crate) track_focus: usize,
    pub(crate) text_entry: Option<EditorTextEntry>,
    pub(crate) metadata_mode: bool,
    pub(crate) metadata_focus: usize,
    pub(crate) structure_kind: usize,
    pub(crate) expression_kind: usize,
    pub(crate) snap_interval: SnapInterval,
    pub(crate) status: String,
}

#[derive(Clone, Copy)]
/// Field being edited in the editor's text-entry mode.
///
/// AI_CODE
pub(crate) enum EditorTextTarget {
    TrackName,
    ChartTitle,
    TempoText,
    RehearsalLabel,
    VocalRangeLow,
    VocalRangeHigh,
}

/// AI_CODE
pub(crate) struct EditorTextEntry {
    pub(crate) target: EditorTextTarget,
    pub(crate) buffer: String,
}

/// AI_CODE
fn list_editable_charts() -> Vec<EditorChartEntry> {
    let mut paths = std::fs::read_dir("charts")
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| {
            let contents = std::fs::read_to_string(&path).ok()?;
            let chart = serde_json::from_str::<Chart>(&contents).ok()?;
            Some(EditorChartEntry {
                title: chart.title,
                path,
            })
        })
        .collect()
}

/// AI_CODE
fn new_chart_document() -> EditorDocument {
    EditorDocument {
        chart: Chart {
            version: 1,
            title: "New Chart".into(),
            resolution: 960,
            tempo_map: vec![TempoChange {
                start: 0,
                bpm: 120.0,
            }],
            time_signature_map: vec![TimeSignatureChange {
                start: 0,
                numerator: 4,
                denominator: 4,
            }],
            tempo_text: Vec::new(),
            expressions: Vec::new(),
            structure: Vec::new(),
            tracks: Vec::new(),
        },
        path: None,
        dirty: true,
        track_focus: 0,
        text_entry: None,
        metadata_mode: false,
        metadata_focus: 0,
        structure_kind: 0,
        expression_kind: 0,
        snap_interval: NoteValue::Quarter,
        status: "New chart".into(),
    }
}

/// Add deterministic IDs only where the score has none, preserving every authored ID.
///
/// AI_CODE
fn all_track_events(track: &ChartTrack) -> impl Iterator<Item = &ChartEvent> {
    track
        .notes
        .iter()
        .chain(track.phrases.iter().flat_map(|phrase| phrase.notes.iter()))
        .chain(
            track
                .lyric_verses
                .iter()
                .flat_map(|verse| verse.phrases.iter())
                .flat_map(|phrase| phrase.notes.iter()),
        )
}

/// AI_CODE
fn ensure_stable_event_ids(chart: &mut Chart) -> bool {
    let mut reserved = HashSet::new();
    for event in chart.tracks.iter().flat_map(all_track_events) {
        if let Some(id) = &event.id {
            reserved.insert(id.clone());
        }
    }
    let mut used = HashSet::new();
    let mut next_id = 1u64;
    let mut changed = false;
    for track in &mut chart.tracks {
        for event in track
            .notes
            .iter_mut()
            .chain(
                track
                    .phrases
                    .iter_mut()
                    .flat_map(|phrase| phrase.notes.iter_mut()),
            )
            .chain(
                track
                    .lyric_verses
                    .iter_mut()
                    .flat_map(|verse| verse.phrases.iter_mut())
                    .flat_map(|phrase| phrase.notes.iter_mut()),
            )
        {
            let id_is_unique = event.id.as_ref().is_some_and(|id| used.insert(id.clone()));
            if !id_is_unique {
                loop {
                    let candidate = format!("event-{next_id:06}");
                    next_id += 1;
                    if !reserved.contains(&candidate) && used.insert(candidate.clone()) {
                        event.id = Some(candidate);
                        changed = true;
                        break;
                    }
                }
            }
        }
    }
    changed
}

/// AI_CODE
fn load_chart_document(path: &Path) -> Result<EditorDocument, String> {
    let contents = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut chart = serde_json::from_str::<Chart>(&contents).map_err(|error| error.to_string())?;
    let assigned_ids = ensure_stable_event_ids(&mut chart);
    Ok(EditorDocument {
        chart,
        path: Some(path.to_path_buf()),
        dirty: assigned_ids,
        track_focus: 0,
        text_entry: None,
        metadata_mode: false,
        metadata_focus: 0,
        structure_kind: 0,
        expression_kind: 0,
        snap_interval: NoteValue::Quarter,
        status: "Loaded".into(),
    })
}

/// Apply a committed score metadata text field.
///
/// AI_CODE
fn commit_text_entry(document: &mut EditorDocument, target: EditorTextTarget, value: String) {
    let value = value.trim().to_string();
    if value.is_empty() {
        return;
    }
    match target {
        EditorTextTarget::TrackName => {
            if let Some(track) = document.chart.tracks.get_mut(document.track_focus) {
                track.name = value;
            }
        }
        EditorTextTarget::ChartTitle => document.chart.title = value,
        EditorTextTarget::TempoText => {
            if let Some(tempo_text) = document.chart.tempo_text.last_mut() {
                tempo_text.text = value;
            } else {
                document.chart.tempo_text.push(TempoText {
                    tick: document.chart.total_ticks(),
                    text: value,
                    bpm: None,
                });
            }
        }
        EditorTextTarget::RehearsalLabel => {
            if let Some(marker) = document
                .chart
                .structure
                .iter_mut()
                .rev()
                .find(|marker| matches!(marker, ScoreMarker::Rehearsal { .. }))
            {
                if let ScoreMarker::Rehearsal { label, .. } = marker {
                    *label = value;
                }
            } else {
                document.chart.structure.push(ScoreMarker::Rehearsal {
                    tick: document.chart.total_ticks(),
                    label: value,
                });
            }
        }
        EditorTextTarget::VocalRangeLow | EditorTextTarget::VocalRangeHigh => {
            if let Some(track) = document.chart.tracks.get_mut(document.track_focus)
                && track.kind == InstrumentKind::Voice
            {
                let range = track.vocal_range.get_or_insert(VocalRange {
                    low: "C3".into(),
                    high: "C6".into(),
                });
                match target {
                    EditorTextTarget::VocalRangeLow => range.low = value,
                    EditorTextTarget::VocalRangeHigh => range.high = value,
                    _ => unreachable!(),
                }
            }
        }
    }
    document.dirty = true;
}

/// Cycle or nudge the currently selected score metadata field.
///
/// AI_CODE
fn adjust_score_metadata(document: &mut EditorDocument, libraries: &EditorLibraries, delta: i32) {
    let track = document.chart.tracks.get_mut(document.track_focus);
    match document.metadata_focus {
        1 => {
            const RESOLUTIONS: [u32; 6] = [120, 240, 480, 960, 1920, 3840];
            let current = RESOLUTIONS
                .iter()
                .position(|resolution| *resolution == document.chart.resolution)
                .unwrap_or(3);
            let next = (current as i32 + delta).rem_euclid(RESOLUTIONS.len() as i32) as usize;
            document.chart.resolution = RESOLUTIONS[next];
        }
        2 => {
            if document.chart.tempo_map.is_empty() {
                document.chart.tempo_map.push(TempoChange {
                    start: 0,
                    bpm: 120.0,
                });
            }
            if let Some(tempo) = document
                .chart
                .tempo_map
                .iter_mut()
                .min_by_key(|tempo| tempo.start)
            {
                tempo.bpm = (tempo.bpm + delta as f32).clamp(20.0, 300.0);
            }
        }
        3 => {
            if document.chart.time_signature_map.is_empty() {
                document.chart.time_signature_map.push(TimeSignatureChange {
                    start: 0,
                    numerator: 4,
                    denominator: 4,
                });
            }
            if let Some(signature) = document
                .chart
                .time_signature_map
                .iter_mut()
                .min_by_key(|signature| signature.start)
            {
                signature.numerator = (signature.numerator as i32 + delta).clamp(1, 32) as u8;
            }
        }
        4 => {
            const DENOMINATORS: [u8; 5] = [1, 2, 4, 8, 16];
            if document.chart.time_signature_map.is_empty() {
                document.chart.time_signature_map.push(TimeSignatureChange {
                    start: 0,
                    numerator: 4,
                    denominator: 4,
                });
            }
            if let Some(signature) = document
                .chart
                .time_signature_map
                .iter_mut()
                .min_by_key(|signature| signature.start)
            {
                let current = DENOMINATORS
                    .iter()
                    .position(|denominator| *denominator == signature.denominator)
                    .unwrap_or(2);
                let next = (current as i32 + delta).rem_euclid(DENOMINATORS.len() as i32) as usize;
                signature.denominator = DENOMINATORS[next];
            }
        }
        7 => {
            document.structure_kind =
                (document.structure_kind as i32 + delta).rem_euclid(10) as usize;
        }
        8 => {
            if let Some(track) = track {
                let clefs = [
                    None,
                    Some(Clef::Treble),
                    Some(Clef::Bass),
                    Some(Clef::Alto),
                    Some(Clef::Tenor),
                ];
                let current = clefs
                    .iter()
                    .position(|clef| *clef == track.clef)
                    .unwrap_or(0);
                let next = (current as i32 + delta).rem_euclid(clefs.len() as i32) as usize;
                track.clef = clefs[next];
            }
        }
        9 => {
            if let Some(track) = track {
                let key = track.key_signature.get_or_insert(KeySignature {
                    fifths: 0,
                    mode: KeyMode::Major,
                });
                key.fifths = (key.fifths as i32 + delta).clamp(-7, 7) as i8;
            }
        }
        10 => {
            if let Some(track) = track {
                let key = track.key_signature.get_or_insert(KeySignature {
                    fifths: 0,
                    mode: KeyMode::Major,
                });
                key.mode = match key.mode {
                    KeyMode::Major => KeyMode::Minor,
                    KeyMode::Minor => KeyMode::Major,
                };
            }
        }
        11 => {
            if let Some(track) = track {
                let capo = track.capo.unwrap_or(0) as i32;
                track.capo = Some((capo + delta).clamp(0, 24) as u8);
            }
        }
        12 => {
            if let Some(track) = track {
                cycle_track_tuning(track, &libraries.tunings, delta);
            }
        }
        13 => {
            if let Some(track) = track {
                cycle_track_kit(track, &libraries.kits, delta);
            }
        }
        14 => {
            document.expression_kind =
                (document.expression_kind as i32 + delta).rem_euclid(4) as usize
        }
        _ => return,
    }
    document.dirty = true;
}

/// Insert the selected navigation marker at the score's current end.
///
/// AI_CODE
fn add_score_structure_marker(document: &mut EditorDocument) {
    let tick = document.chart.total_ticks();
    let marker = match document.structure_kind {
        0 => ScoreMarker::RepeatStart { tick },
        1 => ScoreMarker::RepeatEnd { tick, times: 2 },
        2 => ScoreMarker::EndingStart { tick, number: 1 },
        3 => ScoreMarker::EndingEnd { tick, number: 1 },
        4 => ScoreMarker::Segno { tick },
        5 => ScoreMarker::Coda { tick },
        6 => ScoreMarker::Fine { tick },
        7 => ScoreMarker::DaCapo { tick },
        8 => ScoreMarker::DalSegno { tick },
        _ => ScoreMarker::Rehearsal {
            tick,
            label: "A".into(),
        },
    };
    document.chart.structure.push(marker);
    document.dirty = true;
}

/// Insert the selected score expression over one measure at the score end.
///
/// AI_CODE
fn add_score_expression(document: &mut EditorDocument) {
    let start = document.chart.total_ticks();
    let signature = document
        .chart
        .time_signature_map
        .iter()
        .filter(|signature| signature.start <= start)
        .max_by_key(|signature| signature.start)
        .copied()
        .unwrap_or(TimeSignatureChange {
            start: 0,
            numerator: 4,
            denominator: 4,
        });
    let duration_ticks = (document.chart.resolution.max(1) as u64 * signature.numerator as u64 * 4
        / signature.denominator.max(1) as u64)
        .clamp(1, u32::MAX as u64) as u32;
    let bpm = document
        .chart
        .tempo_map
        .iter()
        .filter(|tempo| tempo.start <= start)
        .max_by_key(|tempo| tempo.start)
        .map_or(120.0, |tempo| tempo.bpm);
    let expression = match document.expression_kind {
        0 => ScoreExpression::Accelerando {
            start,
            duration_ticks,
            from_bpm: bpm,
            to_bpm: (bpm + 20.0).min(300.0),
        },
        1 => ScoreExpression::Ritardando {
            start,
            duration_ticks,
            from_bpm: bpm,
            to_bpm: (bpm - 20.0).max(20.0),
        },
        2 => ScoreExpression::Crescendo {
            start,
            duration_ticks,
        },
        _ => ScoreExpression::Diminuendo {
            start,
            duration_ticks,
        },
    };
    document.chart.expressions.push(expression);
    document.dirty = true;
}

/// Save the full score document, assigning IDs only to events that lack one.
///
/// AI_CODE
fn save_editor_document(document: &mut EditorDocument) {
    if ensure_stable_event_ids(&mut document.chart) {
        document.dirty = true;
    }
    let warnings = document.chart.validate();
    let title = document.chart.title.clone();
    let path = document
        .path
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("charts/{}.json", slugify(&title))));
    match serde_json::to_string_pretty(&document.chart)
        .map_err(|error| error.to_string())
        .and_then(|contents| std::fs::write(&path, contents).map_err(|error| error.to_string()))
    {
        Ok(()) => {
            document.path = Some(path);
            document.dirty = false;
            document.status = if warnings.is_empty() {
                "Saved".into()
            } else {
                format!(
                    "Saved with {} warning(s): {}",
                    warnings.len(),
                    warnings.join("; ")
                )
            };
        }
        Err(error) => document.status = format!("Save failed: {error}"),
    }
}

/// Snap a document position using its active interval and time-signature map.
///
/// This shared helper is intended for both sheet and tab placement and is independent
/// of the tempo map.
///
/// AI_CODE
pub(crate) fn snap_editor_tick(document: &EditorDocument, tick: u32) -> u32 {
    snap_tick_in_chart(tick, document.snap_interval, &document.chart)
}

/// Cycle through the common notation snap intervals.
///
/// AI_CODE
fn cycle_snap_interval(interval: SnapInterval, delta: i32) -> SnapInterval {
    const INTERVALS: [SnapInterval; 5] = [
        NoteValue::Whole,
        NoteValue::Half,
        NoteValue::Quarter,
        NoteValue::Eighth,
        NoteValue::Sixteenth,
    ];
    let current = INTERVALS
        .iter()
        .position(|candidate| *candidate == interval)
        .unwrap_or(2);
    let next = (current as i32 + delta).rem_euclid(INTERVALS.len() as i32) as usize;
    INTERVALS[next]
}

/// Human-readable note value for the editor snap control.
///
/// AI_CODE
fn snap_interval_label(interval: SnapInterval) -> &'static str {
    match interval {
        NoteValue::Whole => "1/1",
        NoteValue::Half => "1/2",
        NoteValue::Quarter => "1/4",
        NoteValue::Eighth => "1/8",
        NoteValue::Sixteenth => "1/16",
    }
}

/// AI_CODE
pub(crate) fn new_track(kind: InstrumentKind) -> ChartTrack {
    ChartTrack {
        name: "New Track".into(),
        kind,
        tuning: matches!(kind, InstrumentKind::Strings).then(Tuning::default),
        kit: matches!(kind, InstrumentKind::Percussion).then(Kit::default),
        vocal_range: None,
        clef: None,
        key_signature: None,
        capo: None,
        notes: Vec::new(),
        ties: Vec::new(),
        slurs: Vec::new(),
        phrases: Vec::new(),
        star_power_phrases: Vec::new(),
        lyric_verses: Vec::new(),
    }
}

#[test]
/// New tracks default to a sensible tuning/kit for their kind, and none for others.
fn new_track_has_expected_defaults_per_kind() {
    let strings = new_track(InstrumentKind::Strings);
    assert!(strings.tuning.is_some());
    assert!(strings.kit.is_none());

    let percussion = new_track(InstrumentKind::Percussion);
    assert!(percussion.kit.is_some());
    assert!(percussion.tuning.is_none());

    let voice = new_track(InstrumentKind::Voice);
    assert!(voice.tuning.is_none());
    assert!(voice.kit.is_none());
}

/// A filesystem-safe slug derived from a chart title, used as the save filename.
///
/// AI_CODE
pub(crate) fn slugify(title: &str) -> String {
    let slug = title
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let slug = slug
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "untitled".into()
    } else {
        slug
    }
}

#[test]
/// Chart titles slugify into filesystem-safe, non-empty save filenames.
fn slugify_produces_filesystem_safe_names() {
    assert_eq!(slugify("Easy Band Jam"), "easy-band-jam");
    assert_eq!(slugify("  Weird!! Title??  "), "weird-title");
    assert_eq!(slugify(""), "untitled");
}

/// AI_CODE
pub(crate) fn setup_editor(mut commands: Commands) {
    commands.insert_resource(EditorBrowser {
        entries: list_editable_charts(),
        selected: 0,
    });
    commands.insert_resource(EditorLibraries {
        tunings: load_tuning_library(),
        kits: load_kit_library(),
    });
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(24.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(40.0),
            left: Val::Px(70.0),
            ..default()
        },
        EditorText,
    ));
}

/// AI_CODE
pub(crate) fn cleanup_editor(mut commands: Commands, entities: Query<Entity, With<EditorText>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<EditorDocument>();
    commands.remove_resource::<EditorBrowser>();
    commands.remove_resource::<EditorLibraries>();
}

/// AI_CODE
pub(crate) fn editor_browse_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut browser: ResMut<EditorBrowser>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Home);
        return;
    }
    if !browser.entries.is_empty() {
        if keyboard.just_pressed(KeyCode::ArrowUp) {
            browser.selected = browser
                .selected
                .checked_sub(1)
                .unwrap_or(browser.entries.len() - 1);
        }
        if keyboard.just_pressed(KeyCode::ArrowDown) {
            browser.selected = (browser.selected + 1) % browser.entries.len();
        }
    }
    if keyboard.just_pressed(KeyCode::KeyN) {
        commands.insert_resource(new_chart_document());
        return;
    }
    if keyboard.just_pressed(KeyCode::Enter)
        && let Some(entry) = browser.entries.get(browser.selected)
    {
        match load_chart_document(&entry.path) {
            Ok(document) => commands.insert_resource(document),
            Err(error) => eprintln!("Could not load chart {}: {error}", entry.path.display()),
        }
    }
}

/// AI_CODE
pub(crate) fn editor_browse_display(
    browser: Res<EditorBrowser>,
    mut text: Query<&mut Text, With<EditorText>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let entries = if browser.entries.is_empty() {
        "  (no charts found in charts/)".to_string()
    } else {
        browser
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                format!(
                    "{} {}",
                    if index == browser.selected { ">" } else { " " },
                    entry.title
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    *text = Text::new(format!(
        "OPEN BAND  //  CHART EDITOR\n\n{entries}\n\n\
        Up/Down: select     Enter: edit chart     N: new chart     Esc: home",
    ));
}

/// AI_CODE
pub(crate) fn cycle_track_tuning(track: &mut ChartTrack, library: &[NamedTuning], delta: i32) {
    if library.is_empty() || track.kind != InstrumentKind::Strings {
        return;
    }
    let current = track
        .tuning
        .as_ref()
        .and_then(|tuning| {
            library
                .iter()
                .position(|named| named.strings == tuning.strings)
        })
        .unwrap_or(0);
    let next = (current as i32 + delta).rem_euclid(library.len() as i32) as usize;
    track.tuning = Some(library[next].tuning());
}

/// AI_CODE
pub(crate) fn cycle_track_kit(track: &mut ChartTrack, library: &[NamedKit], delta: i32) {
    if library.is_empty() || track.kind != InstrumentKind::Percussion {
        return;
    }
    let current = track
        .kit
        .as_ref()
        .and_then(|kit| {
            library
                .iter()
                .position(|named| named.lanes == kit.lanes && named.pieces == kit.pieces)
        })
        .unwrap_or(0);
    let next = (current as i32 + delta).rem_euclid(library.len() as i32) as usize;
    track.kit = Some(library[next].kit());
}

#[test]
/// Cycling a track's tuning/kit steps through the library and wraps around.
fn cycle_track_tuning_and_kit_step_through_the_library() {
    let tunings = load_tuning_library();
    let mut track = new_track(InstrumentKind::Strings);
    let first = track.tuning.clone();
    cycle_track_tuning(&mut track, &tunings, 1);
    assert_ne!(track.tuning, first, "cycling should change the tuning");
    for _ in 0..tunings.len().saturating_sub(1) {
        cycle_track_tuning(&mut track, &tunings, 1);
    }
    assert_eq!(
        track.tuning, first,
        "cycling all the way around should return to start"
    );

    let kits = load_kit_library();
    let mut drum_track = new_track(InstrumentKind::Percussion);
    let first_kit = drum_track.kit.clone();
    cycle_track_kit(&mut drum_track, &kits, 1);
    if kits.len() > 1 {
        assert_ne!(drum_track.kit, first_kit);
    }
}

#[test]
/// Snap interval is shared editor state and uses chart meter/resolution boundaries.
fn editor_snap_interval_cycles_and_snaps_ticks() {
    let mut document = new_chart_document();
    let initially_dirty = document.dirty;
    document.chart.time_signature_map.push(TimeSignatureChange {
        start: 3840,
        numerator: 3,
        denominator: 4,
    });
    assert_eq!(snap_editor_tick(&document, 4080), 3840);
    document.snap_interval = cycle_snap_interval(document.snap_interval, 1);
    assert_eq!(document.snap_interval, NoteValue::Eighth);
    assert_eq!(snap_editor_tick(&document, 4080), 4320);
    assert_eq!(snap_editor_tick(&document, 4320), 4320);
    assert_eq!(
        document.dirty, initially_dirty,
        "snap tool is not a score edit"
    );
    assert_eq!(
        cycle_snap_interval(NoteValue::Sixteenth, 1),
        NoteValue::Whole
    );
}

#[test]
/// Metadata edits cover document, track, tempo, signature, marker, and expression data.
/// AI_CODE
fn score_metadata_edits_preserve_score_and_performance_fields() {
    let libraries = EditorLibraries {
        tunings: load_tuning_library(),
        kits: load_kit_library(),
    };
    let mut document = new_chart_document();
    document
        .chart
        .tracks
        .push(new_track(InstrumentKind::Strings));
    document.chart.tracks[0].notes.push(
        serde_json::from_str(
            r#"{"start":0,"length":4,"note":"E2","performance":{"preferred_string":0,"attack":"tap"}}"#,
        )
        .expect("score note with performance guidance should parse"),
    );

    document.metadata_focus = 1;
    adjust_score_metadata(&mut document, &libraries, 1);
    assert_eq!(document.chart.resolution, 1920);
    document.metadata_focus = 2;
    adjust_score_metadata(&mut document, &libraries, 10);
    assert_eq!(document.chart.tempo_map[0].bpm, 130.0);
    document.metadata_focus = 3;
    adjust_score_metadata(&mut document, &libraries, 1);
    document.metadata_focus = 4;
    adjust_score_metadata(&mut document, &libraries, 1);
    assert_eq!(document.chart.time_signature_map[0].numerator, 5);
    assert_eq!(document.chart.time_signature_map[0].denominator, 8);

    document.metadata_focus = 8;
    adjust_score_metadata(&mut document, &libraries, 1);
    document.metadata_focus = 9;
    adjust_score_metadata(&mut document, &libraries, 1);
    document.metadata_focus = 10;
    adjust_score_metadata(&mut document, &libraries, 1);
    document.metadata_focus = 11;
    adjust_score_metadata(&mut document, &libraries, 3);
    let track = &document.chart.tracks[0];
    assert_eq!(track.clef, Some(Clef::Treble));
    assert_eq!(track.key_signature.map(|key| key.fifths), Some(1));
    assert_eq!(
        track.key_signature.map(|key| key.mode),
        Some(KeyMode::Minor)
    );
    assert_eq!(track.capo, Some(3));
    let standard_tuning = track.tuning.clone();
    document.metadata_focus = 12;
    adjust_score_metadata(&mut document, &libraries, 1);
    assert_ne!(document.chart.tracks[0].tuning, standard_tuning);
    document
        .chart
        .tracks
        .push(new_track(InstrumentKind::Percussion));
    document.track_focus = 1;
    let standard_kit = document.chart.tracks[1].kit.clone();
    document.metadata_focus = 13;
    adjust_score_metadata(&mut document, &libraries, 1);
    if libraries.kits.len() > 1 {
        assert_ne!(document.chart.tracks[1].kit, standard_kit);
    }

    document.expression_kind = 2;
    add_score_expression(&mut document);
    assert!(matches!(
        document.chart.expressions[0],
        ScoreExpression::Crescendo { .. }
    ));
    document.structure_kind = 4;
    add_score_structure_marker(&mut document);
    assert!(matches!(
        document.chart.structure[0],
        ScoreMarker::Segno { .. }
    ));

    assert_eq!(
        document.chart.tracks[0].notes[0]
            .performance
            .as_ref()
            .and_then(|performance| performance.attack),
        Some(NoteAttack::Tap)
    );
}

#[test]
/// Missing IDs are assigned once across rests and nested vocal phrase events.
/// AI_CODE
fn stable_event_ids_cover_explicit_rests_and_vocal_notes() {
    let mut chart = new_chart_document().chart;
    let mut voice = new_track(InstrumentKind::Voice);
    voice.notes = serde_json::from_str(
        r#"[
            {"id":"authored-id","start":0,"length":4,"note":"C4"},
            {"start":960,"length":4,"rest":true},
            {"id":"event-000001","start":1920,"length":4,"note":"E4"}
        ]"#,
    )
    .expect("voice notes and rest should parse");
    voice.phrases.push(VocalPhrase {
        start: 0,
        duration_ticks: 1920,
        text: "hello".into(),
        syllable: None,
        melisma: false,
        hyphen_after: false,
        breath_after: false,
        notes: serde_json::from_str(r#"[{"start":0,"length":4,"note":"C4"}]"#)
            .expect("vocal phrase note should parse"),
    });
    chart.tracks.push(voice);

    assert!(ensure_stable_event_ids(&mut chart));
    let events = all_track_events(&chart.tracks[0]).collect::<Vec<_>>();
    assert_eq!(events[0].id.as_deref(), Some("authored-id"));
    assert_eq!(events[1].id.as_deref(), Some("event-000002"));
    assert_eq!(events[2].id.as_deref(), Some("event-000001"));
    assert!(events.iter().all(|event| event.id.is_some()));
    assert_eq!(
        events
            .iter()
            .filter_map(|event| event.id.as_deref())
            .collect::<HashSet<_>>()
            .len(),
        events.len()
    );
    assert!(!ensure_stable_event_ids(&mut chart));
    let serialized = serde_json::to_string(&chart).expect("score should serialize");
    let reloaded: Chart = serde_json::from_str(&serialized).expect("score should reload");
    assert_eq!(reloaded.tracks[0].notes[0].id, chart.tracks[0].notes[0].id);
    assert_eq!(
        reloaded.tracks[0].phrases[0].notes[0].id,
        chart.tracks[0].phrases[0].notes[0].id
    );
}

#[test]
/// Chart title, tempo text, and rehearsal mark edits update score metadata directly.
/// AI_CODE
fn score_metadata_text_fields_edit_the_score_document() {
    let mut document = new_chart_document();
    document.chart.tracks.push(new_track(InstrumentKind::Voice));
    commit_text_entry(
        &mut document,
        EditorTextTarget::ChartTitle,
        "  Rehearsal Score  ".into(),
    );
    commit_text_entry(&mut document, EditorTextTarget::TempoText, "Allegro".into());
    commit_text_entry(&mut document, EditorTextTarget::RehearsalLabel, "B".into());
    commit_text_entry(&mut document, EditorTextTarget::VocalRangeLow, "A2".into());
    commit_text_entry(&mut document, EditorTextTarget::VocalRangeHigh, "G5".into());
    assert_eq!(document.chart.title, "Rehearsal Score");
    assert_eq!(document.chart.tempo_text[0].text, "Allegro");
    assert!(matches!(
        &document.chart.structure[0],
        ScoreMarker::Rehearsal { label, .. } if label == "B"
    ));
    assert_eq!(
        document.chart.tracks[0].vocal_range.as_ref().unwrap().low,
        "A2"
    );
    assert_eq!(
        document.chart.tracks[0].vocal_range.as_ref().unwrap().high,
        "G5"
    );
    assert!(document.dirty);
}

/// AI_CODE
pub(crate) fn editor_tracks_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut keyboard_events: MessageReader<KeyboardInput>,
    mut document: ResMut<EditorDocument>,
    libraries: Res<EditorLibraries>,
    mut commands: Commands,
) {
    if let Some(entry) = document.text_entry.as_mut() {
        for event in keyboard_events.read() {
            if event.state != bevy::input::ButtonState::Pressed {
                continue;
            }
            if let Some(text) = &event.text {
                entry
                    .buffer
                    .extend(text.chars().filter(|character| !character.is_control()));
            }
        }
        if keyboard.just_pressed(KeyCode::Backspace) {
            if let Some(entry) = document.text_entry.as_mut() {
                entry.buffer.pop();
            }
        }
        if keyboard.just_pressed(KeyCode::Enter) {
            if let Some(entry) = document.text_entry.take() {
                commit_text_entry(&mut document, entry.target, entry.buffer);
            }
        } else if keyboard.just_pressed(KeyCode::Escape) {
            document.text_entry = None;
        }
        return;
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        if document.metadata_mode {
            document.metadata_mode = false;
        } else {
            commands.remove_resource::<EditorDocument>();
        }
        return;
    }

    if document.metadata_mode {
        if keyboard.just_pressed(KeyCode::KeyM) {
            document.metadata_mode = false;
            return;
        }
        if keyboard.just_pressed(KeyCode::Tab) {
            document.snap_interval = cycle_snap_interval(document.snap_interval, 1);
        }
        const METADATA_FIELDS: usize = 17;
        if keyboard.just_pressed(KeyCode::ArrowUp) {
            document.metadata_focus = document
                .metadata_focus
                .checked_sub(1)
                .unwrap_or(METADATA_FIELDS - 1);
        }
        if keyboard.just_pressed(KeyCode::ArrowDown) {
            document.metadata_focus = (document.metadata_focus + 1) % METADATA_FIELDS;
        }
        if keyboard.just_pressed(KeyCode::ArrowLeft) {
            adjust_score_metadata(&mut document, &libraries, -1);
        }
        if keyboard.just_pressed(KeyCode::ArrowRight) {
            adjust_score_metadata(&mut document, &libraries, 1);
        }
        if keyboard.just_pressed(KeyCode::KeyS) {
            save_editor_document(&mut document);
        }
        if keyboard.just_pressed(KeyCode::Delete) {
            match document.metadata_focus {
                5 => {
                    document.chart.tempo_text.pop();
                    document.dirty = true;
                }
                6 => {
                    if let Some(index) = document
                        .chart
                        .structure
                        .iter()
                        .rposition(|marker| matches!(marker, ScoreMarker::Rehearsal { .. }))
                    {
                        document.chart.structure.remove(index);
                        document.dirty = true;
                    }
                }
                7 => {
                    document.chart.structure.pop();
                    document.dirty = true;
                }
                14 => {
                    document.chart.expressions.pop();
                    document.dirty = true;
                }
                _ => {}
            }
        }
        if keyboard.just_pressed(KeyCode::Enter) {
            match document.metadata_focus {
                0 => {
                    document.text_entry = Some(EditorTextEntry {
                        target: EditorTextTarget::ChartTitle,
                        buffer: document.chart.title.clone(),
                    })
                }
                5 => {
                    document.text_entry = Some(EditorTextEntry {
                        target: EditorTextTarget::TempoText,
                        buffer: document
                            .chart
                            .tempo_text
                            .last()
                            .map_or_else(String::new, |tempo| tempo.text.clone()),
                    })
                }
                6 => {
                    document.text_entry = Some(EditorTextEntry {
                        target: EditorTextTarget::RehearsalLabel,
                        buffer: document
                            .chart
                            .structure
                            .iter()
                            .rev()
                            .find_map(|marker| match marker {
                                ScoreMarker::Rehearsal { label, .. } => Some(label.clone()),
                                _ => None,
                            })
                            .unwrap_or_else(|| "A".into()),
                    })
                }
                7 => add_score_structure_marker(&mut document),
                14 => add_score_expression(&mut document),
                15 => {
                    if let Some(track) = document.chart.tracks.get(document.track_focus)
                        && track.kind == InstrumentKind::Voice
                    {
                        document.text_entry = Some(EditorTextEntry {
                            target: EditorTextTarget::VocalRangeLow,
                            buffer: track
                                .vocal_range
                                .as_ref()
                                .map_or_else(|| "C3".into(), |range| range.low.clone()),
                        });
                    }
                }
                16 => {
                    if let Some(track) = document.chart.tracks.get(document.track_focus)
                        && track.kind == InstrumentKind::Voice
                    {
                        document.text_entry = Some(EditorTextEntry {
                            target: EditorTextTarget::VocalRangeHigh,
                            buffer: track
                                .vocal_range
                                .as_ref()
                                .map_or_else(|| "C6".into(), |range| range.high.clone()),
                        });
                    }
                }
                _ => adjust_score_metadata(&mut document, &libraries, 1),
            }
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyM) {
        document.metadata_mode = true;
        return;
    }
    if keyboard.just_pressed(KeyCode::Tab) {
        document.snap_interval = cycle_snap_interval(document.snap_interval, 1);
    }

    let track_count = document.chart.tracks.len();
    if track_count > 0 {
        if keyboard.just_pressed(KeyCode::ArrowUp) {
            document.track_focus = document
                .track_focus
                .checked_sub(1)
                .unwrap_or(track_count - 1);
        }
        if keyboard.just_pressed(KeyCode::ArrowDown) {
            document.track_focus = (document.track_focus + 1) % track_count;
        }
    }
    if keyboard.just_pressed(KeyCode::KeyN) {
        document
            .chart
            .tracks
            .push(new_track(InstrumentKind::Strings));
        document.track_focus = document.chart.tracks.len() - 1;
        document.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyX) && track_count > 0 {
        let focus = document.track_focus;
        document.chart.tracks.remove(focus);
        document.track_focus = document
            .track_focus
            .min(document.chart.tracks.len().saturating_sub(1));
        document.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyK) {
        let focus = document.track_focus;
        if let Some(track) = document.chart.tracks.get_mut(focus) {
            let next_kind = match track.kind {
                InstrumentKind::Strings => InstrumentKind::Percussion,
                InstrumentKind::Percussion => InstrumentKind::Voice,
                InstrumentKind::Voice => InstrumentKind::Strings,
            };
            let name = track.name.clone();
            *track = new_track(next_kind);
            track.name = name;
            document.dirty = true;
        }
    }
    if keyboard.just_pressed(KeyCode::BracketLeft) {
        let focus = document.track_focus;
        if let Some(track) = document.chart.tracks.get_mut(focus) {
            cycle_track_tuning(track, &libraries.tunings, -1);
            cycle_track_kit(track, &libraries.kits, -1);
            document.dirty = true;
        }
    }
    if keyboard.just_pressed(KeyCode::BracketRight) {
        let focus = document.track_focus;
        if let Some(track) = document.chart.tracks.get_mut(focus) {
            cycle_track_tuning(track, &libraries.tunings, 1);
            cycle_track_kit(track, &libraries.kits, 1);
            document.dirty = true;
        }
    }
    if keyboard.just_pressed(KeyCode::Comma) && document.track_focus > 0 {
        let focus = document.track_focus;
        document.chart.tracks.swap(focus, focus - 1);
        document.track_focus -= 1;
        document.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::Period) && document.track_focus + 1 < track_count {
        let focus = document.track_focus;
        document.chart.tracks.swap(focus, focus + 1);
        document.track_focus += 1;
        document.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyR) && track_count > 0 {
        let current_name = document.chart.tracks[document.track_focus].name.clone();
        document.text_entry = Some(EditorTextEntry {
            target: EditorTextTarget::TrackName,
            buffer: current_name,
        });
    }
    if keyboard.just_pressed(KeyCode::KeyS) {
        save_editor_document(&mut document);
    }
}

/// AI_CODE
pub(crate) fn editor_tracks_display(
    document: Res<EditorDocument>,
    mut text: Query<&mut Text, With<EditorText>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    if document.metadata_mode {
        *text = score_metadata_display(&document);
        return;
    }
    let dirty = if document.dirty { "*" } else { "" };
    let track_lines = if document.chart.tracks.is_empty() {
        "  (no tracks yet; press N to add one)".to_string()
    } else {
        document
            .chart
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| {
                let marker = if document.track_focus == index {
                    ">"
                } else {
                    " "
                };
                let detail = match track.kind {
                    InstrumentKind::Strings => track
                        .tuning
                        .as_ref()
                        .map_or("no tuning".into(), |tuning| tuning.strings.join("-")),
                    InstrumentKind::Percussion => {
                        track.kit.as_ref().map_or("no kit".into(), |kit| {
                            format!("{} pieces", kit.pieces.len())
                        })
                    }
                    InstrumentKind::Voice => "voice".into(),
                };
                format!("{marker} {} — {:?} ({detail})", track.name, track.kind)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let text_entry_line = document
        .text_entry
        .as_ref()
        .map_or_else(String::new, |entry| {
            format!("\n\nEDIT: {}_\nEnter: confirm  Esc: cancel", entry.buffer)
        });
    *text = Text::new(format!(
        "OPEN BAND  //  CHART EDITOR  //  {}{dirty}\n\n{track_lines}{text_entry_line}\n\n\
        Up/Down: focus track     N: add     X: remove     K: cycle kind\n\
        [ / ]: cycle tuning/kit     , / .: reorder     R: rename     M: score metadata\n\
        Tab: snap grid {}     S: save     Esc: back to chart list\n\n{}\n{}",
        document.chart.title,
        snap_interval_label(document.snap_interval),
        document.status,
        score_summary(&document.chart),
    ));
}

/// Render editable chart and focused-track metadata.
///
/// AI_CODE
fn score_metadata_display(document: &EditorDocument) -> Text {
    let chart = &document.chart;
    let track = chart.tracks.get(document.track_focus);
    let first_tempo = chart
        .tempo_map
        .iter()
        .min_by_key(|tempo| tempo.start)
        .map_or(120.0, |tempo| tempo.bpm);
    let signature = chart
        .time_signature_map
        .iter()
        .min_by_key(|signature| signature.start)
        .copied()
        .unwrap_or(TimeSignatureChange {
            start: 0,
            numerator: 4,
            denominator: 4,
        });
    let key = track.and_then(|track| track.key_signature);
    let clef = track
        .filter(|track| track.kind != InstrumentKind::Percussion)
        .and_then(|track| track.clef)
        .map_or_else(|| "Default".into(), |clef| format!("{clef:?}"));
    let key_text = key.map_or_else(
        || "None".into(),
        |key| format!("{} {:?}", key.fifths, key.mode),
    );
    let capo = track
        .and_then(|track| track.capo)
        .map_or_else(|| "None".into(), |capo| capo.to_string());
    let tuning = track
        .filter(|track| track.kind == InstrumentKind::Strings)
        .and_then(|track| track.tuning.as_ref())
        .map_or_else(|| "N/A".into(), |tuning| tuning.strings.join("-"));
    let kit = track
        .filter(|track| track.kind == InstrumentKind::Percussion)
        .and_then(|track| track.kit.as_ref())
        .map_or_else(
            || "N/A".into(),
            |kit| format!("{} pieces", kit.pieces.len()),
        );
    let vocal_range = track
        .filter(|track| track.kind == InstrumentKind::Voice)
        .and_then(|track| track.vocal_range.as_ref())
        .map_or_else(
            || "N/A".into(),
            |range| format!("{}-{}", range.low, range.high),
        );
    let tempo_text = chart
        .tempo_text
        .last()
        .map_or("(none)", |tempo| tempo.text.as_str());
    let rehearsal = chart
        .structure
        .iter()
        .rev()
        .find_map(|marker| match marker {
            ScoreMarker::Rehearsal { label, .. } => Some(label.as_str()),
            _ => None,
        })
        .unwrap_or("(none)");
    let marker_name = score_marker_name(document.structure_kind);
    let rows = [
        format!("TITLE          {}", chart.title),
        format!("RESOLUTION     {} ticks/quarter", chart.resolution),
        format!("TEMPO          {first_tempo:.1} BPM"),
        format!(
            "TIME SIGNATURE {}/{}",
            signature.numerator, signature.denominator
        ),
        format!("DENOMINATOR    {}", signature.denominator),
        format!("TEMPO TEXT     {tempo_text}"),
        format!("REHEARSAL      {rehearsal}"),
        format!("ADD MARKER     {marker_name} at score end"),
        format!("CLEF           {clef}"),
        format!("KEY SIGNATURE  {key_text}"),
        format!(
            "KEY MODE       {}",
            key.map_or("Default", |key| match key.mode {
                KeyMode::Major => "Major",
                KeyMode::Minor => "Minor",
            })
        ),
        format!("CAPO           {capo}"),
        format!("TUNING         {tuning}"),
        format!("KIT            {kit}"),
        format!(
            "ADD EXPRESSION {} at score end",
            score_expression_name(document.expression_kind)
        ),
        format!(
            "VOCAL RANGE LOW  {}",
            vocal_range.split('-').next().unwrap_or("N/A")
        ),
        format!(
            "VOCAL RANGE HIGH {}",
            vocal_range.rsplit('-').next().unwrap_or("N/A")
        ),
    ];
    let rows = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            format!(
                "{} {}",
                if index == document.metadata_focus {
                    ">"
                } else {
                    " "
                },
                row
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let text_entry = document
        .text_entry
        .as_ref()
        .map_or_else(String::new, |entry| {
            format!("\n\nEDIT: {}_\nEnter: commit  Esc: cancel", entry.buffer)
        });
    Text::new(format!(
        "OPEN BAND  //  SCORE METADATA  //  {}{}\n\n{rows}\n\n\
        Left/Right: adjust  Enter: edit/add  Delete: remove  Up/Down: field\n\
        Tab: snap grid {}  M/Esc: tracks  S: save\n\n{}{}",
        chart.title,
        if document.dirty { "*" } else { "" },
        snap_interval_label(document.snap_interval),
        score_summary(chart),
        text_entry,
    ))
}

/// Summarize the document's score structure without flattening voices or rests.
///
/// AI_CODE
fn score_summary(chart: &Chart) -> String {
    let mut voices = HashSet::new();
    let mut staves = HashSet::new();
    let mut event_count = 0usize;
    let mut rest_count = 0usize;
    let mut missing_ids = 0usize;
    let mut performance_hints = 0usize;
    for track in &chart.tracks {
        for event in track
            .notes
            .iter()
            .chain(track.phrases.iter().flat_map(|phrase| phrase.notes.iter()))
            .chain(
                track
                    .lyric_verses
                    .iter()
                    .flat_map(|verse| verse.phrases.iter())
                    .flat_map(|phrase| phrase.notes.iter()),
            )
        {
            event_count += 1;
            voices.insert(event.voice);
            staves.insert(event.staff);
            rest_count += usize::from(matches!(event.content, NoteContent::Rest));
            missing_ids += usize::from(event.id.is_none());
            performance_hints += usize::from(event.performance.is_some());
        }
    }
    let measure_count = crate::notation::measures(chart, chart.total_ticks()).len();
    format!(
        "SCORE  {measure_count} measures  |  {} tracks  |  {} events  |  {rest_count} rests  |  {} voices / {} staves  |  {} markers  |  {} expressions  |  {missing_ids} missing IDs  |  {performance_hints} performance hints",
        chart.tracks.len(),
        event_count,
        voices.len(),
        staves.len(),
        chart.structure.len(),
        chart.expressions.len(),
    )
}

/// AI_CODE
fn score_marker_name(index: usize) -> &'static str {
    [
        "Repeat start",
        "Repeat end",
        "Ending start",
        "Ending end",
        "Segno",
        "Coda",
        "Fine",
        "Da capo",
        "Dal segno",
        "Rehearsal mark",
    ]
    .get(index)
    .copied()
    .unwrap_or("Repeat start")
}

/// AI_CODE
fn score_expression_name(index: usize) -> &'static str {
    ["Accelerando", "Ritardando", "Crescendo", "Diminuendo"]
        .get(index)
        .copied()
        .unwrap_or("Accelerando")
}
