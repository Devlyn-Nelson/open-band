use bevy::prelude::*;

#[derive(States, Default, Clone, Eq, PartialEq, Debug, Hash)]
/// Top-level screens in the application flow.
pub(crate) enum AppState {
    #[default]
    Home,
    Setup,
    DeviceSelection,
    Calibration,
    LatencyCalibration,
    Songs,
    ChartCountdown,
    ChartGameplay,
    ChartReview,
    Gameplay,
    Editor,
}
