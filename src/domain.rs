use bevy::prelude::*;
use serde::{Deserialize, Deserializer, Serialize, de};
use std::fmt;

pub(crate) const OPEN_STRINGS_CHART: &str = include_str!("../charts/open-strings.json");
pub(crate) const CHART_HIT_LINE_Y: f32 = -250.0;
pub(crate) const CHART_NOTE_SPEED: f32 = 260.0;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoteDisplayMode {
    Fret,
    Note,
    Both,
}

pub(crate) const CHART_NOTE_DISPLAY: NoteDisplayMode = NoteDisplayMode::Both;

/// Generic instrument category a chart track belongs to; string count, kit layout, and
/// vocal range are all data (`Tuning`/`Kit`/`VocalRange`), not separate kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InstrumentKind {
    Strings,
    Percussion,
    Voice,
}

/// The notated rhythmic value of an event, expressed the way musicians say it: `1` for a
/// whole note, `2` for a half, `4` for a quarter, `8` for an eighth, `16` for a sixteenth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoteValue {
    Whole,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
}

impl NoteValue {
    fn ticks(self, resolution: u32) -> u32 {
        match self {
            NoteValue::Whole => resolution * 4,
            NoteValue::Half => resolution * 2,
            NoteValue::Quarter => resolution,
            NoteValue::Eighth => resolution / 2,
            NoteValue::Sixteenth => resolution / 4,
        }
    }
}

impl<'de> Deserialize<'de> for NoteValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match u8::deserialize(deserializer)? {
            1 => Ok(NoteValue::Whole),
            2 => Ok(NoteValue::Half),
            4 => Ok(NoteValue::Quarter),
            8 => Ok(NoteValue::Eighth),
            16 => Ok(NoteValue::Sixteenth),
            other => Err(de::Error::custom(format!(
                "invalid note length {other}; expected 1, 2, 4, 8, or 16"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NoteDynamics {
    Normal,
    Accent,
    Ghost,
}

impl Default for NoteDynamics {
    fn default() -> Self {
        NoteDynamics::Normal
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RollKind {
    SingleLane,
    DoubleLane,
}

/// A tempo change at a tick position; `tempo_map[0].start` should be `0`.
#[derive(Clone, Copy, Debug, Deserialize)]
pub(crate) struct TempoChange {
    pub(crate) start: u32,
    pub(crate) bpm: f32,
}

/// A time-signature change at a tick position; `time_signature_map[0].start` should be `0`.
#[derive(Clone, Copy, Debug, Deserialize)]
pub(crate) struct TimeSignatureChange {
    pub(crate) start: u32,
    pub(crate) numerator: u8,
    pub(crate) denominator: u8,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Chart {
    pub(crate) version: u8,
    pub(crate) title: String,
    /// Ticks per quarter note; the exact-integer basis for all event positions/durations.
    pub(crate) resolution: u32,
    pub(crate) tempo_map: Vec<TempoChange>,
    pub(crate) time_signature_map: Vec<TimeSignatureChange>,
    pub(crate) tracks: Vec<ChartTrack>,
}

/// Ordered, low-string-first open-string notes (octave-qualified, e.g. `"B0"`). Covers any
/// string count or tuning without code changes.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Tuning {
    pub(crate) strings: Vec<String>,
}

impl Tuning {
    fn open_midi(&self) -> Result<Vec<u8>, String> {
        self.strings.iter().map(|note| parse_note_name(note)).collect()
    }
}

/// A single physical piece in a percussion kit; pieces are referenced by `name` from chart
/// notes, not by index, so reordering a kit never invalidates existing notes.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct KitPiece {
    pub(crate) name: String,
    pub(crate) trigger: String,
    #[serde(default)]
    pub(crate) lane: Option<usize>,
    #[serde(default)]
    pub(crate) lane_span: Option<String>,
    #[serde(default)]
    pub(crate) symbol: Option<String>,
}

/// A percussion kit: a fixed lane count plus named pieces, some sharing a lane
/// (distinguished by `symbol`) and some spanning all lanes (`lane_span`, e.g. a kick).
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Kit {
    pub(crate) lanes: usize,
    pub(crate) pieces: Vec<KitPiece>,
}

impl Kit {
    fn piece(&self, name: &str) -> Option<&KitPiece> {
        self.pieces.iter().find(|piece| piece.name == name)
    }
}

/// A named `Tuning`/`Kit` an editor can offer for selection; presets are resolved and
/// copied into the chart on save, so a saved chart never depends on this list (see
/// todo.md Part A6). Not consumed by gameplay — editor convenience data only.
pub(crate) struct TuningPreset {
    pub(crate) name: &'static str,
    pub(crate) tuning: Tuning,
}

pub(crate) struct KitPreset {
    pub(crate) name: &'static str,
    pub(crate) kit: Kit,
}

fn tuning_of(strings: &[&str]) -> Tuning {
    Tuning {
        strings: strings.iter().map(|note| note.to_string()).collect(),
    }
}

/// Standard string tunings offered as editor presets.
pub(crate) fn standard_tuning_presets() -> Vec<TuningPreset> {
    vec![
        TuningPreset {
            name: "4-String Bass (EADG)",
            tuning: tuning_of(&["E1", "A1", "D2", "G2"]),
        },
        TuningPreset {
            name: "5-String Bass (BEADG)",
            tuning: tuning_of(&["B0", "E1", "A1", "D2", "G2"]),
        },
        TuningPreset {
            name: "Standard Guitar (EADGBE)",
            tuning: tuning_of(&["E2", "A2", "D3", "G3", "B3", "E4"]),
        },
        TuningPreset {
            name: "7-String Guitar (BEADGBE)",
            tuning: tuning_of(&["B1", "E2", "A2", "D3", "G3", "B3", "E4"]),
        },
    ]
}

/// Standard percussion kits offered as editor presets.
pub(crate) fn standard_kit_presets() -> Vec<KitPreset> {
    vec![KitPreset {
        name: "4-Lane Rock Kit",
        kit: Kit {
            lanes: 4,
            pieces: vec![
                KitPiece {
                    name: "kick".into(),
                    trigger: "midi:36".into(),
                    lane: None,
                    lane_span: Some("yellow".into()),
                    symbol: None,
                },
                KitPiece {
                    name: "snare".into(),
                    trigger: "midi:38".into(),
                    lane: Some(0),
                    lane_span: None,
                    symbol: Some("tom".into()),
                },
                KitPiece {
                    name: "tom1".into(),
                    trigger: "midi:48".into(),
                    lane: Some(1),
                    lane_span: None,
                    symbol: Some("tom".into()),
                },
                KitPiece {
                    name: "crash".into(),
                    trigger: "midi:49".into(),
                    lane: Some(1),
                    lane_span: None,
                    symbol: Some("cymbal".into()),
                },
                KitPiece {
                    name: "tom2".into(),
                    trigger: "midi:45".into(),
                    lane: Some(2),
                    lane_span: None,
                    symbol: Some("tom".into()),
                },
                KitPiece {
                    name: "ride".into(),
                    trigger: "midi:51".into(),
                    lane: Some(2),
                    lane_span: None,
                    symbol: Some("cymbal".into()),
                },
                KitPiece {
                    name: "floor_tom".into(),
                    trigger: "midi:41".into(),
                    lane: Some(3),
                    lane_span: None,
                    symbol: Some("tom".into()),
                },
            ],
        },
    }]
}

/// Descriptive-only vocal range; does not gate playability or detection.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct VocalRange {
    pub(crate) low: String,
    pub(crate) high: String,
}

/// A simple range marker; anything played during the span counts as part of the phrase.
/// Used for both Star Power phrases and (via `VocalPhrase`) vocal lyric phrases.
#[derive(Clone, Copy, Debug, Deserialize)]
pub(crate) struct Phrase {
    pub(crate) start: u32,
    pub(crate) duration_ticks: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ChartTrack {
    pub(crate) name: String,
    pub(crate) kind: InstrumentKind,
    #[serde(default)]
    pub(crate) tuning: Option<Tuning>,
    #[serde(default)]
    pub(crate) kit: Option<Kit>,
    #[serde(default)]
    pub(crate) vocal_range: Option<VocalRange>,
    #[serde(default)]
    pub(crate) notes: Vec<ChartEvent>,
    #[serde(default)]
    pub(crate) phrases: Vec<VocalPhrase>,
    #[serde(default)]
    pub(crate) star_power_phrases: Vec<Phrase>,
}

#[derive(Clone, Debug)]
pub(crate) enum NoteContent {
    Pitched {
        note: u8,
        ps: Option<usize>,
    },
    Percussive {
        piece: String,
        dynamics: NoteDynamics,
        roll: Option<RollKind>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct ChartEvent {
    pub(crate) start: u32,
    pub(crate) length: NoteValue,
    pub(crate) dots: u8,
    pub(crate) tied: bool,
    pub(crate) content: NoteContent,
}

impl ChartEvent {
    /// The nominal notated duration in ticks, before following any tie chain.
    pub(crate) fn duration_ticks(&self, resolution: u32) -> u32 {
        let base = self.length.ticks(resolution);
        let mut total = base;
        let mut addition = base;
        for _ in 0..self.dots {
            addition /= 2;
            total += addition;
        }
        total
    }

    pub(crate) fn note(&self) -> Option<u8> {
        match &self.content {
            NoteContent::Pitched { note, .. } => Some(*note),
            NoteContent::Percussive { .. } => None,
        }
    }
}

/// A pitch expressed either as a raw MIDI number or a readable name like `"B0"`.
#[derive(Deserialize)]
#[serde(untagged)]
enum NoteInput {
    Midi(u8),
    Named(String),
}

impl NoteInput {
    fn resolve(&self) -> Result<u8, String> {
        match self {
            NoteInput::Midi(value) => Ok(*value),
            NoteInput::Named(name) => parse_note_name(name),
        }
    }
}

#[derive(Deserialize)]
struct ChartEventFields {
    start: u32,
    length: NoteValue,
    #[serde(default)]
    dots: u8,
    #[serde(default)]
    tied: bool,
    #[serde(default)]
    note: Option<NoteInput>,
    #[serde(default)]
    ps: Option<usize>,
    #[serde(default)]
    piece: Option<String>,
    #[serde(default)]
    dynamics: Option<NoteDynamics>,
    #[serde(default)]
    roll: Option<RollKind>,
}

impl<'de> Deserialize<'de> for ChartEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = ChartEventFields::deserialize(deserializer)?;
        let is_pitched = fields.note.is_some();
        let is_percussive = fields.piece.is_some();
        let content = match (is_pitched, is_percussive) {
            (true, false) => {
                let note = fields
                    .note
                    .expect("checked by is_pitched")
                    .resolve()
                    .map_err(de::Error::custom)?;
                NoteContent::Pitched {
                    note,
                    ps: fields.ps,
                }
            }
            (false, true) => NoteContent::Percussive {
                piece: fields.piece.expect("checked by is_percussive"),
                dynamics: fields.dynamics.unwrap_or_default(),
                roll: fields.roll,
            },
            (true, true) => {
                return Err(de::Error::custom(
                    "chart event cannot mix pitched (note) and percussive (piece) fields",
                ));
            }
            (false, false) => {
                return Err(de::Error::custom(
                    "chart event requires either note (pitched) or piece (percussive)",
                ));
            }
        };
        Ok(Self {
            start: fields.start,
            length: fields.length,
            dots: fields.dots,
            tied: fields.tied,
            content,
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct VocalPhrase {
    pub(crate) start: u32,
    pub(crate) duration_ticks: u32,
    pub(crate) text: String,
    #[serde(default)]
    pub(crate) notes: Vec<ChartEvent>,
}

#[derive(Clone, Debug)]
pub(crate) struct ChartNoteData {
    pub(crate) start: f32,
    pub(crate) duration: f32,
    pub(crate) string: usize,
    pub(crate) fret: u8,
    pub(crate) note: String,
    pub(crate) pitch_hz: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct PercussionNoteData {
    pub(crate) start: f32,
    pub(crate) duration: f32,
    pub(crate) piece: String,
    pub(crate) lane: Option<usize>,
    pub(crate) lane_span_color: Option<String>,
    pub(crate) symbol: Option<String>,
    pub(crate) dynamics: NoteDynamics,
    pub(crate) roll: Option<RollKind>,
}

/// Sums the nominal duration of `events[index]` forward across a `tied` chain.
fn resolved_duration_ticks(events: &[ChartEvent], index: usize, resolution: u32) -> u32 {
    let mut total = events[index].duration_ticks(resolution);
    let mut current = index;
    while events[current].tied {
        let Some(next) = events.get(current + 1) else {
            break;
        };
        total += next.duration_ticks(resolution);
        current += 1;
    }
    total
}

impl Chart {
    /// The tempo at tick 0, defaulting to 120 BPM if `tempo_map` has no tick-0 entry.
    pub(crate) fn starting_bpm(&self) -> f32 {
        self.tempo_map
            .iter()
            .find(|change| change.start == 0)
            .map_or(120.0, |change| change.bpm)
    }

    /// The time signature at tick 0, defaulting to 4/4 if `time_signature_map` has no
    /// tick-0 entry.
    pub(crate) fn starting_time_signature(&self) -> [u8; 2] {
        self.time_signature_map
            .iter()
            .find(|change| change.start == 0)
            .map_or([4, 4], |change| [change.numerator, change.denominator])
    }

    /// Converts a tick position to elapsed seconds by walking the tempo map segment by
    /// segment, so tempo changes mid-song are handled correctly.
    pub(crate) fn tick_to_seconds(&self, tick: u32) -> f32 {
        let resolution = self.resolution.max(1) as f32;
        let mut changes = self.tempo_map.clone();
        changes.sort_by_key(|change| change.start);
        if changes.first().map_or(true, |first| first.start != 0) {
            changes.insert(0, TempoChange { start: 0, bpm: 120.0 });
        }
        let mut seconds = 0.0;
        let mut previous_tick = 0u32;
        let mut previous_bpm = changes[0].bpm;
        for change in changes.iter().skip(1) {
            if change.start >= tick {
                break;
            }
            seconds += (change.start - previous_tick) as f32 / resolution * 60.0 / previous_bpm;
            previous_tick = change.start;
            previous_bpm = change.bpm;
        }
        seconds += (tick - previous_tick) as f32 / resolution * 60.0 / previous_bpm;
        seconds
    }

    pub(crate) fn string_notes(&self) -> Vec<ChartNoteData> {
        self.tracks
            .iter()
            .filter(|track| track.kind == InstrumentKind::Strings)
            .flat_map(|track| self.string_notes_for_track(track))
            .collect()
    }

    fn string_notes_for_track(&self, track: &ChartTrack) -> Vec<ChartNoteData> {
        let Some(tuning) = track.tuning.as_ref() else {
            eprintln!("Strings track {} has no tuning; skipping", track.name);
            return Vec::new();
        };
        let open_midi = match tuning.open_midi() {
            Ok(open_midi) => open_midi,
            Err(error) => {
                eprintln!("Strings track {} has an invalid tuning: {error}", track.name);
                return Vec::new();
            }
        };
        track
            .notes
            .iter()
            .enumerate()
            .filter_map(|(index, event)| {
                let NoteContent::Pitched { note, ps } = &event.content else {
                    return None;
                };
                let Some((string, fret)) = best_string_fret(*note, &open_midi, *ps) else {
                    eprintln!(
                        "Skipping unplayable note {}: no string/fret in tuning",
                        midi_note_name(*note)
                    );
                    return None;
                };
                let duration_ticks = resolved_duration_ticks(&track.notes, index, self.resolution);
                let start = self.tick_to_seconds(event.start);
                let duration = self.tick_to_seconds(event.start + duration_ticks) - start;
                Some(ChartNoteData {
                    start,
                    duration,
                    string,
                    fret,
                    note: midi_note_name(*note),
                    pitch_hz: midi_to_frequency(*note),
                })
            })
            .collect()
    }

    pub(crate) fn percussion_notes(&self) -> Vec<PercussionNoteData> {
        self.tracks
            .iter()
            .filter(|track| track.kind == InstrumentKind::Percussion)
            .flat_map(|track| self.percussion_notes_for_track(track))
            .collect()
    }

    fn percussion_notes_for_track(&self, track: &ChartTrack) -> Vec<PercussionNoteData> {
        let Some(kit) = track.kit.as_ref() else {
            eprintln!("Percussion track {} has no kit; skipping", track.name);
            return Vec::new();
        };
        track
            .notes
            .iter()
            .enumerate()
            .filter_map(|(index, event)| {
                let NoteContent::Percussive {
                    piece,
                    dynamics,
                    roll,
                } = &event.content
                else {
                    return None;
                };
                let Some(kit_piece) = kit.piece(piece) else {
                    eprintln!(
                        "Skipping unknown percussion piece {piece} on track {}",
                        track.name
                    );
                    return None;
                };
                let duration_ticks = resolved_duration_ticks(&track.notes, index, self.resolution);
                let start = self.tick_to_seconds(event.start);
                let duration = self.tick_to_seconds(event.start + duration_ticks) - start;
                Some(PercussionNoteData {
                    start,
                    duration,
                    piece: piece.clone(),
                    lane: kit_piece.lane,
                    lane_span_color: kit_piece.lane_span.clone(),
                    symbol: kit_piece.symbol.clone(),
                    dynamics: *dynamics,
                    roll: *roll,
                })
            })
            .collect()
    }

    pub(crate) fn primary_track_name(&self) -> &str {
        self.tracks
            .first()
            .map_or("unknown", |track| track.name.as_str())
    }

    pub(crate) fn total_ticks(&self) -> u32 {
        self.tracks
            .iter()
            .flat_map(|track| {
                track
                    .notes
                    .iter()
                    .enumerate()
                    .map(|(index, note)| {
                        note.start + resolved_duration_ticks(&track.notes, index, self.resolution)
                    })
                    .chain(
                        track
                            .phrases
                            .iter()
                            .map(|phrase| phrase.start + phrase.duration_ticks),
                    )
                    .chain(
                        track
                            .star_power_phrases
                            .iter()
                            .map(|phrase| phrase.start + phrase.duration_ticks),
                    )
            })
            .max()
            .unwrap_or(0)
    }
}

fn best_string_fret(
    midi_note: u8,
    open_midi: &[u8],
    preferred_string: Option<usize>,
) -> Option<(usize, u8)> {
    let candidates = open_midi
        .iter()
        .enumerate()
        .filter_map(|(string, open)| {
            let fret = midi_note.checked_sub(*open)?;
            (fret <= 24).then_some((string, fret))
        })
        .collect::<Vec<_>>();
    preferred_string
        .and_then(|string| {
            candidates
                .iter()
                .copied()
                .find(|(candidate, _)| *candidate == string)
        })
        .or_else(|| candidates.into_iter().next())
}

pub(crate) fn midi_to_frequency(midi_note: u8) -> f32 {
    440.0 * 2.0_f32.powf((midi_note as f32 - 69.0) / 12.0)
}

pub(crate) fn midi_note_name(midi_note: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!("{}{}", NAMES[(midi_note % 12) as usize], midi_note / 12 - 1)
}

fn parse_note_name(note: &str) -> Result<u8, String> {
    let mut characters = note.trim().chars();
    let letter = characters
        .next()
        .ok_or_else(|| "note name cannot be empty".to_string())?
        .to_ascii_uppercase();
    let base = match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return Err(format!("invalid note name {note}")),
    };
    let accidental = match characters.clone().next() {
        Some('#') => {
            characters.next();
            1
        }
        Some('b') => {
            characters.next();
            -1
        }
        _ => 0,
    };
    let octave = characters
        .as_str()
        .parse::<i32>()
        .map_err(|_| format!("note {note} requires an octave, such as C4"))?;
    let semitone = base + accidental;
    let midi_note = (octave + 1) * 12 + semitone;
    u8::try_from(midi_note).map_err(|_| format!("note {note} is outside MIDI range"))
}

impl fmt::Display for ChartTrack {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.name)
    }
}


#[derive(Resource)]
pub(crate) struct SongMenuSelection {
    pub(crate) selected: usize,
    pub(crate) charts: Vec<Chart>,
}

#[derive(Resource)]
pub(crate) struct ChartSession {
    pub(crate) chart: Chart,
    pub(crate) started_at: f32,
}

#[derive(Resource, Default)]
pub(crate) struct ChartStats {
    pub(crate) total_notes: usize,
    pub(crate) correct_hits: usize,
    pub(crate) sustain_expected: usize,
    pub(crate) sustain_successful: usize,
    pub(crate) timing_offsets_ms: Vec<f32>,
}

#[derive(Resource, Default)]
pub(crate) struct ChartFeedback {
    pub(crate) message: String,
    pub(crate) until: f32,
}

#[derive(Component)]
pub(crate) struct SongMenuText;

#[derive(Component)]
pub(crate) struct ChartEntity;

#[derive(Component)]
pub(crate) struct ChartCountdownText;

#[derive(Component)]
pub(crate) struct ChartNoteVisual {
    pub(crate) string: usize,
    pub(crate) start: f32,
    pub(crate) duration: f32,
    pub(crate) pitch_hz: f32,
    pub(crate) matched: bool,
    pub(crate) missed: bool,
    pub(crate) sustain_observed: f32,
}

/// A percussion note rendered in the chart gameplay highway; visual only for now — hit
/// detection/scoring against live MIDI input is not yet wired up (see todo.md A8).
#[derive(Component)]
pub(crate) struct PercussionNoteVisual {
    pub(crate) lane: Option<usize>,
    pub(crate) spans_lanes: bool,
    pub(crate) start: f32,
    pub(crate) duration: f32,
}

#[derive(Component)]
pub(crate) struct ChartFeedbackText;

#[derive(Component)]
pub(crate) struct ChartReviewText;
