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

### 5. Multi-track editor view: track focus and sheet/tab toggle (implemented)

PageUp/PageDown cycles the focused track, with one full-size editable sheet/tab view.
The tick position and sheet/tab mode are shared across tracks. Other tracks appear
in compact, non-interactive timing references beside or above the focused view.

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
- **Reference lanes (Option C, implemented)**: other tracks appear as thin horizontal
  timing-only rows above sheet view, using the focused view's exact
  visible tick interval. Rows are anchored just above the focused score's working
  area with a fixed 24px gap, not the window header; shorter pages stay bottom-aligned.
  Each row shows track number/name and note-onset markers,
  without pitch/fret detail or editing controls. Simultaneous notes share one marker;
  rests are omitted and nested vocal notes participate. Three reference tracks are
  shown per page in document order, excluding the focused track. Ctrl+PageUp/PageDown
  cycles reference pages without changing focus, selection, cursor, or view mode.
  Scrolling, zoom, track focus, and preview-follow update the reference interval with
  the focused view. In tab mode, editable lanes use the left half of the screen and
  vertical timing-reference lanes use the right half, sharing the exact tick-to-height
  mapping. Beat guides and the playhead cross both halves for direct alignment.
  The reference half is non-interactive; it cannot start focused-track note edits.
  Paging is navigation-only and does not dirty the chart or add undo entries.
  A full synced multi-staff score view remains a longer-term stretch goal.

Regression coverage checks tick clipping, focused-track exclusion, chord/rest and
nested-vocal handling, page wraparound, single-track charts, and keyboard isolation
in both views. Manual visual QA remains recommended.

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
to choose a playable tab position. Sheet tablature frets have filled green circular
backings (widened for multi-character labels) and horizontal green duration bars with
end caps, aligned to authored duration and clipped at the viewport edge. Technique
letters remain readable alongside fret numbers. Percussion symbols use kit-piece metadata, distinguish
lane-spanning pieces, and label shared pieces. Vocal lyrics show syllable boundaries and
melisma spans. Manual visual QA across representative multi-staff, string, percussion, and
vocal charts remains recommended.

### 7. Tab view (implemented)

The shared note-length palette, dot, triplet, rest, and tie toolbar controls are
available in tab mode as well as sheet mode. Rest clicks insert directly without
fret selection; explicit rests have visible, removable REST badges on the first lane.
Duration, dots, tuplets, and backward tie placement reuse the shared insertion path.
In tab tie mode, an endpoint click places a continuation inferred from the preceding
note on that lane without opening a fret picker. The continuation inherits note
properties and retains the active duration/dot/tuplet settings. This takes priority
over endpoint resizing, which remains available with tie mode off. Missing predecessors
and occupied continuation positions leave the score unchanged.

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

The SLIDE toolbar tool in sheet/tab uses two existing pitched notes: select the
source then a later destination. It sets the destination's slide performance hint,
preserving all other note data. The destination fret shows `/`; no slur is created.
The staff connects visible notehead centres with a straight line trimmed to leave
a small gap at each head. Destination slide hints resolve to the preceding pitched
note on the same voice/staff, preferring a matching preferred-string hint.
Escape or toggling the tool cancels the pending pair, and successful
authoring exits the tool and is undoable.

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

### 10. Multi-select and bulk edit (implemented)

Sheet and tab views share snapped, inclusive onset-range selection across the focused
track. Use the `SELECT` toggle and drag in either view, Shift-drag in tab, or
Ctrl+Shift-drag in sheet (preserving Shift-click sharp placement). Holding Shift
extends the existing range. Shift+Left/Right extends the range using the shared
cursor and snap interval. Amber outlines identify selected sheet noteheads/tab
badges and the visible range. Selection persists across sheet/tab switching. Chords and
nested vocal phrase notes participate; authored/inferred rests do not. Escape clears
selection before leaving the editor, and track changes clear selection.
Vocal tab notes use one pitch-name lane, so untuned voice tracks remain visible.

`BULK EDIT` or `Ctrl+B` opens a transactional keyboard field menu using the existing
Up/Down, Left/Right, Enter, and Escape conventions. String operations include moving
all selected notes to one string while preserving their written pitches, assigning
one string/fret (with tuning/capo-derived pitches), transposing by semitones, and
assigning a MIDI pitch. Vocal notes support transpose/set-pitch, and percussion uses
the actual kit's pieces, including shared-lane and lane-spanning pieces.

Every batch validates on a cloned track before commit. Unplayable strings/frets,
MIDI overflow, unknown kit pieces, and pitch/piece changes that break ties at the
selection boundary reject the complete operation and retain the draft. Select the
entire tie chain for pitch/piece changes. Authored rhythm, event IDs, relationships,
and unrelated guidance remain intact; invalidated string hints are cleared after
pitch edits. Apply is one undoable transaction, and cancel leaves the chart untouched.
Selection does not dirty the document or create history; score mutations and
undo/redo clear selection to avoid stale targets.

Regressions cover atomic rollback, capo/fret mapping, tie-chain preservation,
chord/phrase ranges, kit mapping, round-trip fidelity, draft cancellation, pointer
gestures in both orientations, sheet accidental-modifier isolation, keyboard dispatch
in both views, and undo/redo. Manual visual QA remains recommended.

Copy/paste is implemented with `Ctrl+C` / `Ctrl+V` in sheet and tab views. Copy takes
all selected notes into an internal session clipboard. Paste requires a destination
range (including an empty range), replaces its selected notes, and aligns the earliest
copied onset to the earlier boundary. Relative timing and notation are retained;
copied duration is not clipped to the range. Without a range, paste does nothing.
Paste is atomic and one undo step; it clears selection but retains the clipboard.

Pasted events/chords get fresh IDs and internal tie/slur endpoints are remapped,
including resolved legacy ties. Relationships crossing the copy boundary are omitted.
Nested vocal notes become independent authored notes; lyrics/phrase containers are
not copied. Compatible tracks/documents must have the same kind and resolution.
Destination kits must contain copied piece names; unplayable preferred-string hints
are cleared for the destination tuning/capo without altering sounding pitch. Failed
validation leaves the destination unchanged. Tests cover boundary alignment,
replacement/relationship cleanup, uniqueness, nested notes, repeated paste, incompatible
destinations, missing selection, keyboard routing in both views, and undo/redo.

Delete removes all range-selected notes in either view, including nested vocal notes,
and cleans referencing ties/slurs. Rests and unselected notes are preserved. The range
is cleared and the complete deletion is one undoable transaction. With no selected
notes, the existing focused-event deletion remains available. Dialog/metadata Delete
behavior is unchanged. Keyboard regressions cover both views and full undo/redo.

Selected ranges can also be dragged along the time axis in either view. An
unmodified press inside a selected range starts a move, including with SELECT active;
Shift-modified gestures continue to extend selections. The snapped range and cyan
note ghosts preview the destination without mutating the score. Release commits
one undoable translation; Escape cancels. Pitch, duration, relative spacing, mapping,
and IDs are preserved. Moves clamp at tick zero and reject partial tie chains,
same-voice/staff destination overlaps with matching pitches/pieces or explicit rests,
overflow, and vocal notes moved outside their owning phrase. Tests cover preview,
cancel, commit/undo/redo, bounds, collisions, ties, phrase containment, and sheet
pointer dispatch. Movement is time-only; pitch/string/piece edits use Bulk Edit.

### 11. Testing

- Unit tests for snapping math, fret/piece resolution in the dialogs, round-trip
  save/load fidelity (author a chart in the editor, reload it, assert equality), the
  Notation Engine's rest-inference/beaming/tie layout (B4) against hand-picked beat
  patterns including syncopation and multi-measure silence, and track-focus switching
  (B5) preserving the shared playhead position across tracks of different kinds.
- Manual QA: `cargo run` through creating a multi-track chart (strings + percussion +
  voice), placing chords, editing durations, switching the focused track and the
  sheet/tab toggle, and reloading it in the normal chart browser.