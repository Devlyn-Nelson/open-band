use super::{Instrument, InstrumentKind};
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
    /// String count; only meaningful when `kind` is `Strings`.
    #[serde(default)]
    pub(crate) strings: u8,
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
                strings: 4,
                detector: DetectorProfile::PerString,
            },
            InstrumentKind::Percussion | InstrumentKind::Voice => Self {
                kind,
                device: None,
                strings: 0,
                detector: DetectorProfile::PerString,
            },
        }
    }

    pub(crate) fn label(&self) -> String {
        match self.kind {
            InstrumentKind::Strings => format!("STRINGS ({}-string)", self.strings),
            InstrumentKind::Percussion => "PERCUSSION (MIDI)".into(),
            InstrumentKind::Voice => "VOICE".into(),
        }
    }
}

#[derive(Resource)]
/// Available devices and current choices in Input Setup.
pub(crate) struct DeviceSelection {
    pub(crate) audio_devices: Vec<DeviceChoice>,
    pub(crate) midi_devices: Vec<DeviceChoice>,
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

