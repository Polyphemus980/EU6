mod consts;
mod country;
mod hex;
mod layout;
mod map;
mod player;

use crate::map::{HexMap, MapMode, SelectedProvince};
use bevy::log::{Level, LogPlugin};
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(LogPlugin {
            level: Level::INFO,
            ..default()
        }))
        .insert_resource(HexMap::default())
        .insert_resource(SelectedProvince::default())
        .insert_resource(MapMode::default())
        .add_systems(Startup, setup_camera)
        .add_systems(Startup, country::setup_countries)
        .add_systems(Startup, map::generate_map)
        .add_systems(
            Startup,
            country::assign_province_ownership
                .after(map::generate_map)
                .after(country::setup_countries),
        )
        .add_systems(Update, layout::camera_keyboard_system)
        .add_systems(Update, layout::camera_zoom_system)
        .add_systems(Update, layout::click_system)
        .add_systems(
            Update,
            (
                map::render_province_terrain.run_if(resource_equals(MapMode::Terrain)),
                map::render_province_political.run_if(resource_equals(MapMode::Political)),
            ),
        )
        .run();
}

fn setup_camera(mut commands: Commands) {
    info!("Setting up camera");
    commands.spawn(Camera2d);
}
