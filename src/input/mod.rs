pub(crate) const RECORDING_ENVIRONMENT_VARIABLE: &str = "BAND_HERO_RECORDING";

use crate::audio::NoteData;
use crate::{
    AudioDetector, Instrument, InstrumentEvent, InstrumentKind, InstrumentSlot, Kit, LANES,
    NotePhase,
};

/// Runtime device configuration consumed by the input worker.
pub(crate) struct InputConfig {
    pub(crate) slots: Vec<InstrumentSlot>,
}
use bevy::prelude::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use midir::{Ignore, MidiInput};
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, SyncSender, sync_channel};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Resource)]
/// Shared event channel and lifetime handles for the input worker.
///
/// AI_CODE
pub(crate) struct InstrumentStream {
    pub(crate) sender: Sender<InstrumentEvent>,
    pub(crate) events: Mutex<Receiver<InstrumentEvent>>,
    pub(crate) input_levels: InputLevels,
    pub(crate) recording_commands: Sender<RecordingCommand>,
    pub(crate) recording_status: RecordingStatusHandle,
    pub(crate) _thread: Option<thread::JoinHandle<()>>,
    pub(crate) stop_sender: Option<Sender<()>>,
    pub(crate) started: bool,
}

/// Commands sent from the recorder screen to the input worker.
///
/// AI_CODE
pub(crate) enum RecordingCommand {
    Start(Vec<usize>),
    Stop,
}

/// Current recorder state displayed by the recorder screen.
///
/// AI_CODE
#[derive(Clone, Debug)]
pub(crate) enum RecordingStatus {
    Inactive(Option<String>),
    Starting,
    Active(String),
    Stopping(Option<String>),
    Error(String),
}

/// Thread-safe recorder status shared between UI and input worker.
///
/// AI_CODE
#[derive(Clone)]
pub(crate) struct RecordingStatusHandle(Arc<Mutex<RecordingStatus>>);

impl RecordingStatusHandle {
    /// AI_CODE
    pub(crate) fn new() -> Self {
        Self(Arc::new(Mutex::new(RecordingStatus::Inactive(None))))
    }

    /// AI_CODE
    pub(crate) fn get(&self) -> RecordingStatus {
        self.0
            .lock()
            .map(|status| status.clone())
            .unwrap_or_else(|_| RecordingStatus::Error("Recorder status unavailable".into()))
    }

    /// AI_CODE
    pub(crate) fn set(&self, status: RecordingStatus) {
        if let Ok(mut current) = self.0.lock() {
            *current = status;
        }
    }
}

/// Latest raw audio level and frame timestamp for each input slot.
///
/// AI_CODE
#[derive(Clone)]
pub(crate) struct InputLevels(Arc<Mutex<Vec<Option<InputLevelReading>>>>);

/// AI_CODE
struct InputLevelReading {
    rms: f32,
    updated_at: Instant,
}

impl InputLevels {
    /// AI_CODE
    pub(crate) fn new(slot_count: usize) -> Self {
        let mut levels = Vec::new();
        levels.resize_with(slot_count, || None);
        Self(Arc::new(Mutex::new(levels)))
    }

    /// AI_CODE
    pub(crate) fn resize(&self, slot_count: usize) {
        if let Ok(mut levels) = self.0.lock() {
            levels.resize_with(slot_count, || None);
        }
    }

    /// AI_CODE
    pub(crate) fn latest(&self, slot: usize) -> Option<(f32, bool)> {
        let levels = self.0.lock().ok()?;
        let reading = levels.get(slot)?.as_ref()?;
        Some((
            reading.rms,
            reading.updated_at.elapsed() < Duration::from_millis(500),
        ))
    }

    /// AI_CODE
    fn update(&self, slot: usize, rms: f32) {
        if let Ok(mut levels) = self.0.lock()
            && let Some(level) = levels.get_mut(slot)
        {
            *level = Some(InputLevelReading {
                rms,
                updated_at: Instant::now(),
            });
        }
    }
}

/// AI_CODE
pub(crate) struct AudioInput {
    pub(crate) _stream: cpal::Stream,
    pub(crate) frames: Receiver<Vec<f32>>,
    pub(crate) detector: AudioDetector,
    pub(crate) instrument: Instrument,
    pub(crate) open_frequencies: Vec<f32>,
    pub(crate) sample_rate: u32,
}

type WavRecorder = hound::WavWriter<BufWriter<File>>;

/// Open configured audio and MIDI inputs on a dedicated worker thread.
///
/// AI_CODE
pub(crate) fn spawn_instrument_thread(
    sender: Sender<InstrumentEvent>,
    config: InputConfig,
    input_levels: InputLevels,
    recording_commands: Receiver<RecordingCommand>,
    recording_status: RecordingStatusHandle,
    stop_receiver: Receiver<()>,
) -> thread::JoinHandle<()> {
    // Keep device ownership on a worker so real-time callbacks never block Bevy.
    thread::spawn(move || {
        if let Ok(path) = std::env::var(RECORDING_ENVIRONMENT_VARIABLE) {
            let slot = config
                .slots
                .iter()
                .position(|slot| slot.kind == InstrumentKind::Strings)
                .map(|index| (index, config.slots[index].clone()))
                .unwrap_or((0, InstrumentSlot::default_for(InstrumentKind::Strings)));
            run_recording_input(&path, slot.0, &slot.1, sender, stop_receiver);
            return;
        }

        let host = cpal::default_host();
        let mut streams = Vec::new();
        let mut midi_connections = Vec::new();
        let mut recorder: Option<(WavRecorder, String, Vec<usize>)> = None;

        for (index, slot) in config.slots.iter().enumerate() {
            match slot.kind {
                InstrumentKind::Strings | InstrumentKind::Voice => {
                    match open_audio_input(&host, index, slot) {
                        Ok(input) => streams.push(input),
                        Err(error) => {
                            eprintln!("slot {index} ({:?}) input unavailable: {error}", slot.kind)
                        }
                    }
                }
                InstrumentKind::Percussion => {
                    if let Some(connection) = open_midi_input(
                        sender.clone(),
                        index,
                        slot.kit_or_default(),
                        slot.device.as_deref(),
                    ) {
                        midi_connections.push(connection);
                    } else {
                        eprintln!("slot {index} (Percussion) MIDI input unavailable");
                    }
                }
            }
        }

        loop {
            while let Ok(command) = recording_commands.try_recv() {
                match command {
                    RecordingCommand::Start(armed_slots) => {
                        if recorder.is_none() {
                            let active_slots = armed_slots
                                .into_iter()
                                .filter(|slot| {
                                    streams.iter().any(|input| input.instrument.slot == *slot)
                                })
                                .collect::<Vec<_>>();
                            if active_slots.is_empty() {
                                recording_status.set(RecordingStatus::Error(
                                    "None of the armed audio inputs are active.".into(),
                                ));
                                continue;
                            }
                            match create_wav_recorder() {
                                Ok((writer, path)) => {
                                    recording_status.set(RecordingStatus::Active(path.clone()));
                                    recorder = Some((writer, path, active_slots));
                                }
                                Err(error) => {
                                    recording_status.set(RecordingStatus::Error(error));
                                }
                            }
                        }
                    }
                    RecordingCommand::Stop => {
                        if let Some((writer, path, _)) = recorder.take() {
                            match writer.finalize() {
                                Ok(()) => {
                                    recording_status.set(RecordingStatus::Inactive(Some(path)));
                                }
                                Err(error) => {
                                    recording_status.set(RecordingStatus::Error(error.to_string()));
                                }
                            }
                        }
                    }
                }
            }
            let mut stopping = false;
            let mut processed_frame = false;
            let mut recording_frames = Vec::new();
            let recording_active = recorder.is_some();
            for input in &mut streams {
                if stop_receiver.try_recv().is_ok() {
                    stopping = true;
                    break;
                }
                if let Ok(frame) = input.frames.try_recv() {
                    processed_frame = true;
                    if stop_receiver.try_recv().is_ok() {
                        stopping = true;
                        break;
                    }
                    let rms = (frame.iter().map(|sample| sample * sample).sum::<f32>()
                        / frame.len().max(1) as f32)
                        .sqrt();
                    input_levels.update(input.instrument.slot, rms);
                    if recording_active {
                        if recorder
                            .as_ref()
                            .is_some_and(|(_, _, armed)| armed.contains(&input.instrument.slot))
                        {
                            recording_frames.push((
                                input.instrument.slot,
                                input.sample_rate,
                                frame,
                            ));
                        }
                    } else {
                        send_detected_events(
                            &mut input.detector,
                            frame.into_iter(),
                            input.instrument,
                            &sender,
                        );
                    }
                }
                if stopping {
                    break;
                }
            }
            if let Some((writer, _, armed_slots)) = recorder.as_mut()
                && let Err(error) = write_mixed_frames(writer, &recording_frames, armed_slots)
            {
                let path = recorder.as_ref().map(|(_, path, _)| path.clone());
                recording_status.set(RecordingStatus::Error(format!(
                    "Could not write WAV file{}: {error}",
                    path.map_or_else(String::new, |path| format!(" {path}"))
                )));
                recorder = None;
            }
            if stopping {
                if let Some((writer, path, _)) = recorder.take() {
                    let _ = writer.finalize();
                    recording_status.set(RecordingStatus::Inactive(Some(path)));
                }
                break;
            }
            if !processed_frame
                && stop_receiver
                    .recv_timeout(Duration::from_millis(10))
                    .is_ok()
            {
                break;
            }
            if streams.is_empty() && midi_connections.is_empty() {
                eprintln!(
                    "No instrument inputs are active; choose available devices in the dialog."
                );
                break;
            }
        }
    })
}

/// Open one CPAL input and attach the onset/pitch callback.
///
/// AI_CODE
fn open_audio_input(
    host: &cpal::Host,
    slot_index: usize,
    slot: &InstrumentSlot,
) -> Result<AudioInput, String> {
    let open_frequencies = slot.open_frequencies();
    let instrument = Instrument {
        slot: slot_index,
        kind: slot.kind,
        strings: open_frequencies.len() as u8,
    };
    let device = find_input_device(host, slot.device.as_deref())?;
    let supported = device
        .default_input_config()
        .map_err(|error| error.to_string())?;
    let config = supported.config();
    let channels = config.channels as usize;
    let error_callback = |error| eprintln!("audio input error: {error}");
    let (frame_sender, frame_receiver) = sync_channel(256);

    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => build_audio_stream::<f32>(
            &device,
            &config,
            channels,
            frame_sender.clone(),
            error_callback,
        ),
        cpal::SampleFormat::I16 => build_audio_stream::<i16>(
            &device,
            &config,
            channels,
            frame_sender.clone(),
            error_callback,
        ),
        cpal::SampleFormat::I32 => build_audio_stream::<i32>(
            &device,
            &config,
            channels,
            frame_sender.clone(),
            error_callback,
        ),
        cpal::SampleFormat::U16 => {
            build_audio_stream::<u16>(&device, &config, channels, frame_sender, error_callback)
        }
        format => return Err(format!("unsupported sample format {format:?}")),
    }
    .map_err(|error| error.to_string())?;

    stream.play().map_err(|error| error.to_string())?;
    println!(
        "Listening to slot {slot_index} ({:?}) on {device}",
        slot.kind
    );
    Ok(AudioInput {
        _stream: stream,
        frames: frame_receiver,
        detector: AudioDetector::new(
            slot.kind,
            &open_frequencies,
            slot.detector,
            config.sample_rate as f32,
        ),
        instrument,
        open_frequencies,
        sample_rate: config.sample_rate,
    })
}

/// Create a mono 16-bit WAV recorder under the local `recordings` directory.
///
/// AI_CODE
fn create_wav_recorder() -> Result<(WavRecorder, String), String> {
    fs::create_dir_all("recordings").map_err(|error| error.to_string())?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let path = PathBuf::from("recordings").join(format!("open-band-{timestamp}.wav"));
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let writer = hound::WavWriter::create(&path, spec).map_err(|error| error.to_string())?;
    Ok((writer, path.to_string_lossy().into_owned()))
}

/// Resample and average the currently available armed input frames into the WAV mix.
///
/// AI_CODE
fn write_mixed_frames(
    writer: &mut WavRecorder,
    frames: &[(usize, u32, Vec<f32>)],
    armed_slots: &[usize],
) -> Result<(), hound::Error> {
    let frames = frames
        .iter()
        .filter(|(slot, _, _)| armed_slots.contains(slot))
        .collect::<Vec<_>>();
    if frames.is_empty() || armed_slots.is_empty() {
        return Ok(());
    }
    let resampled = frames
        .iter()
        .map(|(_, sample_rate, samples)| resample_mono(samples, *sample_rate, 44_100))
        .collect::<Vec<_>>();
    let output_len = resampled.iter().map(Vec::len).max().unwrap_or_default();
    for index in 0..output_len {
        let mixed = resampled
            .iter()
            .map(|samples| samples.get(index).copied().unwrap_or(0.0))
            .sum::<f32>()
            / armed_slots.len() as f32;
        writer.write_sample((mixed.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16)?;
    }
    Ok(())
}

/// Linearly resample one mono frame to the recording's fixed sample rate.
///
/// AI_CODE
fn resample_mono(samples: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if samples.is_empty() || source_rate == 0 || target_rate == 0 {
        return Vec::new();
    }
    let output_len = (samples.len() as u64 * target_rate as u64 / source_rate as u64) as usize;
    (0..output_len)
        .map(|index| {
            let source_position = index as f32 * source_rate as f32 / target_rate as f32;
            let left = source_position.floor() as usize;
            let right = (left + 1).min(samples.len() - 1);
            let fraction = source_position - left as f32;
            samples[left] * (1.0 - fraction) + samples[right] * fraction
        })
        .collect()
}

#[test]
/// Writes a mono WAV containing only the armed, averaged audio sources.
/// AI_CODE
fn mixed_recording_wav_averages_armed_audio_frames() {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "open-band-mix-test-{}-{timestamp}.wav",
        std::process::id()
    ));
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&path, spec).expect("create test WAV");
    write_mixed_frames(
        &mut writer,
        &[
            (0, 44_100, vec![0.4; 4]),
            (1, 44_100, vec![0.4; 4]),
            (2, 44_100, vec![0.9; 4]),
        ],
        &[0, 1],
    )
    .expect("write mixed samples");
    writer.finalize().expect("finalize test WAV");

    let mut reader = hound::WavReader::open(&path).expect("open test WAV");
    assert_eq!(reader.spec().channels, 1);
    assert_eq!(reader.spec().sample_rate, 44_100);
    let samples = reader
        .samples::<i16>()
        .collect::<Result<Vec<_>, _>>()
        .expect("read test samples");
    assert_eq!(samples.len(), 4);
    assert!(samples.iter().all(|sample| (*sample - 13_107).abs() <= 1));
    fs::remove_file(path).expect("remove test WAV");
}

/// Build a typed CPAL callback that converts samples into normalized events.
///
/// AI_CODE
fn build_audio_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    sender: SyncSender<Vec<f32>>,
    error_callback: impl FnMut(cpal::Error) + Send + 'static,
) -> Result<cpal::Stream, cpal::Error>
where
    T: cpal::Sample + cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    device.build_input_stream(
        *config,
        move |data: &[T], _| {
            let mono = data
                .chunks(channels)
                .map(|frame| {
                    frame
                        .iter()
                        .map(|sample| sample.to_sample::<f32>())
                        .sum::<f32>()
                        / frame.len().max(1) as f32
                })
                .collect::<Vec<_>>();
            let _ = sender.try_send(mono);
        },
        error_callback,
        None,
    )
}

/// Read a mono 24-bit WAV file and feed it through the same detector as live audio.
///
/// AI_CODE
fn run_recording_input(
    path: &str,
    slot_index: usize,
    slot: &InstrumentSlot,
    sender: Sender<InstrumentEvent>,
    stop_receiver: Receiver<()>,
) {
    let open_frequencies = slot.open_frequencies();
    let instrument = Instrument {
        slot: slot_index,
        kind: slot.kind,
        strings: open_frequencies.len() as u8,
    };
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let mut reader = hound::WavReader::open(path)?;
        let spec = reader.spec();
        let mut detector = AudioDetector::new(
            slot.kind,
            &open_frequencies,
            slot.detector,
            spec.sample_rate as f32,
        );
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
///
/// AI_CODE
fn send_detected_events(
    detector: &mut AudioDetector,
    samples: impl Iterator<Item = f32>,
    instrument: Instrument,
    sender: &Sender<InstrumentEvent>,
) {
    for detected in detector.detect(samples) {
        let _ = sender.send(InstrumentEvent {
            instrument,
            strength: detected.strength,
            note: NoteData::Pitch(detected.pitch_hz),
            noise_floor: detected.noise_floor,
            phase: detected.phase,
            duration_secs: detected.duration_secs,
        });
    }
}

/// AI_CODE
fn find_input_device(host: &cpal::Host, requested: Option<&str>) -> Result<cpal::Device, String> {
    if let Some(requested) = requested {
        if let Ok(id) = cpal::DeviceId::from_str(requested) {
            if let Some(device) = host.device_by_id(&id) {
                return Ok(device);
            }
        }
    }
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

/// AI_CODE
fn open_midi_input(
    sender: Sender<InstrumentEvent>,
    slot_index: usize,
    kit: Kit,
    requested: Option<&str>,
) -> Option<midir::MidiInputConnection<()>> {
    let mut input = MidiInput::new("open-band-percussion").ok()?;
    input.ignore(Ignore::None);
    let port = input
        .ports()
        .into_iter()
        .enumerate()
        .find(|(index, port)| {
            let name = input.port_name(port).unwrap_or_default();
            requested.is_none_or(|wanted| {
                wanted == format!("midi:{index}") || name == wanted || name.contains(wanted)
            })
        })?
        .1;
    let name = input.port_name(&port).unwrap_or_default();
    let instrument = Instrument {
        slot: slot_index,
        kind: InstrumentKind::Percussion,
        strings: 0,
    };
    let connection = input
        .connect(
            &port,
            "open-band-midi-input",
            move |_, message, _| {
                if message.len() >= 3 && message[0] & 0xf0 == 0x90 && message[2] > 0 {
                    let _ = sender.send(InstrumentEvent {
                        instrument,
                        strength: message[2] as f32 / 127.0,
                        note: NoteData::Midi(message[1]),
                        noise_floor: 0.0,
                        phase: NotePhase::Start,
                        duration_secs: 0.0,
                    });
                }
            },
            (),
        )
        .ok()?;
    println!("Listening to slot {slot_index} (Percussion) on {name}");
    Some(connection)
}

/// Resolves a raw MIDI note to a gameplay lane using the kit's configured pieces instead
/// of a hardcoded note table. A `lane_span` piece (e.g. kick) uses a dedicated lane past
/// the kit's regular lanes; an unrecognized note falls back to the same dedicated lane.
///
/// AI_CODE
fn percussion_lane_for_trigger(kit: &Kit, note: u8) -> usize {
    let trigger = format!("midi:{note}");
    let spanning_lane = kit.lanes.min(LANES - 1);
    kit.pieces
        .iter()
        .find(|piece| piece.trigger == trigger)
        .map_or(spanning_lane, |piece| {
            piece.lane.map_or(spanning_lane, |lane| lane.min(LANES - 1))
        })
}
