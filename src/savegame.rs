use crate::army::{spawn_army, Army, ArmyComposition, ArmyHexMap, HexPos};
use crate::country::{Coffer, Country, DisplayName, MapColor};
use crate::hex::Hex;
use crate::map::{Owner, Province, ProvinceHexMap};
use crate::player::Player;
use crate::turns::Turn;
use crate::war::{Occupied, War, WarRelations, Wars};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;

pub struct SaveGamePlugin;

impl Plugin for SaveGamePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SaveGameEvent>()
            .add_message::<LoadGameEvent>()
            .add_systems(Update, handle_save_game)
            .add_systems(Update, handle_load_game);
    }
}

const SAVE_FILE_PATH: &str = "savegame.json";

#[derive(Event, Message)]
pub struct SaveGameEvent;

#[derive(Event, Message)]
pub struct LoadGameEvent;

// ============================================================================
// SAVE DATA STRUCTURES
// ============================================================================

#[derive(Serialize, Deserialize)]
pub struct SaveData {
    pub turn: u32,
    pub player_country_name: Option<String>,
    pub countries: Vec<CountrySaveData>,
    pub provinces: Vec<ProvinceSaveData>,
    pub armies: Vec<ArmySaveData>,
    pub wars: Vec<WarSaveData>,
}

#[derive(Serialize, Deserialize)]
pub struct CountrySaveData {
    pub name: String,
    pub coffer: f32,
}

#[derive(Serialize, Deserialize)]
pub struct ProvinceSaveData {
    pub q: i32,
    pub r: i32,
    pub owner: Option<String>,
    pub occupier: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ArmySaveData {
    pub q: i32,
    pub r: i32,
    pub owner: String,
    pub infantry: u32,
    pub cavalry: u32,
    pub artillery: u32,
}

#[derive(Serialize, Deserialize)]
pub struct WarSaveData {
    pub attacker: String,
    pub defender: String,
}

// ============================================================================
// SAVE GAME
// ============================================================================

fn handle_save_game(
    mut events: MessageReader<SaveGameEvent>,
    turn: Res<Turn>,
    player: Res<Player>,
    countries: Query<(Entity, &DisplayName, &Coffer), With<Country>>,
    provinces: Query<(Entity, &Province, Option<&Owner>, Option<&Occupied>)>,
    armies: Query<(&HexPos, &Owner, &ArmyComposition), With<Army>>,
    wars: Res<Wars>,
    war_query: Query<&War>,
) {
    for _ in events.read() {
        info!("Saving game...");
        let country_names = build_country_names(&countries);
        let save_data = build_save_data(
            &turn,
            &player,
            &countries,
            &provinces,
            &armies,
            &wars,
            &war_query,
            &country_names,
        );
        write_save_file(&save_data);
    }
}

fn build_country_names(
    countries: &Query<(Entity, &DisplayName, &Coffer), With<Country>>,
) -> HashMap<Entity, String> {
    countries
        .iter()
        .map(|(e, name, _)| (e, name.0.clone()))
        .collect()
}

fn build_save_data(
    turn: &Res<Turn>,
    player: &Res<Player>,
    countries: &Query<(Entity, &DisplayName, &Coffer), With<Country>>,
    provinces: &Query<(Entity, &Province, Option<&Owner>, Option<&Occupied>)>,
    armies: &Query<(&HexPos, &Owner, &ArmyComposition), With<Army>>,
    wars: &Res<Wars>,
    war_query: &Query<&War>,
    country_names: &HashMap<Entity, String>,
) -> SaveData {
    SaveData {
        turn: turn.current_turn(),
        player_country_name: get_player_country_name(player, countries),
        countries: collect_countries_data(countries),
        provinces: collect_provinces_data(provinces, country_names),
        armies: collect_armies_data(armies, country_names),
        wars: collect_wars_data(wars, war_query, country_names),
    }
}

fn get_player_country_name(
    player: &Res<Player>,
    countries: &Query<(Entity, &DisplayName, &Coffer), With<Country>>,
) -> Option<String> {
    player
        .country
        .and_then(|e| countries.get(e).ok().map(|(_, name, _)| name.0.clone()))
}

fn collect_countries_data(
    countries: &Query<(Entity, &DisplayName, &Coffer), With<Country>>,
) -> Vec<CountrySaveData> {
    countries
        .iter()
        .map(|(_, name, coffer)| CountrySaveData {
            name: name.0.clone(),
            coffer: coffer.get_ducats(),
        })
        .collect()
}

fn collect_provinces_data(
    provinces: &Query<(Entity, &Province, Option<&Owner>, Option<&Occupied>)>,
    country_names: &HashMap<Entity, String>,
) -> Vec<ProvinceSaveData> {
    provinces
        .iter()
        .map(|(_, prov, owner, occupied)| {
            let hex = prov.get_hex();
            ProvinceSaveData {
                q: hex.q(),
                r: hex.r(),
                owner: owner.and_then(|o| country_names.get(&o.0).cloned()),
                occupier: occupied.and_then(|o| country_names.get(&o.occupier).cloned()),
            }
        })
        .collect()
}

fn collect_armies_data(
    armies: &Query<(&HexPos, &Owner, &ArmyComposition), With<Army>>,
    country_names: &HashMap<Entity, String>,
) -> Vec<ArmySaveData> {
    armies
        .iter()
        .filter_map(|(pos, owner, comp)| {
            country_names.get(&owner.0).map(|owner_name| ArmySaveData {
                q: pos.0.q(),
                r: pos.0.r(),
                owner: owner_name.clone(),
                infantry: comp.infantry,
                cavalry: comp.cavalry,
                artillery: comp.artillery,
            })
        })
        .collect()
}

fn collect_wars_data(
    wars: &Res<Wars>,
    war_query: &Query<&War>,
    country_names: &HashMap<Entity, String>,
) -> Vec<WarSaveData> {
    wars.active_wars
        .iter()
        .filter_map(|&war_entity| {
            war_query.get(war_entity).ok().and_then(|war| {
                Some(WarSaveData {
                    attacker: country_names.get(&war.attacker)?.clone(),
                    defender: country_names.get(&war.defender)?.clone(),
                })
            })
        })
        .collect()
}

fn write_save_file(save_data: &SaveData) {
    match serde_json::to_string_pretty(save_data) {
        Ok(json) => {
            if let Err(e) = fs::write(SAVE_FILE_PATH, json) {
                error!("Failed to write save file: {}", e);
            } else {
                info!("Game saved to {}", SAVE_FILE_PATH);
            }
        }
        Err(e) => error!("Failed to serialize save data: {}", e),
    }
}

// ============================================================================
// LOAD GAME
// ============================================================================

fn handle_load_game(
    mut events: MessageReader<LoadGameEvent>,
    mut commands: Commands,
    mut turn: ResMut<Turn>,
    mut player: ResMut<Player>,
    countries: Query<(Entity, &DisplayName, &MapColor), With<Country>>,
    armies: Query<Entity, With<Army>>,
    mut army_hex_map: ResMut<ArmyHexMap>,
    mut wars: ResMut<Wars>,
    war_entities: Query<Entity, With<War>>,
    province_map: Res<ProvinceHexMap>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for _ in events.read() {
        info!("Loading game...");

        let save_data = match read_save_file() {
            Some(data) => data,
            None => continue,
        };

        let (country_lookup, country_colors) = build_country_lookups(&countries);

        restore_turn_and_player(&save_data, &mut turn, &mut player, &country_lookup);
        restore_country_coffers(&mut commands, &save_data, &country_lookup);
        restore_provinces(&mut commands, &save_data, &province_map, &country_lookup);
        restore_armies(
            &mut commands,
            &save_data,
            &armies,
            &mut army_hex_map,
            &country_lookup,
            &country_colors,
            &mut meshes,
            &mut materials,
        );
        restore_wars(
            &mut commands,
            &save_data,
            &war_entities,
            &mut wars,
            &country_lookup,
        );

        info!("Game loaded successfully!");
    }
}

fn read_save_file() -> Option<SaveData> {
    let content = fs::read_to_string(SAVE_FILE_PATH)
        .map_err(|e| error!("Failed to read save file: {}", e))
        .ok()?;
    serde_json::from_str(&content)
        .map_err(|e| error!("Failed to parse save file: {}", e))
        .ok()
}

fn build_country_lookups(
    countries: &Query<(Entity, &DisplayName, &MapColor), With<Country>>,
) -> (HashMap<String, Entity>, HashMap<String, Color>) {
    let lookup = countries
        .iter()
        .map(|(e, name, _)| (name.0.clone(), e))
        .collect();
    let colors = countries
        .iter()
        .map(|(_, name, color)| (name.0.clone(), color.0))
        .collect();
    (lookup, colors)
}

fn restore_turn_and_player(
    save_data: &SaveData,
    turn: &mut ResMut<Turn>,
    player: &mut ResMut<Player>,
    country_lookup: &HashMap<String, Entity>,
) {
    turn.set(save_data.turn);
    player.country = save_data
        .player_country_name
        .as_ref()
        .and_then(|name| country_lookup.get(name).copied());
}

fn restore_country_coffers(
    commands: &mut Commands,
    save_data: &SaveData,
    country_lookup: &HashMap<String, Entity>,
) {
    for country_save in &save_data.countries {
        if let Some(&entity) = country_lookup.get(&country_save.name) {
            commands.entity(entity).insert(Coffer(country_save.coffer));
        }
    }
}

fn restore_provinces(
    commands: &mut Commands,
    save_data: &SaveData,
    province_map: &Res<ProvinceHexMap>,
    country_lookup: &HashMap<String, Entity>,
) {
    for prov_save in &save_data.provinces {
        let hex = Hex::new(prov_save.q, prov_save.r);
        if let Some(&prov_entity) = province_map.get_entity(&hex) {
            commands
                .entity(prov_entity)
                .remove::<Owner>()
                .remove::<Occupied>();

            if let Some(owner_name) = &prov_save.owner
                && let Some(&owner_entity) = country_lookup.get(owner_name)
            {
                commands.entity(prov_entity).insert(Owner(owner_entity));
            }

            if let Some(occupier_name) = &prov_save.occupier
                && let Some(&occupier_entity) = country_lookup.get(occupier_name)
            {
                commands.entity(prov_entity).insert(Occupied {
                    occupier: occupier_entity,
                });
            }
        }
    }
}

fn restore_armies(
    commands: &mut Commands,
    save_data: &SaveData,
    armies: &Query<Entity, With<Army>>,
    army_hex_map: &mut ResMut<ArmyHexMap>,
    country_lookup: &HashMap<String, Entity>,
    country_colors: &HashMap<String, Color>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
) {
    for army_entity in armies.iter() {
        commands.entity(army_entity).despawn();
    }
    army_hex_map.tiles.clear();

    for army_save in &save_data.armies {
        spawn_army_from_save(
            commands,
            army_save,
            army_hex_map,
            country_lookup,
            country_colors,
            meshes,
            materials,
        );
    }
}

fn spawn_army_from_save(
    commands: &mut Commands,
    army_save: &ArmySaveData,
    army_hex_map: &mut ResMut<ArmyHexMap>,
    country_lookup: &HashMap<String, Entity>,
    country_colors: &HashMap<String, Color>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
) {
    if let (Some(&owner_entity), Some(&owner_color)) = (
        country_lookup.get(&army_save.owner),
        country_colors.get(&army_save.owner),
    ) {
        let hex = Hex::new(army_save.q, army_save.r);
        let composition = ArmyComposition {
            infantry: army_save.infantry,
            cavalry: army_save.cavalry,
            artillery: army_save.artillery,
        };
        let army_entity = spawn_army(
            commands,
            meshes,
            materials,
            hex,
            owner_entity,
            owner_color,
            composition,
        );
        army_hex_map.insert(HexPos(hex), army_entity);
    }
}

fn restore_wars(
    commands: &mut Commands,
    save_data: &SaveData,
    war_entities: &Query<Entity, With<War>>,
    wars: &mut ResMut<Wars>,
    country_lookup: &HashMap<String, Entity>,
) {
    for war_entity in war_entities.iter() {
        commands.entity(war_entity).despawn();
    }
    wars.active_wars.clear();

    for war_save in &save_data.wars {
        create_war_from_save(commands, war_save, wars, country_lookup);
    }
}

fn create_war_from_save(
    commands: &mut Commands,
    war_save: &WarSaveData,
    wars: &mut ResMut<Wars>,
    country_lookup: &HashMap<String, Entity>,
) {
    if let (Some(&attacker), Some(&defender)) = (
        country_lookup.get(&war_save.attacker),
        country_lookup.get(&war_save.defender),
    ) {
        let war_entity = commands.spawn(War { attacker, defender }).id();
        wars.active_wars.push(war_entity);
        commands.entity(attacker).insert(WarRelations {
            at_war_with: HashSet::from([defender]),
        });
        commands.entity(defender).insert(WarRelations {
            at_war_with: HashSet::from([attacker]),
        });
    }
}

pub fn save_exists() -> bool {
    std::path::Path::new(SAVE_FILE_PATH).exists()
}
