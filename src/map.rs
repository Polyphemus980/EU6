use crate::consts;
use crate::country::{MapColor, Name};
use crate::hex::Hex;
use bevy::asset::Assets;
use bevy::color::{Color, Mix};
use bevy::mesh::{Mesh, Mesh2d};
use bevy::prelude::{
    ColorMaterial, Commands, Component, Entity, Local, MeshMaterial2d, Query, RegularPolygon,
    ResMut, Resource, Transform,
};
use bevy_egui::egui::Align2;
use bevy_egui::{egui, EguiContexts};
use std::collections::HashMap;
use std::fmt::Display;

#[derive(Resource, Default, PartialEq)]
pub(crate) enum MapMode {
    #[default]
    Terrain,
    Political,
}

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

#[derive(Component)]
pub(crate) struct Owner(pub(crate) Entity);

/// Component representing a province on the map.
#[derive(Component)]
pub(crate) struct Province {
    name: String,
    hex: Hex,
    terrain: Terrain,
}

impl Province {
    /// Returns the color associated with the province's terrain type.
    fn color(&self) -> Color {
        self.terrain.color()
    }

    /// Returns a reference to the hex coordinates of the province.
    pub(crate) fn get_hex(&self) -> &Hex {
        &self.hex
    }

    /// Determines if the province can be owned by a country based on its terrain type.
    pub(crate) fn is_ownable(&self) -> bool {
        !matches!(self.terrain, Terrain::Sea | Terrain::Wasteland)
    }
}

const COLOR_PLAINS: Color = Color::srgb(0.46, 0.79, 0.26); // Grass green
const COLOR_HILLS: Color = Color::srgb(0.58, 0.44, 0.27); // Muted brown
const COLOR_MOUNTAINS: Color = Color::srgb(0.45, 0.45, 0.5); // Slate gray
const COLOR_FOREST: Color = Color::srgb(0.07, 0.31, 0.12); // Deep dark green
const COLOR_DESERT: Color = Color::srgb(0.93, 0.79, 0.48); // Sandy yellow/tan
const COLOR_WASTELAND: Color = Color::srgb(0.55, 0.50, 0.45); // Barren grayish-brown

const COLOR_SEA: Color = Color::srgb(0.0, 0.53, 0.74); // Ocean blue

/// Enum representing different terrain types for provinces.
#[derive(Clone, Copy, PartialEq)]
enum Terrain {
    Plains,
    Hills,
    Mountains,
    Forest,
    Desert,
    Wasteland,
    Sea,
}

impl Display for Terrain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let terrain_str = match self {
            Terrain::Plains => "Plains",
            Terrain::Hills => "Hills",
            Terrain::Mountains => "Mountains",
            Terrain::Forest => "Forest",
            Terrain::Desert => "Desert",
            Terrain::Wasteland => "Wasteland",
            Terrain::Sea => "Sea",
        };
        write!(f, "{}", terrain_str)
    }
}

impl Terrain {
    const fn color(&self) -> Color {
        match self {
            Terrain::Plains => COLOR_PLAINS,
            Terrain::Hills => COLOR_HILLS,
            Terrain::Mountains => COLOR_MOUNTAINS,
            Terrain::Forest => COLOR_FOREST,
            Terrain::Desert => COLOR_DESERT,
            Terrain::Wasteland => COLOR_WASTELAND,
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
    let map_radius = 8i32;

    for q in -map_radius..=map_radius {
        for r in -map_radius..=map_radius {
            let hex = Hex::new(q, r);

            // Calculate distance from center for terrain generation
            let distance = hex.distance(&Hex::ZERO);

            // Skip hexes outside the map radius
            if distance > map_radius {
                continue;
            }

            // Generate terrain based on distance from center and position
            let terrain = if distance >= map_radius - 1 {
                // Outer ring is sea
                Terrain::Sea
            } else if distance >= map_radius - 2 {
                // Next ring is mostly wasteland with some sea
                if (q + r) % 3 == 0 {
                    Terrain::Wasteland
                } else {
                    Terrain::Sea
                }
            } else {
                // Inner areas have varied terrain
                match ((q.abs() + r.abs()) % 5) as u8 {
                    0 => Terrain::Plains,
                    1 => Terrain::Hills,
                    2 => Terrain::Forest,
                    3 => Terrain::Mountains,
                    4 => Terrain::Desert,
                    _ => Terrain::Plains,
                }
            };

            let province = Province {
                name: format!("Province_{}_{}", q, r),
                hex,
                terrain,
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
) -> (
    Province,
    Mesh2d,
    MeshMaterial2d<ColorMaterial>,
    Transform,
    InteractionState,
) {
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
        InteractionState::None,
    )
}

/// System to update province visuals based on if the province is selected or not. Uses
/// province's terrain color.
pub(crate) fn render_province_terrain(
    mut materials: ResMut<Assets<ColorMaterial>>,
    query: Query<(&Province, &MeshMaterial2d<ColorMaterial>, &InteractionState)>,
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

/// System to update province visuals based on if the province is selected or not. Uses
/// province's owner color
pub(crate) fn render_province_political(
    mut materials: ResMut<Assets<ColorMaterial>>,
    query: Query<(
        &Province,
        Option<&Owner>,
        &MeshMaterial2d<ColorMaterial>,
        &InteractionState,
    )>,
    country_query: Query<&MapColor>,
) {
    for (province, maybe_owner, material, state) in query {
        if let Some(mat) = materials.get_mut(&material.0) {
            let base_color = if let Some(owner) = maybe_owner {
                if let Ok(map_color) = country_query.get(owner.0) {
                    map_color.0
                } else {
                    province.color()
                }
            } else {
                province.color()
            };

            mat.color = match *state {
                InteractionState::Selected => base_color.mix(&Color::srgb(1.0, 0.9, 0.0), 0.4),
                InteractionState::None => base_color,
            };
        }
    }
}

pub(crate) fn switch_map_mode(map_mode: &mut ResMut<MapMode>) {
    **map_mode = match **map_mode {
        MapMode::Terrain => MapMode::Political,
        MapMode::Political => MapMode::Terrain,
    };
}

pub(crate) fn display_province_panel(
    mut contexts: EguiContexts,
    selected: ResMut<SelectedProvince>,
    provinces: Query<(&Province, Option<&Owner>)>,
    countries: Query<&Name>,
    mut current_tab: Local<ProvinceTab>,
) {
    if let Some(selected_province) = selected.get()
        && let Ok((province, maybe_owner)) = provinces.get(selected_province)
    {
        let owner_name = if let Some(owner) = maybe_owner {
            if let Ok(name) = countries.get(owner.0) {
                name.0.clone()
            } else {
                "Unknown".to_string()
            }
        } else {
            "Unowned".to_string()
        };
        if let Ok(ctx) = contexts.ctx_mut() {
            egui::Window::new("Province Details")
                .default_size([200f32, 150f32])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut *current_tab, ProvinceTab::Overview, "Overview");
                        ui.selectable_value(&mut *current_tab, ProvinceTab::Buildings, "Buildings");
                    });

                    ui.separator();

                    if *current_tab == ProvinceTab::Buildings {
                        ui.label("Buildings tab is under construction.");
                        return;
                    }

                    ui.label(format!("Name: {}", province.name));
                    ui.label(format!("Owner: {}", owner_name));
                    ui.label(format!("Terrain: {}", province.terrain));
                });
        }
    }
}

pub(crate) fn display_map_modes_panel(mut contexts: EguiContexts, mut map_mode: ResMut<MapMode>) {
    if let Ok(ctx) = contexts.ctx_mut() {
        let font_id = egui::FontId::proportional(24.0);
        egui::Area::new(egui::Id::new("map_modes"))
            .anchor(Align2::RIGHT_BOTTOM, [0.0, 0.0])
            .show(ctx, |ui| {
                if ui
                    .add_sized(
                        [50.0, 50.0],
                        egui::Button::selectable(
                            *map_mode == MapMode::Terrain,
                            egui::RichText::new("🌲").font(font_id.clone()),
                        ),
                    )
                    .on_hover_text("Terrain")
                    .clicked()
                {
                    *map_mode = MapMode::Terrain
                }

                if ui
                    .add_sized(
                        [50.0, 50.0],
                        egui::Button::selectable(
                            *map_mode == MapMode::Political,
                            egui::RichText::new("🏁").font(font_id),
                        ),
                    )
                    .on_hover_text("Political")
                    .clicked()
                {
                    *map_mode = MapMode::Political
                }
            });
    }
}
#[derive(PartialEq, Default)]
pub(crate) enum ProvinceTab {
    #[default]
    Overview,
    Buildings,
}
