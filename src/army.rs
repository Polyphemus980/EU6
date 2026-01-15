use crate::consts;
use crate::country::{Country, MapColor};
use crate::hex::Hex;
use crate::map::{InteractionState, Owner, Province, ProvinceHexMap};
use crate::player::Player;
use bevy::ecs::error::Result;
use bevy::mesh::Mesh;
use bevy::prelude::*;
use bevy::sprite::Sprite;
use pathfinding::prelude::bfs;
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
            .add_systems(Update, handle_army_composition_changed);
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

impl ArmyComposition {
    pub(crate) fn total_size(&self) -> u32 {
        self.infantry + self.cavalry + self.artillery
    }

    pub(crate) fn add(&mut self, other: &ArmyComposition) {
        self.infantry += other.infantry;
        self.cavalry += other.cavalry;
        self.artillery += other.artillery;
    }
}

#[derive(Component)]
pub(crate) struct ArmyLabel(pub(crate) String);

#[derive(Component)]
pub(crate) struct SelectedRing {}

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

                    continue;
                } else {
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
                    infantry: 10,
                    cavalry: 2,
                    artillery: 1,
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
            commands.entity(prev_entity).insert(InteractionState::None);
            selected.clear();
            return;
        }
        commands.entity(prev_entity).insert(InteractionState::None);
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
    mut label_query: Query<&mut ArmyLabel>,
) {
    for (composition, children) in &army_query {
        for &child in children {
            if let Ok(mut label) = label_query.get_mut(child) {
                label.0 = composition.total_size().to_string();
            }
        }
    }
}
