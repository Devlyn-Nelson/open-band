# Chart Editor Project Plan

This plan covers two sequential efforts:

- **Part A — Instrument Model & Chart Schema Rework**: replace the current bass-specific,
  hardcoded tuning system with a generic, data-driven model (`Strings` / `Percussion` /
  `Voice`), and update the detection systems and sample charts accordingly. This is a
  breaking change and must land before Part B.
- **Part B — Chart Editor**: build the sheet-view and tab-view chart editor on top of the
  reworked schema.

## Decisions

1. **Editor delivery shape**: the editor is a new screen/state inside the existing
   `open-band` app. Home gains an `Editor` option alongside `Live Session`, `Songs`, and
   `Set Up`.
2. **Sheet view notation fidelity**: sheet view is a full notation editor (ties, dotted
   notes, beaming, measure-aware layout) — not a simplified grid. It doubles as an optional
   read-only overlay at the bottom of the screen during live gameplay for learning purposes
   (see Part C). This requires chart events to carry explicit rhythmic notation data, not
   just a raw `duration_beats` float (see A2).
3. **Rest/silence representation**: rests are **not** stored in chart data. They're
   inferred at render time from the gaps between notes, using a beat/measure-aware
   quantization algorithm shared by the sheet editor and the live overlay (see B4/Notation
   Engine).
4. **Percussion lane symbol set**: `symbol` is an open-ended, data-driven string, not a
   fixed closed enum — new kit pieces (cowbell, splash, china, rim/cross-stick, etc.)
   should never require a code change to define. The renderer ships a broad default symbol
   catalog and falls back to a generic glyph for unrecognized symbols (see A2/B4).

---

## Part A — Instrument Model & Chart Schema Rework

### A1. Define generic instrument kinds

- Add an `InstrumentKind` enum: `Strings`, `Percussion`, `Voice`. This replaces the current
  free-text `instrument: String` field and the `"bass"`/`"guitar"` naming conventions used
  throughout `domain.rs`.
- `name` remains a free-text human-facing label only (e.g. "Lead Guitar", "Rhythm Guitar 2")
  and carries no logic.

### A2. New chart schema types (`src/domain.rs`)

- `Tuning`: ordered list of open-string notes, **low string first**, octave-qualified note
  names (e.g. `["B0", "E1", "A1", "D2", "G2"]`). Used by `Strings` tracks. Covers any
  string count, drop tunings, and future 7-string guitars without code changes.
- `Kit`: percussion kit definition.
  - `lanes`: visible lane count (e.g. `4`).
  - `pieces`: list of `{ name, trigger, lane | lane_span, symbol? }`.
    - `lane`: index of the shared lane this piece renders in (multiple pieces may share a
      lane, distinguished by `symbol`, e.g. tom vs. cymbal in the same column).
    - `lane_span`: a color string (e.g. `"yellow"`) instead of a lane index — marks a piece
      (e.g. kick) that renders as a full-width bar spanning all lanes in that color, rather
      than occupying a single lane. A piece has either `lane` or `lane_span`, not both.
    - `symbol`: a free-text string identifying the piece's rendered shape (e.g. `"tom"`,
      `"cymbal"`, `"cowbell"`). Not a closed enum — the renderer maintains a broad default
      catalog (kick via `lane_span`, snare, hi-hat closed/open, low/mid/high tom, crash,
      ride, china, splash, cowbell, rim/cross-stick) and falls back to a generic glyph for
      any unrecognized symbol, so new kit pieces never require a code change.
    - `trigger`: the MIDI (or other) note used to identify this piece from input.
  - Percussion chart notes reference a piece **by name**, not index, so reordering/editing
    a kit's piece list doesn't invalidate existing notes.
- `VocalRange` (optional, `Voice` tracks only): `{ low: "A2", high: "A4" }` or a named
  preset resolved to a range at editor-save time. Descriptive only — does not gate
  playability or detection.
- `ChartTrack` becomes: `{ name, kind: InstrumentKind, tuning | kit | vocal_range, notes }`.
- `ChartEvent` needs to represent both pitched (strings/voice) and percussive (percussion)
  content, and both need full rhythmic notation data since sheet view is a real notation
  editor and both kinds get staff/rhythm rendering. Split into a tagged representation:
  - `Pitched { start_beat, note_value, dots, tied, midi_note, preferred_string }`
    (strings/voice)
  - `Percussive { start_beat, note_value, dots, tied, piece }` (percussion, piece = kit
    piece name)
  - `note_value`: an enum (`Whole`, `Half`, `Quarter`, `Eighth`, `Sixteenth`) — the same
    palette the sheet-view toolbar exposes. This is the source of truth for rhythmic
    duration, replacing the old freeform `duration_beats` float.
  - `dots`: `0`-`2`, for dotted-note augmentation.
  - `tied`: `bool`, whether this event is tied into the next event of the same
    pitch/piece — supports durations that can't be expressed as a single notated value.
  - `duration_beats` (in beats, for playback timing) is derived from `note_value` + `dots`
    + the current `time_signature`, summed across a tie chain. Follow the existing
    `midi_note`/`note` pattern in `ChartEventFields` (custom `Deserialize` with
    cross-validation) if an explicit `duration_beats` override is also accepted for
    hand-authored charts.
  - No `Rest` variant: silences are inferred at render time from gaps between notes (see
    Decision 3 and the Notation Engine task in Part B).

### A3. Tuning/kit resolution logic

- Replace the hardcoded `tuning_for()` match table and its `"standard_bass_4"` /
  `"standard_bass_5"` string matching with logic that reads the `Tuning` array directly off
  the chart track — no name-based lookup, no code change needed for new tunings.
- Generalize `best_string_fret()` (already tuning-shape-agnostic) to operate on any
  `Strings` track, not just bass.
- Add equivalent piece-resolution logic for `Percussion` tracks: given a `Percussive` event's
  `piece` name, look up the piece in the track's `Kit` to get lane, symbol, or `lane_span`
  color for rendering.

### A4. Update chart methods in `domain.rs`

- Replace `Chart::bass_notes()` with a generic `Chart::string_notes()` (or similar) that
  works for any `Strings` track (former guitar/bass distinction disappears).
- Add `Chart::percussion_notes()` returning per-note lane/symbol/`lane_span` data suitable
  for rendering (kick as a full-width colored bar, shared-lane pieces distinguished by
  symbol).
- Update `Chart::total_beats()` and any other cross-track helpers to account for the new
  event shape and derived `duration_beats`.

### A5. Rewrite sample charts

- Migrate `charts/open-strings.json` and `charts/devs-test-song.json` to the new schema
  (`kind`, embedded `tuning`/`kit`/`vocal_range`, named-piece percussion notes).

### A6. Instrument/tuning/kit preset library (editor convenience, not a chart dependency)

- Add a small library of standard presets (e.g. "Standard Guitar", "4-String Bass",
  "5-String Bass", "4-Lane Rock Kit") that the editor can offer for selection.
- Presets are **resolved and copied into the chart** on save — charts never reference a
  preset by name at load time, keeping charts self-contained.
- Decide storage location for this preset library (e.g. `instruments/*.json` alongside
  `charts/`, or an embedded Rust table) — either works since it's editor-only convenience
  data, not game-runtime chart data.

### A7. Detection system rework

- Update `src/settings.rs` / `src/setup.rs` / `src/input.rs` / `src/detector.rs` /
  `src/audio.rs` to key off the generic `Strings` kind instead of hardcoded
  "bass"/"guitar" special-casing.
- Generalize the 4-string/5-string toggle (`BAND_HERO_BASS_STRINGS`, the `B` key in Input
  Setup) into logic driven by the selected tuning's string count, rather than a
  bass-specific binary toggle.
- Preserve the per-string energy tracker used for bass menu navigation
  (`src/navigation.rs`), but generalize it to work off any `Strings` tuning's string count
  rather than being bass-specific.
- Confirm polyphony/detector behavior differences between what were previously "guitar"
  and "bass" instrument types are preserved correctly now that both are just `Strings` with
  different tunings (e.g. via per-tuning or per-kind detector profile data if algorithmic
  differences still apply).

### A8. Gameplay rendering updates

- Update `src/chart.rs` / `src/gameplay.rs` to render percussion tracks: fixed lane count,
  shared-lane symbol distinction (tom vs. cymbal in the same column), and full-width
  colored bar for `lane_span` pieces (kick).

### A9. Tests (`src/tests.rs`)

- Update/add tests for: new schema parsing (`Strings`/`Percussion`/`Voice` tracks), tuning
  resolution from embedded data (no hardcoded table), percussion piece-name resolution,
  round-trip parsing of the rewritten sample charts, and vocal range parsing.

### A10. Documentation

- Update `README.md`'s "Chart Format" section and instrument terminology to match the new
  schema and generic instrument kinds.

---

## Part B — Chart Editor

Depends on Part A being complete and merged.

### B1. Editor entry point

- Resolve Open Decision 1 (in-app state vs. separate binary) and scaffold accordingly.
- Add navigation entry point (e.g. a "Chart Editor" option from Home, alongside `Live
  Session`, `Songs`, `Set Up`).

### B2. Document/track management

- New chart creation (title, bpm, time signature) and existing chart loading from
  `charts/`.
- Track list UI: add/remove/rename/reorder tracks; each track selects an `InstrumentKind`
  and then a tuning/kit (from the Part A preset library or custom entry) or vocal range.
- Multiple tracks of the same kind are fully supported (e.g. two `Strings` tracks for lead
  and rhythm guitar), disambiguated only by `name`.
- Save/load using the Part A schema, with validation before save (fret range limits,
  unplayable notes flagged in the editor instead of silently skipped with `eprintln!` at
  runtime).

### B3. Shared snapping model

- Implement a single `SnapInterval` concept (whole/half/quarter/eighth/sixteenth, mapped to
  beat fractions via the chart's `time_signature`/`bpm`) shared by both sheet view and tab
  view, rather than two separate implementations.

### B4. Notation engine (`src/notation.rs`, shared with Part C)

- New module owning sheet-music layout logic, independent of the editor and gameplay UI so
  both can reuse it:
  - Measure/barline layout driven by `time_signature` and `bpm`.
  - Beaming rules for consecutive eighth/sixteenth notes within a beat.
  - Tie rendering across a chain of `tied` events, including across barlines.
  - Clef selection per track kind (e.g. treble for guitar/vocals, bass clef for bass —
    decide the exact kind→clef default mapping during implementation; allow a per-track
    override since `Strings` no longer distinguishes guitar from bass).
  - **Rest inference**: given the known, unambiguous gaps between notes (derived from each
    note's explicit `note_value`/`dots`/`tied` chain — see A2), fill silence with a minimal
    set of standard rest symbols using beat/measure-aware subdivision rules (e.g. prefer a
    single rest spanning a beat over several small ones; split correctly across beat and
    measure boundaries; use repeated whole-measure rests for multi-measure silence).
  - This module has no rendering-framework dependency of its own — it produces a layout
    description (positions, symbol types) that both the editor's sheet view and the
    gameplay overlay (Part C) render.

### B5. Sheet view (editor)

- Toolbar: note-duration palette (whole/half/quarter/eighth/sixteenth, plus a dot toggle
  and tie toggle) and a tool-mode selector (Place Note / Remove Note).
- Staff rendering per track kind, using the Notation Engine (B4): pitched staff for
  `Strings`/`Voice` tracks; a rhythm staff (one line per lane, no pitch) for `Percussion`
  tracks. Rests are rendered automatically wherever the Notation Engine infers a gap — there
  is no "Place Silence" tool, since rests are inferred rather than authored (Decision 3).
- Clicking a line/space places a note at that pitch/beat position (snapped via
  `SnapInterval`) using the active duration/dot/tie state and tool mode.
- Same-beat stacking: clicking an occupied beat position with an unoccupied
  line/space adds a stacked note (chord) rather than replacing the existing note. "Remove
  Note" removes only the specific note at the clicked pitch, not the whole beat position.

### B6. Tab view

- Vertical, top-to-bottom layout mirroring gameplay rendering, with beat markers.
- Scroll wheel traverses time; add a separate zoom control (beats-per-screen) so traversal
  and zoom aren't conflated.
- Toolbar dropdown selects the active hint/snap interval (reusing `SnapInterval` from B3).
- Clicking a string/lane:
  - `Strings` tracks: open a dialog listing fret number + corresponding note name (derived
    from the track's tuning) for selection.
  - `Percussion` tracks: if the lane hosts multiple pieces (shared lane), prompt for which
    piece (e.g. tom vs. cymbal); if unambiguous, place directly. Kick-style `lane_span`
    pieces are placed via a distinct full-width control, not a per-lane click.
- Same-beat chord stacking rules match B5.
- Duration editing: dragging the top edge of a placed note upward extends its duration to
  the nearest snapped beat marker. Placing a duration that spans past the current
  `note_value` palette's single-symbol limit sets the `tied` flag automatically
  (see A2) rather than exposing tie authoring as a separate manual step. Define collision
  behavior (extension is capped at the start of the next note on the same
  string/lane/piece) and a minimum duration.

### B7. Undo/redo

- Undo/redo stack covering note placement, removal, duration edits, and track/document
  structural edits.

### B8. Playback aids

- Metronome/click track playback aligned to `bpm`/`time_signature`.
- A playhead that follows the current scroll/cursor position in tab view for audio preview.

### B9. Copy/paste and multi-select (stretch, later phase)

- Multi-select notes across a beat range; copy/paste within or across tracks of the same
  kind.

### B10. Testing

- Unit tests for snapping math, fret/piece resolution in the dialogs, round-trip
  save/load fidelity (author a chart in the editor, reload it, assert equality), and the
  Notation Engine's rest-inference/beaming/tie layout (B4) against hand-picked beat
  patterns including syncopation and multi-measure silence.
- Manual QA: `cargo run` through creating a multi-track chart (strings + percussion +
  voice), placing chords, editing durations, and reloading it in the normal chart browser.

---

## Part C — Live Notation Overlay (Gameplay Integration)

Depends on the Notation Engine (B4). Adds the read-only, in-gameplay sheet-music view
called out in Decision 2.

### C1. Overlay toggle

- Add a setting/keybind to show or hide a sheet-music strip docked at the bottom of the
  screen during `Live Session` / chart gameplay (`src/gameplay.rs`, `src/chart.rs`).
- Off by default unless there's an existing preference precedent to follow; persisted in
  `open-band-settings/settings.json` alongside other display preferences.

### C2. Read-only rendering

- Reuse the Notation Engine (`src/notation.rs`) layout output to render the currently
  playing track's staff, scrolling in sync with the chart playhead — no editing
  interactions, just a moving "you are here" indicator for learning purposes.
- Support switching which track's notation is displayed when a chart has multiple tracks
  (e.g. a bass player wants the bass staff, not the vocal staff).

### C3. Performance

- Confirm the overlay's rendering cost is acceptable alongside the existing real-time
  detection pipeline; layout should be computed ahead of time (or incrementally) rather
  than recomputed every frame.
