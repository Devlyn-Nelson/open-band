# Agent Guide

## Start Here

- `src/main.rs`: crate root and process entry point; keep it limited to module declarations and `app::run()`.
- `src/app.rs`: Bevy app construction, resource initialization, and system registration.
- `src/audio.rs`: pure pitch estimation and pitch-to-lane mapping; no device or Bevy lifecycle code.
- `src/detector.rs`: stateful audio detectors and spectral analysis.
- `src/state.rs`: top-level Bevy application states.
- `src/input.rs`: worker-owned event channel and audio stream state.
- `src/settings.rs`: persisted settings, device enumeration, and input configuration.
- `src/setup.rs`: device selection, calibration, and tuner screens.
- `src/navigation.rs`: instrument-driven navigation and latency tap routing.
- `src/chart.rs`: song menu, chart countdown/gameplay/review systems.
- `src/menu.rs`: Home and Set Up menu systems.
- `src/gameplay.rs`: live-session rendering, event consumption, diagnostics, note motion, and scoring.
- `src/domain.rs`: chart data, chart gameplay state, screen states, and chart UI marker components.
- `src/editor.rs`: in-app chart editor (chart browse/create/load, track list add/remove/rename/reorder, tuning/kit cycling, save).
- `src/notation.rs`: framework-agnostic sheet-music layout (measures, ties, beaming, rest inference, clef selection); consumed by the chart editor's future sheet view and the future live gameplay notation overlay.
- `charts/`: built-in chart data loaded at compile time.
- `tunings/`: named string tunings loaded at runtime for Input Setup and the chart editor (embedded fallback if missing).
- `kits/`: named percussion kits loaded at runtime for Input Setup and the chart editor (embedded fallback if missing).
- `recordings/`: optional WAV fixtures used only by ignored detector tests.
- `README.md`: user-facing controls, device setup, chart format, and current limitations.

Read only the owning section first. The executable is intentionally split so chart changes do not require loading the input pipeline, and vice versa.

## Runtime Flow

```text
CPAL/MIDI callbacks -> instrument worker -> InstrumentEvent channel -> Bevy systems -> calibration/gameplay UI
chart JSON -> domain chart resources -> chart countdown/gameplay/review systems
```

The worker performs detection off the Bevy thread. Bevy owns state transitions, rendering, scoring, and event consumption.

## Ownership Boundaries

- Chart schema and chart screen state belong in `src/domain.rs`.
- Top-level screen identifiers belong in `src/state.rs`.
- Home, Set Up, and live gameplay systems belong in their respective screen modules.
- Pitch estimation and pitch-to-lane mapping belong in `src/audio.rs`; stateful detector implementations belong in `src/detector.rs`.
- Input lifecycle resources and CPAL/MIDI setup belong in `src/input.rs`.
- Device selection and persisted settings belong in `src/settings.rs` and `src/setup.rs`.
- Chart editing (document/track management) belongs in `src/editor.rs`; it reads the same chart schema as `src/domain.rs` and the same tuning/kit libraries as `src/settings.rs`, but owns no gameplay or input-device state.
- Sheet-music layout logic (measures, ties, beaming, rests, clef) belongs in `src/notation.rs`, independent of any rendering framework or UI; it must not depend on `src/editor.rs` or `src/gameplay.rs` so both can consume it.
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
