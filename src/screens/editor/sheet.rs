use super::*;

const STAFF_SPACING: f32 = 18.0;
const STAFF_Y: f32 = -90.0;
const STAFF_MARGIN: f32 = 48.0;
/// AI_CODE
fn key_signature_steps(clef: Clef, fifths: i8) -> Vec<i32> {
    let sharp_steps: &[i32] = match clef {
        Clef::Treble => &[38, 35, 39, 36, 33, 37, 34],
        Clef::Bass => &[24, 21, 25, 22, 19, 23, 20],
        Clef::Alto => &[31, 28, 32, 29, 26, 30, 27],
        Clef::Tenor => &[31, 28, 25, 29, 26, 30, 27],
    };
    let flat_steps: &[i32] = match clef {
        Clef::Treble => &[34, 37, 33, 36, 32, 35, 31],
        Clef::Bass => &[20, 23, 19, 22, 18, 21, 17],
        Clef::Alto => &[27, 30, 26, 29, 25, 28, 24],
        Clef::Tenor => &[27, 30, 26, 29, 25, 28, 24],
    };
    let positions = if fifths < 0 { flat_steps } else { sharp_steps };
    positions
        .iter()
        .copied()
        .take(fifths.unsigned_abs().min(7) as usize)
        .collect()
}

/// Return the accidental implied by the key signature for a diatonic step.
///
/// AI_CODE
fn key_accidental(step: i32, key: Option<KeySignature>) -> i32 {
    let Some(key) = key else { return 0 };
    let degree = step.rem_euclid(7);
    let order: &[i32] = if key.fifths < 0 {
        &[6, 2, 5, 1, 4, 0, 3]
    } else {
        &[3, 0, 4, 1, 5, 2, 6]
    };
    if order
        .iter()
        .take(key.fifths.unsigned_abs().min(7) as usize)
        .any(|candidate| *candidate == degree)
    {
        key.fifths.signum() as i32
    } else {
        0
    }
}
const EDIT_HEIGHT: f32 = STAFF_SPACING * 6.0;
const NATURAL_SEMITONES: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];

/// A four-quarter-note viewport in centered window coordinates.
///
/// AI_CODE
#[derive(Clone, Copy)]
struct SheetViewport {
    left: f32,
    right: f32,
    start: u32,
    span: u32,
}

/// Choose the visible tick span from the available width while preserving readable beat spacing.
///
/// AI_CODE
fn sheet_span(width: f32, resolution: u32, pixels_per_beat: u16) -> u32 {
    let resolution = resolution.max(1);
    let usable_width = (width - STAFF_MARGIN * 2.0).max(0.0);
    let pixels_per_beat =
        f32::from(pixels_per_beat.clamp(SHEET_PIXELS_PER_BEAT_MIN, SHEET_PIXELS_PER_BEAT_MAX));
    let visible_beats = (usable_width / pixels_per_beat).max(4.0);
    ((f64::from(resolution) * f64::from(visible_beats))
        .round()
        .clamp(f64::from(resolution.saturating_mul(4)), f64::from(u32::MAX))) as u32
}

/// Snap a sheet tick to the nearest subdivision in the active meter beat.
///
/// AI_CODE
fn snap_sheet_tick(chart: &Chart, tick: u32, divisions: u8) -> u32 {
    let resolution = chart.resolution.max(1);
    let layouts = measures(chart, tick.saturating_add(1));
    let Some(measure) = layouts
        .iter()
        .find(|measure| tick >= measure.start_tick && tick < measure.end_tick)
    else {
        return tick;
    };
    let beat_ticks = (u64::from(resolution) * 4 / u64::from(measure.denominator.max(1))).max(1);
    let division_count = u64::from(divisions.clamp(1, SHEET_BEAT_DIVISIONS_MAX));
    let relative = u64::from(tick - measure.start_tick);
    let beat_offset = relative / beat_ticks * beat_ticks;
    let beat_start = u64::from(measure.start_tick) + beat_offset;
    (0..=division_count)
        .map(|index| beat_start + beat_ticks * index / division_count)
        .min_by_key(|position| position.abs_diff(u64::from(tick)))
        .unwrap_or(beat_start)
        .min(u64::from(measure.end_tick)) as u32
}

/// Return a greyscale subdivision guide color, darkest at the finest division.
///
/// AI_CODE
fn subdivision_guide_color(index: u8, divisions: u8) -> Color {
    let progress =
        f32::from(index.saturating_sub(1)) / f32::from(divisions.saturating_sub(2).max(1));
    let brightness = 0.34 - progress.clamp(0.0, 1.0) * 0.14;
    Color::srgb(brightness, brightness, brightness)
}

/// Return guide ticks strictly inside one beat for the configured division count.
///
/// AI_CODE
fn beat_subdivision_ticks(beat_start: u32, beat_ticks: u32, divisions: u8) -> Vec<u32> {
    let divisions = divisions.clamp(1, SHEET_BEAT_DIVISIONS_MAX);
    (1..divisions)
        .map(|index| {
            (u64::from(beat_start)
                + u64::from(beat_ticks) * u64::from(index) / u64::from(divisions))
                as u32
        })
        .collect()
}

/// A persistent text annotation positioned in sheet-view coordinates.
///
/// AI_CODE
#[derive(Clone, Debug, PartialEq)]
struct EngravingLabel {
    text: String,
    x: f32,
    y: f32,
    color: Color,
}

/// Index for reconciling sheet annotations without respawning text entities every frame.
///
/// AI_CODE
#[derive(Component)]
pub(crate) struct SheetEngravingLabel(usize);

type SheetLabelQuery<'world, 'state> = Query<
    'world,
    'state,
    (
        Entity,
        &'static SheetEngravingLabel,
        &'static mut Text2d,
        &'static mut Transform,
        &'static mut TextColor,
    ),
>;

/// Convert supported written score metadata into viewport-aligned labels.
///
/// AI_CODE
fn engraving_labels(
    track: &ChartTrack,
    layout: &NotationLayout,
    viewport: SheetViewport,
    percussion: bool,
) -> Vec<EngravingLabel> {
    let mut labels = Vec::new();
    let note_color = Color::srgb(0.93, 0.94, 0.88);
    let marker_color = Color::srgb(0.79, 0.83, 0.70);
    let label = |text: String, x: f32, y: f32, color: Color| EngravingLabel { text, x, y, color };
    let staff_y = |staff: u8, voice: u8| {
        STAFF_Y - f32::from(staff.saturating_sub(1)) * 170.0 + (1.5 - f32::from(voice.min(4))) * 8.0
    };

    if let Some(x) = viewport.x_at(viewport.start) {
        if !percussion {
            let clef_name = match layout.clef.unwrap_or(Clef::Treble) {
                Clef::Treble => "TREBLE CLEF",
                Clef::Bass => "BASS CLEF",
                Clef::Alto => "ALTO CLEF",
                Clef::Tenor => "TENOR CLEF",
            };
            let staff_count = layout
                .symbols
                .iter()
                .map(|symbol| symbol.staff)
                .max()
                .unwrap_or(1)
                .max(
                    layout
                        .rests
                        .iter()
                        .map(|rest| rest.staff)
                        .max()
                        .unwrap_or(1),
                );
            for staff in 1..=staff_count {
                let base = STAFF_Y - f32::from(staff.saturating_sub(1)) * 170.0;
                labels.push(label(clef_name.into(), x + 4.0, base + 54.0, marker_color));
                if let Some(key) = layout.key_signature {
                    let mark = if key.fifths < 0 { "b" } else { "#" };
                    for (index, step) in
                        key_signature_steps(layout.clef.unwrap_or(Clef::Treble), key.fifths)
                            .into_iter()
                            .enumerate()
                    {
                        labels.push(label(
                            mark.into(),
                            x + 105.0 + index as f32 * 8.0,
                            step_y(step, layout.clef.unwrap_or(Clef::Treble)) + base - STAFF_Y,
                            marker_color,
                        ));
                    }
                }
            }
        }
    }

    for (index, measure) in layout.measures.iter().enumerate() {
        if let Some(x) = viewport.x_at(measure.start_tick) {
            let meter_changed = index == 0
                || layout
                    .measures
                    .get(index.saturating_sub(1))
                    .is_some_and(|previous| {
                        previous.numerator != measure.numerator
                            || previous.denominator != measure.denominator
                    });
            if meter_changed {
                labels.push(label(
                    format!("{}/{}", measure.numerator, measure.denominator),
                    x + 55.0,
                    STAFF_Y + 54.0,
                    marker_color,
                ));
            }
            labels.push(label(
                (measure.index + 1).to_string(),
                x + 2.0,
                STAFF_Y + 67.0,
                marker_color,
            ));
        }
    }

    for symbol in &layout.symbols {
        let Some(x) = viewport.x_at(symbol.start) else {
            continue;
        };
        let y = staff_y(symbol.staff, symbol.voice);
        match &symbol.kind {
            NotationSymbolKind::Pitched { pitch, spelling } => {
                let (step, accidental) = pitch_step(*pitch, spelling.as_ref());
                let implied = key_accidental(step, layout.key_signature);
                if accidental != implied {
                    let glyph = match accidental {
                        0 => "nat".to_owned(),
                        value if value > 0 => "#".repeat(value as usize),
                        value => "b".repeat(value.unsigned_abs() as usize),
                    };
                    labels.push(label(
                        glyph,
                        x - 15.0,
                        step_y(step, layout.clef.unwrap_or(Clef::Treble))
                            + staff_y(symbol.staff, symbol.voice)
                            - STAFF_Y,
                        note_color,
                    ));
                }
                if let Some(dynamic) = symbol.dynamic {
                    labels.push(label(
                        format!("{dynamic:?}").to_lowercase(),
                        x - 4.0,
                        y - 43.0,
                        marker_color,
                    ));
                }
                for (index, articulation) in symbol.articulations.iter().enumerate() {
                    labels.push(label(
                        articulation_label(*articulation).into(),
                        x + index as f32 * 10.0,
                        y + 28.0,
                        marker_color,
                    ));
                }
                if symbol.grace.is_some() {
                    labels.push(label("gr".into(), x - 8.0, y + 22.0, marker_color));
                }
                if let Some(tuning) = track.tuning.as_ref()
                    && let Ok(open_notes) = tuning.open_midi()
                {
                    let preferred = super::editor_source_event(track, symbol.source)
                        .and_then(|event| event.performance.as_ref())
                        .and_then(|hints| hints.preferred_string)
                        .filter(|string| {
                            open_notes.get(*string).is_some_and(|open| *open <= *pitch)
                        });
                    let playable = open_notes
                        .iter()
                        .enumerate()
                        .filter_map(|(string, open)| {
                            let fret = i16::from(*pitch)
                                - i16::from(*open)
                                - i16::from(track.capo.unwrap_or(0));
                            (0..=24).contains(&fret).then_some((string, fret))
                        })
                        .min_by_key(|(_, fret)| *fret);
                    let selected = preferred
                        .and_then(|string| {
                            let fret = i16::from(*pitch)
                                - i16::from(*open_notes.get(string)?)
                                - i16::from(track.capo.unwrap_or(0));
                            (0..=24).contains(&fret).then_some((string, fret))
                        })
                        .or(playable);
                    if let Some((string, fret)) = selected {
                        let tab_y = staff_y(symbol.staff, symbol.voice)
                            - 72.0
                            - (open_notes.len().saturating_sub(1 + string) as f32) * STAFF_SPACING;
                        let hint = super::editor_source_event(track, symbol.source)
                            .and_then(|event| event.performance.as_ref())
                            .map(|performance| match performance.transition {
                                Some(NoteTransition::HammerOn) => "H",
                                Some(NoteTransition::PullOff) => "P",
                                Some(NoteTransition::Slide) => "/",
                                None if performance.attack == Some(NoteAttack::Tap) => "T",
                                _ => "",
                            })
                            .unwrap_or_default();
                        labels.push(label(format!("{fret}{hint}"), x, tab_y, note_color));
                    }
                }
            }
            NotationSymbolKind::Percussion {
                piece,
                lane_span,
                symbol,
                ..
            } => {
                let mark = symbol.as_deref().unwrap_or("x");
                labels.push(label(mark.to_owned(), x - 4.0, y, note_color));
                if lane_span.is_some() {
                    labels.push(label(format!("{piece} span"), x + 10.0, y, marker_color));
                } else {
                    labels.push(label(piece.clone(), x + 10.0, y - 10.0, marker_color));
                }
            }
            NotationSymbolKind::Rest => {}
        }
    }
    for group in &layout.tuplets {
        let Some(first) = group.first().and_then(|index| layout.symbols.get(*index)) else {
            continue;
        };
        let Some(tuplet) = first.tuplet else {
            continue;
        };
        let Some(start_x) = viewport.x_at(first.start) else {
            continue;
        };
        let end_tick = group
            .iter()
            .filter_map(|index| layout.symbols.get(*index))
            .map(|symbol| symbol.start)
            .max()
            .unwrap_or(first.start);
        let end_x = viewport.x_at(end_tick).unwrap_or(start_x);
        labels.push(label(
            format!("{}:{}", tuplet.actual, tuplet.normal),
            (start_x + end_x) / 2.0,
            staff_y(first.staff, first.voice) + 38.0,
            marker_color,
        ));
    }
    for rest in &layout.rests {
        if let Some(x) = viewport.x_at(rest.start) {
            if let Some(tuplet) = rest.tuplet {
                labels.push(label(
                    format!("{}:{}", tuplet.actual, tuplet.normal),
                    x + 7.0,
                    staff_y(rest.staff, rest.voice) - 18.0,
                    marker_color,
                ));
            }
            if rest.whole_measure {
                labels.push(label(
                    "whole measure".into(),
                    x,
                    staff_y(rest.staff, rest.voice) - 20.0,
                    marker_color,
                ));
            }
        }
    }

    for lyric in &layout.lyrics {
        if let Some(x) = viewport.x_at(lyric.start) {
            let mut text = lyric.text.clone();
            if lyric.hyphen_after
                || matches!(
                    lyric.syllable,
                    Some(SyllableKind::Begin | SyllableKind::Middle)
                )
            {
                text.push('-');
            }
            if let Some(verse) = lyric.verse {
                text = format!("{verse}. {text}");
            }
            labels.push(label(text, x, STAFF_Y - 245.0, marker_color));
        }
    }

    for tempo in &layout.tempo_text {
        if let Some(x) = viewport.x_at(tempo.tick) {
            let text = tempo.bpm.map_or_else(
                || tempo.text.clone(),
                |bpm| format!("{} ({bpm:.0})", tempo.text),
            );
            labels.push(label(text, x, STAFF_Y + 90.0, marker_color));
        }
    }
    for marker in &layout.markers {
        let (tick, text) = score_marker_label(marker);
        if let Some(x) = viewport.x_at(tick) {
            labels.push(label(text, x + 5.0, STAFF_Y + 74.0, marker_color));
        }
    }
    for expression in &layout.expressions {
        let (start, text) = match expression {
            ScoreExpression::Accelerando { start, .. } => (*start, "accel."),
            ScoreExpression::Ritardando { start, .. } => (*start, "rit."),
            ScoreExpression::Crescendo { start, .. } => (*start, "cresc."),
            ScoreExpression::Diminuendo { start, .. } => (*start, "dim."),
        };
        if let Some(x) = viewport.x_at(start) {
            labels.push(label(text.into(), x, STAFF_Y - 210.0, marker_color));
        }
    }
    for phrase in &layout.phrase_spans {
        if let Some(x) = viewport.x_at(phrase.start) {
            labels.push(label("phrase".into(), x, STAFF_Y - 225.0, marker_color));
        }
    }
    if let Some(tuning) = track.tuning.as_ref()
        && let Ok(open_notes) = tuning.open_midi()
        && let Some(x) = viewport.x_at(viewport.start)
    {
        let staff_count = layout
            .symbols
            .iter()
            .map(|symbol| symbol.staff)
            .max()
            .unwrap_or(1)
            .max(
                layout
                    .rests
                    .iter()
                    .map(|rest| rest.staff)
                    .max()
                    .unwrap_or(1),
            );
        for staff in 1..=staff_count {
            let base = STAFF_Y - f32::from(staff.saturating_sub(1)) * 170.0;
            for (string, midi) in open_notes.iter().enumerate() {
                let y = base
                    - 72.0
                    - (open_notes.len().saturating_sub(1 + string) as f32) * STAFF_SPACING;
                labels.push(label(midi_note_name(*midi), x + 2.0, y, marker_color));
            }
        }
    }
    labels
}

/// Return an octave-qualified note name for a tuning label.
///
/// AI_CODE
fn midi_note_name(midi: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!(
        "{}{}",
        NAMES[(midi % 12) as usize],
        i16::from(midi) / 12 - 1
    )
}

/// Abbreviate an articulation for the compact sheet annotation layer.
///
/// AI_CODE
fn articulation_label(articulation: Articulation) -> &'static str {
    match articulation {
        Articulation::Staccato => ".",
        Articulation::Tenuto => "-",
        Articulation::Marcato => ">",
        Articulation::Accent => ">",
        Articulation::Fermata => "fermata",
        Articulation::Grace => "gr",
    }
}

/// Convert each written navigation marker into a visible sheet label.
///
/// AI_CODE
fn score_marker_label(marker: &ScoreMarker) -> (u32, String) {
    match marker {
        ScoreMarker::RepeatStart { tick } => (*tick, "repeat start".into()),
        ScoreMarker::RepeatEnd { tick, times } => (*tick, format!("repeat x{times}")),
        ScoreMarker::EndingStart { tick, number } => (*tick, format!("ending {number}")),
        ScoreMarker::EndingEnd { tick, number } => (*tick, format!("end ending {number}")),
        ScoreMarker::Segno { tick } => (*tick, "segno".into()),
        ScoreMarker::Coda { tick } => (*tick, "coda".into()),
        ScoreMarker::Fine { tick } => (*tick, "fine".into()),
        ScoreMarker::DaCapo { tick } => (*tick, "D.C.".into()),
        ScoreMarker::DalSegno { tick } => (*tick, "D.S.".into()),
        ScoreMarker::Rehearsal { tick, label } => (*tick, label.clone()),
    }
}

/// Keep generated score labels in sync without allocating new entities every frame.
///
/// AI_CODE
fn reconcile_engraving_labels(
    commands: &mut Commands,
    entities: &mut SheetLabelQuery,
    labels: &[EngravingLabel],
) {
    let existing = entities
        .iter_mut()
        .map(|(entity, marker, _, _, _)| (marker.0, entity))
        .collect::<std::collections::HashMap<_, _>>();
    for (index, label) in labels.iter().enumerate() {
        if let Some(entity) = existing.get(&index)
            && let Ok((_, _, mut text, mut transform, mut color)) = entities.get_mut(*entity)
        {
            text.0.clone_from(&label.text);
            transform.translation = Vec3::new(label.x, label.y, 8.0);
            color.0 = label.color;
        } else {
            commands.spawn((
                Text2d::new(label.text.clone()),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(label.color),
                Transform::from_xyz(label.x, label.y, 8.0),
                SheetEngravingLabel(index),
            ));
        }
    }
    for (entity, marker, _, _, _) in entities.iter() {
        if marker.0 >= labels.len() {
            commands.entity(entity).despawn();
        }
    }
}

/// Remove labels when leaving the sheet view or when the focused track disappears.
///
/// AI_CODE
fn clear_engraving_labels(commands: &mut Commands, entities: &mut SheetLabelQuery) {
    for (entity, _, _, _, _) in entities.iter_mut() {
        commands.entity(entity).despawn();
    }
}

impl SheetViewport {
    /// Convert a visible tick to a horizontal coordinate.
    ///
    /// AI_CODE
    fn x_at(self, tick: u32) -> Option<f32> {
        let offset = tick.checked_sub(self.start)?;
        if offset >= self.span {
            return None;
        }
        Some(self.left + (self.right - self.left) * offset as f32 / self.span as f32)
    }

    /// Keep a score relationship visible at the viewport edge when its endpoint is off-screen.
    ///
    /// AI_CODE
    fn x_at_clamped(self, tick: u32) -> f32 {
        if tick < self.start {
            self.left
        } else if tick >= self.start.saturating_add(self.span) {
            self.right - f32::EPSILON
        } else {
            self.x_at(tick).unwrap_or(self.left)
        }
    }

    /// Convert a horizontal coordinate inside the viewport to an unsnapped tick.
    ///
    /// AI_CODE
    fn tick_at(self, x: f32) -> Option<u32> {
        if !x.is_finite() || x < self.left || x >= self.right || self.right <= self.left {
            return None;
        }
        let offset = ((x - self.left) / (self.right - self.left) * self.span as f32) as u32;
        self.start
            .checked_add(offset.min(self.span.saturating_sub(1)))
    }
}

/// Return the diatonic index of the middle staff line for the given clef.
///
/// AI_CODE
fn middle_step(clef: Clef) -> i32 {
    match clef {
        Clef::Treble => 4 * 7 + 6,
        Clef::Bass => 3 * 7 + 1,
        Clef::Alto => 4 * 7,
        Clef::Tenor => 3 * 7 + 5,
    }
}

/// Convert an absolute diatonic step into a natural MIDI pitch.
///
/// AI_CODE
fn step_to_pitch(step: i32) -> Option<u8> {
    let pitch = (step.div_euclid(7) + 1) * 12 + NATURAL_SEMITONES[step.rem_euclid(7) as usize];
    u8::try_from(pitch).ok().filter(|pitch| *pitch <= 127)
}

/// Find the written step and accidental, defaulting chromatic pitches to sharps.
///
/// AI_CODE
pub(super) fn pitch_step(pitch: u8, spelling: Option<&PitchSpelling>) -> (i32, i32) {
    if let Some(spelling) = spelling {
        let index = match spelling.step.to_ascii_uppercase() {
            'C' => Some(0),
            'D' => Some(1),
            'E' => Some(2),
            'F' => Some(3),
            'G' => Some(4),
            'A' => Some(5),
            'B' => Some(6),
            _ => None,
        };
        if let Some(index) = index {
            let step = i32::from(spelling.octave) * 7 + index;
            if step_to_pitch(step).is_some_and(|natural| {
                i32::from(natural) + i32::from(spelling.alter) == i32::from(pitch)
            }) {
                return (step, i32::from(spelling.alter));
            }
        }
    }
    let octave = i32::from(pitch) / 12 - 1;
    let semitone = i32::from(pitch) % 12;
    let index = NATURAL_SEMITONES
        .iter()
        .rposition(|value| *value <= semitone)
        .unwrap_or(0);
    (
        octave * 7 + index as i32,
        semitone - NATURAL_SEMITONES[index],
    )
}

/// Map a written diatonic step onto the staff.
///
/// AI_CODE
fn step_y(step: i32, clef: Clef) -> f32 {
    STAFF_Y + (step - middle_step(clef)) as f32 * STAFF_SPACING / 2.0
}

/// Map a staff coordinate to a natural pitch within the editable band.
///
/// AI_CODE
fn pitch_at(y: f32, clef: Clef) -> Option<u8> {
    if !y.is_finite() || (y - STAFF_Y).abs() > EDIT_HEIGHT {
        return None;
    }
    let offset = ((y - STAFF_Y) / (STAFF_SPACING / 2.0)).round() as i32;
    step_to_pitch(middle_step(clef) + offset)
}

/// Draw a compact rest marker, distinguishing authored and inferred silence by color.
///
/// AI_CODE
fn draw_rest(gizmos: &mut Gizmos, position: Vec2, value: NoteValue, color: Color) {
    let width = 6.0;
    match value {
        NoteValue::Whole | NoteValue::Half => {
            let offset = if value == NoteValue::Whole { -3.0 } else { 3.0 };
            gizmos.line_2d(
                position + Vec2::new(-width, offset),
                position + Vec2::new(width, offset),
                color,
            );
            gizmos.line_2d(
                position + Vec2::new(-width, offset - 3.0),
                position + Vec2::new(width, offset - 3.0),
                color,
            );
        }
        _ => {
            gizmos.line_2d(
                position + Vec2::new(3.0, 8.0),
                position + Vec2::new(-3.0, -8.0),
                color,
            );
            gizmos.line_2d(
                position + Vec2::new(-3.0, 2.0),
                position + Vec2::new(3.0, 5.0),
                color,
            );
            if value == NoteValue::Sixteenth {
                gizmos.line_2d(
                    position + Vec2::new(-4.0, -3.0),
                    position + Vec2::new(2.0, 0.0),
                    color,
                );
            }
        }
    }
}

/// Draw a compact arched relationship between two notation positions.
///
/// AI_CODE
fn draw_curve(gizmos: &mut Gizmos, start: Vec2, end: Vec2, height: f32, color: Color) {
    if end.x <= start.x {
        return;
    }
    let mut previous = start;
    for segment in 1..=12 {
        let progress = segment as f32 / 12.0;
        let point =
            start.lerp(end, progress) + Vec2::Y * (height * 4.0 * progress * (1.0 - progress));
        gizmos.line_2d(previous, point, color);
        previous = point;
    }
}

/// Render and edit the focused track in a width-responsive sheet viewport.
///
/// AI_CODE
pub(crate) fn sheet_view_system(
    mut commands: Commands,
    mut gizmos: Gizmos,
    mut document: ResMut<EditorDocument>,
    mut tools: ResMut<SheetTools>,
    settings: Res<PersistentSettings>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut labels: SheetLabelQuery,
) {
    if document.view_mode != EditorViewMode::Sheet
        || document.metadata_mode
        || document.text_entry.is_some()
    {
        clear_engraving_labels(&mut commands, &mut labels);
        return;
    }
    let Ok(window) = windows.single() else {
        clear_engraving_labels(&mut commands, &mut labels);
        return;
    };
    let width = window.width();
    if width <= STAFF_MARGIN * 2.0 || window.height() <= EDIT_HEIGHT * 2.0 {
        clear_engraving_labels(&mut commands, &mut labels);
        return;
    }
    let span = sheet_span(
        width,
        document.chart.resolution,
        settings.sheet_pixels_per_beat,
    );
    if span == 0 {
        clear_engraving_labels(&mut commands, &mut labels);
        return;
    }
    let viewport = SheetViewport {
        left: -width / 2.0 + STAFF_MARGIN,
        right: width / 2.0 - STAFF_MARGIN,
        start: document.current_tick,
        span,
    };
    let Some(track) = document.chart.tracks.get(document.track_focus) else {
        clear_engraving_labels(&mut commands, &mut labels);
        return;
    };
    let percussion = track.kind == InstrumentKind::Percussion;
    let mut layout = layout_track(&document.chart, track);
    layout.measures = measures(
        &document.chart,
        viewport.start.saturating_add(viewport.span),
    );
    let labels_data = engraving_labels(track, &layout, viewport, percussion);
    reconcile_engraving_labels(&mut commands, &mut labels, &labels_data);
    let clef = layout.clef.unwrap_or(Clef::Treble);
    let staff_color = Color::srgb(0.55, 0.60, 0.63);
    let note_color = Color::srgb(0.93, 0.94, 0.88);
    let inferred_color = Color::srgb(0.55, 0.65, 0.66);
    let cursor_color = Color::srgb(0.36, 0.83, 0.68);
    let marker_color = Color::srgb(0.79, 0.83, 0.70);

    let lane_count = if percussion {
        track.kit.as_ref().map_or(0, |kit| kit.lanes)
    } else {
        5
    };
    let staff_count = layout
        .symbols
        .iter()
        .map(|symbol| symbol.staff)
        .max()
        .unwrap_or(1)
        .max(
            layout
                .rests
                .iter()
                .map(|rest| rest.staff)
                .max()
                .unwrap_or(1),
        );
    let staff_base_y = |staff: u8| STAFF_Y - f32::from(staff.saturating_sub(1)) * 170.0;
    let note_y = |staff: u8, voice: u8| staff_base_y(staff) + (1.5 - f32::from(voice.min(4))) * 8.0;
    let line_y = |lane: usize| {
        STAFF_Y + (lane_count.saturating_sub(1) as f32 / 2.0 - lane as f32) * STAFF_SPACING
    };
    let chord_head_offset = |index: usize| {
        let Some(symbol) = layout.symbols.get(index) else {
            return 0.0;
        };
        layout
            .chord_groups
            .iter()
            .find(|group| group.events.contains(&index))
            .and_then(|group| {
                let (NotationSymbolKind::Pitched { pitch, spelling }, Some(other)) = (
                    &symbol.kind,
                    group.events.iter().find(|other| **other != index),
                ) else {
                    return None;
                };
                let other = layout.symbols.get(*other)?;
                if other.start != symbol.start {
                    return None;
                }
                let NotationSymbolKind::Pitched {
                    pitch: other_pitch,
                    spelling: other_spelling,
                } = &other.kind
                else {
                    return None;
                };
                let (step, _) = pitch_step(*pitch, spelling.as_ref());
                let (other_step, _) = pitch_step(*other_pitch, other_spelling.as_ref());
                ((step - other_step).abs() == 1).then_some(if step < other_step {
                    -5.0
                } else {
                    5.0
                })
            })
            .unwrap_or(0.0)
    };
    if percussion {
        for line in 0..lane_count {
            let y = line_y(line);
            gizmos.line_2d(
                Vec2::new(viewport.left, y),
                Vec2::new(viewport.right, y),
                staff_color,
            );
        }
    } else {
        for staff in 1..=staff_count {
            let base = staff_base_y(staff);
            for line in 0..5 {
                let y = base + (2.0 - line as f32) * STAFF_SPACING;
                gizmos.line_2d(
                    Vec2::new(viewport.left, y),
                    Vec2::new(viewport.right, y),
                    staff_color,
                );
            }
            if let Some(tuning) = track.tuning.as_ref()
                && let Ok(open_notes) = tuning.open_midi()
            {
                let tab_top = base - 72.0;
                for string in 0..open_notes.len() {
                    let y = tab_top
                        - (open_notes.len().saturating_sub(1 + string) as f32) * STAFF_SPACING;
                    gizmos.line_2d(
                        Vec2::new(viewport.left, y),
                        Vec2::new(viewport.right, y),
                        staff_color,
                    );
                }
            }
        }
    }
    let guide_top = STAFF_Y + STAFF_SPACING * 2.0 + 24.0;
    let guide_bottom = if percussion {
        STAFF_Y - STAFF_SPACING * 2.0
    } else {
        staff_base_y(staff_count) - 105.0
    };
    let beat_divisions = settings
        .sheet_beat_divisions
        .clamp(SHEET_BEAT_DIVISIONS_MIN, SHEET_BEAT_DIVISIONS_MAX);
    let beat_color = Color::srgb(0.38, 0.38, 0.38);
    let bar_color = Color::srgb(0.78, 0.78, 0.78);
    for measure in &layout.measures {
        let beat_ticks = (u64::from(document.chart.resolution.max(1)) * 4
            / u64::from(measure.denominator.max(1)))
        .max(1);
        for beat_index in 0..measure.numerator.max(1) {
            let beat_tick = (u64::from(measure.start_tick) + u64::from(beat_index) * beat_ticks)
                .min(u64::from(measure.end_tick)) as u32;
            if beat_index > 0
                && let Some(x) = viewport.x_at(beat_tick)
            {
                gizmos.line_2d(
                    Vec2::new(x, guide_bottom),
                    Vec2::new(x, guide_top),
                    beat_color,
                );
            }
            for (index, tick) in beat_subdivision_ticks(
                beat_tick,
                beat_ticks.min(u64::from(u32::MAX)) as u32,
                beat_divisions,
            )
            .into_iter()
            .enumerate()
            {
                let subdivision = index as u8 + 1;
                if let Some(x) = viewport.x_at(tick) {
                    gizmos.line_2d(
                        Vec2::new(x, guide_bottom),
                        Vec2::new(x, guide_top),
                        subdivision_guide_color(subdivision, beat_divisions),
                    );
                }
            }
        }
        for tick in [measure.start_tick, measure.end_tick] {
            if let Some(x) = viewport.x_at(tick) {
                gizmos.line_2d(
                    Vec2::new(x, guide_bottom),
                    Vec2::new(x, guide_top),
                    bar_color,
                );
            }
        }
    }
    for marker in &layout.markers {
        match marker {
            ScoreMarker::RepeatStart { tick } | ScoreMarker::RepeatEnd { tick, .. } => {
                if let Some(x) = viewport.x_at(*tick) {
                    let top = STAFF_Y + STAFF_SPACING * 2.0;
                    let bottom = if percussion {
                        STAFF_Y - STAFF_SPACING * 2.0
                    } else {
                        staff_base_y(staff_count) - STAFF_SPACING * 2.0
                    };
                    let direction = if matches!(marker, ScoreMarker::RepeatStart { .. }) {
                        1.0
                    } else {
                        -1.0
                    };
                    gizmos.line_2d(
                        Vec2::new(x + direction * 4.0, bottom),
                        Vec2::new(x + direction * 4.0, top),
                        staff_color,
                    );
                    let dot_y = STAFF_Y;
                    for offset in [-5.0, 5.0] {
                        gizmos.circle_2d(
                            Vec2::new(x + direction * 12.0, dot_y + offset),
                            1.8,
                            staff_color,
                        );
                    }
                }
            }
            _ => {}
        }
    }
    for marker in &layout.markers {
        if let ScoreMarker::EndingStart { tick, number } = marker
            && let Some(end) = layout.markers.iter().find_map(|candidate| match candidate {
                ScoreMarker::EndingEnd {
                    tick: end_tick,
                    number: end_number,
                } if number == end_number && end_tick > tick => Some(*end_tick),
                _ => None,
            })
            && let Some(start_x) = viewport.x_at(*tick)
        {
            let end_x = viewport.x_at_clamped(end);
            let y = STAFF_Y + STAFF_SPACING * 3.0 + 26.0;
            gizmos.line_2d(Vec2::new(start_x, y), Vec2::new(end_x, y), staff_color);
            gizmos.line_2d(
                Vec2::new(start_x, y - 8.0),
                Vec2::new(start_x, y),
                staff_color,
            );
        }
    }
    for (symbol_index, symbol) in layout.symbols.iter().enumerate() {
        let Some(x) = viewport
            .x_at(symbol.start)
            .map(|x| x + chord_head_offset(symbol_index))
        else {
            continue;
        };
        let y = match &symbol.kind {
            NotationSymbolKind::Pitched { pitch, spelling } => {
                let (step, _) = pitch_step(*pitch, spelling.as_ref());
                step_y(step, clef) + note_y(symbol.staff, symbol.voice) - STAFF_Y
            }
            NotationSymbolKind::Percussion { lane, .. } => lane.map_or(STAFF_Y, line_y),
            NotationSymbolKind::Rest => continue,
        };
        if y.abs() > window.height() / 2.0 - 12.0 {
            continue;
        }
        if !percussion {
            let base = staff_base_y(symbol.staff);
            let offset = ((y - base) / STAFF_SPACING).round() as i32;
            for line in 3..=offset.abs() {
                let ledger_y = base + line as f32 * STAFF_SPACING * offset.signum() as f32;
                gizmos.line_2d(
                    Vec2::new(x - 9.0, ledger_y),
                    Vec2::new(x + 9.0, ledger_y),
                    staff_color,
                );
            }
        }
        match &symbol.kind {
            NotationSymbolKind::Percussion {
                symbol: Some(mark),
                lane_span,
                ..
            } if mark.eq_ignore_ascii_case("x") || lane_span.is_some() => {
                gizmos.line_2d(
                    Vec2::new(x - 5.0, y - 5.0),
                    Vec2::new(x + 5.0, y + 5.0),
                    note_color,
                );
                gizmos.line_2d(
                    Vec2::new(x - 5.0, y + 5.0),
                    Vec2::new(x + 5.0, y - 5.0),
                    note_color,
                );
                if lane_span.is_some() {
                    gizmos.line_2d(
                        Vec2::new(x, STAFF_Y + STAFF_SPACING * 2.0),
                        Vec2::new(x, STAFF_Y - STAFF_SPACING * 2.0),
                        note_color,
                    );
                }
            }
            NotationSymbolKind::Percussion {
                symbol: Some(mark), ..
            } if mark.eq_ignore_ascii_case("diamond") => {
                gizmos.line_2d(Vec2::new(x, y + 6.0), Vec2::new(x + 6.0, y), note_color);
                gizmos.line_2d(Vec2::new(x + 6.0, y), Vec2::new(x, y - 6.0), note_color);
                gizmos.line_2d(Vec2::new(x, y - 6.0), Vec2::new(x - 6.0, y), note_color);
                gizmos.line_2d(Vec2::new(x - 6.0, y), Vec2::new(x, y + 6.0), note_color);
            }
            _ => {
                gizmos.circle_2d(Vec2::new(x, y), 5.0, note_color);
            }
        }
        if symbol.value != NoteValue::Whole {
            gizmos.line_2d(
                Vec2::new(x + 5.0, y),
                Vec2::new(x + 5.0, y + 25.0),
                note_color,
            );
        }
        let beamed = layout
            .beams
            .iter()
            .any(|group| group.contains(&symbol_index));
        if matches!(symbol.value, NoteValue::Eighth | NoteValue::Sixteenth) && !beamed {
            gizmos.line_2d(
                Vec2::new(x + 5.0, y + 25.0),
                Vec2::new(x + 12.0, y + 19.0),
                note_color,
            );
            if symbol.value == NoteValue::Sixteenth {
                gizmos.line_2d(
                    Vec2::new(x + 5.0, y + 20.0),
                    Vec2::new(x + 12.0, y + 14.0),
                    note_color,
                );
            }
        }
        for dot in 0..symbol.dots {
            gizmos.circle_2d(
                Vec2::new(x + 10.0 + f32::from(dot) * 5.0, y + 3.0),
                1.5,
                note_color,
            );
        }
    }
    // \\/ AI_CODE \\
    let symbol_position = |index: usize| {
        let symbol = layout.symbols.get(index)?;
        let x = viewport.x_at_clamped(symbol.start) + chord_head_offset(index);
        let y = match &symbol.kind {
            NotationSymbolKind::Pitched { pitch, spelling } => {
                let (step, _) = pitch_step(*pitch, spelling.as_ref());
                step_y(step, clef) + note_y(symbol.staff, symbol.voice) - STAFF_Y
            }
            NotationSymbolKind::Percussion { lane, .. } => lane.map_or(STAFF_Y, line_y),
            NotationSymbolKind::Rest => return None,
        };
        Some(Vec2::new(x, y))
    };
    for group in &layout.chord_groups {
        let positions = group
            .events
            .iter()
            .filter_map(|index| symbol_position(*index))
            .collect::<Vec<_>>();
        if positions.len() > 1 {
            let x = positions.iter().map(|position| position.x).sum::<f32>()
                / positions.len() as f32
                + 5.0;
            let top = positions
                .iter()
                .map(|position| position.y)
                .fold(f32::MAX, f32::min);
            let bottom = positions
                .iter()
                .map(|position| position.y)
                .fold(f32::MIN, f32::max);
            gizmos.line_2d(Vec2::new(x, top), Vec2::new(x, bottom + 25.0), note_color);
        }
    }
    for group in &layout.beams {
        for pair in group.windows(2) {
            let (Some(start), Some(end)) = (symbol_position(pair[0]), symbol_position(pair[1]))
            else {
                continue;
            };
            gizmos.line_2d(
                start + Vec2::new(5.0, 25.0),
                end + Vec2::new(5.0, 25.0),
                note_color,
            );
            if layout.symbols[pair[0]].value == NoteValue::Sixteenth
                || layout.symbols[pair[1]].value == NoteValue::Sixteenth
            {
                gizmos.line_2d(
                    start + Vec2::new(5.0, 20.0),
                    end + Vec2::new(5.0, 20.0),
                    note_color,
                );
            }
        }
    }
    for chain in &layout.ties {
        for pair in chain.windows(2) {
            let (Some(start), Some(end)) = (symbol_position(pair[0]), symbol_position(pair[1]))
            else {
                continue;
            };
            draw_curve(
                &mut gizmos,
                start + Vec2::new(5.0, 0.0),
                end - Vec2::new(5.0, 0.0),
                9.0,
                note_color,
            );
        }
    }
    for relation in &layout.slurs {
        let (Some(start), Some(end)) =
            (symbol_position(relation.from), symbol_position(relation.to))
        else {
            continue;
        };
        draw_curve(
            &mut gizmos,
            start + Vec2::new(5.0, 0.0),
            end - Vec2::new(5.0, 0.0),
            18.0,
            note_color,
        );
    }
    for group in &layout.tuplets {
        let positions = group
            .iter()
            .filter_map(|index| symbol_position(*index))
            .collect::<Vec<_>>();
        let (Some(first), Some(last)) = (positions.first(), positions.last()) else {
            continue;
        };
        let y = positions
            .iter()
            .map(|position| position.y)
            .fold(f32::MIN, f32::max)
            + 35.0;
        gizmos.line_2d(
            Vec2::new(first.x, y - 5.0),
            Vec2::new(first.x, y),
            staff_color,
        );
        gizmos.line_2d(Vec2::new(first.x, y), Vec2::new(last.x, y), staff_color);
        gizmos.line_2d(
            Vec2::new(last.x, y),
            Vec2::new(last.x, y - 5.0),
            staff_color,
        );
    }
    for expression in &layout.expressions {
        let (start, duration, crescendo) = match expression {
            ScoreExpression::Crescendo {
                start,
                duration_ticks,
            } => (*start, *duration_ticks, true),
            ScoreExpression::Diminuendo {
                start,
                duration_ticks,
            } => (*start, *duration_ticks, false),
            ScoreExpression::Accelerando {
                start,
                duration_ticks,
                ..
            } => {
                let end = start.saturating_add(*duration_ticks);
                if let Some(start_x) = viewport.x_at(*start) {
                    let end_x = viewport.x_at_clamped(end);
                    let y = if percussion {
                        STAFF_Y - STAFF_SPACING * 3.0 - 20.0
                    } else {
                        staff_base_y(staff_count) - 125.0
                    };
                    draw_curve(
                        &mut gizmos,
                        Vec2::new(start_x, y),
                        Vec2::new(end_x, y + 8.0),
                        3.0,
                        marker_color,
                    );
                }
                continue;
            }
            ScoreExpression::Ritardando {
                start,
                duration_ticks,
                ..
            } => {
                let end = start.saturating_add(*duration_ticks);
                if let Some(start_x) = viewport.x_at(*start) {
                    let end_x = viewport.x_at_clamped(end);
                    let y = if percussion {
                        STAFF_Y - STAFF_SPACING * 3.0 - 20.0
                    } else {
                        staff_base_y(staff_count) - 125.0
                    };
                    draw_curve(
                        &mut gizmos,
                        Vec2::new(start_x, y + 8.0),
                        Vec2::new(end_x, y),
                        3.0,
                        marker_color,
                    );
                }
                continue;
            }
        };
        let end = start.saturating_add(duration);
        let Some(start_x) = viewport.x_at(start) else {
            continue;
        };
        let end_x = viewport.x_at_clamped(end);
        let y = if percussion {
            STAFF_Y - STAFF_SPACING * 3.0
        } else {
            staff_base_y(staff_count) - 100.0
        };
        let (start_gap, end_gap) = if crescendo { (2.0, 16.0) } else { (16.0, 2.0) };
        gizmos.line_2d(
            Vec2::new(start_x, y - start_gap),
            Vec2::new(end_x, y - end_gap),
            staff_color,
        );
        gizmos.line_2d(
            Vec2::new(start_x, y + start_gap),
            Vec2::new(end_x, y + end_gap),
            staff_color,
        );
    }
    for phrase in &layout.phrase_spans {
        let end = phrase.start.saturating_add(phrase.duration_ticks);
        if let Some(start_x) = viewport.x_at(phrase.start) {
            let end_x = viewport.x_at_clamped(end);
            let y = STAFF_Y - STAFF_SPACING * 3.0 - 20.0;
            gizmos.line_2d(Vec2::new(start_x, y), Vec2::new(end_x, y), marker_color);
        }
    }
    for lyric in &layout.lyrics {
        if !lyric.melisma {
            continue;
        }
        let end = lyric.start.saturating_add(lyric.duration_ticks);
        if let Some(start_x) = viewport.x_at(lyric.start) {
            let end_x = viewport.x_at_clamped(end);
            let y = STAFF_Y - 245.0;
            gizmos.line_2d(Vec2::new(start_x, y), Vec2::new(end_x, y), marker_color);
        }
    }
    // /\\ AI_CODE /\\
    for rest in &layout.rests {
        if let Some(x) = viewport.x_at(rest.start) {
            let color = if rest.explicit {
                note_color
            } else {
                inferred_color
            };
            let position = Vec2::new(x, note_y(rest.staff, rest.voice));
            draw_rest(&mut gizmos, position, rest.value, color);
            for dot in 0..rest.dots {
                gizmos.circle_2d(
                    position + Vec2::new(10.0 + f32::from(dot) * 5.0, 3.0),
                    1.5,
                    color,
                );
            }
        }
    }
    gizmos.line_2d(
        Vec2::new(viewport.left, STAFF_Y - EDIT_HEIGHT),
        Vec2::new(viewport.left, STAFF_Y + EDIT_HEIGHT),
        cursor_color,
    );

    for (key, value) in [
        (KeyCode::F1, NoteValue::Whole),
        (KeyCode::F2, NoteValue::Half),
        (KeyCode::F3, NoteValue::Quarter),
        (KeyCode::F4, NoteValue::Eighth),
        (KeyCode::F5, NoteValue::Sixteenth),
    ] {
        if keyboard.just_pressed(key) {
            tools.duration = value;
        }
    }
    if keyboard.just_pressed(KeyCode::F6) {
        tools.dots = (tools.dots + 1) % 4;
    }
    if keyboard.just_pressed(KeyCode::F7) {
        tools.tuplet = if tools.tuplet.is_some() {
            None
        } else {
            Some(Tuplet {
                actual: 3,
                normal: 2,
            })
        };
    }
    if keyboard.just_pressed(KeyCode::F8) {
        tools.rest = !tools.rest;
    }
    if keyboard.just_pressed(KeyCode::F9) {
        tools.tied = !tools.tied;
    }
    if keyboard.just_pressed(KeyCode::F10) {
        tools.remove = !tools.remove;
    }
    if keyboard.just_pressed(KeyCode::F11) {
        tools.voice = tools.voice % 4 + 1;
    }
    if keyboard.just_pressed(KeyCode::F12) {
        tools.dynamic = match tools.dynamic {
            None => Some(DynamicLevel::Ppp),
            Some(DynamicLevel::Ppp) => Some(DynamicLevel::Pp),
            Some(DynamicLevel::Pp) => Some(DynamicLevel::Mp),
            Some(DynamicLevel::Mp) => Some(DynamicLevel::Mf),
            Some(DynamicLevel::Mf) => Some(DynamicLevel::F),
            Some(DynamicLevel::F) => Some(DynamicLevel::Ff),
            Some(DynamicLevel::Ff) => Some(DynamicLevel::Fff),
            Some(DynamicLevel::Fff) => None,
        };
    }
    if keyboard.just_pressed(KeyCode::KeyP) && percussion {
        if let Some(kit) = track.kit.as_ref()
            && !kit.pieces.is_empty()
        {
            tools.piece_index = (tools.piece_index + 1) % kit.pieces.len();
        }
    }
    if keyboard.just_pressed(KeyCode::KeyA) {
        tools.articulation = match tools.articulation {
            None => Some(Articulation::Staccato),
            Some(Articulation::Staccato) => Some(Articulation::Tenuto),
            Some(Articulation::Tenuto) => Some(Articulation::Marcato),
            Some(Articulation::Marcato) => Some(Articulation::Accent),
            Some(Articulation::Accent) => Some(Articulation::Fermata),
            _ => None,
        };
    }
    if let Some(cursor) = window.cursor_position() {
        let position = Vec2::new(cursor.x - width / 2.0, window.height() / 2.0 - cursor.y);
        if let Some(tick) = viewport.tick_at(position.x) {
            let snapped = snap_sheet_tick(&document.chart, tick, settings.sheet_beat_divisions);
            if let Some(x) = viewport.x_at(snapped) {
                if percussion && lane_count > 0 {
                    let lane = ((lane_count.saturating_sub(1) as f32 / 2.0)
                        - (position.y - STAFF_Y) / STAFF_SPACING)
                        .round() as isize;
                    if lane >= 0
                        && (lane as usize) < lane_count
                        && (position.y - line_y(lane as usize)).abs() <= STAFF_SPACING / 2.0
                    {
                        gizmos.circle_2d(Vec2::new(x, line_y(lane as usize)), 8.0, cursor_color);
                        if mouse.just_pressed(MouseButton::Left) {
                            if tools.rest {
                                sheet_edit_spelled_pitch_at_tick(
                                    &mut document,
                                    snapped,
                                    0,
                                    None,
                                    &tools,
                                );
                            } else {
                                sheet_edit_percussion_at_tick(
                                    &mut document,
                                    snapped,
                                    lane as usize,
                                    &tools,
                                );
                            }
                        }
                    }
                } else if let Some(natural) = pitch_at(position.y, clef) {
                    let y = step_y(pitch_step(natural, None).0, clef);
                    let alteration = i16::from(
                        keyboard.pressed(KeyCode::ShiftLeft)
                            || keyboard.pressed(KeyCode::ShiftRight),
                    ) - i16::from(
                        keyboard.pressed(KeyCode::AltLeft) || keyboard.pressed(KeyCode::AltRight),
                    );
                    let pitch = (i16::from(natural) + alteration).clamp(0, 127) as u8;
                    let step = pitch_step(natural, None).0;
                    let spelling = PitchSpelling {
                        step: ['C', 'D', 'E', 'F', 'G', 'A', 'B'][step.rem_euclid(7) as usize],
                        alter: alteration as i8,
                        octave: step.div_euclid(7) as i8,
                    };
                    gizmos.circle_2d(Vec2::new(x, y), 8.0, cursor_color);
                    if mouse.just_pressed(MouseButton::Left) {
                        if tools.slur {
                            super::sheet_edit_slur(&mut document, snapped, pitch, &mut tools);
                        } else {
                            sheet_edit_spelled_pitch_at_tick(
                                &mut document,
                                snapped,
                                pitch,
                                Some(spelling),
                                &tools,
                            );
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Middle lines, ledger steps, and enharmonic spelling use diatonic coordinates.
    ///
    /// AI_CODE
    #[test]
    fn pitch_coordinates_follow_clefs_and_spelling() {
        assert_eq!(pitch_at(STAFF_Y, Clef::Treble), Some(71));
        assert_eq!(pitch_at(STAFF_Y, Clef::Bass), Some(50));
        assert_eq!(pitch_at(STAFF_Y, Clef::Alto), Some(60));
        assert_eq!(pitch_at(STAFF_Y, Clef::Tenor), Some(57));
        assert_eq!(
            pitch_at(step_y(4 * 7, Clef::Treble), Clef::Treble),
            Some(60)
        );
        let flat = PitchSpelling {
            step: 'D',
            alter: -1,
            octave: 4,
        };
        assert_eq!(pitch_step(61, Some(&flat)), (4 * 7 + 1, -1));
        assert_eq!(pitch_step(61, None), (4 * 7, 1));
        assert_eq!(pitch_at(STAFF_Y + EDIT_HEIGHT + 1.0, Clef::Treble), None);
    }

    /// Key signatures use conventional staff positions for each clef and note spelling.
    ///
    /// AI_CODE
    #[test]
    fn key_signature_positions_and_cancellations_follow_clef() {
        assert_eq!(key_signature_steps(Clef::Treble, 2), vec![38, 35]);
        assert_eq!(key_signature_steps(Clef::Bass, -2), vec![20, 23]);
        assert_eq!(
            key_accidental(
                4 * 7 + 3,
                Some(KeySignature {
                    fifths: 2,
                    mode: KeyMode::Major
                })
            ),
            1
        );
        assert_eq!(
            key_accidental(
                4 * 7 + 1,
                Some(KeySignature {
                    fifths: 2,
                    mode: KeyMode::Major
                })
            ),
            0
        );
    }

    /// Engraving labels preserve written metadata and align lyrics and tablature with time.
    ///
    /// AI_CODE
    #[test]
    fn engraving_labels_cover_score_metadata_lyrics_and_tab() {
        let mut document = super::super::new_chart_document();
        document
            .chart
            .tracks
            .push(super::super::new_track(InstrumentKind::Strings));
        let track = &mut document.chart.tracks[0];
        track.clef = Some(Clef::Treble);
        track.key_signature = Some(KeySignature {
            fifths: 1,
            mode: KeyMode::Major,
        });
        track.phrases.push(VocalPhrase {
            start: 0,
            duration_ticks: 960,
            text: "hello".into(),
            syllable: Some(SyllableKind::Begin),
            melisma: true,
            hyphen_after: true,
            breath_after: false,
            notes: Vec::new(),
        });
        document.chart.tempo_text.push(TempoText {
            tick: 0,
            text: "Allegro".into(),
            bpm: Some(120.0),
        });
        document.chart.structure.push(ScoreMarker::Rehearsal {
            tick: 0,
            label: "A".into(),
        });
        document.chart.expressions.push(ScoreExpression::Crescendo {
            start: 0,
            duration_ticks: 960,
        });
        let mut tools = SheetTools::default();
        tools.dynamic = Some(DynamicLevel::Mf);
        tools.articulation = Some(Articulation::Accent);
        assert!(super::super::sheet_edit_pitch(&mut document, 0, 60, &tools));
        assert!(super::super::sheet_edit_pitch(&mut document, 0, 65, &tools));

        let track = &document.chart.tracks[0];
        let layout = layout_track(&document.chart, track);
        let viewport = SheetViewport {
            left: -100.0,
            right: 100.0,
            start: 0,
            span: 3840,
        };
        let labels = engraving_labels(&document.chart.tracks[0], &layout, viewport, false);
        let text = labels
            .iter()
            .map(|label| label.text.as_str())
            .collect::<Vec<_>>();

        assert!(text.contains(&"TREBLE CLEF"));
        assert!(text.contains(&"#"));
        assert!(text.contains(&"nat"));
        assert!(text.contains(&"mf"));
        assert!(text.contains(&">"));
        assert!(text.contains(&"hello-"));
        assert!(text.contains(&"Allegro (120)"));
        assert!(text.contains(&"A"));
        assert!(text.contains(&"cresc."));
        assert!(text.contains(&"E1"));
    }

    /// The time viewport is bounded, half-open, and preserves its left tick.
    ///
    /// AI_CODE
    #[test]
    fn viewport_maps_time_and_rejects_outside_coordinates() {
        let viewport = SheetViewport {
            left: -100.0,
            right: 100.0,
            start: 480,
            span: 1920,
        };
        assert_eq!(viewport.tick_at(-100.0), Some(480));
        assert_eq!(viewport.tick_at(0.0), Some(1440));
        assert_eq!(viewport.x_at(1440), Some(0.0));
        assert_eq!(viewport.tick_at(100.0), None);
        assert_eq!(viewport.tick_at(-101.0), None);
        assert_eq!(viewport.x_at(2400), None);
        assert_eq!(viewport.x_at(479), None);
    }

    /// Sheet width controls visible measures while keeping the beat scale readable.
    ///
    /// AI_CODE
    #[test]
    fn sheet_span_expands_with_display_width() {
        let standard = sheet_span(1440.0, 960, 120);
        let ultrawide = sheet_span(3440.0, 960, 120);
        assert!(standard >= 960 * 4);
        assert!(ultrawide > standard);
        assert!(ultrawide >= standard * 2);
    }

    /// Persistent density changes the visible score span independently of chart resolution.
    ///
    /// AI_CODE
    #[test]
    fn sheet_density_setting_controls_visible_bars() {
        let width = 2560.0;
        let denser = sheet_span(width, 960, 75);
        let default = sheet_span(width, 960, 120);
        let spacer = sheet_span(width, 960, 210);
        assert!(denser > default);
        assert!(default > spacer);
        assert_eq!(
            sheet_span(width, 960, 10),
            sheet_span(width, 960, SHEET_PIXELS_PER_BEAT_MIN)
        );
        assert_eq!(
            sheet_span(width, 960, 500),
            sheet_span(width, 960, SHEET_PIXELS_PER_BEAT_MAX)
        );
    }

    /// Beat subdivisions drive sheet snapping for even and triplet note positions.
    ///
    /// AI_CODE
    #[test]
    fn sheet_snapping_supports_configured_beat_subdivisions() {
        let document = super::super::new_chart_document();
        assert_eq!(snap_sheet_tick(&document.chart, 100, 4), 0);
        assert_eq!(snap_sheet_tick(&document.chart, 130, 4), 240);
        assert_eq!(snap_sheet_tick(&document.chart, 400, 3), 320);
        assert_eq!(snap_sheet_tick(&document.chart, 800, 1), 960);

        let mut compound = document.chart.clone();
        compound.time_signature_map[0].numerator = 6;
        compound.time_signature_map[0].denominator = 8;
        assert_eq!(snap_sheet_tick(&compound, 300, 2), 240);
    }

    /// One division adds no sub-beat guides; finer subdivisions use neutral progressively darker greys.
    ///
    /// AI_CODE
    #[test]
    fn subdivision_guides_are_optional_and_descend_in_brightness() {
        assert!(beat_subdivision_ticks(0, 960, 1).is_empty());
        assert_eq!(beat_subdivision_ticks(0, 960, 4), vec![240, 480, 720]);
        let colors = (1..4)
            .map(|index| subdivision_guide_color(index, 4).to_srgba())
            .collect::<Vec<_>>();
        assert!(
            colors
                .iter()
                .all(|color| color.red == color.green && color.green == color.blue)
        );
        assert!(colors[0].red > colors[1].red && colors[1].red > colors[2].red);
        assert!(colors[2].red > 0.0);
    }
}
