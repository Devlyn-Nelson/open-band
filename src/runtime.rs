use super::Instrument;
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

#[derive(Resource)]
/// Available devices and current choices in Input Setup.
pub(crate) struct DeviceSelection {
    pub(crate) audio_devices: Vec<DeviceChoice>,
    pub(crate) midi_devices: Vec<DeviceChoice>,
    pub(crate) selected: [usize; 4],
    pub(crate) focus: usize,
    pub(crate) bass_strings: u8,
}

#[derive(Clone, Debug)]
pub(crate) struct DeviceChoice {
    pub(crate) id: String,
    pub(crate) label: String,
}

#[derive(Resource, Serialize, Deserialize, Default, Debug)]
/// Settings persisted between launches in `open-band-settings/settings.json`.
pub(crate) struct PersistentSettings {
    pub(crate) guitar_device: Option<String>,
    pub(crate) bass_device: Option<String>,
    pub(crate) midi_device: Option<String>,
    pub(crate) vocal_device: Option<String>,
    pub(crate) bass_strings: Option<u8>,
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
    pub(crate) selected: Instrument,
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
    pub(crate) audio_devices: [Option<String>; 3],
    pub(crate) midi_device: Option<String>,
    pub(crate) bass_strings: u8,
}
