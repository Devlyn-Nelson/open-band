use inlet::InletEvent;

use crate::*;

#[derive(Resource, Default)]
/// Selection state for the Home and Set Up menus.
///
/// AI_CODE
pub(crate) struct MenuSelection {
    pub(crate) home_selected: usize,
    pub(crate) setup_selected: usize,
}

#[derive(Component)]
/// Text node used by the Home and Set Up menus.
///
/// AI_CODE
pub(crate) struct MenuText;

#[derive(Component)]
/// Camera owned by a menu screen.
///
/// AI_CODE
pub(crate) struct MenuCamera;

/// Spawn the Home screen.
///
/// AI_CODE
pub(crate) fn setup_home(mut commands: Commands) {
    // Create the two-entry application landing screen.
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(30.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(60.0),
            left: Val::Px(90.0),
            ..default()
        },
        MenuText,
    ));
}

/// Spawn the Set Up screen.
///
/// AI_CODE
pub(crate) fn setup_settings(mut commands: Commands) {
    // Create the submenu for all configuration tools.
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(30.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(60.0),
            left: Val::Px(90.0),
            ..default()
        },
        MenuText,
    ));
}

/// Navigate Home and open Live Session or Set Up.
pub(crate) fn system_home_input(
    mut nav_events: MessageReader<InletEvent<NavigationMessage>>,
    mut menu: ResMut<MenuSelection>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    // Home routes to Live Session, Songs, Editor, or Set Up.
    for event in nav_events.read() {
        match event.kind {
            NavigationMessage::Up => {
                menu.home_selected = menu.home_selected.checked_sub(1).unwrap_or(3);
            }
            NavigationMessage::Down => {
                menu.home_selected = (menu.home_selected + 1) % 4;
            }
            NavigationMessage::Select => {
                next_state.set(match menu.home_selected {
                    0 => AppState::Gameplay,
                    1 => AppState::Songs,
                    2 => AppState::Editor,
                    _ => AppState::Settings,
                });
            }
            NavigationMessage::Slot(num) => {
                menu.home_selected = num.clamp(1, 4).checked_sub(1).unwrap_or(3);
            }
            _ => {}
        }
    }
}

/// Render the selected Home destination.
///
/// AI_CODE
pub(crate) fn home_display(menu: Res<MenuSelection>, mut text: Query<&mut Text, With<MenuText>>) {
    // Keep the highlighted Home option synchronized with keyboard navigation.
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let marker = |index: usize| {
        if menu.home_selected == index {
            ">"
        } else {
            " "
        }
    };
    *text = Text::new(format!(
        "OPEN BAND  //  HOME\n\n\
        {} [1] LIVE SESSION\n\
        {} [2] SONGS\n\
        {} [3] EDITOR\n\
        {} [4] SETTINGS\n\n\
        Up/Down: navigate     Enter: open",
        marker(0),
        marker(1),
        marker(2),
        marker(3),
    ));
}

/// Navigate Set Up and open a setup tool or Home.
pub(crate) fn system_settings_input(
    mut nav_events: MessageReader<InletEvent<NavigationMessage>>,
    mut menu: ResMut<MenuSelection>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    // Route each setup option to its tool or back to Home.
    // \/ AI_CODE \/
    for event in nav_events.read() {
        match event.kind {
            NavigationMessage::Up => {
                menu.setup_selected = menu.setup_selected.checked_sub(1).unwrap_or(4);
            }
            NavigationMessage::Down => {
                menu.setup_selected = (menu.setup_selected + 1) % 5;
            }
            NavigationMessage::Select => {
                next_state.set(match menu.setup_selected {
                    0 => AppState::DeviceSelection,
                    1 => AppState::Calibration,
                    2 => AppState::LatencyCalibration,
                    3 => AppState::Recorder,
                    _ => AppState::Home,
                });
            }
            NavigationMessage::Back => {
                next_state.set(AppState::Home);
                return;
            }
            NavigationMessage::Slot(num) => {
                menu.setup_selected = num.clamp(1, 5).checked_sub(1).unwrap_or(4);
            }
            _ => {}
        }
    }
    // /\ AI_CODE /\
}

/// Render the selected setup tool.
///
/// AI_CODE
pub(crate) fn system_settings_display(
    menu: Res<MenuSelection>,
    mut text: Query<&mut Text, With<MenuText>>,
) {
    // Render the setup destinations and their selection marker.
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let marker = |index: usize| {
        if menu.setup_selected == index {
            ">"
        } else {
            " "
        }
    };
    // \/ AI_CODE \/
    *text = Text::new(format!(
        "OPEN BAND  //  SET UP\n\n\
        {} [1] INPUT SETUP\n\
        {} [2] TUNER\n\
        {} [3] LATENCY CALIBRATION\n\
        {} [4] RECORDING\n\
        {} [5] BACK\n\n\
        Up/Down: navigate     Enter: open     Esc: home",
        marker(0),
        marker(1),
        marker(2),
        marker(3),
        marker(4),
    ));
    // /\ AI_CODE /\
}

/// Despawn the shared menu camera and text entities.
///
/// AI_CODE
pub(crate) fn cleanup_menu(mut commands: Commands, entities: Query<Entity, With<MenuText>>) {
    // Both menu screens share the same text and camera marker types.
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}
