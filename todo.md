# Chart Editor Project Plan

This plan covers a score-authoring pipeline followed by runtime compilation:

- **Part A — Score-Centric Chart Schema and Engraving Semantics:** schema foundation complete;
  notation semantics are the next implementation phase.
- **Part B — Chart Editor:** edit the score directly as sheet music and tab.
- **Part C — Score-to-Performance Compilation:** expand score structure into runtime events.
- **Part D — Live Notation Overlay:** display the score during gameplay.
- **Part E — Clone Hero Import:** import external chart data into the score model.

## Status

- **Part A:** in progress. The score schema foundation is complete; engraving semantics for
  voices, staves, grace events, tabs, percussion, and lyrics are next.
- **Part B:** in progress. The editor foundation and notation engine exist; score rendering
  and full score-document editing remain.
- **Parts C-E:** not started.

## Completed Part A Reference

The completed schema includes tick timing, tempo/time-signature maps, notes, explicit rests,
voices, staves, tuplets, chords, ties/slurs, enharmonic spelling, clefs, key signatures,
dynamics, articulations, grace notes, lyrics, score navigation, tempo expressions, tunings,
kits, and separate Open Band `performance` hints. Runtime expansion and rendering are tracked
in Parts B-D below.

### Part A2 — Engraving semantics and notation data

This phase extends the score model without implementing rendering or gameplay. Each item needs
schema types, validation, serialization coverage, and focused tests before Part B consumes it.

- **Voice-aware rhythm:** add voice-local beam groups, stem directions, and rests. Validate
  that beams, tuplets, ties, and chord groups do not cross voices.
- **Independent staves:** support staff-local clef/key changes and staff assignment rules;
  validate coherent staff/voice ownership and aligned measures.
- **Grace events:** represent ordered pre-beat events linked to a principal event, with
  acciaccatura versus appoggiatura, slash state, and a timing policy that does not consume
  ordinary beat duration.
- **Chord semantics:** make chord groups stable and explicit, with shared onset/duration,
  staff, and voice constraints while preserving individual pitches and techniques.
- **String/tab semantics:** add authored string/fret choices, capo offsets, natural/artificial/
  pinch harmonics, palm mute, vibrato, let-ring, dead-note, and related tab variants.
- **Percussion semantics:** add kit-piece staff positions and notehead roles, cymbal/cross-
  stick markers, hi-hat state, flams, drags, buzz rolls, chokes, sticking, independent
  hand/foot voices, and roll markings, separate from gameplay lanes.
- **Lyrics:** add syllable-to-note relationships, hyphen boundaries, melisma spans, breath
  marks, and multiple lyric verses while preserving the phrase model.
- Add validation and round-trip fixtures covering multiple voices, staves, grace groups,
  chords, tab techniques, percussion notation, and lyric verses.

---

## Part B — Chart Editor

Depends on Part A2 engraving semantics being complete and merged.

### B2. Score document management

- Replace event-only document assumptions with score documents containing measures, staves,
  voices, explicit rests, score structure markers, and stable event IDs.
- Edit clef, key signature, tuning/kit definitions, tempo text, rehearsal marks, and other
  score metadata directly in the editor.
- Keep `performance` hints visible as optional instrument guidance without making them the
  source of notation truth.

### B3. Shared snapping model

- Use the existing `SnapInterval` and `snap_tick()` implementation for the editor, mapped to
  exact tick fractions via the chart's `resolution`) shared by both sheet view and tab
  view, rather than two separate implementations. Snapping is tempo-independent — it only
  needs `resolution` and the active `time_signature_map` entry, not `bpm`.

### B4. Notation engine (`src/notation.rs`, shared with Part D)

- Extend the existing notation engine, independent of the editor and gameplay UI, so
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

### B6.1. Complete score rendering

- Render explicit rests distinctly from inferred rests, including dots, tuplets, voices, and
  measure boundaries.
- Render written clefs, key signatures, enharmonic spelling, dynamics, articulations,
  rehearsal marks, tempo text, expressive ranges, repeats, endings, segno, coda, fine,
  da capo, and dal segno.
- Render ties and slurs from stable event-ID relationships rather than inferring them from
  gameplay techniques.
- Render chord groups and independent voices without collapsing their rhythmic or staff
  context.
- Render strings as aligned notation/tab using score pitches and optional performance hints;
  performance hints must not replace the written score.
- Render percussion symbols from kit-piece metadata, including shared lanes and lane-spanning
  pieces, while keeping this representation separate from the future gameplay highway.
- Render vocal lyrics with syllable boundaries and melisma markers aligned to phrase notes.

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
  (see the completed score schema) rather than exposing tie authoring as a separate manual step. Define collision
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
