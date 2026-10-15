use serde::{Deserialize, Deserializer, Serialize, de};
use std::fmt;

/// AI_CODE
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoteDisplayMode {
    Fret,
    Note,
    Both,
}

/// AI_CODE
pub(crate) const CHART_NOTE_DISPLAY: NoteDisplayMode = NoteDisplayMode::Both;

/// Generic instrument category a chart track belongs to; string count, kit layout, and
/// vocal range are all data (`Tuning`/`Kit`/`VocalRange`), not separate kinds.
///
/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InstrumentKind {
    Strings,
    Percussion,
    Voice,
}

/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Clef {
    Treble,
    Bass,
    Alto,
    Tenor,
}

/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum KeyMode {
    Major,
    Minor,
}

/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct KeySignature {
    /// Number of sharps (positive) or flats (negative), from -7 through 7.
    pub(crate) fifths: i8,
    #[serde(default = "default_key_mode")]
    pub(crate) mode: KeyMode,
}

fn default_key_mode() -> KeyMode {
    KeyMode::Major
}

/// The notated rhythmic value of an event, expressed the way musicians say it: `1` for a
/// whole note, `2` for a half, `4` for a quarter, `8` for an eighth, `16` for a sixteenth.
///
/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoteValue {
    Whole,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
}

impl NoteValue {
    /// AI_CODE
    pub(crate) fn ticks(self, resolution: u32) -> u32 {
        match self {
            NoteValue::Whole => resolution * 4,
            NoteValue::Half => resolution * 2,
            NoteValue::Quarter => resolution,
            NoteValue::Eighth => resolution / 2,
            NoteValue::Sixteenth => resolution / 4,
        }
    }
}

/// A snap/grid unit for note placement, shared by the sheet and tab editor views
/// (todo.md B3). Reuses `NoteValue` directly since both are the same whole/half/quarter/
/// eighth/sixteenth palette; snapping only needs `resolution`, not `bpm`, so it stays
/// correct across tempo changes.
///
/// AI_CODE
pub(crate) type SnapInterval = NoteValue;

/// Rounds a tick position to the nearest multiple of `interval`'s tick length.
///
/// AI_CODE
pub(crate) fn snap_tick(tick: u32, interval: SnapInterval, resolution: u32) -> u32 {
    let step = interval.ticks(resolution).max(1);
    let remainder = tick % step;
    if remainder * 2 >= step {
        tick - remainder + step
    } else {
        tick - remainder
    }
}

#[test]
/// Rounds a tick position to the nearest snap-interval multiple, tempo-independent.
fn snap_tick_rounds_to_the_nearest_interval() {
    assert_eq!(snap_tick(100, SnapInterval::Quarter, 960), 0);
    assert_eq!(snap_tick(500, SnapInterval::Quarter, 960), 960);
    assert_eq!(snap_tick(960, SnapInterval::Quarter, 960), 960);
    assert_eq!(snap_tick(240, SnapInterval::Sixteenth, 960), 240);
}

/// AI_CODE
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

/// AI_CODE
impl Serialize for NoteValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let value: u8 = match self {
            NoteValue::Whole => 1,
            NoteValue::Half => 2,
            NoteValue::Quarter => 4,
            NoteValue::Eighth => 8,
            NoteValue::Sixteenth => 16,
        };
        serializer.serialize_u8(value)
    }
}

/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NoteDynamics {
    Normal,
    Accent,
    Ghost,
}

/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DynamicLevel {
    Ppp,
    Pp,
    Mp,
    Mf,
    F,
    Ff,
    Fff,
}

/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Articulation {
    Staccato,
    Tenuto,
    Marcato,
    Accent,
    Fermata,
    Grace,
}

/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GraceKind {
    Acciaccatura,
    Appoggiatura,
}

/// AI_CODE
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GraceSpec {
    pub(crate) kind: GraceKind,
    #[serde(default)]
    pub(crate) slash: bool,
    #[serde(default)]
    pub(crate) principal: Option<String>,
    #[serde(default)]
    pub(crate) order: u16,
}

/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StemDirection {
    Up,
    Down,
    Auto,
}

/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PercussionGrace {
    Flam,
    Drag,
}

/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PercussionTechnique {
    Buzz,
    CymbalChoke,
}

/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Sticking {
    Right,
    Left,
}

/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HiHatState {
    Open,
    Closed,
    Pedal,
}

/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HarmonicKind {
    Natural,
    Artificial,
    Pinch,
}

/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum VibratoKind {
    Normal,
    Wide,
    Narrow,
}

/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Tuplet {
    pub(crate) actual: u16,
    pub(crate) normal: u16,
}

impl Tuplet {
    /// AI_CODE
    pub(crate) fn valid(self) -> bool {
        self.actual > 0 && self.normal > 0 && self.actual != self.normal
    }
}

impl Default for NoteDynamics {
    fn default() -> Self {
        NoteDynamics::Normal
    }
}

/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NoteAttack {
    Pluck,
    Tap,
}

/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NoteTransition {
    HammerOn,
    PullOff,
    Slide,
}

/// AI_CODE
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct BendSpec {
    #[serde(default)]
    pub(crate) semitones: f32,
    #[serde(default)]
    pub(crate) release: bool,
    #[serde(default)]
    pub(crate) points: Vec<BendPoint>,
}

/// AI_CODE
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct BendPoint {
    /// Normalized position through the event, from 0.0 to 1.0.
    pub(crate) offset: f32,
    pub(crate) semitones: f32,
}

/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MotionKind {
    Trill,
}

/// AI_CODE
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct PitchMotion {
    pub(crate) kind: MotionKind,
    pub(crate) target: u8,
}

/// Optional Open Band instructions for turning a written event into a playable target.
/// These are deliberately separate from score pitch, rhythm, spelling, and notation.
///
/// AI_CODE
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct PerformanceHints {
    #[serde(default)]
    pub(crate) preferred_string: Option<usize>,
    #[serde(default)]
    pub(crate) attack: Option<NoteAttack>,
    #[serde(default)]
    pub(crate) transition: Option<NoteTransition>,
    #[serde(default)]
    pub(crate) bend: Option<BendSpec>,
    #[serde(default)]
    pub(crate) motion: Option<PitchMotion>,
    #[serde(default)]
    pub(crate) percussion_dynamics: Option<NoteDynamics>,
    #[serde(default)]
    pub(crate) roll: Option<String>,
    #[serde(default)]
    pub(crate) droll: Option<String>,
    #[serde(default)]
    pub(crate) percussion_grace: Option<PercussionGrace>,
    #[serde(default)]
    pub(crate) percussion_technique: Option<PercussionTechnique>,
    #[serde(default)]
    pub(crate) sticking: Option<Sticking>,
    #[serde(default)]
    pub(crate) hi_hat: Option<HiHatState>,
    #[serde(default)]
    pub(crate) harmonic: Option<HarmonicKind>,
    #[serde(default)]
    pub(crate) palm_mute: bool,
    #[serde(default)]
    pub(crate) vibrato: Option<VibratoKind>,
}

/// Written pitch spelling retained alongside the sounding MIDI value so enharmonic names
/// such as C-sharp and D-flat remain distinguishable to the notation layer.
///
/// AI_CODE
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PitchSpelling {
    pub(crate) step: char,
    pub(crate) alter: i8,
    pub(crate) octave: i8,
}

/// A tempo change at a tick position; `tempo_map[0].start` should be `0`.
///
/// AI_CODE
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct TempoChange {
    pub(crate) start: u32,
    pub(crate) bpm: f32,
}

/// A time-signature change at a tick position; `time_signature_map[0].start` should be `0`.
///
/// AI_CODE
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct TimeSignatureChange {
    pub(crate) start: u32,
    pub(crate) numerator: u8,
    pub(crate) denominator: u8,
}

/// AI_CODE
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct TempoText {
    pub(crate) tick: u32,
    pub(crate) text: String,
    #[serde(default)]
    pub(crate) bpm: Option<f32>,
}

/// AI_CODE
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ScoreExpression {
    Accelerando {
        start: u32,
        duration_ticks: u32,
        from_bpm: f32,
        to_bpm: f32,
    },
    Ritardando {
        start: u32,
        duration_ticks: u32,
        from_bpm: f32,
        to_bpm: f32,
    },
    Crescendo {
        start: u32,
        duration_ticks: u32,
    },
    Diminuendo {
        start: u32,
        duration_ticks: u32,
    },
}

/// Written score navigation markers. Runtime playback expands these into a linear sequence.
///
/// AI_CODE
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ScoreMarker {
    RepeatStart { tick: u32 },
    RepeatEnd { tick: u32, times: u16 },
    EndingStart { tick: u32, number: u16 },
    EndingEnd { tick: u32, number: u16 },
    Segno { tick: u32 },
    Coda { tick: u32 },
    Fine { tick: u32 },
    DaCapo { tick: u32 },
    DalSegno { tick: u32 },
    Rehearsal { tick: u32, label: String },
}

/// AI_CODE
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Chart {
    pub(crate) version: u8,
    pub(crate) title: String,
    /// Ticks per quarter note; the exact-integer basis for all event positions/durations.
    pub(crate) resolution: u32,
    pub(crate) tempo_map: Vec<TempoChange>,
    pub(crate) time_signature_map: Vec<TimeSignatureChange>,
    #[serde(default)]
    pub(crate) tempo_text: Vec<TempoText>,
    #[serde(default)]
    pub(crate) expressions: Vec<ScoreExpression>,
    #[serde(default)]
    pub(crate) structure: Vec<ScoreMarker>,
    pub(crate) tracks: Vec<ChartTrack>,
}

/// Ordered, low-string-first open-string notes (octave-qualified, e.g. `"B0"`). Covers any
/// string count or tuning without code changes.
///
/// AI_CODE
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Tuning {
    pub(crate) strings: Vec<String>,
}

impl Tuning {
    /// AI_CODE
    pub(crate) fn open_midi(&self) -> Result<Vec<u8>, String> {
        self.strings
            .iter()
            .map(|note| parse_note_name(note))
            .collect()
    }

    /// The real open-string frequencies for this tuning, used for lane assignment and
    /// per-string detection instead of guessing from a string count.
    ///
    /// AI_CODE
    pub(crate) fn open_frequencies(&self) -> Result<Vec<f32>, String> {
        Ok(self
            .open_midi()?
            .into_iter()
            .map(midi_to_frequency)
            .collect())
    }
}

impl Default for Tuning {
    /// Last-resort fallback when no tuning has been configured or loaded.
    fn default() -> Self {
        Tuning {
            strings: vec!["E1".into(), "A1".into(), "D2".into(), "G2".into()],
        }
    }
}

/// A single physical piece in a percussion kit; pieces are referenced by `name` from chart
/// notes, not by index, so reordering a kit never invalidates existing notes.
///
/// AI_CODE
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
///
/// AI_CODE
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Kit {
    pub(crate) lanes: usize,
    pub(crate) pieces: Vec<KitPiece>,
}

impl Kit {
    /// AI_CODE
    fn piece(&self, name: &str) -> Option<&KitPiece> {
        self.pieces.iter().find(|piece| piece.name == name)
    }
}

impl Default for Kit {
    /// Last-resort fallback when no kit has been configured or loaded: the standard rock
    /// kit (kick, snare, 3 toms, hi-hat, crash, ride) across 4 lanes.
    ///
    /// AI_CODE
    fn default() -> Self {
        serde_json::from_str(EMBEDDED_KITS[0]).expect("embedded default kit parses")
    }
}

/// A named tuning loaded from the `tunings/` library directory (one JSON file per
/// tuning, e.g. `{"name": "Standard Guitar", "strings": [...]}`). Presets are resolved
/// and copied into an `InstrumentSlot` on selection, so saved settings never depend on
/// this list still existing on disk (see todo.md Part A6).
///
/// AI_CODE
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct NamedTuning {
    pub(crate) name: String,
    pub(crate) strings: Vec<String>,
}

impl NamedTuning {
    pub(crate) fn tuning(&self) -> Tuning {
        Tuning {
            strings: self.strings.clone(),
        }
    }
}

/// Embedded fallback tunings, used if the `tunings/` directory is missing or empty so the
/// app still has sensible choices to offer.
const EMBEDDED_TUNINGS: &[&str] = &[
    include_str!("../../tunings/bass-4-standard.json"),
    include_str!("../../tunings/bass-5-standard.json"),
    include_str!("../../tunings/guitar-standard.json"),
    include_str!("../../tunings/guitar-7-standard.json"),
];

/// Loads every `*.json` file in the working directory's `tunings/` folder, in sorted
/// filename order. Invalid files are reported and skipped. Falls back to the embedded
/// defaults if the directory is missing or yields no valid tunings.
///
/// AI_CODE
pub(crate) fn load_tuning_library() -> Vec<NamedTuning> {
    let mut paths = std::fs::read_dir("tunings")
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

    let mut tunings = Vec::new();
    for path in paths {
        match std::fs::read_to_string(&path)
            .map_err(|error| error.to_string())
            .and_then(|contents| {
                serde_json::from_str::<NamedTuning>(&contents).map_err(|error| error.to_string())
            }) {
            Ok(tuning) => tunings.push(tuning),
            Err(error) => eprintln!("Could not load tuning {}: {error}", path.display()),
        }
    }
    if tunings.is_empty() {
        for embedded in EMBEDDED_TUNINGS {
            match serde_json::from_str(embedded) {
                Ok(tuning) => tunings.push(tuning),
                Err(error) => eprintln!("Could not load embedded starter tuning: {error}"),
            }
        }
    }
    tunings
}

/// A named percussion kit loaded from the `kits/` library directory (one JSON file per
/// kit, e.g. `{"name": "Standard Rock Kit", "lanes": 4, "pieces": [...]}`). Presets are
/// resolved and copied into an `InstrumentSlot` on selection, so saved settings never
/// depend on this list still existing on disk (see todo.md Part A6).
///
/// AI_CODE
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct NamedKit {
    pub(crate) name: String,
    pub(crate) lanes: usize,
    pub(crate) pieces: Vec<KitPiece>,
}

impl NamedKit {
    pub(crate) fn kit(&self) -> Kit {
        Kit {
            lanes: self.lanes,
            pieces: self.pieces.clone(),
        }
    }
}

/// Embedded fallback kits, used if the `kits/` directory is missing or empty so the app
/// still has sensible choices to offer.
const EMBEDDED_KITS: &[&str] = &[include_str!("../../kits/standard-rock.json")];

/// Loads every `*.json` file in the working directory's `kits/` folder, in sorted
/// filename order. Invalid files are reported and skipped. Falls back to the embedded
/// defaults if the directory is missing or yields no valid kits.
///
/// AI_CODE
pub(crate) fn load_kit_library() -> Vec<NamedKit> {
    let mut paths = std::fs::read_dir("kits")
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

    let mut kits = Vec::new();
    for path in paths {
        match std::fs::read_to_string(&path)
            .map_err(|error| error.to_string())
            .and_then(|contents| {
                serde_json::from_str::<NamedKit>(&contents).map_err(|error| error.to_string())
            }) {
            Ok(kit) => kits.push(kit),
            Err(error) => eprintln!("Could not load kit {}: {error}", path.display()),
        }
    }
    if kits.is_empty() {
        for embedded in EMBEDDED_KITS {
            match serde_json::from_str(embedded) {
                Ok(kit) => kits.push(kit),
                Err(error) => eprintln!("Could not load embedded starter kit: {error}"),
            }
        }
    }
    kits
}

#[test]
/// The kit library's pieces all reference a valid lane or a lane-spanning color, and the
/// standard rock kit specifically has 3 toms, snare, hi-hat, crash, ride, and a kick.
fn kit_library_pieces_are_placed_and_standard_kit_has_expected_pieces() {
    let library = load_kit_library();
    assert!(!library.is_empty());
    for named in &library {
        for piece in &named.pieces {
            assert!(
                piece.lane.is_some() != piece.lane_span.is_some(),
                "{} piece {} should have exactly one of lane/lane_span",
                named.name,
                piece.name
            );
            if let Some(lane) = piece.lane {
                assert!(
                    lane < named.lanes,
                    "{} piece {} lane out of range",
                    named.name,
                    piece.name
                );
            }
        }
    }

    let standard = library
        .iter()
        .find(|named| named.name == "Standard Rock Kit")
        .expect("standard rock kit should be in the library");
    let symbol_counts = |symbol: &str| {
        standard
            .pieces
            .iter()
            .filter(|piece| piece.symbol.as_deref() == Some(symbol))
            .count()
    };
    assert_eq!(
        symbol_counts("tom"),
        4,
        "expected snare + 3 toms tagged as toms"
    );
    assert_eq!(symbol_counts("cymbal"), 2, "expected crash and ride");
    assert_eq!(symbol_counts("hihat"), 1);
    assert!(
        standard
            .pieces
            .iter()
            .any(|piece| piece.name == "kick" && piece.lane_span.is_some()),
        "expected a lane-spanning kick"
    );
}

/// Descriptive-only vocal range; does not gate playability or detection.
///
/// AI_CODE
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct VocalRange {
    pub(crate) low: String,
    pub(crate) high: String,
}

/// AI_CODE
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct LyricVerse {
    pub(crate) number: u16,
    #[serde(default)]
    pub(crate) label: Option<String>,
    pub(crate) phrases: Vec<VocalPhrase>,
}

/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SyllableKind {
    Single,
    Begin,
    Middle,
    End,
}

/// A simple range marker; anything played during the span counts as part of the phrase.
/// Used for both Star Power phrases and (via `VocalPhrase`) vocal lyric phrases.
///
/// AI_CODE
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct Phrase {
    pub(crate) start: u32,
    pub(crate) duration_ticks: u32,
}

/// A relationship between two score events, identified by their stable event IDs.
///
/// AI_CODE
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EventRelation {
    pub(crate) from: String,
    pub(crate) to: String,
}

/// AI_CODE
#[derive(Clone, Debug, Serialize, Deserialize)]
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
    pub(crate) clef: Option<Clef>,
    #[serde(default)]
    pub(crate) key_signature: Option<KeySignature>,
    #[serde(default)]
    pub(crate) capo: Option<u8>,
    #[serde(default)]
    pub(crate) notes: Vec<ChartEvent>,
    #[serde(default)]
    pub(crate) ties: Vec<EventRelation>,
    #[serde(default)]
    pub(crate) slurs: Vec<EventRelation>,
    #[serde(default)]
    pub(crate) phrases: Vec<VocalPhrase>,
    #[serde(default)]
    pub(crate) star_power_phrases: Vec<Phrase>,
    #[serde(default)]
    pub(crate) lyric_verses: Vec<LyricVerse>,
}

/// AI_CODE
#[derive(Clone, Debug)]
pub(crate) enum NoteContent {
    Pitched {
        note: u8,
        spelling: Option<PitchSpelling>,
    },
    Percussive {
        piece: String,
    },
    Rest,
}

/// AI_CODE
#[derive(Clone, Debug)]
pub(crate) struct ChartEvent {
    pub(crate) id: Option<String>,
    pub(crate) start: u32,
    pub(crate) length: NoteValue,
    /// Voice and staff use one-based voice/staff numbers in the score model.
    pub(crate) voice: u8,
    pub(crate) staff: u8,
    pub(crate) dots: u8,
    pub(crate) tied: bool,
    pub(crate) tuplet: Option<Tuplet>,
    pub(crate) chord: Option<String>,
    pub(crate) dynamic: Option<DynamicLevel>,
    pub(crate) articulations: Vec<Articulation>,
    pub(crate) stem: Option<StemDirection>,
    pub(crate) grace: Option<GraceSpec>,
    pub(crate) performance: Option<PerformanceHints>,
    pub(crate) content: NoteContent,
}

impl ChartEvent {
    /// The nominal notated duration in ticks, before following any tie chain.
    ///
    /// AI_CODE
    pub(crate) fn duration_ticks(&self, resolution: u32) -> u32 {
        let base = self.length.ticks(resolution);
        let mut total = base;
        let mut addition = base;
        for _ in 0..self.dots {
            addition /= 2;
            total += addition;
        }
        self.tuplet.map_or(total, |tuplet| {
            total.saturating_mul(tuplet.normal as u32) / tuplet.actual.max(1) as u32
        })
    }

    /// AI_CODE
    pub(crate) fn note(&self) -> Option<u8> {
        match &self.content {
            NoteContent::Pitched { note, .. } => Some(*note),
            NoteContent::Percussive { .. } | NoteContent::Rest => None,
        }
    }
}

/// A pitch expressed either as a raw MIDI number or a readable name like `"B0"`.
///
/// AI_CODE
#[derive(Deserialize)]
#[serde(untagged)]
enum NoteInput {
    Midi(u8),
    Named(String),
}

impl NoteInput {
    /// AI_CODE
    fn resolve(&self) -> Result<u8, String> {
        match self {
            NoteInput::Midi(value) => Ok(*value),
            NoteInput::Named(name) => parse_note_name(name),
        }
    }

    /// AI_CODE
    fn spelling(&self) -> Result<Option<PitchSpelling>, String> {
        match self {
            NoteInput::Midi(_) => Ok(None),
            NoteInput::Named(name) => parse_pitch_spelling(name).map(Some),
        }
    }
}

/// AI_CODE
fn parse_pitch_spelling(note: &str) -> Result<PitchSpelling, String> {
    let mut characters = note.trim().chars();
    let step = characters
        .next()
        .ok_or_else(|| "note name cannot be empty".to_string())?
        .to_ascii_uppercase();
    if !matches!(step, 'A' | 'B' | 'C' | 'D' | 'E' | 'F' | 'G') {
        return Err(format!("invalid note name {note}"));
    }
    let alter = match characters.clone().next() {
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
        .parse::<i8>()
        .map_err(|_| format!("note {note} requires an octave, such as C4"))?;
    Ok(PitchSpelling {
        step,
        alter,
        octave,
    })
}

/// AI_CODE
#[derive(Deserialize)]
struct ChartEventFields {
    #[serde(default)]
    id: Option<String>,
    start: u32,
    length: NoteValue,
    #[serde(default = "default_voice")]
    voice: u8,
    #[serde(default = "default_staff")]
    staff: u8,
    #[serde(default)]
    dots: u8,
    #[serde(default)]
    tied: bool,
    #[serde(default)]
    tuplet: Option<Tuplet>,
    #[serde(default)]
    chord: Option<String>,
    #[serde(default)]
    dynamic: Option<DynamicLevel>,
    #[serde(default)]
    articulations: Vec<Articulation>,
    #[serde(default)]
    stem: Option<StemDirection>,
    #[serde(default)]
    grace: Option<GraceSpec>,
    #[serde(default)]
    performance: Option<PerformanceInput>,
    #[serde(default)]
    note: Option<NoteInput>,
    #[serde(default)]
    spelling: Option<PitchSpelling>,
    #[serde(default)]
    piece: Option<String>,
    #[serde(default)]
    rest: bool,
}

/// AI_CODE
fn default_voice() -> u8 {
    1
}

/// AI_CODE
fn default_staff() -> u8 {
    1
}

/// AI_CODE
#[derive(Deserialize)]
struct PitchMotionInput {
    kind: MotionKind,
    target: NoteInput,
}

/// AI_CODE
#[derive(Deserialize)]
struct PerformanceInput {
    #[serde(default)]
    preferred_string: Option<usize>,
    #[serde(default)]
    attack: Option<NoteAttack>,
    #[serde(default)]
    transition: Option<NoteTransition>,
    #[serde(default)]
    bend: Option<BendSpec>,
    #[serde(default)]
    motion: Option<PitchMotionInput>,
    #[serde(default)]
    percussion_dynamics: Option<NoteDynamics>,
    #[serde(default)]
    roll: Option<String>,
    #[serde(default)]
    droll: Option<String>,
    #[serde(default)]
    percussion_grace: Option<PercussionGrace>,
    #[serde(default)]
    percussion_technique: Option<PercussionTechnique>,
    #[serde(default)]
    sticking: Option<Sticking>,
    #[serde(default)]
    hi_hat: Option<HiHatState>,
    #[serde(default)]
    harmonic: Option<HarmonicKind>,
    #[serde(default)]
    palm_mute: bool,
    #[serde(default)]
    vibrato: Option<VibratoKind>,
}

impl PerformanceInput {
    /// AI_CODE
    fn resolve(self) -> Result<PerformanceHints, String> {
        let motion = self
            .motion
            .map(|motion| -> Result<PitchMotion, String> {
                Ok(PitchMotion {
                    kind: motion.kind,
                    target: motion.target.resolve()?,
                })
            })
            .transpose()?;
        if self.roll.is_some() && self.droll.is_some() {
            return Err("performance hints cannot contain both roll and droll".into());
        }
        Ok(PerformanceHints {
            preferred_string: self.preferred_string,
            attack: self.attack,
            transition: self.transition,
            bend: self.bend,
            motion,
            percussion_dynamics: self.percussion_dynamics,
            roll: self.roll,
            droll: self.droll,
            percussion_grace: self.percussion_grace,
            percussion_technique: self.percussion_technique,
            sticking: self.sticking,
            hi_hat: self.hi_hat,
            harmonic: self.harmonic,
            palm_mute: self.palm_mute,
            vibrato: self.vibrato,
        })
    }
}

/// AI_CODE
impl<'de> Deserialize<'de> for ChartEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = ChartEventFields::deserialize(deserializer)?;
        let performance = fields
            .performance
            .map(PerformanceInput::resolve)
            .transpose()
            .map_err(de::Error::custom)?;
        let is_pitched = fields.note.is_some();
        let is_percussive = fields.piece.is_some();
        if fields.rest && (is_pitched || is_percussive) {
            return Err(de::Error::custom(
                "chart rest events cannot also contain note or piece",
            ));
        }
        if let Some(hints) = &performance {
            if is_pitched
                && (hints.percussion_dynamics.is_some()
                    || hints.roll.is_some()
                    || hints.droll.is_some())
            {
                return Err(de::Error::custom(
                    "pitched chart events cannot contain percussion performance hints",
                ));
            }
            if is_percussive
                && (hints.preferred_string.is_some()
                    || hints.attack.is_some()
                    || hints.transition.is_some()
                    || hints.bend.is_some()
                    || hints.motion.is_some())
            {
                return Err(de::Error::custom(
                    "percussive chart events cannot contain string performance hints",
                ));
            }
        }
        let content = if fields.rest {
            if performance.is_some() {
                return Err(de::Error::custom(
                    "chart rest events cannot contain performance hints",
                ));
            }
            NoteContent::Rest
        } else {
            match (is_pitched, is_percussive) {
                (true, false) => {
                    let note_input = fields.note.expect("checked by is_pitched");
                    let note = note_input.resolve().map_err(de::Error::custom)?;
                    let spelling = fields
                        .spelling
                        .or(note_input.spelling().map_err(de::Error::custom)?);
                    NoteContent::Pitched { note, spelling }
                }
                (false, true) => NoteContent::Percussive {
                    piece: fields.piece.expect("checked by is_percussive"),
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
            }
        };
        Ok(Self {
            id: fields.id,
            start: fields.start,
            length: fields.length,
            voice: fields.voice,
            staff: fields.staff,
            dots: fields.dots,
            tied: fields.tied,
            tuplet: fields.tuplet,
            chord: fields.chord,
            dynamic: fields.dynamic,
            articulations: fields.articulations,
            stem: fields.stem,
            grace: fields.grace,
            performance,
            content,
        })
    }
}

/// AI_CODE
impl Serialize for ChartEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct Fields<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            id: Option<&'a str>,
            start: u32,
            length: NoteValue,
            #[serde(skip_serializing_if = "is_default_voice")]
            voice: u8,
            #[serde(skip_serializing_if = "is_default_staff")]
            staff: u8,
            #[serde(skip_serializing_if = "is_zero")]
            dots: u8,
            #[serde(skip_serializing_if = "is_false")]
            tied: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            tuplet: Option<Tuplet>,
            #[serde(skip_serializing_if = "Option::is_none")]
            chord: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            dynamic: Option<DynamicLevel>,
            #[serde(skip_serializing_if = "slice_is_empty")]
            articulations: &'a [Articulation],
            #[serde(skip_serializing_if = "Option::is_none")]
            stem: Option<StemDirection>,
            #[serde(skip_serializing_if = "Option::is_none")]
            grace: Option<GraceSpec>,
            #[serde(skip_serializing_if = "Option::is_none")]
            performance: Option<&'a PerformanceHints>,
            #[serde(skip_serializing_if = "Option::is_none")]
            note: Option<u8>,
            #[serde(skip_serializing_if = "Option::is_none")]
            spelling: Option<&'a PitchSpelling>,
            #[serde(skip_serializing_if = "Option::is_none")]
            piece: Option<&'a str>,
            #[serde(skip_serializing_if = "is_false")]
            rest: bool,
        }
        fn is_zero(value: &u8) -> bool {
            *value == 0
        }
        fn is_false(value: &bool) -> bool {
            !*value
        }
        fn is_default_voice(value: &u8) -> bool {
            *value == 1
        }
        fn is_default_staff(value: &u8) -> bool {
            *value == 1
        }
        fn slice_is_empty(value: &&[Articulation]) -> bool {
            value.is_empty()
        }
        let (note, spelling, piece, rest) = match &self.content {
            NoteContent::Pitched { note, spelling } => {
                (Some(*note), spelling.as_ref(), None, false)
            }
            NoteContent::Percussive { piece } => (None, None, Some(piece.as_str()), false),
            NoteContent::Rest => (None, None, None, true),
        };
        Fields {
            id: self.id.as_deref(),
            start: self.start,
            length: self.length,
            voice: self.voice,
            staff: self.staff,
            dots: self.dots,
            tied: self.tied,
            tuplet: self.tuplet,
            chord: self.chord.as_deref(),
            dynamic: self.dynamic,
            articulations: &self.articulations,
            stem: self.stem,
            grace: self.grace.clone(),
            performance: self.performance.as_ref(),
            note,
            spelling,
            piece,
            rest,
        }
        .serialize(serializer)
    }
}

/// AI_CODE
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct VocalPhrase {
    pub(crate) start: u32,
    pub(crate) duration_ticks: u32,
    pub(crate) text: String,
    #[serde(default)]
    pub(crate) syllable: Option<SyllableKind>,
    #[serde(default)]
    pub(crate) melisma: bool,
    #[serde(default)]
    pub(crate) hyphen_after: bool,
    #[serde(default)]
    pub(crate) breath_after: bool,
    #[serde(default)]
    pub(crate) notes: Vec<ChartEvent>,
}

/// AI_CODE
#[derive(Clone, Debug)]
pub(crate) struct ChartNoteData {
    pub(crate) start: f32,
    pub(crate) duration: f32,
    pub(crate) string: usize,
    pub(crate) fret: u8,
    pub(crate) note: String,
    pub(crate) pitch_hz: f32,
    pub(crate) attack: Option<NoteAttack>,
    pub(crate) transition: Option<NoteTransition>,
    pub(crate) bend: Option<BendSpec>,
    pub(crate) motion: Option<PitchMotion>,
}

/// AI_CODE
#[derive(Clone, Debug)]
pub(crate) struct PercussionNoteData {
    pub(crate) start: f32,
    pub(crate) duration: f32,
    pub(crate) piece: String,
    pub(crate) lane: Option<usize>,
    pub(crate) lane_span_color: Option<String>,
    pub(crate) symbol: Option<String>,
    pub(crate) dynamics: NoteDynamics,
    pub(crate) dynamic: Option<DynamicLevel>,
    pub(crate) articulations: Vec<Articulation>,
    pub(crate) roll: Option<String>,
    pub(crate) droll: Option<String>,
}

/// Sums the nominal duration of `events[index]` forward across a `tied` chain.
///
/// AI_CODE
pub(crate) fn resolved_duration_ticks(events: &[ChartEvent], index: usize, resolution: u32) -> u32 {
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

/// AI_CODE
fn tie_content_matches(left: &ChartEvent, right: &ChartEvent) -> bool {
    match (&left.content, &right.content) {
        (NoteContent::Pitched { note: left, .. }, NoteContent::Pitched { note: right, .. }) => {
            left == right
        }
        (
            NoteContent::Percussive { piece: left, .. },
            NoteContent::Percussive { piece: right, .. },
        ) => left == right,
        (NoteContent::Rest, NoteContent::Rest) => true,
        _ => false,
    }
}

impl Chart {
    /// The tempo at tick 0, defaulting to 120 BPM if `tempo_map` has no tick-0 entry.
    ///
    /// AI_CODE
    pub(crate) fn starting_bpm(&self) -> f32 {
        self.tempo_map
            .iter()
            .find(|change| change.start == 0)
            .map_or(120.0, |change| change.bpm)
    }

    /// The time signature at tick 0, defaulting to 4/4 if `time_signature_map` has no
    /// tick-0 entry.
    ///
    /// AI_CODE
    pub(crate) fn starting_time_signature(&self) -> [u8; 2] {
        self.time_signature_map
            .iter()
            .find(|change| change.start == 0)
            .map_or([4, 4], |change| [change.numerator, change.denominator])
    }

    /// Converts a tick position to elapsed seconds by walking the tempo map segment by
    /// segment, so tempo changes mid-song are handled correctly.
    ///
    /// AI_CODE
    pub(crate) fn tick_to_seconds(&self, tick: u32) -> f32 {
        let resolution = self.resolution.max(1) as f32;
        let mut changes = self.tempo_map.clone();
        changes.sort_by_key(|change| change.start);
        if changes.first().map_or(true, |first| first.start != 0) {
            changes.insert(
                0,
                TempoChange {
                    start: 0,
                    bpm: 120.0,
                },
            );
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

    /// AI_CODE
    pub(crate) fn string_notes(&self) -> Vec<ChartNoteData> {
        self.tracks
            .iter()
            .filter(|track| track.kind == InstrumentKind::Strings)
            .flat_map(|track| self.string_notes_for_track(track))
            .collect()
    }

    /// AI_CODE
    fn string_notes_for_track(&self, track: &ChartTrack) -> Vec<ChartNoteData> {
        let Some(tuning) = track.tuning.as_ref() else {
            eprintln!("Strings track {} has no tuning; skipping", track.name);
            return Vec::new();
        };
        let open_midi = match tuning.open_midi() {
            Ok(open_midi) => open_midi,
            Err(error) => {
                eprintln!(
                    "Strings track {} has an invalid tuning: {error}",
                    track.name
                );
                return Vec::new();
            }
        };
        track
            .notes
            .iter()
            .enumerate()
            .filter_map(|(index, event)| {
                let NoteContent::Pitched { note, .. } = &event.content else {
                    return None;
                };
                let preferred_string = event
                    .performance
                    .as_ref()
                    .and_then(|hints| hints.preferred_string);
                let Some((string, fret)) = best_string_fret(*note, &open_midi, preferred_string)
                else {
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
                    attack: event.performance.as_ref().and_then(|hints| hints.attack),
                    transition: event
                        .performance
                        .as_ref()
                        .and_then(|hints| hints.transition),
                    bend: event
                        .performance
                        .as_ref()
                        .and_then(|hints| hints.bend.clone()),
                    motion: event
                        .performance
                        .as_ref()
                        .and_then(|hints| hints.motion.clone()),
                })
            })
            .collect()
    }

    /// AI_CODE
    pub(crate) fn percussion_notes(&self) -> Vec<PercussionNoteData> {
        self.tracks
            .iter()
            .filter(|track| track.kind == InstrumentKind::Percussion)
            .flat_map(|track| self.percussion_notes_for_track(track))
            .collect()
    }

    /// AI_CODE
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
                if let Some(hints) = &event.performance {
                    for target in hints.roll.iter().chain(hints.droll.iter()) {
                        if kit.piece(target).is_none() {
                            eprintln!(
                                "Skipping {target} performance target on track {}: unknown percussion piece",
                                track.name
                            );
                            return None;
                        }
                    }
                }
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
                    dynamics: event
                        .performance
                        .as_ref()
                        .and_then(|hints| hints.percussion_dynamics)
                        .unwrap_or(NoteDynamics::Normal),
                    dynamic: event.dynamic,
                    articulations: event.articulations.clone(),
                    roll: event
                        .performance
                        .as_ref()
                        .and_then(|hints| hints.roll.clone())
                        ,
                    droll: event
                        .performance
                        .as_ref()
                        .and_then(|hints| hints.droll.clone())
                        ,
                })
            })
            .collect()
    }

    /// AI_CODE
    pub(crate) fn primary_track_name(&self) -> &str {
        self.tracks
            .first()
            .map_or("unknown", |track| track.name.as_str())
    }

    /// AI_CODE
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

    /// Human-readable warnings for problems that would otherwise be silently skipped at
    /// load time (unplayable notes, missing tuning/kit, or unknown percussion pieces and
    /// roll targets).
    /// Non-blocking: intended for the editor to surface before saving.
    ///
    /// AI_CODE
    pub(crate) fn validate(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        if !self.tempo_map.iter().any(|change| change.start == 0) {
            warnings.push("tempo_map has no entry at tick 0; defaulting to 120 BPM".into());
        }
        if !self
            .time_signature_map
            .iter()
            .any(|change| change.start == 0)
        {
            warnings.push("time_signature_map has no entry at tick 0; defaulting to 4/4".into());
        }
        let score_end = self.total_ticks();
        for tempo in &self.tempo_text {
            if tempo.tick > score_end {
                warnings.push(format!(
                    "tempo text at tick {} is beyond the end of the score",
                    tempo.tick
                ));
            }
            if tempo.text.trim().is_empty() {
                warnings.push(format!(
                    "tempo text at tick {} has an empty label",
                    tempo.tick
                ));
            }
            if tempo.bpm.is_some_and(|bpm| !bpm.is_finite() || bpm <= 0.0) {
                warnings.push(format!(
                    "tempo text at tick {} has an invalid BPM",
                    tempo.tick
                ));
            }
        }
        for expression in &self.expressions {
            let (start, duration_ticks, tempo_range) = match expression {
                ScoreExpression::Accelerando {
                    start,
                    duration_ticks,
                    from_bpm,
                    to_bpm,
                }
                | ScoreExpression::Ritardando {
                    start,
                    duration_ticks,
                    from_bpm,
                    to_bpm,
                } => (*start, *duration_ticks, Some((*from_bpm, *to_bpm))),
                ScoreExpression::Crescendo {
                    start,
                    duration_ticks,
                }
                | ScoreExpression::Diminuendo {
                    start,
                    duration_ticks,
                } => (*start, *duration_ticks, None),
            };
            if duration_ticks == 0 || start.saturating_add(duration_ticks) > score_end {
                warnings.push(format!(
                    "score expression at tick {start} has an invalid range"
                ));
            }
            if let Some((from_bpm, to_bpm)) = tempo_range
                && (!from_bpm.is_finite()
                    || !to_bpm.is_finite()
                    || from_bpm <= 0.0
                    || to_bpm <= 0.0)
            {
                warnings.push(format!(
                    "tempo expression at tick {start} has an invalid BPM range"
                ));
            }
        }
        for marker in &self.structure {
            let tick = match marker {
                ScoreMarker::RepeatStart { tick }
                | ScoreMarker::RepeatEnd { tick, .. }
                | ScoreMarker::EndingStart { tick, .. }
                | ScoreMarker::EndingEnd { tick, .. }
                | ScoreMarker::Segno { tick }
                | ScoreMarker::Coda { tick }
                | ScoreMarker::Fine { tick }
                | ScoreMarker::DaCapo { tick }
                | ScoreMarker::DalSegno { tick }
                | ScoreMarker::Rehearsal { tick, .. } => *tick,
            };
            if tick > score_end {
                warnings.push(format!(
                    "score marker at tick {tick} is beyond the end of the score"
                ));
            }
            match marker {
                ScoreMarker::RepeatEnd { times, .. } if *times < 2 => {
                    warnings.push("repeat end must have times >= 2".into());
                }
                ScoreMarker::EndingStart { number, .. } | ScoreMarker::EndingEnd { number, .. }
                    if *number == 0 =>
                {
                    warnings.push("numbered ending must have number >= 1".into());
                }
                ScoreMarker::Rehearsal { label, .. } if label.trim().is_empty() => {
                    warnings.push("rehearsal marker must have a label".into());
                }
                _ => {}
            }
        }
        for track in &self.tracks {
            if track.capo.is_some_and(|capo| capo > 24) {
                warnings.push(format!("track \"{}\" has a capo above fret 24", track.name));
            }
            let event_ids = track
                .notes
                .iter()
                .filter_map(|event| event.id.as_deref())
                .collect::<Vec<_>>();
            for (index, event) in track.notes.iter().enumerate() {
                if event.grace.is_some() && matches!(event.content, NoteContent::Rest) {
                    warnings.push(format!(
                        "track \"{}\" rest {index} cannot be a grace event",
                        track.name
                    ));
                }
                if let Some(grace) = &event.grace {
                    match grace.principal.as_deref() {
                        Some(principal) if !event_ids.contains(&principal) => warnings.push(format!(
                            "track \"{}\" grace note {index} references an unknown principal event",
                            track.name
                        )),
                        None => warnings.push(format!(
                            "track \"{}\" grace note {index} has no principal event",
                            track.name
                        )),
                        _ => {}
                    }
                }
                if let Some(id) = event.id.as_deref() {
                    if id.is_empty() {
                        warnings.push(format!(
                            "track \"{}\" note {index} has an empty event ID",
                            track.name
                        ));
                    }
                    if event_ids
                        .iter()
                        .filter(|candidate| **candidate == id)
                        .count()
                        > 1
                    {
                        warnings.push(format!(
                            "track \"{}\" has duplicate event ID \"{id}\"",
                            track.name
                        ));
                    }
                }
            }
            for (kind, relations) in [("tie", &track.ties), ("slur", &track.slurs)] {
                for relation in relations {
                    if !event_ids.contains(&relation.from.as_str())
                        || !event_ids.contains(&relation.to.as_str())
                    {
                        warnings.push(format!(
                            "track \"{}\" {kind} references an unknown event ID",
                            track.name
                        ));
                    }
                }
            }
            if let Some(key_signature) = track.key_signature
                && !(-7..=7).contains(&key_signature.fifths)
            {
                warnings.push(format!(
                    "track \"{}\" has a key signature outside the -7..7 range",
                    track.name
                ));
            }
            for (index, event) in track.notes.iter().enumerate() {
                if let Some(tuplet) = event.tuplet
                    && !tuplet.valid()
                {
                    warnings.push(format!(
                        "track \"{}\" note {index} has an invalid tuplet ratio",
                        track.name
                    ));
                }
                if event.dots > 2 {
                    warnings.push(format!(
                        "track \"{}\" note {index} has more than two dots",
                        track.name
                    ));
                }
                if let Some(bend) = event
                    .performance
                    .as_ref()
                    .and_then(|hints| hints.bend.as_ref())
                {
                    if bend.points.is_empty() && bend.semitones == 0.0 {
                        warnings.push(format!(
                            "track \"{}\" note {index} has an empty bend",
                            track.name
                        ));
                    }
                    if bend.points.windows(2).any(|points| {
                        points[0].offset > points[1].offset
                            || !points[0].offset.is_finite()
                            || !points[1].offset.is_finite()
                    }) || bend.points.iter().any(|point| {
                        !(0.0..=1.0).contains(&point.offset) || !point.semitones.is_finite()
                    }) {
                        warnings.push(format!(
                            "track \"{}\" note {index} has invalid bend curve points",
                            track.name
                        ));
                    }
                }
                if event.tied {
                    match track.notes.get(index + 1) {
                        None => warnings.push(format!(
                            "track \"{}\" note {index} starts a tie without a following event",
                            track.name
                        )),
                        Some(next) => {
                            let expected_start =
                                event.start + event.duration_ticks(self.resolution);
                            if next.start != expected_start {
                                warnings.push(format!(
                                    "track \"{}\" note {index} tie does not start at the previous note's end",
                                    track.name
                                ));
                            }
                            if !tie_content_matches(event, next) {
                                warnings.push(format!(
                                    "track \"{}\" note {index} tie changes pitch or piece",
                                    track.name
                                ));
                            }
                        }
                    }
                }
            }
            for chord_id in track
                .notes
                .iter()
                .filter_map(|event| event.chord.as_deref())
                .collect::<Vec<_>>()
            {
                let members = track
                    .notes
                    .iter()
                    .filter(|event| event.chord.as_deref() == Some(chord_id))
                    .collect::<Vec<_>>();
                if members.len() < 2 {
                    warnings.push(format!(
                        "track \"{}\" chord \"{chord_id}\" has fewer than two events",
                        track.name
                    ));
                    continue;
                }
                let first = members[0];
                if members.iter().any(|event| {
                    event.start != first.start
                        || event.duration_ticks(self.resolution)
                            != first.duration_ticks(self.resolution)
                        || event.voice != first.voice
                        || event.staff != first.staff
                }) {
                    warnings.push(format!(
                        "track \"{}\" chord \"{chord_id}\" has inconsistent timing or voice/staff",
                        track.name
                    ));
                }
            }
            match track.kind {
                InstrumentKind::Strings => match track.tuning.as_ref() {
                    None => warnings.push(format!(
                        "track \"{}\" is Strings but has no tuning",
                        track.name
                    )),
                    Some(tuning) => match tuning.open_midi() {
                        Err(error) => {
                            warnings.push(format!(
                                "track \"{}\" tuning is invalid: {error}",
                                track.name
                            ));
                        }
                        Ok(open_midi) => {
                            for note in &track.notes {
                                if let NoteContent::Pitched { note: pitch, .. } = &note.content
                                    && best_string_fret(
                                        *pitch,
                                        &open_midi,
                                        note.performance
                                            .as_ref()
                                            .and_then(|hints| hints.preferred_string),
                                    )
                                    .is_none()
                                {
                                    warnings.push(format!(
                                        "track \"{}\" has an unplayable note {}",
                                        track.name,
                                        midi_note_name(*pitch)
                                    ));
                                }
                            }
                        }
                    },
                },
                InstrumentKind::Percussion => match track.kit.as_ref() {
                    None => warnings.push(format!(
                        "track \"{}\" is Percussion but has no kit",
                        track.name
                    )),
                    Some(kit) => {
                        for note in &track.notes {
                            if let NoteContent::Percussive { piece } = &note.content {
                                if kit.piece(piece).is_none() {
                                    warnings.push(format!(
                                        "track \"{}\" references unknown percussion piece \"{piece}\"",
                                        track.name
                                    ));
                                }
                                if let Some(hints) = &note.performance {
                                    for target in hints.roll.iter().chain(hints.droll.iter()) {
                                        if kit.piece(target).is_none() {
                                            warnings.push(format!(
                                                "track \"{}\" references unknown performance roll target \"{target}\"",
                                                track.name
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                InstrumentKind::Voice => {}
            }
        }
        warnings
    }
}

/// AI_CODE
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

/// AI_CODE
pub(crate) fn midi_to_frequency(midi_note: u8) -> f32 {
    440.0 * 2.0_f32.powf((midi_note as f32 - 69.0) / 12.0)
}

/// AI_CODE
pub(crate) fn midi_note_name(midi_note: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!("{}{}", NAMES[(midi_note % 12) as usize], midi_note / 12 - 1)
}

/// AI_CODE
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

#[test]
fn bass_projection_falls_back_when_preferred_string_cannot_play_note() {
    let chart: Chart = serde_json::from_str(
        r#"
                {
                    "version": 1,
                    "title": "Projection",
                    "resolution": 960,
                    "tempo_map": [{ "start": 0, "bpm": 90.0 }],
                    "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
                    "tracks": [{
                        "name": "Bass",
                        "kind": "strings",
                        "tuning": { "strings": ["B0", "E1", "A1", "D2", "G2"] },
                        "notes": [{
                            "start": 960,
                            "length": 4,
                            "note": "C1",
                            "performance": { "preferred_string": 2 }
                        }]
                    }]
                }
                "#,
    )
    .expect("projection chart should parse");

    let notes = chart.string_notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].string, 0);
    assert_eq!(notes[0].fret, 1);
}

#[test]
fn string_techniques_parse_resolve_and_round_trip() {
    let chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Techniques",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": [{
                "name": "Bass",
                "kind": "strings",
                "tuning": { "strings": ["E1", "A1", "D2", "G2"] },
                "notes": [{
                    "start": 0,
                    "length": 4,
                    "note": "E2",
                    "performance": {
                        "preferred_string": 0,
                        "attack": "tap",
                        "transition": "slide",
                        "bend": { "semitones": 2.0, "release": true },
                        "motion": { "kind": "trill", "target": "F#2" }
                    }
                }]
            }]
        }
        "#,
    )
    .expect("string techniques chart should parse");

    let notes = chart.string_notes();
    assert_eq!(notes[0].attack, Some(NoteAttack::Tap));
    assert_eq!(notes[0].transition, Some(NoteTransition::Slide));
    assert_eq!(
        notes[0].bend,
        Some(BendSpec {
            semitones: 2.0,
            release: true,
            points: Vec::new(),
        })
    );
    assert_eq!(
        notes[0].motion.as_ref().map(|motion| motion.kind),
        Some(MotionKind::Trill)
    );
    assert_eq!(
        notes[0].motion.as_ref().map(|motion| motion.target),
        Some(42)
    );

    let serialized = serde_json::to_string(&chart).expect("techniques chart should serialize");
    assert!(serialized.contains("\"attack\":\"tap\""));
    assert!(serialized.contains("\"transition\":\"slide\""));
    assert!(serialized.contains("\"target\":42"));
    serde_json::from_str::<Chart>(&serialized).expect("serialized techniques chart should reparse");
}

#[test]
fn performance_hints_parse_resolve_and_round_trip() {
    let chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Performance Hints",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": [{
                "name": "Bass",
                "kind": "strings",
                "tuning": { "strings": ["E1", "A1", "D2", "G2"] },
                "notes": [{
                    "start": 0,
                    "length": 4,
                    "note": "E2",
                    "performance": {
                        "preferred_string": 1,
                        "attack": "tap",
                        "motion": { "kind": "trill", "target": "F#2" }
                    }
                }]
            }]
        }
        "#,
    )
    .expect("performance hints chart should parse");

    let NoteContent::Pitched { note, .. } = &chart.tracks[0].notes[0].content else {
        panic!("expected a pitched event");
    };
    assert_eq!(*note, 40);
    assert_eq!(
        chart.tracks[0].notes[0]
            .performance
            .as_ref()
            .and_then(|hints| hints.attack),
        Some(NoteAttack::Tap)
    );
    let resolved = chart.string_notes();
    assert_eq!(resolved[0].string, 1);
    assert_eq!(resolved[0].attack, Some(NoteAttack::Tap));
    assert_eq!(
        resolved[0].motion.as_ref().map(|motion| motion.target),
        Some(42)
    );

    let serialized = serde_json::to_string(&chart).expect("performance hints should serialize");
    assert!(serialized.contains("\"performance\""));
    serde_json::from_str::<Chart>(&serialized).expect("performance hints should reload");

    let _typed_hint = PerformanceHints {
        preferred_string: Some(1),
        attack: Some(NoteAttack::Tap),
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
    };
}

#[test]
fn instrument_notation_metadata_round_trips() {
    let chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Instrument Notation",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": [{
                "name": "Guitar",
                "kind": "strings",
                "tuning": { "strings": ["E2", "A2", "D3", "G3", "B3", "E4"] },
                "capo": 3,
                "notes": [{
                    "start": 0,
                    "length": 4,
                    "note": "E3",
                    "grace": { "kind": "acciaccatura", "slash": true },
                    "performance": {
                        "harmonic": "natural",
                        "palm_mute": true,
                        "vibrato": "wide"
                    }
                }]
            }, {
                "name": "Drums",
                "kind": "percussion",
                "kit": { "lanes": 2, "pieces": [
                    { "name": "snare", "trigger": "midi:38", "lane": 0 },
                    { "name": "hihat", "trigger": "midi:42", "lane": 1 }
                ] },
                "notes": [{
                    "start": 0,
                    "length": 8,
                    "piece": "snare",
                    "performance": {
                        "percussion_grace": "flam",
                        "percussion_technique": "buzz",
                        "sticking": "right",
                        "hi_hat": "closed"
                    }
                }]
            }]
        }
        "#,
    )
    .expect("instrument notation chart should parse");
    assert_eq!(chart.tracks[0].capo, Some(3));
    assert_eq!(
        chart.tracks[0].notes[0].grace.as_ref().unwrap().kind,
        GraceKind::Acciaccatura
    );
    let guitar_hints = chart.tracks[0].notes[0].performance.as_ref().unwrap();
    assert_eq!(guitar_hints.harmonic, Some(HarmonicKind::Natural));
    assert!(guitar_hints.palm_mute);
    assert_eq!(guitar_hints.vibrato, Some(VibratoKind::Wide));
    let drum_hints = chart.tracks[1].notes[0].performance.as_ref().unwrap();
    assert_eq!(drum_hints.percussion_grace, Some(PercussionGrace::Flam));
    assert_eq!(drum_hints.hi_hat, Some(HiHatState::Closed));

    let serialized = serde_json::to_string(&chart).expect("instrument notation should serialize");
    serde_json::from_str::<Chart>(&serialized).expect("instrument notation should reload");
}

#[test]
fn chord_validation_reports_inconsistent_groups() {
    let chart: Chart = serde_json::from_str(
        r#"{
            "version": 1,
            "title": "Bad Chords",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": [{
                "name": "Lead",
                "kind": "strings",
                "tuning": { "strings": ["E1", "A1", "D2", "G2"] },
                "notes": [
                    { "start": 0, "length": 4, "chord": "c1", "note": "E2" },
                    { "start": 480, "length": 4, "chord": "c1", "note": "G2" }
                ]
            }]
        }"#,
    )
    .expect("chord chart should parse");
    assert!(
        chart
            .validate()
            .iter()
            .any(|warning| warning.contains("chord \"c1\" has inconsistent"))
    );
}

#[test]
fn notation_features_parse_and_layout() {
    let chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Notation Features",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": [{
                "name": "Lead",
                "kind": "strings",
                "tuning": { "strings": ["E1", "A1", "D2", "G2"] },
                "clef": "bass",
                "key_signature": { "fifths": -2, "mode": "major" },
                "notes": [
                    {
                        "start": 0,
                        "length": 8,
                        "tuplet": { "actual": 3, "normal": 2 },
                        "chord": "c1",
                        "note": "E2",
                        "dynamic": "mf",
                        "articulations": ["accent"]
                    },
                    {
                        "start": 320,
                        "length": 8,
                        "tuplet": { "actual": 3, "normal": 2 },
                        "chord": "c1",
                        "note": "G2"
                    }
                ],
                "phrases": [{
                    "start": 0,
                    "duration_ticks": 960,
                    "text": "hel",
                    "syllable": "begin",
                    "melisma": true,
                    "notes": []
                }]
            }]
        }
        "#,
    )
    .expect("notation features should parse");

    let track = &chart.tracks[0];
    assert_eq!(track.clef, Some(Clef::Bass));
    assert_eq!(
        track.key_signature,
        Some(KeySignature {
            fifths: -2,
            mode: KeyMode::Major
        })
    );
    assert_eq!(track.notes[0].duration_ticks(960), 320);
    assert_eq!(track.notes[0].chord.as_deref(), Some("c1"));
    assert_eq!(track.notes[0].dynamic, Some(DynamicLevel::Mf));
    assert_eq!(track.notes[0].articulations, vec![Articulation::Accent]);
    assert_eq!(track.phrases[0].syllable, Some(SyllableKind::Begin));
    assert!(track.phrases[0].melisma);

    let layout = layout_track(&chart, track);
    assert_eq!(layout.clef, Some(Clef::Bass));
    assert_eq!(layout.key_signature, track.key_signature);
    assert_eq!(layout.tuplets, vec![vec![0, 1]]);
}

#[test]
fn explicit_rest_events_parse_and_round_trip() {
    let event: ChartEvent = serde_json::from_str(
        r#"{
            "id": "rest-1",
            "start": 960,
            "length": 4,
            "voice": 2,
            "staff": 1,
            "dots": 1,
            "rest": true
        }"#,
    )
    .expect("explicit rest should parse");
    assert!(matches!(event.content, NoteContent::Rest));
    assert_eq!(event.duration_ticks(960), 1440);
    assert_eq!(event.voice, 2);

    let serialized = serde_json::to_string(&event).expect("rest should serialize");
    assert!(serialized.contains("\"rest\":true"));
    let reloaded: ChartEvent = serde_json::from_str(&serialized).expect("rest should reload");
    assert!(matches!(reloaded.content, NoteContent::Rest));

    let chart: Chart = serde_json::from_str(
        r#"{
            "version": 1,
            "title": "Rest Layout",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": [{
                "name": "Lead",
                "kind": "strings",
                "tuning": { "strings": ["E1", "A1", "D2", "G2"] },
                "notes": [{ "start": 960, "length": 4, "rest": true }]
            }]
        }"#,
    )
    .expect("rest layout chart should parse");
    let layout = layout_track(&chart, &chart.tracks[0]);
    assert!(layout.rests.iter().any(|rest| rest.start == 960));

    for invalid in [
        r#"{ "start": 0, "length": 4, "rest": true, "note": "C4" }"#,
        r#"{ "start": 0, "length": 4, "rest": true, "piece": "snare" }"#,
    ] {
        assert!(serde_json::from_str::<ChartEvent>(invalid).is_err());
    }
}

#[test]
fn score_navigation_markers_round_trip_and_validate() {
    let mut chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Navigation",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "structure": [
                { "type": "repeat_start", "tick": 0 },
                { "type": "repeat_end", "tick": 3840, "times": 2 },
                { "type": "ending_start", "tick": 3840, "number": 1 },
                { "type": "ending_end", "tick": 4800, "number": 1 },
                { "type": "segno", "tick": 0 },
                { "type": "coda", "tick": 4800 },
                { "type": "fine", "tick": 5760 },
                { "type": "da_capo", "tick": 5760 },
                { "type": "dal_segno", "tick": 5760 },
                { "type": "rehearsal", "tick": 0, "label": "A" }
            ],
            "tracks": [{
                "name": "Bass",
                "kind": "strings",
                "tuning": { "strings": ["E1", "A1", "D2", "G2"] },
                "notes": [
                    { "start": 0, "length": 4, "note": "E1" },
                    { "start": 5760, "length": 4, "note": "E1" }
                ]
            }]
        }
        "#,
    )
    .expect("navigation chart should parse");
    assert!(matches!(
        chart.structure[1],
        ScoreMarker::RepeatEnd { times: 2, .. }
    ));
    assert!(chart.validate().is_empty());

    let serialized = serde_json::to_string(&chart).expect("navigation chart should serialize");
    chart
        .structure
        .push(ScoreMarker::RepeatEnd { tick: 0, times: 1 });
    assert!(
        chart
            .validate()
            .iter()
            .any(|warning| warning.contains("repeat end must have times"))
    );
    serde_json::from_str::<Chart>(&serialized).expect("navigation chart should reload");
}

#[test]
fn tempo_text_and_score_expressions_round_trip_and_validate() {
    let mut chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Expressions",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tempo_text": [{ "tick": 0, "text": "Andante", "bpm": 80.0 }],
            "expressions": [
                { "type": "accelerando", "start": 0, "duration_ticks": 1920, "from_bpm": 80.0, "to_bpm": 120.0 },
                { "type": "crescendo", "start": 0, "duration_ticks": 1920 }
            ],
            "tracks": [{
                "name": "Bass",
                "kind": "strings",
                "tuning": { "strings": ["E1", "A1", "D2", "G2"] },
                "notes": [{ "start": 0, "length": 4, "note": "E1" }, { "start": 1920, "length": 4, "note": "E1" }]
            }]
        }
        "#,
    )
    .expect("expression chart should parse");
    assert_eq!(chart.tempo_text[0].text, "Andante");
    assert!(matches!(
        chart.expressions[0],
        ScoreExpression::Accelerando { .. }
    ));
    assert!(chart.validate().is_empty());

    let serialized = serde_json::to_string(&chart).expect("expression chart should serialize");
    let reloaded: Chart =
        serde_json::from_str(&serialized).expect("expression chart should reload");
    assert_eq!(
        reloaded.tempo_text[0],
        TempoText {
            tick: 0,
            text: "Andante".into(),
            bpm: Some(80.0)
        }
    );

    chart.tempo_text.push(TempoText {
        tick: 0,
        text: String::new(),
        bpm: None,
    });
    chart.expressions.push(ScoreExpression::Ritardando {
        start: 0,
        duration_ticks: 0,
        from_bpm: 120.0,
        to_bpm: 90.0,
    });
    let warnings = chart.validate();
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("empty label"))
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("invalid range"))
    );
}

#[test]
fn written_pitch_spelling_survives_chart_round_trip() {
    let chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Enharmonic Spelling",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": [{
                "name": "Lead",
                "kind": "strings",
                "tuning": { "strings": ["E1", "A1", "D2", "G2"] },
                "notes": [
                    { "start": 0, "length": 4, "voice": 2, "staff": 1, "note": "C#4" },
                    { "start": 960, "length": 4, "note": "Db4" },
                    { "start": 1920, "length": 4, "note": 61 }
                ]
            }]
        }
        "#,
    )
    .expect("spelled chart should parse");

    let notes = &chart.tracks[0].notes;
    let NoteContent::Pitched {
        spelling: sharp,
        note: sharp_midi,
        ..
    } = &notes[0].content
    else {
        panic!("expected a pitched event");
    };
    let NoteContent::Pitched {
        spelling: flat,
        note: flat_midi,
        ..
    } = &notes[1].content
    else {
        panic!("expected a pitched event");
    };
    assert_eq!(*sharp_midi, 61);
    assert_eq!(*flat_midi, 61);
    assert_eq!(sharp.as_ref().map(|pitch| pitch.step), Some('C'));
    assert_eq!(sharp.as_ref().map(|pitch| pitch.alter), Some(1));
    assert_eq!(flat.as_ref().map(|pitch| pitch.step), Some('D'));
    assert_eq!(flat.as_ref().map(|pitch| pitch.alter), Some(-1));
    assert_eq!(notes[0].voice, 2);
    assert_eq!(notes[0].staff, 1);
    assert_eq!(notes[2].note(), Some(61));
    assert!(matches!(
        &notes[2].content,
        NoteContent::Pitched { spelling: None, .. }
    ));

    let serialized = serde_json::to_string(&chart).expect("spelled chart should serialize");
    assert!(serialized.contains("\"spelling\":{"));
    let reloaded: Chart = serde_json::from_str(&serialized).expect("spelled chart should reload");
    let NoteContent::Pitched { spelling, .. } = &reloaded.tracks[0].notes[1].content else {
        panic!("expected a pitched event");
    };
    assert_eq!(
        spelling,
        &Some(PitchSpelling {
            step: 'D',
            alter: -1,
            octave: 4,
        })
    );
}

#[test]
fn score_event_ids_and_relationships_round_trip_and_validate() {
    let mut chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Relationships",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": [{
                "name": "Lead",
                "kind": "strings",
                "tuning": { "strings": ["E1", "A1", "D2", "G2"] },
                "notes": [
                    { "id": "n1", "start": 0, "length": 4, "note": "E2" },
                    { "id": "n2", "start": 960, "length": 4, "note": "E2" }
                ],
                "ties": [{ "from": "n1", "to": "n2" }],
                "slurs": [{ "from": "n1", "to": "n2" }]
            }]
        }
        "#,
    )
    .expect("relationship chart should parse");

    assert_eq!(chart.tracks[0].notes[0].id.as_deref(), Some("n1"));
    assert_eq!(
        chart.tracks[0].ties[0],
        EventRelation {
            from: "n1".into(),
            to: "n2".into()
        }
    );
    assert!(chart.validate().is_empty());

    let serialized = serde_json::to_string(&chart).expect("relationship chart should serialize");
    let reloaded: Chart =
        serde_json::from_str(&serialized).expect("relationship chart should reload");
    assert_eq!(reloaded.tracks[0].slurs.len(), 1);

    chart.tracks[0].ties.push(EventRelation {
        from: "n2".into(),
        to: "missing".into(),
    });
    assert!(
        chart
            .validate()
            .iter()
            .any(|warning| warning.contains("tie references an unknown event ID"))
    );
}

#[test]
fn chart_validation_reports_invalid_ties_and_bend_curves() {
    let chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Invalid Notation",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": [{
                "name": "Bass",
                "kind": "strings",
                "tuning": { "strings": ["E1", "A1", "D2", "G2"] },
                "notes": [
                    { "start": 0, "length": 4, "tied": true, "note": "E2", "performance": { "bend": {
                        "points": [
                            { "offset": 0.75, "semitones": 2.0 },
                            { "offset": 0.25, "semitones": 0.0 }
                        ]
                    } } },
                    { "start": 1920, "length": 4, "note": "F2" }
                ]
            }]
        }
        "#,
    )
    .expect("invalid notation should still parse");
    let warnings = chart.validate();
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("tie does not start"))
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("tie changes pitch"))
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("invalid bend curve"))
    );
}

#[test]
/// Loads the built-in chart with ordered, playable bass notes.
fn built_in_chart_has_playable_notes() {
    const OPEN_STRINGS_CHART: &str = include_str!("../../../charts/open-strings.json");
    let chart: Chart =
        serde_json::from_str(OPEN_STRINGS_CHART).expect("built-in chart should parse");
    assert_eq!(chart.version, 1);
    assert_eq!(chart.tracks.len(), 1);
    assert_eq!(chart.tracks[0].kind, InstrumentKind::Strings);
    assert_eq!(
        chart.tracks[0].tuning.as_ref().map(|tuning| tuning
            .strings
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()),
        Some(vec!["B0", "E1", "A1", "D2", "G2"])
    );
    let notes = chart.string_notes();
    assert_eq!(notes.len(), 10);
    assert_eq!(notes[0].note, "B0");
    assert_eq!(notes[5].note, "D1");
    assert!(
        notes
            .windows(2)
            .all(|notes| notes[0].start < notes[1].start)
    );
}
#[test]
fn chart_directory_loads_all_valid_charts() {
    let charts = load_charts();
    let chart_files = std::fs::read_dir("charts")
        .expect("charts directory should exist")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .count();
    assert_eq!(
        charts.len(),
        chart_files,
        "every JSON chart in the charts directory should load"
    );
    assert!(!charts.is_empty(), "expected at least one chart");
}

#[test]
/// The drum-only test chart hits every standard rock kit piece at least twice.
fn drum_kit_workout_chart_covers_every_piece_at_least_twice() {
    let charts = load_charts();
    let Some(chart) = charts
        .iter()
        .find(|chart| chart.title == "Drum Kit Workout")
    else {
        return;
    };
    let notes = chart.percussion_notes();
    for piece in [
        "kick", "snare", "tom1", "hihat", "tom2", "ride", "tom3", "crash",
    ] {
        let count = notes.iter().filter(|note| note.piece == piece).count();
        assert!(
            count >= 2,
            "expected {piece} to appear at least twice, got {count}"
        );
    }
}

#[test]
/// The three-track jam chart is roughly 30 seconds and has a part for each instrument.
fn easy_band_jam_chart_has_three_tracks_and_is_about_thirty_seconds() {
    let charts = load_charts();
    let chart = charts
        .iter()
        .find(|chart| chart.title == "Easy Band Jam")
        .expect("easy band jam chart should load");
    assert_eq!(chart.tracks.len(), 3);
    assert!(chart.tracks.iter().any(|track| track.name == "Bass"));
    assert!(chart.tracks.iter().any(|track| track.name == "Guitar"));
    assert!(chart.tracks.iter().any(|track| track.name == "Drums"));
    let seconds = chart.tick_to_seconds(chart.total_ticks());
    assert!(
        (25.0..=35.0).contains(&seconds),
        "expected roughly 30 seconds, got {seconds}"
    );
}

#[test]
fn chart_supports_multiple_instrument_tracks_and_vocal_phrases() {
    let chart: Chart = serde_json::from_str(
        r#"
                {
                    "version": 1,
                    "title": "Band Test",
                    "resolution": 960,
                    "tempo_map": [{ "start": 0, "bpm": 120.0 }],
                    "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
                    "tracks": [
                        {
                            "name": "Bass",
                            "kind": "strings",
                            "tuning": { "strings": ["B0", "E1", "A1", "D2", "G2"] },
                            "notes": [{ "start": 960, "length": 4, "note": 23 }]
                        },
                        {
                            "name": "Lead Vocal",
                            "kind": "voice",
                            "phrases": [{
                                "start": 1920,
                                "duration_ticks": 1920,
                                "text": "Hello",
                                "notes": [{ "start": 1920, "length": 2, "note": "C4" }]
                            }]
                        }
                    ]
                }
                "#,
    )
    .expect("multi-track chart should parse");

    assert_eq!(chart.starting_time_signature(), [4, 4]);
    assert_eq!(chart.tracks.len(), 2);
    assert_eq!(chart.tracks[1].phrases[0].text, "Hello");
    assert_eq!(chart.tracks[1].phrases[0].notes[0].note(), Some(60));
    assert_eq!(chart.total_ticks(), 3840);
    assert_eq!(chart.string_notes()[0].note, "B0");
}

#[test]
/// Resolves shared-lane percussion pieces (tom vs. cymbal) and a lane-spanning kick by name.
fn percussion_track_resolves_pieces_by_name() {
    let chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Drums Test",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": [{
                "name": "Drums",
                "kind": "percussion",
                "kit": {
                    "lanes": 4,
                    "pieces": [
                        { "name": "kick", "trigger": "midi:36", "lane_span": "yellow" },
                        { "name": "snare", "trigger": "midi:38", "lane": 0, "symbol": "tom" },
                        { "name": "crash", "trigger": "midi:49", "lane": 1, "symbol": "cymbal" }
                    ]
                },
                "notes": [
                    { "start": 0, "length": 4, "piece": "kick" },
                    { "start": 960, "length": 4, "piece": "snare", "performance": { "percussion_dynamics": "accent" } },
                    { "start": 1920, "length": 4, "piece": "crash", "performance": { "roll": "crash" } },
                    { "start": 2880, "length": 4, "piece": "snare", "performance": { "droll": "crash" } }
                ]
            }]
        }
        "#,
    )
    .expect("percussion chart should parse");

    let notes = chart.percussion_notes();
    assert_eq!(notes.len(), 4);
    assert_eq!(notes[0].piece, "kick");
    assert_eq!(notes[0].lane_span_color.as_deref(), Some("yellow"));
    assert_eq!(notes[1].dynamics, NoteDynamics::Accent);
    assert_eq!(notes[1].symbol.as_deref(), Some("tom"));
    assert_eq!(notes[2].symbol.as_deref(), Some("cymbal"));
    assert_eq!(notes[2].roll.as_deref(), Some("crash"));
    assert_eq!(notes[2].droll, None);
    assert_eq!(notes[3].roll, None);
    assert_eq!(notes[3].droll.as_deref(), Some("crash"));
}

#[test]
fn chart_rejects_conflicting_or_pitched_roll_fields() {
    for json in [
        r#"{ "start": 0, "length": 4, "piece": "kick", "performance": { "roll": "snare", "droll": "tom1" } }"#,
        r#"{ "start": 0, "length": 4, "note": "E2", "performance": { "roll": "snare" } }"#,
    ] {
        assert!(
            serde_json::from_str::<ChartEvent>(json).is_err(),
            "expected rejection: {json}"
        );
    }
}

#[test]
/// Walks a mid-song tempo change when converting ticks to seconds.
fn tick_to_seconds_honors_a_mid_song_tempo_change() {
    let chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Tempo Change",
            "resolution": 960,
            "tempo_map": [
                { "start": 0, "bpm": 120.0 },
                { "start": 1920, "bpm": 60.0 }
            ],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": []
        }
        "#,
    )
    .expect("tempo-change chart should parse");

    // Two quarter notes at 120 BPM (0.5s each) land exactly on the tempo change.
    assert!((chart.tick_to_seconds(1920) - 1.0).abs() < 1e-4);
    // One further quarter note at 60 BPM adds a full second.
    assert!((chart.tick_to_seconds(2880) - 2.0).abs() < 1e-4);
}

#[test]
/// Star Power phrases are simple range markers with no nested note list.
fn star_power_phrase_parses_as_a_range_marker() {
    let chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Star Power",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": [{
                "name": "Bass",
                "kind": "strings",
                "tuning": { "strings": ["E1", "A1", "D2", "G2"] },
                "notes": [],
                "star_power_phrases": [{ "start": 0, "duration_ticks": 3840 }]
            }]
        }
        "#,
    )
    .expect("star power chart should parse");

    assert_eq!(chart.tracks[0].star_power_phrases.len(), 1);
    assert_eq!(chart.tracks[0].star_power_phrases[0].duration_ticks, 3840);
}

#[test]
/// Chart validation flags unplayable notes, missing tuning/kit, and unknown pieces.
fn chart_validate_reports_expected_problems() {
    let chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Broken Chart",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": [
                {
                    "name": "Bass",
                    "kind": "strings",
                    "tuning": { "strings": ["E1", "A1", "D2", "G2"] },
                    "notes": [{ "start": 0, "length": 4, "note": 100 }]
                },
                {
                    "name": "Drums",
                    "kind": "percussion",
                    "kit": { "lanes": 1, "pieces": [{ "name": "kick", "trigger": "midi:36", "lane": 0 }] },
                    "notes": [{ "start": 0, "length": 4, "piece": "kick", "performance": { "droll": "nonexistent" } }]
                }
            ]
        }
        "#,
    )
    .expect("broken chart should still parse");

    let warnings = chart.validate();
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("unplayable note"))
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("unknown performance roll target"))
    );
}

fn simple_chart(resolution: u32) -> Chart {
    serde_json::from_str(&format!(
        r#"{{
            "version": 1,
            "title": "Notation Test",
            "resolution": {resolution},
            "tempo_map": [{{ "start": 0, "bpm": 120.0 }}],
            "time_signature_map": [{{ "start": 0, "numerator": 4, "denominator": 4 }}],
            "tracks": []
        }}"#
    ))
    .expect("simple chart should parse")
}

#[test]
/// Builds sequential 4/4 measures covering the requested tick span.
fn measures_covers_the_requested_span_in_4_4() {
    let chart = simple_chart(960);
    let built = measures(&chart, 7680);
    assert_eq!(built.len(), 2);
    assert_eq!(built[0].start_tick, 0);
    assert_eq!(built[0].end_tick, 3840);
    assert_eq!(built[1].start_tick, 3840);
    assert_eq!(built[1].end_tick, 7680);
}

#[test]
/// A tied pair of quarter notes forms one tie chain, and the rest of the measure is
/// filled with inferred rests that don't cross the barline.
fn layout_track_ties_notes_and_infers_rests() {
    let mut chart = simple_chart(960);
    chart.tracks = vec![new_track(InstrumentKind::Strings)];
    chart.tracks[0].notes = serde_json::from_str(
        r#"[
            { "start": 0, "length": 4, "note": "E1", "tied": true },
            { "start": 960, "length": 4, "note": "E1" }
        ]"#,
    )
    .expect("notes should parse");

    let layout = layout_track(&chart, &chart.tracks[0]);
    assert_eq!(layout.ties, vec![vec![0, 1]]);
    // The tie chain occupies ticks 0..1920; the rest of the 3840-tick measure is silent.
    let rest_ticks = layout
        .rests
        .iter()
        .map(|rest| rest.value.ticks(960))
        .sum::<u32>();
    assert_eq!(rest_ticks, 3840 - 1920);
    assert!(layout.rests.iter().all(|rest| rest.start >= 1920));
}

#[test]
/// Consecutive eighth notes within the same beat are grouped into one beam.
fn layout_track_beams_eighth_notes_within_a_beat() {
    let mut chart = simple_chart(960);
    chart.tracks = vec![new_track(InstrumentKind::Strings)];
    chart.tracks[0].notes = serde_json::from_str(
        r#"[
            { "start": 0, "length": 8, "note": "E1" },
            { "start": 480, "length": 8, "note": "E1" },
            { "start": 960, "length": 8, "note": "E1" },
            { "start": 1440, "length": 8, "note": "E1" }
        ]"#,
    )
    .expect("notes should parse");

    let layout = layout_track(&chart, &chart.tracks[0]);
    assert_eq!(layout.beams, vec![vec![0, 1], vec![2, 3]]);
}

#[test]
/// Bass tunings default to bass clef, guitar tunings (even 7-string) to treble, and
/// percussion tracks have no clef at all.
fn default_clef_distinguishes_bass_from_guitar_and_percussion() {
    let mut bass = new_track(InstrumentKind::Strings);
    bass.tuning = Some(Tuning {
        strings: vec!["E1".into(), "A1".into(), "D2".into(), "G2".into()],
    });
    assert_eq!(default_clef(&bass), Some(Clef::Bass));

    let mut guitar = new_track(InstrumentKind::Strings);
    guitar.tuning = Some(Tuning {
        strings: vec![
            "B1".into(),
            "E2".into(),
            "A2".into(),
            "D3".into(),
            "G3".into(),
            "B3".into(),
            "E4".into(),
        ],
    });
    assert_eq!(default_clef(&guitar), Some(Clef::Treble));

    let percussion = new_track(InstrumentKind::Percussion);
    assert_eq!(default_clef(&percussion), None);
}

#[test]
/// A chart edited in memory round-trips through serialize/deserialize without losing
/// pitched notes, percussive notes, dynamics, or ties.
fn chart_round_trips_through_serialize_and_deserialize() {
    let mut chart: Chart = serde_json::from_str(
        r#"
        {
            "version": 1,
            "title": "Round Trip",
            "resolution": 960,
            "tempo_map": [{ "start": 0, "bpm": 120.0 }],
            "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
            "tracks": []
        }
        "#,
    )
    .expect("empty chart should parse");
    chart.tracks.push(new_track(InstrumentKind::Strings));
    chart.tracks.push(new_track(InstrumentKind::Percussion));
    chart.tracks[0].notes.push(
        serde_json::from_str(
            r#"{ "start": 0, "length": 4, "dots": 1, "tied": true, "note": "E1", "performance": { "preferred_string": 0 } }"#,
        )
        .unwrap(),
    );
    chart.tracks[1].notes.push(
        serde_json::from_str(r#"{ "start": 0, "length": 8, "piece": "kick", "performance": { "percussion_dynamics": "accent", "droll": "snare" } }"#)
            .unwrap(),
    );

    let serialized = serde_json::to_string(&chart).expect("chart should serialize");
    let reloaded: Chart =
        serde_json::from_str(&serialized).expect("serialized chart should reparse");

    assert_eq!(reloaded.title, "Round Trip");
    assert_eq!(reloaded.tracks.len(), 2);
    assert_eq!(reloaded.tracks[0].notes[0].note(), Some(28));
    assert_eq!(reloaded.tracks[0].notes[0].dots, 1);
    assert!(reloaded.tracks[0].notes[0].tied);
    assert_eq!(reloaded.tracks[1].kind, InstrumentKind::Percussion);
    assert_eq!(
        reloaded.tracks[1].notes[0]
            .performance
            .as_ref()
            .and_then(|hints| hints.droll.as_deref()),
        Some("snare")
    );
}

#[test]
/// Accepts a detected pitch within the chart matcher tolerance.
fn chart_pitch_matching_uses_cents() {
    assert!(cents_error(56.0, 55.0).abs() < 90.0);
    assert!(cents_error(65.4, 55.0).abs() > 90.0);
}
