# Agent Guide

## Start Here

- `src/main.rs`: minimal process entry point; delegates to `open_band::run()`.
- `src/lib.rs`: crate root, module declarations, and public application entry point.
- `src/app/mod.rs`: Bevy app construction, resource initialization, and system registration.
- `src/app/state.rs`: top-level Bevy application states.
- `src/model/mod.rs`: framework-independent chart, instrument, tuning, and percussion data.
- `src/audio/mod.rs`: pure pitch estimation and pitch-to-lane mapping; no device or Bevy lifecycle code.
- `src/audio/detector.rs`: stateful audio detectors and spectral analysis.
- `src/input/mod.rs`: worker-owned event channel and CPAL/MIDI/recording input lifecycle.
- `src/settings/mod.rs`: persisted settings, device configuration, and input slot models.
- `src/screens/setup/mod.rs`: device selection, calibration, tuner, and latency screens.
- `src/screens/navigation.rs`: instrument-driven navigation and latency tap routing.
- `src/screens/songs/mod.rs`: song menu, chart countdown/gameplay/review systems and chart-owned UI state.
- `src/screens/home.rs`: Home screen systems and components.
- `src/screens/live/mod.rs`: live-session rendering, event consumption, diagnostics, note motion, and scoring.
- `src/screens/editor/mod.rs`: in-app chart editor (chart browse/create/load, track list add/remove/rename/reorder, tuning/kit cycling, save).
- `src/notation/mod.rs`: framework-agnostic sheet-music layout (measures, ties, beaming, rest inference, clef selection); consumed by the chart editor's future sheet view and the future live gameplay notation overlay.
- `charts/`: built-in chart data loaded at compile time.
- `tunings/`: named string tunings loaded at runtime for Input Setup and the chart editor (embedded fallback if missing).
- `kits/`: named percussion kits loaded at runtime for Input Setup and the chart editor (embedded fallback if missing).
- `recordings/`: optional WAV fixtures used only by ignored detector tests.
- `README.md`: user-facing controls, device setup, chart format, and current limitations.

Read only the owning section first. The executable is intentionally split so chart changes do not require loading the input pipeline, and vice versa.

## Runtime Flow

```text
CPAL/MIDI callbacks -> instrument worker -> InstrumentEvent channel -> Bevy systems -> calibration/gameplay UI
chart JSON -> model/content data -> song countdown/gameplay/review systems
```

The worker performs detection off the Bevy thread. Bevy owns state transitions, rendering, scoring, and event consumption.

## Ownership Boundaries

- Chart schema and instrument data belong in `src/model/mod.rs`; screen resources and marker components stay with their owning screen.
- Top-level screen identifiers belong in `src/app/state.rs`.
- Home, Set Up, song, live, and editor systems belong in their respective files under `src/screens/`.
- Pitch estimation and pitch-to-lane mapping belong in `src/audio/mod.rs`; stateful detector implementations belong in `src/audio/detector.rs`.
- Input lifecycle resources and CPAL/MIDI setup belong in `src/input/mod.rs`.
- Device selection, input slot models, and persisted settings belong in `src/settings/mod.rs` and `src/screens/setup.rs`.
- Chart editing (document/track management) belongs in `src/screens/editor.rs`; it reads the shared chart model and settings libraries, but owns no gameplay or input-device state.
- Sheet-music layout logic belongs in `src/notation/mod.rs`, independent of any rendering framework or UI; it must not depend on screen modules so both editor and gameplay can consume it.
- Bevy systems should consume normalized `InstrumentEvent` values rather than reaching into audio callbacks.
- Keep constants local to the module that owns the behavior; avoid adding new global configuration without a user-facing need.

## Validation

- Fast regression check: `cargo test`
- Include ignored recording fixtures when tuning DSP: `cargo test supplied_bass_recordings_detect_expected_open_strings -- --ignored`
- Run the app manually with `cargo run` after changes to Bevy state wiring, device setup, or rendering.

## Change Discipline

1. Identify the owning module before editing.
2. Provide in line and function documentation for code.
3. Preserve the callback/worker/Bevy thread boundary.
4. Add or update a focused test for detector, mapping, chart parsing, or settings behavior.
5. Run the narrowest relevant command, then `cargo test` before handing off.
6. Update `README.md` when introducing new features or updating existing ones.
7. Update `AGENTS.md` when changing, removing or adding to the project structure.
