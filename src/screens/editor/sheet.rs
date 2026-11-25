use super::*;

const STAFF_SPACING: f32 = 18.0;
const STAFF_Y: f32 = -90.0;
const STAFF_MARGIN: f32 = 48.0;
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

/// Render and edit the focused track in a centered, four-beat sheet viewport.
///
/// AI_CODE
pub(crate) fn sheet_view_system(
    mut gizmos: Gizmos,
    mut document: ResMut<EditorDocument>,
    mut tools: ResMut<SheetTools>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
) {
    if document.view_mode != EditorViewMode::Sheet
        || document.metadata_mode
        || document.text_entry.is_some()
    {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let width = window.width();
    if width <= STAFF_MARGIN * 2.0 || window.height() <= EDIT_HEIGHT * 2.0 {
        return;
    }
    let span = document.chart.resolution.saturating_mul(4);
    if span == 0 {
        return;
    }
    let viewport = SheetViewport {
        left: -width / 2.0 + STAFF_MARGIN,
        right: width / 2.0 - STAFF_MARGIN,
        start: document.current_tick,
        span,
    };
    let Some(track) = document.chart.tracks.get(document.track_focus) else {
        return;
    };
    let percussion = track.kind == InstrumentKind::Percussion;
    let layout = layout_track(&document.chart, track);
    let clef = layout.clef.unwrap_or(Clef::Treble);
    let staff_color = Color::srgb(0.55, 0.60, 0.63);
    let note_color = Color::srgb(0.93, 0.94, 0.88);
    let inferred_color = Color::srgb(0.55, 0.65, 0.66);
    let cursor_color = Color::srgb(0.36, 0.83, 0.68);

    let lane_count = if percussion {
        track.kit.as_ref().map_or(0, |kit| kit.lanes)
    } else {
        5
    };
    let line_y = |lane: usize| {
        STAFF_Y + (lane_count.saturating_sub(1) as f32 / 2.0 - lane as f32) * STAFF_SPACING
    };
    for line in 0..lane_count {
        let y = line_y(line);
        gizmos.line_2d(
            Vec2::new(viewport.left, y),
            Vec2::new(viewport.right, y),
            staff_color,
        );
    }
    for measure in &layout.measures {
        for tick in [measure.start_tick, measure.end_tick] {
            if let Some(x) = viewport.x_at(tick) {
                gizmos.line_2d(
                    Vec2::new(x, STAFF_Y - STAFF_SPACING * 2.0),
                    Vec2::new(x, STAFF_Y + STAFF_SPACING * 2.0),
                    staff_color,
                );
            }
        }
    }
    for (symbol_index, symbol) in layout.symbols.iter().enumerate() {
        let Some(x) = viewport.x_at(symbol.start) else {
            continue;
        };
        let (y, accidental) = match &symbol.kind {
            NotationSymbolKind::Pitched { pitch, spelling } => {
                let (step, accidental) = pitch_step(*pitch, spelling.as_ref());
                (step_y(step, clef), accidental)
            }
            NotationSymbolKind::Percussion { lane, .. } => (lane.map_or(STAFF_Y, line_y), 0),
            NotationSymbolKind::Rest => continue,
        };
        if y.abs() > window.height() / 2.0 - 12.0 {
            continue;
        }
        if !percussion {
            let offset = ((y - STAFF_Y) / STAFF_SPACING).round() as i32;
            for line in 3..=offset.abs() {
                let ledger_y = STAFF_Y + line as f32 * STAFF_SPACING * offset.signum() as f32;
                gizmos.line_2d(
                    Vec2::new(x - 9.0, ledger_y),
                    Vec2::new(x + 9.0, ledger_y),
                    staff_color,
                );
            }
        }
        gizmos.circle_2d(Vec2::new(x, y), 5.0, note_color);
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
        if accidental != 0 {
            let ax = x - 13.0;
            gizmos.line_2d(Vec2::new(ax, y - 7.0), Vec2::new(ax, y + 7.0), note_color);
            gizmos.line_2d(
                Vec2::new(ax - 3.0, y),
                Vec2::new(ax + 3.0, y + 2.0),
                note_color,
            );
            if accidental > 0 {
                gizmos.line_2d(
                    Vec2::new(ax + 3.0, y - 7.0),
                    Vec2::new(ax + 3.0, y + 7.0),
                    note_color,
                );
                gizmos.line_2d(
                    Vec2::new(ax - 3.0, y - 4.0),
                    Vec2::new(ax + 3.0, y - 2.0),
                    note_color,
                );
            }
        }
    }
    // \\/ AI_CODE \\
    let symbol_position = |index: usize| {
        let symbol = layout.symbols.get(index)?;
        let x = viewport.x_at(symbol.start)?;
        let y = match &symbol.kind {
            NotationSymbolKind::Pitched { pitch, spelling } => {
                let (step, _) = pitch_step(*pitch, spelling.as_ref());
                step_y(step, clef)
            }
            NotationSymbolKind::Percussion { lane, .. } => lane.map_or(STAFF_Y, line_y),
            NotationSymbolKind::Rest => return None,
        };
        Some(Vec2::new(x, y))
    };
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
    // /\\ AI_CODE /\\
    for rest in &layout.rests {
        if let Some(x) = viewport.x_at(rest.start) {
            let color = if rest.explicit {
                note_color
            } else {
                inferred_color
            };
            let position = Vec2::new(x, STAFF_Y + (f32::from(rest.voice) - 1.0) * 12.0);
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
            let snapped = snap_editor_tick(&document, tick);
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
                                sheet_edit_pitch(&mut document, snapped, 0, &tools);
                            } else {
                                sheet_edit_percussion(
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
                            sheet_edit_spelled_pitch(
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
}
