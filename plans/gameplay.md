## Score-to-Performance Compilation

Depends on the score-centric Part A schema. This part creates a runtime-specific model;
gameplay systems do not read authored score JSON directly.

### 1. Runtime model

- Add a `performance`/compiler module with runtime event types for absolute time, duration,
  target pitch/string/fret, percussion piece/lane, vocal target, input rule, and source score
  event ID.
- Preserve source references so runtime hits and diagnostics can point back to score events.
- Keep authored spelling, notation relationships, and score structure out of runtime events
  unless they affect the compiled performance behavior.

### 2. Score expansion

- Expand repeat bars, repeat counts, first/second endings, segno, coda, da capo, dal segno,
  fine, and rehearsal sections into a linear runtime timeline.
- Detect malformed or cyclic navigation and report editor validation errors instead of
  producing an unbounded runtime sequence.
- Resolve explicit rests, voices, chords, ties, tuplets, grace notes, and phrase spans into
  runtime timing without changing the authored score.

### 3. Tempo and expression compilation

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

### 4. Instrument performance hints

- Resolve embedded score tunings and kits into playable string/fret and percussion targets.
- Apply optional `performance` hints for preferred string/fret, attack, transition, bend,
  roll, droll, lane, difficulty, and required/optional status.
- Define deterministic defaults when hints are absent, based on pitch, tuning, kit, and
  instrument kind.

### 4.1. Chart-track to runtime-instrument binding

- Add an explicit runtime binding step from each score track to an `InstrumentSlot`; gameplay
  must never guess a device solely from a track name.
- Match candidates by instrument kind, explicit track role/ID, and compatible tuning or kit.
- Report missing, ambiguous, or incompatible bindings before gameplay starts, including the
  expected score tuning/kit and the available runtime slots.
- Allow the player to override a binding when multiple compatible instruments are configured,
  while retaining the score's expected tuning/kit as the conversion target.
- Keep physical device identity, detector profile, and input calibration in runtime settings;
  do not write them into the score chart.

### 4.2. Best-effort tuning conversion

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

### 4.3. Optional tuning-change workflow

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

### 5. Gameplay feature integration

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

### 6. Tests

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

### 7. Dynamic instrument highway model

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