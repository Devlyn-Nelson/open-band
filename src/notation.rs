use super::*;

/// One measure's tick span and the time signature in effect for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Measure {
    pub(crate) index: usize,
    pub(crate) start_tick: u32,
    pub(crate) end_tick: u32,
    pub(crate) numerator: u8,
    pub(crate) denominator: u8,
}

impl Measure {
    fn beat_ticks(&self, resolution: u32) -> u32 {
        (resolution * 4 / self.denominator.max(1) as u32).max(1)
    }
}

/// An inferred rest; rests are never stored in chart data (see todo.md Decision 3) —
/// they're derived here from the gaps between notes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Rest {
    pub(crate) start: u32,
    pub(crate) value: NoteValue,
}

/// The standard note-value palette, largest first, used to greedily fill rest gaps.
const REST_VALUES: [NoteValue; 5] = [
    NoteValue::Whole,
    NoteValue::Half,
    NoteValue::Quarter,
    NoteValue::Eighth,
    NoteValue::Sixteenth,
];

/// A complete layout for one track: measures, tie chains, beam groups, and inferred
/// rests. Produced independently of any rendering framework — the editor's sheet view
/// and the gameplay overlay (Part C) both render from this same description.
pub(crate) struct NotationLayout {
    pub(crate) clef: Option<Clef>,
    pub(crate) key_signature: Option<KeySignature>,
    pub(crate) measures: Vec<Measure>,
    /// Each inner `Vec` is a chain of `track.notes` indices tied together in sequence.
    pub(crate) ties: Vec<Vec<usize>>,
    /// Each inner `Vec` is a run of `track.notes` indices (eighth/sixteenth) beamed
    /// together within one beat.
    pub(crate) beams: Vec<Vec<usize>>,
    /// Runs of notes sharing the same tuplet ratio.
    pub(crate) tuplets: Vec<Vec<usize>>,
    pub(crate) rests: Vec<Rest>,
}

/// Picks a default clef from a track's kind/tuning; `Strings` uses the lowest string's
/// pitch since `Strings` no longer distinguishes guitar from bass (see todo.md B4).
/// Callers may override this per track once the editor exposes that as a setting.
pub(crate) fn default_clef(track: &ChartTrack) -> Option<Clef> {
    if track.clef.is_some() {
        return track.clef;
    }
    match track.kind {
        InstrumentKind::Percussion => None,
        InstrumentKind::Voice => Some(Clef::Treble),
        InstrumentKind::Strings => {
            let lowest = track
                .tuning
                .as_ref()
                .and_then(|tuning| tuning.open_midi().ok())
                .and_then(|open_midi| open_midi.into_iter().min());
            // Bass tunings bottom out around B0/E1 (23/28); guitar tunings, even extended
            // ones like 7-string (low B1 = 35), sit above this. A pitch threshold is an
            // imperfect proxy now that `Strings` no longer distinguishes guitar from bass,
            // but it's a reasonable default — see todo.md B4 on allowing a per-track override.
            match lowest {
                Some(midi_note) if midi_note < 34 => Some(Clef::Bass),
                _ => Some(Clef::Treble),
            }
        }
    }
}

fn tuplet_groups(notes: &[ChartEvent]) -> Vec<Vec<usize>> {
    let mut groups = Vec::new();
    let mut current = Vec::new();
    let mut ratio: Option<Tuplet> = None;
    for (index, note) in notes.iter().enumerate() {
        if note.tuplet.is_some() && note.tuplet == ratio {
            current.push(index);
            continue;
        }
        if current.len() > 1 {
            groups.push(std::mem::take(&mut current));
        } else {
            current.clear();
        }
        ratio = note.tuplet;
        if ratio.is_some() {
            current.push(index);
        }
    }
    if current.len() > 1 {
        groups.push(current);
    }
    groups
}

/// Builds sequential measures from tick 0 through at least `end_tick`, using whichever
/// `time_signature_map` entry is in effect at each measure's start.
pub(crate) fn measures(chart: &Chart, end_tick: u32) -> Vec<Measure> {
    let mut changes = chart.time_signature_map.clone();
    changes.sort_by_key(|change| change.start);
    if changes.first().map_or(true, |first| first.start != 0) {
        changes.insert(
            0,
            TimeSignatureChange {
                start: 0,
                numerator: 4,
                denominator: 4,
            },
        );
    }

    let mut measures = Vec::new();
    let mut tick = 0u32;
    let mut index = 0usize;
    // A generous safety cap avoids an infinite loop if `end_tick` is somehow unreachable.
    let max_measures = 100_000;
    while tick < end_tick.max(1) && measures.len() < max_measures {
        let change = changes
            .iter()
            .rev()
            .find(|change| change.start <= tick)
            .unwrap_or(&changes[0]);
        let beat_ticks = (chart.resolution * 4 / change.denominator.max(1) as u32).max(1);
        let measure_ticks = beat_ticks * change.numerator.max(1) as u32;
        let end = tick + measure_ticks;
        measures.push(Measure {
            index,
            start_tick: tick,
            end_tick: end,
            numerator: change.numerator,
            denominator: change.denominator,
        });
        tick = end;
        index += 1;
    }
    measures
}

/// Groups consecutive `tied` events into chains, in `track.notes` order.
fn tie_chains(notes: &[ChartEvent]) -> Vec<Vec<usize>> {
    let mut chains = Vec::new();
    let mut current = Vec::new();
    for (index, note) in notes.iter().enumerate() {
        if matches!(note.content, NoteContent::Rest) {
            if !current.is_empty() {
                chains.push(std::mem::take(&mut current));
            }
            continue;
        }
        current.push(index);
        if !note.tied {
            chains.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        chains.push(current);
    }
    chains
}

fn measure_containing(measures: &[Measure], tick: u32) -> Option<&Measure> {
    measures
        .iter()
        .find(|measure| tick >= measure.start_tick && tick < measure.end_tick)
}

/// Groups consecutive eighth/sixteenth notes that fall within the same beat into beams.
/// A rest, a longer note, or a beat boundary breaks the run.
fn beam_groups(notes: &[ChartEvent], resolution: u32, measures: &[Measure]) -> Vec<Vec<usize>> {
    let mut groups = Vec::new();
    let mut current = Vec::new();
    let mut current_beat: Option<(usize, u32)> = None;

    for (index, note) in notes.iter().enumerate() {
        let beamable = matches!(note.length, NoteValue::Eighth | NoteValue::Sixteenth);
        let beat = measure_containing(measures, note.start).map(|measure| {
            (
                measure.index,
                (note.start - measure.start_tick) / measure.beat_ticks(resolution),
            )
        });
        let continues = beamable && beat.is_some() && beat == current_beat;
        if continues {
            current.push(index);
        } else {
            if current.len() > 1 {
                groups.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
            if beamable {
                current.push(index);
                current_beat = beat;
            } else {
                current_beat = None;
            }
        }
    }
    if current.len() > 1 {
        groups.push(current);
    }
    groups
}

/// Fills `[gap_start, gap_end)` with the fewest standard rest symbols, splitting at any
/// measure boundary the gap crosses so no rest spans a barline.
fn infer_rests(gap_start: u32, gap_end: u32, resolution: u32, measures: &[Measure]) -> Vec<Rest> {
    if gap_end <= gap_start {
        return Vec::new();
    }
    let mut cut_points = measures
        .iter()
        .map(|measure| measure.end_tick)
        .filter(|tick| *tick > gap_start && *tick < gap_end)
        .collect::<Vec<_>>();
    cut_points.sort_unstable();
    cut_points.push(gap_end);

    let mut rests = Vec::new();
    let mut segment_start = gap_start;
    for cut in cut_points {
        let mut tick = segment_start;
        while tick < cut {
            let remaining = cut - tick;
            let value = REST_VALUES
                .into_iter()
                .find(|value| value.ticks(resolution) <= remaining)
                .unwrap_or(NoteValue::Sixteenth);
            rests.push(Rest { start: tick, value });
            tick += value.ticks(resolution).max(1);
        }
        segment_start = cut;
    }
    rests
}

/// Produces the full notation layout for one track: measures spanning its notes, tie
/// chains, beam groups, and inferred rests filling the gaps between (and before/after)
/// its notes.
pub(crate) fn layout_track(chart: &Chart, track: &ChartTrack) -> NotationLayout {
    let mut notes = track.notes.clone();
    notes.sort_by_key(|note| note.start);

    let track_end = notes
        .iter()
        .enumerate()
        .map(|(index, note)| note.start + resolved_duration_ticks(&notes, index, chart.resolution))
        .max()
        .unwrap_or(0);
    let measures = measures(chart, track_end);

    let ties = tie_chains(&notes);
    let beams = beam_groups(&notes, chart.resolution, &measures);
    let tuplets = tuplet_groups(&notes);

    // Sounding tie chains and authored rests occupy ranges; infer silence only between them.
    let mut occupied = Vec::new();
    for chain in &ties {
        let Some(&first) = chain.first() else {
            continue;
        };
        let end = notes[first].start + resolved_duration_ticks(&notes, first, chart.resolution);
        occupied.push((notes[first].start, end, None));
    }
    for note in &notes {
        if matches!(note.content, NoteContent::Rest) {
            occupied.push((
                note.start,
                note.start + note.duration_ticks(chart.resolution),
                Some(Rest {
                    start: note.start,
                    value: note.length,
                }),
            ));
        }
    }
    occupied.sort_by_key(|(start, _, _)| *start);
    let mut rests = Vec::new();
    let mut cursor = measures.first().map_or(0, |measure| measure.start_tick);
    for (start, end, explicit_rest) in occupied {
        rests.extend(infer_rests(cursor, start, chart.resolution, &measures));
        if let Some(rest) = explicit_rest {
            rests.push(rest);
        }
        cursor = cursor.max(end);
    }
    if let Some(last_measure) = measures.last() {
        rests.extend(infer_rests(
            cursor,
            last_measure.end_tick,
            chart.resolution,
            &measures,
        ));
    }

    NotationLayout {
        clef: default_clef(track),
        key_signature: track.key_signature,
        measures,
        ties,
        beams,
        tuplets,
        rests,
    }
}
