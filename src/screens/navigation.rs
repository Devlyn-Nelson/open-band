use std::time::Duration;

use bevy::{
    app::{App, Plugin, Startup, Update},
    ecs::{
        message::{MessageReader, MessageWriter, Messages},
        schedule::IntoScheduleConfigs,
        system::{Commands, Res, ResMut},
    },
    input::{gamepad::GamepadButton, keyboard::KeyCode},
    state::{
        condition::in_state,
        state::{EnterSchedules, State, StateTransition, StateTransitionEvent},
    },
    time::Time,
};
use inlet::{InletEvent, InputBindings, InputManagementPlugin, button::ButtonEventBinding};

use crate::{
    app::AppState,
    audio::pitch_to_lane,
    input::InstrumentStream,
    screens::{DeviceSelection, LatencyCalibration},
};

/// AI_CODE
pub(crate) fn system_record_latency_tap(
    latency: &mut Option<ResMut<LatencyCalibration>>,
    time: &Time,
) {
    let Some(latency) = latency.as_mut() else {
        return;
    };
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

#[derive(Hash, PartialEq, Eq, Clone)]
pub enum NavigationMessage {
    Select,
    Back,
    Up,
    Down,
    Right,
    Left,
    Slot(usize),
}

impl NavigationMessage {
    pub fn default_binding() -> InputBindings<NavigationMessage> {
        InputBindings::new()
            .with_action_binding(
                NavigationMessage::Back,
                (
                    vec![KeyCode::Escape.into(), GamepadButton::West.into()],
                    ButtonEventBinding::WhenPressed,
                )
                    .into(),
            )
            .with_action_binding(
                NavigationMessage::Select,
                (
                    vec![KeyCode::Enter.into(), GamepadButton::South.into()],
                    ButtonEventBinding::WhenPressed,
                )
                    .into(),
            )
            .with_action_binding(
                NavigationMessage::Up,
                (
                    vec![KeyCode::ArrowUp.into(), GamepadButton::DPadUp.into()],
                    ButtonEventBinding::WhenPressed,
                )
                    .into(),
            )
            .with_action_binding(
                NavigationMessage::Down,
                (
                    vec![KeyCode::ArrowDown.into(), GamepadButton::DPadDown.into()],
                    ButtonEventBinding::WhenPressed,
                )
                    .into(),
            )
            .with_action_binding(
                NavigationMessage::Right,
                (
                    vec![KeyCode::ArrowRight.into(), GamepadButton::DPadRight.into()],
                    ButtonEventBinding::WhenPressed,
                )
                    .into(),
            )
            .with_action_binding(
                NavigationMessage::Left,
                (
                    vec![KeyCode::ArrowLeft.into(), GamepadButton::DPadLeft.into()],
                    ButtonEventBinding::WhenPressed,
                )
                    .into(),
            )
            .with_action_binding(
                NavigationMessage::Slot(1),
                (
                    vec![KeyCode::Digit1.into()],
                    ButtonEventBinding::WhenPressed,
                )
                    .into(),
            )
            .with_action_binding(
                NavigationMessage::Slot(2),
                (
                    vec![KeyCode::Digit2.into()],
                    ButtonEventBinding::WhenPressed,
                )
                    .into(),
            )
            .with_action_binding(
                NavigationMessage::Slot(3),
                (
                    vec![KeyCode::Digit3.into()],
                    ButtonEventBinding::WhenPressed,
                )
                    .into(),
            )
            .with_action_binding(
                NavigationMessage::Slot(4),
                (
                    vec![KeyCode::Digit4.into()],
                    ButtonEventBinding::WhenPressed,
                )
                    .into(),
            )
            .with_action_binding(
                NavigationMessage::Slot(5),
                (
                    vec![KeyCode::Digit5.into()],
                    ButtonEventBinding::WhenPressed,
                )
                    .into(),
            )
            .with_action_binding(
                NavigationMessage::Slot(6),
                (
                    vec![KeyCode::Digit6.into()],
                    ButtonEventBinding::WhenPressed,
                )
                    .into(),
            )
            .with_action_binding(
                NavigationMessage::Slot(7),
                (
                    vec![KeyCode::Digit7.into()],
                    ButtonEventBinding::WhenPressed,
                )
                    .into(),
            )
            .with_action_binding(
                NavigationMessage::Slot(8),
                (
                    vec![KeyCode::Digit8.into()],
                    ButtonEventBinding::WhenPressed,
                )
                    .into(),
            )
            .with_action_binding(
                NavigationMessage::Slot(9),
                (
                    vec![KeyCode::Digit9.into()],
                    ButtonEventBinding::WhenPressed,
                )
                    .into(),
            )
    }
}

pub struct NavigationPlugin;

impl Plugin for NavigationPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(InputManagementPlugin::<NavigationMessage>::default())
            .add_systems(Startup, system_navigation_setup)
            .add_systems(Update, system_note_to_navigation)
            .add_systems(
                StateTransition,
                system_clear_navigation_on_state_change
                    .after(EnterSchedules::<AppState>::default()),
            );
    }
}

impl Default for NavigationPlugin {
    fn default() -> Self {
        Self
    }
}

pub fn system_navigation_setup(mut cmds: Commands) {
    cmds.spawn(NavigationMessage::default_binding());
}

/// Drop buffered navigation messages whenever the app state changes.
///
/// A screen's input system only runs while its state is active, so its message
/// cursor is stale on entry and would otherwise replay the message that caused
/// the transition.
///
/// AI_CODE
pub fn system_clear_navigation_on_state_change(
    mut transitions: MessageReader<StateTransitionEvent<AppState>>,
    mut nav_messages: ResMut<Messages<InletEvent<NavigationMessage>>>,
) {
    if transitions
        .read()
        .any(|transition| transition.entered != transition.exited)
    {
        nav_messages.clear();
    }
}

pub fn system_note_to_navigation(
    stream: Option<Res<InstrumentStream>>,
    state: Res<State<AppState>>,
    selected_devices: Res<DeviceSelection>,
    mut nav_writer: MessageWriter<InletEvent<NavigationMessage>>,
) {
    let Some(stream) = stream else {
        return;
    };
    if !matches!(
        state.get(),
        AppState::ChartReview | AppState::DeviceSelection | AppState::Home | AppState::Setup
    ) {
        return;
    }
    // TODO currently this emits navigation messages with a player index tied to the instrument's slot.
    // `try_iter` drains only what is queued; `iter` would block until every sender is dropped.
    let events = match stream.events.lock() {
        Ok(events) => events.try_iter().collect::<Vec<_>>(),
        Err(err) => {
            panic!("Failed to lock events: {err}")
        }
    };
    for event in events.iter() {
        if selected_devices.slots.len() > event.instrument.slot {
            let of = selected_devices.slots[event.instrument.slot].open_frequencies();
            let lane = pitch_to_lane(&event.instrument, &of, event.note.pitch_note());
            let thing = match lane {
                0 => Some(NavigationMessage::Select),
                1 => Some(NavigationMessage::Back),
                2 => Some(NavigationMessage::Up),
                3 => Some(NavigationMessage::Down),
                _ => None,
            };
            if let Some(thing) = thing {
                bevy::log::info!("sending instrument navigation");
                let _ = nav_writer.write(InletEvent {
                    player: event.instrument.slot,
                    kind: thing,
                    data: inlet::TriggerValue::Pressed(true),
                    duration: Duration::from_secs_f32(event.duration_secs),
                });
            }
        } else {
            bevy::log::error!(
                "received event for device {}, but only {} devices are setup.",
                event.instrument.slot,
                selected_devices.slots.len()
            );
        }
    }
}
