use super::*;
use serde_json::{Value, json};

/// A transactional, keyboard-operated editor for one event's notation or guidance. AI_CODE
pub(super) struct EventInspector {
    pub(super) target: EditorTextTarget,
    draft: Value,
    focus: usize,
}

/// An individual editable field rather than a serialized JSON document. AI_CODE
enum FieldKind {
    Choices(Vec<Value>),
    Number {
        minimum: f64,
        maximum: f64,
        step: f64,
    },
    Text,
    Articulation(&'static str),
    AddPoint,
    ClearHints,
    Apply,
}

/// A labeled field backed by a path in the draft event section. AI_CODE
struct Field {
    label: String,
    path: String,
    kind: FieldKind,
}

/// Construct a field with a stable draft path. AI_CODE
fn field(label: &str, path: &str, kind: FieldKind) -> Field {
    Field {
        label: label.into(),
        path: path.into(),
        kind,
    }
}

/// Enumerated fields include an unset option unless the value is required. AI_CODE
fn choices(label: &str, path: &str, names: &[&str], optional: bool) -> Field {
    let mut values = Vec::new();
    if optional {
        values.push(Value::Null);
    }
    values.extend(names.iter().map(|name| json!(name)));
    field(label, path, FieldKind::Choices(values))
}

/// Numeric fields use the same arrow-key adjustment model as chart metadata. AI_CODE
fn number(label: &str, path: &str, minimum: f64, maximum: f64, step: f64) -> Field {
    field(
        label,
        path,
        FieldKind::Number {
            minimum,
            maximum,
            step,
        },
    )
}

/// Open a menu from the existing section serializer without exposing its JSON buffer. AI_CODE
pub(super) fn open(document: &mut EditorDocument, target: EditorTextTarget) -> Result<(), String> {
    begin_score_section_edit(document, target)?;
    let entry = document.text_entry.take().ok_or("No event section")?;
    let draft = serde_json::from_str(&entry.buffer).map_err(|error| error.to_string())?;
    document.event_text_cursor = None;
    document.tab_fret_string = None;
    document.tab_resize = None;
    document.inspector = Some(EventInspector {
        target,
        draft,
        focus: 0,
    });
    document.status = "".into();
    Ok(())
}

/// Build musical choices from the actual note, tuning, and kit, preserving sounding pitch. AI_CODE
fn fields(document: &EditorDocument, menu: &EventInspector) -> Vec<Field> {
    let source = match menu.target {
        EditorTextTarget::ScoreNotation(source) | EditorTextTarget::ScorePerformance(source) => {
            source
        }
        _ => return Vec::new(),
    };
    let Some(track) = document.chart.tracks.get(document.track_focus) else {
        return Vec::new();
    };
    let Some(event) = editor_source_event(track, source) else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    if matches!(menu.target, EditorTextTarget::ScoreNotation(_)) {
        rows.push(field(
            "Length",
            "/length",
            FieldKind::Choices([1, 2, 4, 8, 16].map(|value| json!(value)).to_vec()),
        ));
        rows.push(choices(
            "Dynamic",
            "/dynamic",
            &["ppp", "pp", "mp", "mf", "f", "ff", "fff"],
            true,
        ));
        rows.push(number("Dots", "/dots", 0.0, 2.0, 1.0));
        rows.push(number("Voice", "/voice", 1.0, 4.0, 1.0));
        rows.push(number("Staff", "/staff", 1.0, 16.0, 1.0));
        rows.push(field(
            "Tie to next",
            "/tied",
            FieldKind::Choices(vec![json!(false), json!(true)]),
        ));
        if let Some(pitch) = event.note() {
            let mut spellings = vec![Value::Null];
            for octave in -1..=9 {
                for (step, natural) in [
                    ('C', 0),
                    ('D', 2),
                    ('E', 4),
                    ('F', 5),
                    ('G', 7),
                    ('A', 9),
                    ('B', 11),
                ] {
                    let alter = i32::from(pitch) - ((octave + 1) * 12 + natural);
                    if (-2..=2).contains(&alter) {
                        spellings.push(json!({"step":step,"alter":alter,"octave":octave}));
                    }
                }
            }
            rows.push(field(
                "Written spelling",
                "/spelling",
                FieldKind::Choices(spellings),
            ));
        }
        rows.push(number("Tuplet: actual", "/tuplet/actual", 1.0, 32.0, 1.0));
        rows.push(number("Tuplet: normal", "/tuplet/normal", 1.0, 32.0, 1.0));
        rows.push(choices("Stem", "/stem", &["auto", "up", "down"], true));
        for mark in [
            "staccato", "tenuto", "marcato", "accent", "fermata", "grace",
        ] {
            rows.push(field(
                &format!("Articulation: {mark}"),
                "/articulations",
                FieldKind::Articulation(mark),
            ));
        }
        rows.push(choices(
            "Grace kind",
            "/grace/kind",
            &["acciaccatura", "appoggiatura"],
            true,
        ));
        rows.push(field(
            "Grace slash",
            "/grace/slash",
            FieldKind::Choices(vec![json!(false), json!(true)]),
        ));
        rows.push(field(
            "Grace principal",
            "/grace/principal",
            FieldKind::Choices(
                std::iter::once(Value::Null)
                    .chain(
                        all_track_events(track)
                            .filter(|candidate| candidate.id != event.id)
                            .filter_map(|candidate| candidate.id.as_ref())
                            .map(|id| json!(id)),
                    )
                    .collect(),
            ),
        ));
        rows.push(number("Grace order", "/grace/order", 0.0, 65535.0, 1.0));
        rows.push(field("Chord ID", "/chord", FieldKind::Text));
    } else {
        if track.kind == InstrumentKind::Strings {
            let mut strings = vec![Value::Null];
            let mut frets = std::collections::BTreeSet::new();
            if let (Some(pitch), Some(notes)) = (
                event.note(),
                track
                    .tuning
                    .as_ref()
                    .and_then(|tuning| tuning.open_midi().ok()),
            ) {
                strings.extend(notes.iter().enumerate().filter_map(|(index, open)| {
                    let fret =
                        i16::from(pitch) - i16::from(*open) - i16::from(track.capo.unwrap_or(0));
                    if (0..=24).contains(&fret) {
                        frets.insert(fret);
                        Some(json!(index))
                    } else {
                        None
                    }
                }));
            }
            rows.push(field(
                "Preferred string",
                "/preferred_string",
                FieldKind::Choices(strings),
            ));
            rows.push(field(
                "Preferred fret",
                "/preferred_fret",
                FieldKind::Choices(
                    std::iter::once(Value::Null)
                        .chain(frets.into_iter().map(|fret| json!(fret)))
                        .collect(),
                ),
            ));
        }
        rows.push(choices("Attack", "/attack", &["pluck", "tap"], true));
        rows.push(choices(
            "Transition",
            "/transition",
            &["hammer_on", "pull_off", "slide"],
            true,
        ));
        rows.push(field(
            "Palm mute",
            "/palm_mute",
            FieldKind::Choices(vec![json!(false), json!(true)]),
        ));
        rows.push(choices(
            "Harmonic",
            "/harmonic",
            &["natural", "artificial", "pinch"],
            true,
        ));
        rows.push(choices(
            "Vibrato",
            "/vibrato",
            &["normal", "wide", "narrow"],
            true,
        ));
        rows.push(number(
            "Bend semitones",
            "/bend/semitones",
            -24.0,
            24.0,
            0.5,
        ));
        rows.push(field(
            "Bend release",
            "/bend/release",
            FieldKind::Choices(vec![json!(false), json!(true)]),
        ));
        if let Some(points) = menu.draft.pointer("/bend/points").and_then(Value::as_array) {
            for index in 0..points.len() {
                rows.push(number(
                    &format!("Bend point {}: offset", index + 1),
                    &format!("/bend/points/{index}/offset"),
                    0.0,
                    1.0,
                    0.05,
                ));
                rows.push(number(
                    &format!("Bend point {}: semitones", index + 1),
                    &format!("/bend/points/{index}/semitones"),
                    -24.0,
                    24.0,
                    0.5,
                ));
            }
        }
        rows.push(field("Add bend point", "/bend/points", FieldKind::AddPoint));
        rows.push(choices("Motion", "/motion/kind", &["trill"], true));
        rows.push(number(
            "Motion target MIDI",
            "/motion/target",
            0.0,
            127.0,
            1.0,
        ));
        if track.kind == InstrumentKind::Percussion {
            let pieces = track
                .kit
                .as_ref()
                .map(|kit| {
                    kit.pieces
                        .iter()
                        .map(|piece| json!(piece.name))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            rows.push(field(
                "Kit piece",
                "/piece",
                FieldKind::Choices(pieces.clone()),
            ));
            let optional = std::iter::once(Value::Null)
                .chain(pieces)
                .collect::<Vec<_>>();
            rows.push(field("Roll", "/roll", FieldKind::Choices(optional.clone())));
            rows.push(field(
                "Alternating roll (droll)",
                "/droll",
                FieldKind::Choices(optional),
            ));
            rows.push(choices(
                "Percussion dynamics",
                "/percussion_dynamics",
                &["normal", "accent", "ghost"],
                true,
            ));
            rows.push(choices(
                "Percussion grace",
                "/percussion_grace",
                &["flam", "drag"],
                true,
            ));
            rows.push(choices(
                "Technique",
                "/percussion_technique",
                &["buzz", "cymbal_choke"],
                true,
            ));
            rows.push(choices("Sticking", "/sticking", &["right", "left"], true));
            rows.push(choices(
                "Hi-hat",
                "/hi_hat",
                &["open", "closed", "pedal"],
                true,
            ));
        }
        rows.push(field("Clear all hints", "", FieldKind::ClearHints));
    }
    rows.push(field("Apply changes", "", FieldKind::Apply));
    rows
}

/// Set a leaf value, initializing optional compound settings with valid editable defaults. AI_CODE
fn set_value(draft: &mut Value, path: &str, value: Value) {
    for (parent, initial) in [
        ("tuplet", json!({"actual":3,"normal":2})),
        (
            "grace",
            json!({"kind":"acciaccatura","slash":false,"principal":null,"order":0}),
        ),
        ("bend", json!({"semitones":1.0,"release":false,"points":[]})),
        ("motion", json!({"kind":"trill","target":60})),
    ] {
        if path.starts_with(&format!("/{parent}/")) && draft.get(parent).is_none_or(Value::is_null)
        {
            draft[parent] = initial;
        }
    }
    if let Some(target) = draft.pointer_mut(path) {
        *target = value;
    } else if let Some(key) = path.strip_prefix('/').filter(|key| !key.contains('/')) {
        draft[key] = value;
    }
}

/// Adjust a draft field; compound guidance remains separate from written pitch. AI_CODE
fn adjust(document: &EditorDocument, menu: &mut EventInspector, row: &Field, delta: i32) {
    if matches!(row.kind, FieldKind::ClearHints) {
        let piece = menu.draft.get("piece").cloned();
        menu.draft = json!({});
        if let Some(piece) = piece {
            menu.draft["piece"] = piece;
        }
        return;
    }
    let current = menu
        .draft
        .pointer(&row.path)
        .cloned()
        .unwrap_or(Value::Null);
    let value = match &row.kind {
        FieldKind::Choices(values) if !values.is_empty() => {
            let index = values
                .iter()
                .position(|value| *value == current)
                .unwrap_or(0);
            values[(index as i32 + delta).rem_euclid(values.len() as i32) as usize].clone()
        }
        FieldKind::Number {
            minimum,
            maximum,
            step,
        } => {
            let next = (current.as_f64().unwrap_or(*minimum) + f64::from(delta) * step)
                .clamp(*minimum, *maximum);
            if *step == 1.0 {
                json!(next as i64)
            } else {
                json!((next * 100.0).round() / 100.0)
            }
        }
        FieldKind::Articulation(mark) => {
            let mut marks = current.as_array().cloned().unwrap_or_default();
            if marks.contains(&json!(mark)) {
                marks.retain(|value| *value != json!(mark));
            } else {
                marks.push(json!(mark));
            }
            Value::Array(marks)
        }
        FieldKind::AddPoint => {
            let mut points = current.as_array().cloned().unwrap_or_default();
            let offset = points
                .last()
                .and_then(|point| point["offset"].as_f64())
                .unwrap_or(0.0);
            points.push(json!({"offset":offset,"semitones":1.0}));
            Value::Array(points)
        }
        _ => return,
    };
    if row.path == "/grace/kind" && value.is_null() {
        menu.draft["grace"] = Value::Null;
        return;
    }
    if row.path == "/motion/kind" && value.is_null() {
        menu.draft["motion"] = Value::Null;
        return;
    }
    set_value(&mut menu.draft, &row.path, value);
    if row.path == "/roll" {
        menu.draft["droll"] = Value::Null;
    }
    if row.path == "/droll" {
        menu.draft["roll"] = Value::Null;
    }
    if row.path == "/preferred_fret" {
        if menu.draft["preferred_fret"].is_null() {
            menu.draft["preferred_string"] = Value::Null;
            menu.draft
                .as_object_mut()
                .map(|fields| fields.remove("preferred_fret"));
        } else if let EditorTextTarget::ScorePerformance(source) = menu.target
            && let Some(track) = document.chart.tracks.get(document.track_focus)
            && let Some(pitch) = editor_source_event(track, source).and_then(ChartEvent::note)
            && let Some(notes) = track
                .tuning
                .as_ref()
                .and_then(|tuning| tuning.open_midi().ok())
            && let Some(fret) = menu.draft["preferred_fret"].as_i64()
            && let Some(string) = notes.iter().position(|open| {
                i64::from(pitch) - i64::from(*open) - i64::from(track.capo.unwrap_or(0)) == fret
            })
        {
            menu.draft["preferred_string"] = json!(string);
        }
    }
    if row.path == "/preferred_string" {
        menu.draft
            .as_object_mut()
            .map(|fields| fields.remove("preferred_fret"));
        let source = match menu.target {
            EditorTextTarget::ScorePerformance(source) => source,
            _ => return,
        };
        if let Some(track) = document.chart.tracks.get(document.track_focus)
            && let Some(event) = editor_source_event(track, source)
            && let Some(string) = menu.draft["preferred_string"].as_u64()
            && let Some(open) = track
                .tuning
                .as_ref()
                .and_then(|tuning| tuning.open_midi().ok())
                .and_then(|notes| notes.get(string as usize).copied())
            && let Some(pitch) = event.note()
        {
            menu.draft["preferred_fret"] =
                json!(i16::from(pitch) - i16::from(open) - i16::from(track.capo.unwrap_or(0)));
        }
    }
}

/// Clear optional settings without deleting the event or unrelated draft fields. AI_CODE
fn clear(menu: &mut EventInspector, row: &Field) {
    if matches!(row.kind, FieldKind::Apply | FieldKind::ClearHints) {
        return;
    }
    if let FieldKind::Articulation(mark) = row.kind {
        if let Some(marks) = menu.draft["articulations"].as_array_mut() {
            marks.retain(|value| *value != json!(mark));
        }
    } else if row.path.starts_with("/bend/points/") {
        if let Some(index) = row
            .path
            .split('/')
            .nth(3)
            .and_then(|index| index.parse::<usize>().ok())
            && let Some(points) = menu
                .draft
                .pointer_mut("/bend/points")
                .and_then(Value::as_array_mut)
            && index < points.len()
        {
            points.remove(index);
        }
    } else if row.path.contains('/') && row.path[1..].contains('/') {
        let parent = row.path.split('/').nth(1).unwrap_or("");
        menu.draft[parent] = Value::Null;
    } else {
        let value = match row.path.as_str() {
            "/length" => json!(4),
            "/voice" | "/staff" => json!(1),
            "/dots" => json!(0),
            "/tied" | "/palm_mute" => json!(false),
            "/piece" => return,
            _ => Value::Null,
        };
        set_value(&mut menu.draft, &row.path, value);
        if matches!(row.path.as_str(), "/preferred_string" | "/preferred_fret") {
            menu.draft["preferred_string"] = Value::Null;
            menu.draft
                .as_object_mut()
                .map(|fields| fields.remove("preferred_fret"));
        }
    }
}

/// Handle the field menu before global navigation or instrument tools can consume its keys. AI_CODE
pub(super) fn input(document: &mut EditorDocument, keyboard: &ButtonInput<KeyCode>) {
    let Some(mut menu) = document.inspector.take() else {
        return;
    };
    let rows = fields(document, &menu);
    if rows.is_empty() || keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    menu.focus = menu.focus.min(rows.len() - 1);
    if keyboard.just_pressed(KeyCode::ArrowUp) {
        menu.focus = (menu.focus + rows.len() - 1) % rows.len();
    }
    if keyboard.just_pressed(KeyCode::ArrowDown) {
        menu.focus = (menu.focus + 1) % rows.len();
    }
    if keyboard.just_pressed(KeyCode::Home) {
        menu.focus = 0;
    }
    if keyboard.just_pressed(KeyCode::End) {
        menu.focus = rows.len() - 1;
    }
    if keyboard.just_pressed(KeyCode::PageUp) {
        menu.focus = menu.focus.saturating_sub(10);
    }
    if keyboard.just_pressed(KeyCode::PageDown) {
        menu.focus = (menu.focus + 10).min(rows.len() - 1);
    }
    let row = &rows[menu.focus];
    if keyboard.just_pressed(KeyCode::Delete) {
        clear(&mut menu, row);
    }
    if keyboard.just_pressed(KeyCode::ArrowLeft) {
        adjust(document, &mut menu, row, -1);
    }
    if keyboard.just_pressed(KeyCode::ArrowRight) {
        adjust(document, &mut menu, row, 1);
    }
    if keyboard.just_pressed(KeyCode::Enter) {
        match row.kind {
            FieldKind::Apply => {
                match replace_score_section(document, menu.target, &menu.draft.to_string()) {
                    Ok(()) => {
                        document.status = "Note updated".into();
                        return;
                    }
                    Err(error) => document.status = error,
                }
            }
            FieldKind::Text | FieldKind::Number { .. } => {
                let value = menu.draft.pointer(&row.path).unwrap_or(&Value::Null);
                let buffer = value.as_str().map(str::to_owned).unwrap_or_else(|| {
                    if value.is_null() {
                        String::new()
                    } else {
                        value.to_string()
                    }
                });
                document.event_text_cursor = Some(buffer.len());
                document.text_entry = Some(EditorTextEntry {
                    target: EditorTextTarget::InspectorValue,
                    buffer,
                });
            }
            _ => adjust(document, &mut menu, row, 1),
        }
    }
    document.inspector = Some(menu);
}

/// Commit a scalar text entry back into the draft; failures keep the field open for correction. AI_CODE
pub(super) fn commit_value(document: &mut EditorDocument, buffer: &str) -> Result<(), String> {
    let mut menu = document.inspector.take().ok_or("No note menu")?;
    let rows = fields(document, &menu);
    let result = (|| {
        let row = rows.get(menu.focus).ok_or("No selected field")?;
        let value = match row.kind {
            FieldKind::Text => {
                if buffer.trim().is_empty() {
                    Value::Null
                } else {
                    json!(buffer.trim())
                }
            }
            FieldKind::Number {
                minimum,
                maximum,
                step,
            } => {
                let number = buffer.trim().parse::<f64>().map_err(|_| "Enter a number")?;
                if !number.is_finite()
                    || number < minimum
                    || number > maximum
                    || (step == 1.0 && number.fract() != 0.0)
                {
                    return Err(format!("Enter a value from {minimum} to {maximum}"));
                }
                if step == 1.0 {
                    json!(number as i64)
                } else {
                    json!(number)
                }
            }
            _ => return Err("This field uses arrow-key choices".into()),
        };
        set_value(&mut menu.draft, &row.path, value);
        Ok(())
    })();
    document.inspector = Some(menu);
    result
}

/// Format field values as musical labels rather than JSON syntax. AI_CODE
fn display_value(menu: &EventInspector, row: &Field) -> String {
    let value = menu.draft.pointer(&row.path).unwrap_or(&Value::Null);
    if let FieldKind::Articulation(mark) = row.kind {
        return if value
            .as_array()
            .is_some_and(|marks| marks.contains(&json!(mark)))
        {
            "On"
        } else {
            "Off"
        }
        .into();
    }
    if matches!(
        row.kind,
        FieldKind::Apply | FieldKind::AddPoint | FieldKind::ClearHints
    ) {
        return String::new();
    }
    if value.is_null() {
        return "None".into();
    }
    if row.path == "/length" {
        return match value.as_u64() {
            Some(1) => "Whole",
            Some(2) => "Half",
            Some(4) => "Quarter",
            Some(8) => "Eighth",
            _ => "Sixteenth",
        }
        .into();
    }
    if row.path == "/spelling" {
        let alter = value["alter"].as_i64().unwrap_or(0);
        let accidental = match alter {
            -2 => "bb",
            -1 => "b",
            1 => "#",
            2 => "##",
            _ => "",
        };
        return format!(
            "{}{accidental}{}",
            value["step"].as_str().unwrap_or("?"),
            value["octave"]
        );
    }
    if row.path == "/preferred_string" {
        return format!("String {}", value.as_u64().unwrap_or(0) + 1);
    }
    if let Some(flag) = value.as_bool() {
        return if flag { "On" } else { "Off" }.into();
    }
    value
        .as_str()
        .map(|text| text.replace('_', " "))
        .unwrap_or_else(|| value.to_string())
}

/// Display a fixed-size page of fields, keeping long menus within the editor viewport. AI_CODE
pub(super) fn display(document: &EditorDocument) -> Text {
    let Some(menu) = &document.inspector else {
        return Text::new("");
    };
    let rows = fields(document, menu);
    let page_start = menu.focus / 10 * 10;
    let section = if matches!(menu.target, EditorTextTarget::ScoreNotation(_)) {
        "NOTE NOTATION"
    } else {
        "PERFORMANCE AND GUIDANCE"
    };
    let source = match menu.target {
        EditorTextTarget::ScoreNotation(source) | EditorTextTarget::ScorePerformance(source) => {
            Some(source)
        }
        _ => None,
    };
    let event = document
        .chart
        .tracks
        .get(document.track_focus)
        .and_then(|track| source.and_then(|source| editor_source_event(track, source)));
    let identity = event
        .map(|event| {
            format!(
                "{}  |  Tick {}",
                event.id.as_deref().unwrap_or("Note"),
                event.start
            )
        })
        .unwrap_or_else(|| "Note unavailable".into());
    let rows_text = rows
        .iter()
        .enumerate()
        .skip(page_start)
        .take(10)
        .map(|(index, row)| {
            format!(
                "{} {:<28} {}",
                if index == menu.focus { ">" } else { " " },
                row.label,
                display_value(menu, row)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Text::new(format!(
        "{}  //  {section}\n{identity}  |  Page {} of {}\n\n{rows_text}\n\n{}",
        document.chart.title,
        page_start / 10 + 1,
        rows.len().div_ceil(10),
        document.status
    ))
}

/// Dynamic choices update only the draft until Apply commits, preserving pitch and hints. AI_CODE
#[test]
fn notation_menu_sets_mf_transactionally() {
    let mut document = new_chart_document();
    document
        .chart
        .tracks
        .push(new_track(InstrumentKind::Strings));
    assert!(sheet_edit_pitch(
        &mut document,
        0,
        61,
        &SheetTools::default()
    ));
    open(
        &mut document,
        EditorTextTarget::ScoreNotation(EventSource::Track(0)),
    )
    .unwrap();
    let mut menu = document.inspector.take().unwrap();
    let rows = fields(&document, &menu);
    let dynamic = rows.iter().find(|row| row.path == "/dynamic").unwrap();
    for _ in 0..4 {
        adjust(&document, &mut menu, dynamic, 1);
    }
    assert_eq!(menu.draft["dynamic"], "mf");
    assert!(document.chart.tracks[0].notes[0].dynamic.is_none());
    replace_score_section(&mut document, menu.target, &menu.draft.to_string()).unwrap();
    assert_eq!(
        document.chart.tracks[0].notes[0].dynamic,
        Some(DynamicLevel::Mf)
    );
    assert_eq!(document.chart.tracks[0].notes[0].note(), Some(61));
}

/// Keyboard menus select mf, apply, and cancel without forwarding keys to track navigation. AI_CODE
#[test]
fn note_menu_keyboard_applies_dynamic_and_cancels_draft() {
    let mut document = new_chart_document();
    document
        .chart
        .tracks
        .push(new_track(InstrumentKind::Strings));
    assert!(sheet_edit_pitch(
        &mut document,
        0,
        61,
        &SheetTools::default()
    ));
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
        let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keyboard.press(KeyCode::ControlLeft);
        keyboard.press(KeyCode::KeyE);
    }
    app.update();
    assert!(
        app.world()
            .resource::<EditorDocument>()
            .text_entry
            .is_none()
    );
    for key in [
        KeyCode::ArrowDown,
        KeyCode::ArrowRight,
        KeyCode::ArrowRight,
        KeyCode::ArrowRight,
        KeyCode::ArrowRight,
        KeyCode::End,
        KeyCode::Enter,
    ] {
        {
            let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keyboard.reset_all();
            keyboard.press(key);
        }
        app.update();
    }
    let document = app.world().resource::<EditorDocument>();
    assert!(document.inspector.is_none());
    assert_eq!(
        document.chart.tracks[0].notes[0].dynamic,
        Some(DynamicLevel::Mf)
    );
    assert_eq!(document.track_focus, 0);
    assert_eq!(document.current_tick, 0);
    let before = serde_json::to_string(&document.chart).unwrap();
    open(
        &mut app.world_mut().resource_mut::<EditorDocument>(),
        EditorTextTarget::ScorePerformance(EventSource::Track(0)),
    )
    .unwrap();
    for key in [KeyCode::ArrowRight, KeyCode::Escape] {
        {
            let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keyboard.reset_all();
            keyboard.press(key);
        }
        app.update();
    }
    let document = app.world().resource::<EditorDocument>();
    assert!(document.inspector.is_none());
    assert_eq!(serde_json::to_string(&document.chart).unwrap(), before);
}

/// Guidance choices derive matching frets and preserve all hidden or unrelated hints. AI_CODE
#[test]
fn performance_menu_preserves_hints_and_resolves_string() {
    let mut document = new_chart_document();
    document
        .chart
        .tracks
        .push(new_track(InstrumentKind::Strings));
    document.chart.tracks[0].tuning = Some(Tuning {
        strings: ["A0", "D1", "G1", "C2", "F2"].map(str::to_owned).to_vec(),
    });
    assert!(sheet_edit_pitch(
        &mut document,
        0,
        26,
        &SheetTools::default()
    ));
    replace_score_section(&mut document, EditorTextTarget::ScorePerformance(EventSource::Track(0)),
        r#"{"palm_mute":true,"vibrato":"wide","bend":{"semitones":2,"points":[{"offset":0,"semitones":0},{"offset":1,"semitones":2}]}}"#).unwrap();
    open(
        &mut document,
        EditorTextTarget::ScorePerformance(EventSource::Track(0)),
    )
    .unwrap();
    let mut menu = document.inspector.take().unwrap();
    let rows = fields(&document, &menu);
    adjust(
        &document,
        &mut menu,
        rows.iter()
            .find(|row| row.path == "/preferred_string")
            .unwrap(),
        1,
    );
    assert_eq!(menu.draft["preferred_string"], 0);
    assert_eq!(menu.draft["preferred_fret"], 5);
    adjust(
        &document,
        &mut menu,
        rows.iter().find(|row| row.path == "/attack").unwrap(),
        1,
    );
    replace_score_section(&mut document, menu.target, &menu.draft.to_string()).unwrap();
    let event = &document.chart.tracks[0].notes[0];
    let hints = event.performance.as_ref().unwrap();
    assert_eq!(hints.attack, Some(NoteAttack::Pluck));
    assert!(hints.palm_mute);
    assert_eq!(hints.vibrato, Some(VibratoKind::Wide));
    assert_eq!(hints.bend.as_ref().unwrap().points.len(), 2);
    assert_eq!(event.note(), Some(26));
    assert_eq!(event.start, 0);
}

/// Scalar entry uses plain numbers and validates before changing the draft. AI_CODE
#[test]
fn note_menu_numeric_entry_validates_and_returns_to_menu() {
    let mut document = new_chart_document();
    document
        .chart
        .tracks
        .push(new_track(InstrumentKind::Strings));
    assert!(sheet_edit_pitch(
        &mut document,
        0,
        61,
        &SheetTools::default()
    ));
    open(
        &mut document,
        EditorTextTarget::ScoreNotation(EventSource::Track(0)),
    )
    .unwrap();
    document.inspector.as_mut().unwrap().focus = 2;
    let mut keys = ButtonInput::<KeyCode>::default();
    keys.press(KeyCode::Enter);
    input(&mut document, &keys);
    assert!(matches!(
        document.text_entry.as_ref().unwrap().target,
        EditorTextTarget::InspectorValue
    ));
    assert!(commit_value(&mut document, "255").is_err());
    assert_eq!(document.inspector.as_ref().unwrap().draft["dots"], 0);
    commit_value(&mut document, "2").unwrap();
    assert_eq!(document.inspector.as_ref().unwrap().draft["dots"], 2);
    assert_eq!(document.chart.tracks[0].notes[0].dots, 0);
    let mut menu = document.inspector.take().unwrap();
    let row = fields(&document, &menu)
        .into_iter()
        .find(|row| row.path == "/dots")
        .unwrap();
    clear(&mut menu, &row);
    assert_eq!(menu.draft["dots"], 0);
}

/// Apply keeps invalid drafts open, and the displayed menu uses musical labels rather than JSON. AI_CODE
#[test]
fn note_menu_invalid_apply_retains_draft() {
    let mut document = new_chart_document();
    document
        .chart
        .tracks
        .push(new_track(InstrumentKind::Strings));
    assert!(sheet_edit_pitch(
        &mut document,
        0,
        61,
        &SheetTools::default()
    ));
    let before = serde_json::to_string(&document.chart).unwrap();
    open(
        &mut document,
        EditorTextTarget::ScoreNotation(EventSource::Track(0)),
    )
    .unwrap();
    document.inspector.as_mut().unwrap().draft["tuplet"] = json!({"actual":3,"normal":3});
    let mut keys = ButtonInput::<KeyCode>::default();
    keys.press(KeyCode::End);
    keys.press(KeyCode::Enter);
    input(&mut document, &keys);
    assert!(document.inspector.is_some());
    assert!(document.status.contains("tuplet"));
    assert_eq!(serde_json::to_string(&document.chart).unwrap(), before);
    keys.reset_all();
    keys.press(KeyCode::Home);
    input(&mut document, &keys);
    let text = display(&document);
    assert!(text.0.contains("Dynamic"));
    assert!(text.0.contains("Quarter"));
    assert!(!text.0.contains("{\""));
}

/// Kit-derived roll choices stay exclusive, and clearing hints preserves the percussion mapping. AI_CODE
#[test]
fn performance_menu_kit_choices_and_clear_hints() {
    let mut document = new_chart_document();
    document
        .chart
        .tracks
        .push(new_track(InstrumentKind::Percussion));
    let piece = document.chart.tracks[0].kit.as_ref().unwrap().pieces[0]
        .name
        .clone();
    document.chart.tracks[0].notes = serde_json::from_value(json!([
        {"id":"drum-note","start":0,"length":4,"piece":piece}
    ]))
    .unwrap();
    open(
        &mut document,
        EditorTextTarget::ScorePerformance(EventSource::Track(0)),
    )
    .unwrap();
    let mut menu = document.inspector.take().unwrap();
    let rows = fields(&document, &menu);
    assert!(!rows.iter().any(|row| row.path == "/preferred_string"));
    adjust(
        &document,
        &mut menu,
        rows.iter().find(|row| row.path == "/roll").unwrap(),
        1,
    );
    assert_eq!(menu.draft["roll"], piece);
    adjust(
        &document,
        &mut menu,
        rows.iter().find(|row| row.path == "/droll").unwrap(),
        1,
    );
    assert!(menu.draft["roll"].is_null());
    assert_eq!(menu.draft["droll"], piece);
    replace_score_section(&mut document, menu.target, &menu.draft.to_string()).unwrap();
    adjust(
        &document,
        &mut menu,
        rows.iter()
            .find(|row| matches!(row.kind, FieldKind::ClearHints))
            .unwrap(),
        1,
    );
    replace_score_section(&mut document, menu.target, &menu.draft.to_string()).unwrap();
    let event = &document.chart.tracks[0].notes[0];
    assert!(
        matches!(&event.content, NoteContent::Percussive { piece: mapped } if *mapped == piece)
    );
    assert!(event.performance.as_ref().unwrap().droll.is_none());
}
