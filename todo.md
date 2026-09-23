# Chart Editor Project Plan

This plan covers two sequential efforts:

- **Part A — Instrument Model & Chart Schema Rework**: replace the current bass-specific,
  hardcoded tuning system with a generic, data-driven model (`Strings` / `Percussion` /
  `Voice`), and update the detection systems and sample charts accordingly. This is a
  breaking change and must land before Part B.
- **Part B — Chart Editor**: build the sheet-view and tab-view chart editor on top of the
  reworked schema.

## Status

- **Part A: done.** All of A1-A10 are implemented and tested, including two follow-on
  refinements beyond the original plan: instrument input configuration (`InstrumentSlot`)
  is fully dynamic (any number of slots of any kind, not a fixed 4-slot layout), and
  tunings/kits are loaded from real `tunings/`/`kits/` library directories (not just
  embedded Rust presets) with the live detection pipeline resolving lane assignment from
  the actual configured tuning/kit instead of guessing from a string count.
- **Part B: in progress.** B1-B4 are done (editor entry point, document/track management,
  `SnapInterval`, and the `src/notation.rs` Notation Engine). The multi-track view approach
  is now decided (Decision 6) but not yet implemented. B5-B11 (track focus/view toggle,
  sheet view, tab view, undo/redo, playback aids, multi-select/bulk edit, and broader
  manual QA) are not started.
- **Part C and Part D:** not started.

## Decisions

1. **Editor delivery shape**: the editor is a new screen/state inside the existing
   `open-band` app. Home gains an `Editor` option alongside `Live Session`, `Songs`, and
   `Set Up`.
2. **Sheet view notation fidelity**: sheet view is a full notation editor (ties, dotted
  notes, tuplets, beaming, measure-aware layout, clefs, and key signatures) — not a simplified
  grid. It doubles as an optional
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
5. **Timing representation**: chart event positions and chart-level tempo/time signature
   are stored as integer **ticks** against a chart-level `resolution` (ticks per quarter
   note), with a tempo map and time-signature map replacing the old single scalar `bpm`/
   `time_signature` fields — this supports tempo/time-signature changes mid-song and makes
   Clone Hero import exact rather than lossy (see A2). The editor UI and this document
   still *talk* in beats/measures/note values for authoring — ticks are the internal,
   exact storage encoding underneath that, not a user-facing concept.
6. **Multi-track editor view**: sheet/tab view shows one **focused track** at a time
   (Option A), switched via a keybind, with the tick position/playhead shared across the
   whole document so switching tracks doesn't lose your place in time. Sheet vs. tab is a
   **single global toggle**, not a per-track setting. Rendering the other tracks as
   collapsed, non-interactive reference lanes (Option C) is a planned future enhancement,
   not part of the first implementation; a full synced multi-staff score view (Option B)
   is a longer-term stretch goal only (see B5).

---

## Part A — Instrument Model & Chart Schema Rework

### A1. Define generic instrument kinds — DONE

- Add an `InstrumentKind` enum: `Strings`, `Percussion`, `Voice`. This replaces the current
  free-text `instrument: String` field and the `"bass"`/`"guitar"` naming conventions used
  throughout `domain.rs`.
- `name` remains a free-text human-facing label only (e.g. "Lead Guitar", "Rhythm Guitar 2")
  and carries no logic.

### A2. New chart schema types (`src/domain.rs`) — DONE

#### Tick-based timing (Decision 5)

- `Chart.resolution: u32` — ticks per quarter note. Recommend `960` (divisible by
  2/3/4/5/6/8/16), matching common MIDI/`.chart` practice and leaving room for
  triplet/quintuplet support later without rounding.
- `Chart.tempo_map: Vec<TempoChange>` where `TempoChange { start_tick: u32, bpm: f32 }`.
  Must contain an entry at `start_tick = 0`. Replaces the old scalar `bpm` field and
  supports tempo changes mid-song, mirroring `.chart`'s `[SyncTrack]` `B` events.
- `Chart.time_signature_map: Vec<TimeSignatureChange>` where
  `TimeSignatureChange { start_tick: u32, numerator: u8, denominator: u8 }`. Must contain
  an entry at `start_tick = 0`. Replaces the old scalar `time_signature` field and
  supports time-signature changes mid-song, mirroring `.chart`'s `TS` events.
- Add a tick-to-seconds conversion helper that walks the tempo map segment by segment,
  replacing the current single-tempo `event.start_beat * 60.0 / self.bpm` math used
  throughout `domain.rs`. Reference the `.chart`/MIDI "time conversion" algorithm (tempo
  maps are a solved problem in that ecosystem) rather than re-deriving it from scratch.
- `ChartEvent.start_tick: u32` replaces `start_beat: f32`.
- Notated duration is still authored via `note_value`/`dots`/`tied` (Decision 2), but the
  tick length of a value is now exact integer arithmetic off `resolution` (quarter =
  `resolution`, eighth = `resolution / 2`, etc.) instead of a float derived from `bpm` —
  no rounding drift, and exact if triplet support is added later.
- This also makes Clone Hero `.chart` import lossless for position/tempo/time-signature
  data: ticks map directly, rescaled only if the source and target `resolution` differ
  (`target_tick = source_tick * target_resolution / source_resolution`), with no lossy
  float round-trip (see Part D).

#### Instrument/kit types

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
- `ChartTrack` becomes: `{ name, kind: InstrumentKind, tuning | kit | vocal_range, notes,
  clef, key_signature, star_power_phrases }`.

#### Event content and dynamics

- `ChartEvent` needs to represent both pitched (strings/voice) and percussive (percussion)
  content, and both need full rhythmic notation data since sheet view is a real notation
  editor and both kinds get staff/rhythm rendering. Split into a tagged representation:
  - `Pitched { start_tick, note_value, dots, tied, tuplet, chord, dynamic, articulations,
    midi_note, preferred_string, attack, transition, bend, motion }` (strings/voice)
  - `Percussive { start_tick, note_value, dots, tied, tuplet, chord, dynamic, articulations,
    piece, dynamics, roll, droll }`
    (percussion, piece = kit piece name)
  - `note_value`: an enum (`Whole`, `Half`, `Quarter`, `Eighth`, `Sixteenth`) — the same
    palette the sheet-view toolbar exposes. This is the source of truth for rhythmic
    duration, replacing the old freeform `duration_beats` float.
  - `dots`: `0`-`2`, for dotted-note augmentation.
  - `tuplet`: `{ actual, normal }`, such as `{ actual: 3, normal: 2 }` for triplets. The
    resolved tick duration is scaled by `normal / actual`, and notation layout groups
    adjacent events with the same ratio.
  - `chord`: an optional shared identifier for simultaneous notes, preserving chord grouping
    for sheet/tab rendering and editor operations.
  - `tied`: `bool`, whether this event is tied into the next event of the same
    pitch/piece — supports durations that can't be expressed as a single notated value.
    Chart validation checks that the next event starts at the tie endpoint and keeps the
    same pitch/piece; `tied` remains useful and is not redundant with other fields.
  - `duration_ticks` (for playback timing) is derived from `note_value` + `dots` +
    `resolution`, summed across a tie chain. Follow the existing `midi_note`/`note`
    pattern in `ChartEventFields` (custom `Deserialize` with cross-validation) if an
    explicit `duration_ticks` override is also accepted for hand-authored charts.
  - No `Rest` variant: silences are inferred at render time from gaps between notes (see
    Decision 3 and the Notation Engine task in Part B).
  - `dynamics` (percussion only): an enum `Normal | Accent | Ghost`, covering both
    "accents" and "hit strength" — a louder or quieter hit than normal. Optional,
    defaults to `Normal`. Matches Clone Hero's accent/ghost modifiers directly (see
    Part D). A future continuous velocity value is a possible later extension but isn't
    needed for parity with existing formats.
  - `dynamic` (all event kinds): standard written levels `ppp`, `pp`, `mp`, `mf`, `f`,
    `ff`, and `fff`.
  - `articulations` (all event kinds): an extensible list currently supporting `staccato`,
    `tenuto`, `marcato`, `accent`, `fermata`, and `grace`.
  - Drum roll fields (percussion only):
    - `roll: Option<String>` names the ending piece for a single-lane roll or piece
      transition. The event's `piece` is the starting piece; using the same name for both
      represents a same-piece roll.
    - `droll: Option<String>` names the second piece for a double-lane roll, alternating
      between the event's `piece` and `droll` for the event duration.
    - Both fields use kit piece names and are mutually exclusive. The roll duration comes
      from the event's `note_value`/`dots` and any tie chain; the hit subdivision/rate is a
      gameplay rule rather than additional chart data.
    - This replaces the former `RollKind::SingleLane`/`DoubleLane` marker.
  - String technique fields (strings/voice pitched events only):
    - `attack: Option<NoteAttack>` is `pluck` or `tap` and describes how the note starts.
    - `transition: Option<NoteTransition>` is `hammer_on`, `pull_off`, or `slide` and
      describes how the note connects from the preceding event.
    - `bend: Option<BendSpec>` stores a bend amount in semitones plus an optional
      `release: true` return toward the original pitch.
    - `motion: Option<PitchMotion>` currently supports a `trill` target pitch for repeated
      movement between the current note and the target. Note names and MIDI numbers are
      accepted for pitch targets.
    - These fields preserve `note` as the event's pitch and are chart/notation data only;
      gameplay interpretation is deferred.
  - `bend.points` optionally describes a normalized time curve of semitone offsets, allowing
    bend-release and other shaped bends; the simple `semitones`/`release` form remains valid.
- Track notation metadata:
  - `clef` optionally overrides inferred `treble`, `bass`, `alto`, or `tenor`.
  - `key_signature` stores `{ fifths: -7..7, mode: major | minor }`.
- `VocalPhrase` supports `syllable: single | begin | middle | end` and `melisma: bool` so
  lyric syllable boundaries and one-syllable/multiple-note phrases survive notation export.
- `star_power_phrases: Vec<Phrase>` on `ChartTrack` (any instrument kind), where
  `Phrase { start_tick, duration_ticks }` — a simple range marker with no nested note
  list. Anything played during that span counts as part of the phrase; this mirrors the
  existing `VocalPhrase` style already in the schema (start/duration only, membership
  inferred by overlap) rather than requiring an explicit note list.

### A3. Tuning/kit resolution logic — DONE

- Replace the hardcoded `tuning_for()` match table and its `"standard_bass_4"` /
  `"standard_bass_5"` string matching with logic that reads the `Tuning` array directly off
  the chart track — no name-based lookup, no code change needed for new tunings.
- Generalize `best_string_fret()` (already tuning-shape-agnostic) to operate on any
  `Strings` track, not just bass.
- Add equivalent piece-resolution logic for `Percussion` tracks: given a `Percussive` event's
  `piece` name, look up the piece in the track's `Kit` to get lane, symbol, or `lane_span`
  color for rendering.

### A4. Update chart methods in `domain.rs` — DONE

- Replace `Chart::bass_notes()` with a generic `Chart::string_notes()` (or similar) that
  works for any `Strings` track (former guitar/bass distinction disappears).
- Add `Chart::percussion_notes()` returning per-note lane/symbol/`lane_span` data suitable
  for rendering (kick as a full-width colored bar, shared-lane pieces distinguished by
  symbol).
- Update `Chart::total_beats()` (rename to reflect ticks, e.g. `Chart::total_ticks()`) and
  any other cross-track helpers to account for the new event shape, `duration_ticks`, and
  the tempo/time-signature maps.

### A5. Rewrite sample charts — DONE

- Migrate `charts/open-strings.json` and `charts/devs-test-song.json` to the new schema
  (`kind`, embedded `tuning`/`kit`/`vocal_range`, named-piece percussion notes).

### A6. Instrument/tuning/kit preset library (editor convenience, not a chart dependency) — DONE (extended)

- Went further than originally planned: tunings and kits are loaded from real `tunings/`
  and `kits/` library directories at runtime (`load_tuning_library()`, `load_kit_library()`),
  not just an embedded Rust table, with an embedded fallback if the directory is missing.
  Kit presets remain an embedded Rust table (`standard_kit_presets()`); only tunings/kits
  used by Input Setup and the editor were moved to files.

- Add a small library of standard presets (e.g. "Standard Guitar", "4-String Bass",
  "5-String Bass", "4-Lane Rock Kit") that the editor can offer for selection.
- Presets are **resolved and copied into the chart** on save — charts never reference a
  preset by name at load time, keeping charts self-contained.
- Decide storage location for this preset library (e.g. `instruments/*.json` alongside
  `charts/`, or an embedded Rust table) — either works since it's editor-only convenience
  data, not game-runtime chart data.

### A7. Detection system rework — DONE (extended)

- Went further than originally planned: `InstrumentSlot` (formerly a fixed 4-slot layout)
  is now a fully dynamic `Vec<InstrumentSlot>` — any number of slots of any kind
  (`Strings`/`Percussion`/`Voice`) can be configured in Input Setup, each with its own
  device, tuning/kit, and detector profile. Live detection (`pitch_to_lane`, `string_lane`,
  `AudioDetector`, the MIDI-to-lane mapping) resolves lane assignment from the slot's
  actual configured tuning/kit, not a guessed string-count table.

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

### A8. Gameplay rendering updates — DONE

- Percussion rendering is visual-only for now (no hit detection/scoring wired up for
  percussion in chart gameplay yet — flagged in code comments as a follow-up).

- Update `src/chart.rs` / `src/gameplay.rs` to render percussion tracks: fixed lane count,
  shared-lane symbol distinction (tom vs. cymbal in the same column), and full-width
  colored bar for `lane_span` pieces (kick).

### A8.1. Drum roll and string technique chart schema — DONE

- Implement `roll` and `droll` as named percussion-piece fields with parser,
  serialization, kit-resolution, mutual-exclusion, and validation support.
- Implement string/voice pitched-event `attack`, `transition`, `bend`, and `motion` fields,
  including note-name/MIDI target resolution and round-trip tests.

### A8.2. Roll and string-technique gameplay — NOT STARTED

- Resolve `roll` events into timed hit targets for playback and scoring. A `roll` repeats or
  transitions across the starting and ending pieces; a `droll` alternates between the two
  pieces at the selected gameplay subdivision.
- Render roll targets across their complete duration, including lane changes and shared
  lane symbols, instead of rendering one sustained bar.
- Implement live interpretation and scoring for plucks, taps, hammer-ons, pull-offs, slides,
  bends, and trill motion without changing their chart representation.
- Add gameplay tests for roll timing, roll transitions, double-lane alternation, and string
  technique hit/scoring behavior.

### A9. Tests (`src/tests.rs`) — DONE

- Update/add tests for: new schema parsing (`Strings`/`Percussion`/`Voice` tracks), tuning
  resolution from embedded data (no hardcoded table), percussion piece-name resolution,
  round-trip parsing of the rewritten sample charts, and vocal range parsing.

### A10. Documentation — DONE

- Update `README.md`'s "Chart Format" section and instrument terminology to match the new
  schema and generic instrument kinds.

---

## Part B — Chart Editor

Depends on Part A being complete and merged.

### B1. Editor entry point — DONE

- Resolve Open Decision 1 (in-app state vs. separate binary) and scaffold accordingly.
- Add navigation entry point (e.g. a "Chart Editor" option from Home, alongside `Live
  Session`, `Songs`, `Set Up`).

### B2. Document/track management — DONE

- Custom hand-entry of tuning/kit (as opposed to picking from the library) is not
  implemented; only library presets can be selected for now.

- New chart creation (title, initial tempo, initial time signature — written as the
  required tick-0 entries in `tempo_map`/`time_signature_map`) and existing chart loading
  from `charts/`.
- Track list UI: add/remove/rename/reorder tracks; each track selects an `InstrumentKind`
  and then a tuning/kit (from the Part A preset library or custom entry) or vocal range.
- Multiple tracks of the same kind are fully supported (e.g. two `Strings` tracks for lead
  and rhythm guitar), disambiguated only by `name`.
- Save/load using the Part A schema, with validation before save (fret range limits,
  unplayable notes flagged in the editor instead of silently skipped with `eprintln!` at
  runtime).

### B3. Shared snapping model — DONE

- Implemented as `SnapInterval` (a type alias for `NoteValue`, since both are the same
  whole/half/quarter/eighth/sixteenth palette) plus `snap_tick()` in `src/domain.rs`. Not
  yet consumed by any UI since B6/B7 (the actual note-placement views) aren't built yet.

- Implement a single `SnapInterval` concept (whole/half/quarter/eighth/sixteenth, mapped to
  exact tick fractions via the chart's `resolution`) shared by both sheet view and tab
  view, rather than two separate implementations. Snapping is tempo-independent — it only
  needs `resolution` and the active `time_signature_map` entry, not `bpm`.

### B4. Notation engine (`src/notation.rs`, shared with Part C) — DONE

- Implemented: measure layout, tie-chain grouping, beam grouping, and greedy rest
  inference (largest-value-first, split at measure boundaries), plus a per-track default
  clef heuristic (pitch-threshold based, since `Strings` no longer distinguishes guitar
  from bass). Not yet wired into any renderer — B6 (sheet view) and Part C (gameplay
  overlay) are the two future consumers. Rest inference does not yet handle dotted rests
  or non-power-of-two leftover ticks (no tuplet support anywhere in the schema, so this
  hasn't come up in practice).

- New module owning sheet-music layout logic, independent of the editor and gameplay UI so
  both can reuse it:
  - Measure/barline layout driven by `time_signature_map` and `resolution` (tempo/`bpm`
    only affects playback speed, not layout, so measure boundaries stay stable even
    across tempo changes).
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

### B5. Multi-track editor view: track focus and sheet/tab toggle

- A chart can have any number of tracks, but sheet/tab view can only show one track's full
  detail readably at a time. First implementation (**Option A**): the view always shows
  exactly one **focused track**, full-size and fully editable, with its name/kind/tuning
  shown in the header.
- A keybind switches the focused track (e.g. `PageUp`/`PageDown`), cycling through the
  document's track list in order.
- The current tick position (playhead/scroll position) is shared document-wide, not
  per-track — switching the focused track does not lose your place in time, only the
  vertical (pitch/lane) axis changes.
- Sheet vs. tab is a **single global toggle**, not a per-track setting, applying to
  whichever track is currently focused, so switching between the two views is quick
  regardless of which track you're on.
- **Future enhancement (Option C, not part of this first pass)**: render the other
  tracks as thin, non-interactive reference lanes (note positions only, no pitch/fret
  detail) above/below the focused staff, for timing context when composing against other
  parts. A full synced multi-staff score view (every track fully rendered at once) is a
  longer-term stretch goal, not currently planned.

### B6. Sheet view (editor)

- Toolbar: note-duration palette (whole/half/quarter/eighth/sixteenth, plus a dot toggle
  and tie toggle) and a tool-mode selector (Place Note / Remove Note).
- Staff rendering for the focused track (B5), using the Notation Engine (B4): pitched
  staff for `Strings`/`Voice` tracks; a rhythm staff (one line per lane, no pitch) for
  `Percussion` tracks. Rests are rendered automatically wherever the Notation Engine infers
  a gap — there is no "Place Silence" tool, since rests are inferred rather than authored
  (Decision 3).
- Clicking a line/space places a note at that pitch/beat position (snapped via
  `SnapInterval`) using the active duration/dot/tie state and tool mode.
- Same-beat stacking: clicking an occupied beat position with an unoccupied
  line/space adds a stacked note (chord) rather than replacing the existing note. "Remove
  Note" removes only the specific note at the clicked pitch, not the whole beat position.

### B7. Tab view

- Vertical, top-to-bottom layout mirroring gameplay rendering, with beat markers, for the
  focused track (B5).
- Scroll wheel traverses time; add a separate zoom control (beats-per-screen) so traversal
  and zoom aren't conflated.
- Toolbar dropdown selects the active hint/snap interval (reusing `SnapInterval` from B3).
- Clicking a string/lane:
  - `Strings` tracks: open a dialog listing fret number + corresponding note name (derived
    from the track's tuning) for selection.
  - `Percussion` tracks: if the lane hosts multiple pieces (shared lane), prompt for which
    piece (e.g. tom vs. cymbal); if unambiguous, place directly. Kick-style `lane_span`
    pieces are placed via a distinct full-width control, not a per-lane click.
- Same-beat chord stacking rules match B6.
- Duration editing: dragging the top edge of a placed note upward extends its duration to
  the nearest snapped beat marker. Placing a duration that spans past the current
  `note_value` palette's single-symbol limit sets the `tied` flag automatically
  (see A2) rather than exposing tie authoring as a separate manual step. Define collision
  behavior (extension is capped at the start of the next note on the same
  string/lane/piece) and a minimum duration.

### B8. Undo/redo

- Undo/redo stack covering note placement, removal, duration edits, and track/document
  structural edits.

### B9. Playback aids

- Metronome/click track playback aligned to the `tempo_map`/`time_signature_map` (correctly
  speeding up/slowing down through tempo changes rather than assuming one fixed `bpm`).
- A playhead that follows the current scroll/cursor position in tab view for audio preview.

### B10. Multi-select and bulk edit

- Multi-select notes across a tick/beat range in tab view.
- **Bulk reassignment**: with a selection active, reassign all selected notes to a new
  string/fret (or pitch/piece) in a single action. This is needed as a core capability, not
  a stretch item — Part D's guitar/bass import produces musically arbitrary
  lane-to-open-string placements that are only practical to correct with bulk
  reassignment, so this should land before or alongside Part D.
- Copy/paste within or across tracks of the same kind (stretch, can follow later).

### B11. Testing

- Unit tests for snapping math, fret/piece resolution in the dialogs, round-trip
  save/load fidelity (author a chart in the editor, reload it, assert equality), the
  Notation Engine's rest-inference/beaming/tie layout (B4) against hand-picked beat
  patterns including syncopation and multi-measure silence, and track-focus switching
  (B5) preserving the shared playhead position across tracks of different kinds.
- Manual QA: `cargo run` through creating a multi-track chart (strings + percussion +
  voice), placing chords, editing durations, switching the focused track and the
  sheet/tab toggle, and reloading it in the normal chart browser.

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

---

## Part D — Clone Hero (`.chart`) Import

Depends on Part A (tick-based timing, dynamics/roll/star-power fields) and on B10's bulk
reassignment tool for practical guitar/bass cleanup. Import is one-way (`.chart` → Open
Band chart JSON); there's no requirement to export back to `.chart`.

### D1. Parser and top-level mapping

- Parse `.chart`'s section/key-value/track-event syntax (`[Song]`, `[SyncTrack]`,
  `[Events]`, per-difficulty instrument sections).
- `[Song].Resolution` → `Chart.resolution` directly (or rescaled if a different internal
  `resolution` convention is chosen).
- `[SyncTrack]` `B` (tempo) and `TS` (time signature) events map directly to
  `Chart.tempo_map` / `Chart.time_signature_map` entries at the same tick — this is now a
  near 1:1, lossless conversion thanks to the tick-based schema (Decision 5).
- **Difficulty selection**: always import the single hardest difficulty section present
  per instrument (`Expert`, falling back to `Hard` → `Medium` → `Easy` if absent) and
  discard the rest, matching Open Band's preference for one deterministic chart rather
  than tiered difficulties.

### D2. Drums import

- Detect track type (standard 4-lane / 4-lane Pro / 5-lane) via `song.ini` tags
  (`pro_drums`, `five_lane_drums`) first, falling back to note-based heuristics (cymbal
  modifiers present → Pro; 5-lane green note or sustains present → 5-lane; otherwise
  standard 4-lane).
- Build a `Kit` from the detected layout: note `0` → a `lane_span` kick piece; notes
  `1`-`4` (or `1`-`5` for 5-lane) → lanes, each with an appropriate `symbol`; cymbal
  modifiers (`66`-`68`) mark the corresponding piece's `symbol` as `"cymbal"` instead of
  `"tom"` on 4-lane Pro charts, reusing the shared-lane design already in the schema.
- Map accent/ghost modifiers (`34`-`44`) to the new `dynamics` field (`Accent`/`Ghost`).
- Map roll-lane special phrases (`65`/`66`) to the new `roll` field on the notes they
  cover.
- Map the Star Power special phrase (`2`) to `star_power_phrases` on the track.
- Map Expert+/2x kick (`32`) — decide during implementation whether this becomes a second
  `lane_span` kick piece or is folded into the same kick piece, since Open Band's MIDI kick
  input is a single trigger either way.
- Drop anything with no equivalent: Star Power activation phrase (`64`), BRE/`coda`
  events, and other RB-specific stem/mix event metadata.

### D3. Guitar/bass import

- CH's 5 fixed fret lanes have no reliable mapping to an arbitrary `Strings` tuning, so
  import uses a simple, explicitly approximate rule: lane *N* → open string *N* (fret 0),
  up to the track's string count (for a 4-string `Strings` track, decide how to fold the
  5th lane in — e.g. merge into the adjacent string — during implementation).
- This intentionally produces a musically arbitrary starting point. The expected workflow
  is to import, then use the tab editor's multi-select + bulk reassignment tool (B10) to
  correct runs of notes to their real string/fret positions.
- Star Power phrases import the same way as drums (D2).
- HOPO/strum-flip/open-note/forced-note markers (5-fret-specific mechanics) have no
  equivalent in the tab model and are dropped.

### D4. Vocals import

- `.chart`'s text format doesn't typically carry vocal/lyric data (that's more common in
  the `.mid`-based Rock Band format) — if a source chart has no vocal track, this step is
  simply skipped. Treat full vocal import as a candidate for a future `.mid` importer
  rather than blocking on it here.

### D5. Testing

- Round-trip a small hand-authored `.chart` fixture (drums + guitar, including a tempo
  change, a cymbal modifier, an accent, and a Star Power phrase) through the importer and
  assert the resulting `Chart` matches expected ticks, kit pieces, dynamics, and phrases.
- Manual QA: import a real community `.chart` file and confirm it loads and plays back in
  Open Band's existing chart browser without panicking, even if guitar/bass note placement
  needs manual cleanup afterward.
