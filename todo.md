# Chart Editor Project Plan

This plan covers a score-authoring pipeline followed by runtime compilation:

- **Part A — Score-Centric Chart Schema**: define the authoritative music-sheet JSON model.
  It stores written music, notation relationships, instrument definitions, and optional
  Open Band performance hints. Backward compatibility is intentionally out of scope while
  the application is still experimental.
- **Part B — Chart Editor**: build sheet and tab editing directly on the score model.
- **Part C — Score-to-Performance Compilation**: convert authored score data into a separate
  runtime model by expanding repeats, endings, tempo expressions, and gameplay hints.
- **Part D — Live Notation Overlay**: display the score model during gameplay.
- **Part E — Clone Hero Import**: import external chart data into the score model.

## Status

- **Part A: in progress.** The generic instrument infrastructure and initial notation fields
  exist, but the score-centric schema redesign, explicit score structure, written rests,
  voices, relationships, and performance-hint separation are not complete.
- **Part B: in progress.** B1-B4 are done (editor entry point, document/track management,
  `SnapInterval`, and the `src/notation.rs` Notation Engine). The score-centric editor work
  and multi-track view are not complete.
- **Part C, Part D, and Part E:** not started.

## Decisions

1. **Editor delivery shape**: the editor is a new screen/state inside the existing
   `open-band` app. Home gains an `Editor` option alongside `Live Session`, `Songs`, and
   `Set Up`.
2. **Sheet view notation fidelity**: sheet view is a full notation editor (notes, rests,
  voices, ties, dotted notes, tuplets, beaming, measure-aware layout, clefs, key signatures,
  articulations, dynamics, and score navigation) — not a simplified grid. It doubles as an optional
   read-only overlay at the bottom of the screen during live gameplay for learning purposes
  (see Part D). This requires chart events to carry explicit rhythmic notation data, not
   just a raw `duration_beats` float (see A2).
3. **Score versus runtime model**: the chart JSON is the authoritative written score. The
  editor reads and writes it directly. Gameplay never consumes it directly; Part C compiles
  it into a runtime-specific event sequence.
4. **Rest/silence representation**: explicit rests are first-class score events. Inferred
  rests may still be a convenience for empty gaps, but explicit written rests take precedence.
5. **Percussion lane symbol set**: `symbol` is an open-ended, data-driven string, not a
   fixed closed enum — new kit pieces (cowbell, splash, china, rim/cross-stick, etc.)
   should never require a code change to define. The renderer ships a broad default symbol
   catalog and falls back to a generic glyph for unrecognized symbols (see A2/B4).
6. **Timing representation**: chart event positions and chart-level tempo/time signature
   are stored as integer **ticks** against a chart-level `resolution` (ticks per quarter
   note), with a tempo map and time-signature map replacing the old single scalar `bpm`/
   `time_signature` fields — this supports tempo/time-signature changes mid-song and makes
   Clone Hero import exact rather than lossy (see A2). The editor UI and this document
   still *talk* in beats/measures/note values for authoring — ticks are the internal,
   exact storage encoding underneath that, not a user-facing concept.
7. **Multi-track editor view**: sheet/tab view shows one **focused track** at a time
   (Option A), switched via a keybind, with the tick position/playhead shared across the
   whole document so switching tracks doesn't lose your place in time. Sheet vs. tab is a
   **single global toggle**, not a per-track setting. Rendering the other tracks as
   collapsed, non-interactive reference lanes (Option C) is a planned future enhancement,
   not part of the first implementation; a full synced multi-staff score view (Option B)
   is a longer-term stretch goal only (see B5).
8. **Score semantics versus performance hints**: written pitch, rhythm, spelling, dynamics,
  articulations, phrasing, and structure belong to the score. Optional `performance` data
  describes Open Band teaching and input preferences such as string/fret, attack, lane,
  roll, droll, and difficulty. These hints never replace written notation.
9. **Instrument definitions**: tunings and kits belong in the score because they define how
  the part is notated and interpreted. Runtime device selection remains in settings and is
  resolved separately when compiling a playable session.

---

## Part A — Score-Centric Chart Schema

### A1. Define generic instrument kinds — DONE

- Add an `InstrumentKind` enum: `Strings`, `Percussion`, `Voice`. This replaces the current
  free-text `instrument: String` field and the `"bass"`/`"guitar"` naming conventions used
  throughout `domain.rs`.
- `name` remains a free-text human-facing label only (e.g. "Lead Guitar", "Rhythm Guitar 2")
  and carries no logic.

### A2. New chart schema types (`src/domain.rs`) — IN PROGRESS

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
  float round-trip (see Part E).

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
  - Score events contain written pitch/rest, start, duration, voice, staff, chord group,
    dynamics, articulations, and notation relationships.
  - Optional `performance` data contains Open Band-specific string/fret, attack, transition,
    bend, motion, percussion piece/lane, `roll`, `droll`, and difficulty hints.
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
  - Add first-class `Rest` events with written duration, voice, and optional chord/staff
    context. Empty gaps may still be inferred when no explicit rest is authored.
  - `dynamics` (percussion performance hint): an enum `Normal | Accent | Ghost`, covering both
    "accents" and "hit strength" — a louder or quieter hit than normal. Optional,
    defaults to `Normal`. Matches Clone Hero's accent/ghost modifiers directly (see
    Part E). A future continuous velocity value is a possible later extension but isn't
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
    - These are named score/performance hints, not a closed roll-kind enum.
  - String technique hints (strings/voice performance data only):
    - `attack` is `pluck` or `tap` and describes how the note starts.
    - `transition` is `hammer_on`, `pull_off`, or `slide` and describes how the note connects
      from the preceding note.
    - `bend` stores a bend amount in semitones plus an optional release or normalized curve.
    - `motion` currently supports a `trill` target pitch for repeated movement.
    - These hints preserve written pitch as the score's source of truth and are interpreted
      only by the Part C runtime compiler.
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

#### A2.1. Score-centric schema redesign — NOT STARTED

- Replace raw MIDI-only pitch storage with a written `Pitch` containing MIDI value plus
  enharmonic spelling (`step`, `alter`, and `octave`) so `C#4` and `Db4` remain distinct.
- Add explicit `voice` and `staff` identifiers to events. Tuplets, beams, rests, and ties
  must be grouped within a voice rather than across the whole track.
- Move ties and slurs into relationship collections keyed by stable event IDs. Keep
  hammer-ons, pull-offs, and slides as optional performance hints rather than confusing
  them with written ties or slurs.
- Add explicit `performance` objects for Open Band hints: preferred string/fret, attack,
  transition, bend interpretation, percussion lane/piece, `roll`, `droll`, and difficulty.
  The score remains valid when these hints are absent.
- Add score-level structure markers: repeat starts/ends, numbered endings, segno, coda,
  da capo, dal segno, fine, and rehearsal marks. These describe the written score and are
  expanded only by the runtime compiler.
- Add tempo text and expressive ranges for accelerando, ritardando, crescendo, and
  diminuendo without replacing the resolved tempo map used for playback.
- Keep tunings and kits in the score as embedded, versioned instrument definitions. Runtime
  device selection remains outside the chart.
- Increment the chart schema version deliberately; compatibility with the current prototype
  format is not required.

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

### A5. Rewrite sample charts — NOT STARTED

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

### A9. Tests (`src/tests.rs`) — IN PROGRESS

- Update/add tests for: new schema parsing (`Strings`/`Percussion`/`Voice` tracks), tuning
  resolution from embedded data (no hardcoded table), percussion piece-name resolution,
  round-trip parsing of the rewritten sample charts, and vocal range parsing.
- Add score-model tests for explicit rests, written pitch spelling, voices, tuplets, chord
  groups, ties/slurs, score navigation, dynamics/articulations, tempo expressions, and
  separation of score data from `performance` hints.

### A10. Documentation — IN PROGRESS

- Update `README.md`'s "Chart Format" section and instrument terminology to match the new
  schema and generic instrument kinds.
- Document the score-centric JSON model, embedded tunings/kits, optional performance hints,
  runtime compilation boundary, and schema versioning policy.

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

#### B2.1. Score document management — NOT STARTED

- Replace event-only document assumptions with score documents containing measures, staves,
  voices, explicit rests, score structure markers, and stable event IDs.
- Edit clef, key signature, tuning/kit definitions, tempo text, rehearsal marks, and other
  score metadata directly in the editor.
- Keep `performance` hints visible as optional instrument guidance without making them the
  source of notation truth.

### B3. Shared snapping model — DONE

- Implemented as `SnapInterval` (a type alias for `NoteValue`, since both are the same
  whole/half/quarter/eighth/sixteenth palette) plus `snap_tick()` in `src/domain.rs`. Not
  yet consumed by any UI since B6/B7 (the actual note-placement views) aren't built yet.

- Implement a single `SnapInterval` concept (whole/half/quarter/eighth/sixteenth, mapped to
  exact tick fractions via the chart's `resolution`) shared by both sheet view and tab
  view, rather than two separate implementations. Snapping is tempo-independent — it only
  needs `resolution` and the active `time_signature_map` entry, not `bpm`.

### B4. Notation engine (`src/notation.rs`, shared with Part D) — DONE

- Implemented: measure layout, tie-chain grouping, beam grouping, tuplet grouping, explicit
  key-signature propagation, and greedy rest inference. Not yet wired into any renderer —
  B6 (sheet view) and Part D (gameplay overlay) are the future consumers.

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
  - **Rest layout**: render authored rests exactly, and infer only unoccupied gaps that have
    no explicit rest. Support dotted rests, tuplets, and repeated whole-measure rests.
  - Relationship layout: render ties, slurs, phrase spans, chord groups, dynamics, and
    articulations without confusing score relationships with performance hints.
  - This module has no rendering-framework dependency of its own — it produces a layout
    description (positions, symbol types) that both the editor's sheet view and the
    gameplay overlay (Part D) render.

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

- Toolbar: note-duration palette (whole/half/quarter/eighth/sixteenth, tuplets, dot/rest/
  tie controls, dynamics, articulations, and a Place/Remove tool selector).
- Staff rendering for the focused track (B5), using the Notation Engine (B4): pitched
  staff for `Strings`/`Voice` tracks; a rhythm staff (one line per lane, no pitch) for
  `Percussion` tracks. Explicit rests are rendered exactly; inferred rests fill only gaps
  without an authored rest.
- Clicking a line/space places a score note or rest at that pitch/beat position (snapped via
  `SnapInterval`) using the active duration, tuplet, dot, voice, relationship, and notation
  state.
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

### B7.1. Score relationships and navigation editing

- Edit enharmonic spelling independently from sounding MIDI pitch.
- Create and edit ties, slurs, phrase spans, chord groups, tuplets, grace notes, and
  articulation/dynamic markings.
- Add repeat bars, numbered endings, segno/coda navigation, fine markers, and rehearsal
  marks without changing the authored event timeline.
- Edit Open Band `performance` hints separately from score notation, including preferred
  string/fret, attacks, transitions, percussion mapping, rolls, and drolls.

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
  a stretch item — Part E's guitar/bass import produces musically arbitrary
  lane-to-open-string placements that are only practical to correct with bulk
  reassignment, so this should land before or alongside Part E.
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

## Part C — Score-to-Performance Compilation

Depends on the score-centric Part A schema. This part creates a runtime-specific model;
gameplay systems do not read authored score JSON directly.

### C1. Runtime model

- Add a `performance`/compiler module with runtime event types for absolute time, duration,
  target pitch/string/fret, percussion piece/lane, vocal target, input rule, and source score
  event ID.
- Preserve source references so runtime hits and diagnostics can point back to score events.
- Keep authored spelling, notation relationships, and score structure out of runtime events
  unless they affect the compiled performance behavior.

### C2. Score expansion

- Expand repeat bars, repeat counts, first/second endings, segno, coda, da capo, dal segno,
  fine, and rehearsal sections into a linear runtime timeline.
- Detect malformed or cyclic navigation and report editor validation errors instead of
  producing an unbounded runtime sequence.
- Resolve explicit rests, voices, chords, ties, tuplets, grace notes, and phrase spans into
  runtime timing without changing the authored score.

### C3. Tempo and expression compilation

- Convert tempo text, accelerando, and ritardando ranges into the runtime tempo curve.
- Add an optional performance tempo scale without modifying the authored score. A player can
  slow down or speed up the compiled runtime while preserving the score's written tempo
  markings and beat/tick positions.
- Add an optional performance key transposition in semitones. The score keeps its original
  written key and spelling; the runtime compiler applies the selected interval to pitches,
  vocal targets, string targets, and compatible percussion pitch mappings where applicable.
- Keep score-level key signatures separate from runtime transposition so the editor displays
  the original sheet music while gameplay can present a player-friendly key.
- Carry dynamics, articulations, and expressive ranges into runtime input/scoring hints only
  where gameplay needs them.
- Keep the score's written tempo markings and dynamic markings available to the editor.

### C4. Instrument performance hints

- Resolve embedded score tunings and kits into playable string/fret and percussion targets.
- Apply optional `performance` hints for preferred string/fret, attack, transition, bend,
  roll, droll, lane, difficulty, and required/optional status.
- Define deterministic defaults when hints are absent, based on pitch, tuning, kit, and
  instrument kind.

### C4.1. Chart-track to runtime-instrument binding

- Add an explicit runtime binding step from each score track to an `InstrumentSlot`; gameplay
  must never guess a device solely from a track name.
- Match candidates by instrument kind, explicit track role/ID, and compatible tuning or kit.
- Report missing, ambiguous, or incompatible bindings before gameplay starts, including the
  expected score tuning/kit and the available runtime slots.
- Allow the player to override a binding when multiple compatible instruments are configured,
  while retaining the score's expected tuning/kit as the conversion target.
- Keep physical device identity, detector profile, and input calibration in runtime settings;
  do not write them into the score chart.

### C4.2. Best-effort tuning conversion

- Treat the score tuning as the intended instrument setup, not as a requirement that the
  player owns that exact tuning.
- Compare the score tuning with the selected runtime tuning and calculate a compatibility
  report: playable notes, notes requiring alternate strings/frets, notes outside the target
  range, and notes requiring transposition.
- For a song authored in a tuning such as five-string A0-D1-G1-C2-F2, provide a best-effort
  conversion to a standard tuning such as B0-E1-A1-D2-G2. Preserve the sounding pitch when
  the target tuning can play it; otherwise choose the nearest playable octave/pitch and
  report the deviation rather than silently changing it.
- Prefer a deterministic optimization over ad hoc per-note guesses: minimize total pitch
  deviation first, then fret displacement, then unnecessary string changes. Preserve
  explicit performance string/fret hints when they remain playable.
- Keep the original score pitches and tuning unchanged. Store conversion decisions only in
  the compiled runtime chart, with source-event references and a user-visible warning list.
- Support a user-selected global transposition as a fallback when the target tuning cannot
  cover the score's range. Choose the smallest semitone shift that maximizes playable notes,
  then allow the player to accept or adjust it.
- Make conversion policy configurable: exact-pitch preference, octave-preserving preference,
  or maximum-playability preference. The default should favor preserving sounding pitch.

### C4.3. Optional tuning-change workflow

- Add an optional pre-play tuning menu when the score tuning and selected runtime tuning are
  incompatible. Show the expected tuning, current runtime tuning, compatibility summary, and
  proposed transposition/conversion.
- Let the player continue without changing physical tuning, accept the best-effort runtime
  conversion, or choose a compatible tuning preset when the instrument supports it.
- Do not require this menu for compatible tunings; it should be an opt-in setup step rather
  than an interruption for every song.
- Distinguish a real physical retuning from a software conversion. Open Band can recommend or
  record a tuning choice, but it must not assume the application can retune a physical string.
- Add a later hardware integration point for automatic tuners or MIDI-controlled instruments
  without coupling the score format to a specific device.

### C5. Gameplay feature integration

- Notes and rests: emit playable targets only for notes; preserve rests as intentional gaps
  in the runtime timeline.
- Tuplets and voices: compile exact event times per voice without flattening written rhythm
  incorrectly.
- Chords: emit grouped simultaneous runtime targets with one source group ID.
- Ties and slurs: sustain or connect runtime targets according to score relationships;
  do not treat a slur as a gameplay attack by itself.
- Dynamics and articulations: map only the markings that affect teaching or scoring, while
  preserving all written markings for notation.
- Strings: convert written pitches plus optional performance hints into string/fret targets,
  then apply pluck, tap, hammer-on, pull-off, slide, bend, and trill rules.
- Percussion: convert named kit pieces into lanes and apply `roll`, `droll`, grace-note,
  sticking, and cymbal/hi-hat hints when those schema features are available.
- Voice: convert lyric phrase notes, syllable boundaries, melismas, and vocal targets into
  runtime singing prompts.
- Score navigation: expand repeats and endings before hit windows are calculated so gameplay
  timing follows the performed order rather than the unexpanded score order.

### C6. Tests

- Test repeat and ending expansion, navigation errors, tempo curves, tuplets, voices, rests,
  chord timing, score-to-runtime source references, and hint fallback behavior.
- Test key transposition and tempo scaling without mutating the authored score.
- Test score-track/runtime-slot binding for compatible, ambiguous, missing, and incompatible
  instruments.
- Test tuning conversion with A0-D1-G1-C2-F2 to B0-E1-A1-D2-G2, including exact playable
  notes, octave fallback, pitch-deviation reporting, global transposition, and preservation
  of source-event references.
- Test tuning-menu decisions separately from conversion so choosing a runtime preset does not
  alter the score's embedded tuning.

### C7. Dynamic instrument highway model

- Build the gameplay presentation from the compiled runtime instrument bindings rather than
  hardcoding one string highway plus a side-mounted drum strip.
- Create one highway model per active non-vocal instrument track or runtime player part.
  The number of highways is dynamic and must support one instrument, multiple string parts,
  multiple percussion parts, and mixed string/percussion sessions.
- Give every highway the same construction pipeline: runtime targets determine lane count,
  lane semantics, colors, labels, hit line, note shapes, sustain treatment, input feedback,
  and source-track identity. Instrument-specific differences should be data supplied to the
  shared builder, not separate layout implementations.
- Derive the required lane count from the bound runtime part. Examples include one lane per
  playable string, kit lanes plus lane-spanning kick pieces, and future controller-specific
  lane layouts. Do not assume five string lanes or four drum lanes globally.
- Define a layout policy for multiple highways: available screen width, minimum lane width,
  stacking or tiling, focus/player emphasis, and a readable fallback when too many parts are
  active. Preserve stable lane geometry so notes and labels never resize during play.
- Keep highway identity linked to the runtime binding and source score track so feedback can
  identify the instrument, tuning/kit conversion, and originating chart event.
- Add runtime tests for one, many, mixed, and zero active instrument bindings, including lane
  counts that differ from the default guitar/bass/drum layouts.

## Part D — Live Notation Overlay (Gameplay Integration)

Depends on the Notation Engine (B4). Adds the read-only, in-gameplay sheet-music view
called out in Decision 2.

### D1. Dynamic score overlays

- Add a setting/keybind to show or hide score overlays during `Live Session` / chart gameplay
  (`src/gameplay.rs`, `src/chart.rs`).
- Create one sheet/tab overlay per active score track or player part, using the same dynamic
  runtime binding list as the highways. Overlays must appear, disappear, and reorder safely
  when the active instrument set changes.
- Use the track's notation layout, clef, key, voice, and score structure rather than rebuilding
  notation from gameplay lanes.
- Off by default unless there's an existing preference precedent to follow; persisted in
  `open-band-settings/settings.json` alongside other display preferences.

### D2. Read-only rendering

- Reuse the Notation Engine (`src/notation.rs`) layout output to render the currently
  playing tracks' staves, scrolling in sync with the compiled runtime playhead — no editing
  interactions, just a moving "you are here" indicator for learning purposes.
- Place the vocal overlay above the instrument highway/overlay stack so lyrics and vocal pitch
  targets remain visible without competing with instrument lanes.
- Support switching focus, collapsing overlays, and selecting which instrument part is shown
  when a chart has multiple tracks; the underlying runtime can still contain all parts.

### D3. Shared highway construction and lane adaptation

- Use the same highway geometry and note-construction logic for strings and percussion, with
  instrument data supplying lane count, lane labels, colors, shapes, and hit semantics.
- Keep score overlays and highways synchronized through compiled source-event IDs, but allow
  each view to render the representation appropriate to its purpose.
- Ensure lane-spanning percussion pieces, chords, sustains, rolls, and simultaneous parts do
  not change the highway's dimensions or cause visual overlap.

### D4. Future 3D highway presentation

- Replace the current 2D falling-note prototypes with perspective 3D highways inspired by
  Rock Band and Clone Hero: notes begin far down the highway, travel toward a fixed hit line,
  and expose a larger upcoming-note field.
- Use a camera/frustum and highway coordinate system that supports multiple instrument
  highways while maintaining a shared song clock and hit-line timing.
- Keep lane geometry stable in world space; adapt viewport placement and camera framing to
  the number of active highways instead of changing note semantics.
- Design the 3D renderer as a presentation layer over runtime events. The score model and
  score-to-performance compiler must remain independent of Bevy entities, meshes, cameras,
  and screen coordinates.
- Add staged validation: first verify dynamic 2D highway construction, then perspective
  motion, camera framing, multi-highway readability, mobile/window resizing, and performance.

### D5. Performance

- Confirm the overlay's rendering cost is acceptable alongside the existing real-time
  detection pipeline; layout should be computed ahead of time (or incrementally) rather
  than recomputed every frame.

---

## Part E — Clone Hero (`.chart`) Import

Depends on Part A's score schema and Part C's compiler, plus B10's bulk
reassignment tool for practical guitar/bass cleanup. Import is one-way (`.chart` → Open
Band chart JSON); there's no requirement to export back to `.chart`.

### E1. Parser and top-level mapping

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

### E2. Drums import

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

### E3. Guitar/bass import

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

### E4. Vocals import

- `.chart`'s text format doesn't typically carry vocal/lyric data (that's more common in
  the `.mid`-based Rock Band format) — if a source chart has no vocal track, this step is
  simply skipped. Treat full vocal import as a candidate for a future `.mid` importer
  rather than blocking on it here.

### E5. Testing

- Round-trip a small hand-authored `.chart` fixture (drums + guitar, including a tempo
  change, a cymbal modifier, an accent, and a Star Power phrase) through the importer and
  assert the resulting `Chart` matches expected ticks, kit pieces, dynamics, and phrases.
- Manual QA: import a real community `.chart` file and confirm it loads and plays back in
  Open Band's existing chart browser without panicking, even if guitar/bass note placement
  needs manual cleanup afterward.
