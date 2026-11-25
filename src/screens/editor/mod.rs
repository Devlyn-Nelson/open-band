use crate::*;
use bevy::input::keyboard::KeyboardInput;
use bevy::ui_widgets::{Activate, Button};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

mod sheet;
pub(crate) use sheet::sheet_view_system;
mod tab;
pub(crate) use tab::{tab_fret_choice_input, tab_fret_dialog_system, tab_view_system};

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
    pub(crate) current_tick: u32,
    pub(crate) view_mode: EditorViewMode,
    pub(crate) event_focus: usize,
    pub(crate) event_text_cursor: Option<usize>,
    pub(crate) text_entry: Option<EditorTextEntry>,
    pub(crate) metadata_mode: bool,
    pub(crate) metadata_focus: usize,
    pub(crate) structure_kind: usize,
    pub(crate) expression_kind: usize,
    pub(crate) snap_interval: SnapInterval,
    pub(crate) tab_beats_per_screen: u8,
    pub(crate) tab_fret_string: Option<usize>,
    pub(crate) tab_fret_tick: u32,
    pub(crate) tab_percussion_lane: Option<usize>,
    pub(crate) tab_resize: Option<TabDurationResize>,
    pub(crate) status: String,
}

/// In-progress tab note-duration drag, retained across pointer frames.
///
/// AI_CODE
#[derive(Clone, Copy)]
pub(crate) struct TabDurationResize {
    pub(crate) source: EventSource,
    pub(crate) start_tick: u32,
    pub(crate) lane: usize,
}

/// The focused track's document-wide representation.
///
/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditorViewMode {
    Sheet,
    Tab,
}

/// Active sheet notation tools for the current editor session.
///
/// AI_CODE
#[derive(Resource)]
pub(crate) struct SheetTools {
    pub(crate) duration: NoteValue,
    pub(crate) dots: u8,
    pub(crate) tuplet: Option<Tuplet>,
    pub(crate) rest: bool,
    pub(crate) tied: bool,
    pub(crate) remove: bool,
    pub(crate) voice: u8,
    pub(crate) dynamic: Option<DynamicLevel>,
    pub(crate) articulation: Option<Articulation>,
    pub(crate) piece_index: usize,
    pub(crate) slur: bool,
    pub(crate) slur_anchor: Option<String>,
}

/// A control in the sheet editor's clickable notation palette.
///
/// AI_CODE
#[derive(Component, Clone, Copy)]
pub(crate) enum SheetToolAction {
    Duration(NoteValue),
    Dots,
    Tuplet,
    Rest,
    Tie,
    Remove,
    Voice,
    Dynamic,
    Articulation,
    Piece,
    Slur,
    TabSnap,
    ZoomIn,
    ZoomOut,
    SheetDenser,
    SheetSpacer,
}

/// A fret button in the tab editor's note-choice dialog.
///
/// AI_CODE
#[derive(Component)]
pub(crate) struct TabFretChoice(pub(crate) u8);

/// Marks fret-choice dialog buttons for editor cleanup.
///
/// AI_CODE
#[derive(Component)]
pub(crate) struct TabFretChoiceButton;

/// Marks toolbar buttons so editor cleanup removes only its own UI.
///
/// AI_CODE
#[derive(Component)]
pub(crate) struct SheetToolbarButton;

impl Default for SheetTools {
    fn default() -> Self {
        Self {
            duration: NoteValue::Quarter,
            dots: 0,
            tuplet: None,
            rest: false,
            tied: false,
            remove: false,
            voice: 1,
            dynamic: None,
            articulation: None,
            piece_index: 0,
            slur: false,
            slur_anchor: None,
        }
    }
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
    ScoreEvent(EventSource),
    NewScoreEvent,
}

/// AI_CODE
pub(crate) struct EditorTextEntry {
    pub(crate) target: EditorTextTarget,
    pub(crate) buffer: String,
}

/// Find the authored event corresponding to a notation symbol.
///
/// AI_CODE
fn editor_source_event_mut(track: &mut ChartTrack, source: EventSource) -> Option<&mut ChartEvent> {
    match source {
        EventSource::Track(index) => track.notes.get_mut(index),
        EventSource::Phrase { phrase, index } => {
            track.phrases.get_mut(phrase)?.notes.get_mut(index)
        }
        EventSource::Verse {
            verse,
            phrase,
            index,
        } => track
            .lyric_verses
            .get_mut(verse)?
            .phrases
            .get_mut(phrase)?
            .notes
            .get_mut(index),
    }
}

/// Find the authored event for a selected notation symbol.
///
/// AI_CODE
fn editor_source_event(track: &ChartTrack, source: EventSource) -> Option<&ChartEvent> {
    match source {
        EventSource::Track(index) => track.notes.get(index),
        EventSource::Phrase { phrase, index } => track.phrases.get(phrase)?.notes.get(index),
        EventSource::Verse {
            verse,
            phrase,
            index,
        } => track
            .lyric_verses
            .get(verse)?
            .phrases
            .get(phrase)?
            .notes
            .get(index),
    }
}

/// Select an event at the shared tick, including stacked notes.
///
/// AI_CODE
fn focused_event_source(document: &EditorDocument) -> Option<EventSource> {
    let track = document.chart.tracks.get(document.track_focus)?;
    let layout = layout_track(&document.chart, track);
    let events = layout
        .symbols
        .iter()
        .filter(|symbol| symbol.start == document.current_tick)
        .collect::<Vec<_>>();
    events
        .get(document.event_focus % events.len().max(1))
        .map(|symbol| symbol.source)
}

/// Remove a selected event and any relationships that refer to its ID.
///
/// AI_CODE
fn remove_editor_event(document: &mut EditorDocument, source: EventSource) -> bool {
    let Some(track) = document.chart.tracks.get_mut(document.track_focus) else {
        return false;
    };
    let events = match source {
        EventSource::Track(_) => Some(&mut track.notes),
        EventSource::Phrase { phrase, .. } => track
            .phrases
            .get_mut(phrase)
            .map(|phrase| &mut phrase.notes),
        EventSource::Verse { verse, phrase, .. } => track
            .lyric_verses
            .get_mut(verse)
            .and_then(|verse| verse.phrases.get_mut(phrase))
            .map(|phrase| &mut phrase.notes),
    };
    let index = match source {
        EventSource::Track(index)
        | EventSource::Phrase { index, .. }
        | EventSource::Verse { index, .. } => index,
    };
    let Some(events) = events else {
        return false;
    };
    if index >= events.len() {
        return false;
    }
    let event = events.remove(index);
    if let Some(id) = event.id {
        track
            .ties
            .retain(|relation| relation.from != id && relation.to != id);
        track
            .slurs
            .retain(|relation| relation.from != id && relation.to != id);
    }
    document.event_focus = 0;
    document.dirty = true;
    true
}

/// Replace an authored event only after the supplied score JSON parses successfully.
///
/// AI_CODE
fn replace_editor_event(
    document: &mut EditorDocument,
    source: EventSource,
    value: &str,
) -> Result<(), String> {
    let mut event: ChartEvent = serde_json::from_str(value).map_err(|error| error.to_string())?;
    let track = document
        .chart
        .tracks
        .get_mut(document.track_focus)
        .ok_or("No focused track")?;
    let target = editor_source_event_mut(track, source).ok_or("Event no longer exists")?;
    event.id = target.id.clone();
    *target = event;
    document.dirty = true;
    Ok(())
}

/// Place or remove a pitched score event at the snapped editor position.
///
/// AI_CODE
fn sheet_edit_pitch(
    document: &mut EditorDocument,
    tick: u32,
    pitch: u8,
    tools: &SheetTools,
) -> bool {
    sheet_edit_spelled_pitch(document, tick, pitch, None, tools)
}

/// Edit a pitch while retaining its written accidental and staff position.
///
/// AI_CODE
fn sheet_edit_spelled_pitch(
    document: &mut EditorDocument,
    tick: u32,
    pitch: u8,
    spelling: Option<PitchSpelling>,
    tools: &SheetTools,
) -> bool {
    let tick = snap_editor_tick(document, tick);
    let Some(track) = document.chart.tracks.get(document.track_focus) else {
        return false;
    };
    if track.kind == InstrumentKind::Percussion && !tools.rest {
        return false;
    }
    let source = layout_track(&document.chart, track).symbols.iter().find(|symbol| {
        symbol.start == tick && symbol.voice == tools.voice && if tools.rest { matches!(symbol.kind, NotationSymbolKind::Rest) } else { matches!(&symbol.kind, NotationSymbolKind::Pitched { pitch: existing, spelling: existing_spelling } if *existing == pitch && (spelling.is_none() || sheet::pitch_step(*existing, existing_spelling.as_ref()) == sheet::pitch_step(pitch, spelling.as_ref()))) }
    }).map(|symbol| symbol.source);
    if tools.remove {
        return source.is_some_and(|source| remove_editor_event(document, source));
    }
    if source.is_some() {
        return false;
    }
    let event = ChartEvent {
        id: None,
        start: tick,
        length: tools.duration,
        voice: tools.voice,
        staff: 1,
        dots: tools.dots,
        tied: tools.tied && !tools.rest,
        tuplet: tools.tuplet,
        chord: None,
        dynamic: tools.dynamic,
        articulations: tools.articulation.into_iter().collect(),
        stem: None,
        grace: None,
        performance: None,
        content: if tools.rest {
            NoteContent::Rest
        } else {
            NoteContent::Pitched {
                note: pitch,
                spelling,
            }
        },
    };
    document.chart.tracks[document.track_focus]
        .notes
        .push(event);
    ensure_stable_event_ids(&mut document.chart);
    let track = &mut document.chart.tracks[document.track_focus];
    let index = track.notes.len() - 1;
    if !tools.rest {
        let compatible = (0..index).find(|existing| {
            let note = &track.notes[*existing];
            note.start == tick
                && note.voice == tools.voice
                && note.staff == 1
                && note.length == tools.duration
                && note.dots == tools.dots
                && note.tuplet == tools.tuplet
                && matches!(note.content, NoteContent::Pitched { .. })
        });
        if let Some(existing) = compatible {
            let chord = track.notes[existing].chord.clone().unwrap_or_else(|| {
                format!(
                    "chord-{}",
                    track.notes[existing].id.as_deref().unwrap_or("note")
                )
            });
            track.notes[existing].chord = Some(chord.clone());
            track.notes[index].chord = Some(chord);
        }
    }
    document.dirty = true;
    true
}

/// Place or remove the selected percussion piece on its configured lane.
///
/// AI_CODE
fn sheet_edit_percussion(
    document: &mut EditorDocument,
    tick: u32,
    lane: usize,
    tools: &SheetTools,
) -> bool {
    let tick = snap_editor_tick(document, tick);
    let Some(track) = document.chart.tracks.get(document.track_focus) else {
        return false;
    };
    let Some(kit) = track.kit.as_ref() else {
        return false;
    };
    let Some(piece) = kit.pieces.get(tools.piece_index % kit.pieces.len().max(1)) else {
        return false;
    };
    if piece.lane != Some(lane) && piece.lane_span.is_none() {
        return false;
    }
    let piece_name = piece.name.clone();
    let source = layout_track(&document.chart, track).symbols.iter().find(|symbol| symbol.start == tick && symbol.voice == tools.voice && matches!(&symbol.kind, NotationSymbolKind::Percussion { piece, .. } if piece == &piece_name)).map(|symbol| symbol.source);
    if tools.remove {
        return source.is_some_and(|source| remove_editor_event(document, source));
    }
    if source.is_some() {
        return false;
    }
    let event = ChartEvent {
        id: None,
        start: tick,
        length: tools.duration,
        voice: tools.voice,
        staff: 1,
        dots: tools.dots,
        tied: tools.tied,
        tuplet: tools.tuplet,
        chord: None,
        dynamic: tools.dynamic,
        articulations: tools.articulation.into_iter().collect(),
        stem: None,
        grace: None,
        performance: None,
        content: NoteContent::Percussive { piece: piece_name },
    };
    document.chart.tracks[document.track_focus]
        .notes
        .push(event);
    ensure_stable_event_ids(&mut document.chart);
    document.dirty = true;
    true
}

/// Select two existing pitched events to author a stable-ID slur relationship.
///
/// AI_CODE
pub(super) fn sheet_edit_slur(
    document: &mut EditorDocument,
    tick: u32,
    pitch: u8,
    tools: &mut SheetTools,
) -> bool {
    ensure_stable_event_ids(&mut document.chart);
    let target_id = {
        let Some(track) = document.chart.tracks.get(document.track_focus) else {
            return false;
        };
        let layout = layout_track(&document.chart, track);
        let Some(source) = layout
            .symbols
            .iter()
            .find(|symbol| {
                symbol.start == tick
                    && symbol.voice == tools.voice
                    && matches!(symbol.kind, NotationSymbolKind::Pitched { pitch: value, .. } if value == pitch)
            })
            .map(|symbol| symbol.source)
        else {
            return false;
        };
        let Some(id) = editor_source_event(track, source).and_then(|event| event.id.clone()) else {
            return false;
        };
        id
    };

    let Some(anchor) = tools.slur_anchor.take() else {
        tools.slur_anchor = Some(target_id);
        document.status = "Slur start selected; choose its ending note".into();
        return true;
    };
    if anchor == target_id {
        tools.slur_anchor = Some(anchor);
        document.status = "Choose a different ending note for the slur".into();
        return false;
    }
    let Some(track) = document.chart.tracks.get_mut(document.track_focus) else {
        return false;
    };
    if !track
        .slurs
        .iter()
        .any(|relation| relation.from == anchor && relation.to == target_id)
    {
        track.slurs.push(EventRelation {
            from: anchor,
            to: target_id,
        });
    }
    tools.slur = false;
    document.status = "Slur added".into();
    document.dirty = true;
    true
}

#[test]
/// Slur selection links two authored notes by stable event ID.
///
/// AI_CODE
fn sheet_slur_tool_authors_stable_event_relation() {
    let mut document = new_chart_document();
    document
        .chart
        .tracks
        .push(new_track(InstrumentKind::Strings));
    let mut tools = SheetTools::default();
    assert!(sheet_edit_pitch(&mut document, 0, 60, &tools));
    assert!(sheet_edit_pitch(&mut document, 960, 62, &tools));

    tools.slur = true;
    assert!(sheet_edit_slur(&mut document, 0, 60, &mut tools));
    assert!(tools.slur_anchor.is_some());
    assert!(sheet_edit_slur(&mut document, 960, 62, &mut tools));
    assert!(!tools.slur);
    assert_eq!(document.chart.tracks[0].slurs.len(), 1);
    assert_ne!(
        document.chart.tracks[0].slurs[0].from,
        document.chart.tracks[0].slurs[0].to
    );
    assert_eq!(
        layout_track(&document.chart, &document.chart.tracks[0])
            .slurs
            .len(),
        1
    );
}

#[test]
/// A percussion click targets only the selected kit piece on its configured lane.
///
/// AI_CODE
fn sheet_percussion_targets_selected_kit_piece() {
    let mut document = new_chart_document();
    document
        .chart
        .tracks
        .push(new_track(InstrumentKind::Percussion));
    let piece = document.chart.tracks[0]
        .kit
        .as_ref()
        .unwrap()
        .pieces
        .iter()
        .position(|piece| piece.lane.is_some())
        .unwrap();
    let lane = document.chart.tracks[0].kit.as_ref().unwrap().pieces[piece]
        .lane
        .unwrap();
    let mut tools = SheetTools::default();
    tools.piece_index = piece;
    assert!(!sheet_edit_percussion(&mut document, 0, lane + 1, &tools));
    assert!(sheet_edit_percussion(&mut document, 0, lane, &tools));
    tools.remove = true;
    assert!(sheet_edit_percussion(&mut document, 0, lane, &tools));
    assert!(document.chart.tracks[0].notes.is_empty());
    let span = document.chart.tracks[0]
        .kit
        .as_ref()
        .unwrap()
        .pieces
        .iter()
        .position(|piece| piece.lane_span.is_some())
        .unwrap();
    tools.piece_index = span;
    tools.remove = false;
    assert!(sheet_edit_percussion(&mut document, 0, lane, &tools));
    tools.remove = true;
    assert!(sheet_edit_percussion(&mut document, 0, lane, &tools));
}

#[test]
/// Sheet placement stacks pitches without replacing notes and removes one pitch only.
///
/// AI_CODE
fn sheet_pitch_placement_stacks_and_removes_one_note() {
    let mut document = new_chart_document();
    document.chart.tracks.push(new_track(InstrumentKind::Voice));
    let mut tools = SheetTools::default();
    assert!(sheet_edit_pitch(&mut document, 1000, 60, &tools));
    assert!(sheet_edit_pitch(&mut document, 1000, 64, &tools));
    assert!(!sheet_edit_pitch(&mut document, 1000, 60, &tools));
    assert_eq!(document.chart.tracks[0].notes.len(), 2);
    assert!(document.chart.tracks[0].notes[0].chord.is_some());
    assert_eq!(
        document.chart.tracks[0].notes[0].chord,
        document.chart.tracks[0].notes[1].chord
    );
    assert!(
        document.chart.tracks[0]
            .notes
            .iter()
            .all(|note| note.start == 960 && note.id.is_some())
    );
    tools.remove = true;
    assert!(sheet_edit_pitch(&mut document, 1000, 60, &tools));
    assert_eq!(document.chart.tracks[0].notes.len(), 1);
    assert!(matches!(
        document.chart.tracks[0].notes[0].content,
        NoteContent::Pitched { note: 64, .. }
    ));
}

#[test]
/// Sheet placement preserves notation settings and records rests on percussion tracks.
///
/// AI_CODE
fn sheet_placement_preserves_tool_settings() {
    let mut document = new_chart_document();
    document.chart.tracks.push(new_track(InstrumentKind::Voice));
    let mut tools = SheetTools::default();
    tools.duration = NoteValue::Eighth;
    tools.dots = 1;
    tools.tuplet = Some(Tuplet {
        actual: 3,
        normal: 2,
    });
    tools.voice = 2;
    tools.dynamic = Some(DynamicLevel::Mf);
    tools.articulation = Some(Articulation::Accent);
    assert!(sheet_edit_pitch(&mut document, 1000, 60, &tools));
    let event = &document.chart.tracks[0].notes[0];
    assert_eq!(event.start, 960);
    assert_eq!(event.length, NoteValue::Eighth);
    assert_eq!(event.dots, 1);
    assert_eq!(event.tuplet, tools.tuplet);
    assert_eq!(event.voice, 2);
    assert_eq!(event.dynamic, Some(DynamicLevel::Mf));
    assert_eq!(event.articulations, vec![Articulation::Accent]);
    document
        .chart
        .tracks
        .push(new_track(InstrumentKind::Percussion));
    document.track_focus = 1;
    tools.rest = true;
    tools.tied = true;
    assert!(sheet_edit_pitch(&mut document, 0, 0, &tools));
    assert!(matches!(
        document.chart.tracks[1].notes[0].content,
        NoteContent::Rest
    ));
    assert!(!document.chart.tracks[1].notes[0].tied);
}

#[test]
/// Editing a focused event preserves other tracks and document navigation.
///
/// AI_CODE
fn editor_replaces_only_the_focused_source_event() {
    let mut document = new_chart_document();
    document.chart.tracks = vec![
        new_track(InstrumentKind::Strings),
        new_track(InstrumentKind::Voice),
    ];
    document.chart.tracks[0].notes =
        serde_json::from_str(r#"[{"start":0,"length":4,"note":"E2"}]"#).unwrap();
    document.chart.tracks[1].phrases = serde_json::from_str(r#"[{"start":0,"duration_ticks":960,"text":"la","notes":[{"id":"v1","start":0,"length":4,"note":"C4"}]}]"#).unwrap();
    document.track_focus = 1;
    document.current_tick = 480;
    document.view_mode = EditorViewMode::Tab;
    document.dirty = false;
    let source = EventSource::Phrase {
        phrase: 0,
        index: 0,
    };
    assert!(replace_editor_event(&mut document, source, "invalid").is_err());
    assert!(!document.dirty);
    replace_editor_event(
        &mut document,
        source,
        r#"{"id":"v1","start":960,"length":8,"note":"D4"}"#,
    )
    .unwrap();
    assert_eq!(document.chart.tracks[1].phrases[0].notes[0].start, 960);
    assert_eq!(document.chart.tracks[0].notes[0].start, 0);
    assert_eq!(document.current_tick, 480);
    assert_eq!(document.view_mode, EditorViewMode::Tab);
    assert!(document.dirty);
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
        current_tick: 0,
        view_mode: EditorViewMode::Sheet,
        event_focus: 0,
        event_text_cursor: None,
        text_entry: None,
        metadata_mode: false,
        metadata_focus: 0,
        structure_kind: 0,
        expression_kind: 0,
        snap_interval: NoteValue::Quarter,
        tab_beats_per_screen: 8,
        tab_fret_string: None,
        tab_fret_tick: 0,
        tab_percussion_lane: None,
        tab_resize: None,
        status: "New chart".into(),
    }
}

/// Ensure the graphical editor always has a focused track to render and edit.
///
/// AI_CODE
fn ensure_editor_has_track(document: &mut EditorDocument) -> bool {
    if !document.chart.tracks.is_empty() {
        return false;
    }
    document
        .chart
        .tracks
        .push(new_track(InstrumentKind::Strings));
    document.track_focus = 0;
    document.dirty = true;
    true
}

/// Remove the focused track only when another editable track will remain.
///
/// AI_CODE
fn remove_focused_track(document: &mut EditorDocument) -> bool {
    if document.chart.tracks.len() <= 1 || document.track_focus >= document.chart.tracks.len() {
        return false;
    }
    document.chart.tracks.remove(document.track_focus);
    document.track_focus = document
        .track_focus
        .min(document.chart.tracks.len().saturating_sub(1));
    document.event_focus = 0;
    document.dirty = true;
    true
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
    let added_track = if chart.tracks.is_empty() {
        chart.tracks.push(new_track(InstrumentKind::Strings));
        true
    } else {
        false
    };
    Ok(EditorDocument {
        chart,
        path: Some(path.to_path_buf()),
        dirty: assigned_ids || added_track,
        track_focus: 0,
        current_tick: 0,
        view_mode: EditorViewMode::Sheet,
        event_focus: 0,
        event_text_cursor: None,
        text_entry: None,
        metadata_mode: false,
        metadata_focus: 0,
        structure_kind: 0,
        expression_kind: 0,
        snap_interval: NoteValue::Quarter,
        tab_beats_per_screen: 8,
        tab_fret_string: None,
        tab_fret_tick: 0,
        tab_percussion_lane: None,
        tab_resize: None,
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
        EditorTextTarget::ScoreEvent(_) | EditorTextTarget::NewScoreEvent => return,
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

/// Cycle the focused track without changing the document position or view.
///
/// AI_CODE
fn cycle_editor_track(document: &mut EditorDocument, delta: i32) {
    let count = document.chart.tracks.len();
    if count > 0 {
        document.track_focus =
            (document.track_focus as i64 + i64::from(delta)).rem_euclid(count as i64) as usize;
        document.event_focus = 0;
    }
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

#[test]
/// New or trackless charts acquire one renderable Strings track and cannot delete the last track.
///
/// AI_CODE
fn empty_editor_chart_gets_a_focused_track_and_keeps_one() {
    let mut document = new_chart_document();
    assert!(document.chart.tracks.is_empty());
    assert!(ensure_editor_has_track(&mut document));
    assert_eq!(document.chart.tracks.len(), 1);
    assert_eq!(document.track_focus, 0);
    assert_eq!(document.chart.tracks[0].kind, InstrumentKind::Strings);
    assert!(document.chart.tracks[0].tuning.is_some());
    assert!(document.dirty);
    assert!(!ensure_editor_has_track(&mut document));
    assert!(!remove_focused_track(&mut document));

    document.chart.tracks.push(new_track(InstrumentKind::Voice));
    assert!(remove_focused_track(&mut document));
    assert_eq!(document.chart.tracks.len(), 1);
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
    commands.insert_resource(SheetTools::default());
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
            top: Val::Px(122.0),
            left: Val::Px(70.0),
            right: Val::Px(40.0),
            ..default()
        },
        EditorText,
    ));
    // \\/ AI_CODE \\
    let toolbar = [
        ("WHOLE", SheetToolAction::Duration(NoteValue::Whole)),
        ("HALF", SheetToolAction::Duration(NoteValue::Half)),
        ("QUARTER", SheetToolAction::Duration(NoteValue::Quarter)),
        ("EIGHTH", SheetToolAction::Duration(NoteValue::Eighth)),
        ("16TH", SheetToolAction::Duration(NoteValue::Sixteenth)),
        ("DOT", SheetToolAction::Dots),
        ("TRIPLET", SheetToolAction::Tuplet),
        ("REST", SheetToolAction::Rest),
        ("TIE", SheetToolAction::Tie),
        ("PLACE/REMOVE", SheetToolAction::Remove),
        ("VOICE", SheetToolAction::Voice),
        ("DYNAMIC", SheetToolAction::Dynamic),
        ("ARTIC", SheetToolAction::Articulation),
        ("PIECE", SheetToolAction::Piece),
        ("SLUR", SheetToolAction::Slur),
        ("SNAP", SheetToolAction::TabSnap),
        ("ZOOM +", SheetToolAction::ZoomIn),
        ("ZOOM -", SheetToolAction::ZoomOut),
        ("+ BARS", SheetToolAction::SheetDenser),
        ("- BARS", SheetToolAction::SheetSpacer),
    ];
    for (index, (label, action)) in toolbar.into_iter().enumerate() {
        let column = index % 7;
        let row = index / 7;
        commands.spawn((
            Button::default(),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0 + row as f32 * 34.0),
                left: Val::Px(12.0 + column as f32 * 88.0),
                width: Val::Px(82.0),
                height: Val::Px(28.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.13, 0.18, 0.19)),
            BorderColor::all(Color::srgb(0.36, 0.48, 0.46)),
            Text::new(label),
            TextFont {
                font_size: FontSize::Px(12.0),
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.95, 0.9)),
            action,
            SheetToolbarButton,
        ));
    }
    // \\/ AI_CODE \\
    for fret in 0..=24u8 {
        let column = usize::from(fret % 5);
        let row = usize::from(fret / 5);
        commands.spawn((
            Button::default(),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(150.0 + row as f32 * 38.0),
                left: Val::Percent(35.0 + column as f32 * 7.0),
                width: Val::Px(58.0),
                height: Val::Px(32.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.13, 0.18, 0.19)),
            Text::new(fret.to_string()),
            TextFont {
                font_size: FontSize::Px(12.0),
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.95, 0.9)),
            Visibility::Hidden,
            TabFretChoice(fret),
            TabFretChoiceButton,
        ));
    }
    // /\\ AI_CODE /\\
}

/// AI_CODE
pub(crate) fn cleanup_editor(
    mut commands: Commands,
    entities: Query<Entity, With<EditorText>>,
    toolbar: Query<Entity, With<SheetToolbarButton>>,
    labels: Query<Entity, With<sheet::SheetEngravingLabel>>,
    fret_choices: Query<Entity, With<TabFretChoiceButton>>,
    tab_labels: Query<Entity, With<tab::TabSymbolLabel>>,
    tab_note_badges: Query<Entity, With<tab::TabNoteBadge>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    for entity in &toolbar {
        commands.entity(entity).despawn();
    }
    // \\/ AI_CODE \\
    for entity in &labels {
        commands.entity(entity).despawn();
    }
    for entity in &fret_choices {
        commands.entity(entity).despawn();
    }
    for entity in &tab_labels {
        commands.entity(entity).despawn();
    }
    for entity in &tab_note_badges {
        commands.entity(entity).despawn();
    }
    // /\\ AI_CODE /\\
    commands.remove_resource::<EditorDocument>();
    commands.remove_resource::<EditorBrowser>();
    commands.remove_resource::<EditorLibraries>();
    commands.remove_resource::<SheetTools>();
}

/// Apply clicks from the sheet notation palette to the active editor tools.
///
/// AI_CODE
pub(crate) fn sheet_toolbar_input(
    activate: On<Activate>,
    mut document: ResMut<EditorDocument>,
    mut tools: ResMut<SheetTools>,
    mut settings: ResMut<PersistentSettings>,
    actions: Query<&SheetToolAction>,
) {
    if document.metadata_mode || document.text_entry.is_some() {
        return;
    }
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };
    match action {
        SheetToolAction::Duration(value) => tools.duration = *value,
        SheetToolAction::Dots => tools.dots = (tools.dots + 1) % 4,
        SheetToolAction::Tuplet => {
            tools.tuplet = if tools.tuplet.is_some() {
                None
            } else {
                Some(Tuplet {
                    actual: 3,
                    normal: 2,
                })
            };
        }
        SheetToolAction::Rest => tools.rest = !tools.rest,
        SheetToolAction::Tie => tools.tied = !tools.tied,
        SheetToolAction::Remove => tools.remove = !tools.remove,
        SheetToolAction::Voice => tools.voice = tools.voice % 4 + 1,
        SheetToolAction::Dynamic => {
            tools.dynamic = match tools.dynamic {
                None => Some(DynamicLevel::Ppp),
                Some(DynamicLevel::Ppp) => Some(DynamicLevel::Pp),
                Some(DynamicLevel::Pp) => Some(DynamicLevel::Mp),
                Some(DynamicLevel::Mp) => Some(DynamicLevel::Mf),
                Some(DynamicLevel::Mf) => Some(DynamicLevel::F),
                Some(DynamicLevel::F) => Some(DynamicLevel::Ff),
                Some(DynamicLevel::Ff) => Some(DynamicLevel::Fff),
                Some(DynamicLevel::Fff) => None,
            };
        }
        SheetToolAction::Articulation => {
            tools.articulation = match tools.articulation {
                None => Some(Articulation::Staccato),
                Some(Articulation::Staccato) => Some(Articulation::Tenuto),
                Some(Articulation::Tenuto) => Some(Articulation::Marcato),
                Some(Articulation::Marcato) => Some(Articulation::Accent),
                Some(Articulation::Accent) => Some(Articulation::Fermata),
                _ => None,
            };
        }
        SheetToolAction::Piece => {
            if let Some(track) = document.chart.tracks.get(document.track_focus)
                && let Some(kit) = track.kit.as_ref()
                && !kit.pieces.is_empty()
            {
                tools.piece_index = (tools.piece_index + 1) % kit.pieces.len();
            }
        }
        SheetToolAction::Slur => {
            tools.slur = !tools.slur;
            tools.slur_anchor = None;
        }
        SheetToolAction::TabSnap if document.view_mode == EditorViewMode::Tab => {
            document.snap_interval = cycle_snap_interval(document.snap_interval, 1);
        }
        SheetToolAction::ZoomIn if document.view_mode == EditorViewMode::Tab => {
            document.tab_beats_per_screen = document.tab_beats_per_screen.saturating_sub(1).max(2);
        }
        SheetToolAction::ZoomOut if document.view_mode == EditorViewMode::Tab => {
            document.tab_beats_per_screen = document.tab_beats_per_screen.saturating_add(1).min(32);
        }
        SheetToolAction::SheetDenser if document.view_mode == EditorViewMode::Sheet => {
            settings.sheet_pixels_per_beat = settings
                .sheet_pixels_per_beat
                .saturating_sub(15)
                .max(SHEET_PIXELS_PER_BEAT_MIN);
            save_settings(&settings);
        }
        SheetToolAction::SheetSpacer if document.view_mode == EditorViewMode::Sheet => {
            settings.sheet_pixels_per_beat = settings
                .sheet_pixels_per_beat
                .saturating_add(15)
                .min(SHEET_PIXELS_PER_BEAT_MAX);
            save_settings(&settings);
        }
        SheetToolAction::TabSnap | SheetToolAction::ZoomIn | SheetToolAction::ZoomOut => {}
        SheetToolAction::SheetDenser | SheetToolAction::SheetSpacer => {}
    }
}

/// Keep toolbar visibility and active-state colors in sync with the current sheet tools.
///
/// AI_CODE
pub(crate) fn sheet_toolbar_display(
    document: Res<EditorDocument>,
    tools: Res<SheetTools>,
    settings: Res<PersistentSettings>,
    mut buttons: Query<(
        &SheetToolAction,
        &mut BackgroundColor,
        &mut Visibility,
        &mut Text,
    )>,
) {
    let visible = !document.metadata_mode && document.text_entry.is_none();
    for (action, mut background, mut visibility, mut text) in &mut buttons {
        let action_visible = match action {
            SheetToolAction::TabSnap | SheetToolAction::ZoomIn | SheetToolAction::ZoomOut => {
                document.view_mode == EditorViewMode::Tab
            }
            SheetToolAction::SheetDenser | SheetToolAction::SheetSpacer => {
                document.view_mode == EditorViewMode::Sheet
            }
            SheetToolAction::Piece => {
                document.view_mode == EditorViewMode::Sheet
                    || document.view_mode == EditorViewMode::Tab
                        && document
                            .chart
                            .tracks
                            .get(document.track_focus)
                            .is_some_and(|track| track.kind == InstrumentKind::Percussion)
            }
            _ => document.view_mode == EditorViewMode::Sheet,
        };
        *visibility = if visible && action_visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        let active = match action {
            SheetToolAction::Duration(value) => tools.duration == *value,
            SheetToolAction::Dots => tools.dots > 0,
            SheetToolAction::Tuplet => tools.tuplet.is_some(),
            SheetToolAction::Rest => tools.rest,
            SheetToolAction::Tie => tools.tied,
            SheetToolAction::Remove => tools.remove,
            SheetToolAction::Voice => false,
            SheetToolAction::Dynamic => tools.dynamic.is_some(),
            SheetToolAction::Articulation => tools.articulation.is_some(),
            SheetToolAction::Piece => false,
            SheetToolAction::Slur => tools.slur,
            SheetToolAction::TabSnap => false,
            SheetToolAction::ZoomIn | SheetToolAction::ZoomOut => false,
            SheetToolAction::SheetDenser | SheetToolAction::SheetSpacer => false,
        };
        match action {
            SheetToolAction::TabSnap => {
                *text = Text::new(format!("SNAP {:?}", document.snap_interval));
            }
            SheetToolAction::ZoomIn => {
                *text = Text::new(format!("ZOOM +  {}b", document.tab_beats_per_screen));
            }
            SheetToolAction::ZoomOut => {
                *text = Text::new(format!("ZOOM -  {}b", document.tab_beats_per_screen));
            }
            SheetToolAction::SheetDenser => {
                *text = Text::new(format!("+ BARS {}px", settings.sheet_pixels_per_beat));
            }
            SheetToolAction::SheetSpacer => {
                *text = Text::new(format!("- BARS {}px", settings.sheet_pixels_per_beat));
            }
            SheetToolAction::Piece if document.view_mode == EditorViewMode::Tab => {
                let name = document
                    .chart
                    .tracks
                    .get(document.track_focus)
                    .and_then(|track| track.kit.as_ref())
                    .and_then(|kit| kit.pieces.get(tools.piece_index % kit.pieces.len().max(1)))
                    .map_or_else(|| "PIECE".into(), |piece| piece.name.clone());
                *text = Text::new(name);
            }
            _ => {}
        }
        *background = BackgroundColor(if active {
            Color::srgb(0.12, 0.42, 0.34)
        } else {
            Color::srgb(0.13, 0.18, 0.19)
        });
    }
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
        let mut document = new_chart_document();
        ensure_editor_has_track(&mut document);
        document.status = "New chart with a Strings track".into();
        commands.insert_resource(document);
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
/// Track focus wraps across instrument kinds without changing document navigation.
///
/// AI_CODE
fn editor_track_focus_preserves_shared_position_and_view() {
    let mut document = new_chart_document();
    document.chart.tracks = vec![
        new_track(InstrumentKind::Strings),
        new_track(InstrumentKind::Percussion),
        new_track(InstrumentKind::Voice),
    ];
    document.current_tick = 4320;
    document.view_mode = EditorViewMode::Tab;
    document.dirty = false;
    cycle_editor_track(&mut document, -1);
    assert_eq!(document.track_focus, 2);
    cycle_editor_track(&mut document, 1);
    assert_eq!(document.track_focus, 0);
    cycle_editor_track(&mut document, 1);
    assert_eq!(document.track_focus, 1);
    assert_eq!(document.current_tick, 4320);
    assert_eq!(document.view_mode, EditorViewMode::Tab);
    assert!(!document.dirty);
    document.chart.tracks.clear();
    document.track_focus = 0;
    cycle_editor_track(&mut document, -1);
    assert_eq!(document.track_focus, 0);
}

#[test]
/// Navigation keys update shared state through the Bevy editor system.
///
/// AI_CODE
fn editor_navigation_keys_update_shared_state() {
    let mut document = new_chart_document();
    document.chart.tracks = vec![
        new_track(InstrumentKind::Strings),
        new_track(InstrumentKind::Percussion),
        new_track(InstrumentKind::Voice),
    ];
    document.chart.tracks[0].name = "Guitar".into();
    document.chart.tracks[1].name = "Drums".into();
    document.dirty = false;
    let mut app = App::new();
    app.add_message::<KeyboardInput>()
        .insert_resource(ButtonInput::<KeyCode>::default())
        .insert_resource(document)
        .insert_resource(EditorLibraries {
            tunings: Vec::new(),
            kits: Vec::new(),
        })
        .add_systems(Update, editor_tracks_input);
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::PageDown);
        keys.press(KeyCode::ArrowRight);
        keys.press(KeyCode::KeyV);
    }
    app.update();
    {
        let document = app.world().resource::<EditorDocument>();
        assert_eq!(document.track_focus, 1);
        assert_eq!(document.current_tick, 960);
        assert_eq!(document.view_mode, EditorViewMode::Tab);
        assert!(!document.dirty);
        let preview = focused_track_preview(document);
        assert!(preview.contains("Drums"));
        assert!(!preview.contains("Guitar"));
        assert!(preview.contains("Tab preview  |  tick 960"));
    }
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.clear();
        keys.release(KeyCode::PageDown);
        keys.press(KeyCode::PageUp);
    }
    app.world_mut()
        .resource_mut::<EditorDocument>()
        .metadata_mode = true;
    app.update();
    let document = app.world().resource::<EditorDocument>();
    assert_eq!(document.track_focus, 0);
    assert_eq!(document.current_tick, 960);
    assert_eq!(document.view_mode, EditorViewMode::Tab);
}

#[test]
/// Event controls reach off-grid chords and retain invalid edits for correction.
///
/// AI_CODE
fn editor_event_controls_preserve_focus_and_relations() {
    let mut document = new_chart_document();
    document.chart.tracks = vec![
        new_track(InstrumentKind::Strings),
        new_track(InstrumentKind::Voice),
    ];
    document.chart.tracks[0].notes = serde_json::from_str(
        r#"[
        {"id":"a","start":120,"length":4,"note":"E2"},
        {"id":"b","start":120,"length":4,"note":"G2"},
        {"id":"c","start":1080,"length":4,"note":"E2"}
    ]"#,
    )
    .unwrap();
    document.chart.tracks[0].slurs.push(EventRelation {
        from: "a".into(),
        to: "c".into(),
    });
    document.chart.tracks[0].ties.push(EventRelation {
        from: "b".into(),
        to: "c".into(),
    });
    document.view_mode = EditorViewMode::Tab;
    document.dirty = false;
    let mut app = App::new();
    app.add_message::<KeyboardInput>()
        .insert_resource(ButtonInput::<KeyCode>::default())
        .insert_resource(document)
        .insert_resource(EditorLibraries {
            tunings: Vec::new(),
            kits: Vec::new(),
        })
        .add_systems(Update, editor_tracks_input);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyO);
    app.update();
    assert_eq!(app.world().resource::<EditorDocument>().current_tick, 120);
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.clear();
        keys.press(KeyCode::KeyJ);
        keys.press(KeyCode::KeyE);
    }
    app.update();
    {
        let mut document = app.world_mut().resource_mut::<EditorDocument>();
        assert!(matches!(
            document.text_entry.as_ref().unwrap().target,
            EditorTextTarget::ScoreEvent(EventSource::Track(1))
        ));
        document.text_entry.as_mut().unwrap().buffer = "invalid".into();
        document.event_text_cursor = Some(7);
    }
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.clear();
        keys.press(KeyCode::Enter);
    }
    app.update();
    assert!(
        app.world()
            .resource::<EditorDocument>()
            .text_entry
            .is_some()
    );
    assert!(!app.world().resource::<EditorDocument>().dirty);
    {
        let mut document = app.world_mut().resource_mut::<EditorDocument>();
        let buffer = r#"{"id":"changed","start":120,"length":8,"note":"A2"}"#.to_string();
        document.event_text_cursor = Some(buffer.len());
        document.text_entry.as_mut().unwrap().buffer = buffer;
    }
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release(KeyCode::Enter);
        keys.clear();
        keys.press(KeyCode::Enter);
    }
    app.update();
    {
        let document = app.world().resource::<EditorDocument>();
        assert!(document.text_entry.is_none());
        assert_eq!(document.chart.tracks[0].notes[1].id.as_deref(), Some("b"));
        assert_eq!(document.chart.tracks[0].notes[1].length, NoteValue::Eighth);
        assert_eq!(document.current_tick, 120);
        assert_eq!(document.view_mode, EditorViewMode::Tab);
    }
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.clear();
        keys.press(KeyCode::Delete);
    }
    app.update();
    let document = app.world().resource::<EditorDocument>();
    assert_eq!(document.chart.tracks[0].notes.len(), 2);
    assert_eq!(document.chart.tracks[0].notes[0].id.as_deref(), Some("a"));
    assert!(document.chart.tracks[0].ties.is_empty());
    assert_eq!(document.chart.tracks[0].slurs.len(), 1);
    assert_eq!(document.current_tick, 120);
    assert_eq!(document.view_mode, EditorViewMode::Tab);
}

#[test]
/// Inserted events receive stable IDs and survive chart serialization.
///
/// AI_CODE
fn editor_inserts_event_and_round_trips_chart() {
    let mut document = new_chart_document();
    document.chart.tracks.push(new_track(InstrumentKind::Voice));
    document.current_tick = 480;
    let mut app = App::new();
    app.add_message::<KeyboardInput>()
        .insert_resource(ButtonInput::<KeyCode>::default())
        .insert_resource(document)
        .insert_resource(EditorLibraries {
            tunings: Vec::new(),
            kits: Vec::new(),
        })
        .add_systems(Update, editor_tracks_input);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyI);
    app.update();
    assert!(matches!(
        app.world()
            .resource::<EditorDocument>()
            .text_entry
            .as_ref()
            .unwrap()
            .target,
        EditorTextTarget::NewScoreEvent
    ));
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.clear();
        keys.press(KeyCode::Enter);
    }
    app.update();
    let document = app.world().resource::<EditorDocument>();
    assert_eq!(document.chart.tracks[0].notes.len(), 1);
    assert!(document.chart.tracks[0].notes[0].id.is_some());
    assert_eq!(document.chart.tracks[0].notes[0].start, 480);
    let json = serde_json::to_string(&document.chart).unwrap();
    let chart: Chart = serde_json::from_str(&json).unwrap();
    assert_eq!(
        chart.tracks[0].notes[0].id,
        document.chart.tracks[0].notes[0].id
    );
    assert_eq!(chart.tracks[0].notes[0].start, 480);
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
    if document.text_entry.is_some() {
        let mut cursor = document.event_text_cursor;
        let entry = document.text_entry.as_mut().unwrap();
        let control =
            keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
        for event in keyboard_events.read() {
            if event.state != bevy::input::ButtonState::Pressed {
                continue;
            }
            if !control && let Some(text) = &event.text {
                let text = text
                    .chars()
                    .filter(|character| !character.is_control())
                    .collect::<String>();
                if let Some(position) = cursor.as_mut() {
                    entry.buffer.insert_str(*position, &text);
                    *position += text.len();
                } else {
                    entry.buffer.push_str(&text);
                }
            }
        }
        if keyboard.just_pressed(KeyCode::KeyA) && control {
            entry.buffer.clear();
            if cursor.is_some() {
                cursor = Some(0);
            }
        } else if let Some(position) = cursor.as_mut() {
            if keyboard.just_pressed(KeyCode::ArrowLeft) {
                *position = entry.buffer[..*position]
                    .char_indices()
                    .last()
                    .map_or(0, |(index, _)| index);
            }
            if keyboard.just_pressed(KeyCode::ArrowRight) && *position < entry.buffer.len() {
                *position += entry.buffer[*position..].chars().next().unwrap().len_utf8();
            }
            if keyboard.just_pressed(KeyCode::Home) {
                *position = 0;
            }
            if keyboard.just_pressed(KeyCode::End) {
                *position = entry.buffer.len();
            }
            if keyboard.just_pressed(KeyCode::Backspace) && *position > 0 {
                let previous = entry.buffer[..*position]
                    .char_indices()
                    .last()
                    .map_or(0, |(index, _)| index);
                entry.buffer.drain(previous..*position);
                *position = previous;
            }
            if keyboard.just_pressed(KeyCode::Delete) && *position < entry.buffer.len() {
                let next = *position + entry.buffer[*position..].chars().next().unwrap().len_utf8();
                entry.buffer.drain(*position..next);
            }
        } else if keyboard.just_pressed(KeyCode::Backspace) {
            entry.buffer.pop();
        }
        document.event_text_cursor = cursor;
        if keyboard.just_pressed(KeyCode::Enter) {
            if let Some(entry) = document.text_entry.take() {
                let result = match entry.target {
                    EditorTextTarget::ScoreEvent(source) => {
                        Some(replace_editor_event(&mut document, source, &entry.buffer))
                    }
                    EditorTextTarget::NewScoreEvent => Some(
                        serde_json::from_str::<ChartEvent>(&entry.buffer)
                            .map_err(|error| error.to_string())
                            .and_then(|mut event| {
                                event.id = None;
                                let focus = document.track_focus;
                                let track = document
                                    .chart
                                    .tracks
                                    .get_mut(focus)
                                    .ok_or("No focused track")?;
                                track.notes.push(event);
                                ensure_stable_event_ids(&mut document.chart);
                                document.dirty = true;
                                Ok(())
                            }),
                    ),
                    _ => None,
                };
                match result {
                    Some(Err(error)) => {
                        document.status = format!("Event edit failed: {error}");
                        document.text_entry = Some(entry);
                    }
                    Some(Ok(())) => {
                        document.status = "Event updated".into();
                        document.event_text_cursor = None;
                    }
                    None => commit_text_entry(&mut document, entry.target, entry.buffer),
                }
            }
        } else if keyboard.just_pressed(KeyCode::Escape) {
            document.text_entry = None;
            document.event_text_cursor = None;
        }
        return;
    }
    keyboard_events.clear();
    if keyboard.just_pressed(KeyCode::Escape) {
        if document.metadata_mode {
            document.metadata_mode = false;
        } else {
            commands.remove_resource::<EditorDocument>();
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::PageUp) {
        cycle_editor_track(&mut document, -1);
    }
    if keyboard.just_pressed(KeyCode::PageDown) {
        cycle_editor_track(&mut document, 1);
    }
    if keyboard.just_pressed(KeyCode::KeyV) {
        document.view_mode = match document.view_mode {
            EditorViewMode::Sheet => EditorViewMode::Tab,
            EditorViewMode::Tab => EditorViewMode::Sheet,
        };
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
    if keyboard.just_pressed(KeyCode::ArrowUp) {
        cycle_editor_track(&mut document, -1);
    }
    if keyboard.just_pressed(KeyCode::ArrowDown) {
        cycle_editor_track(&mut document, 1);
    }
    let step = document
        .snap_interval
        .ticks(document.chart.resolution)
        .max(1);
    if keyboard.just_pressed(KeyCode::ArrowLeft) {
        document.current_tick =
            snap_editor_tick(&document, document.current_tick.saturating_sub(step));
        document.event_focus = 0;
    }
    if keyboard.just_pressed(KeyCode::ArrowRight) {
        document.current_tick =
            snap_editor_tick(&document, document.current_tick.saturating_add(step));
        document.event_focus = 0;
    }
    if let Some(track) = document.chart.tracks.get(document.track_focus) {
        let target = if keyboard.just_pressed(KeyCode::KeyU) {
            all_track_events(track)
                .filter(|event| event.start < document.current_tick)
                .map(|event| event.start)
                .max()
        } else if keyboard.just_pressed(KeyCode::KeyO) {
            all_track_events(track)
                .filter(|event| event.start > document.current_tick)
                .map(|event| event.start)
                .min()
        } else {
            None
        };
        if let Some(tick) = target {
            document.current_tick = tick;
            document.event_focus = 0;
        }
    }
    if keyboard.just_pressed(KeyCode::KeyJ) {
        document.event_focus = document.event_focus.saturating_add(1);
    }
    if keyboard.just_pressed(KeyCode::KeyE)
        && let Some(source) = focused_event_source(&document)
        && let Some(track) = document.chart.tracks.get(document.track_focus)
        && let Some(event) = editor_source_event(track, source)
        && let Ok(buffer) = serde_json::to_string(event)
    {
        document.event_text_cursor = Some(buffer.len());
        document.text_entry = Some(EditorTextEntry {
            target: EditorTextTarget::ScoreEvent(source),
            buffer,
        });
    }
    if keyboard.just_pressed(KeyCode::KeyI)
        && let Some(track) = document.chart.tracks.get(document.track_focus)
    {
        let content = match track.kind {
            InstrumentKind::Strings => {
                serde_json::json!({"note": track.tuning.as_ref().and_then(|tuning| tuning.strings.first()).map_or("C4", String::as_str)})
            }
            InstrumentKind::Voice => serde_json::json!({"note": "C4"}),
            InstrumentKind::Percussion => {
                serde_json::json!({"piece": track.kit.as_ref().and_then(|kit| kit.pieces.first()).map_or("kick", |piece| piece.name.as_str())})
            }
        };
        let mut event = content;
        event["start"] = serde_json::json!(document.current_tick);
        event["length"] = serde_json::json!(4);
        let buffer = event.to_string();
        document.event_text_cursor = Some(buffer.len());
        document.text_entry = Some(EditorTextEntry {
            target: EditorTextTarget::NewScoreEvent,
            buffer,
        });
    }
    if keyboard.just_pressed(KeyCode::Delete)
        && let Some(source) = focused_event_source(&document)
        && remove_editor_event(&mut document, source)
    {
        document.status = "Event removed".into();
    }
    if keyboard.just_pressed(KeyCode::KeyN) {
        document
            .chart
            .tracks
            .push(new_track(InstrumentKind::Strings));
        document.track_focus = document.chart.tracks.len() - 1;
        document.event_focus = 0;
        document.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyX) && track_count > 1 {
        remove_focused_track(&mut document);
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
            document.event_focus = 0;
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

/// Display notation events for the focused track and shared position.
///
/// AI_CODE
fn focused_track_preview(document: &EditorDocument) -> String {
    let Some(track) = document.chart.tracks.get(document.track_focus) else {
        return "No focused track".into();
    };
    let detail = match track.kind {
        InstrumentKind::Strings => track
            .tuning
            .as_ref()
            .map_or_else(|| "no tuning".into(), |tuning| tuning.strings.join("-")),
        InstrumentKind::Percussion => track.kit.as_ref().map_or_else(
            || "no kit".into(),
            |kit| format!("{} pieces", kit.pieces.len()),
        ),
        InstrumentKind::Voice => "voice".into(),
    };
    let end = document
        .current_tick
        .saturating_add(document.chart.resolution.saturating_mul(4));
    let layout = layout_track(&document.chart, track);
    let events = layout
        .symbols
        .iter()
        .filter(|event| event.start >= document.current_tick && event.start < end)
        .collect::<Vec<_>>();
    let count = events.len();
    let selected = focused_event_source(document);
    let first = selected
        .and_then(|source| events.iter().position(|event| event.source == source))
        .unwrap_or(0)
        .saturating_sub(7);
    let mut lines = events
        .into_iter()
        .skip(first)
        .take(8)
        .map(|event| {
            let content = match &event.kind {
                NotationSymbolKind::Pitched { pitch, .. } => midi_note_name(*pitch),
                NotationSymbolKind::Percussion { piece, .. } => piece.clone(),
                NotationSymbolKind::Rest => "rest".into(),
            };
            let hint = if document.view_mode == EditorViewMode::Tab {
                match &event.kind {
                    NotationSymbolKind::Pitched { pitch, .. }
                        if track.kind == InstrumentKind::Strings =>
                    {
                        let string = editor_source_event(track, event.source)
                            .and_then(|event| event.performance.as_ref())
                            .and_then(|performance| performance.preferred_string);
                        match string {
                            Some(string) => {
                                let open = track
                                    .tuning
                                    .as_ref()
                                    .and_then(|tuning| tuning.open_midi().ok())
                                    .and_then(|tuning| tuning.get(string).copied())
                                    .and_then(|open| open.checked_add(track.capo.unwrap_or(0)));
                                match open.and_then(|open| pitch.checked_sub(open)) {
                                    Some(fret) => format!("  string {} fret {fret}", string + 1),
                                    None => format!("  string {} (unplayable)", string + 1),
                                }
                            }
                            None => "  string unassigned".into(),
                        }
                    }
                    NotationSymbolKind::Percussion {
                        lane, lane_span, ..
                    } => match lane {
                        Some(lane) => format!("  lane {}", lane + 1),
                        None if lane_span.is_some() => "  all lanes".into(),
                        None => "  lane unassigned".into(),
                    },
                    _ => String::new(),
                }
            } else {
                String::new()
            };
            let marker = if Some(event.source) == selected {
                ">"
            } else {
                " "
            };
            format!(
                "{marker} {}: {content}  {:?}  voice {} staff {}{hint}",
                event.start, event.value, event.voice, event.staff
            )
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        lines.push("  (no events in this window)".into());
    } else if count > first + lines.len() {
        lines.push(format!("  ... {} more", count - first - lines.len()));
    }
    format!(
        "FOCUS {}/{}: {}  |  {:?}  |  {detail}\n{:?} preview  |  tick {}\n{}",
        document.track_focus + 1,
        document.chart.tracks.len(),
        track.name,
        track.kind,
        document.view_mode,
        document.current_tick,
        lines.join("\n")
    )
}

/// AI_CODE
pub(crate) fn editor_tracks_display(
    document: Res<EditorDocument>,
    tools: Res<SheetTools>,
    mut text: Query<&mut Text, With<EditorText>>,
) {
    if !document.is_changed() && !tools.is_changed() {
        return;
    }
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    if document.metadata_mode {
        *text = score_metadata_display(&document);
        return;
    }
    if document.view_mode == EditorViewMode::Tab && document.text_entry.is_none() {
        let track = document.chart.tracks.get(document.track_focus);
        let detail = track.map_or_else(String::new, |track| match track.kind {
            InstrumentKind::Strings => track
                .tuning
                .as_ref()
                .map_or_else(|| "no tuning".into(), |tuning| tuning.strings.join(" ")),
            InstrumentKind::Percussion => track
                .kit
                .as_ref()
                .and_then(|kit| kit.pieces.get(tools.piece_index % kit.pieces.len().max(1)))
                .map_or_else(
                    || "no piece".into(),
                    |piece| format!("piece: {}", piece.name),
                ),
            InstrumentKind::Voice => "voice track".into(),
        });
        *text = Text::new(format!(
            "{}  //  {}  |  {}  |  tick {}\nVERTICAL TAB  |  {} beats/screen  |  snap {:?}\nWheel: time  +/-: zoom  Click string: choose fret  Drag note end: resize\nPIECE: cycle kit piece  PgUp/PgDn: track  Left/Right: time  V: sheet\n{}",
            document.chart.title,
            track.map_or("No track", |track| track.name.as_str()),
            detail,
            document.current_tick,
            document.tab_beats_per_screen,
            document.snap_interval,
            document.status,
        ));
        return;
    }
    if document.view_mode == EditorViewMode::Sheet && document.text_entry.is_none() {
        let track = document.chart.tracks.get(document.track_focus);
        let name = track.map_or("No track", |track| track.name.as_str());
        let detail = track.map_or_else(String::new, |track| match track.kind {
            InstrumentKind::Strings => track
                .tuning
                .as_ref()
                .map_or_else(|| "no tuning".into(), |tuning| tuning.strings.join(" ")),
            InstrumentKind::Percussion => track
                .kit
                .as_ref()
                .and_then(|kit| kit.pieces.get(tools.piece_index % kit.pieces.len().max(1)))
                .map_or_else(
                    || "no piece".into(),
                    |piece| format!("piece: {}", piece.name),
                ),
            InstrumentKind::Voice => "voice".into(),
        });
        *text = Text::new(format!(
            "{}  //  {}  |  {}  |  tick {}\nF1-F5: {:?}   F6 dots: {}   F7 triplet: {}   F8 rest: {}\nF9 tie: {}   F10: {}   F11 voice: {}   F12 dynamic: {:?}   A: {:?}\nP: percussion piece   PgUp/PgDn: track   Left/Right: time   V: view\n{}",
            document.chart.title,
            name,
            detail,
            document.current_tick,
            tools.duration,
            tools.dots,
            tools.tuplet.is_some(),
            tools.rest,
            tools.tied,
            if tools.remove { "REMOVE" } else { "PLACE" },
            tools.voice,
            tools.dynamic,
            tools.articulation,
            document.status,
        ));
        return;
    }
    let dirty = if document.dirty { "*" } else { "" };
    let text_entry_line = document
        .text_entry
        .as_ref()
        .map_or_else(String::new, |entry| {
            let mut buffer = entry.buffer.clone();
            buffer.insert(document.event_text_cursor.unwrap_or(buffer.len()), '|');
            format!(
                "\n\nEDIT: {buffer}\nEnter: confirm  Esc: cancel  Ctrl+A: clear\n{}",
                document.status
            )
        });
    *text = Text::new(format!(
        "OPEN BAND  //  CHART EDITOR  //  {}{dirty}\n\n{}{text_entry_line}\n\n\
        PgUp/PgDn or Up/Down: focus     Left/Right: position     V: sheet/tab\n\
        U/O: previous/next event     J: select stacked event\n\
        E: edit event     I: insert event     Delete: remove event\n\
        N: add     X: remove     K: cycle kind\n\
        [ / ]: cycle tuning/kit     , / .: reorder     R: rename     M: score metadata\n\
        Tab: snap grid {}     S: save     Esc: back to chart list\n\n{}\n{}",
        document.chart.title,
        focused_track_preview(&document),
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
