use super::*;
use bevy::input::mouse::MouseWheel;
use bevy::ui_widgets::Activate;

const TAB_TOP: f32 = 250.0;
const TAB_BOTTOM: f32 = -300.0;

/// A persistent non-note label in the vertical tab view.
///
/// AI_CODE
#[derive(Component)]
pub(crate) struct TabSymbolLabel(usize);

type TabLabelQuery<'world, 'state> = Query<
    'world,
    'state,
    (
        Entity,
        &'static TabSymbolLabel,
        &'static mut Text2d,
        &'static mut Transform,
        &'static mut TextColor,
    ),
>;

/// A note badge drawn in UI space so gizmo staff lines cannot cover its text.
///
/// AI_CODE
#[derive(Component)]
pub(crate) struct TabNoteBadge(usize);

type TabNoteBadgeQuery<'world, 'state> = Query<
    'world,
    'state,
    (
        Entity,
        &'static TabNoteBadge,
        &'static mut Node,
        &'static mut Text,
    ),
>;

/// Map a tick in the active tab window to a top-to-bottom coordinate.
///
/// AI_CODE
fn tab_y(tick: u32, start: u32, span: u32) -> Option<f32> {
    let offset = tick.checked_sub(start)?;
    if offset >= span || span == 0 {
        return None;
    }
    Some(TAB_TOP - (TAB_TOP - TAB_BOTTOM) * offset as f32 / span as f32)
}

/// Place a tab event at a selected string and fret, retaining written pitch separately
/// from the optional preferred-string performance hint.
///
/// AI_CODE
fn tab_place_string(
    document: &mut EditorDocument,
    tools: &SheetTools,
    tick: u32,
    string: usize,
    fret: u8,
) -> bool {
    let Some(track) = document.chart.tracks.get(document.track_focus) else {
        return false;
    };
    let Some(tuning) = track.tuning.as_ref() else {
        return false;
    };
    let Ok(open_notes) = tuning.open_midi() else {
        return false;
    };
    let Some(open) = open_notes.get(string) else {
        return false;
    };
    let pitch = i16::from(*open) + i16::from(fret) + i16::from(track.capo.unwrap_or(0));
    if !(0..=127).contains(&pitch) {
        return false;
    }
    let tick = snap_editor_tick(document, tick);
    let added = super::sheet_edit_spelled_pitch(document, tick, pitch as u8, None, tools);
    if !added || tools.remove {
        return added;
    }
    let Some(track) = document.chart.tracks.get_mut(document.track_focus) else {
        return false;
    };
    if let Some(event) = track.notes.iter_mut().rev().find(|event| {
        event.start == tick
            && event.voice == tools.voice
            && matches!(event.content, NoteContent::Pitched { note, .. } if note == pitch as u8)
            && event.performance.is_none()
    }) {
        event.performance = Some(PerformanceHints {
            preferred_string: Some(string),
            attack: None,
            transition: None,
            bend: None,
            motion: None,
            percussion_dynamics: None,
            roll: None,
            droll: None,
            percussion_grace: None,
            percussion_technique: None,
            sticking: None,
            hi_hat: None,
            harmonic: None,
            palm_mute: false,
            vibrato: None,
        });
    }
    true
}

/// Choose the longest dotted note symbol that fits a snapped duration.
///
/// AI_CODE
fn duration_spelling(duration: u32, resolution: u32) -> (NoteValue, u8) {
    let values = [
        NoteValue::Whole,
        NoteValue::Half,
        NoteValue::Quarter,
        NoteValue::Eighth,
        NoteValue::Sixteenth,
    ];
    values
        .into_iter()
        .flat_map(|value| (0..=2).map(move |dots| (value, dots)))
        .filter_map(|(value, dots)| {
            let mut ticks = value.ticks(resolution);
            let mut addition = ticks;
            for _ in 0..dots {
                addition /= 2;
                ticks = ticks.saturating_add(addition);
            }
            (ticks <= duration).then_some((value, dots, ticks))
        })
        .max_by_key(|(_, _, ticks)| *ticks)
        .map_or((NoteValue::Sixteenth, 0), |(value, dots, _)| (value, dots))
}

/// Resolve the string/lane occupied by a laid-out event for resize collision checks.
///
/// AI_CODE
fn tab_lane_for_symbol(track: &ChartTrack, symbol: &NotationSymbol) -> Option<usize> {
    match &symbol.kind {
        NotationSymbolKind::Percussion {
            lane,
            lane_span: Some(_),
            ..
        } => Some(usize::MAX),
        NotationSymbolKind::Percussion { lane, .. } => *lane,
        NotationSymbolKind::Pitched { pitch, .. } => {
            let tuning = track.tuning.as_ref()?;
            let open_notes = tuning.open_midi().ok()?;
            let preferred = super::editor_source_event(track, symbol.source)
                .and_then(|event| event.performance.as_ref())
                .and_then(|hint| hint.preferred_string)
                .filter(|string| open_notes.get(*string).is_some_and(|open| open <= pitch));
            preferred.or_else(|| {
                open_notes
                    .iter()
                    .enumerate()
                    .filter(|(_, open)| **open <= *pitch)
                    .min_by_key(|(_, open)| i16::from(*pitch) - i16::from(**open))
                    .map(|(index, _)| index)
            })
        }
        NotationSymbolKind::Rest => None,
    }
}

/// Resize an authored tab event, capped at the next event on the same physical lane.
///
/// AI_CODE
fn resize_tab_event(
    document: &mut EditorDocument,
    source: EventSource,
    requested_end: u32,
    lane: usize,
    single_symbol_limit: u32,
) -> bool {
    let Some(track) = document.chart.tracks.get(document.track_focus) else {
        return false;
    };
    let layout = layout_track(&document.chart, track);
    let Some(symbol) = layout.symbols.iter().find(|symbol| symbol.source == source) else {
        return false;
    };
    let next_start = layout
        .symbols
        .iter()
        .filter(|candidate| {
            candidate.start > symbol.start && tab_lane_for_symbol(track, candidate) == Some(lane)
        })
        .map(|candidate| candidate.start)
        .min();
    let minimum = document
        .snap_interval
        .ticks(document.chart.resolution.max(1))
        .max(1);
    let end = next_start.map_or(requested_end, |next| requested_end.min(next));
    let duration = end.saturating_sub(symbol.start).max(minimum);
    let (length, dots) = duration_spelling(duration, document.chart.resolution.max(1));
    let tie_required = duration > single_symbol_limit;
    let Some(track) = document.chart.tracks.get_mut(document.track_focus) else {
        return false;
    };
    let Some(event) = super::editor_source_event_mut(track, source) else {
        return false;
    };
    event.length = length;
    event.dots = dots;
    event.tuplet = None;
    event.tied = tie_required;
    document.dirty = true;
    true
}

/// Apply fret-grid clicks, show each choice's resulting pitch, and close the dialog after use.
///
/// AI_CODE
pub(crate) fn tab_fret_dialog_system(
    mut document: ResMut<EditorDocument>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut choices: Query<(&TabFretChoice, &mut Text, &mut Visibility)>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        document.tab_fret_string = None;
    }
    if document.view_mode != EditorViewMode::Tab || document.metadata_mode {
        document.tab_fret_string = None;
    }
    let open = document.view_mode == EditorViewMode::Tab
        && !document.metadata_mode
        && document.tab_fret_string.is_some();
    let Some(string) = document.tab_fret_string else {
        for (_, _, mut visibility) in &mut choices {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let Some(track) = document.chart.tracks.get(document.track_focus) else {
        document.tab_fret_string = None;
        return;
    };
    let Some(tuning) = track.tuning.as_ref() else {
        document.tab_fret_string = None;
        return;
    };
    let Ok(open_notes) = tuning.open_midi() else {
        document.tab_fret_string = None;
        return;
    };
    let Some(open_pitch) = open_notes.get(string).copied() else {
        document.tab_fret_string = None;
        return;
    };
    let capo = track.capo.unwrap_or(0);
    for (choice, mut text, mut visibility) in &mut choices {
        let pitch = i16::from(open_pitch) + i16::from(choice.0) + i16::from(capo);
        text.0 = format!("{}  {}", choice.0, note_name(pitch as u8));
        *visibility = if open && pitch <= 127 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Place a string note from an activated fret-choice button.
///
/// AI_CODE
pub(crate) fn tab_fret_choice_input(
    activate: On<Activate>,
    mut document: ResMut<EditorDocument>,
    tools: Res<SheetTools>,
    choices: Query<&TabFretChoice>,
) {
    if document.view_mode != EditorViewMode::Tab || document.metadata_mode {
        return;
    }
    let Some(string) = document.tab_fret_string else {
        return;
    };
    let Ok(choice) = choices.get(activate.entity) else {
        return;
    };
    let tick = document.tab_fret_tick;
    tab_place_string(&mut document, &tools, tick, string, choice.0);
    document.tab_fret_string = None;
}

/// Format MIDI pitch as a compact note name for the fret-choice dialog.
///
/// AI_CODE
fn note_name(midi: u8) -> &'static str {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    NAMES[(midi % 12) as usize]
}

/// Render and edit the focused track as a vertical, top-to-bottom tab view.
///
/// AI_CODE
pub(crate) fn tab_view_system(
    mut commands: Commands,
    mut gizmos: Gizmos,
    mut document: ResMut<EditorDocument>,
    tools: Res<SheetTools>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<MouseWheel>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut labels: TabLabelQuery,
    mut note_badges: TabNoteBadgeQuery,
) {
    if document.view_mode != EditorViewMode::Tab
        || document.metadata_mode
        || document.text_entry.is_some()
        || document.tab_fret_string.is_some()
    {
        document.tab_resize = None;
        for (entity, _, _, _, _) in labels.iter() {
            commands.entity(entity).despawn();
        }
        for (entity, _, _, _) in note_badges.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }
    let Ok(window) = windows.single() else { return };
    let resolution = document.chart.resolution.max(1);
    let span = resolution.saturating_mul(u32::from(document.tab_beats_per_screen.max(2)));
    for event in wheel.read() {
        let direction = match event.y.total_cmp(&0.0) {
            std::cmp::Ordering::Greater => -1i64,
            std::cmp::Ordering::Less => 1i64,
            std::cmp::Ordering::Equal => 0,
        };
        let step = i64::from(snap_editor_tick(&document, resolution));
        document.current_tick = (i64::from(document.current_tick) + direction * step)
            .clamp(0, i64::from(u32::MAX)) as u32;
    }
    if keyboard.just_pressed(KeyCode::Equal) || keyboard.just_pressed(KeyCode::NumpadAdd) {
        document.tab_beats_per_screen = document.tab_beats_per_screen.saturating_sub(1).max(2);
    }
    if keyboard.just_pressed(KeyCode::Minus) || keyboard.just_pressed(KeyCode::NumpadSubtract) {
        document.tab_beats_per_screen = document.tab_beats_per_screen.saturating_add(1).min(32);
    }
    let Some(track) = document.chart.tracks.get(document.track_focus).cloned() else {
        return;
    };
    let layout = layout_track(&document.chart, &track);
    let percussion = track.kind == InstrumentKind::Percussion;
    let lanes = if percussion {
        track.kit.as_ref().map_or(0, |kit| kit.lanes)
    } else {
        track
            .tuning
            .as_ref()
            .map_or(0, |tuning| tuning.strings.len())
    };
    let left = -window.width() / 2.0 + 100.0;
    let right = window.width() / 2.0 - 50.0;
    let x_for_lane = |lane: usize| {
        if lanes <= 1 {
            (left + right) / 2.0
        } else {
            left + (right - left) * lane as f32 / (lanes - 1) as f32
        }
    };
    let mut next_labels = Vec::<(String, f32, f32, Color)>::new();
    let mut next_badges = Vec::<(String, f32, f32)>::new();
    for lane in 0..lanes {
        let x = x_for_lane(lane);
        gizmos.line_2d(
            Vec2::new(x, TAB_TOP),
            Vec2::new(x, TAB_BOTTOM),
            Color::srgb(0.55, 0.60, 0.63),
        );
        let name = if percussion {
            format!("L{}", lane + 1)
        } else {
            track
                .tuning
                .as_ref()
                .and_then(|tuning| tuning.open_midi().ok())
                .and_then(|notes| notes.get(lane).copied())
                .map_or_else(
                    || format!("S{}", lane + 1),
                    |midi| note_name(midi).to_owned(),
                )
        };
        next_labels.push((name, x, TAB_TOP + 20.0, Color::srgb(0.92, 0.94, 0.88)));
    }
    if percussion {
        let span_x = left - 48.0;
        gizmos.line_2d(
            Vec2::new(span_x, TAB_TOP),
            Vec2::new(span_x, TAB_BOTTOM),
            Color::srgb(0.79, 0.63, 0.31),
        );
        next_labels.push((
            "SPAN".into(),
            span_x,
            TAB_TOP + 20.0,
            Color::srgb(0.92, 0.94, 0.88),
        ));
    }
    let beat = document.snap_interval.ticks(resolution).max(1);
    let mut tick = document.current_tick;
    while tick < document.current_tick.saturating_add(span) {
        if let Some(y) = tab_y(tick, document.current_tick, span) {
            gizmos.line_2d(
                Vec2::new(left - 30.0, y),
                Vec2::new(right + 30.0, y),
                Color::srgb(0.28, 0.34, 0.36),
            );
            next_labels.push((
                tick.to_string(),
                left - 68.0,
                y,
                Color::srgb(0.92, 0.94, 0.88),
            ));
        }
        tick = tick.saturating_add(beat);
        if tick == u32::MAX {
            break;
        }
    }
    for symbol in &layout.symbols {
        let Some(y) = tab_y(symbol.start, document.current_tick, span) else {
            continue;
        };
        let lane = tab_lane_for_symbol(&track, symbol);
        let span_piece = matches!(
            symbol.kind,
            NotationSymbolKind::Percussion {
                lane_span: Some(_),
                ..
            }
        );
        let x = if span_piece {
            left - 48.0
        } else {
            let Some(lane) = lane.filter(|lane| *lane < lanes) else {
                continue;
            };
            x_for_lane(lane)
        };
        let text = match &symbol.kind {
            NotationSymbolKind::Pitched { pitch, .. } => {
                let Some(lane) = lane else { continue };
                track
                    .tuning
                    .as_ref()
                    .and_then(|tuning| tuning.open_midi().ok())
                    .and_then(|open| open.get(lane).copied())
                    .map_or_else(
                        || "?".into(),
                        |open| {
                            (i16::from(*pitch)
                                - i16::from(open)
                                - i16::from(track.capo.unwrap_or(0)))
                            .to_string()
                        },
                    )
            }
            NotationSymbolKind::Percussion {
                piece,
                symbol,
                lane_span,
                ..
            } => {
                if lane_span.is_some() {
                    format!("{} {piece}", symbol.as_deref().unwrap_or("x"))
                } else {
                    format!("{} {piece}", symbol.as_deref().unwrap_or("x"))
                }
            }
            NotationSymbolKind::Rest => continue,
        };
        next_badges.push((text, x, y));
        let end = symbol.start.saturating_add(symbol.duration);
        if let Some(end_y) = tab_y(end, document.current_tick, span) {
            gizmos.line_2d(
                Vec2::new(x, y + 10.0),
                Vec2::new(x, end_y),
                Color::srgb(0.86, 0.88, 0.82),
            );
        }
    }
    let existing = labels
        .iter_mut()
        .map(|(entity, marker, _, _, _)| (marker.0, entity))
        .collect::<std::collections::HashMap<_, _>>();
    for (index, (text_value, x, y, color)) in next_labels.iter().enumerate() {
        if let Some(entity) = existing.get(&index)
            && let Ok((_, _, mut text, mut transform, mut text_color)) = labels.get_mut(*entity)
        {
            text.0.clone_from(text_value);
            transform.translation = Vec3::new(*x, *y, 10.0);
            text_color.0 = *color;
        } else {
            commands.spawn((
                Text2d::new(text_value.clone()),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(*color),
                Transform::from_xyz(*x, *y, 10.0),
                TabSymbolLabel(index),
            ));
        }
    }
    for (entity, marker, _, _, _) in labels.iter() {
        if marker.0 >= next_labels.len() {
            commands.entity(entity).despawn();
        }
    }
    let existing_badges = note_badges
        .iter_mut()
        .map(|(entity, marker, _, _)| (marker.0, entity))
        .collect::<std::collections::HashMap<_, _>>();
    for (index, (badge_text, x, y)) in next_badges.iter().enumerate() {
        let badge_width = (badge_text.chars().count() as f32 * 11.0 + 16.0).max(36.0);
        let badge_height = 36.0;
        let left_px = window.width() / 2.0 + *x - badge_width / 2.0;
        let top_px = window.height() / 2.0 - *y - badge_height / 2.0;
        if let Some(entity) = existing_badges.get(&index)
            && let Ok((_, _, mut node, mut text)) = note_badges.get_mut(*entity)
        {
            node.left = Val::Px(left_px);
            node.top = Val::Px(top_px);
            node.width = Val::Px(badge_width);
            node.height = Val::Px(badge_height);
            text.0.clone_from(badge_text);
        } else {
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(left_px),
                    top: Val::Px(top_px),
                    width: Val::Px(badge_width),
                    height: Val::Px(badge_height),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.16, 0.54, 0.44)),
                Text::new(badge_text.clone()),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                TabNoteBadge(index),
            ));
        }
    }
    for (entity, marker, _, _) in note_badges.iter() {
        if marker.0 >= next_badges.len() {
            commands.entity(entity).despawn();
        }
    }

    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let position = Vec2::new(
        cursor.x - window.width() / 2.0,
        window.height() / 2.0 - cursor.y,
    );
    let tick = document.current_tick.saturating_add(
        ((TAB_TOP - position.y.clamp(TAB_BOTTOM, TAB_TOP)) / (TAB_TOP - TAB_BOTTOM) * span as f32)
            as u32,
    );
    let snapped = snap_editor_tick(&document, tick);
    if mouse.just_released(MouseButton::Left)
        && let Some(resize) = document.tab_resize.take()
    {
        let single_symbol_limit = tools.duration.ticks(document.chart.resolution.max(1));
        resize_tab_event(
            &mut document,
            resize.source,
            snapped,
            resize.lane,
            single_symbol_limit,
        );
        return;
    }
    if percussion && position.x >= left - 78.0 && position.x <= left - 20.0 {
        if document.tab_resize.is_some() {
            if let Some(resize) = document.tab_resize {
                if let Some(y) = tab_y(resize.start_tick, document.current_tick, span) {
                    gizmos.line_2d(
                        Vec2::new(left - 48.0, y),
                        Vec2::new(left - 48.0, position.y.clamp(TAB_BOTTOM, TAB_TOP)),
                        Color::srgb(0.90, 0.75, 0.34),
                    );
                }
            }
            return;
        }
        if mouse.just_pressed(MouseButton::Left) {
            if let Some(symbol) = layout.symbols.iter().find(|symbol| {
                matches!(
                    symbol.kind,
                    NotationSymbolKind::Percussion {
                        lane_span: Some(_),
                        ..
                    }
                ) && tab_y(
                    symbol.start.saturating_add(symbol.duration),
                    document.current_tick,
                    span,
                )
                .is_some_and(|end_y| (position.y - end_y).abs() <= 12.0)
            }) {
                document.tab_resize = Some(TabDurationResize {
                    source: symbol.source,
                    start_tick: symbol.start,
                    lane: usize::MAX,
                });
                return;
            }
            let Some(piece) = track
                .kit
                .as_ref()
                .and_then(|kit| kit.pieces.get(tools.piece_index % kit.pieces.len().max(1)))
            else {
                return;
            };
            if piece.lane_span.is_some() {
                super::sheet_edit_percussion(&mut document, snapped, 0, &tools);
            } else {
                document.status = "Cycle PIECE to a lane-spanning kit piece".into();
            }
        }
        return;
    }
    let lane = if lanes <= 1 {
        0
    } else {
        ((position.x - left) / (right - left) * (lanes - 1) as f32).round() as usize
    };
    if lane >= lanes || (position.x - x_for_lane(lane)).abs() > 25.0 {
        return;
    }
    if document.tab_resize.is_some() {
        let start = document
            .tab_resize
            .map_or(snapped, |resize| resize.start_tick);
        if let Some(y) = tab_y(start, document.current_tick, span) {
            gizmos.line_2d(
                Vec2::new(x_for_lane(lane), y),
                Vec2::new(x_for_lane(lane), position.y),
                Color::srgb(0.90, 0.75, 0.34),
            );
        }
        return;
    }
    if mouse.just_pressed(MouseButton::Left) {
        if let Some(symbol) = layout.symbols.iter().find(|symbol| {
            tab_lane_for_symbol(&track, symbol) == Some(lane)
                && symbol.start.saturating_add(symbol.duration) > symbol.start
                && tab_y(
                    symbol.start.saturating_add(symbol.duration),
                    document.current_tick,
                    span,
                )
                .is_some_and(|end_y| (position.y - end_y).abs() <= 12.0)
        }) {
            document.tab_resize = Some(TabDurationResize {
                source: symbol.source,
                start_tick: symbol.start,
                lane,
            });
            return;
        }
        if percussion {
            let Some(kit) = track.kit.as_ref() else {
                return;
            };
            let pieces = kit
                .pieces
                .iter()
                .enumerate()
                .filter(|(_, piece)| piece.lane == Some(lane))
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            if pieces.len() > 1 {
                if document.tab_percussion_lane != Some(lane) {
                    document.tab_percussion_lane = Some(lane);
                    document.status = format!(
                        "Shared lane: choose a matching piece with PIECE, then click lane {} again",
                        lane + 1
                    );
                    return;
                }
                if !pieces.contains(&tools.piece_index) {
                    document.status = "Selected piece is not on this lane; cycle PIECE".into();
                    return;
                }
            }
            document.tab_percussion_lane = None;
            document.current_tick = snapped;
            super::sheet_edit_percussion(&mut document, snapped, lane, &tools);
        } else {
            document.tab_fret_string = Some(lane);
            document.tab_fret_tick = snapped;
            document.status = format!("Choose fret for string {}", lane + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vertical tab coordinates progress from the top of the window to the bottom in time order.
    ///
    /// AI_CODE
    #[test]
    fn tab_time_maps_top_to_bottom() {
        assert_eq!(tab_y(100, 100, 1000), Some(TAB_TOP));
        assert_eq!(tab_y(600, 100, 1000), Some((TAB_TOP + TAB_BOTTOM) / 2.0));
        assert_eq!(tab_y(1100, 100, 1000), None);
        assert_eq!(tab_y(99, 100, 1000), None);
    }

    /// A fret choice resolves through tuning/capo and records its preferred string separately.
    ///
    /// AI_CODE
    #[test]
    fn tab_fret_choice_places_written_pitch_and_string_hint() {
        let mut document = super::super::new_chart_document();
        document
            .chart
            .tracks
            .push(super::super::new_track(InstrumentKind::Strings));
        document.chart.tracks[0].capo = Some(2);
        let open = document.chart.tracks[0]
            .tuning
            .as_ref()
            .unwrap()
            .open_midi()
            .unwrap()[0];
        let tools = SheetTools::default();

        assert!(tab_place_string(&mut document, &tools, 0, 0, 3));
        let event = &document.chart.tracks[0].notes[0];
        assert!(matches!(event.content, NoteContent::Pitched { note, .. } if note == open + 5));
        assert_eq!(
            event
                .performance
                .as_ref()
                .and_then(|hint| hint.preferred_string),
            Some(0)
        );
    }

    /// Duration spelling respects the active snap minimum and resize stops before lane collisions.
    ///
    /// AI_CODE
    #[test]
    fn tab_resize_snaps_and_caps_before_next_note_on_lane() {
        let mut document = super::super::new_chart_document();
        document
            .chart
            .tracks
            .push(super::super::new_track(InstrumentKind::Strings));
        let tools = SheetTools::default();
        let open = document.chart.tracks[0]
            .tuning
            .as_ref()
            .unwrap()
            .open_midi()
            .unwrap()[0];
        assert!(tab_place_string(&mut document, &tools, 0, 0, 3));
        assert!(tab_place_string(&mut document, &tools, 1920, 0, 5));

        let single_symbol_limit = tools.duration.ticks(document.chart.resolution);
        assert!(resize_tab_event(
            &mut document,
            EventSource::Track(0),
            3000,
            0,
            single_symbol_limit,
        ));
        let first = &document.chart.tracks[0].notes[0];
        assert_eq!(first.duration_ticks(document.chart.resolution), 1920);
        assert!(first.tied);
        assert!(matches!(first.content, NoteContent::Pitched { note, .. } if note == open + 3));

        assert_eq!(
            duration_spelling(0, document.chart.resolution),
            (NoteValue::Sixteenth, 0)
        );
        assert_eq!(
            duration_spelling(1920, document.chart.resolution),
            (NoteValue::Half, 0)
        );
        assert!(resize_tab_event(
            &mut document,
            EventSource::Track(0),
            0,
            0,
            single_symbol_limit,
        ));
        assert_eq!(
            document.chart.tracks[0].notes[0].duration_ticks(document.chart.resolution),
            single_symbol_limit
        );
    }
}
