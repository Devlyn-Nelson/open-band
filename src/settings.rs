use super::*;

pub(crate) fn initial_app_state() -> AppState {
    if std::path::Path::new(SETTINGS_FILE).exists() {
        AppState::Home
    } else {
        AppState::DeviceSelection
    }
}

pub(crate) fn input_config_from_settings(settings: &PersistentSettings) -> InputConfig {
    InputConfig {
        audio_devices: [
            settings.guitar_device.clone(),
            settings.bass_device.clone(),
            settings.vocal_device.clone(),
        ],
        midi_device: settings.midi_device.clone(),
        bass_strings: settings.bass_strings.unwrap_or(4),
    }
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

    DeviceSelection {
        selected: [
            selected_device_index(
                &audio_devices,
                settings.guitar_device.as_deref(),
                "BAND_HERO_GUITAR_DEVICE",
            ),
            selected_device_index(
                &audio_devices,
                settings.bass_device.as_deref(),
                "BAND_HERO_BASS_DEVICE",
            ),
            selected_device_index(
                &midi_devices,
                settings.midi_device.as_deref(),
                "BAND_HERO_MIDI_DEVICE",
            ),
            selected_device_index(
                &audio_devices,
                settings.vocal_device.as_deref(),
                "BAND_HERO_VOCAL_DEVICE",
            ),
        ],
        focus: 0,
        bass_strings: settings.bass_strings.unwrap_or_else(|| {
            if std::env::var("BAND_HERO_BASS_STRINGS").as_deref() == Ok("5") {
                5
            } else {
                4
            }
        }),
        audio_devices,
        midi_devices,
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
