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
mod war;

use crate::army::ArmyPlugin;
use crate::country::CountryPlugin;
use crate::layout::LayoutPlugin;
use crate::map::MapPlugin;
use crate::player::PlayerPlugin;
use crate::turns::TurnsPlugin;
use crate::war::WarPlugin;
use bevy::log::{Level, LogPlugin};
use bevy::prelude::*;
use bevy_egui::EguiPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(LogPlugin {
            level: Level::INFO,
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_plugins(MeshPickingPlugin)
        .add_plugins((
            MapPlugin,
            CountryPlugin,
            ArmyPlugin,
            LayoutPlugin,
            TurnsPlugin,
            PlayerPlugin,
            WarPlugin,
        ))
        .add_systems(Startup, setup_camera)
        .run();
}

fn setup_camera(mut commands: Commands) {
    info!("Setting up camera");
    commands.spawn(Camera2d);
}
