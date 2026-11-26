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
/// `duration_override` preserves nonstandard gaps and whole measures exactly; a
/// consumer must not interpret such a duration as the nominal value alone.
///
/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Rest {
    pub(crate) start: u32,
    pub(crate) voice: u8,
    pub(crate) staff: u8,
    pub(crate) value: NoteValue,
    pub(crate) dots: u8,
    pub(crate) tuplet: Option<Tuplet>,
    pub(crate) explicit: bool,
    pub(crate) duration_override: Option<u32>,
    pub(crate) whole_measure: bool,
}

impl Rest {
    /// Returns the exact laid-out rest duration, including dots and tuplets.
    ///
    /// AI_CODE
    pub(crate) fn duration_ticks(&self, resolution: u32) -> u32 {
        if let Some(duration) = self.duration_override {
            return duration;
        }
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

/// Layout for one track, spanning the score and independent of a rendering framework.
/// Every relationship and lyric note index refers to `symbols`; each symbol's source
/// identifies the original event, even when input is not in chronological order.
///
/// AI_CODE
pub(crate) struct NotationLayout {
    pub(crate) clef: Option<Clef>,
    pub(crate) key_signature: Option<KeySignature>,
    pub(crate) measures: Vec<Measure>,
    /// Each inner `Vec` contains indices into `symbols` in tie order.
    pub(crate) ties: Vec<Vec<usize>>,
    pub(crate) slurs: Vec<NotationRelation>,
    /// Each inner `Vec` is a run of `symbols` indices (eighth/sixteenth) beamed
    /// together within one beat.
    pub(crate) beams: Vec<Vec<usize>>,
    /// Runs of notes sharing the same tuplet ratio.
    pub(crate) tuplets: Vec<Vec<usize>>,
    pub(crate) chord_groups: Vec<ChordGroup>,
    pub(crate) symbols: Vec<NotationSymbol>,
    pub(crate) rests: Vec<Rest>,
    pub(crate) lyrics: Vec<LyricLayout>,
    pub(crate) phrase_spans: Vec<Phrase>,
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
fn tuplet_groups(notes: &[ChartEvent], resolution: u32) -> Vec<Vec<usize>> {
    let mut groups = Vec::new();
    let mut lanes = notes
        .iter()
        .map(|note| (note.staff, note.voice))
        .collect::<Vec<_>>();
    lanes.sort_unstable();
    lanes.dedup();
    for (staff, voice) in lanes {
        let mut current = Vec::new();
        let mut ratio: Option<Tuplet> = None;
        let mut previous_end = None;
        let mut previous_start = None;
        for (index, note) in notes
            .iter()
            .enumerate()
            .filter(|(_, note)| note.staff == staff && note.voice == voice)
        {
            if note.tuplet.is_some()
                && note.tuplet == ratio
                && (previous_end == Some(note.start) || previous_start == Some(note.start))
            {
                current.push(index);
            } else {
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
            previous_start = Some(note.start);
            previous_end = Some(note.start.saturating_add(note.duration_ticks(resolution)));
        }
        if current.len() > 1 {
            groups.push(current);
        }
    }
    groups.sort_by_key(|group| group[0]);
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
fn measure_containing(measures: &[Measure], tick: u32) -> Option<&Measure> {
    measures
        .iter()
        .find(|measure| tick >= measure.start_tick && tick < measure.end_tick)
}

/// Groups consecutive eighth/sixteenth notes that fall within the same beat into beams.
/// A rest, a longer note, or a beat boundary breaks the run.
///
/// AI_CODE
#[cfg(test)]
fn beam_groups(notes: &[ChartEvent], resolution: u32, measures: &[Measure]) -> Vec<Vec<usize>> {
    beam_groups_without_ties(notes, resolution, measures, &[])
}

/// Break beams across resolved tie connections while allowing beams on either side. AI_CODE
fn beam_groups_without_ties(
    notes: &[ChartEvent],
    resolution: u32,
    measures: &[Measure],
    ties: &[Vec<usize>],
) -> Vec<Vec<usize>> {
    let mut groups = Vec::new();
    let mut lanes = notes
        .iter()
        .map(|note| (note.staff, note.voice))
        .collect::<Vec<_>>();
    lanes.sort_unstable();
    lanes.dedup();
    for (staff, voice) in lanes {
        let mut current = Vec::new();
        let mut current_beat: Option<(usize, u32)> = None;
        let mut previous_end = None;
        let mut previous_start = None;

        for (index, note) in notes
            .iter()
            .enumerate()
            .filter(|(_, note)| note.staff == staff && note.voice == voice)
        {
            let beamable = !matches!(note.content, NoteContent::Rest)
                && matches!(note.length, NoteValue::Eighth | NoteValue::Sixteenth);
            let beat = measure_containing(measures, note.start).map(|measure| {
                (
                    measure.index,
                    (note.start - measure.start_tick) / measure.beat_ticks(resolution),
                )
            });
            let continues = beamable
                && beat.is_some()
                && beat == current_beat
                && !current.last().is_some_and(|previous| {
                    ties.iter().any(|chain| chain.windows(2)
                        .any(|pair| pair[0] == *previous && pair[1] == index))
                })
                && (previous_end.is_some_and(|end| end >= note.start)
                    || previous_start == Some(note.start));
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
            previous_start = Some(note.start);
            previous_end = Some(note.start.saturating_add(note.duration_ticks(resolution)));
        }
        if current.len() > 1 {
            groups.push(current);
        }
    }
    groups.sort_by_key(|group| group[0]);
    groups
}

/// Dotted short notes beam to contiguous or overlapping neighbors without spanning silence.
///
/// AI_CODE
#[test]
fn dotted_notes_keep_beams_within_the_beat() {
    let chart: Chart = serde_json::from_str(r#"{
        "version":1,"title":"Dotted beams","resolution":960,
        "tempo_map":[{"start":0,"bpm":100}],
        "time_signature_map":[{"start":0,"numerator":4,"denominator":4}],
        "tracks":[]
    }"#).unwrap();
    let mut notes: Vec<ChartEvent> = serde_json::from_str(r#"[
        {"start":0,"length":8,"dots":1,"note":26},
        {"start":480,"length":16,"note":32}
    ]"#).unwrap();
    let bars = measures(&chart, 3840);
    assert_eq!(beam_groups(&notes, 960, &bars), vec![vec![0, 1]]);
    notes[1].start = 720;
    assert_eq!(beam_groups(&notes, 960, &bars), vec![vec![0, 1]]);
    notes[1].start = 840;
    assert!(beam_groups(&notes, 960, &bars).is_empty());
    notes[1].start = 960;
    assert!(beam_groups(&notes, 960, &bars).is_empty());
    notes[1].start = 480;
    notes.insert(1, serde_json::from_str(r#"{"start":240,"length":16,"rest":true}"#).unwrap());
    assert!(beam_groups(&notes, 960, &bars).is_empty());
}

/// Tie connections break beams but their endpoints can beam to neighboring notes. AI_CODE
#[test]
fn resolved_ties_break_beams() {
    let mut chart: Chart = serde_json::from_str(r#"{
        "version":1,"title":"Tie beams","resolution":960,
        "tempo_map":[{"start":0,"bpm":100}],
        "time_signature_map":[{"start":0,"numerator":4,"denominator":4}],
        "tracks":[{"name":"Voice","kind":"voice","notes":[
            {"id":"a","start":0,"length":16,"note":60},
            {"id":"b","start":240,"length":16,"note":60},
            {"id":"c","start":480,"length":16,"note":62},
            {"id":"d","start":720,"length":16,"note":64}
        ],"ties":[{"from":"a","to":"b"}]}]
    }"#).unwrap();
    let layout = layout_track(&chart, &chart.tracks[0]);
    assert_eq!(layout.ties, vec![vec![0, 1]]);
    assert_eq!(layout.beams, vec![vec![1, 2, 3]]);
    chart.tracks[0].ties.clear();
    chart.tracks[0].notes[0].tied = true;
    let layout = layout_track(&chart, &chart.tracks[0]);
    assert_eq!(layout.ties, vec![vec![0, 1]]);
    assert_eq!(layout.beams, vec![vec![1, 2, 3]]);
    chart.tracks[0].notes[0].tied = false;
    assert_eq!(layout_track(&chart, &chart.tracks[0]).beams, vec![vec![0, 1, 2, 3]]);
    chart.tracks[0].notes[2].content = NoteContent::Pitched { note: 60, spelling: None };
    chart.tracks[0].ties.push(EventRelation { from: "b".into(), to: "c".into() });
    let layout = layout_track(&chart, &chart.tracks[0]);
    assert_eq!(layout.ties, vec![vec![1, 2]]);
    assert_eq!(layout.beams, vec![vec![0, 1], vec![2, 3]]);
    chart.tracks[0].ties.clear();
    chart.tracks[0].notes[1].tied = true;
    assert_eq!(layout_track(&chart, &chart.tracks[0]).beams, vec![vec![0, 1], vec![2, 3]]);
}

/// Fills `[gap_start, gap_end)` with the fewest standard rest symbols, splitting at any
/// measure boundary the gap crosses so no rest spans a barline.
///
/// AI_CODE
fn infer_rests(
    gap_start: u32,
    gap_end: u32,
    resolution: u32,
    measures: &[Measure],
    voice: u8,
    staff: u8,
    tuplets: &[Tuplet],
) -> Vec<Rest> {
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
            let whole_measure = measures
                .iter()
                .any(|measure| measure.start_tick == tick && measure.end_tick == cut);
            let mut candidates = Vec::new();
            for value in REST_VALUES {
                for dots in 0..=2 {
                    for tuplet in std::iter::once(None).chain(tuplets.iter().copied().map(Some)) {
                        let candidate = Rest {
                            start: tick,
                            voice,
                            staff,
                            value,
                            dots,
                            tuplet,
                            explicit: false,
                            duration_override: None,
                            whole_measure: false,
                        };
                        let duration = candidate.duration_ticks(resolution);
                        if duration > 0 && duration <= remaining {
                            candidates.push((duration, candidate));
                        }
                    }
                }
            }
            candidates.sort_by_key(|(duration, rest)| {
                (
                    *duration,
                    rest.tuplet.is_none(),
                    std::cmp::Reverse(rest.dots),
                )
            });
            let (duration, mut rest) = candidates.pop().unwrap_or((
                remaining,
                Rest {
                    start: tick,
                    voice,
                    staff,
                    value: NoteValue::Sixteenth,
                    dots: 0,
                    tuplet: None,
                    explicit: false,
                    duration_override: Some(remaining),
                    whole_measure: false,
                },
            ));
            if whole_measure {
                rest.value = NoteValue::Whole;
                rest.dots = 0;
                rest.tuplet = None;
                rest.duration_override = Some(remaining);
                rest.whole_measure = true;
            }
            let duration = if whole_measure { remaining } else { duration };
            rests.push(rest);
            tick += duration;
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
    let mut events = track
        .notes
        .iter()
        .enumerate()
        .map(|(index, note)| (EventSource::Track(index), note.clone()))
        .collect::<Vec<_>>();
    for (phrase_index, phrase) in track.phrases.iter().enumerate() {
        events.extend(phrase.notes.iter().enumerate().map(|(index, note)| {
            (
                EventSource::Phrase {
                    phrase: phrase_index,
                    index,
                },
                note.clone(),
            )
        }));
    }
    for (verse_index, verse) in track.lyric_verses.iter().enumerate() {
        for (phrase_index, phrase) in verse.phrases.iter().enumerate() {
            events.extend(phrase.notes.iter().enumerate().map(|(index, note)| {
                (
                    EventSource::Verse {
                        verse: verse_index,
                        phrase: phrase_index,
                        index,
                    },
                    note.clone(),
                )
            }));
        }
    }
    events.sort_by_key(|(_, note)| note.start);
    let notes = events
        .iter()
        .map(|(_, note)| note.clone())
        .collect::<Vec<_>>();

    let track_end = chart
        .tracks
        .iter()
        .flat_map(|track| {
            track
                .notes
                .iter()
                .chain(track.phrases.iter().flat_map(|phrase| phrase.notes.iter()))
                .chain(
                    track
                        .lyric_verses
                        .iter()
                        .flat_map(|verse| verse.phrases.iter())
                        .flat_map(|phrase| phrase.notes.iter()),
                )
        })
        .chain(notes.iter())
        .map(|note| {
            note.start
                .saturating_add(note.duration_ticks(chart.resolution))
        })
        .chain(
            chart
                .tracks
                .iter()
                .flat_map(|track| {
                    track.phrases.iter().chain(
                        track
                            .lyric_verses
                            .iter()
                            .flat_map(|verse| verse.phrases.iter()),
                    )
                })
                .map(|phrase| phrase.start.saturating_add(phrase.duration_ticks)),
        )
        .chain(
            chart
                .tracks
                .iter()
                .flat_map(|track| track.star_power_phrases.iter())
                .map(|phrase| phrase.start.saturating_add(phrase.duration_ticks)),
        )
        .max()
        .unwrap_or(0);
    let measures = measures(chart, track_end);

    let ties = relation_ties(&notes, &track.ties, chart.resolution);
    let slurs = slur_relations(&notes, &track.slurs);
    let beams = beam_groups_without_ties(&notes, chart.resolution, &measures, &ties);
    let tuplets = tuplet_groups(&notes, chart.resolution);
    let chord_groups = chord_groups(&notes);
    let symbols = notation_symbols(&events, track, chart.resolution);
    let lyrics = lyric_layout(track, &events);

    let mut lanes = notes
        .iter()
        .map(|note| (note.staff, note.voice))
        .collect::<Vec<_>>();
    lanes.sort_unstable();
    lanes.dedup();
    if lanes.is_empty() {
        lanes.push((1, 1));
    }
    let mut rests = Vec::new();
    for (staff, voice) in lanes {
        let mut occupied = Vec::new();
        let lane_notes = notes
            .iter()
            .filter(|note| note.staff == staff && note.voice == voice)
            .collect::<Vec<_>>();
        let mut tuplets = Vec::new();
        for tuplet in lane_notes.iter().filter_map(|note| note.tuplet) {
            if tuplet.valid() && !tuplets.contains(&tuplet) {
                tuplets.push(tuplet);
            }
        }
        for note in lane_notes {
            if matches!(note.content, NoteContent::Rest) {
                occupied.push((
                    note.start,
                    note.start
                        .saturating_add(note.duration_ticks(chart.resolution)),
                    Some(Rest {
                        start: note.start,
                        voice,
                        staff,
                        value: note.length,
                        dots: note.dots,
                        tuplet: note.tuplet,
                        explicit: true,
                        duration_override: None,
                        whole_measure: false,
                    }),
                ));
            } else {
                occupied.push((
                    note.start,
                    note.start
                        .saturating_add(note.duration_ticks(chart.resolution)),
                    None,
                ));
            }
        }
        occupied.sort_by_key(|(start, _, _)| *start);
        let mut cursor = measures.first().map_or(0, |measure| measure.start_tick);
        for (start, end, explicit_rest) in occupied {
            rests.extend(infer_rests(
                cursor,
                start,
                chart.resolution,
                &measures,
                voice,
                staff,
                &tuplets,
            ));
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
                voice,
                staff,
                &tuplets,
            ));
        }
    }
    rests.sort_by_key(|rest| (rest.start, rest.staff, rest.voice));

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
        phrase_spans: track.star_power_phrases.clone(),
        expressions: chart.expressions.clone(),
        markers: chart.structure.clone(),
        tempo_text: chart.tempo_text.clone(),
    }
}

/// Builds tie chains from stable event IDs, with authored `tied` flags as a fallback.
///
/// AI_CODE
fn relation_ties(
    notes: &[ChartEvent],
    track_relations: &[EventRelation],
    resolution: u32,
) -> Vec<Vec<usize>> {
    let ids = notes
        .iter()
        .enumerate()
        .filter_map(|(index, note)| note.id.as_ref().map(|id| (id.as_str(), index)))
        .collect::<std::collections::HashMap<_, _>>();
    let mut edges = std::collections::BTreeMap::new();
    for relation in track_relations {
        let (Some(&from), Some(&to)) = (
            ids.get(relation.from.as_str()),
            ids.get(relation.to.as_str()),
        ) else {
            continue;
        };
        if from != to {
            edges.insert(from, to);
        }
    }
    for (index, note) in notes.iter().enumerate() {
        if !note.tied || edges.contains_key(&index) || matches!(note.content, NoteContent::Rest) {
            continue;
        }
        let end = note.start.saturating_add(note.duration_ticks(resolution));
        if let Some((next, _)) = notes.iter().enumerate().find(|(_, candidate)| {
            candidate.start == end
                && candidate.staff == note.staff
                && candidate.voice == note.voice
                && match (&note.content, &candidate.content) {
                    (
                        NoteContent::Pitched { note: left, .. },
                        NoteContent::Pitched { note: right, .. },
                    ) => left == right,
                    (
                        NoteContent::Percussive { piece: left },
                        NoteContent::Percussive { piece: right },
                    ) => left == right,
                    _ => false,
                }
        }) {
            edges.insert(index, next);
        }
    }
    let targets = edges
        .values()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let mut chains = Vec::new();
    for start in edges
        .keys()
        .copied()
        .filter(|index| !targets.contains(index))
    {
        let mut chain = vec![start];
        while let Some(&next) = edges.get(chain.last().unwrap()) {
            if chain.contains(&next) {
                break;
            }
            chain.push(next);
        }
        if chain.len() > 1 {
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
fn lyric_layout(track: &ChartTrack, events: &[(EventSource, ChartEvent)]) -> Vec<LyricLayout> {
    let mut phrases = Vec::new();
    for (index, phrase) in track.phrases.iter().enumerate() {
        phrases.push((None, index, phrase));
    }
    for (verse, data) in track.lyric_verses.iter().enumerate() {
        for (index, phrase) in data.phrases.iter().enumerate() {
            phrases.push((Some((verse, data.number)), index, phrase));
        }
    }
    phrases
        .into_iter()
        .map(|(verse, index, phrase)| LyricLayout {
            start: phrase.start,
            duration_ticks: phrase.duration_ticks,
            text: phrase.text.clone(),
            syllable: phrase.syllable,
            melisma: phrase.melisma,
            hyphen_after: phrase.hyphen_after,
            breath_after: phrase.breath_after,
            verse: verse.map(|(_, number)| number),
            notes: events
                .iter()
                .enumerate()
                .filter_map(|(symbol, (source, _))| match (*source, verse) {
                    (EventSource::Phrase { phrase, .. }, None) if phrase == index => Some(symbol),
                    (
                        EventSource::Verse {
                            verse: source_verse,
                            phrase,
                            ..
                        },
                        Some((verse_index, _)),
                    ) if source_verse == verse_index && phrase == index => Some(symbol),
                    _ => None,
                })
                .collect(),
        })
        .collect()
}

/// Identifies the authored event behind a sorted layout symbol.
///
/// AI_CODE
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EventSource {
    Track(usize),
    Phrase {
        phrase: usize,
        index: usize,
    },
    Verse {
        verse: usize,
        phrase: usize,
        index: usize,
    },
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
    pub(crate) source: EventSource,
    pub(crate) start: u32,
    pub(crate) duration: u32,
    pub(crate) value: NoteValue,
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

/// Stable score relationship, expressed as indices into the layout's symbols.
///
/// AI_CODE
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NotationRelation {
    pub(crate) from: usize,
    pub(crate) to: usize,
}

/// A chord group containing symbol indices sharing a chord ID.
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
    pub(crate) verse: Option<u16>,
    pub(crate) notes: Vec<usize>,
}

/// Builds a framework-independent symbol for every authored event.
///
/// AI_CODE
fn notation_symbols(
    notes: &[(EventSource, ChartEvent)],
    track: &ChartTrack,
    resolution: u32,
) -> Vec<NotationSymbol> {
    notes
        .iter()
        .enumerate()
        .map(|(event_index, (source, note))| {
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
                source: *source,
                start: note.start,
                duration: note.duration_ticks(resolution),
                value: note.length,
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
