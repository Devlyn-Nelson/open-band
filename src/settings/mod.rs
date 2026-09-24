use crate::*;
use serde::{Deserialize, Serialize};

pub(crate) const SETTINGS_DIRECTORY: &str = "open-band-settings";
pub(crate) const SETTINGS_FILE: &str = "open-band-settings/settings.json";

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
    /// The configured tuning; only meaningful when `kind` is `Strings`.
    #[serde(default)]
    pub(crate) tuning: Option<Tuning>,
    /// The configured kit; only meaningful when `kind` is `Percussion`.
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

    pub(crate) fn kit_or_default(&self) -> Kit {
        self.kit.clone().unwrap_or_default()
    }
}

pub(crate) fn initial_app_state() -> AppState {
    if std::path::Path::new(SETTINGS_FILE).exists() {
        AppState::Home
    } else {
        AppState::DeviceSelection
    }
}

/// The starter slot list shown the first time Input Setup runs, matching the previous
/// fixed guitar/bass/drums/vocals lineup so existing users see a familiar default. Device
/// choices are preselected from `BAND_HERO_*_DEVICE` environment variables when available,
/// matching the previous first-run behavior.
fn default_slots(
    audio_devices: &[DeviceChoice],
    midi_devices: &[DeviceChoice],
) -> Vec<InstrumentSlot> {
    let audio_device = |variable: &str| {
        let index = selected_device_index(audio_devices, None, variable);
        audio_devices.get(index).map(|device| device.id.clone())
    };
    let midi_device = |variable: &str| {
        let index = selected_device_index(midi_devices, None, variable);
        midi_devices.get(index).map(|device| device.id.clone())
    };
    let bass_tuning = if std::env::var("BAND_HERO_BASS_STRINGS").as_deref() == Ok("5") {
        Tuning {
            strings: vec![
                "B0".into(),
                "E1".into(),
                "A1".into(),
                "D2".into(),
                "G2".into(),
            ],
        }
    } else {
        Tuning::default()
    };
    let guitar_tuning = Tuning {
        strings: vec![
            "E2".into(),
            "A2".into(),
            "D3".into(),
            "G3".into(),
            "B3".into(),
            "E4".into(),
        ],
    };
    vec![
        InstrumentSlot {
            kind: InstrumentKind::Strings,
            device: audio_device("BAND_HERO_GUITAR_DEVICE"),
            tuning: Some(guitar_tuning),
            kit: None,
            detector: DetectorProfile::Polyphonic,
        },
        InstrumentSlot {
            kind: InstrumentKind::Strings,
            device: audio_device("BAND_HERO_BASS_DEVICE"),
            tuning: Some(bass_tuning),
            kit: None,
            detector: DetectorProfile::PerString,
        },
        InstrumentSlot {
            device: midi_device("BAND_HERO_MIDI_DEVICE"),
            ..InstrumentSlot::default_for(InstrumentKind::Percussion)
        },
        InstrumentSlot {
            device: audio_device("BAND_HERO_VOCAL_DEVICE"),
            ..InstrumentSlot::default_for(InstrumentKind::Voice)
        },
    ]
}

pub(crate) fn input_config_from_settings(settings: &PersistentSettings) -> InputConfig {
    let slots = if settings.slots.is_empty() {
        default_slots(&[], &[])
    } else {
        settings.slots.clone()
    };
    InputConfig { slots }
}

pub(crate) fn scan_devices(settings: &PersistentSettings) -> DeviceSelection {
    let host = cpal::default_host();
    let audio_devices = host
        .input_devices()
        .map(|devices| {
            devices
                .filter_map(|device| {
                    let id = device.id().ok()?.to_string();
                    Some(DeviceChoice {
                        label: device.to_string(),
                        id,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let midi_devices = MidiInput::new("open-band-device-scan")
        .map(|input| {
            input
                .ports()
                .iter()
                .enumerate()
                .map(|(index, port)| {
                    let label = input
                        .port_name(port)
                        .unwrap_or_else(|_| "Unknown MIDI device".into());
                    DeviceChoice {
                        id: format!("midi:{index}"),
                        label,
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let slots = if settings.slots.is_empty() {
        default_slots(&audio_devices, &midi_devices)
    } else {
        settings.slots.clone()
    };

    DeviceSelection {
        audio_devices,
        midi_devices,
        tuning_library: load_tuning_library(),
        kit_library: load_kit_library(),
        slots,
        focus: 0,
    }
}

pub(crate) fn load_settings() -> PersistentSettings {
    std::fs::read_to_string(SETTINGS_FILE)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_default()
}

pub(crate) fn save_settings(settings: &PersistentSettings) {
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        std::fs::create_dir_all(SETTINGS_DIRECTORY)?;
        let contents = serde_json::to_string_pretty(settings)?;
        std::fs::write(SETTINGS_FILE, contents)?;
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("Could not save settings to {SETTINGS_FILE}: {error}");
    }
}

pub(crate) fn selected_device_index(
    devices: &[DeviceChoice],
    saved: Option<&str>,
    variable: &str,
) -> usize {
    saved
        .map(str::to_owned)
        .or_else(|| std::env::var(variable).ok())
        .and_then(|wanted| {
            devices.iter().position(|device| {
                device.id == wanted || device.label == wanted || device.label.contains(&wanted)
            })
        })
        .unwrap_or(0)
}
