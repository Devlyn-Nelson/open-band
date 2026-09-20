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
- Audio onset detection with normalized YIN-style pitch estimation.
- Pitch-to-lane mapping for guitar, bass, and vocals.
- MIDI drum note mapping to gameplay lanes.
- Bass tuner with string, frequency, and cents-offset feedback.
- Timing calibration screen with a moving beat target before bass gameplay.
- Session menu for returning to input devices, calibration, latency calibration, or gameplay.
- Toggleable live-session input debug window with pitch, bass string, note, lane, and signal data.
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

The application opens with an input device dialog. Select an instrument row with `1`-`4`, choose its device with Left/Right, and press `B` to switch between four- and five-string bass. Press Enter to continue. Bass calibration opens a tuner first; after accepting it, a timing screen measures tap offset before entering the gameplay prototype.

## Controls

### Device Selection

| Key | Action |
| --- | --- |
| `1` | Select guitar device |
| `2` | Select bass device |
| `3` | Select MIDI drum device |
| `4` | Select vocal device |
| `Left` / `Right` | Cycle through available devices |
| `B` | Toggle four- or five-string bass |
| `Enter` | Continue to calibration |

### Calibration

| Key | Instrument |
| --- | --- |
| `1` | Guitar |
| `2` | Four-string bass |
| `3` | Five-string bass |
| `4` | MIDI drums |
| `5` | Vocals |
| `Enter` | Accept calibration; bass continues to latency calibration |

### Bass Latency Calibration

| Key | Action |
| --- | --- |
| `Space` | Tap with the moving beat line to measure timing offset |
| `Enter` | Accept the latency result and open the live session |

### Session Menu

Press `Esc` from calibration, latency calibration, or the live session to open the session menu. Choose a destination with `Up` / `Down` or `1`-`4`, then press `Enter`:

| Key | Destination |
| --- | --- |
| `1` | Input devices |
| `2` | Bass tuner / calibration |
| `3` | Latency calibration |
| `4` | Live session |

Returning to Input Devices stops the current input worker before reconnecting the newly selected devices.

The gameplay prototype also supports `A S D F G` as lane controls.

Press `F3` during the live session to show or hide the input debug window. For bass, it reports the estimated frequency, nearest string, note, and cents offset.

## Input Device Configuration

Inputs can be selected in the startup dialog. Environment variables are still supported as optional defaults:

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

When a device variable is set, the matching device is preselected in the dialog. Without one, the first available device of that type is preselected. MIDI requires an available MIDI input port. The worker thread reports unavailable devices in the terminal and continues with whichever inputs opened successfully.

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
- Pitch detection uses a normalized period search and is more stable across signal levels, but it can still produce errors with heavy noise, distortion, chords, harmonics, and vocals.
- Guitar and bass events currently use pitch bands rather than string and fret recognition.
- Strum direction is not detected.
- MIDI drum mapping is a small General MIDI-style mapping and is not configurable yet.
- Vocal gameplay does not yet compare pitch against lyric or melody targets.
- The calibration screen displays signal activity but does not yet save gain, noise-floor, latency, or pitch calibration values.
- The gameplay highway is still a visual and keyboard-test prototype.

## Likely Next Steps

### 1. Make calibration real

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

- Persist device selections and calibration profiles to a configuration file.
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
