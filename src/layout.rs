use crate::map;
use crate::map::MapMode;
use bevy::camera::{Camera2d, Projection};
use bevy::input::mouse::MouseWheel;
use bevy::input::ButtonInput;
use bevy::log::info;
use bevy::math::Vec3;
use bevy::prelude::{
    KeyCode, MessageReader, Plugin, Query, Res, ResMut, Single, Time, Transform, With,
};

pub struct LayoutPlugin;

impl Plugin for LayoutPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        use bevy::prelude::*;
        app.add_systems(Update, camera_keyboard_system)
            .add_systems(Update, camera_zoom_system);
    }
}

/// System to handle keyboard input for moving the camera.
pub(crate) fn camera_keyboard_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut Transform, With<Camera2d>>,
    mut map_mode: ResMut<MapMode>,
    time: Res<Time>,
) {
    let mut movement = Vec3::ZERO;
    let speed = 500.0 * time.delta_secs();

    if keyboard.pressed(KeyCode::KeyW) {
        movement.y += speed;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        movement.x -= speed;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        movement.x += speed;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        movement.y -= speed;
    }

    if keyboard.just_pressed(KeyCode::KeyM) {
        info!("Switching map mode");
        map::switch_map_mode(&mut map_mode);
    }

    for mut transform in &mut query {
        transform.translation += movement;
    }
}

/// System to handle mouse wheel events and zoom the camera in/out.
pub(crate) fn camera_zoom_system(
    mut scroll_events: MessageReader<MouseWheel>,
    mut projection: Single<&mut Projection, With<Camera2d>>,
) {
    for event in scroll_events.read() {
        // Since we are using 2d camera, the projection is Orthographic.
        let Projection::Orthographic(perspective) = projection.as_mut() else {
            return;
        };

        // The event's y value contains 1.0 for scroll up and -1.0 for scroll down, so we subtract it
        // to make scrolling up decrease the scale (zoom in) and scrolling down increase the scale (zoom out).
        perspective.scale -= event.y * 0.1;
    }
}
