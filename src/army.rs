use crate::consts;
use crate::country::{Country, MapColor};
use crate::hex::Hex;
use crate::map::{InteractionState, Owner, Province, ProvinceHexMap};
use crate::player::Player;
use bevy::ecs::error::Result;
use bevy::mesh::Mesh;
use bevy::prelude::*;
use bevy::sprite::Sprite;
use bevy_egui::egui::{Align2, Color32, RichText};
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use pathfinding::prelude::bfs;
use rand::Rng;
use std::collections::{HashMap, VecDeque};

pub struct ArmyPlugin;

impl Plugin for ArmyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ArmyHexMap::default())
            .insert_resource(SelectedArmy::default())
            .add_message::<MoveArmyEvent>()
            .add_systems(
                Startup,
                spawn_initial_armies.after(crate::country::assign_province_ownership),
            )
            .add_systems(Update, army_movement_system)
            .add_systems(Update, draw_path_gizmos) // Add this for visualization
            .add_systems(Update, handle_army_interaction_changed)
            .add_systems(Update, handle_army_composition_changed)
            .add_systems(EguiPrimaryContextPass, display_army_panel)
            .add_systems(EguiPrimaryContextPass, display_battle_panel)
            .add_systems(Update, resolve_battles);
    }
}

/// Resource mapping hex positions to army entities. One army per hex - stacking = auto-merge.
#[derive(Resource, Default)]
pub(crate) struct ArmyHexMap {
    pub(crate) tiles: HashMap<HexPos, Entity>,
}

impl ArmyHexMap {
    pub(crate) fn insert(&mut self, pos: HexPos, army: Entity) {
        self.tiles.insert(pos, army);
    }

    pub(crate) fn remove(&mut self, pos: &HexPos) {
        self.tiles.remove(pos);
    }

    pub(crate) fn get(&self, pos: &HexPos) -> Option<&Entity> {
        self.tiles.get(pos)
    }
}

#[derive(Resource, Default)]
pub(crate) struct SelectedArmy {
    pub(crate) selected: Option<Entity>,
}

impl SelectedArmy {
    pub(crate) fn clear(&mut self) {
        self.selected = None;
    }

    pub(crate) fn set(&mut self, army: Entity) {
        self.selected = Some(army);
    }

    pub(crate) fn get(&self) -> Option<Entity> {
        self.selected
    }
}

#[derive(Component)]
pub(crate) struct ActivePath {
    pub(crate) path: VecDeque<Hex>,
}

#[derive(Component)]
pub(crate) struct Army {}
#[derive(Component, Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub(crate) struct HexPos(pub(crate) Hex);

impl HexPos {
    pub(crate) fn new(hex: Hex) -> Self {
        Self(hex)
    }
}
#[derive(Component, Copy, Clone)]
pub(crate) struct ArmyComposition {
    pub(crate) infantry: u32,
    pub(crate) cavalry: u32,
    pub(crate) artillery: u32,
}

#[derive(PartialEq, Copy, Clone)]
pub(crate) enum UnitType {
    Infantry,
    Cavalry,
    Artillery,
}

impl UnitType {
    pub(crate) fn cost(&self) -> f32 {
        match self {
            UnitType::Infantry => 10.0,
            UnitType::Cavalry => 25.0,
            UnitType::Artillery => 30.0,
        }
    }

    pub(crate) fn name(&self) -> &'static str {
        match self {
            UnitType::Infantry => "Infantry",
            UnitType::Cavalry => "Cavalry",
            UnitType::Artillery => "Artillery",
        }
    }

    pub(crate) fn all() -> [UnitType; 3] {
        [UnitType::Infantry, UnitType::Cavalry, UnitType::Artillery]
    }
}

pub(crate) const REGIMENT_SIZE: u32 = 1000;

impl ArmyComposition {
    pub(crate) fn total_size(&self) -> u32 {
        self.infantry + self.cavalry + self.artillery
    }

    pub(crate) fn add(&mut self, other: &ArmyComposition) {
        self.infantry += other.infantry;
        self.cavalry += other.cavalry;
        self.artillery += other.artillery;
    }

    pub(crate) fn add_unit(&mut self, unit: UnitType) {
        match unit {
            UnitType::Infantry => self.infantry += REGIMENT_SIZE,
            UnitType::Cavalry => self.cavalry += REGIMENT_SIZE,
            UnitType::Artillery => self.artillery += REGIMENT_SIZE,
        }
    }
}

pub(crate) const MIN_DAMAGE: u32 = 5;

#[derive(Component)]
pub(crate) struct ArmyLabel(pub(crate) String);

#[derive(Component)]
pub(crate) struct SelectedRing {}

#[derive(Component)]
pub(crate) struct InBattle {
    pub(crate) battle_entity: Entity,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum BattleSide {
    Attacker,
    Defender,
}

#[derive(Component)]
pub(crate) struct Battle {
    /// All armies on the attacking side
    pub(crate) attackers: Vec<Entity>,
    /// All armies on the defending side
    pub(crate) defenders: Vec<Entity>,
    /// Country that initiated the attack
    pub(crate) attacker_country: Entity,
    /// Country that is defending
    pub(crate) defender_country: Entity,
    pub(crate) location: Hex,
    pub(crate) round: u32,
    pub(crate) last_damage_attacker: u32,
    pub(crate) last_damage_defender: u32,
}

#[derive(Bundle)]
pub(crate) struct ArmyBundle {
    pub(crate) marker: Army,
    pub(crate) pos: HexPos,
    pub(crate) owner: Owner,
    pub(crate) composition: ArmyComposition,
    pub(crate) interaction_state: InteractionState,
    pub(crate) transform: Transform,
    pub(crate) visibility: Visibility,
    pub(crate) sprite: Sprite,
    pub(crate) pickable: Pickable,
}

#[derive(Message)]
pub(crate) struct MoveArmyEvent {
    pub(crate) army: Entity,
    pub(crate) to: HexPos,
}

impl MoveArmyEvent {
    pub(crate) fn new(army: Entity, to: HexPos) -> Self {
        Self { army, to }
    }
}

pub(crate) fn army_movement_system(
    mut commands: Commands,
    mut move_events: MessageReader<MoveArmyEvent>,
    army_hex_map: ResMut<ArmyHexMap>,
    province_map: Res<ProvinceHexMap>,
    provinces: Query<&Province>,
) -> Result {
    for event in move_events.read() {
        let from_pos = army_hex_map
            .tiles
            .iter()
            .find_map(|(pos, &army)| if army == event.army { Some(*pos) } else { None });

        let from_pos = match from_pos {
            Some(pos) => pos,
            None => {
                warn!(
                    "Army movement event for unknown army entity: {:?}",
                    event.army
                );
                continue;
            }
        };

        if from_pos == event.to {
            continue;
        }

        // Calculate path
        let path = bfs(
            &from_pos.0,
            |p| {
                let neighbors: Vec<Hex> = p
                    .neighbors()
                    .into_iter()
                    .filter(|n| {
                        if let Some(&entity) = province_map.get_entity(n)
                            && let Ok(province) = provinces.get(entity)
                        {
                            return province.is_passable();
                        }
                        false
                    })
                    .collect();
                neighbors
            },
            |p| *p == event.to.0,
        );

        if let Some(path) = path {
            let mut deck = VecDeque::from(path);
            deck.pop_front(); // Remove current position
            if !deck.is_empty() {
                commands
                    .entity(event.army)
                    .insert(ActivePath { path: deck.clone() });
                info!(
                    "Army {:?} started moving to {:?}, path length: {}",
                    event.army,
                    event.to,
                    deck.len()
                );
            }
        } else {
            warn!(
                "No path found for army {:?} from {:?} to {:?}",
                event.army, from_pos, event.to
            );
        }
    }
    Ok(())
}

pub(crate) fn move_active_armies(
    mut commands: Commands,
    mut army_hex_map: ResMut<ArmyHexMap>,
    mut armies_query: Query<
        (
            Entity,
            &mut Transform,
            &Owner,
            &mut ArmyComposition,
            &mut HexPos,
            Option<&mut ActivePath>,
            Option<&InBattle>,
        ),
        With<Army>,
    >,
    mut selected_army: ResMut<SelectedArmy>,
    war_relations: Query<&crate::war::WarRelations>,
    mut battles: Query<&mut Battle>,
    _province_map: Res<ProvinceHexMap>,
) {
    let movers: Vec<Entity> = armies_query
        .iter()
        .filter_map(|(e, _, _, _, _, path, _)| if path.is_some() { Some(e) } else { None })
        .collect();

    for entity in movers {
        let (next_hex, old_pos) = {
            if let Ok((_, _, _, _, pos, Some(active_path), _)) = armies_query.get(entity) {
                if let Some(h) = active_path.path.front() {
                    (*h, *pos)
                } else {
                    commands.entity(entity).remove::<ActivePath>();
                    continue;
                }
            } else {
                continue;
            }
        };

        let next_pos = HexPos(next_hex);

        // Find battle at location properly
        let battle_at_location: Option<Entity> = {
            let mut found = None;
            for (_, _, _, _, _, _, maybe_in_battle) in armies_query.iter() {
                if let Some(in_battle) = maybe_in_battle
                    && let Ok(battle) = battles.get(in_battle.battle_entity)
                    && battle.location == next_hex
                {
                    found = Some(in_battle.battle_entity);
                    break;
                }
            }
            found
        };

        if let Some(battle_entity) = battle_at_location {
            // There's an ongoing battle - try to join it
            if let Ok((_, _, owner, _, _, _, _)) = armies_query.get(entity)
                && let Ok(mut battle) = battles.get_mut(battle_entity)
            {
                let owner_entity = owner.0;

                // Determine which side to join
                let side = if owner_entity == battle.attacker_country {
                    Some(BattleSide::Attacker)
                } else if owner_entity == battle.defender_country {
                    Some(BattleSide::Defender)
                } else if crate::war::are_at_war(
                    owner_entity,
                    battle.defender_country,
                    &war_relations,
                ) {
                    // Allied with attacker (at war with defender)
                    Some(BattleSide::Attacker)
                } else if crate::war::are_at_war(
                    owner_entity,
                    battle.attacker_country,
                    &war_relations,
                ) {
                    // Allied with defender (at war with attacker)
                    Some(BattleSide::Defender)
                } else {
                    None
                };

                if let Some(side) = side {
                    info!(
                        "Army {:?} joins battle at {:?} on {:?} side",
                        entity, next_hex, side
                    );

                    // Add to battle
                    match side {
                        BattleSide::Attacker => battle.attackers.push(entity),
                        BattleSide::Defender => battle.defenders.push(entity),
                    }

                    // Mark army as in battle
                    commands.entity(entity).remove::<ActivePath>();
                    commands.entity(entity).insert(InBattle { battle_entity });

                    // Move army to battle location
                    army_hex_map.remove(&old_pos);
                    // Don't insert into hex map - battle location is shared
                    if let Ok((_, mut transform, _, _, mut pos, _, _)) =
                        armies_query.get_mut(entity)
                    {
                        *pos = next_pos;
                        transform.translation =
                            next_hex.axial_to_world(consts::HEX_SIZE).extend(5.0);
                    }

                    continue;
                }
            }
        }

        if let Some(&occupant_entity) = army_hex_map.get(&next_pos) {
            // Check if occupant entity still exists (might have been destroyed in battle)
            if armies_query.get(occupant_entity).is_err() {
                // Occupant was destroyed, clean up hex map
                army_hex_map.remove(&next_pos);
                // Continue to normal movement below
            } else if let Ok(
                [
                    (e1, _, owner1, comp1, _, _, _),
                    (e2, _, owner2, mut comp2, _, _, _),
                ],
            ) = armies_query.get_many_mut([entity, occupant_entity])
            {
                if owner1.0 == owner2.0 {
                    info!("Merging army {:?} into {:?}", e1, e2);
                    comp2.add(&comp1);

                    army_hex_map.remove(&old_pos);
                    commands.entity(e1).despawn();

                    // If the merged army was selected, clear selection or select the target
                    if selected_army.get() == Some(e1) {
                        selected_army.set(e2);
                        commands.entity(e2).insert(InteractionState::Selected);
                    }

                    continue;
                } else {
                    // Check if countries are at war before starting combat
                    let are_at_war = crate::war::are_at_war(owner1.0, owner2.0, &war_relations);

                    if !are_at_war {
                        // Not at war - cannot attack, stop movement
                        info!(
                            "Cannot attack: {:?} and {:?} are not at war",
                            owner1.0, owner2.0
                        );
                        commands.entity(e1).remove::<ActivePath>();
                        continue;
                    }

                    // COMBAT START
                    info!(
                        "Battle started between {:?} (attacker) and {:?} (defender) at {:?}",
                        e1, e2, next_hex
                    );

                    // Stop movement
                    commands.entity(e1).remove::<ActivePath>();

                    // Create Battle Entity with multi-army support
                    let battle_id = commands
                        .spawn(Battle {
                            attackers: vec![e1],
                            defenders: vec![e2],
                            attacker_country: owner1.0,
                            defender_country: owner2.0,
                            location: next_hex,
                            round: 0,
                            last_damage_attacker: 0,
                            last_damage_defender: 0,
                        })
                        .id();

                    // Mark armies with their side
                    commands.entity(e1).insert(InBattle {
                        battle_entity: battle_id,
                    });
                    commands.entity(e2).insert(InBattle {
                        battle_entity: battle_id,
                    });

                    continue;
                }
            } else {
                // Could not get both armies - one might have been destroyed, skip
                continue;
            }
        }

        if let Ok((_, mut transform, _, _, mut pos, Some(mut active_path), _)) =
            armies_query.get_mut(entity)
        {
            active_path.path.pop_front();

            army_hex_map.remove(&old_pos);
            army_hex_map.insert(next_pos, entity);
            *pos = next_pos;
            transform.translation = next_hex.axial_to_world(consts::HEX_SIZE).extend(5.0);

            if active_path.path.is_empty() {
                commands.entity(entity).remove::<ActivePath>();
                info!("Army {:?} arrived at destination {:?}", entity, next_pos);
            }
        }
    }
}

fn draw_path_gizmos(
    mut gizmos: Gizmos,
    selected_army: Res<SelectedArmy>,
    armies: Query<&ActivePath>,
    armies_pos: Query<&HexPos>,
) {
    if let Some(entity) = selected_army.get()
        && let Ok(path) = armies.get(entity)
    {
        let mut points = Vec::new();
        // Start from current position
        if let Ok(start_pos) = armies_pos.get(entity) {
            points.push(start_pos.0.axial_to_world(consts::HEX_SIZE));
        }

        for hex in &path.path {
            points.push(hex.axial_to_world(consts::HEX_SIZE));
        }

        if points.len() >= 2 {
            gizmos.linestrip_2d(points, Color::srgb(1.0, 1.0, 0.0));
        }

        // Draw waypoints
        for hex in &path.path {
            gizmos.circle_2d(
                hex.axial_to_world(consts::HEX_SIZE),
                5.0,
                Color::srgb(1.0, 1.0, 0.0),
            );
        }
    }
}

pub(crate) fn spawn_army(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    position: Hex,
    owner: Entity,
    owner_color: Color,
    composition: ArmyComposition,
) -> Entity {
    let ring_mesh = meshes.add(Circle::new(25.0));
    let ring_material = materials.add(Color::srgba(1.0, 1.0, 0.0, 0.4));

    commands
        .spawn((ArmyBundle {
            marker: Army {},
            pos: HexPos(position),
            owner: Owner(owner),
            composition,
            interaction_state: InteractionState::None,
            transform: Transform::from_translation(
                position.axial_to_world(consts::HEX_SIZE).extend(5.0),
            ),
            visibility: Visibility::Visible,
            sprite: Sprite {
                color: owner_color.darker(0.2),
                custom_size: Some(Vec2::new(40.0, 30.0)),
                ..default()
            },
            pickable: Pickable::default(),
        },))
        .with_children(|parent| {
            // Label for displaying army size.
            parent.spawn((
                Text2d::new(composition.total_size().to_string()),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Center),
                ArmyLabel(composition.total_size().to_string()),
                Visibility::Visible,
            ));

            parent.spawn((
                Mesh2d(ring_mesh),
                MeshMaterial2d(ring_material),
                Transform::from_xyz(0.0, 0.0, -0.1),
                Visibility::Hidden,
                SelectedRing {},
            ));
        })
        .observe(handle_army_click)
        .id()
}

pub(crate) fn spawn_initial_armies(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut army_hex_map: ResMut<ArmyHexMap>,
    countries: Query<(Entity, &MapColor), With<Country>>,
    provinces: Query<(&Owner, &Province)>,
) {
    let mut country_provinces: HashMap<Entity, Vec<Hex>> = HashMap::new();

    for (owner, province) in provinces.iter() {
        country_provinces
            .entry(owner.0)
            .or_default()
            .push(*province.get_hex());
    }

    for (country, map_color) in countries.iter() {
        if let Some(province_hexes) = country_provinces.get(&country)
            && let Some(&start_hex) = province_hexes.first()
        {
            let army = spawn_army(
                &mut commands,
                &mut meshes,
                &mut materials,
                start_hex,
                country,
                map_color.0,
                ArmyComposition {
                    infantry: 10 * REGIMENT_SIZE,
                    cavalry: 2 * REGIMENT_SIZE,
                    artillery: REGIMENT_SIZE,
                },
            );
            army_hex_map.insert(HexPos(start_hex), army);
        }
    }
}

fn handle_army_click(
    click: On<Pointer<Click>>,
    mut selected: ResMut<SelectedArmy>,
    mut commands: Commands,
    player: Res<Player>,
    owners: Query<&Owner>,
) {
    if click.button != PointerButton::Primary {
        return;
    }

    info!("Army clicked: {:?}", click.entity);
    let clicked_entity = click.entity;

    if let Ok(owner) = owners.get(clicked_entity)
        && Some(owner.0) != player.country
    {
        info!("Cannot select army of another country");
        return;
    }

    if let Some(prev_entity) = selected.get() {
        if prev_entity == clicked_entity {
            // Checking if entity still exists is nice but here if clicked_entity exists, and prev == clicked, then prev exists.
            commands.entity(prev_entity).insert(InteractionState::None);
            selected.clear();
            return;
        }
        // SAFETY CHECK: Only issue commands if prev_entity still exists.
        if owners.contains(prev_entity) {
            commands.entity(prev_entity).insert(InteractionState::None);
        } else {
            // It's dead. Just ignore.
            warn!(
                "Previously selected army {:?} no longer exists.",
                prev_entity
            );
        }
    }
    commands
        .entity(clicked_entity)
        .insert(InteractionState::Selected);
    selected.set(clicked_entity);
}

pub(crate) fn handle_army_interaction_changed(
    army_query: Query<(&InteractionState, &Children), (With<Army>, Changed<InteractionState>)>,
    mut ring_query: Query<&mut Visibility, With<SelectedRing>>,
) {
    for (interaction_state, children) in &army_query {
        for &child in children {
            if let Ok(mut ring_visibility) = ring_query.get_mut(child) {
                match *interaction_state {
                    InteractionState::Selected => {
                        *ring_visibility = Visibility::Visible;
                    }
                    _ => {
                        *ring_visibility = Visibility::Hidden;
                    }
                }
            }
        }
    }
}

pub(crate) fn handle_army_composition_changed(
    army_query: Query<(&ArmyComposition, &Children), (With<Army>, Changed<ArmyComposition>)>,
    mut label_query: Query<(&mut ArmyLabel, &mut Text2d)>,
) {
    for (composition, children) in &army_query {
        for &child in children {
            if let Ok((mut label, mut text)) = label_query.get_mut(child) {
                let size_str = composition.total_size().to_string();
                label.0 = size_str.clone();
                *text = Text2d::new(size_str);
            }
        }
    }
}

pub(crate) fn display_army_panel(
    mut contexts: EguiContexts,
    mut commands: Commands,
    mut selected_army: ResMut<SelectedArmy>,
    armies: Query<(Entity, &ArmyComposition, &Owner), With<Army>>,
    countries: Query<&crate::country::DisplayName>,
) {
    let Some(army_entity) = selected_army.get() else {
        return;
    };

    let Ok((entity, composition, owner)) = armies.get(army_entity) else {
        return;
    };

    let ctx = match contexts.ctx_mut() {
        Ok(c) => c,
        Err(_) => return,
    };

    let owner_name = countries
        .get(owner.0)
        .map(|d| d.0.as_str())
        .unwrap_or("Unknown");

    egui::Window::new("Army")
        .frame(crate::egui_common::default_frame())
        .title_bar(false)
        .anchor(Align2::RIGHT_TOP, [-20.0, 20.0])
        .resizable(false)
        .default_width(200.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Army Info");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::egui_common::close_button(ui) {
                        commands.entity(entity).insert(InteractionState::None);
                        selected_army.clear();
                    }
                });
            });
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Owner:");
                ui.label(RichText::new(owner_name).color(Color32::from_rgb(100, 200, 255)));
            });

            ui.add_space(5.0);
            ui.label(RichText::new("Composition").strong());

            egui::Grid::new("army_comp_grid")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Infantry:");
                    ui.label(composition.infantry.to_string());
                    ui.end_row();

                    ui.label("Cavalry:");
                    ui.label(composition.cavalry.to_string());
                    ui.end_row();

                    ui.label("Artillery:");
                    ui.label(composition.artillery.to_string());
                    ui.end_row();

                    ui.separator();
                    ui.end_row();

                    ui.label(RichText::new("Total:").strong());
                    ui.label(RichText::new(composition.total_size().to_string()).strong());
                    ui.end_row();
                });
        });
}

pub(crate) fn display_battle_panel(
    mut contexts: EguiContexts,
    mut commands: Commands,
    mut selected_army: ResMut<SelectedArmy>,
    armies: Query<(&ArmyComposition, &Owner, Option<&InBattle>), With<Army>>,
    battles: Query<&Battle>,
    countries: Query<&crate::country::DisplayName>,
    province_map: Res<ProvinceHexMap>,
    provinces: Query<&Province>,
) {
    let Some(selected_entity) = selected_army.get() else {
        return;
    };

    // Check if selected army is in battle
    let Ok((_, _, maybe_in_battle)) = armies.get(selected_entity) else {
        return;
    };

    let Some(in_battle) = maybe_in_battle else {
        return; // Not in battle, don't show this panel
    };

    let Ok(battle) = battles.get(in_battle.battle_entity) else {
        return; // Battle entity missing?
    };

    // Get terrain at battle location
    let terrain = province_map
        .get_entity(&battle.location)
        .and_then(|&e| provinces.get(e).ok())
        .map(|p| p.terrain())
        .unwrap_or(crate::map::Terrain::Plains);

    let ctx = match contexts.ctx_mut() {
        Ok(c) => c,
        Err(_) => return,
    };

    egui::Window::new("Battle")
        .frame(crate::egui_common::default_frame())
        .title_bar(false)
        .anchor(Align2::RIGHT_TOP, [-20.0, 20.0])
        .resizable(false)
        .default_width(300.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("⚔ Battle ⚔");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::egui_common::close_button(ui) {
                        commands
                            .entity(selected_entity)
                            .insert(InteractionState::None);
                        selected_army.clear();
                    }
                });
            });
            ui.separator();

            // Terrain info
            ui.horizontal(|ui| {
                ui.label(format!("Terrain: {}", terrain));
                let def_bonus = terrain.defender_bonus();
                if def_bonus > 1.0 {
                    ui.label(
                        RichText::new(format!("(+{:.0}% def)", (def_bonus - 1.0) * 100.0))
                            .color(Color32::from_rgb(100, 100, 255)),
                    );
                } else if def_bonus < 1.0 {
                    ui.label(
                        RichText::new(format!("({:.0}% def)", (def_bonus - 1.0) * 100.0))
                            .color(Color32::from_rgb(255, 100, 100)),
                    );
                }
            });

            // Unit modifiers
            let cav_mod = terrain.cavalry_modifier();
            let art_mod = terrain.artillery_modifier();
            if cav_mod != 1.0 || art_mod != 1.0 {
                ui.horizontal(|ui| {
                    if cav_mod != 1.0 {
                        let color = if cav_mod > 1.0 {
                            Color32::GREEN
                        } else {
                            Color32::RED
                        };
                        ui.label(
                            RichText::new(format!("Cav: {:.0}%", cav_mod * 100.0)).color(color),
                        );
                    }
                    if art_mod != 1.0 {
                        let color = if art_mod > 1.0 {
                            Color32::GREEN
                        } else {
                            Color32::RED
                        };
                        ui.label(
                            RichText::new(format!("Art: {:.0}%", art_mod * 100.0)).color(color),
                        );
                    }
                });
            }

            ui.separator();
            ui.label(format!("Round: {}", battle.round));
            ui.separator();

            // Calculate total strength for each side
            let mut att_total = ArmyComposition {
                infantry: 0,
                cavalry: 0,
                artillery: 0,
            };
            let mut def_total = ArmyComposition {
                infantry: 0,
                cavalry: 0,
                artillery: 0,
            };

            for &army_entity in &battle.attackers {
                if let Ok((comp, _, _)) = armies.get(army_entity) {
                    att_total.infantry += comp.infantry;
                    att_total.cavalry += comp.cavalry;
                    att_total.artillery += comp.artillery;
                }
            }

            for &army_entity in &battle.defenders {
                if let Ok((comp, _, _)) = armies.get(army_entity) {
                    def_total.infantry += comp.infantry;
                    def_total.cavalry += comp.cavalry;
                    def_total.artillery += comp.artillery;
                }
            }

            // Columns for Attacker vs Defender
            ui.columns(2, |columns| {
                columns[0].vertical_centered(|ui| {
                    ui.label(
                        RichText::new("Attackers")
                            .strong()
                            .color(Color32::from_rgb(255, 100, 100)),
                    );
                    let attacker_name = countries
                        .get(battle.attacker_country)
                        .map(|d| d.0.as_str())
                        .unwrap_or("Unknown");
                    ui.label(format!("{} ({})", attacker_name, battle.attackers.len()));
                    ui.add_space(4.0);
                    ui.label(format!("Inf: {}", att_total.infantry));
                    ui.label(format!("Cav: {}", att_total.cavalry));
                    ui.label(format!("Art: {}", att_total.artillery));
                    ui.label(RichText::new(format!("Total: {}", att_total.total_size())).strong());
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("Lost: {}", battle.last_damage_attacker))
                            .color(Color32::RED),
                    );
                });

                columns[1].vertical_centered(|ui| {
                    ui.label(
                        RichText::new("Defenders")
                            .strong()
                            .color(Color32::from_rgb(100, 100, 255)),
                    );
                    let defender_name = countries
                        .get(battle.defender_country)
                        .map(|d| d.0.as_str())
                        .unwrap_or("Unknown");
                    ui.label(format!("{} ({})", defender_name, battle.defenders.len()));
                    ui.add_space(4.0);
                    ui.label(format!("Inf: {}", def_total.infantry));
                    ui.label(format!("Cav: {}", def_total.cavalry));
                    ui.label(format!("Art: {}", def_total.artillery));
                    ui.label(RichText::new(format!("Total: {}", def_total.total_size())).strong());
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("Lost: {}", battle.last_damage_defender))
                            .color(Color32::RED),
                    );
                });
            });
        });
}

pub(crate) fn resolve_battles(
    mut commands: Commands,
    mut battles: Query<(Entity, &mut Battle)>,
    mut armies: Query<(Entity, &mut ArmyComposition, &mut HexPos, &Owner)>,
    mut army_hex_map: ResMut<ArmyHexMap>,
    province_map: Res<ProvinceHexMap>,
    provinces: Query<(&Province, &Owner)>,
) {
    for (battle_entity, mut battle) in battles.iter_mut() {
        // Clean up dead armies from the battle
        battle.attackers.retain(|&e| {
            armies
                .get(e)
                .map(|(_, comp, _, _)| comp.total_size() > 0)
                .unwrap_or(false)
        });
        battle.defenders.retain(|&e| {
            armies
                .get(e)
                .map(|(_, comp, _, _)| comp.total_size() > 0)
                .unwrap_or(false)
        });

        // Check if battle should end
        if battle.attackers.is_empty() && battle.defenders.is_empty() {
            info!(
                "Battle at {:?} ended in mutual destruction after {} rounds",
                battle.location, battle.round
            );
            commands.entity(battle_entity).despawn();
            continue;
        } else if battle.attackers.is_empty() {
            info!(
                "Defenders won battle at {:?} after {} rounds",
                battle.location, battle.round
            );
            end_battle_multi(
                &mut commands,
                &mut armies,
                &mut army_hex_map,
                battle_entity,
                &battle,
                BattleSide::Defender,
                &province_map,
                &provinces,
            );
            continue;
        } else if battle.defenders.is_empty() {
            info!(
                "Attackers won battle at {:?} after {} rounds",
                battle.location, battle.round
            );
            end_battle_multi(
                &mut commands,
                &mut armies,
                &mut army_hex_map,
                battle_entity,
                &battle,
                BattleSide::Attacker,
                &province_map,
                &provinces,
            );
            continue;
        }

        // Get terrain at battle location for combat modifiers
        let terrain = province_map
            .get_entity(&battle.location)
            .and_then(|&e| provinces.get(e).ok())
            .map(|(p, _)| p.terrain())
            .unwrap_or(crate::map::Terrain::Plains);

        let defender_terrain_bonus = terrain.defender_bonus();
        let cavalry_modifier = terrain.cavalry_modifier();
        let artillery_modifier = terrain.artillery_modifier();

        // Log terrain effects on first round
        if battle.round == 0 {
            info!(
                "Battle at {:?} on {:?} terrain - Attackers: {} armies, Defenders: {} armies",
                battle.location,
                terrain,
                battle.attackers.len(),
                battle.defenders.len()
            );
        }

        // Calculate combined strength for each side
        fn calc_side_damage(
            armies: &Query<(Entity, &mut ArmyComposition, &mut HexPos, &Owner)>,
            army_list: &[Entity],
            cavalry_mod: f32,
            artillery_mod: f32,
        ) -> f32 {
            let mut total_damage = 0.0;
            for &army_entity in army_list {
                if let Ok((_, comp, _, _)) = armies.get(army_entity) {
                    total_damage += (comp.infantry as f32 * 0.5)
                        + (comp.cavalry as f32 * 1.0 * cavalry_mod)
                        + (comp.artillery as f32 * 2.0 * artillery_mod);
                }
            }
            total_damage
        }

        let mut rng = rand::rng();
        let att_roll: f32 = rng.random_range(0.8..1.2);
        let def_roll: f32 = rng.random_range(0.8..1.2);

        let att_base_dmg = calc_side_damage(
            &armies,
            &battle.attackers,
            cavalry_modifier,
            artillery_modifier,
        );
        let def_base_dmg = calc_side_damage(
            &armies,
            &battle.defenders,
            cavalry_modifier,
            artillery_modifier,
        );

        // Apply terrain bonuses
        let att_dmg = (att_base_dmg * att_roll / defender_terrain_bonus) as u32;
        let def_dmg = (def_base_dmg * def_roll * defender_terrain_bonus) as u32;

        // Distribute damage across armies on each side
        fn apply_damage_to_side(
            armies: &mut Query<(Entity, &mut ArmyComposition, &mut HexPos, &Owner)>,
            army_list: &[Entity],
            total_damage: u32,
        ) -> u32 {
            if army_list.is_empty() {
                return 0;
            }

            let damage_per_army = total_damage / army_list.len() as u32;
            let mut total_lost = 0;

            for &army_entity in army_list {
                if let Ok((_, mut comp, _, _)) = armies.get_mut(army_entity) {
                    let lost = apply_damage_to_composition(&mut comp, damage_per_army.max(1));
                    total_lost += lost;
                }
            }
            total_lost
        }

        let att_lost = apply_damage_to_side(&mut armies, &battle.attackers, def_dmg);
        let def_lost = apply_damage_to_side(&mut armies, &battle.defenders, att_dmg);

        battle.last_damage_attacker = att_lost;
        battle.last_damage_defender = def_lost;
        battle.round += 1;

        info!(
            "Battle round {} at {:?}: Attackers lost {}, Defenders lost {}",
            battle.round, battle.location, att_lost, def_lost
        );

        // Remove dead armies from hex map and despawn
        let mut to_despawn = Vec::new();
        for &army_entity in battle.attackers.iter().chain(battle.defenders.iter()) {
            if let Ok((_, comp, _, _)) = armies.get(army_entity)
                && comp.total_size() == 0
            {
                if let Some(pos) = army_hex_map
                    .tiles
                    .iter()
                    .find_map(|(k, v)| if *v == army_entity { Some(*k) } else { None })
                {
                    army_hex_map.remove(&pos);
                }
                to_despawn.push(army_entity);
            }
        }
        for army_entity in to_despawn {
            commands.entity(army_entity).despawn();
        }

        // Battle continues next turn - don't end it here
    }
}

fn apply_damage_to_composition(comp: &mut ArmyComposition, damage: u32) -> u32 {
    let units_lost = damage / 20;
    let mut remaining_to_kill = units_lost;

    if remaining_to_kill == 0 && damage > 0 {
        remaining_to_kill = damage.min(MIN_DAMAGE).min(comp.total_size());
    }

    let total = comp.total_size();
    if remaining_to_kill > total {
        remaining_to_kill = total;
    }
    if remaining_to_kill == 0 && total > 0 && damage > 0 {
        remaining_to_kill = 1;
    }

    let actual_lost = remaining_to_kill;

    // Distribute kills (Inf -> Cav -> Art)
    let kill_inf = remaining_to_kill.min(comp.infantry);
    comp.infantry -= kill_inf;
    remaining_to_kill -= kill_inf;

    let kill_cav = remaining_to_kill.min(comp.cavalry);
    comp.cavalry -= kill_cav;
    remaining_to_kill -= kill_cav;

    let kill_art = remaining_to_kill.min(comp.artillery);
    comp.artillery -= kill_art;

    actual_lost
}

fn end_battle_multi(
    commands: &mut Commands,
    armies: &mut Query<(Entity, &mut ArmyComposition, &mut HexPos, &Owner)>,
    army_hex_map: &mut ArmyHexMap,
    battle_entity: Entity,
    battle: &Battle,
    winner_side: BattleSide,
    province_map: &ProvinceHexMap,
    provinces: &Query<(&Province, &Owner)>,
) {
    let battle_location = battle.location;
    let winner_country = match winner_side {
        BattleSide::Attacker => battle.attacker_country,
        BattleSide::Defender => battle.defender_country,
    };

    let (winners, losers) = match winner_side {
        BattleSide::Attacker => (&battle.attackers, &battle.defenders),
        BattleSide::Defender => (&battle.defenders, &battle.attackers),
    };

    // Remove losers from hex map and despawn them
    for &army_entity in losers {
        // Find and remove from hex map
        if let Some(pos) = army_hex_map
            .tiles
            .iter()
            .find_map(|(k, v)| if *v == army_entity { Some(*k) } else { None })
        {
            army_hex_map.remove(&pos);
            info!(
                "Removed defeated army {:?} from hex map at {:?}",
                army_entity, pos
            );
        }
        commands.entity(army_entity).remove::<InBattle>();
        commands.entity(army_entity).despawn();
    }

    // Remove InBattle from all surviving armies and position them
    for &army_entity in winners {
        commands.entity(army_entity).remove::<InBattle>();

        // Move winner to battle location
        if let Ok((_, _, mut pos, _)) = armies.get_mut(army_entity) {
            // First remove from old position
            army_hex_map.remove(&pos);
            *pos = HexPos(battle_location);
            commands
                .entity(army_entity)
                .insert(Transform::from_translation(
                    battle_location.axial_to_world(consts::HEX_SIZE).extend(5.0),
                ));
        }
    }

    // Put one winner army on the hex map (others are "stacked")
    if let Some(&first_winner) = winners.first() {
        army_hex_map.insert(HexPos(battle_location), first_winner);
        info!(
            "Winner army {:?} placed at {:?}",
            first_winner, battle_location
        );
    }

    // Occupy province if attackers won
    if winner_side == BattleSide::Attacker
        && let Some(&province_entity) = province_map.get_entity(&battle_location)
    {
        if let Ok((province, owner)) = provinces.get(province_entity) {
            if province.is_ownable() && owner.0 != winner_country {
                crate::war::occupy_province(commands, province_entity, winner_country);
            }
        }
    }

    commands.entity(battle_entity).despawn();
}
