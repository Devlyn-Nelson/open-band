use super::*;

/// Spawn the live highway and its diagnostic panel.
pub(crate) fn setup_gameplay(mut commands: Commands, debug: Res<DebugInputData>) {
    commands.spawn((Camera2d, GameplayEntity));
    commands.spawn((
        Text::new("OPEN BAND  //  LIVE SESSION"),
        TextFont {
            font_size: FontSize::Px(28.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(22.0),
            left: Val::Px(34.0),
            ..default()
        },
        GameplayEntity,
    ));
    commands.spawn((
        Text::new("GUITAR  •  BASS 4/5  •  MIDI DRUMS  •  VOCALS    |    A S D F G to play"),
        TextFont {
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::srgb(0.45, 0.55, 0.68)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(62.0),
            left: Val::Px(36.0),
            ..default()
        },
        GameplayEntity,
    ));
    commands.spawn((
        Text::new(debug_text(&debug)),
        TextFont {
            font_size: FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::srgb(0.75, 0.85, 0.95)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(112.0),
            right: Val::Px(32.0),
            width: Val::Px(310.0),
            padding: UiRect::all(Val::Px(16.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.025, 0.04, 0.07, 0.92)),
        DebugWindow,
        DebugText,
        GameplayEntity,
    ));
    for lane in 0..LANES {
        let x = -360.0 + lane as f32 * 180.0;
        commands.spawn((
            Sprite {
                color: Color::srgba(0.15, 0.2, 0.3, 0.55),
                custom_size: Some(Vec2::new(3.0, 570.0)),
                ..default()
            },
            Transform::from_xyz(x, -15.0, 0.0),
            GameplayEntity,
        ));
        commands.spawn((
            Sprite {
                color: lane_color(lane).with_alpha(0.28),
                custom_size: Some(Vec2::new(122.0, 5.0)),
                ..default()
            },
            Transform::from_xyz(x, HIT_LINE_Y, 0.0),
            GameplayEntity,
        ));
    }
}

pub(crate) fn cleanup_gameplay(
    mut commands: Commands,
    entities: Query<Entity, With<GameplayEntity>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

pub(crate) fn gameplay_menu_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Home);
    }
}

pub(crate) fn debug_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut visibility: Query<&mut Visibility, With<DebugWindow>>,
) {
    if !keyboard.just_pressed(KeyCode::F3) {
        return;
    }
    let Ok(mut visibility) = visibility.single_mut() else {
        return;
    };
    *visibility = match *visibility {
        Visibility::Visible => Visibility::Hidden,
        Visibility::Hidden | Visibility::Inherited => Visibility::Visible,
    };
}

pub(crate) fn receive_instrument_events(
    mut commands: Commands,
    stream: Res<InstrumentStream>,
    selection: Res<DeviceSelection>,
    mut debug: ResMut<DebugInputData>,
    mut text: Query<&mut Text, With<DebugText>>,
    mut notes: Query<(&mut FallingNote, &mut Sprite)>,
    time: Res<Time>,
) {
    let Ok(events) = stream.events.lock() else {
        return;
    };
    for event in events.try_iter() {
        debug.instrument = Some(event.instrument);
        debug.open_frequencies = selection
            .slots
            .get(event.instrument.slot)
            .map_or_else(Vec::new, InstrumentSlot::open_frequencies);
        debug.pitch_hz = event.pitch_hz;
        debug.lane = Some(event.lane);
        debug.strength = event.strength;
        debug.noise_floor = event.noise_floor;
        debug.duration_secs = event.duration_secs;
        if event.phase == NotePhase::Started {
            debug.event_count += 1;
            let x = -360.0 + event.lane as f32 * 180.0;
            commands.spawn((
                Sprite {
                    color: instrument_color(event.instrument, event.lane),
                    custom_size: Some(Vec2::new(108.0, 8.0)),
                    ..default()
                },
                Transform::from_xyz(x, 300.0 + event.strength * 10.0, 1.0),
                FallingNote {
                    lane: event.lane,
                    instrument: event.instrument,
                    spawned_at: time.elapsed_secs(),
                    duration_secs: 0.0,
                },
                GameplayEntity,
            ));
        } else if event.phase == NotePhase::Updated {
            let mut latest_spawn = f32::NEG_INFINITY;
            for (mut note, mut sprite) in &mut notes {
                if note.instrument == event.instrument
                    && note.lane == event.lane
                    && note.spawned_at > latest_spawn
                {
                    latest_spawn = note.spawned_at;
                    note.duration_secs = event.duration_secs;
                    sprite.custom_size = Some(Vec2::new(
                        108.0,
                        (8.0 + event.duration_secs * NOTE_SPEED).clamp(8.0, 420.0),
                    ));
                }
            }
        }
    }
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    *text = Text::new(debug_text(&debug));
}

fn debug_text(debug: &DebugInputData) -> String {
    let instrument = debug.instrument.map_or_else(
        || "NONE".into(),
        |instrument| instrument_name(instrument.kind, instrument.strings),
    );
    let pitch = debug
        .pitch_hz
        .map_or_else(|| "--".into(), |pitch| format!("{pitch:>7.2} Hz"));
    let lane = debug
        .lane
        .map_or_else(|| "--".into(), |lane| (lane + 1).to_string());
    let bass = debug.instrument.zip(debug.pitch_hz).map_or_else(
        || "STRING  --\nNOTE    --".into(),
        |(instrument, pitch)| bass_debug_details(instrument, &debug.open_frequencies, pitch),
    );
    format!(
        "DEBUG INPUT  [F3]\n\nINSTRUMENT  {instrument}\nEST PITCH   {pitch}\n{bass}\nLANE        {lane}\nSIGNAL      {:>5.1}%\nNOISE FLOOR {:>5.2}%\nDURATION    {:>5.2} s\nEVENTS      {}",
        debug.strength * 100.0,
        debug.noise_floor * 100.0,
        debug.duration_secs,
        debug.event_count,
    )
}

fn bass_debug_details(instrument: Instrument, open_frequencies: &[f32], pitch_hz: f32) -> String {
    if instrument.kind != InstrumentKind::Strings || open_frequencies.is_empty() {
        return "STRING      --\nNOTE        --".into();
    }
    let (string, target) = open_frequencies
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            (pitch_hz - *left)
                .abs()
                .total_cmp(&(pitch_hz - *right).abs())
        })
        .map(|(index, target)| (index, *target))
        .unwrap_or((0, pitch_hz));
    let cents = 1200.0 * (pitch_hz / target).log2();
    format!(
        "STRING      {}\nPITCH       {target:.2} Hz ({cents:+.1} cents)",
        string + 1
    )
}

pub(crate) fn move_notes(
    mut notes: Query<(&FallingNote, &Sprite, &mut Transform)>,
    time: Res<Time>,
) {
    for (note, sprite, mut transform) in &mut notes {
        let head_y = 300.0 - (time.elapsed_secs() - note.spawned_at) * NOTE_SPEED;
        let height = sprite.custom_size.map_or(8.0, |size| size.y);
        transform.translation.y = head_y + height * 0.5;
    }
}

pub(crate) fn hit_notes(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut notes: Query<(Entity, &FallingNote, &Sprite, &Transform)>,
    mut score: ResMut<Score>,
) {
    let keys = [
        KeyCode::KeyA,
        KeyCode::KeyS,
        KeyCode::KeyD,
        KeyCode::KeyF,
        KeyCode::KeyG,
    ];
    for (entity, note, sprite, transform) in &mut notes {
        let height = sprite.custom_size.map_or(8.0, |size| size.y);
        let head_y = transform.translation.y - height * 0.5;
        if keyboard.just_pressed(keys[note.lane]) && (head_y - HIT_LINE_Y).abs() < 55.0 {
            commands.entity(entity).despawn();
            score.hits += 1;
            score.combo += 1;
            score.accuracy = (score.accuracy * (score.hits - 1) as f32 + 1.0) / score.hits as f32;
        }
        if head_y < -330.0 {
            commands.entity(entity).despawn();
            score.combo = 0;
        }
    }
}

pub(crate) fn lane_color(lane: usize) -> Color {
    match lane {
        0 => Color::srgb(0.95, 0.2, 0.25),
        1 => Color::srgb(0.95, 0.65, 0.18),
        2 => Color::srgb(0.25, 0.85, 0.45),
        3 => Color::srgb(0.25, 0.55, 1.0),
        _ => Color::srgb(0.8, 0.3, 0.95),
    }
}

pub(crate) fn instrument_color(instrument: Instrument, lane: usize) -> Color {
    match instrument.kind {
        InstrumentKind::Strings => {
            if instrument.strings >= 6 {
                lane_color(lane)
            } else {
                Color::srgb(0.95, 0.45, 0.15)
            }
        }
        InstrumentKind::Percussion => Color::srgb(0.25, 0.85, 0.45),
        InstrumentKind::Voice => Color::srgb(0.8, 0.3, 0.95),
    }
}
