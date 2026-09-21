use bevy::prelude::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use midir::{Ignore, MidiInput};
use rustfft::{FftPlanner, num_complex::Complex};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

const LANES: usize = 5;
const HIT_LINE_Y: f32 = -250.0;
const NOTE_SPEED: f32 = 260.0;
const SETTINGS_DIRECTORY: &str = "open-band-settings";
const SETTINGS_FILE: &str = "open-band-settings/settings.json";
const RECORDING_ENVIRONMENT_VARIABLE: &str = "BAND_HERO_RECORDING";

#[derive(States, Default, Clone, Eq, PartialEq, Debug, Hash)]
/// Top-level screens in the application flow.
enum AppState {
    #[default]
    Home,
    Setup,
    DeviceSelection,
    Calibration,
    LatencyCalibration,
    Gameplay,
}

struct PolyphonicAudioDetector {
    sample_rate: f32,
    processed_samples: usize,
    noise_floor: f32,
    samples: Vec<f32>,
    tracks: Vec<PolyphonicTrack>,
}

struct PolyphonicTrack {
    pitch_hz: f32,
    strength: f32,
    started_sample: usize,
    missed_windows: usize,
}

impl PolyphonicAudioDetector {
    fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            processed_samples: 0,
            noise_floor: 0.0,
            samples: Vec::new(),
            tracks: Vec::new(),
        }
    }

    fn detect(&mut self, samples: impl Iterator<Item = f32>) -> Vec<DetectedNote> {
        self.samples.extend(samples);
        if self.samples.len() < 4096 {
            return Vec::new();
        }
        let window = self.samples[..4096].to_vec();
        self.samples.drain(..2048);
        self.processed_samples += 2048;

        let level =
            (window.iter().map(|sample| sample * sample).sum::<f32>() / window.len() as f32).sqrt();
        self.noise_floor = self.noise_floor * 0.995 + level * 0.005;
        let peaks = spectral_peaks(
            &window,
            self.sample_rate,
            (self.noise_floor * 2.5).max(0.008),
        );
        let mut events = Vec::new();
        let mut matched = vec![false; self.tracks.len()];

        for (pitch_hz, strength) in peaks {
            let matching_track = self
                .tracks
                .iter()
                .enumerate()
                .filter(|(index, track)| {
                    !matched[*index] && (pitch_hz / track.pitch_hz).log2().abs() <= 90.0 / 1200.0
                })
                .min_by(|(_, left), (_, right)| {
                    (pitch_hz / left.pitch_hz)
                        .log2()
                        .abs()
                        .total_cmp(&(pitch_hz / right.pitch_hz).log2().abs())
                })
                .map(|(index, _)| index);

            if let Some(index) = matching_track {
                matched[index] = true;
                let track = &mut self.tracks[index];
                track.pitch_hz = pitch_hz;
                track.strength = strength;
                track.missed_windows = 0;
                events.push(DetectedNote {
                    pitch_hz,
                    strength,
                    noise_floor: self.noise_floor,
                    phase: NotePhase::Updated,
                    duration_secs: (self.processed_samples - track.started_sample) as f32
                        / self.sample_rate,
                });
            } else if self.tracks.len() < 6 {
                self.tracks.push(PolyphonicTrack {
                    pitch_hz,
                    strength,
                    started_sample: self.processed_samples,
                    missed_windows: 0,
                });
                matched.push(true);
                events.push(DetectedNote {
                    pitch_hz,
                    strength,
                    noise_floor: self.noise_floor,
                    phase: NotePhase::Started,
                    duration_secs: 0.0,
                });
            }
        }

        for index in (0..self.tracks.len()).rev() {
            if matched.get(index).copied().unwrap_or(false) {
                continue;
            }
            self.tracks[index].missed_windows += 1;
            if self.tracks[index].missed_windows >= 2 {
                let track = self.tracks.remove(index);
                events.push(DetectedNote {
                    pitch_hz: track.pitch_hz,
                    strength: track.strength,
                    noise_floor: self.noise_floor,
                    phase: NotePhase::Ended,
                    duration_secs: (self.processed_samples - track.started_sample) as f32
                        / self.sample_rate,
                });
            }
        }
        events
    }
}

fn spectral_peaks(samples: &[f32], sample_rate: f32, threshold: f32) -> Vec<(f32, f32)> {
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(samples.len());
    let mut spectrum = samples
        .iter()
        .enumerate()
        .map(|(index, sample)| {
            let window = 0.5
                * (1.0 - (std::f32::consts::TAU * index as f32 / (samples.len() - 1) as f32).cos());
            Complex::new(sample * window, 0.0)
        })
        .collect::<Vec<_>>();
    fft.process(&mut spectrum);
    let minimum_bin = (30.0 * samples.len() as f32 / sample_rate).ceil() as usize;
    let maximum_bin = (1400.0 * samples.len() as f32 / sample_rate)
        .floor()
        .min((samples.len() / 2 - 1) as f32) as usize;
    let magnitudes = (minimum_bin..=maximum_bin)
        .map(|bin| spectrum[bin].norm() / samples.len() as f32 * 2.0)
        .collect::<Vec<_>>();
    let mut peaks = (1..magnitudes.len().saturating_sub(1))
        .filter_map(|offset| {
            let magnitude = magnitudes[offset];
            (magnitude > threshold
                && magnitude >= magnitudes[offset - 1]
                && magnitude >= magnitudes[offset + 1])
                .then_some((minimum_bin + offset, magnitude))
        })
        .collect::<Vec<_>>();
    peaks.sort_by(|left, right| right.1.total_cmp(&left.1));
    peaks
        .into_iter()
        .take(6)
        .map(|(bin, strength)| {
            (
                bin as f32 * sample_rate / samples.len() as f32,
                (strength * 8.0).clamp(0.15, 1.0),
            )
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
/// Instruments supported by the input pipeline.
enum Instrument {
    Guitar,
    Bass4,
    Bass5,
    Drums,
    Vocals,
}

#[derive(Clone, Copy, Debug)]
/// Normalized input event sent from an audio or MIDI callback to Bevy.
struct InstrumentEvent {
    instrument: Instrument,
    lane: usize,
    strength: f32,
    pitch_hz: Option<f32>,
    noise_floor: f32,
    phase: NotePhase,
    duration_secs: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NotePhase {
    Started,
    Updated,
    Ended,
}

#[derive(Resource)]
/// Shared event channel and lifetime handles for the input worker.
struct InstrumentStream {
    sender: Sender<InstrumentEvent>,
    events: Mutex<Receiver<InstrumentEvent>>,
    _thread: Option<thread::JoinHandle<()>>,
    stop_sender: Option<Sender<()>>,
    started: bool,
}

#[derive(Resource, Default)]
/// Selection state for the Home and Set Up menus.
struct MenuSelection {
    home_selected: usize,
    setup_selected: usize,
}

#[derive(Component)]
/// Text node used by the Home and Set Up menus.
struct MenuText;

#[derive(Component)]
/// Camera owned by a menu screen.
struct MenuCamera;

#[derive(Resource)]
/// Available devices and current choices in Input Setup.
struct DeviceSelection {
    audio_devices: Vec<String>,
    midi_devices: Vec<String>,
    selected: [usize; 4],
    focus: usize,
    bass_strings: u8,
}

#[derive(Resource, Serialize, Deserialize, Default, Debug)]
/// Settings persisted between launches in `open-band-settings/settings.json`.
struct PersistentSettings {
    guitar_device: Option<String>,
    bass_device: Option<String>,
    midi_device: Option<String>,
    vocal_device: Option<String>,
    bass_strings: Option<u8>,
    latency_ms: Option<f32>,
}

#[derive(Component)]
/// Text node used by Input Setup.
struct DeviceSelectionText;

#[derive(Component)]
/// Camera owned by Input Setup.
struct DeviceSelectionCamera;

#[derive(Resource, Default)]
/// Accumulated gameplay score.
struct Score {
    hits: u32,
    combo: u32,
    accuracy: f32,
}

#[derive(Resource)]
/// Signal and tuner state displayed during instrument calibration.
struct Calibration {
    selected: Instrument,
    level: f32,
    peak: f32,
    samples: u32,
    last_pitch_hz: Option<f32>,
}

#[derive(Component)]
/// Calibration heading and status text.
struct CalibrationText;

#[derive(Component)]
/// Calibration signal meter transform.
struct CalibrationMeter;

#[derive(Component)]
/// Camera owned by the calibration screen.
struct CalibrationCamera;

#[derive(Resource)]
/// Timing measurements collected by the latency calibration screen.
struct LatencyCalibration {
    started_at: f32,
    best_ms: Option<f32>,
    attempts: u32,
}

#[derive(Component)]
/// Latency screen status text.
struct LatencyText;

#[derive(Component)]
/// Moving beat marker used for latency taps.
struct LatencyPulse;

#[derive(Component)]
/// Camera owned by latency calibration.
struct LatencyCamera;

#[derive(Component)]
/// A falling gameplay note and the lane it belongs to.
struct FallingNote {
    lane: usize,
    spawned_at: f32,
}

#[derive(Component)]
/// Marker shared by all entities owned by the live session.
struct GameplayEntity;

#[derive(Resource, Default)]
/// Latest event values shown in the live-session debug window.
struct DebugInputData {
    instrument: Option<Instrument>,
    pitch_hz: Option<f32>,
    lane: Option<usize>,
    strength: f32,
    noise_floor: f32,
    duration_secs: f32,
    event_count: u64,
}

#[derive(Component)]
/// Debug panel background and visibility target.
struct DebugWindow;

#[derive(Component)]
/// Text node containing live input diagnostics.
struct DebugText;

/// Build and run the Bevy application.
fn main() {
    // Load persisted choices before scanning devices so saved names can be preselected.
    let (sender, receiver) = mpsc::channel();
    let settings = load_settings();
    let device_selection = scan_devices(&settings);
    let (stop_sender, stop_receiver) = mpsc::channel();
    let input_thread = spawn_instrument_thread(
        sender.clone(),
        input_config_from_settings(&settings),
        stop_receiver,
    );

    // Start the input worker before Home so Live Session works immediately after launch.
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.025, 0.035, 0.06)))
        .insert_resource(device_selection)
        .insert_resource(settings)
        .insert_resource(InstrumentStream {
            sender,
            events: Mutex::new(receiver),
            _thread: Some(input_thread),
            stop_sender: Some(stop_sender),
            started: true,
        })
        .init_resource::<MenuSelection>()
        .insert_resource(Calibration {
            selected: Instrument::Guitar,
            level: 0.0,
            peak: 0.0,
            samples: 0,
            last_pitch_hz: None,
        })
        .init_resource::<Score>()
        .init_resource::<DebugInputData>()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Open band".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .init_state::<AppState>()
        .add_systems(OnEnter(AppState::Home), setup_home)
        .add_systems(Update, home_input.run_if(in_state(AppState::Home)))
        .add_systems(Update, home_display.run_if(in_state(AppState::Home)))
        .add_systems(OnExit(AppState::Home), cleanup_menu)
        .add_systems(OnEnter(AppState::Setup), setup_setup)
        .add_systems(Update, setup_input.run_if(in_state(AppState::Setup)))
        .add_systems(Update, setup_display.run_if(in_state(AppState::Setup)))
        .add_systems(OnExit(AppState::Setup), cleanup_menu)
        .add_systems(OnEnter(AppState::DeviceSelection), setup_device_selection)
        .add_systems(
            Update,
            device_selection_input.run_if(in_state(AppState::DeviceSelection)),
        )
        .add_systems(
            Update,
            device_selection_display.run_if(in_state(AppState::DeviceSelection)),
        )
        .add_systems(OnExit(AppState::DeviceSelection), cleanup_device_selection)
        .add_systems(OnEnter(AppState::Calibration), setup_calibration)
        .add_systems(
            Update,
            calibration_input.run_if(in_state(AppState::Calibration)),
        )
        .add_systems(
            Update,
            calibration_display.run_if(in_state(AppState::Calibration)),
        )
        .add_systems(OnExit(AppState::Calibration), cleanup_calibration)
        .add_systems(
            OnEnter(AppState::LatencyCalibration),
            setup_latency_calibration,
        )
        .add_systems(
            Update,
            latency_calibration_input.run_if(in_state(AppState::LatencyCalibration)),
        )
        .add_systems(
            Update,
            latency_calibration_display.run_if(in_state(AppState::LatencyCalibration)),
        )
        .add_systems(
            OnExit(AppState::LatencyCalibration),
            cleanup_latency_calibration,
        )
        .add_systems(OnEnter(AppState::Gameplay), setup_gameplay)
        .add_systems(
            Update,
            (
                receive_instrument_events,
                move_notes,
                hit_notes,
                gameplay_menu_input,
                debug_input,
            )
                .run_if(in_state(AppState::Gameplay)),
        )
        .add_systems(OnExit(AppState::Gameplay), cleanup_gameplay)
        .run();
}

/// Runtime device configuration consumed by the input worker.
struct InputConfig {
    /// Device names passed to the worker after the setup screen is accepted.
    audio_devices: [Option<String>; 3],
    midi_device: Option<String>,
    bass_strings: u8,
}

/// Convert saved settings into the worker's runtime input configuration.
fn input_config_from_settings(settings: &PersistentSettings) -> InputConfig {
    // Convert the serializable settings shape into the worker's runtime configuration.
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

/// Enumerate audio and MIDI devices and restore saved selections where possible.
fn scan_devices(settings: &PersistentSettings) -> DeviceSelection {
    // Enumerate current hardware once; unavailable saved devices fall back to index zero.
    let host = cpal::default_host();
    let audio_devices = host
        .input_devices()
        .map(|devices| devices.map(|device| device.to_string()).collect::<Vec<_>>())
        .unwrap_or_default();
    let midi_devices = MidiInput::new("open-band-device-scan")
        .map(|input| {
            input
                .ports()
                .iter()
                .map(|port| {
                    input
                        .port_name(port)
                        .unwrap_or_else(|_| "Unknown MIDI device".into())
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

/// Load JSON settings from the working directory, or return defaults on failure.
fn load_settings() -> PersistentSettings {
    // Invalid or missing files are treated as first-run defaults.
    std::fs::read_to_string(SETTINGS_FILE)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_default()
}

/// Serialize settings to the working-directory settings file.
fn save_settings(settings: &PersistentSettings) {
    // Create the working-directory folder lazily when the user confirms setup.
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

/// Spawn the Input Setup screen.
fn setup_device_selection(mut commands: Commands) {
    // Build the device selection screen and its camera-owned text node.
    commands.spawn((Camera2d, DeviceSelectionCamera));
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(25.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(44.0),
            left: Val::Px(70.0),
            ..default()
        },
        DeviceSelectionText,
    ));
}

/// Handle device row navigation, device cycling, and setup confirmation.
fn device_selection_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<DeviceSelection>,
    mut stream: ResMut<InstrumentStream>,
    mut settings: ResMut<PersistentSettings>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    // Apply row and device cycling before committing the chosen configuration.
    let focus_keys = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ];
    for (index, key) in focus_keys.into_iter().enumerate() {
        if keyboard.just_pressed(key) {
            selection.focus = index;
        }
    }
    if keyboard.just_pressed(KeyCode::KeyB) {
        selection.bass_strings = if selection.bass_strings == 4 { 5 } else { 4 };
    }

    let device_count = if selection.focus == 2 {
        selection.midi_devices.len()
    } else {
        selection.audio_devices.len()
    };
    if device_count > 0 && keyboard.just_pressed(KeyCode::ArrowLeft) {
        let focus = selection.focus;
        selection.selected[focus] = selection.selected[focus]
            .checked_sub(1)
            .unwrap_or(device_count - 1);
    }
    if device_count > 0 && keyboard.just_pressed(KeyCode::ArrowRight) {
        let focus = selection.focus;
        selection.selected[focus] = (selection.selected[focus] + 1) % device_count;
    }

    if keyboard.just_pressed(KeyCode::Enter) {
        let audio_name = |index: usize| {
            selection
                .audio_devices
                .get(selection.selected[index])
                .cloned()
        };
        let midi_name = selection.midi_devices.get(selection.selected[2]).cloned();
        let config = InputConfig {
            audio_devices: [audio_name(0), audio_name(1), audio_name(3)],
            midi_device: midi_name,
            bass_strings: selection.bass_strings,
        };
        settings.guitar_device = config.audio_devices[0].clone();
        settings.bass_device = config.audio_devices[1].clone();
        settings.vocal_device = config.audio_devices[2].clone();
        settings.midi_device = config.midi_device.clone();
        settings.bass_strings = Some(config.bass_strings);
        save_settings(&settings);
        if let Some(stop_sender) = stream.stop_sender.take() {
            let _ = stop_sender.send(());
            if let Some(thread) = stream._thread.take() {
                let _ = thread.join();
            }
        }
        let (stop_sender, stop_receiver) = mpsc::channel();
        stream._thread = Some(spawn_instrument_thread(
            stream.sender.clone(),
            config,
            stop_receiver,
        ));
        stream.stop_sender = Some(stop_sender);
        stream.started = true;
        next_state.set(AppState::Setup);
    }
}

/// Render current device selections and the focused setup row.
fn device_selection_display(
    selection: Res<DeviceSelection>,
    mut text: Query<&mut Text, With<DeviceSelectionText>>,
) {
    // Render the currently focused row and selected device names.
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let device_name = |devices: &[String], selected: usize| {
        devices
            .get(selected)
            .cloned()
            .unwrap_or_else(|| "NO DEVICE FOUND".into())
    };
    let marker = |index: usize| if selection.focus == index { ">" } else { " " };
    *text = Text::new(format!(
        "OPEN BAND  //  INPUT DEVICES\n\n\
        {} [1] GUITAR\n      {}\n\n\
        {} [2] BASS (B to switch {}-string)\n      {}\n\n\
        {} [3] MIDI DRUMS\n      {}\n\n\
        {} [4] VOCALS\n      {}\n\n\
        Left/Right: choose device     Enter: continue\n\
        Environment variables remain supported as defaults.",
        marker(0),
        device_name(&selection.audio_devices, selection.selected[0]),
        marker(1),
        selection.bass_strings,
        device_name(&selection.audio_devices, selection.selected[1]),
        marker(2),
        device_name(&selection.midi_devices, selection.selected[2]),
        marker(3),
        device_name(&selection.audio_devices, selection.selected[3]),
    ));
}

/// Despawn Input Setup entities when leaving that screen.
fn cleanup_device_selection(
    mut commands: Commands,
    entities: Query<Entity, Or<(With<DeviceSelectionText>, With<DeviceSelectionCamera>)>>,
) {
    // Remove this screen's entities before entering Setup.
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

/// Spawn the Home screen.
fn setup_home(mut commands: Commands) {
    // Create the two-entry application landing screen.
    commands.spawn((Camera2d, MenuCamera));
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(30.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(60.0),
            left: Val::Px(90.0),
            ..default()
        },
        MenuText,
    ));
}

/// Spawn the Set Up screen.
fn setup_setup(mut commands: Commands) {
    // Create the submenu for all configuration tools.
    commands.spawn((Camera2d, MenuCamera));
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(30.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(60.0),
            left: Val::Px(90.0),
            ..default()
        },
        MenuText,
    ));
}

/// Navigate Home and open Live Session or Set Up.
fn home_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<MenuSelection>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    // Home routes only to Live Session or Set Up.
    if keyboard.just_pressed(KeyCode::ArrowUp) {
        menu.home_selected = menu.home_selected.checked_sub(1).unwrap_or(1);
    }
    if keyboard.just_pressed(KeyCode::ArrowDown) {
        menu.home_selected = (menu.home_selected + 1) % 2;
    }
    for (index, key) in [KeyCode::Digit1, KeyCode::Digit2].into_iter().enumerate() {
        if keyboard.just_pressed(key) {
            menu.home_selected = index;
        }
    }
    if keyboard.just_pressed(KeyCode::Enter) {
        next_state.set(match menu.home_selected {
            0 => AppState::Gameplay,
            _ => AppState::Setup,
        });
    }
}

/// Render the selected Home destination.
fn home_display(menu: Res<MenuSelection>, mut text: Query<&mut Text, With<MenuText>>) {
    // Keep the highlighted Home option synchronized with keyboard navigation.
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let marker = |index: usize| {
        if menu.home_selected == index {
            ">"
        } else {
            " "
        }
    };
    *text = Text::new(format!(
        "OPEN BAND  //  HOME\n\n\
        {} [1] LIVE SESSION\n\
        {} [2] SET UP\n\n\
        Up/Down: navigate     Enter: open",
        marker(0),
        marker(1),
    ));
}

/// Navigate Set Up and open a setup tool or Home.
fn setup_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<MenuSelection>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    // Route each setup option to its tool or back to Home.
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Home);
        return;
    }
    if keyboard.just_pressed(KeyCode::ArrowUp) {
        menu.setup_selected = menu.setup_selected.checked_sub(1).unwrap_or(3);
    }
    if keyboard.just_pressed(KeyCode::ArrowDown) {
        menu.setup_selected = (menu.setup_selected + 1) % 4;
    }
    for (index, key) in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ]
    .into_iter()
    .enumerate()
    {
        if keyboard.just_pressed(key) {
            menu.setup_selected = index;
        }
    }
    if keyboard.just_pressed(KeyCode::Enter) {
        next_state.set(match menu.setup_selected {
            0 => AppState::DeviceSelection,
            1 => AppState::Calibration,
            2 => AppState::LatencyCalibration,
            _ => AppState::Home,
        });
    }
}

/// Render the selected setup tool.
fn setup_display(menu: Res<MenuSelection>, mut text: Query<&mut Text, With<MenuText>>) {
    // Render the four setup destinations and their selection marker.
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let marker = |index: usize| {
        if menu.setup_selected == index {
            ">"
        } else {
            " "
        }
    };
    *text = Text::new(format!(
        "OPEN BAND  //  SET UP\n\n\
        {} [1] INPUT SETUP\n\
        {} [2] TUNER\n\
        {} [3] LATENCY CALIBRATION\n\
        {} [4] BACK\n\n\
        Up/Down: navigate     Enter: open     Esc: home",
        marker(0),
        marker(1),
        marker(2),
        marker(3),
    ));
}

/// Despawn the shared menu camera and text entities.
fn cleanup_menu(
    mut commands: Commands,
    entities: Query<Entity, Or<(With<MenuText>, With<MenuCamera>)>>,
) {
    // Both menu screens share the same text and camera marker types.
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

/// Resolve a saved or environment-provided device name to an enumeration index.
fn selected_device_index(devices: &[String], saved: Option<&str>, variable: &str) -> usize {
    // Prefer an exact saved name, then retain environment-variable compatibility.
    saved
        .map(str::to_owned)
        .or_else(|| std::env::var(variable).ok())
        .and_then(|wanted| {
            devices
                .iter()
                .position(|device| device == &wanted || device.contains(&wanted))
        })
        .unwrap_or(0)
}

/// Open configured audio and MIDI inputs on a dedicated worker thread.
fn spawn_instrument_thread(
    sender: Sender<InstrumentEvent>,
    config: InputConfig,
    stop_receiver: Receiver<()>,
) -> thread::JoinHandle<()> {
    // Keep device ownership on a worker so real-time callbacks never block Bevy.
    thread::spawn(move || {
        let bass_instrument = if config.bass_strings == 5 {
            Instrument::Bass5
        } else {
            Instrument::Bass4
        };
        if let Ok(path) = std::env::var(RECORDING_ENVIRONMENT_VARIABLE) {
            run_recording_input(&path, bass_instrument, sender, stop_receiver);
            return;
        }

        let host = cpal::default_host();
        let mut streams = Vec::new();

        for (instrument, device_name) in [
            (Instrument::Guitar, config.audio_devices[0].as_deref()),
            (bass_instrument, config.audio_devices[1].as_deref()),
            (Instrument::Vocals, config.audio_devices[2].as_deref()),
        ] {
            match open_audio_input(&host, instrument, device_name, sender.clone()) {
                Ok(stream) => streams.push(stream),
                Err(error) => eprintln!("{instrument:?} input unavailable: {error}"),
            }
        }

        let midi_connection = open_midi_input(sender.clone(), config.midi_device.as_deref());
        loop {
            if stop_receiver
                .recv_timeout(Duration::from_millis(100))
                .is_ok()
            {
                break;
            }
            if streams.is_empty() && midi_connection.is_none() {
                eprintln!(
                    "No instrument inputs are active; choose available devices in the dialog."
                );
                break;
            }
        }
    })
}

/// Open one CPAL input and attach the onset/pitch callback.
fn open_audio_input(
    host: &cpal::Host,
    instrument: Instrument,
    device_name: Option<&str>,
    sender: Sender<InstrumentEvent>,
) -> Result<cpal::Stream, String> {
    // Select the requested device and build a callback for its native sample format.
    let device = find_input_device(host, device_name)?;
    let supported = device
        .default_input_config()
        .map_err(|error| error.to_string())?;
    let config = supported.config();
    let channels = config.channels as usize;
    let error_callback = |error| eprintln!("audio input error: {error}");

    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => build_audio_stream::<f32>(
            &device,
            &config,
            channels,
            instrument,
            sender,
            error_callback,
        ),
        cpal::SampleFormat::I16 => build_audio_stream::<i16>(
            &device,
            &config,
            channels,
            instrument,
            sender,
            error_callback,
        ),
        cpal::SampleFormat::U16 => build_audio_stream::<u16>(
            &device,
            &config,
            channels,
            instrument,
            sender,
            error_callback,
        ),
        format => return Err(format!("unsupported sample format {format:?}")),
    }
    .map_err(|error| error.to_string())?;

    stream.play().map_err(|error| error.to_string())?;
    println!("Listening to {instrument:?} on {device}");
    Ok(stream)
}

/// Build a typed CPAL callback that converts samples into normalized events.
fn build_audio_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    instrument: Instrument,
    sender: Sender<InstrumentEvent>,
    error_callback: impl FnMut(cpal::Error) + Send + 'static,
) -> Result<cpal::Stream, cpal::Error>
where
    T: cpal::Sample + cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    // Convert each input frame to mono before onset and pitch analysis.
    let mut detector = AudioDetector::new(instrument, config.sample_rate as f32);
    device.build_input_stream(
        *config,
        move |data: &[T], _| {
            let mono = data.chunks(channels).map(|frame| {
                frame
                    .iter()
                    .map(|sample| sample.to_sample::<f32>())
                    .sum::<f32>()
                    / frame.len().max(1) as f32
            });
            send_detected_events(&mut detector, mono, instrument, &sender);
        },
        error_callback,
        None,
    )
}

/// Read a mono 24-bit WAV file and feed it through the same detector as live audio.
fn run_recording_input(
    path: &str,
    instrument: Instrument,
    sender: Sender<InstrumentEvent>,
    stop_receiver: Receiver<()>,
) {
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let mut reader = hound::WavReader::open(path)?;
        let spec = reader.spec();
        let mut detector = AudioDetector::Mono(AudioOnsetDetector {
            sample_rate: spec.sample_rate as f32,
            ..Default::default()
        });
        for sample in reader.samples::<i32>() {
            if stop_receiver.try_recv().is_ok() {
                return Ok(());
            }
            send_detected_events(
                &mut detector,
                std::iter::once(sample? as f32 / 8_388_608.0),
                instrument,
                &sender,
            );
        }
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("Could not play recording {path}: {error}");
    }
}

/// Convert detector output into the event shape consumed by gameplay and calibration.
fn send_detected_events(
    detector: &mut AudioDetector,
    samples: impl Iterator<Item = f32>,
    instrument: Instrument,
    sender: &Sender<InstrumentEvent>,
) {
    for detected in detector.detect(samples) {
        let _ = sender.send(InstrumentEvent {
            instrument,
            lane: pitch_to_lane(instrument, detected.pitch_hz),
            strength: detected.strength,
            pitch_hz: Some(detected.pitch_hz),
            noise_floor: detected.noise_floor,
            phase: detected.phase,
            duration_secs: detected.duration_secs,
        });
    }
}

struct DetectedNote {
    pitch_hz: f32,
    strength: f32,
    noise_floor: f32,
    phase: NotePhase,
    duration_secs: f32,
}

enum AudioDetector {
    Mono(AudioOnsetDetector),
    Poly(PolyphonicAudioDetector),
}

impl AudioDetector {
    fn new(instrument: Instrument, sample_rate: f32) -> Self {
        match instrument {
            Instrument::Vocals | Instrument::Drums => Self::Mono(AudioOnsetDetector {
                sample_rate,
                ..Default::default()
            }),
            Instrument::Guitar | Instrument::Bass4 | Instrument::Bass5 => {
                Self::Poly(PolyphonicAudioDetector::new(sample_rate))
            }
        }
    }

    fn detect(&mut self, samples: impl Iterator<Item = f32>) -> Vec<DetectedNote> {
        match self {
            Self::Mono(detector) => detector.detect_with_duration(samples),
            Self::Poly(detector) => detector.detect(samples),
        }
    }
}

#[derive(Default)]
/// Stateful onset detector and audio sample buffer.
struct AudioOnsetDetector {
    average: f32,
    noise_floor: f32,
    processed_samples: usize,
    last_event_sample: Option<usize>,
    last_pitch_hz: Option<f32>,
    pending_pitch_hz: Option<f32>,
    pending_pitch_count: usize,
    sample_rate: f32,
    samples: Vec<f32>,
    last_level: f32,
    active_pitch_hz: Option<f32>,
    active_started_sample: usize,
    silent_windows: usize,
}

impl AudioOnsetDetector {
    /// Analyze a buffered window and return strength, frequency, and noise floor.
    /// Detect a strong onset and estimate its fundamental frequency.
    fn detect(&mut self, samples: impl Iterator<Item = f32>) -> Option<(f32, f32, f32)> {
        // Wait for a low-frequency-friendly window before analyzing the signal.
        self.samples.extend(samples);
        if self.samples.len() < 4096 {
            return None;
        }

        let window = self.samples[..4096].to_vec();
        self.samples.drain(..2048);
        self.processed_samples += 2048;
        let level =
            (window.iter().map(|sample| sample * sample).sum::<f32>() / window.len() as f32).sqrt();
        self.last_level = level;
        // Track the background slowly so quiet playing can sit close to the noise floor.
        self.noise_floor = self.noise_floor * 0.995 + level * 0.005;
        self.average = self.average * 0.96 + level * 0.04;
        let pitch_hz = estimate_pitch(&window, self.sample_rate);
        let ready = self.last_event_sample.map_or(true, |event| {
            self.processed_samples.saturating_sub(event) > (self.sample_rate * 0.12) as usize
        });
        let minimum_level = (self.noise_floor * 3.0).max(0.015);
        let pitch_changed = self
            .last_pitch_hz
            .zip(pitch_hz)
            .is_some_and(|(last, current)| (current / last).log2().abs() > 150.0 / 1200.0);
        let confirmed_pitch_change = if pitch_changed {
            let Some(current_pitch) = pitch_hz else {
                return None;
            };
            if self
                .pending_pitch_hz
                .is_some_and(|pending| (current_pitch / pending).log2().abs() <= 80.0 / 1200.0)
            {
                self.pending_pitch_count += 1;
            } else {
                self.pending_pitch_hz = Some(current_pitch);
                self.pending_pitch_count = 1;
            }
            self.pending_pitch_count >= 2
        } else {
            self.pending_pitch_hz = None;
            self.pending_pitch_count = 0;
            false
        };
        if level > minimum_level && (level > self.average * 1.6 || confirmed_pitch_change) && ready
        {
            self.last_event_sample = Some(self.processed_samples);
            if let Some(pitch_hz) = pitch_hz {
                self.last_pitch_hz = Some(pitch_hz);
                self.pending_pitch_hz = None;
                self.pending_pitch_count = 0;
                Some(((level * 4.0).clamp(0.15, 1.0), pitch_hz, self.noise_floor))
            } else {
                None
            }
        } else {
            None
        }
    }

    fn detect_with_duration(&mut self, samples: impl Iterator<Item = f32>) -> Vec<DetectedNote> {
        let onset = self.detect(samples);
        if let Some((strength, pitch_hz, noise_floor)) = onset {
            let phase = if self.active_pitch_hz.is_some() {
                NotePhase::Updated
            } else {
                self.active_started_sample = self.processed_samples;
                NotePhase::Started
            };
            self.active_pitch_hz = Some(pitch_hz);
            self.silent_windows = 0;
            return vec![DetectedNote {
                pitch_hz,
                strength,
                noise_floor,
                phase,
                duration_secs: (self.processed_samples - self.active_started_sample) as f32
                    / self.sample_rate,
            }];
        }

        let Some(pitch_hz) = self.active_pitch_hz else {
            return Vec::new();
        };
        let minimum_level = (self.noise_floor * 3.0).max(0.015);
        if self.last_level <= minimum_level {
            self.silent_windows += 1;
        } else {
            self.silent_windows = 0;
        }
        if self.silent_windows < 2 {
            return vec![DetectedNote {
                pitch_hz,
                strength: (self.last_level * 4.0).clamp(0.15, 1.0),
                noise_floor: self.noise_floor,
                phase: NotePhase::Updated,
                duration_secs: (self.processed_samples - self.active_started_sample) as f32
                    / self.sample_rate,
            }];
        }
        self.active_pitch_hz = None;
        vec![DetectedNote {
            pitch_hz,
            strength: 0.0,
            noise_floor: self.noise_floor,
            phase: NotePhase::Ended,
            duration_secs: (self.processed_samples - self.active_started_sample) as f32
                / self.sample_rate,
        }]
    }
}

/// Estimate a fundamental frequency with a normalized YIN-style period search.
fn estimate_pitch(samples: &[f32], sample_rate: f32) -> Option<f32> {
    // Estimate the fundamental period with a normalized difference function.
    if samples.len() < 256 || sample_rate <= 0.0 {
        return None;
    }

    let mean = samples.iter().sum::<f32>() / samples.len() as f32;
    let centered = samples
        .iter()
        .map(|sample| sample - mean)
        .collect::<Vec<_>>();
    let min_lag = (sample_rate / 1400.0).floor().max(2.0) as usize;
    let max_lag = (sample_rate / 30.0).ceil() as usize;
    if max_lag >= centered.len() {
        return None;
    }

    // YIN's cumulative difference function finds the first strong period,
    // which avoids mistaking a strong harmonic for the fundamental.
    let mut difference = vec![0.0; max_lag + 1];
    for lag in min_lag..=max_lag {
        difference[lag] = centered[..centered.len() - lag]
            .iter()
            .zip(&centered[lag..])
            .map(|(left, right)| {
                let delta = left - right;
                delta * delta
            })
            .sum();
    }

    let mut running_sum = 0.0;
    let mut best_lag = None;
    for lag in min_lag..=max_lag {
        running_sum += difference[lag];
        let normalized = difference[lag] * lag as f32 / running_sum.max(f32::EPSILON);
        if normalized < 0.18
            && (lag == max_lag
                || normalized
                    <= difference[lag + 1] * (lag + 1) as f32
                        / (running_sum + difference[lag + 1]).max(f32::EPSILON))
        {
            best_lag = Some(lag);
            break;
        }
    }
    let lag = best_lag.or_else(|| {
        (min_lag..=max_lag).min_by(|left, right| difference[*left].total_cmp(&difference[*right]))
    })?;

    let refined_lag = if lag > min_lag && lag < max_lag {
        let previous = difference[lag - 1];
        let current = difference[lag];
        let next = difference[lag + 1];
        let denominator = previous - 2.0 * current + next;
        if denominator.abs() > f32::EPSILON {
            lag as f32 + 0.5 * (previous - next) / denominator
        } else {
            lag as f32
        }
    } else {
        lag as f32
    };
    let pitch_hz = sample_rate / refined_lag;
    (30.0..=1400.0).contains(&pitch_hz).then_some(pitch_hz)
}

/// Convert an instrument pitch into its gameplay lane.
fn pitch_to_lane(instrument: Instrument, pitch_hz: f32) -> usize {
    // Bass uses physical-string lanes; other pitched instruments use logarithmic bands.
    let (low, high, lane_count) = match instrument {
        Instrument::Guitar => (82.0, 988.0, LANES),
        Instrument::Vocals => (80.0, 1200.0, LANES),
        Instrument::Drums => return 0,
        Instrument::Bass4 | Instrument::Bass5 => return bass_string_lane(instrument, pitch_hz),
    };
    let normalized = ((pitch_hz / low).ln() / (high / low).ln()).clamp(0.0, 0.999);
    (normalized * lane_count as f32) as usize
}

/// Map bass pitch to the nearest physical string lane.
fn bass_string_lane(instrument: Instrument, pitch_hz: f32) -> usize {
    // Choose the string whose open-note frequency is closest to the detected pitch.
    let strings = match instrument {
        Instrument::Bass5 => [30.87, 41.20, 55.00, 73.42, 98.00],
        Instrument::Bass4 => [41.20, 55.00, 73.42, 98.00, 98.00],
        _ => return 0,
    };
    strings
        .into_iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            (pitch_hz / left)
                .ln()
                .abs()
                .total_cmp(&(pitch_hz / right).ln().abs())
        })
        .map_or(0, |(lane, _)| {
            lane.min(if matches!(instrument, Instrument::Bass5) {
                4
            } else {
                3
            })
        })
}

/// Find a CPAL input by exact configured name, or return an error.
fn find_input_device(host: &cpal::Host, requested: Option<&str>) -> Result<cpal::Device, String> {
    // CPAL exposes names dynamically, so compare the saved name while enumerating.
    let devices = host.input_devices().map_err(|error| error.to_string())?;
    for device in devices {
        let name = device.to_string();
        if requested.is_none_or(|wanted| name == wanted) {
            return Ok(device);
        }
    }
    Err(requested.map_or_else(
        || "no input device is available".into(),
        |name| format!("no input device matched {name}"),
    ))
}

/// Connect to the configured MIDI port and forward drum hit events.
fn open_midi_input(
    sender: Sender<InstrumentEvent>,
    requested: Option<&str>,
) -> Option<midir::MidiInputConnection<()>> {
    // Connect the selected MIDI port and translate drum notes into lanes.
    let mut input = MidiInput::new("open-band-drums").ok()?;
    input.ignore(Ignore::None);
    let port = input.ports().into_iter().find(|port| {
        let name = input.port_name(port).unwrap_or_default();
        requested.is_none_or(|wanted| name == wanted)
    })?;
    let name = input.port_name(&port).unwrap_or_default();
    let connection = input
        .connect(
            &port,
            "open-band-midi-input",
            move |_, message, _| {
                if message.len() >= 3 && message[0] & 0xf0 == 0x90 && message[2] > 0 {
                    let lane = match message[1] {
                        36 | 35 => 0,
                        38 | 40 => 1,
                        42 | 44 | 46 => 2,
                        45 | 47 | 48 => 3,
                        _ => 4,
                    };
                    let _ = sender.send(InstrumentEvent {
                        instrument: Instrument::Drums,
                        lane,
                        strength: message[2] as f32 / 127.0,
                        pitch_hz: None,
                        noise_floor: 0.0,
                        phase: NotePhase::Started,
                        duration_secs: 0.0,
                    });
                }
            },
            (),
        )
        .ok()?;
    println!("Listening to MIDI drums on {name}");
    Some(connection)
}

/// Spawn the signal calibration and bass tuner screen.
fn setup_calibration(mut commands: Commands) {
    // Build the signal meter and tuner screen.
    commands.spawn((Camera2d, CalibrationCamera));
    commands.spawn((
        Text::new("OPEN BAND  //  INPUT CALIBRATION"),
        TextFont {
            font_size: FontSize::Px(34.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(42.0),
            left: Val::Px(70.0),
            ..default()
        },
        CalibrationText,
    ));
    commands.spawn((
        Sprite {
            color: Color::srgb(0.15, 0.2, 0.3),
            custom_size: Some(Vec2::new(760.0, 26.0)),
            ..default()
        },
        Transform::from_xyz(0.0, -160.0, 0.0),
        CalibrationMeter,
    ));
}

/// Spawn the timing calibration screen with the previous saved result.
fn setup_latency_calibration(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<PersistentSettings>,
) {
    // Restore the previous best result while starting a fresh timing session.
    commands.insert_resource(LatencyCalibration {
        started_at: time.elapsed_secs(),
        best_ms: settings.latency_ms,
        attempts: 0,
    });
    commands.spawn((Camera2d, LatencyCamera));
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(27.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(34.0),
            left: Val::Px(52.0),
            ..default()
        },
        LatencyText,
    ));
    for lane in 0..LANES {
        let x = -360.0 + lane as f32 * 180.0;
        commands.spawn((
            Sprite {
                color: lane_color(lane).with_alpha(0.2),
                custom_size: Some(Vec2::new(122.0, 520.0)),
                ..default()
            },
            Transform::from_xyz(x, -40.0, 0.0),
        ));
    }
    commands.spawn((
        Sprite {
            color: Color::srgb(1.0, 0.85, 0.25),
            custom_size: Some(Vec2::new(9.0, 520.0)),
            ..default()
        },
        Transform::from_xyz(0.0, -40.0, 1.0),
        LatencyPulse,
    ));
}

/// Collect timing taps and save the accepted latency result.
fn latency_calibration_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut latency: ResMut<LatencyCalibration>,
    mut settings: ResMut<PersistentSettings>,
    mut next_state: ResMut<NextState<AppState>>,
    time: Res<Time>,
) {
    // Record Space taps against the repeating half-second beat.
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Setup);
        return;
    }
    if keyboard.just_pressed(KeyCode::Space) {
        let beat = (time.elapsed_secs() - latency.started_at) % 0.5;
        let offset = if beat > 0.25 { beat - 0.5 } else { beat };
        let offset_ms = offset * 1000.0;
        latency.best_ms = Some(
            latency
                .best_ms
                .map_or(offset_ms.abs(), |best| best.min(offset_ms.abs())),
        );
        latency.attempts += 1;
    }
    if latency.attempts > 0 && keyboard.just_pressed(KeyCode::Enter) {
        settings.latency_ms = latency.best_ms;
        save_settings(&settings);
        next_state.set(AppState::Setup);
    }
}

/// Animate and render the latency calibration target.
fn latency_calibration_display(
    latency: Res<LatencyCalibration>,
    time: Res<Time>,
    mut text: Query<&mut Text, With<LatencyText>>,
    mut pulse: Query<&mut Transform, With<LatencyPulse>>,
) {
    // Animate the beat marker and report the best measured offset.
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let elapsed = time.elapsed_secs() - latency.started_at;
    let phase = (elapsed % 0.5) / 0.5;
    if let Ok(mut transform) = pulse.single_mut() {
        transform.translation.x = -360.0 + phase * 720.0;
    }
    let result = latency.best_ms.map_or("NO TAP RECORDED".into(), |ms| {
        format!("BEST OFFSET  {ms:>5.1} ms")
    });
    *text = Text::new(format!(
        "OPEN BAND  //  LATENCY CALIBRATION\n\n\
        LIVE SESSION CHECK\n\n\
        Tap SPACE as the yellow beat line crosses the center marker.\n\
        Attempts: {}\n{}\n\n\
        Press ENTER to accept and return to Set Up.",
        latency.attempts, result,
    ));
}

/// Despawn latency calibration entities.
fn cleanup_latency_calibration(
    mut commands: Commands,
    entities: Query<Entity, Or<(With<LatencyText>, With<LatencyPulse>, With<LatencyCamera>)>>,
) {
    // Remove latency-specific entities before returning to Set Up.
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

/// Handle calibration instrument selection and collect matching input events.
fn calibration_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut calibration: ResMut<Calibration>,
    stream: Res<InstrumentStream>,
    time: Res<Time>,
) {
    // Change the monitored instrument and collect matching signal events.
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Setup);
        return;
    }
    let choices = [
        (KeyCode::Digit1, Instrument::Guitar),
        (KeyCode::Digit2, Instrument::Bass4),
        (KeyCode::Digit3, Instrument::Bass5),
        (KeyCode::Digit4, Instrument::Drums),
        (KeyCode::Digit5, Instrument::Vocals),
    ];
    for (key, instrument) in choices {
        if keyboard.just_pressed(key) {
            calibration.selected = instrument;
            calibration.level = 0.0;
            calibration.peak = 0.0;
            calibration.samples = 0;
            calibration.last_pitch_hz = None;
        }
    }
    if keyboard.just_pressed(KeyCode::Enter) {
        next_state.set(AppState::Setup);
    }

    calibration.level = (calibration.level - time.delta_secs() * 0.7).max(0.0);
    let Ok(events) = stream.events.lock() else {
        return;
    };
    for event in events.try_iter() {
        if event.instrument as u8 == calibration.selected as u8 {
            calibration.level = event.strength;
            calibration.peak = calibration.peak.max(event.strength);
            calibration.samples += 1;
            calibration.last_pitch_hz = event.pitch_hz;
        }
    }
}

/// Render calibration levels and tuner information.
fn calibration_display(
    calibration: Res<Calibration>,
    mut text: Query<&mut Text, With<CalibrationText>>,
    mut meter: Query<&mut Transform, With<CalibrationMeter>>,
) {
    // Show signal level, peak, and bass tuning feedback.
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let instrument = instrument_name(calibration.selected);
    let source = input_source(calibration.selected);
    let tuner = bass_tuner_reading(calibration.selected, calibration.last_pitch_hz);
    let status = if calibration.samples > 0 {
        "SIGNAL DETECTED"
    } else {
        "WAITING FOR INPUT"
    };
    *text = Text::new(format!(
        concat!(
            "OPEN BAND  //  INPUT CALIBRATION\n\n",
            "[1] GUITAR       [2] BASS 4-STRING\n",
            "[3] BASS 5-STRING [4] MIDI DRUMS\n",
            "[5] VOCALS\n\n",
            "ACTIVE: {}\nSOURCE: {}\n\n",
            "{}\n\n",
            "{}\nLEVEL  {:>3.0}%     PEAK  {:>3.0}%\n\n",
            "Play the selected instrument. Press ENTER when ready."
        ),
        instrument,
        source,
        tuner,
        status,
        calibration.level * 100.0,
        calibration.peak * 100.0,
    ));
    if let Ok(mut transform) = meter.single_mut() {
        transform.scale.x = calibration.level.max(0.02);
    }
}

/// Despawn calibration entities when returning to Set Up.
fn cleanup_calibration(
    mut commands: Commands,
    entities: Query<
        Entity,
        Or<(
            With<CalibrationText>,
            With<CalibrationMeter>,
            With<CalibrationCamera>,
        )>,
    >,
) {
    // Remove calibration text, meter, and camera entities.
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

/// Format the selected bass pitch as a note and cents offset.
fn bass_tuner_reading(instrument: Instrument, pitch_hz: Option<f32>) -> String {
    // Convert the latest bass frequency into a note and cents offset.
    if !matches!(instrument, Instrument::Bass4 | Instrument::Bass5) {
        return "TUNER: select a bass input with [2] or [3]".into();
    }
    let Some(pitch_hz) = pitch_hz else {
        return "BASS TUNER\nPlay an open string to begin tuning.".into();
    };
    let targets = if matches!(instrument, Instrument::Bass5) {
        [
            (30.87, "B"),
            (41.20, "E"),
            (55.00, "A"),
            (73.42, "D"),
            (98.00, "G"),
        ]
    } else {
        [
            (41.20, "E"),
            (55.00, "A"),
            (73.42, "D"),
            (98.00, "G"),
            (98.00, "G"),
        ]
    };
    let (target, note) = targets
        .into_iter()
        .min_by(|(left, _), (right, _)| {
            (pitch_hz - left).abs().total_cmp(&(pitch_hz - right).abs())
        })
        .unwrap();
    let cents = 1200.0 * (pitch_hz / target).log2();
    let verdict = if cents.abs() < 5.0 {
        "IN TUNE"
    } else if cents < 0.0 {
        "TUNE UP"
    } else {
        "TUNE DOWN"
    };
    format!(
        "BASS TUNER\nNOTE  {note}\nPITCH  {pitch_hz:>6.2} Hz\nOFFSET {cents:>+6.1} cents   {verdict}"
    )
}

/// Return the display name for an instrument variant.
fn instrument_name(instrument: Instrument) -> &'static str {
    // Provide stable display labels for instrument variants.
    match instrument {
        Instrument::Guitar => "GUITAR",
        Instrument::Bass4 => "BASS 4-STRING",
        Instrument::Bass5 => "BASS 5-STRING",
        Instrument::Drums => "MIDI DRUMS",
        Instrument::Vocals => "VOCALS",
    }
}

/// Describe where the current instrument input was selected.
fn input_source(instrument: Instrument) -> String {
    // Describe the source consistently now that choices come from Input Setup.
    let _ = instrument;
    "selected in device dialog".into()
}

/// Spawn the live highway and its diagnostic panel.
fn setup_gameplay(mut commands: Commands, debug: Res<DebugInputData>) {
    // Build the live highway, debug panel, and its lane markers.
    commands.spawn((Camera2d, GameplayEntity));
    commands.spawn((
        Text::new("OPEN BAND  //  LIVE SESSION"),
        TextFont {
            font_size: FontSize::Px(28.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(22.0),
            left: Val::Px(34.0),
            ..default()
        },
        GameplayEntity,
    ));
    commands.spawn((
        Text::new("GUITAR  •  BASS 4/5  •  MIDI DRUMS  •  VOCALS    |    A S D F G to play"),
        TextFont {
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::srgb(0.45, 0.55, 0.68)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(62.0),
            left: Val::Px(36.0),
            ..default()
        },
        GameplayEntity,
    ));
    commands.spawn((
        Text::new(debug_text(&debug)),
        TextFont {
            font_size: FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::srgb(0.75, 0.85, 0.95)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(112.0),
            right: Val::Px(32.0),
            width: Val::Px(310.0),
            padding: UiRect::all(Val::Px(16.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.025, 0.04, 0.07, 0.92)),
        DebugWindow,
        DebugText,
        GameplayEntity,
    ));
    for lane in 0..LANES {
        let x = -360.0 + lane as f32 * 180.0;
        commands.spawn((
            Sprite {
                color: Color::srgba(0.15, 0.2, 0.3, 0.55),
                custom_size: Some(Vec2::new(3.0, 570.0)),
                ..default()
            },
            Transform::from_xyz(x, -15.0, 0.0),
            GameplayEntity,
        ));
        commands.spawn((
            Sprite {
                color: lane_color(lane).with_alpha(0.28),
                custom_size: Some(Vec2::new(122.0, 5.0)),
                ..default()
            },
            Transform::from_xyz(x, HIT_LINE_Y, 0.0),
            GameplayEntity,
        ));
    }
}

/// Despawn all live-session entities when leaving Gameplay.
fn cleanup_gameplay(mut commands: Commands, entities: Query<Entity, With<GameplayEntity>>) {
    // Ensure no live-session graphics remain behind another screen.
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

/// Return from Live Session to Home when Escape is pressed.
fn gameplay_menu_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    // Return to Home without leaving the gameplay camera or notes alive.
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Home);
    }
}

/// Toggle the live-session debug panel with F3.
fn debug_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut visibility: Query<&mut Visibility, With<DebugWindow>>,
) {
    // Toggle the diagnostic panel without affecting gameplay input.
    if !keyboard.just_pressed(KeyCode::F3) {
        return;
    }
    let Ok(mut visibility) = visibility.single_mut() else {
        return;
    };
    *visibility = match *visibility {
        Visibility::Visible => Visibility::Hidden,
        Visibility::Hidden | Visibility::Inherited => Visibility::Visible,
    };
}

/// Consume input events, update diagnostics, and spawn falling bars.
fn receive_instrument_events(
    mut commands: Commands,
    stream: Res<InstrumentStream>,
    mut debug: ResMut<DebugInputData>,
    mut text: Query<&mut Text, With<DebugText>>,
    time: Res<Time>,
) {
    // Consume each event once, update diagnostics, and spawn its falling bar.
    let Ok(events) = stream.events.lock() else {
        return;
    };
    for event in events.try_iter() {
        debug.instrument = Some(event.instrument);
        debug.pitch_hz = event.pitch_hz;
        debug.lane = Some(event.lane);
        debug.strength = event.strength;
        debug.noise_floor = event.noise_floor;
        debug.duration_secs = event.duration_secs;
        if event.phase == NotePhase::Started {
            debug.event_count += 1;
            let x = -360.0 + event.lane as f32 * 180.0;
            commands.spawn((
                Sprite {
                    color: instrument_color(event.instrument, event.lane),
                    custom_size: Some(Vec2::new(108.0, 24.0)),
                    ..default()
                },
                Transform::from_xyz(x, 300.0 + event.strength * 10.0, 1.0),
                FallingNote {
                    lane: event.lane,
                    spawned_at: time.elapsed_secs(),
                },
                GameplayEntity,
            ));
        }
    }
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    *text = Text::new(debug_text(&debug));
}

/// Format the latest live input snapshot for display.
fn debug_text(debug: &DebugInputData) -> String {
    // Format the latest input snapshot for the diagnostic panel.
    let instrument = debug.instrument.map_or("NONE", instrument_name);
    let pitch = debug
        .pitch_hz
        .map_or_else(|| "--".into(), |pitch| format!("{pitch:>7.2} Hz"));
    let lane = debug
        .lane
        .map_or_else(|| "--".into(), |lane| (lane + 1).to_string());
    let bass = debug.instrument.zip(debug.pitch_hz).map_or_else(
        || "STRING  --\nNOTE    --".into(),
        |(instrument, pitch)| bass_debug_details(instrument, pitch),
    );
    format!(
        "DEBUG INPUT  [F3]\n\nINSTRUMENT  {instrument}\nEST PITCH   {pitch}\n{bass}\nLANE        {lane}\nSIGNAL      {:>5.1}%\nNOISE FLOOR {:>5.2}%\nDURATION    {:>5.2} s\nEVENTS      {}",
        debug.strength * 100.0,
        debug.noise_floor * 100.0,
        debug.duration_secs,
        debug.event_count,
    )
}

/// Format bass string, note, and cents details for the debug panel.
fn bass_debug_details(instrument: Instrument, pitch_hz: f32) -> String {
    // Report the nearest bass string, note name, and cents deviation.
    let targets = match instrument {
        Instrument::Bass5 => [
            (30.87, "B0", "String 5"),
            (41.20, "E1", "String 4"),
            (55.00, "A1", "String 3"),
            (73.42, "D2", "String 2"),
            (98.00, "G2", "String 1"),
        ],
        Instrument::Bass4 => [
            (41.20, "E1", "String 4"),
            (55.00, "A1", "String 3"),
            (73.42, "D2", "String 2"),
            (98.00, "G2", "String 1"),
            (98.00, "G2", "String 1"),
        ],
        _ => return "STRING      --\nNOTE        --".into(),
    };
    let (target, note, string) = targets
        .into_iter()
        .min_by(|(left, _, _), (right, _, _)| {
            (pitch_hz - left).abs().total_cmp(&(pitch_hz - right).abs())
        })
        .unwrap();
    let cents = 1200.0 * (pitch_hz / target).log2();
    format!("STRING      {string}\nNOTE        {note} ({cents:+.1} cents)")
}

/// Move falling bars toward the hit line.
fn move_notes(mut notes: Query<(&FallingNote, &mut Transform)>, time: Res<Time>) {
    // Advance every falling bar at a fixed visual speed.
    for (note, mut transform) in &mut notes {
        transform.translation.y = 300.0 - (time.elapsed_secs() - note.spawned_at) * NOTE_SPEED;
    }
}

/// Score keyboard hits and remove missed or successful bars.
fn hit_notes(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut notes: Query<(Entity, &FallingNote, &Transform)>,
    mut score: ResMut<Score>,
) {
    // Match keyboard lane presses against bars near the hit line.
    let keys = [
        KeyCode::KeyA,
        KeyCode::KeyS,
        KeyCode::KeyD,
        KeyCode::KeyF,
        KeyCode::KeyG,
    ];
    for (entity, note, transform) in &mut notes {
        if keyboard.just_pressed(keys[note.lane])
            && (transform.translation.y - HIT_LINE_Y).abs() < 55.0
        {
            commands.entity(entity).despawn();
            score.hits += 1;
            score.combo += 1;
            score.accuracy = (score.accuracy * (score.hits - 1) as f32 + 1.0) / score.hits as f32;
        }
        if transform.translation.y < -330.0 {
            commands.entity(entity).despawn();
            score.combo = 0;
        }
    }
}

/// Return the visual color for a lane index.
fn lane_color(lane: usize) -> Color {
    // Return the stable display color for a highway lane.
    match lane {
        0 => Color::srgb(0.95, 0.2, 0.25),
        1 => Color::srgb(0.95, 0.65, 0.18),
        2 => Color::srgb(0.25, 0.85, 0.45),
        3 => Color::srgb(0.25, 0.55, 1.0),
        _ => Color::srgb(0.8, 0.3, 0.95),
    }
}

/// Return the visual color for an instrument event.
fn instrument_color(instrument: Instrument, lane: usize) -> Color {
    // Tint spawned bars by instrument while preserving lane colors for guitar.
    match instrument {
        Instrument::Guitar => lane_color(lane),
        Instrument::Bass4 | Instrument::Bass5 => Color::srgb(0.95, 0.45, 0.15),
        Instrument::Drums => Color::srgb(0.25, 0.85, 0.45),
        Instrument::Vocals => Color::srgb(0.8, 0.3, 0.95),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AudioOnsetDetector, Instrument, NotePhase, PolyphonicAudioDetector, bass_string_lane,
        estimate_pitch, pitch_to_lane,
    };
    use std::f32::consts::TAU;
    use std::path::Path;

    #[test]
    /// Keeps the same pitch stable when its amplitude changes.
    fn pitch_estimation_is_stable_across_signal_strengths() {
        let sample_rate = 44_100.0;
        let pitches = [0.15, 0.9].map(|amplitude| {
            let samples = (0..4096)
                .map(|index| amplitude * (TAU * 55.0 * index as f32 / sample_rate).sin())
                .collect::<Vec<_>>();
            estimate_pitch(&samples, sample_rate).expect("a clear bass tone should be detected")
        });

        assert!(
            (pitches[0] - 55.0).abs() < 1.0,
            "quiet pitch: {}",
            pitches[0]
        );
        assert!(
            (pitches[1] - 55.0).abs() < 1.0,
            "loud pitch: {}",
            pitches[1]
        );
        assert!(
            (pitches[0] - pitches[1]).abs() < 0.5,
            "pitch drift: {pitches:?}"
        );
    }

    #[test]
    /// Confirms that each bass open string owns a distinct lane.
    fn pitch_estimation_detects_five_string_low_b() {
        let sample_rate = 44_100.0;
        let samples = (0..4096)
            .map(|index| (TAU * 30.87 * index as f32 / sample_rate).sin())
            .collect::<Vec<_>>();

        let pitch = estimate_pitch(&samples, sample_rate).expect("low B should be detected");
        assert!((pitch - 30.87).abs() < 0.75, "detected pitch: {pitch}");
    }

    #[test]
    /// Confirms four- and five-string open-note lane assignments.
    fn bass_open_strings_map_to_their_string_lanes() {
        let five_string_open_notes = [30.87, 41.20, 55.00, 73.42, 98.00];
        for (lane, pitch) in five_string_open_notes.into_iter().enumerate() {
            assert_eq!(bass_string_lane(Instrument::Bass5, pitch), lane);
        }

        let four_string_open_notes = [41.20, 55.00, 73.42, 98.00];
        for (lane, pitch) in four_string_open_notes.into_iter().enumerate() {
            assert_eq!(bass_string_lane(Instrument::Bass4, pitch), lane);
        }
    }

    #[test]
    /// Allows a quiet but clean bass pluck through the onset gate.
    fn quiet_bass_pluck_is_detected() {
        let sample_rate = 44_100.0;
        let samples = (0..4096)
            .map(|index| 0.03 * (TAU * 55.0 * index as f32 / sample_rate).sin())
            .collect::<Vec<_>>();
        let mut detector = AudioOnsetDetector {
            sample_rate,
            ..Default::default()
        };

        let result = detector.detect(samples.into_iter());

        assert!(
            result.is_some(),
            "quiet bass pluck should pass the onset gate"
        );
    }

    #[test]
    /// Tracks more than one pitched voice and reports its playing duration.
    fn polyphonic_detector_tracks_chord_duration() {
        let sample_rate = 44_100.0;
        let chord = |amplitude: f32| {
            (0..4096).map(move |index| {
                amplitude * (TAU * 110.0 * index as f32 / sample_rate).sin()
                    + amplitude * 0.8 * (TAU * 164.81 * index as f32 / sample_rate).sin()
            })
        };
        let mut detector = PolyphonicAudioDetector::new(sample_rate);
        let starts = detector.detect(chord(0.4));
        assert!(
            starts
                .iter()
                .filter(|event| event.phase == NotePhase::Started)
                .count()
                >= 2
        );

        let updates = detector.detect(chord(0.4).take(2048));
        assert!(
            updates
                .iter()
                .any(|event| { event.phase == NotePhase::Updated && event.duration_secs > 0.0 })
        );

        let mut silence = detector.detect((0..4096).map(|_| 0.0));
        silence.extend(detector.detect(std::iter::empty::<f32>()));
        assert!(
            silence
                .iter()
                .any(|event| { event.phase == NotePhase::Ended && event.duration_secs > 0.0 })
        );
    }

    fn detect_recording_pitches(path: &Path) -> Result<Vec<f32>, String> {
        let mut reader = hound::WavReader::open(path)
            .map_err(|error| format!("recording should open: {error}"))?;
        let spec = reader.spec();
        let mut detector = AudioOnsetDetector {
            sample_rate: spec.sample_rate as f32,
            ..Default::default()
        };
        let mut pitches = Vec::new();
        for sample in reader.samples::<i32>() {
            let sample =
                sample.map_err(|error| format!("recording samples should decode: {error}"))?;
            if let Some((_, pitch_hz, _)) =
                detector.detect(std::iter::once(sample as f32 / 8_388_608.0))
            {
                pitches.push(pitch_hz);
            }
        }
        Ok(pitches)
    }

    fn report_recording_errors(test_name: &str, errors: Vec<String>) {
        if !errors.is_empty() {
            panic!(
                "{test_name} found {} recording error(s):\n{}",
                errors.len(),
                errors.join("\n")
            );
        }
    }

    #[test]
    // #[ignore = "the supplied recordings currently expose pitch-detector false positives; run explicitly while tuning DSP"]
    /// Checks stable single-string detection and ordered multi-string detection.
    fn supplied_bass_recordings_detect_expected_open_strings() {
        let recording_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("recordings");
        let mut paths = std::fs::read_dir(&recording_directory)
            .expect("recordings directory should exist")
            .map(|entry| entry.expect("recording entry should be readable").path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "wav"))
            .collect::<Vec<_>>();
        paths.sort();

        let mut errors = Vec::new();
        for path in paths {
            let file_name = match path.file_name().and_then(|name| name.to_str()) {
                Some(file_name) => file_name,
                None => {
                    errors.push(format!(
                        "{}: recording filename should be valid UTF-8",
                        path.display()
                    ));
                    continue;
                }
            };
            let result = (|| -> Result<(), String> {
                let expected_strings = file_name
                    .strip_prefix("open-")
                    .and_then(|name| name.strip_suffix(".wav"))
                    .and_then(|name| name.split('-').next())
                    .ok_or_else(|| {
                        "recording should use the open-<strings>-<suffix>.wav format".to_string()
                    })?;
                let detected_pitches = detect_recording_pitches(&path)?;
                let mut detected_lanes = detected_pitches
                    .iter()
                    .map(|pitch| pitch_to_lane(Instrument::Bass5, *pitch))
                    .collect::<Vec<_>>();
                let expected_lanes = expected_strings
                    .chars()
                    .map(|string| match string {
                        'b' => Ok(0),
                        'e' => Ok(1),
                        'a' => Ok(2),
                        'd' => Ok(3),
                        'g' => Ok(4),
                        _ => Err(format!("unknown bass string '{string}'")),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if expected_lanes.len() == 1 {
                    if detected_lanes.len() < 2 {
                        return Err(format!(
                            "should contain multiple plucks; detected lanes={detected_lanes:?}"
                        ));
                    }
                    if !detected_lanes.iter().all(|lane| *lane == expected_lanes[0]) {
                        return Err(format!(
                            "changed lanes during a single-string recording: lanes={detected_lanes:?}, pitches={detected_pitches:?}"
                        ));
                    }
                } else {
                    detected_lanes.dedup();
                    if detected_lanes != expected_lanes {
                        return Err(format!(
                            "unexpected string order; expected={expected_lanes:?}, detected={detected_lanes:?}, pitches={detected_pitches:?}"
                        ));
                    }
                }
                Ok(())
            })();
            if let Err(error) = result {
                errors.push(format!("{file_name}: {error}"));
            }
        }
        report_recording_errors(
            "supplied_bass_recordings_detect_expected_open_strings",
            errors,
        );
    }

    #[test]
    #[ignore = "run explicitly as fret recordings are added and the detector is tuned"]
    /// Checks fret recordings for open string through fret 24 in order.
    fn supplied_bass_fret_recordings_detect_open_through_fret_24() {
        let recording_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("recordings");
        let mut paths = std::fs::read_dir(&recording_directory)
            .expect("recordings directory should exist")
            .map(|entry| entry.expect("recording entry should be readable").path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("fret-") && name.ends_with(".wav"))
            })
            .collect::<Vec<_>>();
        paths.sort();
        let mut errors = Vec::new();
        if paths.is_empty() {
            errors.push("expected at least one fret-<string>.wav recording".to_string());
        }

        for path in paths {
            let file_name = match path.file_name().and_then(|name| name.to_str()) {
                Some(file_name) => file_name,
                None => {
                    errors.push(format!(
                        "{}: recording filename should be valid UTF-8",
                        path.display()
                    ));
                    continue;
                }
            };
            let result = (|| -> Result<(), String> {
                let string = file_name
                    .strip_prefix("fret-")
                    .and_then(|name| name.strip_suffix(".wav"))
                    .and_then(|name| name.split('-').next())
                    .ok_or_else(|| {
                        "fret recording should use fret-<string>-<suffix>.wav format".to_string()
                    })?;
                let open_pitch = match string {
                    "b" => 30.87,
                    "e" => 41.20,
                    "a" => 55.00,
                    "d" => 73.42,
                    "g" => 98.00,
                    _ => return Err(format!("unknown bass string '{string}'")),
                };
                let expected_pitches = (0..=24)
                    .map(|fret| open_pitch * 2.0_f32.powf(fret as f32 / 12.0))
                    .collect::<Vec<_>>();
                let mut detected_frets = detect_recording_pitches(&path)?
                    .into_iter()
                    .map(|pitch| {
                        expected_pitches
                            .iter()
                            .enumerate()
                            .min_by(|(_, left), (_, right)| {
                                (pitch / *left)
                                    .ln()
                                    .abs()
                                    .total_cmp(&(pitch / *right).ln().abs())
                            })
                            .map(|(fret, _)| fret)
                            .ok_or_else(|| "expected fret list should not be empty".to_string())
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                detected_frets.dedup();
                let expected_frets = (0..=24).collect::<Vec<_>>();
                if detected_frets != expected_frets {
                    return Err(format!(
                        "unexpected fret sequence; expected={expected_frets:?}, detected={detected_frets:?}"
                    ));
                }
                Ok(())
            })();
            if let Err(error) = result {
                errors.push(format!("{file_name}: {error}"));
            }
        }
        report_recording_errors(
            "supplied_bass_fret_recordings_detect_open_through_fret_24",
            errors,
        );
    }
}
