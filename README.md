# Open band

Open band is an experimental Rust rhythm game built with Bevy. The goal is Rocksmith-style gameplay for real guitar and bass, with MIDI drums and microphone vocals.

The current project is an input and gameplay prototype. It proves the real-time instrument pipeline before adding songs, charts, richer scoring rules, and polished presentation.

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
- Adaptive low-level onset gating for quieter plucks.
- Pitch-to-lane mapping for guitar and vocals, plus physical-string mapping for bass.
- MIDI drum note mapping to gameplay lanes.
- Bass tuner with string, frequency, and cents-offset feedback.
- Timing calibration screen with a moving beat target before bass gameplay.
- Home screen with direct access to the live session and setup tools.
- Persistent setup settings stored in `open-band-settings/settings.json`.
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

To feed a WAV recording through the bass detector instead of a hardware input, set
`BAND_HERO_RECORDING` to a recording path:

```bash
BAND_HERO_RECORDING=recordings/open-gdeab.wav BAND_HERO_BASS_STRINGS=5 cargo run
```

The recording input uses the same onset and pitch detector as CPAL input. The supplied
filename convention is covered by an ignored diagnostic test while detector accuracy is
being tuned; run it with `cargo test supplied_bass_recordings_detect_expected_open_strings -- --ignored`.

The application opens on Home. Choose `Live Session` to start playing immediately with saved settings, or choose `Set Up` to configure devices and calibration. Input Setup can still be revisited at any time.

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
| `Enter` | Accept the latency result and return to Set Up |

### Home and Setup Navigation

The Home screen has two options: `Live Session` and `Set Up`. Choose with `Up` / `Down` or `1` / `2`, then press `Enter`.

Set Up contains the following options:

| Key | Destination |
| --- | --- |
| `1` | Input setup |
| `2` | Tuner |
| `3` | Latency calibration |
| `4` | Back to Home |

Each setup tool returns to Set Up when accepted or exited. Returning to Input Setup stops the current input worker before reconnecting the newly selected devices. Press `Esc` from Set Up to return Home, and press `Esc` from the live session to return Home.

## Saved Settings

When Input Setup is accepted, Open Band saves the selected device names and bass string mode. Accepting latency calibration saves the best measured offset. The settings file is created in the working directory at `open-band-settings/settings.json` and is loaded on the next launch. Environment variables remain supported as fallback defaults when no saved device matches.

The gameplay prototype also supports `A S D F G` as lane controls.

Press `F3` during the live session to show or hide the input debug window. For bass, it reports the estimated frequency, nearest string, note, and cents offset.
The panel also reports signal level, adaptive noise floor, lane, and event count. The noise-floor value is the detector's current estimate of the background input level.

## Input Device Configuration

Inputs can be selected from `Set Up` -> `Input Setup`. Environment variables are still supported as optional first-run defaults:

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

Saved device names are preferred on later launches. When no saved device is available, a matching environment variable is tried, followed by the first available device. MIDI requires an available MIDI input port. The worker thread reports unavailable devices in the terminal and continues with whichever inputs opened successfully.

The detector accepts quieter bass plucks than the original fixed-level gate. The bass should still be connected through an audio interface or preamp that presents a clean, non-clipped input signal; an inline amp is only needed if the hardware signal is still too weak or noisy at the interface input.

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

Audio callbacks stay small and perform level, onset, and pitch analysis. They send normalized `InstrumentEvent` values to Bevy through a channel. Bevy owns the UI, state transitions, falling notes, scoring, diagnostics, and gameplay timing. The input worker is started from saved settings at launch and is safely stopped and replaced when Input Setup is confirmed.

## Source Documentation

`src/main.rs` uses Rustdoc comments for application states, resources, components, and functions. Short inline comments identify the larger blocks inside systems, such as device enumeration, worker startup, pitch estimation, event consumption, and screen cleanup.

## Current Limitations

- Audio events are generated from detected playing onsets, not from a song chart.
- Pitch detection uses a normalized period search and is more stable across signal levels, but it can still produce errors with heavy noise, distortion, chords, harmonics, and vocals.
- Bass lanes now choose the nearest physical string by pitch (`B-E-A-D-G` for five-string and `E-A-D-G` for four-string), but pitch alone cannot distinguish the same note played on different frets and strings.
- Guitar events still use pitch bands rather than string and fret recognition.
- Strum direction is not detected.
- MIDI drum mapping is a small General MIDI-style mapping and is not configurable yet.
- Vocal gameplay does not yet compare pitch against lyric or melody targets.
- The calibration screen displays signal activity and persists the selected devices, bass mode, and accepted latency result, but it does not yet save gain, noise-floor, or pitch calibration values.
- The gameplay highway is still a visual and keyboard-test prototype.

## Likely Next Steps

### 1. Make calibration richer

- Measure and store each input's noise floor and gain.
- Add input latency measurement and compensation.
- Add a calibration profile for guitar, four-string bass, five-string bass, drums, and vocals.
- Persist gain, noise-floor, and pitch calibration profiles alongside the existing settings file.

### 2. Improve audio analysis

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

- Add profile management around the existing persistent settings file.
- Add audio playback, song loading, pause, restart, and practice mode.
- Add visual feedback for hit quality, combo, multiplier, and per-instrument score.
- Add an audio monitor toggle with feedback protection.
- Add diagnostics showing device sample rate, channel count, buffer size, and measured latency.

### 6. Test the real-time path

- Add more unit tests for pitch-to-lane mapping and MIDI drum mapping.
- Add deterministic tests for onset detection using recorded sample buffers.
- Record and replay audio input for regression testing.
- Test multiple sample formats, sample rates, buffer sizes, and disconnected devices.
- Test the Linux audio backend with the actual USB interface and microphone hardware.

## Development Checks

```bash
cargo check
cargo run
```

The repository has focused unit tests for pitch estimation and bass string lanes, but no automated gameplay test suite. Hardware validation requires connected audio and MIDI devices.
