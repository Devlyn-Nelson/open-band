use super::*;

/// Spawn the Home screen.
pub(crate) fn setup_home(mut commands: Commands) {
    // Create the two-entry application landing screen.
    commands.spawn((Camera2d, MenuCamera));
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
pub(crate) fn setup_setup(mut commands: Commands) {
    // Create the submenu for all configuration tools.
    commands.spawn((Camera2d, MenuCamera));
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
pub(crate) fn home_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<MenuSelection>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    // Home routes to Live Session, Songs, Editor, or Set Up.
    if keyboard.just_pressed(KeyCode::ArrowUp) {
        menu.home_selected = menu.home_selected.checked_sub(1).unwrap_or(3);
    }
    if keyboard.just_pressed(KeyCode::ArrowDown) {
        menu.home_selected = (menu.home_selected + 1) % 4;
    }
    for (index, key) in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ]
    .into_iter()
    .enumerate()
    {
        if keyboard.just_pressed(key) {
            menu.home_selected = index;
        }
    }
    if keyboard.just_pressed(KeyCode::Enter) {
        next_state.set(match menu.home_selected {
            0 => AppState::Gameplay,
            1 => AppState::Songs,
            2 => AppState::Editor,
            _ => AppState::Setup,
        });
    }
}

/// Render the selected Home destination.
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
        {} [4] SET UP\n\n\
        Up/Down: navigate     Enter: open",
        marker(0),
        marker(1),
        marker(2),
        marker(3),
    ));
}

/// Navigate Set Up and open a setup tool or Home.
pub(crate) fn setup_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<MenuSelection>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    // Route each setup option to its tool or back to Home.
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Home);
        return;
    }
    if keyboard.just_pressed(KeyCode::ArrowUp) {
        menu.setup_selected = menu.setup_selected.checked_sub(1).unwrap_or(3);
    }
    if keyboard.just_pressed(KeyCode::ArrowDown) {
        menu.setup_selected = (menu.setup_selected + 1) % 4;
    }
    for (index, key) in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ]
    .into_iter()
    .enumerate()
    {
        if keyboard.just_pressed(key) {
            menu.setup_selected = index;
        }
    }
    if keyboard.just_pressed(KeyCode::Enter) {
        next_state.set(match menu.setup_selected {
            0 => AppState::DeviceSelection,
            1 => AppState::Calibration,
            2 => AppState::LatencyCalibration,
            _ => AppState::Home,
        });
    }
}

/// Render the selected setup tool.
pub(crate) fn setup_display(menu: Res<MenuSelection>, mut text: Query<&mut Text, With<MenuText>>) {
    // Render the four setup destinations and their selection marker.
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
    *text = Text::new(format!(
        "OPEN BAND  //  SET UP\n\n\
        {} [1] INPUT SETUP\n\
        {} [2] TUNER\n\
        {} [3] LATENCY CALIBRATION\n\
        {} [4] BACK\n\n\
        Up/Down: navigate     Enter: open     Esc: home",
        marker(0),
        marker(1),
        marker(2),
        marker(3),
    ));
}

/// Despawn the shared menu camera and text entities.
pub(crate) fn cleanup_menu(
    mut commands: Commands,
    entities: Query<Entity, Or<(With<MenuText>, With<MenuCamera>)>>,
) {
    // Both menu screens share the same text and camera marker types.
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}
