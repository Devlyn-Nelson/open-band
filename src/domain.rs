use bevy::prelude::*;
use serde::{Deserialize, Deserializer, de};
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

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Chart {
    pub(crate) version: u8,
    pub(crate) title: String,
    pub(crate) bpm: f32,
    pub(crate) time_signature: [u8; 2],
    pub(crate) tracks: Vec<ChartTrack>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ChartTrack {
    pub(crate) name: String,
    pub(crate) instrument: String,
    #[serde(default)]
    pub(crate) tuning: Option<String>,
    #[serde(default)]
    pub(crate) notes: Vec<ChartEvent>,
    #[serde(default)]
    pub(crate) phrases: Vec<VocalPhrase>,
}

#[derive(Clone, Debug)]
pub(crate) struct ChartEvent {
    pub(crate) start_beat: f32,
    pub(crate) duration_beats: f32,
    pub(crate) midi_note: u8,
    pub(crate) preferred_string: Option<usize>,
}

#[derive(Deserialize)]
struct ChartEventFields {
    start_beat: f32,
    duration_beats: f32,
    #[serde(default)]
    midi_note: Option<u8>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    preferred_string: Option<usize>,
}

impl<'de> Deserialize<'de> for ChartEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = ChartEventFields::deserialize(deserializer)?;
        let midi_note = match (fields.midi_note, fields.note) {
            (Some(midi_note), None) => midi_note,
            (None, Some(note)) => parse_note_name(&note).map_err(de::Error::custom)?,
            (Some(midi_note), Some(note)) => {
                let named_note = parse_note_name(&note).map_err(de::Error::custom)?;
                if midi_note != named_note {
                    return Err(de::Error::custom(format!(
                        "midi_note {midi_note} does not match note {note}"
                    )));
                }
                midi_note
            }
            (None, None) => {
                return Err(de::Error::custom(
                    "chart event requires either midi_note or note",
                ));
            }
        };
        Ok(Self {
            start_beat: fields.start_beat,
            duration_beats: fields.duration_beats,
            midi_note,
            preferred_string: fields.preferred_string,
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct VocalPhrase {
    pub(crate) start_beat: f32,
    pub(crate) duration_beats: f32,
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

impl Chart {
    pub(crate) fn bass_notes(&self) -> Vec<ChartNoteData> {
        self.tracks
            .iter()
            .filter(|track| track.instrument.starts_with("bass"))
            .flat_map(|track| {
                let tuning = tuning_for(track.tuning.as_deref(), &track.instrument);
                track.notes.iter().filter_map(move |event| {
                    let Some((string, fret)) =
                        best_string_fret(event.midi_note, &tuning, event.preferred_string)
                    else {
                        eprintln!(
                            "Skipping unplayable bass note {}: no string/fret in tuning",
                            midi_note_name(event.midi_note)
                        );
                        return None;
                    };
                    Some(ChartNoteData {
                        start: event.start_beat * 60.0 / self.bpm,
                        duration: event.duration_beats * 60.0 / self.bpm,
                        string,
                        fret,
                        note: midi_note_name(event.midi_note),
                        pitch_hz: midi_to_frequency(event.midi_note),
                    })
                })
            })
            .collect()
    }

    pub(crate) fn first_instrument(&self) -> &str {
        self.tracks
            .first()
            .map_or("unknown", |track| track.instrument.as_str())
    }

    pub(crate) fn total_beats(&self) -> f32 {
        self.tracks
            .iter()
            .flat_map(|track| {
                track
                    .notes
                    .iter()
                    .map(|note| note.start_beat + note.duration_beats)
                    .chain(
                        track
                            .phrases
                            .iter()
                            .map(|phrase| phrase.start_beat + phrase.duration_beats),
                    )
            })
            .fold(0.0, f32::max)
    }
}

#[derive(Clone, Copy, Debug)]
struct Tuning {
    open_midi: &'static [u8],
}

fn tuning_for(name: Option<&str>, instrument: &str) -> Tuning {
    match name.unwrap_or(instrument) {
        "standard_bass_4" | "bass4" => Tuning {
            open_midi: &[28, 33, 38, 43],
        },
        "standard_bass_5" | "bass5" | "standard" => Tuning {
            open_midi: &[23, 28, 33, 38, 43],
        },
        _ => Tuning {
            open_midi: &[23, 28, 33, 38, 43],
        },
    }
}

fn best_string_fret(
    midi_note: u8,
    tuning: &Tuning,
    preferred_string: Option<usize>,
) -> Option<(usize, u8)> {
    let candidates = tuning
        .open_midi
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

#[derive(Component)]
pub(crate) struct ChartFeedbackText;

#[derive(Component)]
pub(crate) struct ChartReviewText;
