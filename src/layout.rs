use crate::hex::Hex;
use crate::map::{HexMap, InteractionState, MapMode, SelectedProvince};
use crate::{consts, map};
use bevy::camera::{Camera, Camera2d, Projection};
use bevy::input::mouse::MouseWheel;
use bevy::input::ButtonInput;
use bevy::log::{error, info};
use bevy::math::Vec3;
use bevy::prelude::{
    Commands, Component, GlobalTransform, KeyCode, MessageReader, MouseButton, Query, Res, ResMut,
    Single, Time, Transform, Window, With,
};
use bevy::window::PrimaryWindow;

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

/// System to handle mouse clicks and select provinces on the hex map.
pub(crate) fn click_system(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    hex_map: Res<HexMap>,
    mut selected: ResMut<SelectedProvince>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let cursor_pos = match window.cursor_position() {
        Some(pos) => pos,
        None => {
            info!("Cursor is outside window");
            return;
        }
    };

    // Since we have only one camera, we can safely use single().
    let (camera, camera_transform) = match camera.single() {
        Ok(cam) => cam,
        Err(err) => {
            error!("Error getting camera: {err}");
            return;
        }
    };

    // Convert the cursor position from viewport space to world space.
    let world_pos = match camera.viewport_to_world_2d(camera_transform, cursor_pos) {
        Ok(pos) => pos,
        Err(err) => {
            error!("Error converting viewport to world position: {err}");
            return;
        }
    };

    // Use the hex conversion function to get the clicked hex.
    let clicked_hex = Hex::world_to_axial(world_pos, consts::HEX_SIZE);

    // Since the hex map contains a mapping of hexes to provinces (entities), we can check if
    // the clicked hex corresponds to any province.
    if let Some(entity) = hex_map.get_entity(&clicked_hex) {
        info!("Clicked hex: {:?}, entity: {:?}", clicked_hex, entity);

        // If there was a previously selected province, deselect it.
        if let Some(prev_entity) = selected.get() {
            commands.entity(prev_entity).insert(InteractionState::None);

            // Clicking the same province again deselects it.
            if prev_entity == *entity {
                selected.clear();
                return;
            }
        }

        // Add the Selected component to the newly selected province and update the resource.
        commands.entity(*entity).insert(InteractionState::Selected);
        selected.set(*entity);
    } else {
        info!("Didn't find any hex");
    }
}
