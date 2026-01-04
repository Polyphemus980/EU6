mod consts;
mod hex;
mod layout;
mod map;
mod player;

use crate::map::{HexMap, SelectedProvince};
use bevy::log::{Level, LogPlugin};
use bevy::prelude::KeyCode::Select;
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(LogPlugin {
            level: Level::INFO,
            ..default()
        }))
        .insert_resource(HexMap::default())
        .insert_resource(SelectedProvince::default())
        .add_systems(Startup, setup_camera)
        .add_systems(Startup, map::generate_map)
        .add_systems(Update, layout::camera_keyboard_system)
        .add_systems(Update, layout::camera_zoom_system)
        .add_systems(Update, layout::click_system)
        .add_systems(Update, map::province_visual_system)
        .run();
}

fn setup_camera(mut commands: Commands) {
    info!("Setting up camera");
    commands.spawn(Camera2d);
}
