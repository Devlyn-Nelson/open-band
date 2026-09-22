use bevy::prelude::*;
use serde::Deserialize;

pub(crate) const OPEN_STRINGS_CHART: &str = include_str!("../charts/open-strings.json");
pub(crate) const CHART_HIT_LINE_Y: f32 = -250.0;
pub(crate) const CHART_NOTE_SPEED: f32 = 260.0;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoteDisplayMode {
    Fret,
    Note,
    Both,
}

pub(crate) const CHART_NOTE_DISPLAY: NoteDisplayMode = NoteDisplayMode::Both;

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Chart {
    pub(crate) title: String,
    pub(crate) bpm: f32,
    pub(crate) instrument: String,
    pub(crate) notes: Vec<ChartNoteData>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ChartNoteData {
    pub(crate) start: f32,
    pub(crate) duration: f32,
    pub(crate) string: usize,
    pub(crate) fret: u8,
    pub(crate) note: String,
    pub(crate) pitch_hz: f32,
}

#[derive(Resource)]
pub(crate) struct SongMenuSelection {
    pub(crate) selected: usize,
    pub(crate) charts: Vec<Chart>,
}

#[derive(Resource)]
pub(crate) struct ChartSession {
    pub(crate) chart: Chart,
    pub(crate) started_at: f32,
}

#[derive(Resource, Default)]
pub(crate) struct ChartStats {
    pub(crate) total_notes: usize,
    pub(crate) correct_hits: usize,
    pub(crate) sustain_expected: usize,
    pub(crate) sustain_successful: usize,
    pub(crate) timing_offsets_ms: Vec<f32>,
}

#[derive(Resource, Default)]
pub(crate) struct ChartFeedback {
    pub(crate) message: String,
    pub(crate) until: f32,
}

#[derive(Component)]
pub(crate) struct SongMenuText;

#[derive(Component)]
pub(crate) struct ChartEntity;

#[derive(Component)]
pub(crate) struct ChartCountdownText;

#[derive(Component)]
pub(crate) struct ChartNoteVisual {
    pub(crate) string: usize,
    pub(crate) start: f32,
    pub(crate) duration: f32,
    pub(crate) pitch_hz: f32,
    pub(crate) matched: bool,
    pub(crate) missed: bool,
    pub(crate) sustain_observed: f32,
}

#[derive(Component)]
pub(crate) struct ChartFeedbackText;

#[derive(Component)]
pub(crate) struct ChartReviewText;
