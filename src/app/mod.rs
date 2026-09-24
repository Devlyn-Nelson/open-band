mod state;

pub(crate) use state::*;

use super::*;

/// Build and run the Bevy application.
pub(crate) fn run() {
    // Load persisted choices before scanning devices so saved IDs can be preselected.
    let (sender, receiver) = mpsc::channel();
    let settings = load_settings();
    let startup_state = initial_app_state();
    let device_selection = scan_devices(&settings);
    let (stop_sender, stop_receiver) = mpsc::channel();
    let input_thread = spawn_instrument_thread(
        sender.clone(),
        input_config_from_settings(&settings),
        stop_receiver,
    );

    // Start the input worker before Home so Live Session works immediately after launch.
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.025, 0.035, 0.06)))
        .insert_resource(device_selection)
        .insert_resource(settings)
        .insert_resource(InstrumentStream {
            sender,
            events: Mutex::new(receiver),
            _thread: Some(input_thread),
            stop_sender: Some(stop_sender),
            started: true,
        })
        .init_resource::<MenuSelection>()
        .insert_resource(Calibration {
            selected: 0,
            level: 0.0,
            peak: 0.0,
            samples: 0,
            last_pitch_hz: None,
        })
        .init_resource::<Score>()
        .init_resource::<DebugInputData>()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Open band".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_state(startup_state)
        .add_systems(OnEnter(AppState::Home), setup_home)
        .add_systems(Update, home_input.run_if(in_state(AppState::Home)))
        .add_systems(Update, home_display.run_if(in_state(AppState::Home)))
        .add_systems(OnExit(AppState::Home), cleanup_menu)
        .add_systems(OnEnter(AppState::Setup), setup_setup)
        .add_systems(Update, setup_input.run_if(in_state(AppState::Setup)))
        .add_systems(Update, setup_display.run_if(in_state(AppState::Setup)))
        .add_systems(OnExit(AppState::Setup), cleanup_menu)
        .add_systems(OnEnter(AppState::DeviceSelection), setup_device_selection)
        .add_systems(
            Update,
            device_selection_input.run_if(in_state(AppState::DeviceSelection)),
        )
        .add_systems(
            Update,
            device_selection_display.run_if(in_state(AppState::DeviceSelection)),
        )
        .add_systems(OnExit(AppState::DeviceSelection), cleanup_device_selection)
        .add_systems(OnEnter(AppState::Calibration), setup_calibration)
        .add_systems(
            Update,
            calibration_input.run_if(in_state(AppState::Calibration)),
        )
        .add_systems(
            Update,
            calibration_display.run_if(in_state(AppState::Calibration)),
        )
        .add_systems(OnExit(AppState::Calibration), cleanup_calibration)
        .add_systems(
            OnEnter(AppState::LatencyCalibration),
            setup_latency_calibration,
        )
        .add_systems(
            Update,
            latency_calibration_input.run_if(in_state(AppState::LatencyCalibration)),
        )
        .add_systems(
            Update,
            latency_calibration_display.run_if(in_state(AppState::LatencyCalibration)),
        )
        .add_systems(
            OnExit(AppState::LatencyCalibration),
            cleanup_latency_calibration,
        )
        .add_systems(OnEnter(AppState::Songs), setup_song_menu)
        .add_systems(Update, song_menu_input.run_if(in_state(AppState::Songs)))
        .add_systems(Update, song_menu_display.run_if(in_state(AppState::Songs)))
        .add_systems(OnExit(AppState::Songs), cleanup_song_menu)
        .add_systems(OnEnter(AppState::ChartCountdown), setup_chart_countdown)
        .add_systems(
            Update,
            chart_countdown_system.run_if(in_state(AppState::ChartCountdown)),
        )
        .add_systems(OnExit(AppState::ChartCountdown), cleanup_chart_entities)
        .add_systems(OnEnter(AppState::ChartGameplay), setup_chart_gameplay)
        .add_systems(
            Update,
            (
                chart_gameplay_system,
                chart_menu_input,
                percussion_note_motion,
            )
                .run_if(in_state(AppState::ChartGameplay)),
        )
        .add_systems(OnExit(AppState::ChartGameplay), cleanup_chart_entities)
        .add_systems(OnEnter(AppState::ChartReview), setup_chart_review)
        .add_systems(
            Update,
            chart_review_input.run_if(in_state(AppState::ChartReview)),
        )
        .add_systems(
            Update,
            chart_review_display.run_if(in_state(AppState::ChartReview)),
        )
        .add_systems(OnExit(AppState::ChartReview), cleanup_chart_review)
        .add_systems(Update, instrument_navigation)
        .add_systems(OnEnter(AppState::Gameplay), setup_gameplay)
        .add_systems(
            Update,
            (
                receive_instrument_events,
                move_notes,
                hit_notes,
                gameplay_menu_input,
                debug_input,
            )
                .run_if(in_state(AppState::Gameplay)),
        )
        .add_systems(OnExit(AppState::Gameplay), cleanup_gameplay)
        .add_systems(OnEnter(AppState::Editor), setup_editor)
        .add_systems(
            Update,
            editor_browse_input
                .run_if(in_state(AppState::Editor))
                .run_if(not(resource_exists::<EditorDocument>)),
        )
        .add_systems(
            Update,
            editor_browse_display
                .run_if(in_state(AppState::Editor))
                .run_if(not(resource_exists::<EditorDocument>)),
        )
        .add_systems(
            Update,
            editor_tracks_input
                .run_if(in_state(AppState::Editor))
                .run_if(resource_exists::<EditorDocument>),
        )
        .add_systems(
            Update,
            editor_tracks_display
                .run_if(in_state(AppState::Editor))
                .run_if(resource_exists::<EditorDocument>),
        )
        .add_systems(OnExit(AppState::Editor), cleanup_editor)
        .run();
}
