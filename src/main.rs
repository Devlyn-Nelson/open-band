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
    Calibration,
    Gameplay,
}

#[derive(Clone, Copy, Debug)]
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
}

#[derive(Resource)]
struct InstrumentStream {
    events: Mutex<Receiver<InstrumentEvent>>,
    _thread: thread::JoinHandle<()>,
}

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
}

#[derive(Component)]
struct CalibrationText;

#[derive(Component)]
struct CalibrationMeter;

#[derive(Component)]
struct FallingNote {
    lane: usize,
    spawned_at: f32,
}

fn main() {
    let (sender, receiver) = mpsc::channel();

    App::new()
        .insert_resource(ClearColor(Color::srgb(0.025, 0.035, 0.06)))
        .insert_resource(InstrumentStream {
            events: Mutex::new(receiver),
            _thread: spawn_instrument_thread(sender),
        })
        .insert_resource(Calibration {
            selected: Instrument::Guitar,
            level: 0.0,
            peak: 0.0,
            samples: 0,
        })
        .init_resource::<Score>()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Open band".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .init_state::<AppState>()
        .add_systems(OnEnter(AppState::Calibration), setup_calibration)
        .add_systems(Update, calibration_input.run_if(in_state(AppState::Calibration)))
        .add_systems(Update, calibration_display.run_if(in_state(AppState::Calibration)))
        .add_systems(OnExit(AppState::Calibration), cleanup_calibration)
        .add_systems(OnEnter(AppState::Gameplay), setup_gameplay)
        .add_systems(
            Update,
            (receive_instrument_events, move_notes, hit_notes).run_if(in_state(AppState::Gameplay)),
        )
        .run();
}

fn spawn_instrument_thread(sender: Sender<InstrumentEvent>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let host = cpal::default_host();
        let mut streams = Vec::new();
        let bass_instrument = match std::env::var("BAND_HERO_BASS_STRINGS").as_deref() {
            Ok("5") => Instrument::Bass5,
            _ => Instrument::Bass4,
        };

        for (instrument, variable) in [
            (Instrument::Guitar, "BAND_HERO_GUITAR_DEVICE"),
            (bass_instrument, "BAND_HERO_BASS_DEVICE"),
            (Instrument::Vocals, "BAND_HERO_VOCAL_DEVICE"),
        ] {
            match open_audio_input(&host, instrument, variable, sender.clone()) {
                Ok(stream) => streams.push(stream),
                Err(error) => eprintln!("{instrument:?} input unavailable: {error}"),
            }
        }

        let midi_connection = open_midi_input(sender.clone());
        loop {
            thread::sleep(Duration::from_millis(100));
            if streams.is_empty() && midi_connection.is_none() {
                eprintln!("No instrument inputs are active; configure device environment variables.");
                break;
            }
        }
    })
}

fn open_audio_input(
    host: &cpal::Host,
    instrument: Instrument,
    device_variable: &str,
    sender: Sender<InstrumentEvent>,
) -> Result<cpal::Stream, String> {
    let device = find_input_device(host, device_variable)?;
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
        if self.samples.len() < 2048 {
            return None;
        }

        let window = self.samples.drain(..1024).collect::<Vec<_>>();
        let level = (window.iter().map(|sample| sample * sample).sum::<f32>()
            / window.len() as f32)
            .sqrt();
        self.average = self.average * 0.96 + level * 0.04;
        let now = Instant::now();
        let ready = self.last_event.map_or(true, |event| now.duration_since(event) > Duration::from_millis(120));
        if level > 0.08 && level > self.average * 2.2 && ready {
            self.last_event = Some(now);
            let crossings = window
                .windows(2)
                .filter(|pair| pair[0] <= 0.0 && pair[1] > 0.0)
                .count();
            let pitch_hz = crossings as f32 * self.sample_rate / window.len() as f32;
            if (30.0..=1400.0).contains(&pitch_hz) {
                Some(((level * 4.0).clamp(0.15, 1.0), pitch_hz))
            } else {
                None
            }
        } else {
            None
        }
    }
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

fn find_input_device(host: &cpal::Host, variable: &str) -> Result<cpal::Device, String> {
    let requested = std::env::var(variable).ok();
    let devices = host.input_devices().map_err(|error| error.to_string())?;
    for device in devices {
        let name = device.to_string();
        if requested.as_ref().is_none_or(|wanted| name.contains(wanted)) {
            return Ok(device);
        }
    }
    Err(format!("no input device matched {variable}"))
}

fn open_midi_input(sender: Sender<InstrumentEvent>) -> Option<midir::MidiInputConnection<()>> {
    let mut input = MidiInput::new("open-band-drums").ok()?;
    input.ignore(Ignore::None);
    let requested = std::env::var("BAND_HERO_MIDI_DEVICE").ok();
    let port = input.ports().into_iter().find(|port| {
        let name = input.port_name(port).unwrap_or_default();
        requested.as_ref().is_none_or(|wanted| name.contains(wanted))
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
    commands.spawn(Camera2d);
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

fn calibration_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut calibration: ResMut<Calibration>,
    stream: Res<InstrumentStream>,
    time: Res<Time>,
) {
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
        }
    }
    if keyboard.just_pressed(KeyCode::Enter) {
        next_state.set(AppState::Gameplay);
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
    let status = if calibration.samples > 0 { "SIGNAL DETECTED" } else { "WAITING FOR INPUT" };
    *text = Text::new(format!(
        concat!(
            "OPEN BAND  //  INPUT CALIBRATION\n\n",
            "[1] GUITAR       [2] BASS 4-STRING\n",
            "[3] BASS 5-STRING [4] MIDI DRUMS\n",
            "[5] VOCALS\n\n",
            "ACTIVE: {}\nSOURCE: {}\n\n",
            "{}\nLEVEL  {:>3.0}%     PEAK  {:>3.0}%\n\n",
            "Play the selected instrument. Press ENTER when ready."
        ),
        instrument,
        source,
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
    entities: Query<Entity, Or<(With<CalibrationText>, With<CalibrationMeter>)>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
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
    let variable = match instrument {
        Instrument::Guitar => "BAND_HERO_GUITAR_DEVICE",
        Instrument::Bass4 | Instrument::Bass5 => "BAND_HERO_BASS_DEVICE",
        Instrument::Drums => "BAND_HERO_MIDI_DEVICE",
        Instrument::Vocals => "BAND_HERO_VOCAL_DEVICE",
    };
    std::env::var(variable).unwrap_or_else(|_| format!("auto-selected ({variable})"))
}

fn setup_gameplay(mut commands: Commands) {
    commands.spawn(Camera2d);
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
        ));
        commands.spawn((
            Sprite {
                color: lane_color(lane).with_alpha(0.28),
                custom_size: Some(Vec2::new(122.0, 5.0)),
                ..default()
            },
            Transform::from_xyz(x, HIT_LINE_Y, 0.0),
        ));
    }
}

fn receive_instrument_events(
    mut commands: Commands,
    stream: Res<InstrumentStream>,
    time: Res<Time>,
) {
    let Ok(events) = stream.events.lock() else {
        return;
    };
    for event in events.try_iter() {
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
        ));
    }
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
