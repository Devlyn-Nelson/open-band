# Open band

Open band is an experimental Rust rhythm game built with Bevy. The goal is Rocksmith-style gameplay for real guitar and bass, with MIDI drums and microphone vocals.

The current project is an input and gameplay prototype. It proves the real-time instrument pipeline before adding songs, charts, richer scoring rules, and polished presentation.

## Current Features

- Bevy `0.20.0-rc.1` application and state management.
- Dedicated instrument worker thread separate from the Bevy gameplay thread.
- CPAL audio input for any number of configured `Strings`/`Voice` instrument slots:
  - Electric guitar or bass through a 1/4-inch-to-USB audio interface.
  - Vocal microphones.
- Any number of `Strings` slots, each with a configured tuning (loaded from the
  `tunings/` library) and a detector profile
  (polyphonic for chords, per-string for bass-style physical strings).
- MIDI input for electronic drums, with any number of `Percussion` slots.
- Audio onset detection with normalized YIN-style pitch estimation.
- Polyphonic FFT pitch tracking, with up to six simultaneous notes.
- Monophonic YIN-style tracking retained for vocals, with note start, sustain, and release events.
- Note-duration events report elapsed playing time in seconds for tracked notes.
- Adaptive low-level onset gating for quieter plucks.
- Confidence-gated pitch estimation: low-confidence YIN reads are rejected instead of reported as notes.
- A dedicated per-string energy tracker for open-string navigation (menu/song selection), independent from the general pitch detector, so a fresh pluck registers even while another string is still ringing. This tracker is menu-navigation only; chart gameplay and fretted notes still use the general YIN detector.
- Pitch-to-lane mapping for strings and vocals.
- MIDI drum note mapping to gameplay lanes.
- Calibration tuners with open-string feedback for strings and nearest-note/cents feedback for voice.
- Voice tuning rejects low-level input and clears the detected note when a confident pitch or sufficient level is no longer present.
- Live audio inputs are monitored through the default output while the application is running. Use headphones or mute nearby speakers to prevent microphone feedback.
- Timing calibration screen with a moving beat target before chart gameplay.
- Home screen with direct access to the live session and setup tools.
- Persistent setup settings stored in `open-band-settings/settings.json`.
- Toggleable live-session input debug window with pitch, string, note, lane, and signal data.
- Basic falling-note highway and keyboard fallback controls.

## Running

Install the system audio and MIDI development libraries required by CPAL and `midir` for your Linux distribution. On Debian or Ubuntu, this commonly includes ALSA development packages:

```bash
sudo apt install libasound2-dev libudev-dev
```

Then run:


To feed a WAV recording through the detector instead of a hardware input, set
slot, or a 4-string default if none is configured):

The application opens on Home. Choose `Live Session` to start playing immediately with saved settings, or choose `Set Up` to configure devices and calibration. Input Setup can still be revisited at any time.

## Controls

### Device Selection
are supported (e.g. two `Strings` slots for guitar and bass).

| Key | Action |
| --- | --- |
| `Up` / `Down` | Move focus between slots |
| `Left` / `Right` | Cycle the focused slot's device (audio devices for `Strings`/`Voice`, MIDI ports for `Percussion`) |
| `N` | Add a new slot (defaults to `Strings`) |
| `X` | Remove the focused slot |
| `K` | Cycle the focused slot's kind (`Strings` -> `Percussion` -> `Voice`) |
| `[` / `]` | Cycle the focused `Strings` slot's tuning (from `tunings/`) or `Percussion` slot's kit (from `kits/`) |
| `P` | Toggle the focused `Strings` slot's detector profile (polyphonic vs. per-string) |
| `Enter` | Continue to calibration |

### Calibration

Calibration lists the currently configured slots; press the number key matching a slot
(`1`-`9`, in configured order) to select it for calibration.
For audio slots, the meter shows raw microphone RMS separately from pitch detection, so
an active input stream is visible even when the tuner cannot identify a note.

| Key | Action |
| --- | --- |
| `1`-`9` | Select a configured instrument slot |
| `Enter` | Accept calibration and return to Set Up |

### Bass Latency Calibration

| Key | Action |
| --- | --- |
| `Space` | Tap with the moving beat line to measure timing offset |
| `Enter` | Accept the latency result and return to Set Up |

### Home and Setup Navigation

The Home screen has four options: `Live Session`, `Songs`, `Editor`, and `Set Up`. Choose with `Up` / `Down` or `1`-`4`, then press `Enter`.

Songs opens the chart browser. Select a chart with `Up` / `Down` and press `Enter`; a five-second count-in starts the chart-driven string highway. Press `Esc` to return to the Songs menu.

Set Up contains the following options:

| Key | Destination |
| --- | --- |
| `1` | Input setup |
| `2` | Tuner |
| `3` | Latency calibration |
| `4` | Recorder |
| `5` | Back to Home |

Each setup tool returns to Set Up when accepted or exited. Returning to Input Setup stops the current input worker before reconnecting the newly selected devices. Press `Esc` from Set Up to return Home, and press `Esc` from the live session to return Home.

### Recorder

The Recorder screen lists configured inputs. Use `Up` / `Down` and `Enter` to arm or
disarm audio inputs, then select `START RECORDING`; press `Enter` on that row again to
stop. Active recordings are mixed to a timestamped mono 16-bit WAV in `recordings/`.
MIDI percussion slots are shown but cannot be armed because they provide note events,
not an audio waveform. Leaving the Recorder screen stops and finalizes an active file.

### Chart Editor

The Editor opens a chart picker listing every chart in `charts/`.

| Key | Action |
| --- | --- |
| `Up` / `Down` | Select a chart |
| `Enter` | Open the selected chart for editing |
| `N` | Create a new chart with an empty default Strings track |
| `Esc` | Back to Home |

Once a chart is open, its track list is shown:

| Key | Action |
| --- | --- |
| `Page Up` / `Page Down` or `Up` / `Down` | Cycle the focused track (Page keys also work in score metadata) |
| `Left` / `Right` | Move the shared position by the active snap interval |
| `V` | Toggle the document-wide sheet/tab preview mode |
| `U` / `O` | Jump to the previous/next event on the focused track, including off-grid events |
| `J` | Cycle among events at the current tick |
| `E` / `I` | Edit the selected event or insert a new event as score JSON |
| `Ctrl+E` | Edit the selected event's notation without changing sounding pitch, onset, or performance |
| `Ctrl+P` | Edit the selected event's performance hints and instrument mapping separately |
| `Ctrl+L` | Edit the focused track's ties, slurs, chord groups, and phrase spans |
| `Ctrl+M` | Edit score navigation markers, tempo text, and expressive spans |
| `Delete` | Remove the selected event and its tie/slur relationships |
| `N` | Add a new track (defaults to `Strings`) |
| `X` | Remove the focused track |
| `K` | Cycle the focused track's kind (`Strings` -> `Percussion` -> `Voice`) |
| `[` / `]` | Cycle the focused track's tuning (`Strings`, from `tunings/`) or kit (`Percussion`, from `kits/`) |
| `,` / `.` | Move the focused track up/down in the list |
| `R` | Rename the focused track (type the new name, `Enter` to confirm, `Esc` to cancel) |
| `M` | Open score metadata for the chart and focused track |
| `Tab` | Cycle the shared snap interval (whole, half, quarter, eighth, sixteenth) |
| `S` | Save the chart (writes to its original file, or `charts/<slugified-title>.json` for a new chart) |
| `Esc` | Back to the chart picker |

In score metadata, `Up` / `Down` selects a field, `Left` / `Right` adjusts numeric and
track metadata, and `Enter` edits text fields or inserts the selected score marker or
expression at the end of the score. `Delete` removes the selected tempo text, rehearsal
mark, navigation marker, or expression. Metadata includes title, resolution, tempo, time
signature, tempo text, rehearsal marks, navigation markers, expressions, clef, key
signature, capo, tuning, kit, and descriptive vocal range. Missing event IDs are assigned
when a chart is saved; existing IDs, explicit rests, voice/staff assignments, relationships,
and performance hints are preserved. Performance hints remain separate from notation fields.
The snap interval is shared by the document and its tick positions are calculated from
resolution and the active time signature; tempo changes do not move the grid.
Mouse-wheel scrolling in sheet mode moves the shared cursor by the current navigation
snap interval: up moves left (earlier), down moves right (later), clamped at tick zero.
Sheet mode shows a staff starting at the shared position, with visible measures expanding
to fill the available window width while keeping beat spacing readable. Click a line or space
to place a note on the configured beat-division grid; hold `Shift` for a sharp or
`Alt` for a flat. In remove mode (`F10` or `PLACE/REMOVE`), with `REST` off, click
the visible notehead to remove that event. No accidental modifier is needed, and
removal works for ledger-line notes below the placement band and off-grid events.
For new pitches outside the sheet placement band, use the tab fret picker.
In percussion tracks, `P` cycles kit pieces and clicks target the
selected piece's lane. The clickable on-screen palette selects duration, dots, triplets,
rests, ties, place/remove, voice, dynamics, articulations, percussion pieces, and the slur
tool. Keyboard shortcuts remain available: `F1`-`F5` for duration, `F6` for dots, `F7` for
triplets, `F8` for rests, `F9` for automatic tie placement, `F10` for place/remove, `F11` for voice,
`F12` for dynamics, `A` for articulations, and `P` for percussion pieces. When `TIE`
is enabled, placing a note connects it to a preceding note of the same pitch, voice,
and staff whose duration ends exactly at the new note's start. Dots and tuplets count
toward that duration. No match means no tie; the new note is not marked as the start
of a future tie. Existing notes are unchanged by toggling the tool. Percussion uses
the same rule with matching kit pieces. The inspector's **Tie to next** field remains
available for forward ties on existing notes. With the slur tool
active, click two existing pitched notes in order to link them by stable event IDs. The sheet
draws beams (including half-length secondary beamlets for isolated sixteenths next
to eighth notes), ties, slurs, tuplets, rests, clefs, key signatures, accidentals, dynamics,
with dotted short notes remaining beamed to contiguous or overlapping neighbors within
the same beat; tie connections, rests, gaps, and beat boundaries break beam groups.
Explicit stable-ID ties and the legacy tied flag prevent beams across the tied pair,
but each endpoint can still beam to neighboring notes on its other side.
Accidentals are shown only on the first note of a tie chain, not repeated on tied
continuations, including across barlines. Stored pitch and spelling remain unchanged.
When no spelling is authored, chromatic pitches use flat equivalents in flat keys
and sharp equivalents otherwise. Explicit spelling takes precedence over this fallback.
Tied continuations inherit the first note's displayed spelling so every note in the
tie stays at the same staff position, even if a continuation stores an enharmonic name.
This affects display only; authored spellings and pitches are preserved.
A key signature of `-2` means two flats, not double-flat accidentals.
It also draws
articulations, score markers, and aligned tablature from written pitches. Percussion symbols
follow kit metadata, while vocal lyrics retain syllable and melisma markings. Removal has no
undo; manual visual QA across representative multi-track charts remains recommended.
Use the `+ BARS` and `- BARS` sheet toolbar buttons to adjust note spacing (10-240 pixels per
quarter note). In either sheet or tab mode, use `DIV -` and `DIV +` to choose
1-16 placement divisions per meter beat; both views share the same saved setting
(default 4); the current division count is shown on the buttons. `1` keeps only beat guides,
while finer divisions add progressively darker grey guides and
allow syncopated note placement. Bar lines are brightest, beat lines darker, and subdivisions
fade darker without going black. Density and beat divisions are saved in user settings and
do not modify chart data.
Tab mode renders the focused track vertically with time flowing downward. Bar and beat
guides follow the active time signature. Unlabeled subdivision lines between beats use
the same progressively darker grey colors as the sheet view. Pointer selection and note
placement use the shared beat-division grid, including non-power-of-two divisions.
Scroll the mouse
wheel to traverse time; the view extends to the bottom of the window, and taller windows show
more of the song at the same note scale. Use `+` / `-` or the `ZOOM` toolbar buttons to
change tab zoom density without changing the placement grid. The `SNAP` toolbar button
cycles the legacy interval for time navigation and the resize minimum; `DIV -` / `DIV +`
control pointer placement precision in both views. Click a string to choose a fret from
the tuning-derived fret/note grid.
In place mode, a circle marks the snapped insertion position on the hovered string/lane.
Each note badge has a green duration bar extending downward to its end tick, with
a short end cap. Bars extending beyond the visible window clip at the bottom edge.
The `PLACE/REMOVE` toolbar toggle and `F10` work in tab mode too. In remove mode,
the outline surrounds the hovered note badge; click it to delete only that event,
without choosing a fret or percussion piece. This also works on the `SPAN` rail.
Removal does not start a duration drag, and clicks outside the tab's vertical bounds
do not insert or remove notes. For shared
percussion lanes, select the desired kit piece with `PIECE`, then click that lane again;
lane-spanning pieces use the separate `SPAN` rail. Drag a note's lower endpoint to resize it
to the snap grid, capped before the next note on that string/lane.
#### Score Relationships And Navigation

Notation and performance use keyboard field menus. Select an event with
`U` / `O` (previous/next event) and `J` (stacked event), then open `Ctrl+E` or
`Ctrl+P`. `Ctrl+L` edits relationships for the focused track; `Ctrl+M` edits
navigation across the document. These shortcuts work in either view and in metadata mode.

| Menu Key | Action |
| --- | --- |
| `Up` / `Down` | Select a field; the menu pages automatically |
| `Left` / `Right` | Cycle choices, toggle flags, or adjust numbers |
| `Enter` | Cycle a choice or open a plain text/number field |
| `Delete` | Clear an optional field or reset a required field |
| `Home` / `End` | Jump to the first field / Apply changes |
| `Page Up` / `Page Down` | Move one page without switching tracks |
| `Esc` | Cancel a value entry, or discard the menu draft and close |

Choose **Apply changes** and press `Enter` to commit the draft. Invalid edits stay
open with an error and leave the document unchanged. Press `S` after closing the
menu to save. Existing chart warnings do not block an unrelated edit.

Notation (`Ctrl+E`) offers note length, dynamic, dots, voice/staff, tie, written
spelling, custom tuplet counts, stem direction, individual articulation toggles,
grace kind/slash/principal/order, and chord ID. Spelling choices are generated from
the existing MIDI pitch: choose D-flat 4 instead of C-sharp 4 without changing the sound.
Grace principals are selected from existing event IDs, not typed as JSON.
Deleting either tuplet field clears the tuplet; deleting a grace field clears the
grace setting. Use `Ctrl+L` to assign all chord members in one transaction.

To add `mf` to an existing low bass note: select it with `U` / `O`, open `Ctrl+E`,
select **Dynamic**, use `Left` / `Right` until it reads `mf`, then press `End` and
`Enter` to apply. No staff click or JSON editing is needed.

Relationships (`Ctrl+L`) replace the displayed arrays. Ties and slurs use stable
event IDs, which are shown in the event preview and full event editor. Ties require
matching pitches and contiguous timing; both relations stay within a voice/staff.
Chord members must share tick, duration, voice, and staff:

```json
{
  "ties":[{"from":"event-000001","to":"event-000003"}],
  "slurs":[],
  "chords":[{"id":"chord-a","events":["event-000001","event-000002"]}],
  "phrases":[{"start":0,"duration_ticks":3840}],
  "star_power_phrases":[]
}
```

Phrase spans edit the existing track phrases by array index, preserving their text
and notes. Additional spans create empty phrases. A span containing notes cannot
be deleted through this editor. Lyric-verse phrase notes retain their existing event
editing path. Removing chord or relation entries clears those relationships only.

Saved `performance` objects omit unset (`null`) hints and disabled (`false`) flags;
missing hint fields load as unset or false, preserving compatibility with older charts.
Performance (`Ctrl+P`) offers attack, transition, palm mute, harmonic, vibrato,
bend amount/release/curve points, and trill target fields. String tracks also show
preferred string and fret choices consistent with tuning, capo, and written pitch.
Strings are displayed one-based; the saved hints remain zero-based. Changing either
guidance choice updates its counterpart. Fret is derived rather than separately stored.
Percussion tracks offer kit piece, roll/droll targets, dynamics, grace, technique,
sticking, and hi-hat choices. Choosing a roll clears droll and vice versa.
**Add bend point** appends a point; point offsets and amounts can be edited separately.
Deleting a point field removes that point; deleting a main bend field clears the bend.
**Clear all hints** clears the draft hints without removing the percussion piece.
Edits preserve hidden and unrelated hints, written pitch, and rhythm.

Relationships (`Ctrl+L`) and navigation (`Ctrl+M`) still use JSON text entry.
Navigation replaces `structure`, `tempo_text`, and `expressions` using
the existing chart schema. Edit marker ticks, repeat counts, numbered endings,
segno/coda/fine, da capo/dal segno, rehearsal labels, and expressive spans directly.
Removing an array entry removes that marker or expression. Note timing, tempo-map
entries, and time signatures are left unchanged. Press `S` after committing to save.

In the event JSON editor, use `Left` / `Right`, `Home` / `End`, `Backspace`, and `Delete`
to edit text, `Enter` to commit, or `Esc` to cancel. `Ctrl+A` clears the text. Invalid JSON
stays open for correction. Editing preserves an event's ID; insertion creates a new one.
Inserted events go into the track's main note list. There is currently no undo for removal.

Saving runs validation (unplayable notes, missing tuning/kit, unknown percussion pieces) and reports any warnings in the status line without blocking the save. Graphical tab placement is not yet implemented.

## Saved Settings

When Input Setup is accepted, Open Band saves the full list of configured instrument
slots (kind, device, tuning, detector profile). Sheet density (pixels per beat) and sheet beat
divisions are also saved here as user preferences, independently of chart files. Audio settings use CPAL device IDs
such as `alsa:...`; MIDI settings use a deterministic `midi:<index>` port key. The setup
screen shows both the friendly device name and concrete identifier. The settings file is
created in the working directory at `open-band-settings/settings.json` and is loaded on
the next launch. Environment variables remain supported as first-run defaults for the
starter slot list when no settings file exists yet.

The gameplay prototype also supports `A S D F G` as lane controls.

## Chart Format

Charts are versioned JSON songs with a shared timeline and one or more instrument tracks.
At startup, Open Band loads every `*.json` file in the working directory's `charts/`
folder in sorted filename order. Invalid files are reported and skipped. If no charts can
be loaded, the embedded starter chart is used as a fallback. The included starter chart is
`charts/open-strings.json`:

```json
{
  "version": 1,
  "title": "Open Strings Study",
  "resolution": 960,
  "tempo_map": [{ "start": 0, "bpm": 90.0 }],
  "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
  "tracks": [
    {
      "name": "Bass",
      "kind": "strings",
      "tuning": { "strings": ["B0", "E1", "A1", "D2", "G2"] },
      "notes": [
        {
          "start": 1440,
          "length": 8,
          "dots": 1,
          "note": 23,
          "performance": {
            "preferred_string": 0,
            "attack": "pluck"
          }
        }
      ]
    }
  ]
}
```

### Shared chart rules

Each track declares a `kind`: `strings`, `percussion`, or `voice`. A song can contain any
number of tracks and any combination of instrument kinds. Event positions and durations
are integer **ticks** against the chart's `resolution` (ticks per quarter note), not raw
beats. Tempo and time-signature changes are keyed by `start` ticks, with a tick-0 entry
expected.

Written tempo text and expressive score ranges are stored separately from the resolved
tempo map:

```json
"tempo_text": [{ "tick": 0, "text": "Andante", "bpm": 80.0 }],
"expressions": [
  {
    "type": "accelerando",
    "start": 0,
    "duration_ticks": 1920,
    "from_bpm": 80.0,
    "to_bpm": 120.0
  },
  { "type": "crescendo", "start": 0, "duration_ticks": 1920 }
]
```

Supported tempo expressions are `accelerando` and `ritardando`; supported dynamic
expressions are `crescendo` and `diminuendo`. Runtime tempo-curve expansion is planned for
Part C, while the score and editor preserve the written markings.

Every event uses `length` (`1`/`2`/`4`/`8`/`16` for whole, half, quarter, eighth, or
sixteenth) and may use `dots`. `tuplet` supports irregular subdivisions such as triplets:
`{ "actual": 3, "normal": 2 }` means three written events in the time of two. `tied: true`
continues the duration into the next event of the same pitch or piece; validation checks
that the next event starts at the expected end and keeps the same pitch or piece. Written
silence can be represented explicitly with `rest: true`; gaps without authored rests are
also inferred by the notation engine independently for each voice and staff. Its layout
preserves exact gap durations, authored dotted and tuplet rests, and whole-measure rests;
the editor's full notation rendering is still planned.

```json
{ "start": 960, "length": 4, "voice": 1, "staff": 1, "rest": true }
```

Events may specify `voice` and `staff` as one-based notation contexts. They default to
`voice: 1` and `staff: 1` when omitted. `chord` is an optional identifier shared by
simultaneous events for notation and editor grouping; chord members are validated to share
onset, duration, voice, and staff. `stem` may be `up`, `down`, or `auto`.

Any event may also use a shared `dynamic` (`ppp`, `pp`, `mp`, `mf`, `f`, `ff`, or `fff`)
and an `articulations` array containing `staccato`, `tenuto`, `marcato`, `accent`,
`fermata`, or `grace`.

Any track may contain `star_power_phrases`, represented as simple
`{ "start": 0, "duration_ticks": 3840 }` range markers.

Score navigation is stored at chart level in `structure`. Markers include `repeat_start`,
`repeat_end` with `times`, numbered `ending_start`/`ending_end`, `segno`, `coda`, `fine`,
`da_capo`, `dal_segno`, and `rehearsal` with a label. The editor preserves these written
markers; runtime repeat and ending expansion is deferred to Part C.

```json
{
    "version": 1,
    "title": "Star Power",
    "resolution": 960,
    "tempo_map": [{ "start": 0, "bpm": 120.0 }],
    "time_signature_map": [{ "start": 0, "numerator": 4, "denominator": 4 }],
    "tracks": [{
        "name": "Bass",
        "kind": "strings",
        "tuning": { "strings": ["E1", "A1", "D2", "G2"] },
        "notes": [],
        "star_power_phrases": [{ "start": 0, "duration_ticks": 3840 }]
    }]
}
```

### Strings

Strings tracks embed a `tuning` with ordered, low-string-first, octave-qualified open-string
notes. Any string count or tuning is supported:

```json
"tuning": { "strings": ["B0", "E1", "A1", "D2", "G2"] }
```

Use `clef` to override the inferred staff clef (`treble`, `bass`, `alto`, or `tenor`).
`key_signature` stores the number of fifths as sharps (positive) or flats (negative), from `-7` through `7`,
and its mode:

```json
"clef": "bass",
"key_signature": { "fifths": -2, "mode": "major" }
```

Pitched events use `note`, either a raw MIDI number or a readable name such as `"C4"`.
When a note name is used, its written spelling is preserved separately from the sounding
MIDI value, so `C#4` and `Db4` remain distinct notation even though both sound as MIDI 61.
MIDI-only notes have no explicit spelling and use renderer defaults. The chart system uses
the tuning and pitch to derive the playable string and fret.

String events may also describe notation and intended performance guidance:

```json
{
  "start": 1440,
  "length": 8,
  "grace": {
    "kind": "acciaccatura",
    "slash": true,
    "principal": "note-42",
    "order": 0
  },
  "note": "E2",
  "performance": {
    "preferred_string": 0,
    "attack": "pluck",
    "transition": "slide",
    "bend": { "semitones": 2.0, "release": true },
    "motion": { "kind": "trill", "target": "F#2" }
  }
}
```

- `performance.preferred_string`: optional zero-based preferred-string index.
- `performance.attack`: `pluck` or `tap`; describes how the note starts.
- `performance.transition`: `hammer_on`, `pull_off`, or `slide`; describes how the note connects
  from the preceding note.
- `performance.bend`: a simple bend amount in semitones, or a time-shaped curve. Curve points use a
  normalized `offset` from `0.0` to `1.0`:

  ```json
  "bend": { "points": [
    { "offset": 0.0, "semitones": 0.0 },
    { "offset": 0.5, "semitones": 2.0 },
    { "offset": 1.0, "semitones": 0.0 }
  ] }
  ```

  The simple form may also use `semitones` and `release: true` to return toward the
  original pitch.
- `performance.motion`: currently supports `trill`, alternating the current note and its `target`.
- `stem`: optional written stem direction, `up`, `down`, or `auto`.
- `grace`: `acciaccatura` or `appoggiatura`, optionally slashed, linked to a principal event
  ID and ordered within a grace group. Grace events are pre-beat notation data and do not
  consume ordinary beat duration.

These fields preserve `note` as the event's pitch and are ready for notation rendering.
Gameplay playback and scoring for the techniques are not implemented yet. The current
fret/note label mode is configured by `CHART_NOTE_DISPLAY` in `src/domain.rs` and supports
`Fret`, `Note`, or `Both`.

Additional string notation/performance metadata includes:

- `grace`: `{ "kind": "acciaccatura" | "appoggiatura", "slash": true | false }`.
- Track-level `capo`: a fret number from `0` through `24`.
- `performance.harmonic`: `natural`, `artificial`, or `pinch`.
- `performance.palm_mute`: boolean.
- `performance.vibrato`: `normal`, `wide`, or `narrow`.

These fields describe the written/tab part and optional playing guidance only; gameplay and
visual rendering are deferred.

The track-level `capo` applies to tablature display and score interpretation; it does not
alter the stored sounding pitch. Voice and staff numbers can be included alongside these fields when a track contains
independent notation voices or multiple staves:

```json
{
  "start": 960,
  "length": 8,
  "voice": 2,
  "staff": 1,
  "note": "Db4"
}
```

### Percussion

Percussion tracks embed a `kit` with a lane count and named pieces. Each piece has a
`trigger`, plus either a shared `lane` or a full-width `lane_span` color. `symbol`
distinguishes pieces sharing a lane, such as a tom and cymbal.

Percussion events reference a kit piece by name. They can also use the shared `dynamic`
and `articulations` fields described above, in addition to percussion-specific hints inside
`performance`:

```json
{ "start": 960, "length": 4, "piece": "snare",
  "performance": { "percussion_dynamics": "accent" } }
```

- `performance.percussion_dynamics`: optional kit-specific `normal`, `accent`, or `ghost`.
- `performance.roll`: names the ending piece for a single-lane roll. The event's `piece` is the
  starting piece; using the same name describes a same-piece roll.
- `performance.droll`: names the second piece for an alternating double-lane roll between `piece` and
  `droll`.
- `performance.percussion_grace`: `flam` or `drag`.
- `performance.percussion_technique`: `buzz` or `cymbal_choke`.
- `performance.sticking`: `right` or `left`.
- `performance.hi_hat`: `open`, `closed`, or `pedal`.

The kit's `symbol` metadata determines notation roles such as tom, cymbal, hi-hat, and
cross-stick independently of gameplay lanes.

`performance.roll` and `performance.droll` are mutually exclusive and must reference pieces
in the track's kit.
Roll rendering, hit generation, and scoring are not implemented yet.

### Voice

Voice tracks may include a descriptive `vocal_range`:

```json
"vocal_range": { "low": "A2", "high": "A4" }
```

Lyrics and melody are grouped into `phrases`. Each phrase has a start tick, duration,
display text, optional `syllable` boundary (`single`, `begin`, `middle`, or `end`),
`hyphen_after`, `breath_after`, and an optional `melisma: true` marker when one syllable
spans multiple notes. Tracks may also provide `lyric_verses` with numbered verse phrases.

```json
{
  "start": 1920,
  "duration_ticks": 1920,
  "text": "Hello",
  "syllable": "single",
  "hyphen_after": false,
  "breath_after": false,
  "melisma": false,
  "notes": [
    { "start": 1920, "length": 2, "note": "C4" }
  ]
}
```

```json
"lyric_verses": [
  { "number": 1, "label": "Verse 1", "phrases": [] }
]
```

The range is descriptive only, and vocal pitch comparison against phrase targets is not
implemented yet.

Press `F3` during the live session to show or hide the input debug window. For strings
inputs, it reports the estimated frequency, nearest string, pitch, and cents offset.
The panel also reports signal level, adaptive noise floor, lane, and event count. The
noise-floor value is the detector's current estimate of the background input level.

## Input Device Configuration

Inputs are configured from `Set Up` -> `Input Setup` as a dynamic list of instrument
slots (see Device Selection controls above). Environment variables preselect devices for
the four starter slots (`Strings` x2, `Percussion`, `Voice`) shown the first time Input
Setup runs, before any settings have been saved:

```bash
BAND_HERO_GUITAR_DEVICE="USB Audio" \
BAND_HERO_BASS_DEVICE="USB Audio" \
BAND_HERO_BASS_STRINGS=5 \
BAND_HERO_VOCAL_DEVICE="Microphone" \
BAND_HERO_MIDI_DEVICE="Drum Controller" \
cargo run
```

Available variables:

- `BAND_HERO_GUITAR_DEVICE`: audio input for the first (guitar-like, polyphonic) `Strings` slot.
- `BAND_HERO_BASS_DEVICE`: audio input for the second (bass-like, per-string) `Strings` slot.
- `BAND_HERO_BASS_STRINGS`: string count for that slot; use `4` or `5`, defaults to `4`.
- `BAND_HERO_VOCAL_DEVICE`: audio input for the `Voice` slot.
- `BAND_HERO_MIDI_DEVICE`: MIDI input for the `Percussion` slot.

These only apply to the starter slots on first run; slots added or removed afterward, and
all slots once a settings file exists, are configured entirely from the Input Setup screen.
Saved device identifiers are preferred on later launches. MIDI requires an available MIDI
input port. The worker thread reports unavailable devices in the terminal and continues
with whichever inputs opened successfully.

## Tuning Library

`Strings` slots select a tuning (used for real per-string lane assignment, the live
per-string tuner, and menu navigation) from the `tunings/` directory: one JSON file per
tuning, each shaped like:

```json
{ "name": "5-String Bass (BEADG)", "strings": ["B0", "E1", "A1", "D2", "G2"] }
```

`strings` is ordered low string first, with octave-qualified note names. At startup, Open
Band loads every `*.json` file in the working directory's `tunings/` folder in sorted
filename order; invalid files are reported and skipped. If the directory is missing or
empty, four embedded defaults are used (4-string bass, 5-string bass, standard guitar,
7-string guitar). Selecting a tuning in Input Setup resolves and copies it into the saved
`InstrumentSlot`, so a saved settings file never depends on the `tunings/` directory still
existing or being unchanged. Add a new tuning by dropping another `*.json` file (any
filename) into `tunings/`.

The detector accepts quieter bass plucks than the original fixed-level gate. The bass should still be connected through an audio interface or preamp that presents a clean, non-clipped input signal; an inline amp is only needed if the hardware signal is still too weak or noisy at the interface input.

## Kit Library

`Percussion` slots select a kit (used for real MIDI-note-to-lane mapping instead of a
hardcoded table) from the `kits/` directory: one JSON file per kit, shaped like:

```json
{
  "name": "Standard Rock Kit",
  "lanes": 4,
  "pieces": [
    { "name": "kick", "trigger": "midi:36", "lane_span": "yellow" },
    { "name": "snare", "trigger": "midi:38", "lane": 0, "symbol": "tom" },
    { "name": "tom1", "trigger": "midi:50", "lane": 1, "symbol": "tom" },
    { "name": "hihat", "trigger": "midi:42", "lane": 1, "symbol": "hihat" }
  ]
}
```

Each piece has a `trigger` (`midi:<note>`, the raw MIDI note number that fires it) and
either a `lane` (0-based, shared with other pieces on the same lane and distinguished by
`symbol`) or a `lane_span` color (a piece that isn't pinned to one lane, e.g. a kick). At
startup, Open Band loads every `*.json` file in the working directory's `kits/` folder in
sorted filename order; invalid files are reported and skipped. If the directory is
missing or empty, an embedded "Standard Rock Kit" (kick, snare, 3 toms, hi-hat, crash,
ride across 4 lanes) is used. Selecting a kit in Input Setup resolves and copies it into
the saved `InstrumentSlot`, so a saved settings file never depends on the `kits/`
directory still existing or being unchanged. Add a new kit by dropping another `*.json`
file (any filename) into `kits/`.

## Architecture

```text
CPAL audio callbacks       MIDI callback
(guitar/bass/vocals)       (drums)
          |                    |
          +---- typed events -+
                    |
        dedicated instrument thread
                    |
          std::sync::mpsc channel
                    |
              Bevy main thread
       calibration, notes, scoring, rendering
```

Audio callbacks stay real-time safe: they downmix samples and enqueue bounded audio blocks without running DSP or blocking. The dedicated instrument worker owns onset, YIN, and FFT analysis, then sends normalized `InstrumentEvent` values to Bevy through a channel. Bevy owns the UI, state transitions, falling notes, scoring, diagnostics, and gameplay timing. The input worker is started from saved settings at launch and is safely stopped and replaced when Input Setup is confirmed.

## Source Documentation

`src/main.rs` is the crate entry point. `src/app.rs` owns Bevy app construction and system registration. `src/state.rs` owns top-level application states. `src/domain.rs` owns chart data, chart screen state, and chart UI markers. `src/audio.rs` owns pure pitch estimation and pitch-to-lane mapping. `src/detector.rs` owns stateful audio detectors. `src/input.rs` owns worker lifecycle and device callbacks. `src/settings.rs` owns persisted settings and device enumeration. `src/setup.rs` owns device selection and calibration. `src/navigation.rs` owns instrument-driven navigation. `src/chart.rs` owns chart screens. `src/menu.rs` owns Home and Set Up menus. `src/gameplay.rs` owns live-session rendering, diagnostics, note motion, and scoring. `AGENTS.md` is the concise contributor map with ownership boundaries and validation commands.

## Current Limitations

- Audio events are generated from detected playing onsets, not from a song chart.
- Pitch detection uses a normalized period search and is more stable across signal levels, but it can still produce errors with heavy noise, distortion, chords, harmonics, and vocals.
- Bass lanes now choose the nearest physical string by pitch (`B-E-A-D-G` for five-string and `E-A-D-G` for four-string), but pitch alone cannot distinguish the same note played on different frets and strings.
- The per-string open-note detector that improves bass menu navigation has not been implemented for chart gameplay; fretted notes during gameplay still rely on the general YIN pitch detector.
- Guitar events still use pitch bands rather than string and fret recognition.
- Strum direction is not detected.
- MIDI drum mapping is a small General MIDI-style mapping and is not configurable yet.
- Vocal gameplay does not yet compare pitch against lyric or melody targets.
- The calibration screen displays signal activity and instrument-specific tuner feedback, and persists the selected devices, bass mode, and accepted latency result; it does not yet save gain, noise-floor, or pitch calibration values.
- The gameplay highway is still a visual and keyboard-test prototype.

## Likely Next Steps

### 1. Make calibration richer

- Measure and store each input's noise floor and gain.
- Add input latency measurement and compensation.
- Add a calibration profile for guitar, four-string bass, five-string bass, drums, and vocals.
- Persist gain, noise-floor, and pitch calibration profiles alongside the existing settings file.

### 2. Improve audio analysis

- Extend confidence-gated pitch estimation (already used for bass) to guitar and vocals.
- Extend the per-string energy tracker used for bass open-string navigation to fretted notes and chart gameplay.
- Add separate tuning ranges for guitar, four-string bass, and five-string bass.
- Detect pick attacks and sustain separately.
- Add an optional noise gate and input gain controls.
- Consider a dedicated DSP crate or worker pool once analysis becomes more expensive.

### 3. Build chart-driven gameplay

- Define a song chart format containing note time, duration, lane, instrument, and difficulty.
- Add a song clock driven by audio playback rather than by event arrival.
- Match detected input against chart targets with configurable timing windows.
- Add sustain notes, chords, hammer-ons, pull-offs, slides, bends, and vocal phrases.
- Separate live instrument events from chart notes so events judge gameplay instead of creating notes spontaneously.

### 4. Add instrument-specific gameplay

- Guitar: map pitch and attack to strings/frets and support tuning calibration.
- Bass: support separate four-string and five-string ranges and tunings.
- Drums: make MIDI note mappings user-configurable and support velocity-sensitive scoring.
- Vocals: add lyric display, target melody curves, pitch confidence, and microphone monitoring.
- Add an optional controller or MIDI guitar path for strum direction and fret input.

### 5. Improve the player experience

- Add profile management around the existing persistent settings file.
- Add audio playback, song loading, pause, restart, and practice mode.
- Add visual feedback for hit quality, combo, multiplier, and per-instrument score.
- Add diagnostics showing device sample rate, channel count, buffer size, and measured latency.

### 6. Test the real-time path

- Add more unit tests for pitch-to-lane mapping and MIDI drum mapping.
- Add deterministic tests for onset detection using recorded sample buffers.
- Record and replay audio input for regression testing.
- Test multiple sample formats, sample rates, buffer sizes, and disconnected devices.
- Test the Linux audio backend with the actual USB interface and microphone hardware.

## Development Checks

```bash
cargo check
cargo run
```

The repository has focused unit tests for pitch estimation and bass string lanes, but no automated gameplay test suite. Hardware validation requires connected audio and MIDI devices.
