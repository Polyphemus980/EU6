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

#[derive(Component)]
pub(crate) struct Battle {
    pub(crate) attacker: Entity,
    pub(crate) defender: Entity,
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
                p.neighbors().into_iter().filter(|n| {
                    if let Some(&entity) = province_map.get_entity(n) {
                        if let Ok(province) = provinces.get(entity) {
                            return province.is_passable();
                        }
                    }
                    false
                })
            },
            |p| *p == event.to.0,
        );

        if let Some(path) = path {
            let mut deck = VecDeque::from(path);
            deck.pop_front(); // Remove current position
            if !deck.is_empty() {
                commands
                    .entity(event.army)
                    .insert(ActivePath { path: deck });
                info!("Army {:?} started moving to {:?}", event.army, event.to);
            }
        } else {
            info!("No path found for army {:?} to {:?}", event.army, event.to);
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
        ),
        With<Army>,
    >,
    mut selected_army: ResMut<SelectedArmy>,
) {
    let movers: Vec<Entity> = armies_query
        .iter()
        .filter_map(|(e, _, _, _, _, path)| if path.is_some() { Some(e) } else { None })
        .collect();

    for entity in movers {
        let (next_hex, old_pos) = {
            if let Ok((_, _, _, _, pos, Some(active_path))) = armies_query.get(entity) {
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

        if let Some(&occupant_entity) = army_hex_map.get(&next_pos) {
            // Check if already in battle to avoid double triggers or weird states
            // We can check if either entity has InBattle component, but we don't have it in query yet.
            // Let's assume if they have ActivePath they are not fighting yet (we remove it).

            if let Ok(
                [
                    (e1, _, owner1, comp1, _, _),
                    (e2, _, owner2, mut comp2, _, _),
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
                    // COMBAT START
                    info!(
                        "Battle started between {:?} (attacker) and {:?} (defender) at {:?}",
                        e1, e2, next_hex
                    );

                    // Stop movement
                    commands.entity(e1).remove::<ActivePath>();

                    // Create Battle Entity
                    let battle_id = commands
                        .spawn(Battle {
                            attacker: e1,
                            defender: e2,
                            location: next_hex,
                            round: 0,
                            last_damage_attacker: 0,
                            last_damage_defender: 0,
                        })
                        .id();

                    // Mark armies
                    commands.entity(e1).insert(InBattle {
                        battle_entity: battle_id,
                    });
                    commands.entity(e2).insert(InBattle {
                        battle_entity: battle_id,
                    });

                    continue;
                }
            } else {
                warn!("Could not retrieve both armies for collision resolution");
                continue;
            }
        }

        if let Ok((_, mut transform, _, _, mut pos, Some(mut active_path))) =
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
                    artillery: 1 * REGIMENT_SIZE,
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
    mut label_query: Query<(&mut ArmyLabel, &mut Text2d)>, // Query both ArmyLabel and Text2d
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
            ui.label(format!("Round: {}", battle.round));
            ui.separator();

            // Columns for Attacker vs Defender
            ui.columns(2, |columns| {
                columns[0].vertical_centered(|ui| {
                    ui.label(
                        RichText::new("Attacker")
                            .strong()
                            .color(Color32::from_rgb(255, 100, 100)),
                    );
                    if let Ok((comp, owner, _)) = armies.get(battle.attacker) {
                        let name = countries
                            .get(owner.0)
                            .map(|d| d.0.as_str())
                            .unwrap_or("Unknown");
                        ui.label(name);
                        ui.label(format!("Inf: {}", comp.infantry));
                        ui.label(format!("Cav: {}", comp.cavalry));
                        ui.label(format!("Art: {}", comp.artillery));
                        ui.label(format!("Total: {}", comp.total_size()));
                        ui.add_space(4.0);
                        ui.label(RichText::new(format!(
                            "-{} casualties",
                            battle.last_damage_defender
                        ))); // Damage dealt BY attacker (so defender casualties)
                        // Actually logic might be mixed. `last_damage_attacker` usually means damage SUFFERED by attacker.
                        // Let's assume `last_damage_attacker` is damage TAKEN by attacker.
                        ui.label(
                            RichText::new(format!("Lost: {}", battle.last_damage_attacker))
                                .color(Color32::RED),
                        );
                    } else {
                        ui.label("(Eliminated)");
                    }
                });

                columns[1].vertical_centered(|ui| {
                    ui.label(
                        RichText::new("Defender")
                            .strong()
                            .color(Color32::from_rgb(100, 100, 255)),
                    );
                    if let Ok((comp, owner, _)) = armies.get(battle.defender) {
                        let name = countries
                            .get(owner.0)
                            .map(|d| d.0.as_str())
                            .unwrap_or("Unknown");
                        ui.label(name);
                        ui.label(format!("Inf: {}", comp.infantry));
                        ui.label(format!("Cav: {}", comp.cavalry));
                        ui.label(format!("Art: {}", comp.artillery));
                        ui.label(format!("Total: {}", comp.total_size()));
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!("Lost: {}", battle.last_damage_defender))
                                .color(Color32::RED),
                        );
                    } else {
                        ui.label("(Eliminated)");
                    }
                });
            });
        });
}

pub(crate) fn resolve_battles(
    mut commands: Commands,
    mut battles: Query<(Entity, &mut Battle)>,
    mut armies: Query<(Entity, &mut ArmyComposition, &mut HexPos)>,
    mut army_hex_map: ResMut<ArmyHexMap>,
    province_map: Res<ProvinceHexMap>,
    provinces: Query<&Province>,
) {
    for (battle_entity, mut battle) in battles.iter_mut() {
        // Collect participants
        let (mut attacker_comp, mut attacker_pos) =
            if let Ok((_, c, p)) = armies.get_mut(battle.attacker) {
                (c, p)
            } else {
                // Attacker missing (despawned?), defender wins by default
                end_battle(
                    &mut commands,
                    battle_entity,
                    battle.defender,
                    battle.attacker,
                    &mut army_hex_map,
                    &province_map,
                    &provinces,
                    None,
                    None,
                );
                continue;
            };

        // Re-query needed because get_mut borrows exclusive
        // Actually we need get_many_mut
        let Ok(
            [
                (attacker_entity, mut attacker_comp, mut attacker_pos),
                (defender_entity, mut defender_comp, mut defender_pos),
            ],
        ) = armies.get_many_mut([battle.attacker, battle.defender])
        else {
            // Someone missing
            commands.entity(battle_entity).despawn();
            if let Ok((entity, _, _)) = armies.get(battle.attacker) {
                commands.entity(entity).remove::<InBattle>();
            }
            if let Ok((entity, _, _)) = armies.get(battle.defender) {
                commands.entity(entity).remove::<InBattle>();
            }
            continue;
        };

        // Calculate stats
        fn calc_strength(comp: &ArmyComposition) -> u32 {
            (comp.infantry as f32 * UnitType::Infantry.cost() * 0.1) as u32
                + (comp.cavalry as f32 * UnitType::Cavalry.cost() * 0.1) as u32
                + (comp.artillery as f32 * UnitType::Artillery.cost() * 0.1) as u32
        }

        // Basic damage formula: 10% of "cost" value?
        // User asked: "damage based on its type".
        // Let's use: Inf=1, Cav=2, Art=3 per unit.
        fn calc_damage(comp: &ArmyComposition) -> u32 {
            // Damage scaling:
            // Infantry: 0.5 per man
            // Cavalry: 1.0 per man
            // Artillery: 2.0 per man
            // With 10,000 inf => 5,000 damage score.

            ((comp.infantry as f32 * 0.5)
                + (comp.cavalry as f32 * 1.0)
                + (comp.artillery as f32 * 2.0)) as u32
        }

        let mut rng = rand::rng();
        // Random multiplier between 0.8 and 1.2
        let att_roll: f32 = rng.random_range(0.8..1.2);
        let def_roll: f32 = rng.random_range(0.8..1.2);

        let att_dmg = (calc_damage(&attacker_comp) as f32 * att_roll) as u32;
        let def_dmg = (calc_damage(&defender_comp) as f32 * def_roll) as u32;

        // Apply casualties (simultaneous)
        // Divide incoming damage score by a defense factor / toughness.
        // Let's say damage score / 10 = men killed.
        // 5,000 damage / 10 = 500 killed (5% of 10k).
        fn apply_damage(comp: &mut ArmyComposition, damage: u32) -> u32 {
            let units_lost = damage / 20; // 2.5% casualties per round approx base
            let mut remaining_to_kill = units_lost;

            // Ensure at least 1 kill if there is overwhelming damage but divide makes it 0 (unlikely with thousands)
            // or if damage is decent but armies small.
            // NEW: Always ensure some minimum damage if there is any damage at all, to prevent stalling.
            if remaining_to_kill == 0 && damage > 0 {
                remaining_to_kill = damage.min(MIN_DAMAGE).min(comp.total_size());
            }

            // Cap kills at total size (cannot kill more than exist)
            let total = comp.total_size();
            if remaining_to_kill > total {
                remaining_to_kill = total;
            }
            // Ensure we kill at least 1 unit if units exist, to prevent infinite loops with micro armies (e.g. 5 vs 5)
            if remaining_to_kill == 0 && total > 0 && damage > 0 {
                remaining_to_kill = 1;
            }

            let actual_lost = remaining_to_kill;

            // Distribute kills (Inf -> Cav -> Art)
            // "Meat shield" logic
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

        let att_lost = apply_damage(&mut attacker_comp, def_dmg);
        let def_lost = apply_damage(&mut defender_comp, att_dmg);

        battle.last_damage_attacker = att_lost;
        battle.last_damage_defender = def_lost;
        battle.round += 1;

        // Result check
        let att_alive = attacker_comp.total_size() > 0;
        let def_alive = defender_comp.total_size() > 0;

        if !att_alive && !def_alive {
            // Mutual destruction
            info!(
                "Battle at {:?} ended in mutual destruction",
                battle.location
            );
            end_battle(
                &mut commands,
                battle_entity,
                Entity::PLACEHOLDER,
                Entity::PLACEHOLDER,
                &mut army_hex_map,
                &province_map,
                &provinces,
                Some(attacker_entity),
                Some(defender_entity),
            );
        } else if !att_alive {
            // Defender wins
            info!(
                "Defender {:?} won battle at {:?}",
                defender_entity, battle.location
            );
            end_battle(
                &mut commands,
                battle_entity,
                defender_entity,
                attacker_entity,
                &mut army_hex_map,
                &province_map,
                &provinces,
                Some(attacker_entity),
                None,
            );
        } else if !def_alive {
            // Attacker wins
            info!(
                "Attacker {:?} won battle at {:?}",
                attacker_entity, battle.location
            );
            // Attacker moves to tile? Or stays at previous?
            // Usually attacker *moves into* the tile if they win.
            // Defender was at `battle.location`. Attacker was at `attacker_pos` (neighbor).
            // We should move attacker to `battle.location`.
            army_hex_map.remove(&attacker_pos);
            army_hex_map.insert(HexPos(battle.location), attacker_entity);
            *attacker_pos = HexPos(battle.location);
            commands
                .entity(attacker_entity)
                .insert(Transform::from_translation(
                    battle.location.axial_to_world(consts::HEX_SIZE).extend(5.0),
                ));

            end_battle(
                &mut commands,
                battle_entity,
                attacker_entity,
                defender_entity,
                &mut army_hex_map,
                &province_map,
                &provinces,
                Some(defender_entity),
                None,
            );
        } else {
            // Battle continues
            // Can add morale check here later
        }
    }
}

fn end_battle(
    commands: &mut Commands,
    battle_entity: Entity,
    winner: Entity,
    loser: Entity,
    army_hex_map: &mut ArmyHexMap,
    province_map: &ProvinceHexMap,
    provinces: &Query<&Province>,
    dead_entity_1: Option<Entity>,
    dead_entity_2: Option<Entity>,
) {
    commands.entity(battle_entity).despawn();

    // Despawn dead
    if let Some(e) = dead_entity_1 {
        if e != Entity::PLACEHOLDER {
            if let Some(pos) = army_hex_map
                .tiles
                .iter()
                .find_map(|(k, v)| if *v == e { Some(*k) } else { None })
            {
                army_hex_map.remove(&pos);
            }
            commands.entity(e).despawn();
        }
    }
    if let Some(e) = dead_entity_2 {
        if e != Entity::PLACEHOLDER && Some(e) != dead_entity_1 {
            if let Some(pos) = army_hex_map
                .tiles
                .iter()
                .find_map(|(k, v)| if *v == e { Some(*k) } else { None })
            {
                army_hex_map.remove(&pos);
            }
            commands.entity(e).despawn();
        }
    }

    // Cleanup InBattle components for survivors
    if winner != Entity::PLACEHOLDER
        && Some(winner) != dead_entity_1
        && Some(winner) != dead_entity_2
    {
        commands.entity(winner).remove::<InBattle>();
    }
    if loser != Entity::PLACEHOLDER && Some(loser) != dead_entity_1 && Some(loser) != dead_entity_2
    {
        commands.entity(loser).remove::<InBattle>();
    }
}
