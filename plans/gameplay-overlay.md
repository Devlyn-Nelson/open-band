## Live Notation Overlay (Gameplay Integration)

Depends on the Notation Engine (B4). Adds the read-only, in-gameplay sheet-music view
called out in Decision 2.

### 1. Dynamic score overlays

- Add a setting/keybind to show or hide score overlays during `Live Session` / chart gameplay
  (`src/gameplay.rs`, `src/chart.rs`).
- Create one sheet/tab overlay per active score track or player part, using the same dynamic
  runtime binding list as the highways. Overlays must appear, disappear, and reorder safely
  when the active instrument set changes.
- Use the track's notation layout, clef, key, voice, and score structure rather than rebuilding
  notation from gameplay lanes.
- Off by default unless there's an existing preference precedent to follow; persisted in
  `open-band-settings/settings.json` alongside other display preferences.

### 2. Read-only rendering

- Reuse the Notation Engine (`src/notation.rs`) layout output to render the currently
  playing tracks' staves, scrolling in sync with the compiled runtime playhead — no editing
  interactions, just a moving "you are here" indicator for learning purposes.
- Place the vocal overlay above the instrument highway/overlay stack so lyrics and vocal pitch
  targets remain visible without competing with instrument lanes.
- Support switching focus, collapsing overlays, and selecting which instrument part is shown
  when a chart has multiple tracks; the underlying runtime can still contain all parts.

### 3. Shared highway construction and lane adaptation

- Use the same highway geometry and note-construction logic for strings and percussion, with
  instrument data supplying lane count, lane labels, colors, shapes, and hit semantics.
- Keep score overlays and highways synchronized through compiled source-event IDs, but allow
  each view to render the representation appropriate to its purpose.
- Ensure lane-spanning percussion pieces, chords, sustains, rolls, and simultaneous parts do
  not change the highway's dimensions or cause visual overlap.

### 4. Future 3D highway presentation

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

### 5. Performance

- Confirm the overlay's rendering cost is acceptable alongside the existing real-time
  detection pipeline; layout should be computed ahead of time (or incrementally) rather
  than recomputed every frame.