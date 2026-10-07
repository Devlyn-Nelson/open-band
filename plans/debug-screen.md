# Waterfall diagnostic screen — plan

Replaces the Live Session screen (`AppState::Gameplay`, `src/screens/live/`) as the main
diagnostic screen. Three vertical columns, one instrument slot at a time:

| Left (≈25%)              | Center (≈50%)                          | Right (≈25%)                       |
|--------------------------|----------------------------------------|------------------------------------|
| Raw detector internals   | Waterfall: log-pitch (x) × time (y),   | Detector output: active notes,     |
| (level, noise floor, YIN | newest row on top, open-string/note    | sustain duration, recent start/end |
| confidence, per-string   | name labels along the x-axis           | event log                          |
| energy ratios, …)        |                                        |                                    |

## Decisions (confirmed)
- Live Session is removed entirely; Waterfall takes Home menu slot `[1]`.
- One slot at a time; `Left`/`Right` cycles slots, `Esc` returns Home.
- Log/semitone pitch axis (~30–1400 Hz), labeled with note names + the slot's open strings.
- Worker computes diagnostics only while the Waterfall is open (shared `AtomicBool`).

## The core problem: data access
Bevy currently only sees `InstrumentEvent` (detector *output*). The waterfall and left
panel need the spectrum and detector *internals*, which exist only on the worker thread
inside `AudioDetector` (`src/audio/detector.rs`). So the worker must publish a second,
diagnostic stream. Keep the callback → worker → Bevy boundary: DSP stays on the worker.

## Phase 1 — Diagnostic data model (`src/audio/`)
- `src/audio/mod.rs`: add framework-independent types:
  - `SpectrumFrame { slot, sample_rate, bins: Vec<f32> }` — magnitudes resampled onto a
    fixed log-pitch grid (e.g. 1/3-semitone bins from 30 Hz to 1400 Hz ≈ 200 bins).
  - `DetectorSnapshot` enum per detector kind:
    - `Mono { level, average, noise_floor, yin_pitch_hz, yin_confidence, pending_pitch_hz, pending_count, active_pitch_hz }`
    - `Poly { level, noise_floor, peak_threshold, peaks: Vec<(hz, strength)>, tracks: Vec<(hz, strength, missed_windows)> }`
    - `Bass { mono: Mono…, level, per_string: Vec<(target_hz, energy, baseline, ratio)>, active_lane, pending_lane, pending_count }`
  - `DiagnosticFrame { slot, spectrum: SpectrumFrame, snapshot: DetectorSnapshot }`.
  - Pure helper `log_pitch_bins(magnitudes, sample_rate, fft_size) -> Vec<f32>` + `bin_to_hz` / `hz_to_bin`.
- `src/audio/detector.rs`:
  - Factor the windowed-FFT part of `spectral_peaks` into `magnitude_spectrum(window, sample_rate)`
    so the waterfall and polyphonic detector share one FFT implementation.
  - Have each detector record its last-window internals (cheap field copies) and expose
    `AudioDetector::diagnostics(&self) -> Option<DiagnosticFrame>` for the most recent 4096-sample
    window (all detectors already hop 2048 samples, so one frame per hop ≈ 21 fps @ 44.1 kHz).
  - Spectrum is computed only when requested (flag below), not on every hop.

## Phase 2 — Worker plumbing (`src/input/mod.rs`)
- Add to `InstrumentStream`: `diagnostics: Mutex<Receiver<DiagnosticFrame>>`,
  `diagnostics_enabled: Arc<AtomicBool>`; pass the sender + flag into `spawn_instrument_thread`.
- Use a bounded `sync_channel` + `try_send` for diagnostics so a slow/absent consumer
  never blocks or grows memory; dropping frames is acceptable.
- In the worker loop and `run_recording_input`: after `send_detected_events`, if the flag is
  set, push `detector.diagnostics()`. MIDI slots produce no spectrum (right column still
  shows their events).
- Recreate the channel/flag wherever the worker is restarted (Input Setup commit path in
  `src/screens/settings/mod.rs`).

## Phase 3 — Waterfall screen (`src/screens/waterfall/mod.rs`, new)
- `AppState::Gameplay` → `AppState::Waterfall` (`src/app/state.rs`), Home entry `[1] WATERFALL`.
- Resources (screen-owned):
  - `WaterfallView { slot: usize, history: VecDeque<Vec<f32>> (≈ 10 s of rows), last_snapshot, active_notes: HashMap<note key, ActiveNote{pitch_hz, started, duration, strength}>, event_log: VecDeque<String> }`.
  - `WaterfallImage(Handle<Image>)` — a `width = bin count`, `height = history rows` RGBA texture.
- Systems:
  - `setup_waterfall` (OnEnter): spawn a root `Node` row with three column `Node`s: left `Text`,
    center `ImageNode` (+ an absolute-positioned x-axis label row), right `Text`. Set
    `diagnostics_enabled = true`; drain stale frames.
  - `waterfall_input`: `NavigationMessage::Left/Right` cycle through configured slots
    (clearing history), `Back` → Home.
  - `receive_waterfall_data`: drain `InstrumentEvent`s (update active notes / sustain /
    end + event log; filter by selected slot) and `DiagnosticFrame`s (push rows, keep latest snapshot).
  - `paint_waterfall`: write history into the image, newest row at the top; map magnitude
    → color with a dB scale relative to the noise floor (dark blue → yellow → white).
    Rewrite only when new rows arrived.
  - `waterfall_text_display`: render left (snapshot) and right (active notes + sustain + recent log).
  - `cleanup_waterfall` (OnExit): despawn, `diagnostics_enabled = false`, drop resources.
- Axis labels: note names at each C plus the slot's open strings (via `slot.open_frequencies()`),
  highlighted with `lane_color`.
- Navigation: add `AppState::Waterfall` to `system_note_to_navigation`'s allowed states **only if**
  instrument navigation is wanted here. Caution: that system drains the shared event channel, which
  would compete with `receive_waterfall_data` — recommended to leave Waterfall out of that list.

## Phase 4 — Remove Live Session
- Move `lane_color` / `instrument_color` to a shared place (e.g. `src/screens/mod.rs` or a
  small `src/screens/palette.rs`) — they are used by the Tuner (`settings/mod.rs`) and Songs.
- Delete `src/screens/live/`, `Score`, `DebugInputData`, `FallingNote`, F3 debug window,
  `A S D F G` keyboard lanes, and their registrations in `src/app/mod.rs`.
- `bass_debug_details` logic (string / note name breakdown) moves into the waterfall left panel.

## Phase 5 — Tests, docs
- Unit tests (`src/tests.rs`):
  - log-pitch binning: a pure 110 Hz sine lands its peak in the A2 bin; bins are monotonic in Hz.
  - `magnitude_spectrum` refactor keeps `polyphonic_detector_tracks_chord_duration` passing.
  - `AudioDetector::diagnostics()` returns the right snapshot variant per profile, and a
    per-string `Bass` snapshot shows the plucked string with the highest ratio.
  - Active-note bookkeeping (Start → Sustain → End) as a pure function.
- `README.md`: replace the Live Session / F3 / A-S-D-F-G sections with the Waterfall controls and panels;
  fix Home menu description.
- `AGENTS.md`: replace `src/screens/live/mod.rs` entry with `src/screens/waterfall/mod.rs`; note the
  diagnostic channel in the Runtime Flow diagram.
- Validate: `cargo test`, then `cargo run` and check with a live instrument and with
  `BAND_HERO_RECORDING=recordings/<file>.wav`.

## Risks / open points
- FFT resolution: a 4096-sample window at 44.1 kHz gives ~10.8 Hz bins; below ~100 Hz
  that is wider than a semitone, so low bass rows will look smeared even with 4× zero-padding.
  Acceptable for diagnostics; a longer window could be offered later.
- Image updates: upload cost for a ~200×200 RGBA texture at ~21 fps is negligible.
- Bevy 0.20 RC API for `ImageNode`/`Image` asset mutation should be checked against the
  version in `Cargo.toml` while implementing.
