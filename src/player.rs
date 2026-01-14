use crate::country::{Country, DisplayName};
use bevy::prelude::*;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Player::default())
            .add_systems(PostStartup, setup_player);
    }
}

#[derive(Resource, Default)]
pub(crate) struct Player {
    pub(crate) country: Option<Entity>,
}

fn setup_player(
    mut player: ResMut<Player>,
    countries: Query<(Entity, &DisplayName), With<Country>>,
) {
    let target_country = countries
        .iter()
        .find(|(_, name)| name.0 == "Francia")
        .or_else(|| countries.iter().next());

    if let Some((entity, name)) = target_country {
        info!("Player assigned to country: {} ({:?})", name.0, entity);
        player.country = Some(entity);
    } else {
        warn!("No countries found to assign to player");
    }
}
