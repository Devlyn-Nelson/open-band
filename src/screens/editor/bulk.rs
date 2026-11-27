use super::*;

/// A focused-track onset range; both snapped endpoints are included. AI_CODE
#[derive(Clone, Copy)]
pub(super) struct TabSelection {
    pub(super) track: usize,
    pub(super) anchor: u32,
    pub(super) end: u32,
}

/// A pointer move previews range bounds without mutating the authored score until release. AI_CODE
pub(super) struct SelectionMove {
    sources: Vec<EventSource>,
    selection: TabSelection,
    pointer: u32,
    delta: i64,
}

/// Start a time move only inside an existing nonempty range on the focused track. AI_CODE
pub(super) fn begin_move(document: &mut EditorDocument, tick: u32) -> bool {
    let Some(selection) = document.selection.filter(|selection| selection.track == document.track_focus
        && tick >= selection.anchor.min(selection.end) && tick <= selection.anchor.max(selection.end))
    else { return false };
    let sources = selected_sources(document);
    if sources.is_empty() { return false; }
    document.selection_move = Some(SelectionMove { sources, selection, pointer: tick, delta: 0 });
    document.tab_resize = None;
    document.selection_drag = false;
    document.playback.playing = false;
    true
}

/// Cancel a pending move without changing the document or losing the original range. AI_CODE
pub(super) fn cancel_move(document: &mut EditorDocument) {
    if let Some(moving) = document.selection_move.take() {
        document.selection = Some(moving.selection);
        document.status = "Move canceled".into();
    }
}

/// Return an authored note's ghost onset while a selection move is pending. AI_CODE
pub(super) fn move_preview_tick(document: &EditorDocument, source: EventSource, start: u32) -> Option<u32> {
    let moving = document.selection_move.as_ref()?;
    moving.sources.contains(&source).then(|| (i64::from(start) + moving.delta) as u32)
}

/// Preview and commit one snapped time translation, clamping the range at timeline limits. AI_CODE
pub(super) fn update_move(document: &mut EditorDocument, tick: u32, finish: bool) {
    let Some(mut moving) = document.selection_move.take() else { return };
    let start = moving.selection.anchor.min(moving.selection.end);
    let end = moving.selection.anchor.max(moving.selection.end);
    moving.delta = (i64::from(tick) - i64::from(moving.pointer))
        .clamp(-i64::from(start), i64::from(u32::MAX - end));
    document.selection = Some(TabSelection {
        track: moving.selection.track,
        anchor: (i64::from(moving.selection.anchor) + moving.delta) as u32,
        end: (i64::from(moving.selection.end) + moving.delta) as u32,
    });
    document.status = format!("Move {:+} ticks", moving.delta);
    if finish {
        if let Err(error) = move_notes(document, &moving.sources, moving.delta) {
            document.selection = Some(moving.selection);
            document.status = error;
        }
    } else {
        document.selection_move = Some(moving);
    }
}

/// Translate selected authored onsets atomically, retaining complete ties and phrase ownership. AI_CODE
fn move_notes(document: &mut EditorDocument, sources: &[EventSource], delta: i64) -> Result<(), String> {
    if delta == 0 { return Ok(()); }
    let track = document.chart.tracks.get(document.track_focus).ok_or("No focused track")?;
    let layout = layout_track(&document.chart, track);
    for chain in &layout.ties {
        let selected = chain.iter().filter(|index| sources.contains(&layout.symbols[**index].source)).count();
        if selected != 0 && selected != chain.len() { return Err("Select the entire tie chain to move it".into()); }
    }
    let mut edited = track.clone();
    for source in sources {
        let event = editor_source_event_mut(&mut edited, *source).ok_or("Selected note no longer exists")?;
        let start = i64::from(event.start).checked_add(delta).ok_or("Move exceeds timeline bounds")?;
        if !(0..=i64::from(u32::MAX)).contains(&start)
            || start + i64::from(event.duration_ticks(document.chart.resolution)) > i64::from(u32::MAX)
        { return Err("Move exceeds timeline bounds".into()); }
        event.start = start as u32;
        let phrase = match *source {
            EventSource::Track(_) => None,
            EventSource::Phrase { phrase, .. } => edited.phrases.get(phrase),
            EventSource::Verse { verse, phrase, .. } => edited.lyric_verses.get(verse).and_then(|verse| verse.phrases.get(phrase)),
        };
        if let Some(phrase) = phrase {
            let event = editor_source_event(&edited, *source).unwrap();
            if event.start < phrase.start || event.start.saturating_add(event.duration_ticks(document.chart.resolution))
                > phrase.start.saturating_add(phrase.duration_ticks)
            { return Err("Moved vocal notes must stay inside their phrase".into()); }
        }
    }
    for source in sources {
        let moved = editor_source_event(&edited, *source).unwrap();
        for symbol in &layout.symbols {
            if !sources.contains(&symbol.source) {
                let other = editor_source_event(track, symbol.source).ok_or("Missing destination note")?;
                if moved.start < other.start.saturating_add(other.duration_ticks(document.chart.resolution))
                    && other.start < moved.start.saturating_add(moved.duration_ticks(document.chart.resolution))
                    && moved.voice == other.voice && moved.staff == other.staff
                    && match (&moved.content, &other.content) {
                        (NoteContent::Pitched { note: left, .. }, NoteContent::Pitched { note: right, .. }) => left == right,
                        (NoteContent::Percussive { piece: left }, NoteContent::Percussive { piece: right }) => left == right,
                        (NoteContent::Rest, _) => true,
                        (_, NoteContent::Rest) => true,
                        _ => false,
                    }
                { return Err("Destination already contains that note or rest".into()); }
            }
        }
    }
    document.chart.tracks[document.track_focus] = edited;
    document.dirty = true;
    document.status = format!("Moved {} notes by {delta:+} ticks", sources.len());
    Ok(())
}

/// Explicit musical operations distinguish pitch-preserving remaps from rewrites. AI_CODE
#[derive(Clone)]
pub(super) enum BulkEdit {
    String(usize),
    Fret { string: usize, fret: u8 },
    Transpose(i16),
    Pitch(u8),
    Piece(String),
}

/// A batch draft holds resolved sources so navigation cannot retarget an open transaction. AI_CODE
pub(super) struct BulkDraft {
    sources: Vec<EventSource>,
    mode: usize,
    string: usize,
    value: i16,
    piece: usize,
    focus: usize,
}

/// Open the same keyboard field-menu pattern as the per-note inspector. AI_CODE
pub(super) fn open(document: &mut EditorDocument) -> Result<(), String> {
    let sources = selected_sources(document);
    if sources.is_empty() { return Err("Select a note range first".into()); }
    document.tab_resize = None;
    document.tab_fret_string = None;
    document.tab_percussion_lane = None;
    document.selection_drag = false;
    document.bulk_edit = Some(BulkDraft { sources, mode: 0, string: 0, value: 0, piece: 0, focus: 0 });
    document.status.clear();
    Ok(())
}

/// Dynamic draft rows expose only operations appropriate to the focused instrument. AI_CODE
fn rows(document: &EditorDocument, draft: &BulkDraft) -> Vec<(String, &'static str)> {
    let track = &document.chart.tracks[document.track_focus];
    let mut rows = Vec::new();
    if track.kind == InstrumentKind::Percussion {
        let piece = track.kit.as_ref().and_then(|kit| kit.pieces.get(draft.piece))
            .map_or("No kit pieces", |piece| piece.name.as_str());
        rows.push((format!("Piece       {piece}"), "piece"));
    } else {
        let modes: &[&str] = if track.kind == InstrumentKind::Strings {
            &["Move string (preserve pitch)", "Set string/fret", "Transpose", "Set pitch"]
        } else { &["Transpose", "Set pitch"] };
        rows.push((format!("Operation   {}", modes[draft.mode]), "mode"));
        let operation = if track.kind == InstrumentKind::Strings { draft.mode } else { draft.mode + 2 };
        if operation <= 1 { rows.push((format!("String      {}", draft.string + 1), "string")); }
        if operation >= 1 {
            rows.push((match operation {
                1 => format!("Fret        {}", draft.value),
                2 => format!("Semitones   {:+}", draft.value),
                _ => format!("MIDI pitch  {}", draft.value),
            }, "value"));
        }
    }
    rows.push(("Apply".into(), "apply"));
    rows
}

/// Render a compact transactional field menu without exposing batch JSON. AI_CODE
pub(super) fn display(document: &EditorDocument) -> Text {
    let draft = document.bulk_edit.as_ref().unwrap();
    let rows = rows(document, draft).iter().enumerate()
        .map(|(index, (label, _))| format!("{} {label}", if index == draft.focus { ">" } else { " " }))
        .collect::<Vec<_>>().join("\n");
    Text::new(format!("BULK EDIT  //  {} notes\n\n{rows}\n\n{}", draft.sources.len(), document.status))
}

/// Edit draft fields; only Apply mutates the chart, and failed validation retains the draft. AI_CODE
pub(super) fn input(document: &mut EditorDocument, keys: &ButtonInput<KeyCode>) {
    let Some(mut draft) = document.bulk_edit.take() else { return };
    if keys.just_pressed(KeyCode::Escape) { document.status = "Bulk edit canceled".into(); return; }
    let rows = rows(document, &draft);
    if keys.just_pressed(KeyCode::ArrowUp) { draft.focus = (draft.focus + rows.len() - 1) % rows.len(); }
    if keys.just_pressed(KeyCode::ArrowDown) { draft.focus = (draft.focus + 1) % rows.len(); }
    let row = rows[draft.focus].1;
    if row == "apply" && keys.just_pressed(KeyCode::Enter) {
        let track = &document.chart.tracks[document.track_focus];
        let edit = if track.kind == InstrumentKind::Percussion {
            track.kit.as_ref().and_then(|kit| kit.pieces.get(draft.piece))
                .map(|piece| BulkEdit::Piece(piece.name.clone()))
        } else {
            let mode = if track.kind == InstrumentKind::Strings { draft.mode } else { draft.mode + 2 };
            Some(match mode {
                0 => BulkEdit::String(draft.string),
                1 => BulkEdit::Fret { string: draft.string, fret: draft.value as u8 },
                2 => BulkEdit::Transpose(draft.value),
                _ => BulkEdit::Pitch(draft.value as u8),
            })
        };
        match edit.ok_or_else(|| "No kit piece available".into()).and_then(|edit| apply(document, &draft.sources, &edit)) {
            Ok(()) => return,
            Err(error) => document.status = error,
        }
    } else {
        let direction = if keys.just_pressed(KeyCode::ArrowLeft) { -1 } else if keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::Enter) { 1 } else { 0 };
        if direction != 0 {
            let track = &document.chart.tracks[document.track_focus];
            match row {
                "mode" => {
                    let count = if track.kind == InstrumentKind::Strings { 4 } else { 2 };
                    draft.mode = (draft.mode as i32 + direction).rem_euclid(count) as usize;
                    draft.value = if (track.kind == InstrumentKind::Strings && draft.mode == 3)
                        || (track.kind == InstrumentKind::Voice && draft.mode == 1) { 60 } else { 0 };
                }
                "string" => {
                    let count = track.tuning.as_ref().map_or(0, |tuning| tuning.strings.len());
                    if count > 0 { draft.string = (draft.string as i32 + direction).rem_euclid(count as i32) as usize; }
                }
                "piece" => {
                    let count = track.kit.as_ref().map_or(0, |kit| kit.pieces.len());
                    if count > 0 { draft.piece = (draft.piece as i32 + direction).rem_euclid(count as i32) as usize; }
                }
                "value" => {
                    let mode = if track.kind == InstrumentKind::Strings { draft.mode } else { draft.mode + 2 };
                    let (minimum, maximum) = match mode { 1 => (0, 24), 2 => (-127, 127), _ => (0, 127) };
                    draft.value = (draft.value + direction as i16).clamp(minimum, maximum);
                }
                _ => {}
            }
        }
    }
    document.bulk_edit = Some(draft);
}

/// Keyboard range extension uses the shared snap grid and never edits the score. AI_CODE
pub(super) fn extend_selection(document: &mut EditorDocument, direction: i32) {
    document.playback.playing = false;
    let anchor = document.current_tick;
    let step = i64::from(document.snap_interval.ticks(document.chart.resolution.max(1)).max(1));
    document.current_tick = (i64::from(anchor) + i64::from(direction) * step).clamp(0, i64::from(u32::MAX)) as u32;
    let selection = document.selection.get_or_insert(TabSelection { track: document.track_focus, anchor, end: anchor });
    selection.end = document.current_tick;
}

/// Begin or extend a pointer range without invoking note placement or duration resizing. AI_CODE
pub(super) fn begin_range(document: &mut EditorDocument, tick: u32, extend: bool) {
    let track = document.track_focus;
    if !extend || !document.selection.is_some_and(|selection| selection.track == track) {
        document.selection = Some(TabSelection { track, anchor: tick, end: tick });
    } else if let Some(selection) = &mut document.selection { selection.end = tick; }
    document.selection_drag = true;
    document.tab_resize = None;
    document.playback.playing = false;
}

/// Update the snapped endpoint and finalize the selected-note count on release. AI_CODE
pub(super) fn update_range(document: &mut EditorDocument, tick: u32, finish: bool) {
    if let Some(selection) = &mut document.selection { selection.end = tick; }
    if finish {
        document.selection_drag = false;
        document.status = format!("{} notes selected", selected_sources(document).len());
    }
}

/// Resolve selected authored sources, including vocal phrases, without selecting inferred rests. AI_CODE
pub(super) fn selected_sources(document: &EditorDocument) -> Vec<EventSource> {
    if let Some(moving) = &document.selection_move { return moving.sources.clone(); }
    let Some(selection) = document.selection.filter(|selection| selection.track == document.track_focus) else {
        return Vec::new();
    };
    let Some(track) = document.chart.tracks.get(selection.track) else { return Vec::new() };
    let start = selection.anchor.min(selection.end);
    let end = selection.anchor.max(selection.end);
    let mut sources = Vec::new();
    for symbol in layout_track(&document.chart, track).symbols {
        if symbol.start >= start && symbol.start <= end
            && !matches!(symbol.kind, NotationSymbolKind::Rest)
            && !sources.contains(&symbol.source)
        {
            sources.push(symbol.source);
        }
    }
    sources
}

/// Delete selected sources back-to-front within each event container, retaining ID cleanup. AI_CODE
pub(super) fn delete_selected(document: &mut EditorDocument) -> bool {
    let mut sources = selected_sources(document);
    if sources.is_empty() { return false; }
    sources.sort_by_key(|source| match *source {
        EventSource::Track(index) => (0, 0, 0, index),
        EventSource::Phrase { phrase, index } => (1, 0, phrase, index),
        EventSource::Verse { verse, phrase, index } => (2, verse, phrase, index),
    });
    let count = sources.into_iter().rev()
        .filter(|source| remove_editor_event(document, *source)).count();
    document.selection = None;
    document.selection_drag = false;
    document.status = format!("Removed {count} selected notes");
    true
}

/// Delete prioritizes a range over cursor focus, cleans relationships, and undoes as one edit. AI_CODE
#[test]
fn bulk_delete_selection_keyboard_and_undo() {
    for view in [EditorViewMode::Sheet, EditorViewMode::Tab] {
        let mut document = new_chart_document();
        let mut track = new_track(InstrumentKind::Voice);
        track.notes = serde_json::from_str(r#"[
            {"id":"outside","start":1920,"length":4,"note":67},
            {"id":"second","start":960,"length":4,"note":60},
            {"id":"first","start":0,"length":4,"note":60},
            {"id":"rest","start":0,"length":8,"rest":true}
        ]"#).unwrap();
        track.phrases = serde_json::from_str(r#"[{"start":0,"duration_ticks":1920,"text":"la","notes":[
            {"id":"p2","start":960,"length":4,"note":64},
            {"id":"p1","start":0,"length":4,"note":62}
        ]}]"#).unwrap();
        track.ties = vec![EventRelation { from: "first".into(), to: "second".into() }];
        track.slurs = vec![EventRelation { from: "second".into(), to: "outside".into() }];
        document.chart.tracks.push(track);
        document.view_mode = view;
        document.current_tick = 1920;
        document.selection = Some(TabSelection { track: 0, anchor: 0, end: 960 });
        document.dirty = false;
        let before = serde_json::to_value(&document.chart).unwrap();
        let mut app = App::new();
        app.add_message::<KeyboardInput>()
            .insert_resource(document)
            .insert_resource(EditorLibraries { tunings: Vec::new(), kits: Vec::new() })
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(PreUpdate, editor_history_checkpoint)
            .add_systems(Update, editor_tracks_input)
            .add_systems(PostUpdate, editor_history_checkpoint);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Delete);
        app.update();
        let document = app.world().resource::<EditorDocument>();
        let track = &document.chart.tracks[0];
        assert_eq!(track.notes.len(), 2);
        assert_eq!(track.notes[0].id.as_deref(), Some("outside"));
        assert!(matches!(track.notes[1].content, NoteContent::Rest));
        assert!(track.phrases[0].notes.is_empty());
        assert!(track.ties.is_empty());
        assert!(track.slurs.is_empty());
        assert!(document.selection.is_none());
        assert_eq!(document.history.undo.len(), 1);
        let after = serde_json::to_value(&document.chart).unwrap();
        let mut document = app.world_mut().resource_mut::<EditorDocument>();
        document.restore_history(false);
        assert_eq!(serde_json::to_value(&document.chart).unwrap(), before);
        assert!(!document.dirty);
        document.restore_history(true);
        assert_eq!(serde_json::to_value(&document.chart).unwrap(), after);
    }
}

/// Move previews leave notes untouched until release, which creates one reversible edit. AI_CODE
#[test]
fn bulk_move_preview_commit_and_undo() {
    let mut document = new_chart_document();
    document.chart.tracks.push(new_track(InstrumentKind::Voice));
    document.chart.tracks[0].notes = serde_json::from_str(r#"[
        {"id":"a","start":0,"length":8,"note":60},
        {"id":"b","start":480,"length":8,"note":64}
    ]"#).unwrap();
    document.selection = Some(TabSelection { track: 0, anchor: 480, end: 0 });
    document.dirty = false;
    document.checkpoint();
    let before = serde_json::to_value(&document.chart).unwrap();
    assert!(begin_move(&mut document, 240));
    update_move(&mut document, 1200, false);
    assert_eq!(document.selection.unwrap().end, 960);
    assert_eq!(selected_sources(&document).len(), 2);
    assert_eq!(move_preview_tick(&document, EventSource::Track(0), 0), Some(960));
    assert_eq!(serde_json::to_value(&document.chart).unwrap(), before);
    document.checkpoint();
    assert!(document.history.undo.is_empty());
    update_move(&mut document, 1200, true);
    assert_eq!(document.chart.tracks[0].notes[0].start, 960);
    assert_eq!(document.chart.tracks[0].notes[1].start, 1440);
    document.checkpoint();
    assert_eq!(document.history.undo.len(), 1);
    let after = serde_json::to_value(&document.chart).unwrap();
    document.restore_history(false);
    assert_eq!(serde_json::to_value(&document.chart).unwrap(), before);
    assert!(!document.dirty);
    document.restore_history(true);
    assert_eq!(serde_json::to_value(&document.chart).unwrap(), after);
}

/// Cancel and timeline-zero clamping never mutate notes or create a history transaction. AI_CODE
#[test]
fn bulk_move_cancel_and_zero_clamp() {
    let mut document = new_chart_document();
    document.chart.tracks.push(new_track(InstrumentKind::Voice));
    document.chart.tracks[0].notes = serde_json::from_str(r#"[{"start":240,"length":8,"note":60}]"#).unwrap();
    document.selection = Some(TabSelection { track: 0, anchor: 240, end: 480 });
    document.dirty = false;
    assert!(!begin_move(&mut document, 960));
    assert!(begin_move(&mut document, 480));
    update_move(&mut document, 0, false);
    assert_eq!(document.selection.unwrap().anchor, 0);
    cancel_move(&mut document);
    assert_eq!(document.selection.unwrap().anchor, 240);
    assert_eq!(document.chart.tracks[0].notes[0].start, 240);
    assert!(!document.dirty);
    assert!(begin_move(&mut document, 480));
    update_move(&mut document, 0, true);
    assert_eq!(document.chart.tracks[0].notes[0].start, 0);
}

/// Incomplete ties and occupied destinations reject a complete move without partial mutation. AI_CODE
#[test]
fn bulk_move_validates_ties_and_destination_overlap() {
    let mut document = new_chart_document();
    document.chart.tracks.push(new_track(InstrumentKind::Voice));
    let mut tools = SheetTools::default();
    tools.tied = true;
    sheet_edit_spelled_pitch_at_tick(&mut document, 0, 60, None, &tools);
    sheet_edit_spelled_pitch_at_tick(&mut document, 960, 60, None, &tools);
    let before = serde_json::to_value(&document.chart).unwrap();
    assert!(move_notes(&mut document, &[EventSource::Track(0)], 480).is_err());
    assert_eq!(serde_json::to_value(&document.chart).unwrap(), before);
    move_notes(&mut document, &[EventSource::Track(0), EventSource::Track(1)], 480).unwrap();
    assert_eq!(document.chart.tracks[0].ties.len(), 1);
    document.chart.tracks[0].ties.clear();
    let before = serde_json::to_value(&document.chart).unwrap();
    assert!(move_notes(&mut document, &[EventSource::Track(0)], 1200).is_err());
    assert_eq!(serde_json::to_value(&document.chart).unwrap(), before);
    assert!(move_notes(&mut document, &[EventSource::Track(0)], i64::from(u32::MAX)).is_err());
}

/// Nested vocal notes retain phrase ownership and cannot be moved outside the phrase span. AI_CODE
#[test]
fn bulk_move_keeps_vocal_notes_inside_phrase() {
    let mut document = new_chart_document();
    let mut track = new_track(InstrumentKind::Voice);
    track.phrases = serde_json::from_str(r#"[{"start":0,"duration_ticks":1920,"text":"la","notes":[{"id":"v","start":0,"length":4,"note":60}]}]"#).unwrap();
    document.chart.tracks.push(track);
    let sources = [EventSource::Phrase { phrase: 0, index: 0 }];
    move_notes(&mut document, &sources, 480).unwrap();
    assert_eq!(document.chart.tracks[0].phrases[0].notes[0].start, 480);
    let before = serde_json::to_value(&document.chart).unwrap();
    assert!(move_notes(&mut document, &sources, 960).is_err());
    assert_eq!(serde_json::to_value(&document.chart).unwrap(), before);
}

/// Apply a batch to a clone first; unplayable mappings and broken ties leave the score untouched. AI_CODE
pub(super) fn apply(document: &mut EditorDocument, sources: &[EventSource], edit: &BulkEdit) -> Result<(), String> {
    if sources.is_empty() { return Err("No notes selected".into()); }
    let track = document.chart.tracks.get(document.track_focus).ok_or("No focused track")?;
    let open = track.tuning.as_ref().map(|tuning| tuning.open_midi()).transpose()?;
    let capo = i16::from(track.capo.unwrap_or(0));
    let string_pitch = |string: usize| -> Result<i16, String> {
        if track.kind != InstrumentKind::Strings { return Err("String mapping requires a strings track".into()); }
        open.as_ref().and_then(|notes| notes.get(string)).map(|pitch| i16::from(*pitch) + capo)
            .ok_or_else(|| "String is outside the track tuning".into())
    };
    if let BulkEdit::Piece(piece) = edit {
        if track.kind != InstrumentKind::Percussion || !track.kit.as_ref()
            .is_some_and(|kit| kit.pieces.iter().any(|candidate| candidate.name == *piece))
        {
            return Err("Choose a piece from this track's kit".into());
        }
    }
    let layout = layout_track(&document.chart, track);
    let mut edited = track.clone();
    for source in sources {
        let event = editor_source_event_mut(&mut edited, *source).ok_or("Selected note no longer exists")?;
        match edit {
            BulkEdit::Piece(piece) => {
                if !matches!(event.content, NoteContent::Percussive { .. }) { return Err("Selection contains non-percussion notes".into()); }
                event.content = NoteContent::Percussive { piece: piece.clone() };
            }
            BulkEdit::String(string) => {
                let pitch = event.note().ok_or("Selection contains unpitched notes")?;
                let fret = i16::from(pitch) - string_pitch(*string)?;
                if !(0..=24).contains(&fret) { return Err("Not every selected pitch is playable on that string (frets 0-24)".into()); }
                event.performance.get_or_insert_with(|| serde_json::from_str("{}").unwrap()).preferred_string = Some(*string);
            }
            _ => {
                let previous = event.note().ok_or("Selection contains unpitched notes")?;
                let pitch = match edit {
                    BulkEdit::Fret { string, fret } => {
                        if *fret > 24 { return Err("Fret must be between 0 and 24".into()); }
                        string_pitch(*string)? + i16::from(*fret)
                    }
                    BulkEdit::Transpose(semitones) => i16::from(previous).checked_add(*semitones).ok_or("Pitch exceeds MIDI range")?,
                    BulkEdit::Pitch(pitch) => i16::from(*pitch),
                    _ => unreachable!(),
                };
                if !(0..=127).contains(&pitch) { return Err("Pitch exceeds MIDI range (0-127)".into()); }
                if pitch != i16::from(previous) {
                    event.content = NoteContent::Pitched { note: pitch as u8, spelling: None };
                }
                if let BulkEdit::Fret { string, .. } = edit {
                    event.performance.get_or_insert_with(|| serde_json::from_str("{}").unwrap()).preferred_string = Some(*string);
                } else if let Some(hints) = &mut event.performance {
                    let playable = hints.preferred_string.and_then(|string| open.as_ref()?.get(string))
                        .is_some_and(|open| (0..=24).contains(&(pitch - i16::from(*open) - capo)));
                    if !playable { hints.preferred_string = None; }
                }
            }
        }
    }
    for chain in &layout.ties {
        for pair in chain.windows(2) {
            let from = editor_source_event(&edited, layout.symbols[pair[0]].source).ok_or("Missing tie source")?;
            let to = editor_source_event(&edited, layout.symbols[pair[1]].source).ok_or("Missing tie target")?;
            let matches = match (&from.content, &to.content) {
                (NoteContent::Pitched { note: left, .. }, NoteContent::Pitched { note: right, .. }) => left == right,
                (NoteContent::Percussive { piece: left }, NoteContent::Percussive { piece: right }) => left == right,
                _ => false,
            };
            if !matches { return Err("Include the entire tie chain when changing pitch or piece".into()); }
        }
    }
    if serde_json::to_value(track).expect("source track serializes")
        != serde_json::to_value(&edited).expect("edited track serializes")
    {
        document.chart.tracks[document.track_focus] = edited;
        document.dirty = true;
    }
    document.status = format!("Updated {} selected notes", sources.len());
    Ok(())
}

/// Range selection and mapping preserve authored rhythm while committing one undo transaction. AI_CODE
#[test]
fn bulk_range_mapping_is_atomic_and_undoable() {
    let mut document = new_chart_document();
    ensure_editor_has_track(&mut document);
    let open = document.chart.tracks[0].tuning.as_ref().unwrap().open_midi().unwrap();
    let pitch = open[1];
    document.chart.tracks[0].notes = serde_json::from_value(serde_json::json!([
        {"id":"a","start":0,"length":8,"note":pitch,"performance":{"preferred_string":0}},
        {"id":"b","start":480,"length":4,"note":pitch + 2},
        {"id":"c","start":960,"length":4,"note":pitch + 4}
    ])).unwrap();
    document.selection = Some(TabSelection { track: 0, anchor: 480, end: 0 });
    let sources = selected_sources(&document);
    assert_eq!(sources.len(), 2);
    document.checkpoint();
    let before = serde_json::to_value(&document.chart).unwrap();
    apply(&mut document, &sources, &BulkEdit::String(1)).unwrap();
    assert_eq!(document.chart.tracks[0].notes[0].note(), Some(pitch));
    assert_eq!(document.chart.tracks[0].notes[1].performance.as_ref().unwrap().preferred_string, Some(1));
    assert!(document.chart.tracks[0].notes[2].performance.is_none());
    document.checkpoint();
    let after = serde_json::to_value(&document.chart).unwrap();
    assert!(apply(&mut document, &sources, &BulkEdit::String(99)).is_err());
    assert_eq!(serde_json::to_value(&document.chart).unwrap(), after);
    document.restore_history(false);
    assert_eq!(serde_json::to_value(&document.chart).unwrap(), before);
    document.restore_history(true);
    assert_eq!(serde_json::to_value(&document.chart).unwrap(), after);
}

/// A batch rolls back even if a later note fails after an earlier note could be remapped. AI_CODE
#[test]
fn bulk_rejects_partially_unplayable_string_and_pitch_batches() {
    let mut document = new_chart_document();
    ensure_editor_has_track(&mut document);
    let open = document.chart.tracks[0].tuning.as_ref().unwrap().open_midi().unwrap()[0];
    document.chart.tracks[0].notes = serde_json::from_value(serde_json::json!([
        {"start":0,"length":4,"note":open + 5},
        {"start":960,"length":4,"note":127}
    ])).unwrap();
    document.dirty = false;
    let before = serde_json::to_value(&document.chart).unwrap();
    let sources = [EventSource::Track(0), EventSource::Track(1)];
    assert!(apply(&mut document, &sources, &BulkEdit::String(0)).is_err());
    assert_eq!(serde_json::to_value(&document.chart).unwrap(), before);
    assert!(apply(&mut document, &sources, &BulkEdit::Transpose(1)).is_err());
    assert_eq!(serde_json::to_value(&document.chart).unwrap(), before);
    assert!(!document.dirty);
}

/// Fixed fret assignment respects capo, while transposition preserves intervals and note identity. AI_CODE
#[test]
fn bulk_fret_pitch_and_transpose_respect_tuning_and_capo() {
    let mut document = new_chart_document();
    ensure_editor_has_track(&mut document);
    document.chart.tracks[0].capo = Some(2);
    document.chart.tracks[0].notes = serde_json::from_str(r#"[
        {"id":"a","start":0,"length":8,"note":60,"performance":{"palm_mute":true}},
        {"id":"b","start":480,"length":4,"note":64}
    ]"#).unwrap();
    let sources = [EventSource::Track(0), EventSource::Track(1)];
    apply(&mut document, &sources, &BulkEdit::Transpose(3)).unwrap();
    assert_eq!(document.chart.tracks[0].notes[0].note(), Some(63));
    assert_eq!(document.chart.tracks[0].notes[1].note(), Some(67));
    let open = document.chart.tracks[0].tuning.as_ref().unwrap().open_midi().unwrap()[1];
    apply(&mut document, &sources, &BulkEdit::Fret { string: 1, fret: 7 }).unwrap();
    for event in &document.chart.tracks[0].notes {
        assert_eq!(event.note(), Some(open + 9));
        assert_eq!(event.performance.as_ref().unwrap().preferred_string, Some(1));
    }
    assert_eq!(document.chart.tracks[0].notes[0].start, 0);
    assert_eq!(document.chart.tracks[0].notes[0].length, NoteValue::Eighth);
    assert_eq!(document.chart.tracks[0].notes[0].id.as_deref(), Some("a"));
    assert_eq!(serde_json::to_value(&document.chart.tracks[0].notes[0]).unwrap()["performance"]["palm_mute"], true);
    apply(&mut document, &sources, &BulkEdit::Pitch(1)).unwrap();
    assert_eq!(document.chart.tracks[0].notes[0].note(), Some(1));
    assert_eq!(document.chart.tracks[0].notes[0].performance.as_ref().unwrap().preferred_string, None);
}

/// Pitch rewrites cannot silently break explicit ties at a range boundary. AI_CODE
#[test]
fn bulk_preserves_tie_chain_integrity() {
    let mut document = new_chart_document();
    document.chart.tracks.push(new_track(InstrumentKind::Voice));
    let mut tools = SheetTools::default();
    tools.tied = true;
    sheet_edit_spelled_pitch_at_tick(&mut document, 0, 60, None, &tools);
    sheet_edit_spelled_pitch_at_tick(&mut document, 960, 60, None, &tools);
    let before = serde_json::to_value(&document.chart).unwrap();
    assert!(apply(&mut document, &[EventSource::Track(0)], &BulkEdit::Transpose(2)).is_err());
    assert_eq!(serde_json::to_value(&document.chart).unwrap(), before);
    apply(&mut document, &[EventSource::Track(0), EventSource::Track(1)], &BulkEdit::Transpose(2)).unwrap();
    assert_eq!(document.chart.tracks[0].ties.len(), 1);
    assert!(document.chart.tracks[0].notes.iter().all(|note| note.note() == Some(62)));
    document.chart.tracks[0].ties.clear();
    document.chart.tracks[0].notes[0].tied = true;
    assert!(apply(&mut document, &[EventSource::Track(1)], &BulkEdit::Pitch(64)).is_err());
}

/// Range selection includes chords and nested phrase notes but excludes authored rests. AI_CODE
#[test]
fn bulk_range_includes_nested_vocal_notes_and_chords() {
    let mut document = new_chart_document();
    let mut track = new_track(InstrumentKind::Voice);
    track.notes = serde_json::from_str(r#"[
        {"start":0,"length":4,"note":60},
        {"start":0,"length":4,"note":64},
        {"start":480,"length":8,"rest":true},
        {"start":1920,"length":4,"note":67}
    ]"#).unwrap();
    track.phrases = serde_json::from_str(r#"[{"start":0,"duration_ticks":1920,"text":"la","notes":[{"start":960,"length":4,"note":62}]}]"#).unwrap();
    document.chart.tracks.push(track);
    document.selection = Some(TabSelection { track: 0, anchor: 0, end: 960 });
    let sources = selected_sources(&document);
    assert_eq!(sources.len(), 3);
    apply(&mut document, &sources, &BulkEdit::Transpose(1)).unwrap();
    assert_eq!(document.chart.tracks[0].phrases[0].notes[0].note(), Some(63));
    assert_eq!(document.chart.tracks[0].notes[3].note(), Some(67));
    assert!(matches!(document.chart.tracks[0].notes[2].content, NoteContent::Rest));
}

/// Kit-piece batches validate against the real kit and retain rhythm and other guidance. AI_CODE
#[test]
fn bulk_percussion_uses_track_kit() {
    let mut document = new_chart_document();
    document.chart.tracks.push(new_track(InstrumentKind::Percussion));
    let kit = document.chart.tracks[0].kit.as_ref().unwrap();
    let initial = kit.pieces[0].name.clone();
    let target = kit.pieces[1].name.clone();
    document.chart.tracks[0].notes = serde_json::from_value(serde_json::json!([
        {"start":0,"length":4,"piece":initial},
        {"start":960,"length":8,"piece":initial}
    ])).unwrap();
    let sources = [EventSource::Track(0), EventSource::Track(1)];
    let before = serde_json::to_value(&document.chart).unwrap();
    assert!(apply(&mut document, &sources, &BulkEdit::Piece("not-in-kit".into())).is_err());
    assert_eq!(serde_json::to_value(&document.chart).unwrap(), before);
    apply(&mut document, &sources, &BulkEdit::Piece(target.clone())).unwrap();
    assert!(document.chart.tracks[0].notes.iter().all(|event| matches!(&event.content, NoteContent::Percussive { piece } if piece == &target)));
    assert_eq!(document.chart.tracks[0].notes[1].length, NoteValue::Eighth);
    let json = serde_json::to_value(&document.chart).unwrap();
    let reloaded: Chart = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(serde_json::to_value(reloaded).unwrap(), json);
}

/// Draft cancellation and invalid commits never mutate the chart or lose the user's draft. AI_CODE
#[test]
fn bulk_menu_cancels_and_retains_invalid_drafts() {
    let mut document = new_chart_document();
    ensure_editor_has_track(&mut document);
    document.chart.tracks[0].notes = serde_json::from_str(r#"[{"start":0,"length":4,"note":1}]"#).unwrap();
    document.selection = Some(TabSelection { track: 0, anchor: 0, end: 0 });
    let before = serde_json::to_value(&document.chart).unwrap();
    open(&mut document).unwrap();
    document.bulk_edit.as_mut().unwrap().focus = 2;
    let mut keys = ButtonInput::default();
    keys.press(KeyCode::Enter);
    input(&mut document, &keys);
    assert!(document.bulk_edit.is_some());
    assert_eq!(serde_json::to_value(&document.chart).unwrap(), before);
    let mut keys = ButtonInput::default();
    keys.press(KeyCode::Escape);
    input(&mut document, &keys);
    assert!(document.bulk_edit.is_none());
    assert_eq!(serde_json::to_value(&document.chart).unwrap(), before);
}

/// Pointer range gestures select in either direction without creating score history. AI_CODE
#[test]
fn bulk_pointer_range_is_inclusive_and_non_destructive() {
    let mut document = new_chart_document();
    document.chart.tracks.push(new_track(InstrumentKind::Voice));
    document.chart.tracks[0].notes = serde_json::from_str(r#"[
        {"start":0,"length":4,"note":60},
        {"start":960,"length":4,"note":62},
        {"start":1920,"length":4,"note":64}
    ]"#).unwrap();
    document.dirty = false;
    document.checkpoint();
    begin_range(&mut document, 1920, false);
    assert!(document.selection_drag);
    update_range(&mut document, 960, true);
    assert!(!document.selection_drag);
    assert_eq!(selected_sources(&document).len(), 2);
    begin_range(&mut document, 0, true);
    update_range(&mut document, 0, true);
    assert_eq!(selected_sources(&document).len(), 3);
    document.checkpoint();
    assert!(document.history.undo.is_empty());
    assert!(!document.dirty);
    document.chart.tracks.push(new_track(InstrumentKind::Voice));
    cycle_editor_track(&mut document, 1);
    assert!(selected_sources(&document).is_empty());
}

/// Shortcuts open an isolated batch draft, apply it as one edit, and route undo normally. AI_CODE
#[test]
fn bulk_keyboard_selection_menu_and_undo() {
    for view in [EditorViewMode::Sheet, EditorViewMode::Tab] {
    let mut document = new_chart_document();
    document.chart.tracks.push(new_track(InstrumentKind::Voice));
    document.chart.tracks[0].notes = serde_json::from_str(r#"[
        {"start":0,"length":4,"note":60},
        {"start":960,"length":4,"note":64}
    ]"#).unwrap();
    document.view_mode = view;
    document.dirty = false;
    let before = serde_json::to_value(&document.chart).unwrap();
    let mut app = App::new();
    app.add_message::<KeyboardInput>()
        .insert_resource(document)
        .insert_resource(EditorLibraries { tunings: Vec::new(), kits: Vec::new() })
        .init_resource::<ButtonInput<KeyCode>>()
        .add_systems(PreUpdate, editor_history_checkpoint)
        .add_systems(Update, editor_tracks_input)
        .add_systems(PostUpdate, editor_history_checkpoint);
    for keys in [
        vec![KeyCode::ShiftLeft, KeyCode::ArrowRight],
        vec![KeyCode::ControlLeft, KeyCode::KeyB],
        vec![KeyCode::KeyN],
        vec![KeyCode::ArrowDown],
        vec![KeyCode::ArrowRight],
        vec![KeyCode::ArrowDown],
        vec![KeyCode::Enter],
    ] {
        let mut input = ButtonInput::default();
        for key in keys { input.press(key); }
        app.insert_resource(input);
        app.update();
    }
    let document = app.world().resource::<EditorDocument>();
    assert!(document.bulk_edit.is_none());
    assert_eq!(document.chart.tracks.len(), 1);
    assert_eq!(document.chart.tracks[0].notes[0].note(), Some(61));
    assert_eq!(document.chart.tracks[0].notes[1].note(), Some(65));
    assert_eq!(document.history.undo.len(), 1);
    let mut input = ButtonInput::default();
    input.press(KeyCode::ControlLeft);
    input.press(KeyCode::KeyZ);
    app.insert_resource(input);
    app.update();
    let document = app.world().resource::<EditorDocument>();
    assert_eq!(serde_json::to_value(&document.chart).unwrap(), before);
    assert!(!document.dirty);
    assert!(document.selection.is_none());
    }
}