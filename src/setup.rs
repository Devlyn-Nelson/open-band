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
    if !selection.slots.is_empty() {
        if keyboard.just_pressed(KeyCode::ArrowUp) {
            selection.focus = selection
                .focus
                .checked_sub(1)
                .unwrap_or(selection.slots.len() - 1);
        }
        if keyboard.just_pressed(KeyCode::ArrowDown) {
            selection.focus = (selection.focus + 1) % selection.slots.len();
        }
    }
    if keyboard.just_pressed(KeyCode::ArrowLeft) {
        cycle_slot_device(&mut selection, -1);
    }
    if keyboard.just_pressed(KeyCode::ArrowRight) {
        cycle_slot_device(&mut selection, 1);
    }
    if keyboard.just_pressed(KeyCode::KeyN) {
        selection
            .slots
            .push(InstrumentSlot::default_for(InstrumentKind::Strings));
        selection.focus = selection.slots.len() - 1;
    }
    if keyboard.just_pressed(KeyCode::KeyX) && !selection.slots.is_empty() {
        let focus = selection.focus;
        selection.slots.remove(focus);
        selection.focus = selection.focus.min(selection.slots.len().saturating_sub(1));
    }
    if keyboard.just_pressed(KeyCode::KeyK) {
        let focus = selection.focus;
        if let Some(slot) = selection.slots.get_mut(focus) {
            let next_kind = match slot.kind {
                InstrumentKind::Strings => InstrumentKind::Percussion,
                InstrumentKind::Percussion => InstrumentKind::Voice,
                InstrumentKind::Voice => InstrumentKind::Strings,
            };
            *slot = InstrumentSlot::default_for(next_kind);
        }
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        let focus = selection.focus;
        if let Some(slot) = selection.slots.get_mut(focus)
            && slot.kind == InstrumentKind::Strings
        {
            slot.detector = match slot.detector {
                DetectorProfile::Polyphonic => DetectorProfile::PerString,
                DetectorProfile::PerString => DetectorProfile::Polyphonic,
            };
        }
    }
    if keyboard.just_pressed(KeyCode::BracketLeft) {
        cycle_slot_tuning(&mut selection, -1);
        cycle_slot_kit(&mut selection, -1);
    }
    if keyboard.just_pressed(KeyCode::BracketRight) {
        cycle_slot_tuning(&mut selection, 1);
        cycle_slot_kit(&mut selection, 1);
    }

    if keyboard.just_pressed(KeyCode::Enter) {
        commit_device_selection(&selection, &mut stream, &mut settings, &mut next_state);
    }
}

/// Cycles the focused `Percussion` slot's kit among the loaded `kits/` library, starting
/// from whichever library entry currently matches (or the start of the list).
fn cycle_slot_kit(selection: &mut DeviceSelection, delta: i32) {
    if selection.kit_library.is_empty() {
        return;
    }
    let Some(slot) = selection.slots.get(selection.focus) else {
        return;
    };
    if slot.kind != InstrumentKind::Percussion {
        return;
    }
    let current = slot
        .kit
        .as_ref()
        .and_then(|kit| {
            selection
                .kit_library
                .iter()
                .position(|named| named.pieces == kit.pieces && named.lanes == kit.lanes)
        })
        .unwrap_or(0);
    let len = selection.kit_library.len() as i32;
    let next = (current as i32 + delta).rem_euclid(len) as usize;
    if let Some(slot) = selection.slots.get_mut(selection.focus) {
        slot.kit = Some(selection.kit_library[next].kit());
    }
}

/// Cycles the focused `Strings` slot's tuning among the loaded `tunings/` library,
/// starting from whichever library entry currently matches (or the start of the list).
fn cycle_slot_tuning(selection: &mut DeviceSelection, delta: i32) {
    if selection.tuning_library.is_empty() {
        return;
    }
    let Some(slot) = selection.slots.get(selection.focus) else {
        return;
    };
    if slot.kind != InstrumentKind::Strings {
        return;
    }
    let current = slot
        .tuning
        .as_ref()
        .and_then(|tuning| {
            selection
                .tuning_library
                .iter()
                .position(|named| named.strings == tuning.strings)
        })
        .unwrap_or(0);
    let len = selection.tuning_library.len() as i32;
    let next = (current as i32 + delta).rem_euclid(len) as usize;
    if let Some(slot) = selection.slots.get_mut(selection.focus) {
        slot.tuning = Some(selection.tuning_library[next].tuning());
    }
}

/// Cycles the focused slot's device selection among the device list matching its kind.
fn cycle_slot_device(selection: &mut DeviceSelection, delta: i32) {
    let Some(slot) = selection.slots.get(selection.focus).cloned() else {
        return;
    };
    let devices = selection.devices_for(slot.kind).to_vec();
    if devices.is_empty() {
        return;
    }
    let current = slot
        .device
        .as_deref()
        .and_then(|id| devices.iter().position(|device| device.id == id))
        .unwrap_or(0);
    let next = (current as i32 + delta).rem_euclid(devices.len() as i32) as usize;
    if let Some(slot) = selection.slots.get_mut(selection.focus) {
        slot.device = Some(devices[next].id.clone());
    }
}

pub(crate) fn commit_device_selection(
    selection: &DeviceSelection,
    stream: &mut InstrumentStream,
    settings: &mut PersistentSettings,
    next_state: &mut NextState<AppState>,
) {
    let config = InputConfig {
        slots: selection.slots.clone(),
    };
    settings.slots = selection.slots.clone();
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
    let mut slot_lines = String::new();
    for (index, slot) in selection.slots.iter().enumerate() {
        let marker = if selection.focus == index { ">" } else { " " };
        let devices = selection.devices_for(slot.kind);
        let device_label = slot
            .device
            .as_deref()
            .and_then(|id| devices.iter().find(|device| device.id == id))
            .map(|device| format!("{}  (ID: {})", device.label, device.id))
            .unwrap_or_else(|| "NO DEVICE SELECTED".into());
        let detail = if slot.kind == InstrumentKind::Strings {
            format!(" [{:?}]", slot.detector)
        } else {
            String::new()
        };
        let preset = match slot.kind {
            InstrumentKind::Strings | InstrumentKind::Percussion => selection
                .preset_name_for(slot)
                .map_or_else(|| " (custom)".to_string(), |name| format!(" — {name}")),
            InstrumentKind::Voice => String::new(),
        };
        slot_lines.push_str(&format!(
            "{marker} [{}] {}{preset}{detail}\n      {device_label}\n\n",
            index + 1,
            slot.label(),
        ));
    }
    if selection.slots.is_empty() {
        slot_lines.push_str("  (no instrument slots configured; press N to add one)\n\n");
    }
    *text = Text::new(format!(
        "OPEN BAND  //  INPUT DEVICES\n\n\
        {slot_lines}\
        Up/Down: focus slot     Left/Right: choose device\n\
        N: add slot     X: remove focused slot     K: cycle slot kind\n\
        [ / ]: cycle tuning (Strings) / kit (Percussion)     P: detector profile (Strings only)\n\
        Enter: continue\n\
        Environment variables remain supported as first-run defaults.",
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
    selection: Res<DeviceSelection>,
    stream: Res<InstrumentStream>,
    time: Res<Time>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Setup);
        return;
    }
    let digit_keys = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    for (index, key) in digit_keys.into_iter().enumerate() {
        if index < selection.slots.len() && keyboard.just_pressed(key) {
            calibration.selected = index;
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
        if event.instrument.slot == calibration.selected {
            calibration.level = event.strength;
            calibration.peak = calibration.peak.max(event.strength);
            calibration.samples += 1;
            calibration.last_pitch_hz = event.pitch_hz;
        }
    }
}

pub(crate) fn calibration_display(
    calibration: Res<Calibration>,
    selection: Res<DeviceSelection>,
    mut text: Query<&mut Text, With<CalibrationText>>,
    mut meter: Query<&mut Transform, With<CalibrationMeter>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let slot_list = selection
        .slots
        .iter()
        .enumerate()
        .map(|(index, slot)| format!("[{}] {}", index + 1, slot.label()))
        .collect::<Vec<_>>()
        .join("   ");
    let slot = selection.slots.get(calibration.selected);
    let instrument = slot.map_or_else(|| "NONE CONFIGURED".into(), InstrumentSlot::label);
    let tuner = slot.map_or_else(
        || "TUNER: no instrument slots configured".into(),
        |slot| bass_tuner_reading(slot.kind, slot.tuning.as_ref(), calibration.last_pitch_hz),
    );
    let status = if calibration.samples > 0 {
        "SIGNAL DETECTED"
    } else {
        "WAITING FOR INPUT"
    };
    *text = Text::new(format!(
        concat!(
            "OPEN BAND  //  INPUT CALIBRATION\n\n",
            "{}\n\n",
            "ACTIVE: {}\n\n",
            "{}\n\n",
            "{}\nLEVEL  {:>3.0}%     PEAK  {:>3.0}%\n\n",
            "Play the selected instrument. Press ENTER when ready."
        ),
        slot_list,
        instrument,
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

pub(crate) fn bass_tuner_reading(
    kind: InstrumentKind,
    tuning: Option<&Tuning>,
    pitch_hz: Option<f32>,
) -> String {
    if kind != InstrumentKind::Strings {
        return format!("TUNER: not applicable to {}", instrument_name(kind, 0));
    }
    let Some(pitch_hz) = pitch_hz else {
        return "STRING TUNER\nPlay an open string to begin tuning.".into();
    };
    let tuning = tuning.cloned().unwrap_or_default();
    let Ok(open_frequencies) = tuning.open_frequencies() else {
        return "STRING TUNER\nConfigured tuning failed to parse.".into();
    };
    let (index, target) = open_frequencies
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
    let verdict = if cents.abs() < 5.0 {
        "IN TUNE"
    } else if cents < 0.0 {
        "TUNE UP"
    } else {
        "TUNE DOWN"
    };
    let note = tuning.strings.get(index).map_or("?", String::as_str);
    format!(
        "STRING TUNER\nSTRING  {} ({note})\nPITCH   {pitch_hz:>6.2} Hz\nOFFSET  {cents:>+6.1} cents   {verdict}",
        index + 1
    )
}

pub(crate) fn instrument_name(kind: InstrumentKind, strings: u8) -> String {
    match kind {
        InstrumentKind::Strings => format!("STRINGS ({strings}-STRING)"),
        InstrumentKind::Percussion => "PERCUSSION (MIDI)".into(),
        InstrumentKind::Voice => "VOICE".into(),
    }
}
