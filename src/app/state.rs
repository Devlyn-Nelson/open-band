use bevy::prelude::*;

#[derive(States, Default, Clone, Eq, PartialEq, Debug, Hash)]
/// Top-level screens in the application flow.
///
/// AI_CODE
pub(crate) enum AppState {
    #[default]
    Home,
    Settings,
    DeviceSelection,
    Calibration,
    LatencyCalibration,
    // \/ AI_CODE \/
    Recorder,
    // /\ AI_CODE /\
    Songs,
    ChartCountdown,
    ChartGameplay,
    ChartReview,
    Gameplay,
    Editor,
}
