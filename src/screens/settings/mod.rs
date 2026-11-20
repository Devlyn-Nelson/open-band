use crate::*;
use inlet::InletEvent;
use serde::{Deserialize, Serialize};

#[derive(Resource)]
/// Available devices and current choices in Input Setup.
///
/// AI_CODE
pub(crate) struct DeviceSelection {
    pub(crate) audio_devices: Vec<DeviceChoice>,
    pub(crate) midi_devices: Vec<DeviceChoice>,
    pub(crate) tuning_library: Vec<NamedTuning>,
    pub(crate) kit_library: Vec<NamedKit>,
    pub(crate) slots: Vec<InstrumentSlot>,
    pub(crate) focus: usize,
}

impl DeviceSelection {
    /// The device list a slot's device selection should cycle through.
    ///
    /// AI_CODE
    pub(crate) fn devices_for(&self, kind: InstrumentKind) -> &[DeviceChoice] {
        match kind {
            InstrumentKind::Percussion => &self.midi_devices,
            InstrumentKind::Strings | InstrumentKind::Voice => &self.audio_devices,
        }
    }

    /// The library preset name matching a slot's configured tuning/kit, if any (a slot
    /// loaded from an older settings file, or with a hand-edited tuning/kit, may not
    /// match a current library entry).
    ///
    /// AI_CODE
    pub(crate) fn preset_name_for(&self, slot: &InstrumentSlot) -> Option<&str> {
        match slot.kind {
            InstrumentKind::Strings => {
                let tuning = slot.tuning.as_ref()?;
                self.tuning_library
                    .iter()
                    .find(|named| named.strings == tuning.strings)
                    .map(|named| named.name.as_str())
            }
            InstrumentKind::Percussion => {
                let kit = slot.kit.as_ref()?;
                self.kit_library
                    .iter()
                    .find(|named| named.lanes == kit.lanes && named.pieces == kit.pieces)
                    .map(|named| named.name.as_str())
            }
            InstrumentKind::Voice => None,
        }
    }
}

/// AI_CODE
#[derive(Clone, Debug)]
pub(crate) struct DeviceChoice {
    pub(crate) id: String,
    pub(crate) label: String,
}

#[test]
/// Selects the exact stable ID when display names are duplicated.
fn device_selection_prefers_stable_id_over_duplicate_name() {
    let devices = vec![
        DeviceChoice {
            id: "alsa:hw:1,0".into(),
            label: "USB Audio".into(),
        },
        DeviceChoice {
            id: "alsa:hw:2,0".into(),
            label: "USB Audio".into(),
        },
    ];
    assert_eq!(
        selected_device_index(&devices, Some("alsa:hw:2,0"), "MISSING"),
        1
    );
}

#[derive(Resource, Serialize, Deserialize, Default, Debug)]
/// Settings persisted between launches in `open-band-settings/settings.json`.
///
/// AI_CODE
pub(crate) struct PersistentSettings {
    #[serde(default)]
    pub(crate) slots: Vec<InstrumentSlot>,
    pub(crate) latency_ms: Option<f32>,
}

#[derive(Component)]
pub(crate) struct DeviceSelectionText;

#[derive(Component)]
pub(crate) struct DeviceSelectionCamera;

#[derive(Resource)]
/// Signal and tuner state displayed during instrument calibration.
///
/// AI_CODE
pub(crate) struct Calibration {
    /// Index into the configured `InstrumentSlot` list.
    pub(crate) selected: usize,
    pub(crate) level: f32,
    pub(crate) peak: f32,
    pub(crate) samples: u32,
    pub(crate) last_pitch_hz: Option<f32>,
    pub(crate) input_active: bool,
}

#[derive(Component)]
pub(crate) struct CalibrationText;

#[derive(Component)]
pub(crate) struct CalibrationMeter;

#[derive(Component)]
pub(crate) struct CalibrationCamera;

#[derive(Resource)]
/// Timing measurements collected by the latency calibration screen.
///
/// AI_CODE
pub(crate) struct LatencyCalibration {
    pub(crate) started_at: f32,
    pub(crate) best_ms: Option<f32>,
    pub(crate) attempts: u32,
}

#[derive(Component)]
pub(crate) struct LatencyText;

#[derive(Component)]
pub(crate) struct LatencyPulse;

#[derive(Component)]
pub(crate) struct LatencyEntity;

#[derive(Component)]
pub(crate) struct LatencyCamera;

#[derive(Resource, Default)]
/// Arming and menu focus for the audio recorder.
///
/// AI_CODE
pub(crate) struct RecordingMenu {
    pub(crate) selected: usize,
    pub(crate) armed: Vec<bool>,
    pub(crate) message: Option<String>,
}

#[derive(Component)]
/// Text node owned by the input recorder screen.
///
/// AI_CODE
pub(crate) struct RecordingText;

/// AI_CODE
pub(crate) fn setup_recording_screen(
    mut commands: Commands,
    selection: Res<DeviceSelection>,
    mut menu: ResMut<RecordingMenu>,
) {
    menu.armed.resize(selection.slots.len(), false);
    for (armed, slot) in menu.armed.iter_mut().zip(&selection.slots) {
        if slot.kind == InstrumentKind::Percussion {
            *armed = false;
        }
    }
    menu.selected = menu.selected.min(selection.slots.len() + 1);
    menu.message = None;
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(24.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(42.0),
            left: Val::Px(64.0),
            ..default()
        },
        RecordingText,
    ));
}

/// AI_CODE
pub(crate) fn recording_screen_input(
    mut nav_events: MessageReader<InletEvent<NavigationMessage>>,
    mut next_state: ResMut<NextState<AppState>>,
    selection: Res<DeviceSelection>,
    mut menu: ResMut<RecordingMenu>,
    stream: Res<InstrumentStream>,
) {
    menu.armed.resize(selection.slots.len(), false);
    for (armed, slot) in menu.armed.iter_mut().zip(&selection.slots) {
        if slot.kind == InstrumentKind::Percussion {
            *armed = false;
        }
    }
    let rows = selection.slots.len() + 2;
    for event in nav_events.read() {
        match event.kind {
            NavigationMessage::Up => {
                menu.selected = menu.selected.checked_sub(1).unwrap_or(rows - 1)
            }
            NavigationMessage::Down => menu.selected = (menu.selected + 1) % rows,
            NavigationMessage::Slot(number) => {
                menu.selected = number
                    .saturating_sub(1)
                    .min(selection.slots.len().saturating_sub(1));
            }
            NavigationMessage::Select => {
                menu.message = None;
                if menu.selected < selection.slots.len() {
                    if selection.slots[menu.selected].kind == InstrumentKind::Percussion {
                        menu.message =
                            Some("MIDI inputs cannot be mixed into a WAV recording.".into());
                    } else {
                        let index = menu.selected;
                        menu.armed[index] = !menu.armed[index];
                    }
                } else if menu.selected == selection.slots.len() {
                    toggle_recording(&menu.armed, &stream);
                } else {
                    stop_recording(&stream);
                    next_state.set(AppState::Settings);
                }
            }
            NavigationMessage::Back => {
                stop_recording(&stream);
                next_state.set(AppState::Settings);
            }
            _ => {}
        }
    }
}

/// AI_CODE
fn toggle_recording(armed: &[bool], stream: &InstrumentStream) {
    match stream.recording_status.get() {
        RecordingStatus::Active(path) => {
            let _ = stream.recording_commands.send(RecordingCommand::Stop);
            stream
                .recording_status
                .set(RecordingStatus::Stopping(Some(path)));
        }
        RecordingStatus::Inactive(_) | RecordingStatus::Error(_) => {
            let armed_slots = armed
                .iter()
                .enumerate()
                .filter_map(|(slot, armed)| armed.then_some(slot))
                .collect::<Vec<_>>();
            if armed_slots.is_empty() {
                stream.recording_status.set(RecordingStatus::Error(
                    "Arm at least one audio input first.".into(),
                ));
                return;
            }
            stream.recording_status.set(RecordingStatus::Starting);
            if let Err(error) = stream
                .recording_commands
                .send(RecordingCommand::Start(armed_slots))
            {
                stream
                    .recording_status
                    .set(RecordingStatus::Error(error.to_string()));
            }
        }
        RecordingStatus::Starting | RecordingStatus::Stopping(_) => {}
    }
}

/// AI_CODE
fn stop_recording(stream: &InstrumentStream) {
    match stream.recording_status.get() {
        RecordingStatus::Active(path) => {
            let _ = stream.recording_commands.send(RecordingCommand::Stop);
            stream
                .recording_status
                .set(RecordingStatus::Stopping(Some(path)));
        }
        RecordingStatus::Starting => {
            let _ = stream.recording_commands.send(RecordingCommand::Stop);
            stream.recording_status.set(RecordingStatus::Stopping(None));
        }
        _ => {}
    }
}

/// AI_CODE
pub(crate) fn recording_screen_display(
    selection: Res<DeviceSelection>,
    menu: Res<RecordingMenu>,
    stream: Res<InstrumentStream>,
    mut text: Query<&mut Text, With<RecordingText>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let mut rows = selection
        .slots
        .iter()
        .enumerate()
        .map(|(index, slot)| {
            let marker = if menu.selected == index { ">" } else { " " };
            if slot.kind == InstrumentKind::Percussion {
                format!(
                    "{marker} [MIDI] {}  (not recordable as WAV)\n",
                    slot.label()
                )
            } else {
                let armed = menu.armed.get(index).copied().unwrap_or(false);
                format!(
                    "{marker} [{}] {}\n",
                    if armed { "x" } else { " " },
                    slot.label()
                )
            }
        })
        .collect::<Vec<_>>();
    let record_marker = if menu.selected == selection.slots.len() {
        ">"
    } else {
        " "
    };
    let record_label = match stream.recording_status.get() {
        RecordingStatus::Active(_) | RecordingStatus::Stopping(_) => "STOP RECORDING",
        _ => "START RECORDING",
    };
    rows.push(format!("{record_marker} {record_label}\n"));
    let back_marker = if menu.selected == selection.slots.len() + 1 {
        ">"
    } else {
        " "
    };
    rows.push(format!("{back_marker} BACK\n"));

    let status = match stream.recording_status.get() {
        RecordingStatus::Inactive(Some(path)) => format!("INACTIVE  |  SAVED {path}"),
        RecordingStatus::Inactive(None) => "INACTIVE".into(),
        RecordingStatus::Starting => "STARTING...".into(),
        RecordingStatus::Active(path) => format!("RECORDING  |  {path}"),
        RecordingStatus::Stopping(path) => path.map_or_else(
            || "STOPPING...".into(),
            |path| format!("STOPPING  |  {path}"),
        ),
        RecordingStatus::Error(error) => format!("ERROR  |  {error}"),
    };
    let message = menu.message.as_deref().unwrap_or("");
    *text = Text::new(format!(
        "OPEN BAND  //  INPUT RECORDER\n\n{}\n\nSTATUS: {status}\n{message}\n\nUp/Down: select   Enter: arm / record / stop   Esc: back\nAudio inputs are mixed into a mono WAV in recordings/.",
        rows.join(""),
    ));
}

/// AI_CODE
pub(crate) fn cleanup_recording_screen(
    mut commands: Commands,
    stream: Res<InstrumentStream>,
    entities: Query<Entity, With<RecordingText>>,
) {
    stop_recording(&stream);
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

/// AI_CODE
pub(crate) fn setup_device_selection(mut commands: Commands) {
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

/// AI_CODE
pub(crate) fn device_selection_input(
    mut nav_events: MessageReader<InletEvent<NavigationMessage>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<DeviceSelection>,
    mut stream: ResMut<InstrumentStream>,
    mut settings: ResMut<PersistentSettings>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for event in nav_events.read() {
        match event.kind {
            NavigationMessage::Select => {
                commit_device_selection(&selection, &mut stream, &mut settings);
                next_state.set(AppState::Settings);
            }
            NavigationMessage::Back => next_state.set(AppState::Settings),
            NavigationMessage::Up => {
                selection.focus = selection
                    .focus
                    .checked_sub(1)
                    .unwrap_or(selection.slots.len() - 1)
            }
            NavigationMessage::Down => {
                selection.focus = (selection.focus + 1) % selection.slots.len()
            }
            NavigationMessage::Right => cycle_slot_device(&mut selection, 1),
            NavigationMessage::Left => cycle_slot_device(&mut selection, -1),
            NavigationMessage::Slot(_) => {}
        }
    }
    // \/ AI_CODE \/
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
    // /\ AI_CODE /\
}

/// Cycles the focused `Percussion` slot's kit among the loaded `kits/` library, starting
/// from whichever library entry currently matches (or the start of the list).
///
/// AI_CODE
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
///
/// AI_CODE
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
///
/// AI_CODE
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

/// AI_CODE
pub(crate) fn commit_device_selection(
    selection: &DeviceSelection,
    stream: &mut InstrumentStream,
    settings: &mut PersistentSettings,
) {
    let config = InputConfig {
        slots: selection.slots.clone(),
    };
    settings.slots = selection.slots.clone();
    save_settings(settings);
    if let Some(stop_sender) = stream.stop_sender.take() {
        let _ = stream.recording_commands.send(RecordingCommand::Stop);
        let _ = stop_sender.send(());
        if let Some(thread) = stream._thread.take() {
            let _ = thread.join();
        }
    }
    stream.input_levels.resize(selection.slots.len());
    stream.monitor_mixer.resize(selection.slots.len());
    let (recording_commands, recording_receiver) = mpsc::channel();
    stream.recording_commands = recording_commands;
    let (stop_sender, stop_receiver) = mpsc::channel();
    stream._thread = Some(spawn_instrument_thread(
        stream.sender.clone(),
        config,
        stream.input_levels.clone(),
        stream.monitor_mixer.clone(),
        recording_receiver,
        stream.recording_status.clone(),
        stop_receiver,
    ));
    stream.stop_sender = Some(stop_sender);
    stream.started = true;
}

/// AI_CODE
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

/// AI_CODE
pub(crate) fn cleanup_device_selection(
    mut commands: Commands,
    entities: Query<Entity, With<DeviceSelectionText>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

/// AI_CODE
pub(crate) fn setup_calibration(mut commands: Commands) {
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

/// AI_CODE
pub(crate) fn settings_latency_calibration(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<PersistentSettings>,
) {
    commands.insert_resource(LatencyCalibration {
        started_at: time.elapsed_secs(),
        best_ms: settings.latency_ms,
        attempts: 0,
    });
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

/// AI_CODE
pub(crate) fn latency_calibration_input(
    mut nav_events: MessageReader<InletEvent<NavigationMessage>>,
    mut latency: ResMut<LatencyCalibration>,
    mut settings: ResMut<PersistentSettings>,
    mut next_state: ResMut<NextState<AppState>>,
    time: Res<Time>,
) {
    for event in nav_events.read() {
        match event.kind {
            NavigationMessage::Back => {
                next_state.set(AppState::Settings);
                return;
            }
            NavigationMessage::Down | NavigationMessage::Up => {
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
            NavigationMessage::Select => {
                if latency.attempts > 0 {
                    settings.latency_ms = latency.best_ms;
                    save_settings(&settings);
                    next_state.set(AppState::Settings);
                }
            }
            _ => {}
        }
    }
}

/// AI_CODE
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
        Tap Up or Down as the yellow beat line crosses the center marker.\n\
        Attempts: {}\n{}\n\n\
        Press ENTER to accept and return to Settings.",
        latency.attempts, result,
    ));
}

/// AI_CODE
pub(crate) fn cleanup_latency_calibration(
    mut commands: Commands,
    entities: Query<Entity, Or<(With<LatencyText>, With<LatencyPulse>, With<LatencyEntity>)>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

/// AI_CODE
pub(crate) fn calibration_input(
    mut nav_events: MessageReader<InletEvent<NavigationMessage>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut calibration: ResMut<Calibration>,
    selection: Res<DeviceSelection>,
    stream: Res<InstrumentStream>,
    time: Res<Time>,
) {
    let previous_selection = calibration.selected;
    for event in nav_events.read() {
        match event.kind {
            NavigationMessage::Select | NavigationMessage::Back => {
                next_state.set(AppState::Settings);
            }
            NavigationMessage::Up => {
                calibration.selected = (calibration.selected + 1) % selection.slots.len();
            }
            NavigationMessage::Down => {
                calibration.selected = calibration
                    .selected
                    .checked_sub(1)
                    .unwrap_or(selection.slots.len().checked_sub(1).unwrap_or(0))
            }
            NavigationMessage::Slot(num) => {
                let num = num.clamp(1, 9).checked_sub(1).unwrap_or(8);
                calibration.selected = num;
                calibration.level = 0.0;
                calibration.peak = 0.0;
                calibration.samples = 0;
                calibration.last_pitch_hz = None;
            }
            _ => {}
        }
    }

    if calibration.selected != previous_selection {
        calibration.level = 0.0;
        calibration.peak = 0.0;
        calibration.samples = 0;
        calibration.last_pitch_hz = None;
        calibration.input_active = false;
    }
    let is_percussion = selection
        .slots
        .get(calibration.selected)
        .is_some_and(|slot| slot.kind == InstrumentKind::Percussion);
    if is_percussion {
        calibration.level = (calibration.level - time.delta_secs() * 0.7).max(0.0);
        calibration.input_active = false;
    } else if let Some((level, active)) = stream.input_levels.latest(calibration.selected) {
        calibration.level = level;
        calibration.peak = calibration.peak.max(level);
        calibration.input_active = active;
        if !active {
            calibration.samples = 0;
            calibration.last_pitch_hz = None;
        }
    } else {
        calibration.level = 0.0;
        calibration.input_active = false;
    }
    let Ok(events) = stream.events.lock() else {
        return;
    };
    for event in events.try_iter() {
        if event.instrument.slot == calibration.selected {
            if event.phase == NotePhase::End {
                calibration.samples = 0;
                calibration.last_pitch_hz = None;
            } else {
                calibration.samples += 1;
                calibration.last_pitch_hz = Some(event.note.pitch_note());
                if is_percussion {
                    calibration.level = event.strength;
                    calibration.peak = calibration.peak.max(event.strength);
                }
            }
        }
    }
}

/// AI_CODE
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
        |slot| tuner_reading(slot.kind, slot.tuning.as_ref(), calibration.last_pitch_hz),
    );
    let is_percussion = slot.is_some_and(|slot| slot.kind == InstrumentKind::Percussion);
    let status = if is_percussion {
        if calibration.samples > 0 {
            "MIDI INPUT DETECTED"
        } else {
            "WAITING FOR MIDI INPUT"
        }
    } else if !calibration.input_active {
        "NO RECENT AUDIO FRAMES"
    } else if calibration.samples > 0 {
        "AUDIO ACTIVE - PITCH DETECTED"
    } else {
        "AUDIO ACTIVE - NO PITCH DETECTED"
    };
    let (level_label, level, peak) = if is_percussion {
        (
            "LEVEL (%)",
            calibration.level * 100.0,
            calibration.peak * 100.0,
        )
    } else {
        let to_dbfs = |value: f32| 20.0 * value.max(0.000001).log10();
        (
            "MIC RMS (dBFS)",
            to_dbfs(calibration.level),
            to_dbfs(calibration.peak),
        )
    };
    *text = Text::new(format!(
        concat!(
            "OPEN BAND  //  INPUT CALIBRATION\n\n",
            "{}\n\n",
            "ACTIVE: {}\n\n",
            "{}\n\n",
            "{}\n{}  {:>6.1}     PEAK  {:>6.1}\n\n",
            "Play the selected instrument. Press ENTER when ready."
        ),
        slot_list, instrument, tuner, status, level_label, level, peak,
    ));
    if let Ok(mut transform) = meter.single_mut() {
        let meter_level = if is_percussion {
            calibration.level
        } else {
            ((20.0 * calibration.level.max(0.000001).log10() + 60.0) / 60.0).clamp(0.02, 1.0)
        };
        transform.scale.x = meter_level.max(0.02);
    }
}

/// AI_CODE
pub(crate) fn cleanup_calibration(
    mut commands: Commands,
    entities: Query<Entity, Or<(With<CalibrationText>, With<CalibrationMeter>)>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}

/// AI_CODE
pub(crate) fn tuner_reading(
    kind: InstrumentKind,
    tuning: Option<&Tuning>,
    pitch_hz: Option<f32>,
) -> String {
    if kind == InstrumentKind::Voice {
        return voice_tuner_reading(pitch_hz);
    }
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
    let verdict = tuning_verdict(cents);
    let note = tuning.strings.get(index).map_or("?", String::as_str);
    format!(
        "STRING TUNER\nSTRING  {} ({note})\nPITCH   {pitch_hz:>6.2} Hz\nOFFSET  {cents:>+6.1} cents   {verdict}",
        index + 1
    )
}

/// AI_CODE
fn voice_tuner_reading(pitch_hz: Option<f32>) -> String {
    let Some(pitch_hz) = pitch_hz else {
        return "VOICE TUNER\nSing a note to begin tuning.".into();
    };
    let midi_note = NoteData::Pitch(pitch_hz).midi_note();
    let target_hz = midi_to_frequency(midi_note);
    let cents = 1200.0 * (pitch_hz / target_hz).log2();
    let instruction = if cents.abs() < 5.0 {
        "IN TUNE"
    } else if cents < 0.0 {
        "SING HIGHER"
    } else {
        "SING LOWER"
    };
    format!(
        "VOICE TUNER\nNOTE    {}\nPITCH   {pitch_hz:>6.2} Hz\nOFFSET  {cents:>+6.1} cents   {instruction}",
        midi_note_name(midi_note)
    )
}

/// AI_CODE
fn tuning_verdict(cents: f32) -> &'static str {
    if cents.abs() < 5.0 {
        "IN TUNE"
    } else if cents < 0.0 {
        "TUNE UP"
    } else {
        "TUNE DOWN"
    }
}

#[test]
/// Shows chromatic note and direction feedback for voice input.
/// AI_CODE
fn voice_tuner_reports_nearest_note_and_singing_direction() {
    let reading = voice_tuner_reading(Some(445.0));
    assert!(reading.contains("NOTE    A4"));
    assert!(reading.contains("SING LOWER"));

    let reading = voice_tuner_reading(Some(435.0));
    assert!(reading.contains("SING HIGHER"));

    let reading = voice_tuner_reading(Some(440.0));
    assert!(reading.contains("IN TUNE"));
}

#[test]
/// Prompts for vocal input before a pitch has been detected.
/// AI_CODE
fn voice_tuner_prompts_for_a_note_without_pitch() {
    assert_eq!(
        voice_tuner_reading(None),
        "VOICE TUNER\nSing a note to begin tuning."
    );
}

/// AI_CODE
pub(crate) fn instrument_name(kind: InstrumentKind, strings: u8) -> String {
    match kind {
        InstrumentKind::Strings => format!("STRINGS ({strings}-STRING)"),
        InstrumentKind::Percussion => "PERCUSSION (MIDI)".into(),
        InstrumentKind::Voice => "VOICE".into(),
    }
}
