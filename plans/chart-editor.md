## Chart Editor

Depends on Part A2 engraving semantics being complete and merged.

### 2. Score document management

- Replace event-only document assumptions with score documents containing measures, staves,
  voices, explicit rests, score structure markers, and stable event IDs.
- Edit clef, key signature, tuning/kit definitions, tempo text, rehearsal marks, and other
  score metadata directly in the editor.
- Keep `performance` hints visible as optional instrument guidance without making them the
  source of notation truth.

### 3. Shared snapping model

- Use the existing `SnapInterval` and `snap_tick()` implementation for the editor, mapped to
  exact tick fractions via the chart's `resolution`) shared by both sheet view and tab
  view, rather than two separate implementations. Snapping is tempo-independent — it only
  needs `resolution` and the active `time_signature_map` entry, not `bpm`.

### 4. Notation engine (`src/notation.rs`, shared with Part D)

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

### 5. Multi-track editor view: track focus and sheet/tab toggle

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

### 6. Sheet view (editor)

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

### 6.1. Complete score rendering

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

### 7. Tab view

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

### 7.1. Score relationships and navigation editing

- Edit enharmonic spelling independently from sounding MIDI pitch.
- Create and edit ties, slurs, phrase spans, chord groups, tuplets, grace notes, and
  articulation/dynamic markings.
- Add repeat bars, numbered endings, segno/coda navigation, fine markers, and rehearsal
  marks without changing the authored event timeline.
- Edit Open Band `performance` hints separately from score notation, including preferred
  string/fret, attacks, transitions, percussion mapping, rolls, and drolls.

### 8. Undo/redo

- Undo/redo stack covering note placement, removal, duration edits, and track/document
  structural edits.

### 9. Playback aids

- Metronome/click track playback aligned to the `tempo_map`/`time_signature_map` (correctly
  speeding up/slowing down through tempo changes rather than assuming one fixed `bpm`).
- A playhead that follows the current scroll/cursor position in tab view for audio preview.

### 10. Multi-select and bulk edit

- Multi-select notes across a tick/beat range in tab view.
- **Bulk reassignment**: with a selection active, reassign all selected notes to a new
  string/fret (or pitch/piece) in a single action. This is needed as a core capability, not
  a stretch item — Part E's guitar/bass import produces musically arbitrary
  lane-to-open-string placements that are only practical to correct with bulk
  reassignment, so this should land before or alongside Part E.
- Copy/paste within or across tracks of the same kind (stretch, can follow later).

### 11. Testing

- Unit tests for snapping math, fret/piece resolution in the dialogs, round-trip
  save/load fidelity (author a chart in the editor, reload it, assert equality), the
  Notation Engine's rest-inference/beaming/tie layout (B4) against hand-picked beat
  patterns including syncopation and multi-measure silence, and track-focus switching
  (B5) preserving the shared playhead position across tracks of different kinds.
- Manual QA: `cargo run` through creating a multi-track chart (strings + percussion +
  voice), placing chords, editing durations, switching the focused track and the
  sheet/tab toggle, and reloading it in the normal chart browser.