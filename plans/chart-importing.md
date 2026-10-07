## Clone Hero (`.chart`) Import

Depends on Part A's score schema and Part C's compiler, plus B10's bulk
reassignment tool for practical guitar/bass cleanup. Import is one-way (`.chart` → Open
Band chart JSON); there's no requirement to export back to `.chart`.

### 1. Parser and top-level mapping

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

### 2. Drums import

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

### 3. Guitar/bass import

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

### 4. Vocals import

- `.chart`'s text format doesn't typically carry vocal/lyric data (that's more common in
  the `.mid`-based Rock Band format) — if a source chart has no vocal track, this step is
  simply skipped. Treat full vocal import as a candidate for a future `.mid` importer
  rather than blocking on it here.

### 5. Testing

- Round-trip a small hand-authored `.chart` fixture (drums + guitar, including a tempo
  change, a cymbal modifier, an accent, and a Star Power phrase) through the importer and
  assert the resulting `Chart` matches expected ticks, kit pieces, dynamics, and phrases.
- Manual QA: import a real community `.chart` file and confirm it loads and plays back in
  Open Band's existing chart browser without panicking, even if guitar/bass note placement
  needs manual cleanup afterward.

## Other formats

we may also want importing capibilities for formats:
- guitar pro (gp, gp3, gp4, gp5, gpx).