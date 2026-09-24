use crate::*;

pub(crate) fn instrument_navigation(
    state: Res<State<AppState>>,
    mut input_stream: ResMut<InstrumentStream>,
    mut settings: ResMut<PersistentSettings>,
    mut menu: ResMut<MenuSelection>,
    mut next_state: ResMut<NextState<AppState>>,
    mut selection: ResMut<DeviceSelection>,
    mut songs: Option<ResMut<SongMenuSelection>>,
    mut latency: Option<ResMut<LatencyCalibration>>,
    time: Res<Time>,
) {
    if matches!(
        state.get(),
        AppState::Calibration | AppState::Gameplay | AppState::ChartGameplay
    ) {
        return;
    }
    let Ok(events) = input_stream.events.lock() else {
        return;
    };
    let commands = events
        .try_iter()
        .filter(|event| {
            event.instrument.kind == InstrumentKind::Strings && event.phase == NotePhase::Started
        })
        .map(|event| event.lane)
        .collect::<Vec<_>>();
    drop(events);

    for lane in commands {
        match lane {
            4 => instrument_navigation_command(
                KeyCode::Enter,
                state.get(),
                &mut menu,
                &mut next_state,
                &mut selection,
                &mut input_stream,
                &mut settings,
                &mut songs,
                &mut latency,
                &time,
            ),
            3 => instrument_navigation_command(
                KeyCode::Escape,
                state.get(),
                &mut menu,
                &mut next_state,
                &mut selection,
                &mut input_stream,
                &mut settings,
                &mut songs,
                &mut latency,
                &time,
            ),
            2 => instrument_navigation_command(
                KeyCode::ArrowUp,
                state.get(),
                &mut menu,
                &mut next_state,
                &mut selection,
                &mut input_stream,
                &mut settings,
                &mut songs,
                &mut latency,
                &time,
            ),
            1 => instrument_navigation_command(
                KeyCode::ArrowDown,
                state.get(),
                &mut menu,
                &mut next_state,
                &mut selection,
                &mut input_stream,
                &mut settings,
                &mut songs,
                &mut latency,
                &time,
            ),
            _ => {}
        }
    }
}

pub(crate) fn instrument_navigation_command(
    key: KeyCode,
    state: &AppState,
    menu: &mut MenuSelection,
    next_state: &mut NextState<AppState>,
    selection: &mut DeviceSelection,
    stream: &mut InstrumentStream,
    settings: &mut PersistentSettings,
    songs: &mut Option<ResMut<SongMenuSelection>>,
    latency: &mut Option<ResMut<LatencyCalibration>>,
    time: &Time,
) {
    match state {
        AppState::Home => match key {
            KeyCode::ArrowUp => menu.home_selected = menu.home_selected.checked_sub(1).unwrap_or(2),
            KeyCode::ArrowDown => menu.home_selected = (menu.home_selected + 1) % 3,
            KeyCode::Enter => next_state.set(match menu.home_selected {
                0 => AppState::Gameplay,
                1 => AppState::Songs,
                _ => AppState::Setup,
            }),
            _ => {}
        },
        AppState::Setup => match key {
            KeyCode::ArrowUp => {
                menu.setup_selected = menu.setup_selected.checked_sub(1).unwrap_or(3)
            }
            KeyCode::ArrowDown => menu.setup_selected = (menu.setup_selected + 1) % 4,
            KeyCode::Enter => next_state.set(match menu.setup_selected {
                0 => AppState::DeviceSelection,
                1 => AppState::Calibration,
                2 => AppState::LatencyCalibration,
                _ => AppState::Home,
            }),
            KeyCode::Escape => next_state.set(AppState::Home),
            _ => {}
        },
        AppState::DeviceSelection => match key {
            KeyCode::ArrowUp => selection.focus = selection.focus.checked_sub(1).unwrap_or(3),
            KeyCode::ArrowDown => selection.focus = (selection.focus + 1) % 4,
            KeyCode::Escape => next_state.set(AppState::Setup),
            KeyCode::Enter => commit_device_selection(selection, stream, settings, next_state),
            _ => {}
        },
        AppState::Songs => {
            let Some(songs) = songs.as_mut() else { return };
            match key {
                KeyCode::ArrowUp => {
                    songs.selected = songs
                        .selected
                        .checked_sub(1)
                        .unwrap_or(songs.charts.len().saturating_sub(1))
                }
                KeyCode::ArrowDown if !songs.charts.is_empty() => {
                    songs.selected = (songs.selected + 1) % songs.charts.len()
                }
                KeyCode::Escape => next_state.set(AppState::Home),
                KeyCode::Enter => next_state.set(AppState::ChartCountdown),
                _ => {}
            }
        }
        AppState::LatencyCalibration => match key {
            KeyCode::ArrowUp | KeyCode::ArrowDown => {
                record_latency_tap(latency, time);
            }
            KeyCode::Escape => next_state.set(AppState::Setup),
            KeyCode::Enter => {
                if let Some(latency) = latency.as_ref() {
                    if latency.attempts > 0 {
                        settings.latency_ms = latency.best_ms;
                        save_settings(settings);
                        next_state.set(AppState::Setup);
                    }
                }
            }
            _ => {}
        },
        AppState::ChartReview => match key {
            KeyCode::Enter => next_state.set(AppState::Songs),
            KeyCode::Escape => next_state.set(AppState::Home),
            _ => {}
        },
        _ => {}
    }
}

pub(crate) fn record_latency_tap(latency: &mut Option<ResMut<LatencyCalibration>>, time: &Time) {
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
