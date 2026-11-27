## Chart Editor

Depends on Part A2 engraving semantics being complete and merged.

### 2. Score document management (completed)

- Replace event-only document assumptions with score documents containing measures, staves,
  voices, explicit rests, score structure markers, and stable event IDs.
- Edit clef, key signature, tuning/kit definitions, tempo text, rehearsal marks, and other
  score metadata directly in the editor.
- Keep `performance` hints visible as optional instrument guidance without making them the
  source of notation truth.

### 3. Shared snapping model (completed)

- Use the existing `SnapInterval` and `snap_tick()` implementation for the editor, mapped to
  exact tick fractions via the chart's `resolution`) shared by both sheet view and tab
  view, rather than two separate implementations. Snapping is tempo-independent — it only
  needs `resolution` and the active `time_signature_map` entry, not `bpm`.

### 4. Notation engine (`src/notation/mod.rs`, shared with Part D) (completed)

Layout is independent of rendering. Relationship indices address the sorted symbol list,
and symbols retain their authored source. Inferred rests are scoped to voice and staff;
nonstandard gaps retain their exact duration even when no standard glyph matches them.
The sheet view and gameplay overlay still need to consume and render this layout.

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

Navigation state and controls are implemented: PageUp/PageDown cycles the focused track,
the tick position and sheet/tab mode are shared across tracks, and the current text editor
shows a focused-track event preview. Events can be inserted, edited, and removed in a
JSON-based text editor, including existing vocal phrase notes. Full-size graphical sheet
and tab views remain dependent on parts 6 and 7; the text editor is not a substitute
for those views, so this part is still open.

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

Implemented: a graphical four-quarter-note staff, clickable notation toolbar, snapped
pitched/rest placement, selected-piece percussion lane placement, chord stacking, per-note
removal, and stable-ID slur authoring. Shared layout beams, ties, slurs, and tuplets are
rendered. Manual visual QA remains; complete score engraving remains in 6.1.

- Toolbar: clickable note-duration palette (whole/half/quarter/eighth/sixteenth), tuplets,
  dot/rest/tie controls, dynamics, articulations, voice selection, percussion piece
  selection, slur authoring, and a Place/Remove tool selector. Keyboard shortcuts remain
  available.
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
- Relationship authoring: the Slur tool links two existing pitched notes by stable event
  IDs; the shared notation layout supplies beam, tie, slur, and tuplet groups to the sheet
  renderer.

### 6.1. Complete score rendering (implemented)

The sheet view now renders authored and inferred rests with distinct colors, dotted and
tuplet markings, voice/staff placement, barlines, written clefs and key signatures, and
key-aware enharmonic accidentals. Dynamics, articulations, rehearsal/tempo text, expressive
spans, repeats, endings, segno/coda/fine, and da capo/dal segno markers are shown. Tie and
slur chains use the notation layout's stable-ID relationships; chord groups receive shared
stems and independent voices/staves retain separate positions.

Strings show aligned notation and pitch-derived tablature, using preferred-string hints only
to choose a playable tab position. Percussion symbols use kit-piece metadata, distinguish
lane-spanning pieces, and label shared pieces. Vocal lyrics show syllable boundaries and
melisma spans. Manual visual QA across representative multi-staff, string, percussion, and
vocal charts remains recommended.

### 7. Tab view (implemented)

The focused track has a vertical, top-to-bottom tab view with meter-aware bar/beat markers
and unlabeled, progressively darker subdivision guides. `DIV -` / `DIV +` use the same
persisted beat-division setting as sheet mode; pointer placement preserves those exact
ticks through string fret selection and percussion insertion. Mouse-wheel
scroll traverses time; toolbar zoom buttons and `+`/`-` adjust beats-per-screen separately.
The tab snap button cycles the shared `SnapInterval` values. String clicks open a 0-24 fret
choice grid with tuning-derived note names, and placement stores the preferred string as an
optional performance hint while preserving the written pitch. Percussion shared lanes pause
for piece selection before placement; lane-spanning kit pieces use a separate full-width
control rail. Same-tick chord stacking and per-note removal reuse the sheet editor behavior.

The tab view shows a snapped placement circle on the hovered string/lane. The shared
`PLACE/REMOVE` toolbar toggle and `F10` select removal; its outline follows the actual
event badge and clicking removes that event directly, including on the SPAN rail,
without a fret dialog or duration drag.

Dragging a note's lower endpoint resizes it to the snapped minimum/duration. The end is capped
at the next event on the same resolved string, lane, or piece; durations beyond the active
palette's single-symbol limit set `tied` automatically. Manual visual QA across tunings,
shared percussion lanes, lane-spanning pieces, and overlapping notes remains recommended.

### 7.1. Score relationships and navigation editing (implemented)

Section-specific editors are available in sheet/tab and metadata views:
`Ctrl+E` opens a keyboard field menu for focused-event notation (including enharmonic spelling, custom tuplets,
grace notes, articulations, and dynamics) without changing pitch, onset, or hints;
`Ctrl+P` opens a separate keyboard menu for performance and instrument mapping. Up/Down
select fields; Left/Right adjust values; Enter cycles choices or opens scalar input.
Delete clears optional settings, and Home/End or Page keys navigate the paginated menu.
Apply commits the draft; Esc cancels. Fret/string choices stay consistent with written
pitch, and percussion choices come from the actual kit. Bend points and grace principals
are editable without JSON. `Ctrl+L` retains JSON editing for stable-ID
ties/slurs, complete chord groups, track phrase spans, and star-power spans atomically.
`Ctrl+M` edits navigation markers, repeat counts, ending numbers, rehearsal/tempo text,
and expressive spans without rewriting the authored event timeline.

Commits parse and validate before mutation. Invalid edits remain open for correction;
optional fields and arrays can be cleared to remove notation, hints, or relationships.
Phrase span edits preserve existing lyrics and notes, and reject removal of spans
containing notes. Preferred fret is verified against tuning/capo/pitch rather than stored
as competing notation truth. The existing full event editor remains available for pitch
and timeline changes. Regression tests cover isolation, relationships/navigation round trips,
fret/percussion mapping, keyboard dispatch, and invalid-commit retention. Manual visual QA remains.

- Edit enharmonic spelling independently from sounding MIDI pitch.
- Create and edit ties, slurs, phrase spans, chord groups, tuplets, grace notes, and
  articulation/dynamic markings.
- Add repeat bars, numbered endings, segno/coda navigation, fine markers, and rehearsal
  marks without changing the authored event timeline.
- Edit Open Band `performance` hints separately from score notation, including preferred
  string/fret, attacks, transitions, percussion mapping, rolls, and drolls.

### 8. Undo/redo (implemented)

Each open document retains up to 100 committed score changes. `Ctrl+Z` undoes;
`Ctrl+Y` or `Ctrl+Shift+Z` redoes. History captures keyboard edits and sheet/tab
pointer actions, including note placement/removal, duration changes, ties/slurs,
notation/performance edits, track addition/removal/renaming/reordering/kind changes,
and score metadata/structure changes. Changes within one editor update form one
transaction; uncommitted dialog drafts and navigation do not create history entries.

Restoring a snapshot also restores track/event selection and clears pending pointer
operations, while retaining the current scroll position, sheet/tab mode, and zoom.
Undo/redo is disabled while editing a dialog draft. New edits discard the redo branch.
Saving retains history and records a clean save point, so returning to that exact
score clears the dirty indicator. History is session-local and resets on opening a
different document. Regression tests cover note/tie restoration, tab duration edits,
structural edits, shortcut dispatch/modal safety, depth limits, redo invalidation,
and save-point tracking. Manual visual QA remains recommended.

### 9. Playback aids (implemented)

`Space` and the `PLAY`/`PAUSE` toolbar control preview a metronome from the shared
cursor. `STOP` returns to the position where the current pass began. The clock uses
the chart's segmented tempo-map conversion, so tempo changes alter speed without
changing tick positions. Meter denominators determine click spacing; each measure
starts with a higher-pitched accent and meter changes reset the accent grid.

Tab view draws an amber moving playhead and pages the viewport to keep it visible.
Manual scrolling/cursor movement seeks the running preview. Pausing retains the
reached tick, edit dialogs pause playback, and preview stops at the last authored
event (empty charts support an ongoing metronome). Playback state is separate from
score/history state. Clicks are generated WAV assets played through Bevy's default
audio output; this is a timing preview, not instrument-note synthesis. Dispatch is
frame-driven and missed clicks after a stalled frame are coalesced rather than burst.

Regressions cover tempo and meter transitions, seeking/paging, pause/stop, chart-end
behavior, keyboard/modal routing, and decoding non-silent click audio. Manual audio
and visual QA remains recommended.

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