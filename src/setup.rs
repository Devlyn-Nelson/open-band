use super::*;

pub(crate) fn setup_device_selection(mut commands: Commands) {
    commands.spawn((Camera2d, DeviceSelectionCamera));
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(25.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(44.0),
            left: Val::Px(70.0),
            ..default()
        },
        DeviceSelectionText,
    ));
}

pub(crate) fn device_selection_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<DeviceSelection>,
    mut stream: ResMut<InstrumentStream>,
    mut settings: ResMut<PersistentSettings>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let focus_keys = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ];
    for (index, key) in focus_keys.into_iter().enumerate() {
        if keyboard.just_pressed(key) {
            selection.focus = index;
        }
    }
    if keyboard.just_pressed(KeyCode::KeyB) {
        selection.bass_strings = if selection.bass_strings == 4 { 5 } else { 4 };
    }

    let device_count = if selection.focus == 2 {
        selection.midi_devices.len()
    } else {
        selection.audio_devices.len()
    };
    if device_count > 0 && keyboard.just_pressed(KeyCode::ArrowLeft) {
        let focus = selection.focus;
        selection.selected[focus] = selection.selected[focus]
            .checked_sub(1)
            .unwrap_or(device_count - 1);
    }
    if device_count > 0 && keyboard.just_pressed(KeyCode::ArrowRight) {
        let focus = selection.focus;
        selection.selected[focus] = (selection.selected[focus] + 1) % device_count;
    }

    if keyboard.just_pressed(KeyCode::Enter) {
        commit_device_selection(&selection, &mut stream, &mut settings, &mut next_state);
    }
}

pub(crate) fn commit_device_selection(
    selection: &DeviceSelection,
    stream: &mut InstrumentStream,
    settings: &mut PersistentSettings,
    next_state: &mut NextState<AppState>,
) {
    let audio_id = |index: usize| {
        selection
            .audio_devices
            .get(selection.selected[index])
            .map(|device| device.id.clone())
    };
    let midi_id = selection
        .midi_devices
        .get(selection.selected[2])
        .map(|device| device.id.clone());
    let config = InputConfig {
        audio_devices: [audio_id(0), audio_id(1), audio_id(3)],
        midi_device: midi_id,
        bass_strings: selection.bass_strings,
    };
    settings.guitar_device = config.audio_devices[0].clone();
    settings.bass_device = config.audio_devices[1].clone();
    settings.vocal_device = config.audio_devices[2].clone();
    settings.midi_device = config.midi_device.clone();
    settings.bass_strings = Some(config.bass_strings);
    save_settings(settings);
    if let Some(stop_sender) = stream.stop_sender.take() {
        let _ = stop_sender.send(());
        if let Some(thread) = stream._thread.take() {
            let _ = thread.join();
        }
    }
    let (stop_sender, stop_receiver) = mpsc::channel();
    stream._thread = Some(spawn_instrument_thread(
        stream.sender.clone(),
        config,
        stop_receiver,
    ));
    stream.stop_sender = Some(stop_sender);
    stream.started = true;
    next_state.set(AppState::Setup);
}

pub(crate) fn device_selection_display(
    selection: Res<DeviceSelection>,
    mut text: Query<&mut Text, With<DeviceSelectionText>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let device_name = |devices: &[DeviceChoice], selected: usize| {
        devices
            .get(selected)
            .map(|device| format!("{}\n      ID: {}", device.label, device.id))
            .unwrap_or_else(|| "NO DEVICE FOUND".into())
    };
    let marker = |index: usize| if selection.focus == index { ">" } else { " " };
    *text = Text::new(format!(
        "OPEN BAND  //  INPUT DEVICES\n\n\
        {} [1] GUITAR\n      {}\n\n\
        {} [2] BASS (B to switch {}-string)\n      {}\n\n\
        {} [3] MIDI DRUMS\n      {}\n\n\
        {} [4] VOCALS\n      {}\n\n\
        Left/Right: choose device     Enter: continue\n\
        Environment variables remain supported as defaults.",
        marker(0),
        device_name(&selection.audio_devices, selection.selected[0]),
        marker(1),
        selection.bass_strings,
        device_name(&selection.audio_devices, selection.selected[1]),
        marker(2),
        device_name(&selection.midi_devices, selection.selected[2]),
        marker(3),
        device_name(&selection.audio_devices, selection.selected[3]),
    ));
}

pub(crate) fn cleanup_device_selection(
    mut commands: Commands,
    entities: Query<Entity, Or<(With<DeviceSelectionText>, With<DeviceSelectionCamera>)>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

pub(crate) fn setup_calibration(mut commands: Commands) {
    commands.spawn((Camera2d, CalibrationCamera));
    commands.spawn((
        Text::new("OPEN BAND  //  INPUT CALIBRATION"),
        TextFont {
            font_size: FontSize::Px(34.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(42.0),
            left: Val::Px(70.0),
            ..default()
        },
        CalibrationText,
    ));
    commands.spawn((
        Sprite {
            color: Color::srgb(0.15, 0.2, 0.3),
            custom_size: Some(Vec2::new(760.0, 26.0)),
            ..default()
        },
        Transform::from_xyz(0.0, -160.0, 0.0),
        CalibrationMeter,
    ));
}

pub(crate) fn setup_latency_calibration(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<PersistentSettings>,
) {
    commands.insert_resource(LatencyCalibration {
        started_at: time.elapsed_secs(),
        best_ms: settings.latency_ms,
        attempts: 0,
    });
    commands.spawn((Camera2d, LatencyCamera));
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(27.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(34.0),
            left: Val::Px(52.0),
            ..default()
        },
        LatencyText,
    ));
    for lane in 0..LANES {
        let x = -360.0 + lane as f32 * 180.0;
        commands.spawn((
            Sprite {
                color: lane_color(lane).with_alpha(0.2),
                custom_size: Some(Vec2::new(122.0, 520.0)),
                ..default()
            },
            Transform::from_xyz(x, -40.0, 0.0),
            LatencyEntity,
        ));
    }
    commands.spawn((
        Sprite {
            color: Color::srgb(1.0, 0.85, 0.25),
            custom_size: Some(Vec2::new(9.0, 520.0)),
            ..default()
        },
        Transform::from_xyz(0.0, -40.0, 1.0),
        LatencyPulse,
        LatencyEntity,
    ));
}

pub(crate) fn latency_calibration_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut latency: ResMut<LatencyCalibration>,
    mut settings: ResMut<PersistentSettings>,
    mut next_state: ResMut<NextState<AppState>>,
    time: Res<Time>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Setup);
        return;
    }
    if keyboard.just_pressed(KeyCode::Space) {
        let beat = (time.elapsed_secs() - latency.started_at) % 0.5;
        let offset = if beat > 0.25 { beat - 0.5 } else { beat };
        let offset_ms = offset * 1000.0;
        latency.best_ms = Some(
            latency
                .best_ms
                .map_or(offset_ms.abs(), |best| best.min(offset_ms.abs())),
        );
        latency.attempts += 1;
    }
    if latency.attempts > 0 && keyboard.just_pressed(KeyCode::Enter) {
        settings.latency_ms = latency.best_ms;
        save_settings(&settings);
        next_state.set(AppState::Setup);
    }
}

pub(crate) fn latency_calibration_display(
    latency: Res<LatencyCalibration>,
    time: Res<Time>,
    mut text: Query<&mut Text, With<LatencyText>>,
    mut pulse: Query<&mut Transform, With<LatencyPulse>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let elapsed = time.elapsed_secs() - latency.started_at;
    let phase = (elapsed % 0.5) / 0.5;
    if let Ok(mut transform) = pulse.single_mut() {
        transform.translation.x = -360.0 + phase * 720.0;
    }
    let result = latency.best_ms.map_or("NO TAP RECORDED".into(), |ms| {
        format!("BEST OFFSET  {ms:>5.1} ms")
    });
    *text = Text::new(format!(
        "OPEN BAND  //  LATENCY CALIBRATION\n\n\
        LIVE SESSION CHECK\n\n\
        Tap SPACE as the yellow beat line crosses the center marker.\n\
        Attempts: {}\n{}\n\n\
        Press ENTER to accept and return to Set Up.",
        latency.attempts, result,
    ));
}

pub(crate) fn cleanup_latency_calibration(
    mut commands: Commands,
    entities: Query<
        Entity,
        Or<(
            With<LatencyText>,
            With<LatencyPulse>,
            With<LatencyCamera>,
            With<LatencyEntity>,
        )>,
    >,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

pub(crate) fn calibration_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut calibration: ResMut<Calibration>,
    stream: Res<InstrumentStream>,
    time: Res<Time>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Setup);
        return;
    }
    let choices = [
        (KeyCode::Digit1, Instrument::Guitar),
        (KeyCode::Digit2, Instrument::Bass4),
        (KeyCode::Digit3, Instrument::Bass5),
        (KeyCode::Digit4, Instrument::Drums),
        (KeyCode::Digit5, Instrument::Vocals),
    ];
    for (key, instrument) in choices {
        if keyboard.just_pressed(key) {
            calibration.selected = instrument;
            calibration.level = 0.0;
            calibration.peak = 0.0;
            calibration.samples = 0;
            calibration.last_pitch_hz = None;
        }
    }
    if keyboard.just_pressed(KeyCode::Enter) {
        next_state.set(AppState::Setup);
    }

    calibration.level = (calibration.level - time.delta_secs() * 0.7).max(0.0);
    let Ok(events) = stream.events.lock() else {
        return;
    };
    for event in events.try_iter() {
        if event.instrument as u8 == calibration.selected as u8 {
            calibration.level = event.strength;
            calibration.peak = calibration.peak.max(event.strength);
            calibration.samples += 1;
            calibration.last_pitch_hz = event.pitch_hz;
        }
    }
}

pub(crate) fn calibration_display(
    calibration: Res<Calibration>,
    mut text: Query<&mut Text, With<CalibrationText>>,
    mut meter: Query<&mut Transform, With<CalibrationMeter>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let instrument = instrument_name(calibration.selected);
    let source = input_source(calibration.selected);
    let tuner = bass_tuner_reading(calibration.selected, calibration.last_pitch_hz);
    let status = if calibration.samples > 0 {
        "SIGNAL DETECTED"
    } else {
        "WAITING FOR INPUT"
    };
    *text = Text::new(format!(
        concat!(
            "OPEN BAND  //  INPUT CALIBRATION\n\n",
            "[1] GUITAR       [2] BASS 4-STRING\n",
            "[3] BASS 5-STRING [4] MIDI DRUMS\n",
            "[5] VOCALS\n\n",
            "ACTIVE: {}\nSOURCE: {}\n\n",
            "{}\n\n",
            "{}\nLEVEL  {:>3.0}%     PEAK  {:>3.0}%\n\n",
            "Play the selected instrument. Press ENTER when ready."
        ),
        instrument,
        source,
        tuner,
        status,
        calibration.level * 100.0,
        calibration.peak * 100.0,
    ));
    if let Ok(mut transform) = meter.single_mut() {
        transform.scale.x = calibration.level.max(0.02);
    }
}

pub(crate) fn cleanup_calibration(
    mut commands: Commands,
    entities: Query<
        Entity,
        Or<(
            With<CalibrationText>,
            With<CalibrationMeter>,
            With<CalibrationCamera>,
        )>,
    >,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

pub(crate) fn bass_tuner_reading(instrument: Instrument, pitch_hz: Option<f32>) -> String {
    if !matches!(instrument, Instrument::Bass4 | Instrument::Bass5) {
        return "TUNER: select a bass input with [2] or [3]".into();
    }
    let Some(pitch_hz) = pitch_hz else {
        return "BASS TUNER\nPlay an open string to begin tuning.".into();
    };
    let targets = if matches!(instrument, Instrument::Bass5) {
        [
            (30.87, "B"),
            (41.20, "E"),
            (55.00, "A"),
            (73.42, "D"),
            (98.00, "G"),
        ]
    } else {
        [
            (41.20, "E"),
            (55.00, "A"),
            (73.42, "D"),
            (98.00, "G"),
            (98.00, "G"),
        ]
    };
    let (target, note) = targets
        .into_iter()
        .min_by(|(left, _), (right, _)| {
            (pitch_hz - left).abs().total_cmp(&(pitch_hz - right).abs())
        })
        .unwrap();
    let cents = 1200.0 * (pitch_hz / target).log2();
    let verdict = if cents.abs() < 5.0 {
        "IN TUNE"
    } else if cents < 0.0 {
        "TUNE UP"
    } else {
        "TUNE DOWN"
    };
    format!(
        "BASS TUNER\nNOTE  {note}\nPITCH  {pitch_hz:>6.2} Hz\nOFFSET {cents:>+6.1} cents   {verdict}"
    )
}

pub(crate) fn instrument_name(instrument: Instrument) -> &'static str {
    match instrument {
        Instrument::Guitar => "GUITAR",
        Instrument::Bass4 => "BASS 4-STRING",
        Instrument::Bass5 => "BASS 5-STRING",
        Instrument::Drums => "MIDI DRUMS",
        Instrument::Vocals => "VOCALS",
    }
}

pub(crate) fn input_source(instrument: Instrument) -> String {
    let _ = instrument;
    "selected in device dialog".into()
}
