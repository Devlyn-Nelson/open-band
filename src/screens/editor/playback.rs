use super::*;

/// Session-local preview clock; score history deliberately excludes playback state. AI_CODE
#[derive(Default)]
pub(super) struct EditorPlayback {
    pub(super) playing: bool,
    pub(super) tick: u32,
    seconds: f32,
    cursor: u32,
    next_beat: u32,
    origin: Option<u32>,
}

impl EditorDocument {
    /// Toggle preview at the shared cursor, retaining the reached position when paused. AI_CODE
    pub(super) fn toggle_preview(&mut self) {
        self.playback.playing = !self.playback.playing;
        if self.playback.playing {
            self.playback.origin = Some(self.current_tick);
            self.playback.cursor = u32::MAX;
        } else {
            self.current_tick = self.playback.tick;
        }
    }

    /// Stop preview and return to the position where this playback pass began. AI_CODE
    pub(super) fn stop_preview(&mut self) {
        if let Some(origin) = self.playback.origin.take() {
            self.current_tick = origin;
        }
        self.playback.playing = false;
        self.playback.tick = self.current_tick;
    }

    /// Keep a paging tab viewport aligned without interpreting automatic scroll as a seek. AI_CODE
    pub(super) fn follow_preview(&mut self, span: u32) {
        if self.playback.playing
            && self.playback.cursor == self.current_tick
            && self.playback.tick >= self.current_tick.saturating_add(span)
        {
            self.current_tick = self.playback.tick;
            self.playback.cursor = self.current_tick;
        }
    }

    /// Advance on the tempo map, coalescing missed clicks rather than replaying a burst. AI_CODE
    fn advance_preview(&mut self, delta: f32) -> Option<bool> {
        if !self.playback.playing { return None; }
        if self.playback.cursor != self.current_tick {
            self.playback.seconds = self.chart.tick_to_seconds(self.current_tick);
            self.playback.tick = self.current_tick;
            self.playback.next_beat = next_beat(&self.chart, self.current_tick).0;
            self.playback.cursor = self.current_tick;
        } else {
            self.playback.seconds += delta;
            self.playback.tick = tick_at_seconds(&self.chart, self.playback.seconds);
        }
        let end = self.chart.total_ticks();
        if end > 0 && self.playback.tick >= end {
            self.playback.tick = end;
            self.playback.playing = false;
            self.current_tick = end;
            return None;
        }
        let mut last_accent = None;
        while self.playback.next_beat <= self.playback.tick {
            let beat = self.playback.next_beat;
            last_accent = Some(next_beat(&self.chart, beat).1);
            let following = next_beat(&self.chart, beat.saturating_add(1)).0;
            if following <= beat { break; }
            self.playback.next_beat = following;
        }
        last_accent
    }
}

/// Invert the shared tempo-map conversion without assuming a fixed BPM. AI_CODE
fn tick_at_seconds(chart: &Chart, seconds: f32) -> u32 {
    let mut lower = 0u32;
    let mut upper = u32::MAX;
    while lower < upper {
        let middle = lower + (upper - lower).div_ceil(2);
        if chart.tick_to_seconds(middle) <= seconds {
            lower = middle;
        } else {
            upper = middle - 1;
        }
    }
    lower
}

/// Find the next notated beat, resetting the accent grid on meter changes. AI_CODE
fn next_beat(chart: &Chart, tick: u32) -> (u32, bool) {
    let meter = chart.time_signature_map.iter().filter(|meter| meter.start <= tick)
        .max_by_key(|meter| meter.start);
    let (start, numerator, denominator) = meter.map_or((0, 4, 4), |meter| {
        (meter.start, u32::from(meter.numerator.max(1)), u32::from(meter.denominator.max(1)))
    });
    let step = (chart.resolution.max(1).saturating_mul(4) / denominator).max(1);
    let index = tick.saturating_sub(start).div_ceil(step);
    let candidate = start.saturating_add(index.saturating_mul(step));
    if let Some(change) = chart.time_signature_map.iter()
        .filter(|meter| meter.start > tick && meter.start <= candidate)
        .min_by_key(|meter| meter.start)
    {
        return (change.start, true);
    }
    (candidate, index % numerator == 0)
}

/// Generate a short enveloped PCM click as a standard WAV audio asset. AI_CODE
fn click_audio(accent: bool) -> bevy::audio::AudioSource {
    let sample_rate = 44100u32;
    let count = 1764u32;
    let mut output = std::io::Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(&mut output, hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    }).expect("click WAV header is valid");
    let frequency = if accent { 1760.0 } else { 1100.0 };
    for sample in 0..count {
        let time = sample as f32 / sample_rate as f32;
        let envelope = (1.0 - sample as f32 / count as f32).powi(3);
        let value = (time * frequency * std::f32::consts::TAU).sin() * envelope * 0.3;
        writer.write_sample((value * i16::MAX as f32) as i16).expect("in-memory click sample writes");
    }
    writer.finalize().expect("in-memory click WAV finalizes");
    bevy::audio::AudioSource { bytes: output.into_inner().into() }
}

/// Advance preview from the shared editor cursor and emit meter-aware clicks. AI_CODE
pub(crate) fn editor_playback_system(
    mut commands: Commands,
    time: Res<Time<Real>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    document: Option<ResMut<EditorDocument>>,
    mut audio: ResMut<Assets<bevy::audio::AudioSource>>,
    mut clicks: Local<Option<(Handle<bevy::audio::AudioSource>, Handle<bevy::audio::AudioSource>)>>,
) {
    let Some(mut document) = document else { return };
    let blocked = document.text_entry.is_some() || document.inspector.is_some()
        || document.tab_fret_string.is_some() || document.tab_percussion_lane.is_some();
    if blocked {
        if document.playback.playing {
            document.current_tick = document.playback.tick;
        }
        document.playback.playing = false;
        return;
    }
    let control = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    if keyboard.just_pressed(KeyCode::Space) && !control {
        document.toggle_preview();
    }
    if !document.playback.playing { return; }
    let handles = clicks.get_or_insert_with(|| (audio.add(click_audio(false)), audio.add(click_audio(true))));
    if let Some(accent) = document.advance_preview(time.delta_secs()) {
        commands.spawn((
            bevy::audio::AudioPlayer::new(if accent { handles.1.clone() } else { handles.0.clone() }),
            bevy::audio::PlaybackSettings::DESPAWN,
        ));
    }
}

/// Tempo changes alter playback speed while meter changes reset beat spacing and accents. AI_CODE
#[test]
fn preview_timing_respects_tempo_and_meter() {
    let mut document = new_chart_document();
    document.chart.tempo_map.push(TempoChange { start: 960, bpm: 60.0 });
    document.chart.time_signature_map.push(TimeSignatureChange { start: 1920, numerator: 3, denominator: 8 });
    assert_eq!(tick_at_seconds(&document.chart, 0.5), 960);
    assert_eq!(tick_at_seconds(&document.chart, 1.5), 1920);
    assert_eq!(next_beat(&document.chart, 0), (0, true));
    assert_eq!(next_beat(&document.chart, 1), (960, false));
    assert_eq!(next_beat(&document.chart, 1920), (1920, true));
    assert_eq!(next_beat(&document.chart, 1921), (2400, false));
    assert_eq!(next_beat(&document.chart, 2881), (3360, true));
}

/// Seeking, paging, pausing, and stopping keep audio position and the tab cursor consistent. AI_CODE
#[test]
fn preview_transport_follows_cursor_without_paging_restarts() {
    let mut document = new_chart_document();
    document.dirty = false;
    document.current_tick = 480;
    document.toggle_preview();
    assert_eq!(document.advance_preview(0.0), None);
    assert_eq!(document.advance_preview(0.25), Some(false));
    assert_eq!(document.playback.tick, 960);
    document.follow_preview(480);
    assert_eq!(document.current_tick, 960);
    assert_eq!(document.advance_preview(0.25), None);
    assert_eq!(document.playback.tick, 1440);
    document.current_tick = 3840;
    assert_eq!(document.advance_preview(0.25), Some(true));
    assert_eq!(document.playback.tick, 3840);
    document.current_tick = 0;
    document.follow_preview(480);
    assert_eq!(document.current_tick, 0);
    assert_eq!(document.advance_preview(0.0), Some(true));
    assert_eq!(document.playback.tick, 0);
    document.current_tick = 3840;
    document.advance_preview(0.0);
    document.toggle_preview();
    assert!(!document.playback.playing);
    assert_eq!(document.current_tick, 3840);
    document.toggle_preview();
    document.advance_preview(0.0);
    document.advance_preview(0.5);
    document.stop_preview();
    assert_eq!(document.current_tick, 3840);
    assert!(!document.playback.playing);
    assert!(!document.dirty);
    assert!(document.history.undo.is_empty());
}

/// Authored chart endings stop preview without clicking beyond the final duration. AI_CODE
#[test]
fn preview_stops_at_chart_end() {
    let mut document = new_chart_document();
    document.chart.tracks.push(new_track(InstrumentKind::Voice));
    sheet_edit_spelled_pitch_at_tick(&mut document, 0, 60, None, &SheetTools::default());
    document.dirty = false;
    document.toggle_preview();
    assert_eq!(document.advance_preview(0.0), Some(true));
    assert_eq!(document.advance_preview(0.5), None);
    assert!(!document.playback.playing);
    assert_eq!(document.current_tick, 960);
    assert!(!document.dirty);
}

/// Both click accents decode through the actual enabled Bevy audio format pipeline. AI_CODE
#[test]
fn preview_click_assets_decode_non_silent_audio() {
    use bevy::audio::Decodable;
    for accent in [false, true] {
        let samples = click_audio(accent).decoder().collect::<Vec<_>>();
        assert_eq!(samples.len(), 1764);
        assert!(samples.iter().any(|sample| sample.abs() > 0.1));
    }
}

/// Space dispatch starts audio preview, and modal input pauses it instead of toggling playback. AI_CODE
#[test]
fn preview_keyboard_dispatch_and_modal_safety() {
    let mut document = new_chart_document();
    document.current_tick = 3840;
    let mut app = App::new();
    app.init_resource::<Time<Real>>()
        .init_resource::<Assets<bevy::audio::AudioSource>>()
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(document)
        .add_systems(Update, editor_playback_system);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Space);
    app.update();
    assert!(app.world().resource::<EditorDocument>().playback.playing);
    assert_eq!(app.world_mut().query::<&bevy::audio::AudioPlayer>().iter(app.world()).count(), 1);
    app.insert_resource(ButtonInput::<KeyCode>::default());
    app.world_mut().resource_mut::<EditorDocument>().text_entry = Some(EditorTextEntry {
        target: EditorTextTarget::ChartTitle,
        buffer: "Draft".into(),
    });
    app.update();
    assert!(!app.world().resource::<EditorDocument>().playback.playing);
    app.world_mut().resource_mut::<EditorDocument>().text_entry = None;
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Space);
    app.update();
    assert!(app.world().resource::<EditorDocument>().playback.playing);
}