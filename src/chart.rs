use super::*;

pub(crate) fn load_charts() -> Vec<Chart> {
    let mut paths = std::fs::read_dir("charts")
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    paths.sort();

    let mut charts = Vec::new();
    for path in paths {
        match std::fs::read_to_string(&path)
            .map_err(|error| error.to_string())
            .and_then(|contents| {
                serde_json::from_str::<Chart>(&contents).map_err(|error| error.to_string())
            }) {
            Ok(chart) => charts.push(chart),
            Err(error) => eprintln!("Could not load chart {}: {error}", path.display()),
        }
    }
    if charts.is_empty() {
        match serde_json::from_str(OPEN_STRINGS_CHART) {
            Ok(chart) => charts.push(chart),
            Err(error) => eprintln!("Could not load embedded starter chart: {error}"),
        }
    }
    charts
}

pub(crate) fn setup_song_menu(mut commands: Commands) {
    commands.insert_resource(SongMenuSelection {
        selected: 0,
        charts: load_charts(),
    });
    commands.spawn((Camera2d, ChartEntity));
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(28.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(52.0),
            left: Val::Px(80.0),
            ..default()
        },
        SongMenuText,
        ChartEntity,
    ));
}

pub(crate) fn song_menu_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<SongMenuSelection>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
    time: Res<Time>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Home);
    }
    if keyboard.just_pressed(KeyCode::ArrowUp) {
        menu.selected = menu
            .selected
            .checked_sub(1)
            .unwrap_or(menu.charts.len().saturating_sub(1));
    }
    if keyboard.just_pressed(KeyCode::ArrowDown) && !menu.charts.is_empty() {
        menu.selected = (menu.selected + 1) % menu.charts.len();
    }
    if keyboard.just_pressed(KeyCode::Enter) {
        if let Some(chart) = menu.charts.get(menu.selected).cloned() {
            commands.insert_resource(ChartSession {
                chart,
                started_at: time.elapsed_secs() + 5.0,
            });
            next_state.set(AppState::ChartCountdown);
        }
    }
}

pub(crate) fn song_menu_display(
    menu: Res<SongMenuSelection>,
    mut text: Query<&mut Text, With<SongMenuText>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let entries = if menu.charts.is_empty() {
        "NO CHARTS FOUND".into()
    } else {
        menu.charts
            .iter()
            .enumerate()
            .map(|(index, chart)| {
                format!(
                    "{} [{}] {} ({:.0} BPM)",
                    if index == menu.selected { ">" } else { " " },
                    index + 1,
                    chart.title,
                    chart.bpm
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    *text = Text::new(format!(
        "OPEN BAND  //  SONGS\n\n{entries}\n\nUp/Down: select     Enter: play     Esc: home\n\nNote display: {:?}",
        CHART_NOTE_DISPLAY,
    ));
}

pub(crate) fn cleanup_song_menu(
    mut commands: Commands,
    entities: Query<Entity, With<ChartEntity>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

pub(crate) fn setup_chart_countdown(
    mut commands: Commands,
    time: Res<Time>,
    menu: Option<Res<SongMenuSelection>>,
) {
    let chart = menu
        .and_then(|menu| menu.charts.get(menu.selected).cloned())
        .unwrap_or_else(|| serde_json::from_str(OPEN_STRINGS_CHART).expect("built-in chart JSON"));
    let notes = chart.bass_notes();
    let total_notes = notes.len();
    let sustain_expected = notes.iter().filter(|note| note.duration > 0.0).count();
    commands.insert_resource(ChartSession {
        chart,
        started_at: time.elapsed_secs() + 5.0,
    });
    commands.insert_resource(ChartStats {
        total_notes,
        sustain_expected,
        ..Default::default()
    });
    commands.insert_resource(ChartFeedback::default());
    commands.spawn((Camera2d, ChartEntity));
    commands.spawn((
        Text2d::new("5"),
        TextFont {
            font_size: FontSize::Px(96.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.8, 0.2)),
        Transform::from_xyz(0.0, 80.0, 5.0),
        ChartCountdownText,
        ChartEntity,
    ));
}

pub(crate) fn chart_countdown_system(
    time: Res<Time>,
    session: Res<ChartSession>,
    mut text: Query<&mut Text2d, With<ChartCountdownText>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let remaining = session.started_at - time.elapsed_secs();
    if remaining <= 0.0 {
        next_state.set(AppState::ChartGameplay);
        return;
    }
    if let Ok(mut text) = text.single_mut() {
        text.0 = remaining.ceil().to_string();
    }
}

pub(crate) fn setup_chart_gameplay(mut commands: Commands, session: Res<ChartSession>) {
    let notes = session.chart.bass_notes();
    commands.spawn((Camera2d, ChartEntity));
    commands.spawn((
        Text2d::new(""),
        TextFont {
            font_size: FontSize::Px(30.0),
            ..default()
        },
        TextColor(Color::srgb(0.3, 1.0, 0.45)),
        Transform::from_xyz(0.0, 245.0, 6.0),
        ChartFeedbackText,
        ChartEntity,
    ));
    commands.spawn((
        Text2d::new(format!(
            "{}  //  {}  //  {:.0} BPM",
            session.chart.title,
            session.chart.first_instrument(),
            session.chart.bpm
        )),
        TextFont {
            font_size: FontSize::Px(26.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.95, 1.0)),
        Transform::from_xyz(0.0, 320.0, 5.0),
        ChartEntity,
    ));
    let strings = ["B", "E", "A", "D", "G"];
    for (string, label) in strings.into_iter().enumerate() {
        let x = -360.0 + string as f32 * 180.0;
        commands.spawn((
            Sprite {
                color: Color::srgba(0.25, 0.3, 0.4, 0.65),
                custom_size: Some(Vec2::new(3.0, 570.0)),
                ..default()
            },
            Transform::from_xyz(x, 0.0, 0.0),
            ChartEntity,
        ));
        commands.spawn((
            Text2d::new(label),
            TextFont {
                font_size: FontSize::Px(18.0),
                ..default()
            },
            TextColor(Color::srgb(0.65, 0.75, 0.9)),
            Transform::from_xyz(x, 285.0, 2.0),
            ChartEntity,
        ));
    }
    commands.spawn((
        Sprite {
            color: Color::srgb(1.0, 0.8, 0.2),
            custom_size: Some(Vec2::new(760.0, 5.0)),
            ..default()
        },
        Transform::from_xyz(0.0, CHART_HIT_LINE_Y, 1.0),
        ChartEntity,
    ));
    for note in &notes {
        let x = -360.0 + note.string.min(4) as f32 * 180.0;
        let label = match CHART_NOTE_DISPLAY {
            NoteDisplayMode::Fret => format!("{}", note.fret),
            NoteDisplayMode::Note => note.note.clone(),
            NoteDisplayMode::Both => format!("{}\n{}", note.fret, note.note),
        };
        commands
            .spawn((
                Sprite {
                    color: lane_color(note.string).with_alpha(0.95),
                    custom_size: Some(Vec2::new(108.0, 8.0)),
                    ..default()
                },
                Transform::from_xyz(x, 300.0, 3.0),
                ChartNoteVisual {
                    string: note.string,
                    start: note.start,
                    duration: note.duration,
                    pitch_hz: note.pitch_hz,
                    matched: false,
                    missed: false,
                    sustain_observed: 0.0,
                },
                ChartEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text2d::new(label),
                    TextFont {
                        font_size: FontSize::Px(18.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Transform::from_xyz(0.0, 12.0, 1.0),
                ));
            });
    }
}

pub(crate) fn chart_gameplay_system(
    mut commands: Commands,
    time: Res<Time>,
    session: Res<ChartSession>,
    stream: Res<InstrumentStream>,
    mut stats: ResMut<ChartStats>,
    mut feedback: ResMut<ChartFeedback>,
    mut next_state: ResMut<NextState<AppState>>,
    mut feedback_text: Query<&mut Text2d, With<ChartFeedbackText>>,
    mut notes: Query<(Entity, &mut ChartNoteVisual, &mut Transform, &mut Sprite)>,
) {
    let chart_notes = session.chart.bass_notes();
    let elapsed = time.elapsed_secs() - session.started_at;
    let mut detected_events = Vec::new();
    if let Ok(events) = stream.events.lock() {
        for event in events.try_iter() {
            if event.instrument == Instrument::Bass5 {
                detected_events.push((
                    event.phase,
                    event.lane,
                    event.pitch_hz.unwrap_or(0.0),
                    event.duration_secs,
                ));
            }
        }
    }
    for (phase, lane, pitch, duration) in detected_events {
        match phase {
            NotePhase::Started => {
                let mut best_match = None;
                for (entity, note, _, _) in &mut notes {
                    if !note.matched
                        && lane == note.string
                        && (note.start - elapsed).abs() <= 0.25
                        && cents_error(pitch, note.pitch_hz).abs() <= 90.0
                    {
                        best_match = Some(entity);
                        break;
                    }
                }
                if let Some(entity) = best_match {
                    if let Ok((_, mut note, _, mut sprite)) = notes.get_mut(entity) {
                        note.matched = true;
                        note.sustain_observed = duration;
                        sprite.color = Color::srgb(0.3, 1.0, 0.45);
                        stats.correct_hits += 1;
                        stats
                            .timing_offsets_ms
                            .push((elapsed - note.start) * 1000.0);
                        feedback.message =
                            format!("HIT  {:+.0} ms", (elapsed - note.start) * 1000.0);
                        feedback.until = time.elapsed_secs() + 0.75;
                    }
                } else {
                    feedback.message = "WRONG NOTE".into();
                    feedback.until = time.elapsed_secs() + 0.75;
                }
            }
            NotePhase::Updated => {
                for (_, mut note, _, _) in &mut notes {
                    if note.matched && note.string == lane {
                        note.sustain_observed = note.sustain_observed.max(duration);
                    }
                }
            }
            NotePhase::Ended => {
                let mut ended = None;
                for (entity, note, _, _) in &mut notes {
                    if note.matched && note.string == lane {
                        if note.duration <= 0.0
                            || duration.max(note.sustain_observed) >= note.duration * 0.7
                        {
                            if note.duration > 0.0 {
                                stats.sustain_successful += 1;
                            }
                            ended = Some(entity);
                            break;
                        }
                    }
                }
                if let Some(entity) = ended {
                    commands.entity(entity).despawn();
                }
            }
        }
    }
    for (_, mut note, mut transform, mut sprite) in &mut notes {
        if !note.matched && !note.missed && elapsed > note.start + 0.25 {
            note.missed = true;
            feedback.message = "MISS".into();
            feedback.until = time.elapsed_secs() + 0.75;
        }
        let head_y = CHART_HIT_LINE_Y + (note.start - elapsed) * CHART_NOTE_SPEED;
        let height = (8.0 + note.duration * CHART_NOTE_SPEED).clamp(8.0, 360.0);
        sprite.custom_size = Some(Vec2::new(108.0, height));
        transform.translation.y = head_y + height * 0.5;
    }
    let chart_end = chart_notes
        .iter()
        .map(|note| note.start + note.duration)
        .fold(0.0, f32::max);
    if elapsed > chart_end + 1.0 {
        next_state.set(AppState::ChartReview);
    }
    if let Ok(mut text) = feedback_text.single_mut() {
        text.0 = if feedback.until >= time.elapsed_secs() {
            feedback.message.clone()
        } else {
            String::new()
        };
    }
}

pub(crate) fn cents_error(actual_hz: f32, expected_hz: f32) -> f32 {
    1200.0 * (actual_hz / expected_hz).log2()
}

pub(crate) fn chart_menu_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Songs);
    }
}

pub(crate) fn cleanup_chart_entities(
    mut commands: Commands,
    entities: Query<Entity, With<ChartEntity>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

pub(crate) fn setup_chart_review(mut commands: Commands, session: Res<ChartSession>) {
    commands.spawn((Camera2d, ChartReviewText));
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(28.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(56.0),
            left: Val::Px(100.0),
            ..default()
        },
        ChartReviewText,
    ));
    let _ = session;
}

pub(crate) fn chart_review_display(
    stats: Res<ChartStats>,
    session: Res<ChartSession>,
    mut text: Query<&mut Text, With<ChartReviewText>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let average_offset = if stats.timing_offsets_ms.is_empty() {
        0.0
    } else {
        stats.timing_offsets_ms.iter().sum::<f32>() / stats.timing_offsets_ms.len() as f32
    };
    let sustain_percent = if stats.sustain_expected == 0 {
        0.0
    } else {
        stats.sustain_successful as f32 / stats.sustain_expected as f32 * 100.0
    };
    *text = Text::new(format!(
        "OPEN BAND  //  SONG REVIEW\n\n{}\n\nCORRECT HITS  {:>3} / {:<3}\nSUSTAIN SUCCESS {:>5.1}%\nAVG ATTACK OFFSET {:>+6.1} ms\n\nEnter: songs     Esc: home",
        session.chart.title,
        stats.correct_hits,
        stats.total_notes.max(session.chart.bass_notes().len()),
        sustain_percent,
        average_offset,
    ));
}

pub(crate) fn chart_review_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Home);
    } else if keyboard.just_pressed(KeyCode::Enter) {
        next_state.set(AppState::Songs);
    }
}

pub(crate) fn cleanup_chart_review(
    mut commands: Commands,
    entities: Query<Entity, With<ChartReviewText>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}
