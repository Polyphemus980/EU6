use crate::army::{HexPos, MoveArmyEvent, SelectedArmy};
use crate::buildings::Income;
use crate::country::{DisplayName, MapColor, SelectedCountry};
use crate::hex::Hex;
use crate::{consts, egui_common};
use bevy::asset::Assets;
use bevy::color::{Color, Mix};
use bevy::mesh::{Mesh, Mesh2d};
use bevy::picking::Pickable;
use bevy::prelude::{
    Click, ColorMaterial, Commands, Component, Entity, Local, MeshMaterial2d, MessageWriter, On,
    Pointer, PointerButton, Query, RegularPolygon, ResMut, Resource, Transform,
};
use bevy::prelude::{Res, Result};
use bevy_egui::egui::{Align2, Color32, RichText, Stroke};
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use std::collections::HashMap;
use std::fmt::Display;

pub struct MapPlugin;

impl bevy::prelude::Plugin for MapPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        use bevy::prelude::*;
        app.insert_resource(ProvinceHexMap::default())
            .insert_resource(SelectedProvince::default())
            .insert_resource(MapMode::default())
            .add_systems(Startup, generate_map)
            .add_systems(Update, update_province_colors)
            .add_systems(EguiPrimaryContextPass, display_province_panel)
            .add_systems(EguiPrimaryContextPass, display_map_modes_panel);
    }
}

#[derive(Resource, Default, PartialEq)]
pub(crate) enum MapMode {
    #[default]
    Terrain,
    Political,
}

/// Resource mapping hex coordinates to province entities. Allows clicking on hex tiles to find
/// the corresponding province.
#[derive(Resource, Default)]
pub(crate) struct ProvinceHexMap {
    tiles: HashMap<Hex, Entity>,
}

impl ProvinceHexMap {
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

/// Component indicating that an entity is currently selected.
#[derive(Component, Default, PartialEq, Copy, Clone)]
pub(crate) enum InteractionState {
    #[default]
    None,
    Selected,
}

#[derive(Component, PartialEq)]
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
    pub(crate) fn is_passable(&self) -> bool {
        !matches!(self.terrain, Terrain::Sea | Terrain::Wasteland)
    }
    pub(crate) fn base_income(&self) -> f32 {
        self.terrain.base_income()
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

    const fn base_income(&self) -> f32 {
        match self {
            Terrain::Plains => 0.2,
            Terrain::Hills => 0.16,
            Terrain::Mountains => 0.1,
            Terrain::Forest => 0.14,
            Terrain::Desert => 0.5,
            Terrain::Wasteland => 0.0,
            Terrain::Sea => 0.0,
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
    mut hex_map: ResMut<ProvinceHexMap>,
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

            let province_id = commands
                .spawn(province_entity)
                .observe(handle_province_click)
                .id();

            hex_map.tiles.insert(hex, province_id);
        }
    }
}

/// Event handler for when a province is clicked. Manages selection and deselection of provinces.
fn handle_province_click(
    click: On<Pointer<Click>>,
    mut selected_province: ResMut<SelectedProvince>,
    selected_army: Res<SelectedArmy>,
    mut army_event_messenger: MessageWriter<MoveArmyEvent>,
    mut commands: Commands,
    province: Query<&Province>,
) -> Result {
    let clicked_entity = click.entity;

    if let Some(army) = selected_army.get()
        && click.button == PointerButton::Secondary
    {
        let province_pos = province.get(clicked_entity).map(|p| p.get_hex())?;
        army_event_messenger.write(MoveArmyEvent::new(army, HexPos::new(*province_pos)));
        return Ok(());
    }

    if click.button != PointerButton::Primary {
        return Ok(());
    }

    // 1. Deselect the previous entity if it exists
    if let Some(prev_entity) = selected_province.get() {
        // If the user clicks the same hex, just deselect and return
        if prev_entity == clicked_entity {
            commands.entity(prev_entity).insert(InteractionState::None);
            selected_province.clear();
            return Ok(());
        }

        // Otherwise, reset the old one before selecting the new one
        commands.entity(prev_entity).insert(InteractionState::None);
    }

    // 2. Select the new entity
    commands
        .entity(clicked_entity)
        .insert(InteractionState::Selected);
    selected_province.set(clicked_entity);

    Ok(())
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
    Income,
    Pickable,
) {
    let mesh = Mesh::from(RegularPolygon::new(size, 6));
    let mesh_handle = meshes.add(mesh);

    let color = province.color();
    let material_handle = materials.add(color);

    let hex = province.hex;
    let transform = Transform::from_translation(hex.axial_to_world(size).extend(0.0));

    let income = Income::new(province.base_income());

    (
        province,
        Mesh2d(mesh_handle),
        MeshMaterial2d(material_handle),
        transform,
        InteractionState::None,
        income,
        Pickable::default(),
    )
}

/// System to update province visuals based on map mode and selection state.
pub(crate) fn update_province_colors(
    mut materials: ResMut<Assets<ColorMaterial>>,
    map_mode: Res<MapMode>,
    query: Query<(
        &Province,
        Option<&Owner>,
        &MeshMaterial2d<ColorMaterial>,
        &InteractionState,
    )>,
    country_query: Query<&MapColor>,
) {
    let selection_mix = 0.4;
    let selection_color = Color::srgb(1.0, 0.9, 0.0);

    for (province, maybe_owner, material, state) in &query {
        if let Some(mat) = materials.get_mut(&material.0) {
            let base_color = match *map_mode {
                MapMode::Terrain => province.color(),
                MapMode::Political => {
                    if let Some(owner) = maybe_owner
                        && let Ok(map_color) = country_query.get(owner.0)
                    {
                        map_color.0
                    } else {
                        province.color()
                    }
                }
            };

            mat.color = match *state {
                InteractionState::Selected => base_color.mix(&selection_color, selection_mix),
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
    mut commands: Commands,
    mut contexts: EguiContexts,
    mut selected_province: ResMut<SelectedProvince>,
    mut selected_country: ResMut<SelectedCountry>,
    provinces: Query<(&Province, Option<&Owner>)>,
    countries: Query<&DisplayName>,
    mut current_tab: Local<ProvinceTab>,
) {
    let Some(selected_id) = selected_province.get() else {
        return;
    };
    let Ok((province, maybe_owner)) = provinces.get(selected_id) else {
        return;
    };

    let owner_name = maybe_owner
        .and_then(|owner| countries.get(owner.0).ok())
        .map(|name| name.0.clone())
        .unwrap_or_else(|| "Unowned".to_string());

    let ctx = match contexts.ctx_mut() {
        Ok(c) => c,
        Err(_) => return,
    };

    let panel_frame = egui_common::default_frame();

    egui::Window::new("Province")
        .frame(panel_frame)
        .title_bar(false)
        .anchor(Align2::LEFT_TOP, [20.0, 20.0])
        .resizable(false)
        .default_width(250.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(
                    RichText::new(&province.name)
                        .font(egui::FontId::proportional(22.0))
                        .color(Color32::WHITE)
                        .strong(),
                ));
                ui.add_space(8.0);
                if egui_common::close_button(ui) {
                    commands.entity(selected_id).insert(InteractionState::None);
                    selected_province.clear();
                }
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let tabs = [
                    (ProvinceTab::Overview, "Overview"),
                    (ProvinceTab::Buildings, "Buildings"),
                ];

                for (tab, label) in tabs {
                    let is_selected = *current_tab == tab;
                    let text_color = if is_selected {
                        Color32::WHITE
                    } else {
                        Color32::GRAY
                    };
                    let bg_color = if is_selected {
                        Color32::from_rgb(60, 80, 120)
                    } else {
                        Color32::TRANSPARENT
                    };

                    if ui
                        .add(
                            egui::Button::new(RichText::new(label).color(text_color))
                                .fill(bg_color)
                                .stroke(Stroke::new(1.0, Color32::from_rgb(100, 100, 100))),
                        )
                        .clicked()
                    {
                        *current_tab = tab;
                    }
                }
            });

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(8.0);

            match *current_tab {
                ProvinceTab::Buildings => {
                    ui.label(
                        RichText::new("Buildings tab is under construction.")
                            .italics()
                            .weak(),
                    );
                }
                ProvinceTab::Overview => {
                    egui::Grid::new("province_stats")
                        .num_columns(2)
                        .spacing([20.0, 8.0])
                        .show(ui, |ui| {
                            ui.label(RichText::new("Owner").color(Color32::LIGHT_GRAY));
                            if ui
                                .button(
                                    RichText::new(&owner_name)
                                        .color(Color32::from_rgb(100, 200, 255))
                                        .underline(),
                                )
                                .clicked()
                                && let Some(owner) = maybe_owner
                            {
                                selected_country.select(owner.0);
                            }
                            ui.end_row();

                            ui.label(RichText::new("Terrain").color(Color32::LIGHT_GRAY));
                            ui.label(
                                RichText::new(province.terrain.to_string()).color(Color32::WHITE),
                            );
                            ui.end_row();
                        });
                }
            }
        });
}

/// Egui component for showing and selecting possible map modes (political and terrain).
pub(crate) fn display_map_modes_panel(mut contexts: EguiContexts, mut map_mode: ResMut<MapMode>) {
    let ctx = match contexts.ctx_mut() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    let font_id = egui::FontId::proportional(24.0);
    egui::Area::new(egui::Id::new("map_modes"))
        .anchor(Align2::RIGHT_BOTTOM, [0.0, 0.0])
        .show(ctx, |ui| {
            if ui
                .add_sized(
                    [50.0, 50.0],
                    egui::Button::selectable(
                        *map_mode == MapMode::Terrain,
                        RichText::new("🌲").font(font_id.clone()),
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
                        RichText::new("🏁").font(font_id),
                    ),
                )
                .on_hover_text("Political")
                .clicked()
            {
                *map_mode = MapMode::Political
            }
        });
}

#[derive(PartialEq, Default)]
pub(crate) enum ProvinceTab {
    #[default]
    Overview,
    Buildings,
}
