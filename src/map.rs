use crate::consts;
use crate::hex::Hex;
use bevy::asset::Assets;
use bevy::color::{Color, Mix};
use bevy::mesh::{Mesh, Mesh2d};
use bevy::prelude::{
    Changed, ColorMaterial, Commands, Component, Entity, MeshMaterial2d, Query, RegularPolygon,
    ResMut, Resource, Transform,
};
use std::collections::HashMap;

/// Resource mapping hex coordinates to province entities. Allows clicking on hex tiles to find
/// the corresponding province.
#[derive(Resource, Default)]
pub(crate) struct HexMap {
    tiles: HashMap<Hex, Entity>,
}

impl HexMap {
    pub(crate) fn get_entity(&self, hex: &Hex) -> Option<&Entity> {
        self.tiles.get(hex)
    }
}

/// Resource tracking the currently selected province entity, if there exists any.
#[derive(Resource, Default)]
pub(crate) struct SelectedProvince {
    selected: Option<Entity>,
}

impl SelectedProvince {
    pub(crate) fn set(&mut self, entity: Entity) {
        self.selected = Some(entity);
    }

    pub(crate) fn clear(&mut self) {
        self.selected = None;
    }

    pub(crate) fn get(&self) -> Option<Entity> {
        self.selected
    }
}

/// Component indicating that a province is currently selected.
#[derive(Component, Default, PartialEq, Copy, Clone)]
pub(crate) enum InteractionState {
    #[default]
    None,
    Selected,
}

/// Component representing a province on the map.
#[derive(Component)]
pub(crate) struct Province {
    id: u32,
    name: String,
    hex: Hex,
    terrain: Terrain,
}

impl Province {
    fn color(&self) -> Color {
        self.terrain.color()
    }
}

const COLOR_PLAINS: Color = Color::srgb(0.46, 0.79, 0.26); // Grass green
const COLOR_HILLS: Color = Color::srgb(0.58, 0.44, 0.27); // Muted brown
const COLOR_MOUNTAINS: Color = Color::srgb(0.45, 0.45, 0.5); // Slate gray
const COLOR_FOREST: Color = Color::srgb(0.07, 0.31, 0.12); // Deep dark green
const COLOR_DESERT: Color = Color::srgb(0.93, 0.79, 0.48); // Sandy yellow/tan

const COLOR_SEA: Color = Color::srgb(0.0, 0.53, 0.74);

/// Enum representing different terrain types for provinces.
#[derive(Clone, Copy)]
enum Terrain {
    Plains,
    Hills,
    Mountains,
    Forest,
    Desert,
    Sea,
}

impl Terrain {
    const fn color(&self) -> Color {
        match self {
            Terrain::Plains => COLOR_PLAINS,
            Terrain::Hills => COLOR_HILLS,
            Terrain::Mountains => COLOR_MOUNTAINS,
            Terrain::Forest => COLOR_FOREST,
            Terrain::Desert => COLOR_DESERT,
            Terrain::Sea => COLOR_SEA,
        }
    }
}

/// Converts an u8 value to a Terrain variant for simple terrain assignment.
impl From<u8> for Terrain {
    fn from(value: u8) -> Self {
        match value {
            0 => Terrain::Plains,
            1 => Terrain::Hills,
            2 => Terrain::Mountains,
            3 => Terrain::Forest,
            4 => Terrain::Desert,
            _ => Terrain::Sea,
        }
    }
}

/// System to generate a hex map of provinces at startup.
pub(crate) fn generate_map(
    mut commands: Commands,
    mut hex_map: ResMut<HexMap>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let map_radius = 5i32;

    for q in -map_radius..=map_radius {
        for r in -map_radius..=map_radius {
            let hex = Hex::new(q, r);

            let province = Province {
                id: 1,
                name: format!("Province_{}_{}", q, r),
                hex,
                terrain: Terrain::from(((q + r) % 6) as u8),
            };

            let province_entity =
                build_province_entity(&mut meshes, &mut materials, province, consts::HEX_SIZE);

            let province_id = commands.spawn(province_entity).id();

            hex_map.tiles.insert(hex, province_id);
        }
    }
}

/// Builds the visual representation of a province as a hex tile.
fn build_province_entity(
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    province: Province,
    size: f32,
) -> (Province, Mesh2d, MeshMaterial2d<ColorMaterial>, Transform) {
    let mesh = Mesh::from(RegularPolygon::new(size, 6));
    let mesh_handle = meshes.add(mesh);

    let color = province.color();
    let material_handle = materials.add(color);

    let hex = province.hex;
    let transform = Transform::from_translation(hex.axial_to_world(size).extend(0.0));

    (
        province,
        Mesh2d(mesh_handle),
        MeshMaterial2d(material_handle),
        transform,
    )
}

/// System to update province visuals based on selection and hover states.
pub(crate) fn province_visual_system(
    mut materials: ResMut<Assets<ColorMaterial>>,
    query: Query<
        (&Province, &MeshMaterial2d<ColorMaterial>, &InteractionState),
        Changed<InteractionState>,
    >,
) {
    for (province, material, state) in &query {
        if let Some(mat) = materials.get_mut(&material.0) {
            let base_color = province.color();

            mat.color = match *state {
                InteractionState::Selected => base_color.mix(&Color::srgb(1.0, 0.9, 0.0), 0.4),
                InteractionState::None => base_color,
            };
        }
    }
}
