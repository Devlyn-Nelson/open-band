use super::*;

/// One measure's tick span and the time signature in effect for it.
///
/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Measure {
    pub(crate) index: usize,
    pub(crate) start_tick: u32,
    pub(crate) end_tick: u32,
    pub(crate) numerator: u8,
    pub(crate) denominator: u8,
}

impl Measure {
    /// AI_CODE
    fn beat_ticks(&self, resolution: u32) -> u32 {
        (resolution * 4 / self.denominator.max(1) as u32).max(1)
    }
}

/// A layout rest distinguishes authored rests from rests inferred into silence.
///
/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Rest {
    pub(crate) start: u32,
    pub(crate) value: NoteValue,
    pub(crate) dots: u8,
    pub(crate) tuplet: Option<Tuplet>,
    pub(crate) explicit: bool,
}

impl Rest {
    /// Returns the exact laid-out rest duration, including dots and tuplets.
    ///
    /// AI_CODE
    pub(crate) fn duration_ticks(&self, resolution: u32) -> u32 {
        let mut total = self.value.ticks(resolution);
        let mut addition = total;
        for _ in 0..self.dots {
            addition /= 2;
            total += addition;
        }
        self.tuplet.map_or(total, |tuplet| {
            total.saturating_mul(tuplet.normal as u32) / tuplet.actual.max(1) as u32
        })
    }
}

/// The standard note-value palette, largest first, used to greedily fill rest gaps.
///
/// AI_CODE
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
///
/// AI_CODE
pub(crate) struct NotationLayout {
    pub(crate) clef: Option<Clef>,
    pub(crate) key_signature: Option<KeySignature>,
    pub(crate) measures: Vec<Measure>,
    /// Each inner `Vec` is a chain of `track.notes` indices tied together in sequence.
    pub(crate) ties: Vec<Vec<usize>>,
    pub(crate) slurs: Vec<NotationRelation>,
    /// Each inner `Vec` is a run of `track.notes` indices (eighth/sixteenth) beamed
    /// together within one beat.
    pub(crate) beams: Vec<Vec<usize>>,
    /// Runs of notes sharing the same tuplet ratio.
    pub(crate) tuplets: Vec<Vec<usize>>,
    pub(crate) chord_groups: Vec<ChordGroup>,
    pub(crate) symbols: Vec<NotationSymbol>,
    pub(crate) rests: Vec<Rest>,
    pub(crate) lyrics: Vec<LyricLayout>,
    pub(crate) expressions: Vec<ScoreExpression>,
    pub(crate) markers: Vec<ScoreMarker>,
    pub(crate) tempo_text: Vec<TempoText>,
}

/// Picks a default clef from a track's kind/tuning; `Strings` uses the lowest string's
/// pitch since `Strings` no longer distinguishes guitar from bass (see todo.md B4).
/// Callers may override this per track once the editor exposes that as a setting.
///
/// AI_CODE
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

/// AI_CODE
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
///
/// AI_CODE
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
///
/// AI_CODE
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

/// AI_CODE
fn measure_containing(measures: &[Measure], tick: u32) -> Option<&Measure> {
    measures
        .iter()
        .find(|measure| tick >= measure.start_tick && tick < measure.end_tick)
}

/// Groups consecutive eighth/sixteenth notes that fall within the same beat into beams.
/// A rest, a longer note, or a beat boundary breaks the run.
///
/// AI_CODE
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
///
/// AI_CODE
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
            rests.push(Rest {
                start: tick,
                value,
                dots: 0,
                tuplet: None,
                explicit: false,
            });
            tick += value.ticks(resolution).max(1);
        }
        segment_start = cut;
    }
    rests
}

/// Produces the full notation layout for one track: measures spanning its notes, tie
/// chains, beam groups, and inferred rests filling the gaps between (and before/after)
/// its notes.
///
/// AI_CODE
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

    let ties = relation_ties(&notes, &track.ties);
    let slurs = slur_relations(&notes, &track.slurs);
    let beams = beam_groups(&notes, chart.resolution, &measures);
    let tuplets = tuplet_groups(&notes);
    let chord_groups = chord_groups(&notes);
    let symbols = notation_symbols(&notes, track, chart.resolution);
    let lyrics = lyric_layout(track);

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
                    dots: note.dots,
                    tuplet: note.tuplet,
                    explicit: true,
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
        slurs,
        beams,
        tuplets,
        chord_groups,
        symbols,
        rests,
        lyrics,
        expressions: chart.expressions.clone(),
        markers: chart.structure.clone(),
        tempo_text: chart.tempo_text.clone(),
    }
}

/// Builds tie chains from stable event IDs, with authored `tied` flags as a fallback.
///
/// AI_CODE
fn relation_ties(notes: &[ChartEvent], track_relations: &[EventRelation]) -> Vec<Vec<usize>> {
    let ids = notes
        .iter()
        .enumerate()
        .filter_map(|(index, note)| note.id.as_ref().map(|id| (id.as_str(), index)))
        .collect::<std::collections::HashMap<_, _>>();
    let mut chains = Vec::new();
    let mut used = std::collections::HashSet::new();
    for relation in track_relations {
        let (Some(&from), Some(&to)) = (
            ids.get(relation.from.as_str()),
            ids.get(relation.to.as_str()),
        ) else {
            continue;
        };
        if used.insert(from) && used.insert(to) {
            chains.push(vec![from, to]);
        } else if let Some(chain) = chains
            .iter_mut()
            .find(|chain: &&mut Vec<usize>| chain.contains(&from) || chain.contains(&to))
        {
            if !chain.contains(&from) {
                chain.push(from);
            }
            if !chain.contains(&to) {
                chain.push(to);
            }
            chain.sort_by_key(|index| notes[*index].start);
        }
    }
    let fallback = tie_chains(notes);
    for chain in fallback {
        if chain.len() > 1 && !chains.iter().any(|existing| existing == &chain) {
            chains.push(chain);
        }
    }
    chains
}

/// Resolves stable slur IDs into layout indices.
///
/// AI_CODE
fn slur_relations(notes: &[ChartEvent], relations: &[EventRelation]) -> Vec<NotationRelation> {
    let ids = notes
        .iter()
        .enumerate()
        .filter_map(|(index, note)| note.id.as_ref().map(|id| (id.as_str(), index)))
        .collect::<std::collections::HashMap<_, _>>();
    relations
        .iter()
        .filter_map(|relation| {
            let from = *ids.get(relation.from.as_str())?;
            let to = *ids.get(relation.to.as_str())?;
            Some(NotationRelation { from, to })
        })
        .collect()
}

/// Groups chord members by stable chord ID.
///
/// AI_CODE
fn chord_groups(notes: &[ChartEvent]) -> Vec<ChordGroup> {
    let mut groups = Vec::new();
    let mut ids = std::collections::HashMap::<&str, Vec<usize>>::new();
    for (index, note) in notes.iter().enumerate() {
        if let Some(id) = note.chord.as_deref() {
            ids.entry(id).or_default().push(index);
        }
    }
    let mut entries = ids.into_iter().collect::<Vec<_>>();
    entries.sort_by_key(|(id, _)| *id);
    for (id, events) in entries {
        if events.len() > 1 {
            groups.push(ChordGroup {
                id: id.to_owned(),
                events,
            });
        }
    }
    groups
}

/// Converts authored lyrics into a rendering-independent lyric layout.
///
/// AI_CODE
fn lyric_layout(track: &ChartTrack) -> Vec<LyricLayout> {
    track
        .lyric_verses
        .iter()
        .flat_map(|verse| verse.phrases.iter())
        .chain(track.phrases.iter())
        .map(|phrase| LyricLayout {
            start: phrase.start,
            duration_ticks: phrase.duration_ticks,
            text: phrase.text.clone(),
            syllable: phrase.syllable,
            melisma: phrase.melisma,
            hyphen_after: phrase.hyphen_after,
            breath_after: phrase.breath_after,
        })
        .collect()
}

/// A rendering-independent pitched or percussion symbol at a tick position.
///
/// AI_CODE
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NotationSymbolKind {
    Pitched {
        pitch: u8,
        spelling: Option<PitchSpelling>,
    },
    Percussion {
        piece: String,
        lane: Option<usize>,
        lane_span: Option<String>,
        symbol: Option<String>,
    },
    Rest,
}

/// One symbol in the notation layout.
///
/// AI_CODE
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NotationSymbol {
    pub(crate) event_index: Option<usize>,
    pub(crate) start: u32,
    pub(crate) duration: u32,
    pub(crate) voice: u8,
    pub(crate) staff: u8,
    pub(crate) dots: u8,
    pub(crate) tuplet: Option<Tuplet>,
    pub(crate) chord: Option<String>,
    pub(crate) dynamic: Option<DynamicLevel>,
    pub(crate) articulations: Vec<Articulation>,
    pub(crate) stem: Option<StemDirection>,
    pub(crate) grace: Option<GraceSpec>,
    pub(crate) kind: NotationSymbolKind,
}

/// Stable score relationship, expressed as track-note indices.
///
/// AI_CODE
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NotationRelation {
    pub(crate) from: usize,
    pub(crate) to: usize,
}

/// A chord group containing all track-note indices sharing a chord ID.
///
/// AI_CODE
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChordGroup {
    pub(crate) id: String,
    pub(crate) events: Vec<usize>,
}

/// Vocal lyric content positioned across an event range.
///
/// AI_CODE
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LyricLayout {
    pub(crate) start: u32,
    pub(crate) duration_ticks: u32,
    pub(crate) text: String,
    pub(crate) syllable: Option<SyllableKind>,
    pub(crate) melisma: bool,
    pub(crate) hyphen_after: bool,
    pub(crate) breath_after: bool,
}

/// Builds a framework-independent symbol for every authored event.
///
/// AI_CODE
fn notation_symbols(
    notes: &[ChartEvent],
    track: &ChartTrack,
    resolution: u32,
) -> Vec<NotationSymbol> {
    notes
        .iter()
        .enumerate()
        .map(|(event_index, note)| {
            let kind = match &note.content {
                NoteContent::Pitched {
                    note: pitch,
                    spelling,
                } => NotationSymbolKind::Pitched {
                    pitch: *pitch,
                    spelling: spelling.clone(),
                },
                NoteContent::Percussive { piece } => {
                    let kit_piece = track.kit.as_ref().and_then(|kit| {
                        kit.pieces.iter().find(|candidate| candidate.name == *piece)
                    });
                    NotationSymbolKind::Percussion {
                        piece: piece.clone(),
                        lane: kit_piece.and_then(|piece| piece.lane),
                        lane_span: kit_piece.and_then(|piece| piece.lane_span.clone()),
                        symbol: kit_piece.and_then(|piece| piece.symbol.clone()),
                    }
                }
                NoteContent::Rest => NotationSymbolKind::Rest,
            };
            NotationSymbol {
                event_index: Some(event_index),
                start: note.start,
                duration: resolved_duration_ticks(notes, event_index, resolution),
                voice: note.voice,
                staff: note.staff,
                dots: note.dots,
                tuplet: note.tuplet,
                chord: note.chord.clone(),
                dynamic: note.dynamic,
                articulations: note.articulations.clone(),
                stem: note.stem,
                grace: note.grace.clone(),
                kind,
            }
        })
        .collect()
}
