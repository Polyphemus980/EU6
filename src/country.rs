use crate::map::{Owner, Province};
use bevy::prelude::*;

/// Marker component for country entities. No data as I am trying to do ECS :P.
#[derive(Component)]
pub(crate) struct Country {}

/// Components representing the name and map color of a faction. They are not attached to the
/// country entity, as things like rebels may have names/colors but not be countries.
#[derive(Component)]
pub struct Name(pub String);
#[derive(Component)]
pub struct MapColor(pub Color);

/// Spawns a country entity with the given name and map color.
pub fn spawn_country(commands: &mut Commands, name: &str, color: Color) -> Entity {
    commands
        .spawn((Country {}, Name(name.to_string()), MapColor(color)))
        .id()
}

pub(crate) fn setup_countries(mut commands: Commands) {
    // Create some sample countries with distinct colors
    spawn_country(&mut commands, "Francia", Color::srgb(0.2, 0.3, 0.8)); // Blue
    spawn_country(&mut commands, "Hispania", Color::srgb(0.9, 0.8, 0.1)); // Yellow
    spawn_country(&mut commands, "Germania", Color::srgb(0.3, 0.3, 0.3)); // Gray
    spawn_country(&mut commands, "Italia", Color::srgb(0.0, 0.6, 0.3)); // Green
    spawn_country(&mut commands, "Britannia", Color::srgb(0.8, 0.1, 0.2)); // Red
}

/// System to assign province ownership to countries based on province location.
/// This runs after both countries and provinces have been spawned.
pub(crate) fn assign_province_ownership(
    mut commands: Commands,
    provinces: Query<(Entity, &Province)>,
    countries: Query<(Entity, &Name), With<Country>>,
) {
    // Create a list of countries for easy access
    let country_list: Vec<(Entity, &str)> = countries
        .iter()
        .map(|(entity, name)| (entity, name.0.as_str()))
        .collect();

    if country_list.is_empty() {
        return;
    }

    // Assign provinces to countries based on hex position
    for (province_entity, province) in provinces.iter() {
        // Skip sea and wasteland provinces - they remain unowned
        if !province.is_ownable() {
            continue;
        }

        let hex = province.get_hex();

        // Assign ownership based on hex coordinates
        // This creates distinct regions for each country
        let owner = if hex.q() > 2 && hex.r() > -3 {
            // East: Francia (Blue)
            country_list
                .iter()
                .find(|(_, name)| *name == "Francia")
                .map(|(e, _)| e)
        } else if hex.q() < -2 && hex.r() < 3 {
            // West: Britannia (Red)
            country_list
                .iter()
                .find(|(_, name)| *name == "Britannia")
                .map(|(e, _)| e)
        } else if hex.r() > 2 && hex.q() > -3 {
            // South: Hispania (Yellow)
            country_list
                .iter()
                .find(|(_, name)| *name == "Hispania")
                .map(|(e, _)| e)
        } else if hex.r() < -2 && hex.q() < 3 {
            // North: Germania (Gray)
            country_list
                .iter()
                .find(|(_, name)| *name == "Germania")
                .map(|(e, _)| e)
        } else {
            // Center: Italia (Green)
            country_list
                .iter()
                .find(|(_, name)| *name == "Italia")
                .map(|(e, _)| e)
        };

        if let Some(owner_entity) = owner {
            commands
                .entity(province_entity)
                .insert(Owner(*owner_entity));
        }
    }
}
