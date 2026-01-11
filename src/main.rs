mod army;
mod buildings;
mod consts;
mod country;
mod egui_common;
mod hex;
mod layout;
mod map;
mod player;
mod turns;

use crate::army::{ArmyHexMap, MoveArmyEvent, SelectedArmy};
use crate::country::SelectedCountry;
use crate::map::{MapMode, ProvinceHexMap, SelectedProvince};
use crate::turns::{GameState, Turn};
use bevy::log::{Level, LogPlugin};
use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(LogPlugin {
            level: Level::INFO,
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_plugins(MeshPickingPlugin)
        .insert_resource(ProvinceHexMap::default())
        .insert_resource(ArmyHexMap::default())
        .insert_resource(SelectedProvince::default())
        .insert_resource(SelectedCountry::default())
        .insert_resource(SelectedArmy::default())
        .insert_resource(MapMode::default())
        .insert_resource(Turn::default())
        .add_message::<MoveArmyEvent>()
        .add_systems(Startup, setup_camera)
        .add_systems(Startup, country::setup_countries)
        .add_systems(Startup, map::generate_map)
        .add_systems(
            Startup,
            country::assign_province_ownership
                .after(map::generate_map)
                .after(country::setup_countries),
        )
        .add_systems(
            Startup,
            army::spawn_initial_armies.after(country::assign_province_ownership),
        )
        .add_systems(Update, layout::camera_keyboard_system)
        .add_systems(Update, layout::camera_zoom_system)
        .add_systems(
            Update,
            (
                map::render_province_terrain.run_if(resource_equals(MapMode::Terrain)),
                map::render_province_political.run_if(resource_equals(MapMode::Political)),
            ),
        )
        .add_systems(EguiPrimaryContextPass, map::display_province_panel)
        .add_systems(EguiPrimaryContextPass, map::display_map_modes_panel)
        .add_systems(EguiPrimaryContextPass, country::display_country_panel)
        .init_state::<GameState>()
        .add_systems(OnEnter(GameState::Processing), turns::handle_new_turn)
        .add_systems(EguiPrimaryContextPass, turns::display_turn_button)
        .add_systems(Update, army::army_movement_system)
        .add_systems(Update, army::handle_army_interaction_changed)
        .add_systems(Update, army::handle_army_composition_changed)
        .run();
}

fn setup_camera(mut commands: Commands) {
    info!("Setting up camera");
    commands.spawn(Camera2d);
}
