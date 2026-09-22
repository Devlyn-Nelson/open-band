# Open band

Open band is an experimental Rust rhythm game built with Bevy. The goal is Rocksmith-style gameplay for real guitar and bass, with MIDI drums and microphone vocals.

The current project is an input and gameplay prototype. It proves the real-time instrument pipeline before adding songs, charts, richer scoring rules, and polished presentation.

## Current Features

- Bevy `0.20.0-rc.1` application and state management.
- Dedicated instrument worker thread separate from the Bevy gameplay thread.
- CPAL audio input for any number of configured `Strings`/`Voice` instrument slots:
  - Electric guitar or bass through a 1/4-inch-to-USB audio interface.
  - Vocal microphones.
- Any number of `Strings` slots, each with a configured tuning (loaded from the
  `tunings/` library) and a detector profile
  (polyphonic for chords, per-string for bass-style physical strings).
- MIDI input for electronic drums, with any number of `Percussion` slots.
- Audio onset detection with normalized YIN-style pitch estimation.
- Polyphonic FFT pitch tracking, with up to six simultaneous notes.
- Monophonic YIN-style tracking retained for vocals, with note start, sustain, and release events.
- Note-duration events report elapsed playing time in seconds for tracked notes.
- Adaptive low-level onset gating for quieter plucks.
- Confidence-gated pitch estimation: low-confidence YIN reads are rejected instead of reported as notes.
- A dedicated per-string energy tracker for open-string navigation (menu/song selection), independent from the general pitch detector, so a fresh pluck registers even while another string is still ringing. This tracker is menu-navigation only; chart gameplay and fretted notes still use the general YIN detector.
- Pitch-to-lane mapping for strings and vocals.
- MIDI drum note mapping to gameplay lanes.
- String tuner with string, frequency, and cents-offset feedback.
- Timing calibration screen with a moving beat target before chart gameplay.
- Home screen with direct access to the live session and setup tools.
- Persistent setup settings stored in `open-band-settings/settings.json`.
- Toggleable live-session input debug window with pitch, string, note, lane, and signal data.
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

To feed a WAV recording through the detector instead of a hardware input, set
`BAND_HERO_RECORDING` to a recording path (fed through the first configured `Strings`
slot, or a 4-string default if none is configured):

```bash
BAND_HERO_RECORDING=recordings/open-gdeab.wav BAND_HERO_BASS_STRINGS=5 cargo run
```

The recording input uses the same onset and pitch detector as CPAL input. The supplied
filename convention is covered by an ignored diagnostic test while detector accuracy is
being tuned; run it with `cargo test supplied_bass_recordings_detect_expected_open_strings -- --ignored`.

Recording test filenames use these conventions:

- `open-<strings>-<suffix>.wav`: open strings, in the order named. A suffix may be a version number or `no-mute`.
- `fret-<string>-<suffix>.wav`: the open string followed by one pluck at each fret from 1 through 24. The suffix is optional.

The fret corpus test can be run with `cargo test supplied_bass_fret_recordings_detect_open_through_fret_24 -- --ignored`.

The application opens on Home. Choose `Live Session` to start playing immediately with saved settings, or choose `Set Up` to configure devices and calibration. Input Setup can still be revisited at any time.

## Controls

### Device Selection

Input Setup shows a dynamic list of instrument slots; any number of slots of any kind
(`Strings`, `Percussion`, `Voice`) can be configured, and multiple slots of the same kind
are supported (e.g. two `Strings` slots for guitar and bass).

| Key | Action |
| --- | --- |
| `Up` / `Down` | Move focus between slots |
| `Left` / `Right` | Cycle the focused slot's device (audio devices for `Strings`/`Voice`, MIDI ports for `Percussion`) |
| `N` | Add a new slot (defaults to `Strings`) |
| `X` | Remove the focused slot |
| `K` | Cycle the focused slot's kind (`Strings` -> `Percussion` -> `Voice`) |
| `[` / `]` | Cycle the focused `Strings` slot's tuning (from `tunings/`) or `Percussion` slot's kit (from `kits/`) |
| `P` | Toggle the focused `Strings` slot's detector profile (polyphonic vs. per-string) |
| `Enter` | Continue to calibration |

### Calibration

Calibration lists the currently configured slots; press the number key matching a slot
(`1`-`9`, in configured order) to select it for calibration.

| Key | Action |
| --- | --- |
| `1`-`9` | Select a configured instrument slot |
| `Enter` | Accept calibration and return to Set Up |

### Bass Latency Calibration

| Key | Action |
| --- | --- |
| `Space` | Tap with the moving beat line to measure timing offset |
| `Enter` | Accept the latency result and return to Set Up |

### Home and Setup Navigation

The Home screen has four options: `Live Session`, `Songs`, `Editor`, and `Set Up`. Choose with `Up` / `Down` or `1`-`4`, then press `Enter`.

Songs opens the chart browser. Select a chart with `Up` / `Down` and press `Enter`; a five-second count-in starts the chart-driven string highway. Press `Esc` to return to the Songs menu.

Set Up contains the following options:

| Key | Destination |
| --- | --- |
| `1` | Input setup |
| `2` | Tuner |
| `3` | Latency calibration |
| `4` | Back to Home |

Each setup tool returns to Set Up when accepted or exited. Returning to Input Setup stops the current input worker before reconnecting the newly selected devices. Press `Esc` from Set Up to return Home, and press `Esc` from the live session to return Home.

### Chart Editor

The Editor opens a chart picker listing every chart in `charts/`.

| Key | Action |
| --- | --- |
| `Up` / `Down` | Select a chart |
| `Enter` | Open the selected chart for editing |
| `N` | Create a new, empty chart |
| `Esc` | Back to Home |

Once a chart is open, its track list is shown:

| Key | Action |
| --- | --- |
| `Up` / `Down` | Focus a track |
| `N` | Add a new track (defaults to `Strings`) |
| `X` | Remove the focused track |
| `K` | Cycle the focused track's kind (`Strings` -> `Percussion` -> `Voice`) |
| `[` / `]` | Cycle the focused track's tuning (`Strings`, from `tunings/`) or kit (`Percussion`, from `kits/`) |
| `,` / `.` | Move the focused track up/down in the list |
| `R` | Rename the focused track (type the new name, `Enter` to confirm, `Esc` to cancel) |
| `S` | Save the chart (writes to its original file, or `charts/<slugified-title>.json` for a new chart) |
| `Esc` | Back to the chart picker |

Saving runs validation (unplayable notes, missing tuning/kit, unknown percussion pieces) and reports any warnings in the status line without blocking the save. Note placement (sheet/tab views) is not yet implemented — the editor currently manages chart metadata and tracks only.

## Saved Settings

When Input Setup is accepted, Open Band saves the full list of configured instrument
slots (kind, device, tuning, detector profile). Audio settings use CPAL device IDs
such as `alsa:...`; MIDI settings use a deterministic `midi:<index>` port key. The setup
screen shows both the friendly device name and concrete identifier. The settings file is
created in the working directory at `open-band-settings/settings.json` and is loaded on
the next launch. Environment variables remain supported as first-run defaults for the
starter slot list when no settings file exists yet.

The gameplay prototype also supports `A S D F G` as lane controls.

## Chart Format

Charts are versioned JSON songs with a shared timeline and one or more instrument tracks.
At startup, Open Band loads every `*.json` file in the working directory's `charts/`
folder in sorted filename order. Invalid files are reported and skipped. If no charts can
be loaded, the embedded starter chart is used as a fallback. The included starter chart is
`charts/open-strings.json`:

```json
{
  "version": 1,
  "title": "Open Strings Study",
  "resolution": 960,
  "tempo_map": [{ "start": 0, "bpm": 90.0 }],
  "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
  "tracks": [
    {
      "name": "Bass",
      "kind": "strings",
      "tuning": { "strings": ["B0", "E1", "A1", "D2", "G2"] },
      "notes": [
        { "start": 1440, "length": 8, "dots": 1, "note": 23, "ps": 0 }
      ]
    }
  ]
}
```

Event positions and durations are integer **ticks** against the chart's `resolution`
(ticks per quarter note), not raw beats — this supports exact tempo/time-signature
changes mid-song via `tempo_map`/`time_signature_map` (each keyed by `start` tick, with a
tick-0 entry expected). A note's notated duration is `length` (`1`/`2`/`4`/`8`/`16` for
whole/half/quarter/eighth/sixteenth) plus optional `dots`; `tied: true` continues the
duration into the next event of the same pitch/piece.

Each track declares a `kind`: `strings`, `percussion`, or `voice`. `strings` tracks embed
a `tuning` (ordered, low-string-first, octave-qualified open-string notes, e.g. `["B0",
"E1", "A1", "D2", "G2"]`) covering any string count or tuning. `percussion` tracks embed a
`kit` (a lane count plus named pieces, each on a `lane` or spanning all lanes via
`lane_span`); percussion notes reference a piece by name (`"piece": "kick"`), optionally
with `dynamics` (`accent`/`ghost`) or `roll` (`single_lane`/`double_lane`). Pitched events
use `note` — either a raw MIDI number or a readable name like `"C4"` — and an optional
`ps` (preferred string) hint. Any track can carry `star_power_phrases`, simple
`{ start, duration_ticks }` range markers.

Songs can contain any number of `strings`, `percussion`, and `voice` tracks. `voice`
tracks use lyric phrases with nested pitch targets, allowing lyrics and melody to share
the same timeline. The current fret/note label mode is configured by `CHART_NOTE_DISPLAY`
in `src/domain.rs` and supports `Fret`, `Note`, or `Both`.

Press `F3` during the live session to show or hide the input debug window. For strings
inputs, it reports the estimated frequency, nearest string, pitch, and cents offset.
The panel also reports signal level, adaptive noise floor, lane, and event count. The
noise-floor value is the detector's current estimate of the background input level.

## Input Device Configuration

Inputs are configured from `Set Up` -> `Input Setup` as a dynamic list of instrument
slots (see Device Selection controls above). Environment variables preselect devices for
the four starter slots (`Strings` x2, `Percussion`, `Voice`) shown the first time Input
Setup runs, before any settings have been saved:

```bash
BAND_HERO_GUITAR_DEVICE="USB Audio" \
BAND_HERO_BASS_DEVICE="USB Audio" \
BAND_HERO_BASS_STRINGS=5 \
BAND_HERO_VOCAL_DEVICE="Microphone" \
BAND_HERO_MIDI_DEVICE="Drum Controller" \
cargo run
```

Available variables:

- `BAND_HERO_GUITAR_DEVICE`: audio input for the first (guitar-like, polyphonic) `Strings` slot.
- `BAND_HERO_BASS_DEVICE`: audio input for the second (bass-like, per-string) `Strings` slot.
- `BAND_HERO_BASS_STRINGS`: string count for that slot; use `4` or `5`, defaults to `4`.
- `BAND_HERO_VOCAL_DEVICE`: audio input for the `Voice` slot.
- `BAND_HERO_MIDI_DEVICE`: MIDI input for the `Percussion` slot.

These only apply to the starter slots on first run; slots added or removed afterward, and
all slots once a settings file exists, are configured entirely from the Input Setup screen.
Saved device identifiers are preferred on later launches. MIDI requires an available MIDI
input port. The worker thread reports unavailable devices in the terminal and continues
with whichever inputs opened successfully.

## Tuning Library

`Strings` slots select a tuning (used for real per-string lane assignment, the live
per-string tuner, and menu navigation) from the `tunings/` directory: one JSON file per
tuning, each shaped like:

```json
{ "name": "5-String Bass (BEADG)", "strings": ["B0", "E1", "A1", "D2", "G2"] }
```

`strings` is ordered low string first, with octave-qualified note names. At startup, Open
Band loads every `*.json` file in the working directory's `tunings/` folder in sorted
filename order; invalid files are reported and skipped. If the directory is missing or
empty, four embedded defaults are used (4-string bass, 5-string bass, standard guitar,
7-string guitar). Selecting a tuning in Input Setup resolves and copies it into the saved
`InstrumentSlot`, so a saved settings file never depends on the `tunings/` directory still
existing or being unchanged. Add a new tuning by dropping another `*.json` file (any
filename) into `tunings/`.

The detector accepts quieter bass plucks than the original fixed-level gate. The bass should still be connected through an audio interface or preamp that presents a clean, non-clipped input signal; an inline amp is only needed if the hardware signal is still too weak or noisy at the interface input.

## Kit Library

`Percussion` slots select a kit (used for real MIDI-note-to-lane mapping instead of a
hardcoded table) from the `kits/` directory: one JSON file per kit, shaped like:

```json
{
  "name": "Standard Rock Kit",
  "lanes": 4,
  "pieces": [
    { "name": "kick", "trigger": "midi:36", "lane_span": "yellow" },
    { "name": "snare", "trigger": "midi:38", "lane": 0, "symbol": "tom" },
    { "name": "tom1", "trigger": "midi:50", "lane": 1, "symbol": "tom" },
    { "name": "hihat", "trigger": "midi:42", "lane": 1, "symbol": "hihat" }
  ]
}
```

Each piece has a `trigger` (`midi:<note>`, the raw MIDI note number that fires it) and
either a `lane` (0-based, shared with other pieces on the same lane and distinguished by
`symbol`) or a `lane_span` color (a piece that isn't pinned to one lane, e.g. a kick). At
startup, Open Band loads every `*.json` file in the working directory's `kits/` folder in
sorted filename order; invalid files are reported and skipped. If the directory is
missing or empty, an embedded "Standard Rock Kit" (kick, snare, 3 toms, hi-hat, crash,
ride across 4 lanes) is used. Selecting a kit in Input Setup resolves and copies it into
the saved `InstrumentSlot`, so a saved settings file never depends on the `kits/`
directory still existing or being unchanged. Add a new kit by dropping another `*.json`
file (any filename) into `kits/`.

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

Audio callbacks stay real-time safe: they downmix samples and enqueue bounded audio blocks without running DSP or blocking. The dedicated instrument worker owns onset, YIN, and FFT analysis, then sends normalized `InstrumentEvent` values to Bevy through a channel. Bevy owns the UI, state transitions, falling notes, scoring, diagnostics, and gameplay timing. The input worker is started from saved settings at launch and is safely stopped and replaced when Input Setup is confirmed.

## Source Documentation

`src/main.rs` is the crate entry point. `src/app.rs` owns Bevy app construction and system registration. `src/state.rs` owns top-level application states. `src/domain.rs` owns chart data, chart screen state, and chart UI markers. `src/audio.rs` owns pure pitch estimation and pitch-to-lane mapping. `src/detector.rs` owns stateful audio detectors. `src/input.rs` owns worker lifecycle and device callbacks. `src/settings.rs` owns persisted settings and device enumeration. `src/setup.rs` owns device selection and calibration. `src/navigation.rs` owns instrument-driven navigation. `src/chart.rs` owns chart screens. `src/menu.rs` owns Home and Set Up menus. `src/gameplay.rs` owns live-session rendering, diagnostics, note motion, and scoring. `AGENTS.md` is the concise contributor map with ownership boundaries and validation commands.

## Current Limitations

- Audio events are generated from detected playing onsets, not from a song chart.
- Pitch detection uses a normalized period search and is more stable across signal levels, but it can still produce errors with heavy noise, distortion, chords, harmonics, and vocals.
- Bass lanes now choose the nearest physical string by pitch (`B-E-A-D-G` for five-string and `E-A-D-G` for four-string), but pitch alone cannot distinguish the same note played on different frets and strings.
- The per-string open-note detector that improves bass menu navigation has not been implemented for chart gameplay; fretted notes during gameplay still rely on the general YIN pitch detector.
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

- Extend confidence-gated pitch estimation (already used for bass) to guitar and vocals.
- Extend the per-string energy tracker used for bass open-string navigation to fretted notes and chart gameplay.
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
