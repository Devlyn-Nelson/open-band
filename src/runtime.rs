use super::{Instrument, InstrumentKind, Kit, NamedKit, NamedTuning, Tuning};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub(crate) const LANES: usize = 5;
pub(crate) const HIT_LINE_Y: f32 = -250.0;
pub(crate) const NOTE_SPEED: f32 = 260.0;
pub(crate) const SETTINGS_DIRECTORY: &str = "open-band-settings";
pub(crate) const SETTINGS_FILE: &str = "open-band-settings/settings.json";
pub(crate) const RECORDING_ENVIRONMENT_VARIABLE: &str = "BAND_HERO_RECORDING";

#[derive(Resource, Default)]
/// Selection state for the Home and Set Up menus.
pub(crate) struct MenuSelection {
    pub(crate) home_selected: usize,
    pub(crate) setup_selected: usize,
}

#[derive(Component)]
/// Text node used by the Home and Set Up menus.
pub(crate) struct MenuText;

#[derive(Component)]
/// Camera owned by a menu screen.
pub(crate) struct MenuCamera;

/// Which detector algorithm a `Strings` slot uses; irrelevant for other kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DetectorProfile {
    /// Multiple simultaneous pitches (chords), e.g. a guitar.
    Polyphonic,
    /// One physical string at a time, matched to the nearest open-string lane, e.g. a bass.
    PerString,
}

/// One configured physical instrument input. Any number of slots of any kind can exist.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct InstrumentSlot {
    pub(crate) kind: InstrumentKind,
    /// Stable CPAL device id (`Strings`/`Voice`) or `midi:<index>` port key (`Percussion`).
    #[serde(default)]
    pub(crate) device: Option<String>,
    /// The configured tuning; only meaningful when `kind` is `Strings`. Resolved and
    /// copied from the `tunings/` library on selection, so this never depends on the
    /// library still existing on disk once saved.
    #[serde(default)]
    pub(crate) tuning: Option<Tuning>,
    /// The configured kit; only meaningful when `kind` is `Percussion`. Resolved and
    /// copied from the `kits/` library on selection, so this never depends on the library
    /// still existing on disk once saved.
    #[serde(default)]
    pub(crate) kit: Option<Kit>,
    /// Only meaningful when `kind` is `Strings`.
    #[serde(default = "default_detector_profile")]
    pub(crate) detector: DetectorProfile,
}

fn default_detector_profile() -> DetectorProfile {
    DetectorProfile::PerString
}

impl InstrumentSlot {
    pub(crate) fn default_for(kind: InstrumentKind) -> Self {
        match kind {
            InstrumentKind::Strings => Self {
                kind,
                device: None,
                tuning: Some(Tuning::default()),
                kit: None,
                detector: DetectorProfile::PerString,
            },
            InstrumentKind::Percussion => Self {
                kind,
                device: None,
                tuning: None,
                kit: Some(Kit::default()),
                detector: DetectorProfile::PerString,
            },
            InstrumentKind::Voice => Self {
                kind,
                device: None,
                tuning: None,
                kit: None,
                detector: DetectorProfile::PerString,
            },
        }
    }

    pub(crate) fn label(&self) -> String {
        match self.kind {
            InstrumentKind::Strings => {
                let tuning = self.tuning.clone().unwrap_or_default();
                format!("STRINGS ({})", tuning.strings.join("-"))
            }
            InstrumentKind::Percussion => {
                let kit = self.kit.clone().unwrap_or_default();
                format!("PERCUSSION (MIDI, {} pieces)", kit.pieces.len())
            }
            InstrumentKind::Voice => "VOICE".into(),
        }
    }

    /// The real open-string frequencies for this slot's tuning, falling back to a
    /// default 4-string tuning if none is configured or it fails to parse.
    pub(crate) fn open_frequencies(&self) -> Vec<f32> {
        self.tuning
            .as_ref()
            .and_then(|tuning| tuning.open_frequencies().ok())
            .unwrap_or_else(|| {
                Tuning::default()
                    .open_frequencies()
                    .expect("default tuning parses")
            })
    }

    /// This slot's configured kit, falling back to the default kit if none is configured.
    pub(crate) fn kit_or_default(&self) -> Kit {
        self.kit.clone().unwrap_or_default()
    }
}

#[derive(Resource)]
/// Available devices and current choices in Input Setup.
pub(crate) struct DeviceSelection {
    pub(crate) audio_devices: Vec<DeviceChoice>,
    pub(crate) midi_devices: Vec<DeviceChoice>,
    pub(crate) tuning_library: Vec<NamedTuning>,
    pub(crate) kit_library: Vec<NamedKit>,
    pub(crate) slots: Vec<InstrumentSlot>,
    pub(crate) focus: usize,
}

impl DeviceSelection {
    /// The device list a slot's device selection should cycle through.
    pub(crate) fn devices_for(&self, kind: InstrumentKind) -> &[DeviceChoice] {
        match kind {
            InstrumentKind::Percussion => &self.midi_devices,
            InstrumentKind::Strings | InstrumentKind::Voice => &self.audio_devices,
        }
    }

    /// The library preset name matching a slot's configured tuning/kit, if any (a slot
    /// loaded from an older settings file, or with a hand-edited tuning/kit, may not
    /// match a current library entry).
    pub(crate) fn preset_name_for(&self, slot: &InstrumentSlot) -> Option<&str> {
        match slot.kind {
            InstrumentKind::Strings => {
                let tuning = slot.tuning.as_ref()?;
                self.tuning_library
                    .iter()
                    .find(|named| named.strings == tuning.strings)
                    .map(|named| named.name.as_str())
            }
            InstrumentKind::Percussion => {
                let kit = slot.kit.as_ref()?;
                self.kit_library
                    .iter()
                    .find(|named| named.lanes == kit.lanes && named.pieces == kit.pieces)
                    .map(|named| named.name.as_str())
            }
            InstrumentKind::Voice => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DeviceChoice {
    pub(crate) id: String,
    pub(crate) label: String,
}

#[derive(Resource, Serialize, Deserialize, Default, Debug)]
/// Settings persisted between launches in `open-band-settings/settings.json`.
pub(crate) struct PersistentSettings {
    #[serde(default)]
    pub(crate) slots: Vec<InstrumentSlot>,
    pub(crate) latency_ms: Option<f32>,
}

#[derive(Component)]
pub(crate) struct DeviceSelectionText;

#[derive(Component)]
pub(crate) struct DeviceSelectionCamera;

#[derive(Resource, Default)]
/// Accumulated gameplay score.
pub(crate) struct Score {
    pub(crate) hits: u32,
    pub(crate) combo: u32,
    pub(crate) accuracy: f32,
}

#[derive(Resource)]
/// Signal and tuner state displayed during instrument calibration.
pub(crate) struct Calibration {
    /// Index into the configured `InstrumentSlot` list.
    pub(crate) selected: usize,
    pub(crate) level: f32,
    pub(crate) peak: f32,
    pub(crate) samples: u32,
    pub(crate) last_pitch_hz: Option<f32>,
}

#[derive(Component)]
pub(crate) struct CalibrationText;

#[derive(Component)]
pub(crate) struct CalibrationMeter;

#[derive(Component)]
pub(crate) struct CalibrationCamera;

#[derive(Resource)]
/// Timing measurements collected by the latency calibration screen.
pub(crate) struct LatencyCalibration {
    pub(crate) started_at: f32,
    pub(crate) best_ms: Option<f32>,
    pub(crate) attempts: u32,
}

#[derive(Component)]
pub(crate) struct LatencyText;

#[derive(Component)]
pub(crate) struct LatencyPulse;

#[derive(Component)]
pub(crate) struct LatencyEntity;

#[derive(Component)]
pub(crate) struct LatencyCamera;

#[derive(Component)]
/// A falling gameplay note and the lane it belongs to.
pub(crate) struct FallingNote {
    pub(crate) lane: usize,
    pub(crate) instrument: Instrument,
    pub(crate) spawned_at: f32,
    pub(crate) duration_secs: f32,
}

#[derive(Component)]
/// Marker shared by all entities owned by the live session.
pub(crate) struct GameplayEntity;

#[derive(Resource, Default)]
/// Latest event values shown in the live-session debug window.
pub(crate) struct DebugInputData {
    pub(crate) instrument: Option<Instrument>,
    /// The current instrument's real configured open-string frequencies, for the debug
    /// window's per-string breakdown (empty for non-`Strings` kinds).
    pub(crate) open_frequencies: Vec<f32>,
    pub(crate) pitch_hz: Option<f32>,
    pub(crate) lane: Option<usize>,
    pub(crate) strength: f32,
    pub(crate) noise_floor: f32,
    pub(crate) duration_secs: f32,
    pub(crate) event_count: u64,
}

#[derive(Component)]
pub(crate) struct DebugWindow;

#[derive(Component)]
pub(crate) struct DebugText;

/// Runtime device configuration consumed by the input worker.
pub(crate) struct InputConfig {
    pub(crate) slots: Vec<InstrumentSlot>,
}
