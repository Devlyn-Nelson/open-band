use bevy::prelude::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use midir::{Ignore, MidiInput};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

const LANES: usize = 5;
const HIT_LINE_Y: f32 = -250.0;
const NOTE_SPEED: f32 = 260.0;

#[derive(States, Default, Clone, Eq, PartialEq, Debug, Hash)]
enum AppState {
    #[default]
    Home,
    Setup,
    DeviceSelection,
    Calibration,
    LatencyCalibration,
    Gameplay,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Instrument {
    Guitar,
    Bass4,
    Bass5,
    Drums,
    Vocals,
}

#[derive(Clone, Copy, Debug)]
struct InstrumentEvent {
    instrument: Instrument,
    lane: usize,
    strength: f32,
    pitch_hz: Option<f32>,
}

#[derive(Resource)]
struct InstrumentStream {
    sender: Sender<InstrumentEvent>,
    events: Mutex<Receiver<InstrumentEvent>>,
    _thread: Option<thread::JoinHandle<()>>,
    stop_sender: Option<Sender<()>>,
    started: bool,
}

#[derive(Resource, Default)]
struct MenuSelection {
    home_selected: usize,
    setup_selected: usize,
}

#[derive(Component)]
struct MenuText;

#[derive(Component)]
struct MenuCamera;

#[derive(Resource)]
struct DeviceSelection {
    audio_devices: Vec<String>,
    midi_devices: Vec<String>,
    selected: [usize; 4],
    focus: usize,
    bass_strings: u8,
}

#[derive(Component)]
struct DeviceSelectionText;

#[derive(Component)]
struct DeviceSelectionCamera;

#[derive(Resource, Default)]
struct Score {
    hits: u32,
    combo: u32,
    accuracy: f32,
}

#[derive(Resource)]
struct Calibration {
    selected: Instrument,
    level: f32,
    peak: f32,
    samples: u32,
    last_pitch_hz: Option<f32>,
}

#[derive(Component)]
struct CalibrationText;

#[derive(Component)]
struct CalibrationMeter;

#[derive(Component)]
struct CalibrationCamera;

#[derive(Resource)]
struct LatencyCalibration {
    started_at: f32,
    best_ms: Option<f32>,
    attempts: u32,
}

#[derive(Component)]
struct LatencyText;

#[derive(Component)]
struct LatencyPulse;

#[derive(Component)]
struct LatencyCamera;

#[derive(Component)]
struct FallingNote {
    lane: usize,
    spawned_at: f32,
}

#[derive(Component)]
struct GameplayEntity;

#[derive(Resource, Default)]
struct DebugInputData {
    instrument: Option<Instrument>,
    pitch_hz: Option<f32>,
    lane: Option<usize>,
    strength: f32,
    event_count: u64,
}

#[derive(Component)]
struct DebugWindow;

#[derive(Component)]
struct DebugText;

fn main() {
    let (sender, receiver) = mpsc::channel();
    let device_selection = scan_devices();

    App::new()
        .insert_resource(ClearColor(Color::srgb(0.025, 0.035, 0.06)))
        .insert_resource(device_selection)
        .insert_resource(InstrumentStream {
            sender,
            events: Mutex::new(receiver),
            _thread: None,
            stop_sender: None,
            started: false,
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
        .add_systems(Update, device_selection_input.run_if(in_state(AppState::DeviceSelection)))
        .add_systems(Update, device_selection_display.run_if(in_state(AppState::DeviceSelection)))
        .add_systems(OnExit(AppState::DeviceSelection), cleanup_device_selection)
        .add_systems(OnEnter(AppState::Calibration), setup_calibration)
        .add_systems(Update, calibration_input.run_if(in_state(AppState::Calibration)))
        .add_systems(Update, calibration_display.run_if(in_state(AppState::Calibration)))
        .add_systems(OnExit(AppState::Calibration), cleanup_calibration)
        .add_systems(OnEnter(AppState::LatencyCalibration), setup_latency_calibration)
        .add_systems(Update, latency_calibration_input.run_if(in_state(AppState::LatencyCalibration)))
        .add_systems(Update, latency_calibration_display.run_if(in_state(AppState::LatencyCalibration)))
        .add_systems(OnExit(AppState::LatencyCalibration), cleanup_latency_calibration)
        .add_systems(OnEnter(AppState::Gameplay), setup_gameplay)
        .add_systems(
            Update,
            (receive_instrument_events, move_notes, hit_notes, gameplay_menu_input, debug_input)
                .run_if(in_state(AppState::Gameplay)),
        )
        .add_systems(OnExit(AppState::Gameplay), cleanup_gameplay)
        .run();
}

struct InputConfig {
    audio_devices: [Option<String>; 3],
    midi_device: Option<String>,
    bass_strings: u8,
}

fn scan_devices() -> DeviceSelection {
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
                .map(|port| input.port_name(port).unwrap_or_else(|_| "Unknown MIDI device".into()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    DeviceSelection {
        selected: [
            selected_device_index(&audio_devices, "BAND_HERO_GUITAR_DEVICE"),
            selected_device_index(&audio_devices, "BAND_HERO_BASS_DEVICE"),
            selected_device_index(&midi_devices, "BAND_HERO_MIDI_DEVICE"),
            selected_device_index(&audio_devices, "BAND_HERO_VOCAL_DEVICE"),
        ],
        focus: 0,
        bass_strings: if std::env::var("BAND_HERO_BASS_STRINGS").as_deref() == Ok("5") { 5 } else { 4 },
        audio_devices,
        midi_devices,
    }
}

fn setup_device_selection(mut commands: Commands) {
    commands.spawn((Camera2d, DeviceSelectionCamera));
    commands.spawn((
        Text::new(""),
        TextFont { font_size: FontSize::Px(25.0), ..default() },
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

fn device_selection_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<DeviceSelection>,
    mut stream: ResMut<InstrumentStream>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let focus_keys = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4];
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
        let audio_name = |index: usize| selection.audio_devices.get(selection.selected[index]).cloned();
        let midi_name = selection.midi_devices.get(selection.selected[2]).cloned();
        let config = InputConfig {
            audio_devices: [audio_name(0), audio_name(1), audio_name(3)],
            midi_device: midi_name,
            bass_strings: selection.bass_strings,
        };
        if let Some(stop_sender) = stream.stop_sender.take() {
            let _ = stop_sender.send(());
            if let Some(thread) = stream._thread.take() {
                let _ = thread.join();
            }
        }
        let (stop_sender, stop_receiver) = mpsc::channel();
        stream._thread = Some(spawn_instrument_thread(stream.sender.clone(), config, stop_receiver));
        stream.stop_sender = Some(stop_sender);
        stream.started = true;
        next_state.set(AppState::Setup);
    }
}

fn device_selection_display(
    selection: Res<DeviceSelection>,
    mut text: Query<&mut Text, With<DeviceSelectionText>>,
) {
    let Ok(mut text) = text.single_mut() else { return };
    let device_name = |devices: &[String], selected: usize| {
        devices.get(selected).cloned().unwrap_or_else(|| "NO DEVICE FOUND".into())
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
        marker(0), device_name(&selection.audio_devices, selection.selected[0]),
        marker(1), selection.bass_strings, device_name(&selection.audio_devices, selection.selected[1]),
        marker(2), device_name(&selection.midi_devices, selection.selected[2]),
        marker(3), device_name(&selection.audio_devices, selection.selected[3]),
    ));
}

fn cleanup_device_selection(
    mut commands: Commands,
    entities: Query<Entity, Or<(With<DeviceSelectionText>, With<DeviceSelectionCamera>)>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

fn setup_home(mut commands: Commands) {
    commands.spawn((Camera2d, MenuCamera));
    commands.spawn((
        Text::new(""),
        TextFont { font_size: FontSize::Px(30.0), ..default() },
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

fn setup_setup(mut commands: Commands) {
    commands.spawn((Camera2d, MenuCamera));
    commands.spawn((
        Text::new(""),
        TextFont { font_size: FontSize::Px(30.0), ..default() },
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

fn home_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<MenuSelection>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keyboard.just_pressed(KeyCode::ArrowUp) {
        menu.home_selected = menu.home_selected.checked_sub(1).unwrap_or(1);
    }
    if keyboard.just_pressed(KeyCode::ArrowDown) {
        menu.home_selected = (menu.home_selected + 1) % 2;
    }
    for (index, key) in [KeyCode::Digit1, KeyCode::Digit2]
        .into_iter()
        .enumerate()
    {
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

fn home_display(
    menu: Res<MenuSelection>,
    mut text: Query<&mut Text, With<MenuText>>,
) {
    let Ok(mut text) = text.single_mut() else { return };
    let marker = |index: usize| if menu.home_selected == index { ">" } else { " " };
    *text = Text::new(format!(
        "OPEN BAND  //  HOME\n\n\
        {} [1] LIVE SESSION\n\
        {} [2] SET UP\n\n\
        Up/Down: navigate     Enter: open",
        marker(0), marker(1),
    ));
}

fn setup_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<MenuSelection>,
    mut next_state: ResMut<NextState<AppState>>,
) {
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
    for (index, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4]
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

fn setup_display(
    menu: Res<MenuSelection>,
    mut text: Query<&mut Text, With<MenuText>>,
) {
    let Ok(mut text) = text.single_mut() else { return };
    let marker = |index: usize| if menu.setup_selected == index { ">" } else { " " };
    *text = Text::new(format!(
        "OPEN BAND  //  SET UP\n\n\
        {} [1] INPUT SETUP\n\
        {} [2] TUNER\n\
        {} [3] LATENCY CALIBRATION\n\
        {} [4] BACK\n\n\
        Up/Down: navigate     Enter: open     Esc: home",
        marker(0), marker(1), marker(2), marker(3),
    ));
}

fn cleanup_menu(
    mut commands: Commands,
    entities: Query<Entity, Or<(With<MenuText>, With<MenuCamera>)>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

fn selected_device_index(devices: &[String], variable: &str) -> usize {
    std::env::var(variable)
        .ok()
        .and_then(|wanted| devices.iter().position(|device| device.contains(&wanted)))
        .unwrap_or(0)
}

fn spawn_instrument_thread(
    sender: Sender<InstrumentEvent>,
    config: InputConfig,
    stop_receiver: Receiver<()>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let host = cpal::default_host();
        let mut streams = Vec::new();
        let bass_instrument = if config.bass_strings == 5 { Instrument::Bass5 } else { Instrument::Bass4 };

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
            if stop_receiver.recv_timeout(Duration::from_millis(100)).is_ok() {
                break;
            }
            if streams.is_empty() && midi_connection.is_none() {
                eprintln!("No instrument inputs are active; choose available devices in the dialog.");
                break;
            }
        }
    })
}

fn open_audio_input(
    host: &cpal::Host,
    instrument: Instrument,
    device_name: Option<&str>,
    sender: Sender<InstrumentEvent>,
) -> Result<cpal::Stream, String> {
    let device = find_input_device(host, device_name)?;
    let supported = device.default_input_config().map_err(|error| error.to_string())?;
    let config = supported.config();
    let channels = config.channels as usize;
    let error_callback = |error| eprintln!("audio input error: {error}");

    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => build_audio_stream::<f32>(&device, &config, channels, instrument, sender, error_callback),
        cpal::SampleFormat::I16 => build_audio_stream::<i16>(&device, &config, channels, instrument, sender, error_callback),
        cpal::SampleFormat::U16 => build_audio_stream::<u16>(&device, &config, channels, instrument, sender, error_callback),
        format => return Err(format!("unsupported sample format {format:?}")),
    }
    .map_err(|error| error.to_string())?;

    stream.play().map_err(|error| error.to_string())?;
    println!("Listening to {instrument:?} on {device}");
    Ok(stream)
}

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
    let mut detector = AudioOnsetDetector::default();
    detector.sample_rate = config.sample_rate as f32;
    device.build_input_stream(
        *config,
        move |data: &[T], _| {
            let mono = data.chunks(channels).map(|frame| {
                frame.iter().map(|sample| sample.to_sample::<f32>()).sum::<f32>()
                    / frame.len().max(1) as f32
            });
            if let Some((strength, pitch_hz)) = detector.detect(mono) {
                let lane = pitch_to_lane(instrument, pitch_hz);
                let _ = sender.send(InstrumentEvent {
                    instrument,
                    lane,
                    strength,
                    pitch_hz: Some(pitch_hz),
                });
            }
        },
        error_callback,
        None,
    )
}

#[derive(Default)]
struct AudioOnsetDetector {
    average: f32,
    last_event: Option<Instant>,
    sample_rate: f32,
    samples: Vec<f32>,
}

impl AudioOnsetDetector {
    fn detect(&mut self, samples: impl Iterator<Item = f32>) -> Option<(f32, f32)> {
        self.samples.extend(samples);
        if self.samples.len() < 4096 {
            return None;
        }

        let window = self.samples.drain(..2048).collect::<Vec<_>>();
        let level = (window.iter().map(|sample| sample * sample).sum::<f32>()
            / window.len() as f32)
            .sqrt();
        self.average = self.average * 0.96 + level * 0.04;
        let now = Instant::now();
        let ready = self.last_event.map_or(true, |event| now.duration_since(event) > Duration::from_millis(120));
        if level > 0.08 && level > self.average * 2.2 && ready {
            self.last_event = Some(now);
            if let Some(pitch_hz) = estimate_pitch(&window, self.sample_rate) {
                Some(((level * 4.0).clamp(0.15, 1.0), pitch_hz))
            } else {
                None
            }
        } else {
            None
        }
    }
}

fn estimate_pitch(samples: &[f32], sample_rate: f32) -> Option<f32> {
    if samples.len() < 256 || sample_rate <= 0.0 {
        return None;
    }

    let mean = samples.iter().sum::<f32>() / samples.len() as f32;
    let centered = samples.iter().map(|sample| sample - mean).collect::<Vec<_>>();
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
            && (lag == max_lag || normalized <= difference[lag + 1] * (lag + 1) as f32
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

fn pitch_to_lane(instrument: Instrument, pitch_hz: f32) -> usize {
    let (low, high, lane_count) = match instrument {
        Instrument::Guitar => (82.0, 988.0, LANES),
        Instrument::Bass4 => (41.0, 392.0, 4),
        Instrument::Bass5 => (31.0, 392.0, LANES),
        Instrument::Vocals => (80.0, 1200.0, LANES),
        Instrument::Drums => return 0,
    };
    let normalized = ((pitch_hz / low).ln() / (high / low).ln()).clamp(0.0, 0.999);
    (normalized * lane_count as f32) as usize
}

fn find_input_device(host: &cpal::Host, requested: Option<&str>) -> Result<cpal::Device, String> {
    let devices = host.input_devices().map_err(|error| error.to_string())?;
    for device in devices {
        let name = device.to_string();
        if requested.is_none_or(|wanted| name == wanted) {
            return Ok(device);
        }
    }
    Err(requested.map_or_else(|| "no input device is available".into(), |name| format!("no input device matched {name}")))
}

fn open_midi_input(sender: Sender<InstrumentEvent>, requested: Option<&str>) -> Option<midir::MidiInputConnection<()>> {
    let mut input = MidiInput::new("open-band-drums").ok()?;
    input.ignore(Ignore::None);
    let port = input.ports().into_iter().find(|port| {
        let name = input.port_name(port).unwrap_or_default();
        requested.is_none_or(|wanted| name == wanted)
    })?;
    let name = input.port_name(&port).unwrap_or_default();
    let connection = input.connect(
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
                });
            }
        },
        (),
    )
    .ok()?;
    println!("Listening to MIDI drums on {name}");
    Some(connection)
}

fn setup_calibration(mut commands: Commands) {
    commands.spawn((Camera2d, CalibrationCamera));
    commands.spawn((
        Text::new("OPEN BAND  //  INPUT CALIBRATION"),
        TextFont { font_size: FontSize::Px(34.0), ..default() },
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

fn setup_latency_calibration(mut commands: Commands, time: Res<Time>) {
    commands.insert_resource(LatencyCalibration {
        started_at: time.elapsed_secs(),
        best_ms: None,
        attempts: 0,
    });
    commands.spawn((Camera2d, LatencyCamera));
    commands.spawn((
        Text::new(""),
        TextFont { font_size: FontSize::Px(27.0), ..default() },
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

fn latency_calibration_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut latency: ResMut<LatencyCalibration>,
    mut next_state: ResMut<NextState<AppState>>,
    time: Res<Time>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Setup);
        return;
    }
    if keyboard.just_pressed(KeyCode::Space) {
        let beat = (time.elapsed_secs() - latency.started_at) % 0.5;
        let offset = if beat > 0.25 { beat - 0.5 } else { beat };
        let offset_ms = offset * 1000.0;
        latency.best_ms = Some(latency.best_ms.map_or(offset_ms.abs(), |best| best.min(offset_ms.abs())));
        latency.attempts += 1;
    }
    if latency.attempts > 0 && keyboard.just_pressed(KeyCode::Enter) {
        next_state.set(AppState::Setup);
    }
}

fn latency_calibration_display(
    latency: Res<LatencyCalibration>,
    time: Res<Time>,
    mut text: Query<&mut Text, With<LatencyText>>,
    mut pulse: Query<&mut Transform, With<LatencyPulse>>,
) {
    let Ok(mut text) = text.single_mut() else { return };
    let elapsed = time.elapsed_secs() - latency.started_at;
    let phase = (elapsed % 0.5) / 0.5;
    if let Ok(mut transform) = pulse.single_mut() {
        transform.translation.x = -360.0 + phase * 720.0;
    }
    let result = latency.best_ms.map_or("NO TAP RECORDED".into(), |ms| format!("BEST OFFSET  {ms:>5.1} ms"));
    *text = Text::new(format!(
        "OPEN BAND  //  LATENCY CALIBRATION\n\n\
        LIVE SESSION CHECK\n\n\
        Tap SPACE as the yellow beat line crosses the center marker.\n\
        Attempts: {}\n{}\n\n\
        Press ENTER to accept and return to Set Up.",
        latency.attempts, result,
    ));
}

fn cleanup_latency_calibration(
    mut commands: Commands,
    entities: Query<Entity, Or<(With<LatencyText>, With<LatencyPulse>, With<LatencyCamera>)>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

fn calibration_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut calibration: ResMut<Calibration>,
    stream: Res<InstrumentStream>,
    time: Res<Time>,
) {
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

fn calibration_display(
    calibration: Res<Calibration>,
    mut text: Query<&mut Text, With<CalibrationText>>,
    mut meter: Query<&mut Transform, With<CalibrationMeter>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let instrument = instrument_name(calibration.selected);
    let source = input_source(calibration.selected);
    let tuner = bass_tuner_reading(calibration.selected, calibration.last_pitch_hz);
    let status = if calibration.samples > 0 { "SIGNAL DETECTED" } else { "WAITING FOR INPUT" };
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

fn cleanup_calibration(
    mut commands: Commands,
    entities: Query<Entity, Or<(With<CalibrationText>, With<CalibrationMeter>, With<CalibrationCamera>)>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

fn bass_tuner_reading(instrument: Instrument, pitch_hz: Option<f32>) -> String {
    if !matches!(instrument, Instrument::Bass4 | Instrument::Bass5) {
        return "TUNER: select a bass input with [2] or [3]".into();
    }
    let Some(pitch_hz) = pitch_hz else {
        return "BASS TUNER\nPlay an open string to begin tuning.".into();
    };
    let targets = if matches!(instrument, Instrument::Bass5) {
        [(30.87, "B"), (41.20, "E"), (55.00, "A"), (73.42, "D"), (98.00, "G")]
    } else {
        [(41.20, "E"), (55.00, "A"), (73.42, "D"), (98.00, "G"), (98.00, "G")]
    };
    let (target, note) = targets
        .into_iter()
        .min_by(|(left, _), (right, _)| (pitch_hz - left).abs().total_cmp(&(pitch_hz - right).abs()))
        .unwrap();
    let cents = 1200.0 * (pitch_hz / target).log2();
    let verdict = if cents.abs() < 5.0 { "IN TUNE" } else if cents < 0.0 { "TUNE UP" } else { "TUNE DOWN" };
    format!("BASS TUNER\nNOTE  {note}\nPITCH  {pitch_hz:>6.2} Hz\nOFFSET {cents:>+6.1} cents   {verdict}")
}

fn instrument_name(instrument: Instrument) -> &'static str {
    match instrument {
        Instrument::Guitar => "GUITAR",
        Instrument::Bass4 => "BASS 4-STRING",
        Instrument::Bass5 => "BASS 5-STRING",
        Instrument::Drums => "MIDI DRUMS",
        Instrument::Vocals => "VOCALS",
    }
}

fn input_source(instrument: Instrument) -> String {
    let _ = instrument;
    "selected in device dialog".into()
}

fn setup_gameplay(mut commands: Commands, debug: Res<DebugInputData>) {
    commands.spawn((Camera2d, GameplayEntity));
    commands.spawn((
        Text::new("OPEN BAND  //  LIVE SESSION"),
        TextFont { font_size: FontSize::Px(28.0), ..default() },
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
        TextFont { font_size: FontSize::Px(15.0), ..default() },
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
        TextFont { font_size: FontSize::Px(16.0), ..default() },
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

fn cleanup_gameplay(mut commands: Commands, entities: Query<Entity, With<GameplayEntity>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

fn gameplay_menu_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Home);
    }
}

fn debug_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut visibility: Query<&mut Visibility, With<DebugWindow>>,
) {
    if !keyboard.just_pressed(KeyCode::F3) {
        return;
    }
    let Ok(mut visibility) = visibility.single_mut() else { return };
    *visibility = match *visibility {
        Visibility::Visible => Visibility::Hidden,
        Visibility::Hidden | Visibility::Inherited => Visibility::Visible,
    };
}

fn receive_instrument_events(
    mut commands: Commands,
    stream: Res<InstrumentStream>,
    mut debug: ResMut<DebugInputData>,
    mut text: Query<&mut Text, With<DebugText>>,
    time: Res<Time>,
) {
    let Ok(events) = stream.events.lock() else {
        return;
    };
    for event in events.try_iter() {
        debug.instrument = Some(event.instrument);
        debug.pitch_hz = event.pitch_hz;
        debug.lane = Some(event.lane);
        debug.strength = event.strength;
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
    let Ok(mut text) = text.single_mut() else { return };
    *text = Text::new(debug_text(&debug));
}

fn debug_text(debug: &DebugInputData) -> String {
    let instrument = debug.instrument.map_or("NONE", instrument_name);
    let pitch = debug.pitch_hz.map_or_else(|| "--".into(), |pitch| format!("{pitch:>7.2} Hz"));
    let lane = debug.lane.map_or_else(|| "--".into(), |lane| (lane + 1).to_string());
    let bass = debug
        .instrument
        .zip(debug.pitch_hz)
        .map_or_else(|| "STRING  --\nNOTE    --".into(), |(instrument, pitch)| bass_debug_details(instrument, pitch));
    format!(
        "DEBUG INPUT  [F3]\n\nINSTRUMENT  {instrument}\nEST PITCH   {pitch}\n{bass}\nLANE        {lane}\nSIGNAL      {:>5.1}%\nEVENTS      {}",
        debug.strength * 100.0,
        debug.event_count,
    )
}

fn bass_debug_details(instrument: Instrument, pitch_hz: f32) -> String {
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
        .min_by(|(left, _, _), (right, _, _)| (pitch_hz - left).abs().total_cmp(&(pitch_hz - right).abs()))
        .unwrap();
    let cents = 1200.0 * (pitch_hz / target).log2();
    format!("STRING      {string}\nNOTE        {note} ({cents:+.1} cents)")
}

fn move_notes(mut notes: Query<(&FallingNote, &mut Transform)>, time: Res<Time>) {
    for (note, mut transform) in &mut notes {
        transform.translation.y = 300.0 - (time.elapsed_secs() - note.spawned_at) * NOTE_SPEED;
    }
}

fn hit_notes(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut notes: Query<(Entity, &FallingNote, &Transform)>,
    mut score: ResMut<Score>,
) {
    let keys = [KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyD, KeyCode::KeyF, KeyCode::KeyG];
    for (entity, note, transform) in &mut notes {
        if keyboard.just_pressed(keys[note.lane]) && (transform.translation.y - HIT_LINE_Y).abs() < 55.0 {
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

fn lane_color(lane: usize) -> Color {
    match lane {
        0 => Color::srgb(0.95, 0.2, 0.25),
        1 => Color::srgb(0.95, 0.65, 0.18),
        2 => Color::srgb(0.25, 0.85, 0.45),
        3 => Color::srgb(0.25, 0.55, 1.0),
        _ => Color::srgb(0.8, 0.3, 0.95),
    }
}

fn instrument_color(instrument: Instrument, lane: usize) -> Color {
    match instrument {
        Instrument::Guitar => lane_color(lane),
        Instrument::Bass4 | Instrument::Bass5 => Color::srgb(0.95, 0.45, 0.15),
        Instrument::Drums => Color::srgb(0.25, 0.85, 0.45),
        Instrument::Vocals => Color::srgb(0.8, 0.3, 0.95),
    }
}

#[cfg(test)]
mod tests {
    use super::estimate_pitch;
    use std::f32::consts::TAU;

    #[test]
    fn pitch_estimation_is_stable_across_signal_strengths() {
        let sample_rate = 44_100.0;
        let pitches = [0.15, 0.9].map(|amplitude| {
            let samples = (0..4096)
                .map(|index| amplitude * (TAU * 55.0 * index as f32 / sample_rate).sin())
                .collect::<Vec<_>>();
            estimate_pitch(&samples, sample_rate).expect("a clear bass tone should be detected")
        });

        assert!((pitches[0] - 55.0).abs() < 1.0, "quiet pitch: {}", pitches[0]);
        assert!((pitches[1] - 55.0).abs() < 1.0, "loud pitch: {}", pitches[1]);
        assert!((pitches[0] - pitches[1]).abs() < 0.5, "pitch drift: {pitches:?}");
    }

    #[test]
    fn pitch_estimation_detects_five_string_low_b() {
        let sample_rate = 44_100.0;
        let samples = (0..4096)
            .map(|index| (TAU * 30.87 * index as f32 / sample_rate).sin())
            .collect::<Vec<_>>();

        let pitch = estimate_pitch(&samples, sample_rate).expect("low B should be detected");
        assert!((pitch - 30.87).abs() < 0.75, "detected pitch: {pitch}");
    }
}
