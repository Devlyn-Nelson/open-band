use super::{
    AudioDetector, DetectorProfile, InputConfig, Instrument, InstrumentEvent, InstrumentKind,
    InstrumentSlot, Kit, LANES, NotePhase, RECORDING_ENVIRONMENT_VARIABLE, pitch_to_lane,
};
use bevy::prelude::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use midir::{Ignore, MidiInput};
use std::str::FromStr;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, SyncSender, sync_channel};
use std::thread;
use std::time::Duration;

#[derive(Resource)]
/// Shared event channel and lifetime handles for the input worker.
pub(crate) struct InstrumentStream {
    pub(crate) sender: Sender<InstrumentEvent>,
    pub(crate) events: Mutex<Receiver<InstrumentEvent>>,
    pub(crate) _thread: Option<thread::JoinHandle<()>>,
    pub(crate) stop_sender: Option<Sender<()>>,
    pub(crate) started: bool,
}

pub(crate) struct AudioInput {
    pub(crate) _stream: cpal::Stream,
    pub(crate) frames: Receiver<Vec<f32>>,
    pub(crate) detector: AudioDetector,
    pub(crate) instrument: Instrument,
    pub(crate) open_frequencies: Vec<f32>,
}

/// Open configured audio and MIDI inputs on a dedicated worker thread.
pub(crate) fn spawn_instrument_thread(
    sender: Sender<InstrumentEvent>,
    config: InputConfig,
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
            let mut stopping = false;
            let mut processed_frame = false;
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
                    send_detected_events(
                        &mut input.detector,
                        frame.into_iter(),
                        input.instrument,
                        &input.open_frequencies,
                        &sender,
                    );
                }
                if stopping {
                    break;
                }
            }
            if stopping {
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
fn open_audio_input(host: &cpal::Host, slot_index: usize, slot: &InstrumentSlot) -> Result<AudioInput, String> {
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
    let (frame_sender, frame_receiver) = sync_channel(32);

    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => {
            build_audio_stream::<f32>(&device, &config, channels, frame_sender.clone(), error_callback)
        }
        cpal::SampleFormat::I16 => {
            build_audio_stream::<i16>(&device, &config, channels, frame_sender.clone(), error_callback)
        }
        cpal::SampleFormat::U16 => {
            build_audio_stream::<u16>(&device, &config, channels, frame_sender, error_callback)
        }
        format => return Err(format!("unsupported sample format {format:?}")),
    }
    .map_err(|error| error.to_string())?;

    stream.play().map_err(|error| error.to_string())?;
    println!("Listening to slot {slot_index} ({:?}) on {device}", slot.kind);
    Ok(AudioInput {
        _stream: stream,
        frames: frame_receiver,
        detector: AudioDetector::new(slot.kind, &open_frequencies, slot.detector, config.sample_rate as f32),
        instrument,
        open_frequencies,
    })
}

/// Build a typed CPAL callback that converts samples into normalized events.
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
                &open_frequencies,
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
    open_frequencies: &[f32],
    sender: &Sender<InstrumentEvent>,
) {
    for detected in detector.detect(samples) {
        let _ = sender.send(InstrumentEvent {
            instrument,
            lane: pitch_to_lane(instrument, open_frequencies, detected.pitch_hz),
            strength: detected.strength,
            pitch_hz: Some(detected.pitch_hz),
            noise_floor: detected.noise_floor,
            phase: detected.phase,
            duration_secs: detected.duration_secs,
        });
    }
}

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
                    let lane = percussion_lane_for_trigger(&kit, message[1]);
                    let _ = sender.send(InstrumentEvent {
                        instrument,
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
    println!("Listening to slot {slot_index} (Percussion) on {name}");
    Some(connection)
}

/// Resolves a raw MIDI note to a gameplay lane using the kit's configured pieces instead
/// of a hardcoded note table. A `lane_span` piece (e.g. kick) uses a dedicated lane past
/// the kit's regular lanes; an unrecognized note falls back to the same dedicated lane.
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

