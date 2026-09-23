use super::*;
use bevy::input::keyboard::KeyboardInput;
use std::path::{Path, PathBuf};

#[derive(Component)]
pub(crate) struct EditorCamera;

#[derive(Component)]
pub(crate) struct EditorText;

pub(crate) struct EditorChartEntry {
    pub(crate) title: String,
    pub(crate) path: PathBuf,
}

#[derive(Resource, Default)]
/// The chart-picker shown before a document is open (see `EditorDocument`).
pub(crate) struct EditorBrowser {
    pub(crate) entries: Vec<EditorChartEntry>,
    pub(crate) selected: usize,
}

#[derive(Resource)]
/// Tuning/kit libraries loaded once per editor session, shared by the track list's
/// tuning/kit cycling.
pub(crate) struct EditorLibraries {
    pub(crate) tunings: Vec<NamedTuning>,
    pub(crate) kits: Vec<NamedKit>,
}

#[derive(Resource)]
/// The chart currently open for editing. Its presence (vs. `EditorBrowser` alone) is what
/// switches the Editor state between the chart picker and the track list.
pub(crate) struct EditorDocument {
    pub(crate) chart: Chart,
    pub(crate) path: Option<PathBuf>,
    pub(crate) dirty: bool,
    pub(crate) track_focus: usize,
    pub(crate) renaming: Option<String>,
    pub(crate) status: String,
}

fn list_editable_charts() -> Vec<EditorChartEntry> {
    let mut paths = std::fs::read_dir("charts")
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| {
            let contents = std::fs::read_to_string(&path).ok()?;
            let chart = serde_json::from_str::<Chart>(&contents).ok()?;
            Some(EditorChartEntry {
                title: chart.title,
                path,
            })
        })
        .collect()
}

fn new_chart_document() -> EditorDocument {
    EditorDocument {
        chart: Chart {
            version: 1,
            title: "New Chart".into(),
            resolution: 960,
            tempo_map: vec![TempoChange {
                start: 0,
                bpm: 120.0,
            }],
            time_signature_map: vec![TimeSignatureChange {
                start: 0,
                numerator: 4,
                denominator: 4,
            }],
            tempo_text: Vec::new(),
            expressions: Vec::new(),
            structure: Vec::new(),
            tracks: Vec::new(),
        },
        path: None,
        dirty: true,
        track_focus: 0,
        renaming: None,
        status: "New chart".into(),
    }
}

fn load_chart_document(path: &Path) -> Result<EditorDocument, String> {
    let contents = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let chart = serde_json::from_str::<Chart>(&contents).map_err(|error| error.to_string())?;
    Ok(EditorDocument {
        chart,
        path: Some(path.to_path_buf()),
        dirty: false,
        track_focus: 0,
        renaming: None,
        status: "Loaded".into(),
    })
}

pub(crate) fn new_track(kind: InstrumentKind) -> ChartTrack {
    ChartTrack {
        name: "New Track".into(),
        kind,
        tuning: matches!(kind, InstrumentKind::Strings).then(Tuning::default),
        kit: matches!(kind, InstrumentKind::Percussion).then(Kit::default),
        vocal_range: None,
        clef: None,
        key_signature: None,
        capo: None,
        notes: Vec::new(),
        ties: Vec::new(),
        slurs: Vec::new(),
        phrases: Vec::new(),
        star_power_phrases: Vec::new(),
        lyric_verses: Vec::new(),
    }
}

/// A filesystem-safe slug derived from a chart title, used as the save filename.
pub(crate) fn slugify(title: &str) -> String {
    let slug = title
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let slug = slug
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "untitled".into()
    } else {
        slug
    }
}

pub(crate) fn setup_editor(mut commands: Commands) {
    commands.insert_resource(EditorBrowser {
        entries: list_editable_charts(),
        selected: 0,
    });
    commands.insert_resource(EditorLibraries {
        tunings: load_tuning_library(),
        kits: load_kit_library(),
    });
    commands.spawn((Camera2d, EditorCamera));
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(24.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(40.0),
            left: Val::Px(70.0),
            ..default()
        },
        EditorText,
    ));
}

pub(crate) fn cleanup_editor(
    mut commands: Commands,
    entities: Query<Entity, Or<(With<EditorCamera>, With<EditorText>)>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<EditorDocument>();
    commands.remove_resource::<EditorBrowser>();
    commands.remove_resource::<EditorLibraries>();
}

pub(crate) fn editor_browse_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut browser: ResMut<EditorBrowser>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Home);
        return;
    }
    if !browser.entries.is_empty() {
        if keyboard.just_pressed(KeyCode::ArrowUp) {
            browser.selected = browser
                .selected
                .checked_sub(1)
                .unwrap_or(browser.entries.len() - 1);
        }
        if keyboard.just_pressed(KeyCode::ArrowDown) {
            browser.selected = (browser.selected + 1) % browser.entries.len();
        }
    }
    if keyboard.just_pressed(KeyCode::KeyN) {
        commands.insert_resource(new_chart_document());
        return;
    }
    if keyboard.just_pressed(KeyCode::Enter)
        && let Some(entry) = browser.entries.get(browser.selected)
    {
        match load_chart_document(&entry.path) {
            Ok(document) => commands.insert_resource(document),
            Err(error) => eprintln!("Could not load chart {}: {error}", entry.path.display()),
        }
    }
}

pub(crate) fn editor_browse_display(
    browser: Res<EditorBrowser>,
    mut text: Query<&mut Text, With<EditorText>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let entries = if browser.entries.is_empty() {
        "  (no charts found in charts/)".to_string()
    } else {
        browser
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                format!(
                    "{} {}",
                    if index == browser.selected { ">" } else { " " },
                    entry.title
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    *text = Text::new(format!(
        "OPEN BAND  //  CHART EDITOR\n\n{entries}\n\n\
        Up/Down: select     Enter: edit chart     N: new chart     Esc: home",
    ));
}

pub(crate) fn cycle_track_tuning(track: &mut ChartTrack, library: &[NamedTuning], delta: i32) {
    if library.is_empty() || track.kind != InstrumentKind::Strings {
        return;
    }
    let current = track
        .tuning
        .as_ref()
        .and_then(|tuning| {
            library
                .iter()
                .position(|named| named.strings == tuning.strings)
        })
        .unwrap_or(0);
    let next = (current as i32 + delta).rem_euclid(library.len() as i32) as usize;
    track.tuning = Some(library[next].tuning());
}

pub(crate) fn cycle_track_kit(track: &mut ChartTrack, library: &[NamedKit], delta: i32) {
    if library.is_empty() || track.kind != InstrumentKind::Percussion {
        return;
    }
    let current = track
        .kit
        .as_ref()
        .and_then(|kit| {
            library
                .iter()
                .position(|named| named.lanes == kit.lanes && named.pieces == kit.pieces)
        })
        .unwrap_or(0);
    let next = (current as i32 + delta).rem_euclid(library.len() as i32) as usize;
    track.kit = Some(library[next].kit());
}

pub(crate) fn editor_tracks_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut keyboard_events: MessageReader<KeyboardInput>,
    mut document: ResMut<EditorDocument>,
    libraries: Res<EditorLibraries>,
    mut commands: Commands,
) {
    // Renaming captures all keyboard text input until confirmed or canceled.
    if let Some(buffer) = document.renaming.as_mut() {
        for event in keyboard_events.read() {
            if event.state != bevy::input::ButtonState::Pressed {
                continue;
            }
            if let Some(text) = &event.text {
                buffer.extend(text.chars().filter(|character| !character.is_control()));
            }
        }
        if keyboard.just_pressed(KeyCode::Backspace) {
            buffer.pop();
        }
        if keyboard.just_pressed(KeyCode::Enter) {
            let name = document.renaming.take().unwrap_or_default();
            let focus = document.track_focus;
            if let Some(track) = document.chart.tracks.get_mut(focus)
                && !name.trim().is_empty()
            {
                track.name = name.trim().to_string();
                document.dirty = true;
            }
        }
        if keyboard.just_pressed(KeyCode::Escape) {
            document.renaming = None;
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::Escape) {
        commands.remove_resource::<EditorDocument>();
        return;
    }

    let track_count = document.chart.tracks.len();
    if track_count > 0 {
        if keyboard.just_pressed(KeyCode::ArrowUp) {
            document.track_focus = document
                .track_focus
                .checked_sub(1)
                .unwrap_or(track_count - 1);
        }
        if keyboard.just_pressed(KeyCode::ArrowDown) {
            document.track_focus = (document.track_focus + 1) % track_count;
        }
    }
    if keyboard.just_pressed(KeyCode::KeyN) {
        document
            .chart
            .tracks
            .push(new_track(InstrumentKind::Strings));
        document.track_focus = document.chart.tracks.len() - 1;
        document.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyX) && track_count > 0 {
        let focus = document.track_focus;
        document.chart.tracks.remove(focus);
        document.track_focus = document
            .track_focus
            .min(document.chart.tracks.len().saturating_sub(1));
        document.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyK) {
        let focus = document.track_focus;
        if let Some(track) = document.chart.tracks.get_mut(focus) {
            let next_kind = match track.kind {
                InstrumentKind::Strings => InstrumentKind::Percussion,
                InstrumentKind::Percussion => InstrumentKind::Voice,
                InstrumentKind::Voice => InstrumentKind::Strings,
            };
            let name = track.name.clone();
            *track = new_track(next_kind);
            track.name = name;
            document.dirty = true;
        }
    }
    if keyboard.just_pressed(KeyCode::BracketLeft) {
        let focus = document.track_focus;
        if let Some(track) = document.chart.tracks.get_mut(focus) {
            cycle_track_tuning(track, &libraries.tunings, -1);
            cycle_track_kit(track, &libraries.kits, -1);
            document.dirty = true;
        }
    }
    if keyboard.just_pressed(KeyCode::BracketRight) {
        let focus = document.track_focus;
        if let Some(track) = document.chart.tracks.get_mut(focus) {
            cycle_track_tuning(track, &libraries.tunings, 1);
            cycle_track_kit(track, &libraries.kits, 1);
            document.dirty = true;
        }
    }
    if keyboard.just_pressed(KeyCode::Comma) && document.track_focus > 0 {
        let focus = document.track_focus;
        document.chart.tracks.swap(focus, focus - 1);
        document.track_focus -= 1;
        document.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::Period) && document.track_focus + 1 < track_count {
        let focus = document.track_focus;
        document.chart.tracks.swap(focus, focus + 1);
        document.track_focus += 1;
        document.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyR) && track_count > 0 {
        let current_name = document.chart.tracks[document.track_focus].name.clone();
        document.renaming = Some(current_name);
    }
    if keyboard.just_pressed(KeyCode::KeyS) {
        let warnings = document.chart.validate();
        let title = document.chart.title.clone();
        let path = document
            .path
            .clone()
            .unwrap_or_else(|| PathBuf::from(format!("charts/{}.json", slugify(&title))));
        match serde_json::to_string_pretty(&document.chart)
            .map_err(|error| error.to_string())
            .and_then(|contents| std::fs::write(&path, contents).map_err(|error| error.to_string()))
        {
            Ok(()) => {
                document.path = Some(path);
                document.dirty = false;
                document.status = if warnings.is_empty() {
                    "Saved".into()
                } else {
                    format!(
                        "Saved with {} warning(s): {}",
                        warnings.len(),
                        warnings.join("; ")
                    )
                };
            }
            Err(error) => document.status = format!("Save failed: {error}"),
        }
    }
}

pub(crate) fn editor_tracks_display(
    document: Res<EditorDocument>,
    mut text: Query<&mut Text, With<EditorText>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let dirty = if document.dirty { "*" } else { "" };
    let track_lines = if document.chart.tracks.is_empty() {
        "  (no tracks yet; press N to add one)".to_string()
    } else {
        document
            .chart
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| {
                let marker = if document.track_focus == index {
                    ">"
                } else {
                    " "
                };
                let detail = match track.kind {
                    InstrumentKind::Strings => track
                        .tuning
                        .as_ref()
                        .map_or("no tuning".into(), |tuning| tuning.strings.join("-")),
                    InstrumentKind::Percussion => {
                        track.kit.as_ref().map_or("no kit".into(), |kit| {
                            format!("{} pieces", kit.pieces.len())
                        })
                    }
                    InstrumentKind::Voice => "voice".into(),
                };
                format!("{marker} {} — {:?} ({detail})", track.name, track.kind)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let rename_line = document
        .renaming
        .as_ref()
        .map_or_else(String::new, |buffer| {
            format!("\n\nRENAME: {buffer}_\nEnter: confirm  Esc: cancel")
        });
    *text = Text::new(format!(
        "OPEN BAND  //  CHART EDITOR  //  {}{dirty}\n\n{track_lines}{rename_line}\n\n\
        Up/Down: focus track     N: add     X: remove     K: cycle kind\n\
        [ / ]: cycle tuning/kit     , / .: reorder     R: rename     S: save\n\
        Esc: back to chart list\n\n{}",
        document.chart.title, document.status,
    ));
}
