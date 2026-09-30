use bevy::{
    ecs::{
        message::{Message, MessageWriter},
        system::{Res, ResMut},
    },
    time::Time,
};

use crate::{
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

#[derive(Message)]
pub enum NavigationEvent {
    Select,
    Back,
    Up,
    Down,
}

pub fn system_note_to_navigation(
    stream: Res<InstrumentStream>,
    selected_devices: Res<DeviceSelection>,
    mut nav_writer: MessageWriter<NavigationEvent>,
) {
    match stream.events.lock() {
        Ok(events) => {
            for event in events.iter() {
                if selected_devices.slots.len() > event.instrument.slot {
                    let of = selected_devices.slots[event.instrument.slot].open_frequencies();
                    let lane = pitch_to_lane(&event.instrument, &of, event.note.pitch_note());
                    let _ = match lane {
                        0 => Some(nav_writer.write(NavigationEvent::Select)),
                        1 => Some(nav_writer.write(NavigationEvent::Back)),
                        2 => Some(nav_writer.write(NavigationEvent::Up)),
                        3 => Some(nav_writer.write(NavigationEvent::Down)),
                        _ => None,
                    };
                } else {
                    bevy::log::error!(
                        "received event for device {}, but only {} devices are setup.",
                        event.instrument.slot,
                        selected_devices.slots.len()
                    );
                }
            }
        }
        Err(err) => {
            panic!("Failed to lock events: {err}")
        }
    }
}
