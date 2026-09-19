# Open band

Open band is an experimental Rust rhythm game built with Bevy. The goal is Rocksmith-style gameplay for real guitar and bass, with MIDI drums and microphone vocals.

The current project is an early input and gameplay prototype. It is designed to prove the real-time instrument pipeline before adding songs, charts, scoring rules, and polished presentation.

## Current Features

- Bevy `0.20.0-rc.1` application and state management.
- Dedicated instrument worker thread separate from the Bevy gameplay thread.
- CPAL audio input for:
  - Electric guitar through a 1/4-inch-to-USB audio interface.
  - Electric bass through a 1/4-inch-to-USB audio interface.
  - Vocal microphones.
- Configurable four-string or five-string bass mode.
- MIDI input for electronic drums.
- Audio onset detection with lightweight zero-crossing pitch estimation.
- Pitch-to-lane mapping for guitar, bass, and vocals.
- MIDI drum note mapping to gameplay lanes.
- Calibration screen before gameplay.
- Basic falling-note highway and keyboard fallback controls.

## Running

Install the system audio and MIDI development libraries required by CPAL and `midir` for your Linux distribution. On Debian or Ubuntu, this commonly includes ALSA development packages:

```bash
sudo apt install libasound2-dev libudev-dev
```

Then run:

```bash
cargo run
```

The application opens on the calibration screen. Select an instrument with a number key, play or trigger the device, and press Enter to enter the gameplay prototype.

## Calibration Controls

| Key | Instrument |
| --- | --- |
| `1` | Guitar |
| `2` | Four-string bass |
| `3` | Five-string bass |
| `4` | MIDI drums |
| `5` | Vocals |
| `Enter` | Continue to gameplay |

The gameplay prototype also supports `A S D F G` as lane controls.

## Input Device Configuration

Inputs can be selected by setting environment variables to a substring of the device name:

```bash
BAND_HERO_GUITAR_DEVICE="USB Audio" \
BAND_HERO_BASS_DEVICE="USB Audio" \
BAND_HERO_BASS_STRINGS=5 \
BAND_HERO_VOCAL_DEVICE="Microphone" \
BAND_HERO_MIDI_DEVICE="Drum Controller" \
cargo run
```

Available variables:

- `BAND_HERO_GUITAR_DEVICE`: audio input for guitar.
- `BAND_HERO_BASS_DEVICE`: audio input for bass.
- `BAND_HERO_BASS_STRINGS`: use `4` or `5`; defaults to `4`.
- `BAND_HERO_VOCAL_DEVICE`: audio input for vocals.
- `BAND_HERO_MIDI_DEVICE`: MIDI input for drums.

If a device variable is not set, the first matching input device is selected. MIDI requires a matching MIDI input port. The worker thread reports unavailable devices in the terminal and continues with whichever inputs opened successfully.

## Architecture

```text
CPAL audio callbacks       MIDI callback
(guitar/bass/vocals)       (drums)
          |                    |
          +---- typed events -+
                    |
        dedicated instrument thread
                    |
          std::sync::mpsc channel
                    |
              Bevy main thread
       calibration, notes, scoring, rendering
```

Audio callbacks stay small and perform only basic level and pitch analysis. They send normalized `InstrumentEvent` values to Bevy through a channel. Bevy owns the UI, state transitions, falling notes, and gameplay timing.

## Current Limitations

- Audio events are generated from detected playing onsets, not from a song chart.
- Pitch detection currently uses zero crossings. It is useful for a prototype but will produce errors with noisy signals, distortion, chords, harmonics, and vocals.
- Guitar and bass events currently use pitch bands rather than string and fret recognition.
- Strum direction is not detected.
- MIDI drum mapping is a small General MIDI-style mapping and is not configurable yet.
- Vocal gameplay does not yet compare pitch against lyric or melody targets.
- The calibration screen displays signal activity but does not yet save gain, noise-floor, latency, or pitch calibration values.
- The gameplay highway is still a visual and keyboard-test prototype.

## Likely Next Steps

### 1. Make calibration real

- Add device enumeration instead of relying only on environment variables.
- Measure and store each input's noise floor and gain.
- Add input latency measurement and compensation.
- Add a calibration profile for guitar, four-string bass, five-string bass, drums, and vocals.
- Persist profiles to a configuration file.

### 2. Improve audio analysis

- Replace zero-crossing pitch estimation with YIN or autocorrelation.
- Add confidence scores and reject uncertain pitch estimates.
- Add separate tuning ranges for guitar, four-string bass, and five-string bass.
- Detect pick attacks and sustain separately.
- Add an optional noise gate and input gain controls.
- Consider a dedicated DSP crate or worker pool once analysis becomes more expensive.

### 3. Build chart-driven gameplay

- Define a song chart format containing note time, duration, lane, instrument, and difficulty.
- Add a song clock driven by audio playback rather than by event arrival.
- Match detected input against chart targets with configurable timing windows.
- Add sustain notes, chords, hammer-ons, pull-offs, slides, bends, and vocal phrases.
- Separate live instrument events from chart notes so events judge gameplay instead of creating notes spontaneously.

### 4. Add instrument-specific gameplay

- Guitar: map pitch and attack to strings/frets and support tuning calibration.
- Bass: support separate four-string and five-string ranges and tunings.
- Drums: make MIDI note mappings user-configurable and support velocity-sensitive scoring.
- Vocals: add lyric display, target melody curves, pitch confidence, and microphone monitoring.
- Add an optional controller or MIDI guitar path for strum direction and fret input.

### 5. Improve the player experience

- Add menus for device selection and calibration profiles.
- Add audio playback, song loading, pause, restart, and practice mode.
- Add visual feedback for hit quality, combo, multiplier, and per-instrument score.
- Add an audio monitor toggle with feedback protection.
- Add diagnostics showing device sample rate, channel count, buffer size, and measured latency.

### 6. Test the real-time path

- Add unit tests for pitch-to-lane mapping and MIDI drum mapping.
- Add deterministic tests for onset detection using recorded sample buffers.
- Record and replay audio input for regression testing.
- Test multiple sample formats, sample rates, buffer sizes, and disconnected devices.
- Test the Linux audio backend with the actual USB interface and microphone hardware.

## Development Checks

```bash
cargo check
cargo run
```

This repository currently has no automated gameplay test suite. Hardware validation requires connected audio and MIDI devices.
