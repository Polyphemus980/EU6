use crate::egui_common;
use crate::map::{Owner, Province};
use bevy::prelude::*;
use bevy_egui::egui::{Color32, RichText};
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

pub struct CountryPlugin;

impl Plugin for CountryPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SelectedCountry::default())
            .add_systems(Startup, setup_countries)
            .add_systems(
                Startup,
                assign_province_ownership
                    .after(crate::map::generate_map)
                    .after(setup_countries),
            )
            .add_systems(EguiPrimaryContextPass, display_country_panel);
    }
}

/// Marker component for country entities. No data as I am trying to do ECS :P.
#[derive(Component)]
pub(crate) struct Country {}

/// Components representing the name and map color of a faction. They are not attached to the
/// country entity, as things like rebels may have names/colors but not be countries.
#[derive(Component)]
pub(crate) struct DisplayName(pub(crate) String);
#[derive(Component)]
pub(crate) struct MapColor(pub(crate) Color);

/// Component representing the amount of gold a country has.
#[derive(Component)]
pub(crate) struct Coffer(pub(crate) f32);

impl Coffer {
    pub(crate) fn add_ducats(&mut self, ducats: f32) {
        self.0 += ducats;
    }

    pub(crate) fn remove_ducats(&mut self, ducats: f32) {
        self.0 -= ducats;
    }
}

#[derive(Resource, Default)]
pub(crate) struct SelectedCountry {
    selected: Option<Entity>,
}

impl SelectedCountry {
    pub(crate) fn clear(&mut self) {
        self.selected = None;
    }

    pub(crate) fn select(&mut self, country: Entity) {
        self.selected = Some(country);
    }

    pub(crate) fn get(&self) -> Option<Entity> {
        self.selected
    }
}

#[derive(Bundle)]
pub(crate) struct CountryBundle {
    country: Country,
    name: DisplayName,
    color: MapColor,
    coffer: Coffer,
}

impl CountryBundle {
    fn new(name: &str, color: Color) -> Self {
        CountryBundle {
            country: Country {},
            name: DisplayName(name.to_string()),
            color: MapColor(color),
            coffer: Coffer(0.0),
        }
    }
}
/// Spawns a country entity with the given name and map color.
pub fn spawn_country(commands: &mut Commands, name: &str, color: Color) -> Entity {
    commands.spawn(CountryBundle::new(name, color)).id()
}

/// Setup function for countries.
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
    countries: Query<(Entity, &DisplayName), With<Country>>,
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

pub(crate) fn display_country_panel(
    mut contexts: EguiContexts,
    mut selected_country: ResMut<SelectedCountry>,
    countries: Query<(&DisplayName, &Coffer, &MapColor), With<Country>>,
) {
    let country = match selected_country.get() {
        Some(entity) => entity,
        None => return,
    };

    if let Ok((name, coffer, color)) = countries.get(country) {
        let ctx = match contexts.ctx_mut() {
            Ok(c) => c,
            Err(_) => return,
        };

        let country_frame = egui_common::default_frame();

        egui::Window::new("Country")
            .frame(country_frame)
            .title_bar(false)
            .anchor(egui::Align2::RIGHT_TOP, [-20.0, 20.0])
            .resizable(false)
            .default_width(250.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(
                        RichText::new(&name.0)
                            .font(egui::FontId::proportional(22.0))
                            .color(Color32::WHITE)
                            .strong(),
                    ));
                    ui.add_space(8.0);

                    if egui_common::close_button(ui) {
                        selected_country.clear();
                    }
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(12.0);

                egui::Grid::new("country_stats")
                    .num_columns(2)
                    .spacing([20.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Treasury").color(Color32::LIGHT_GRAY));
                        ui.label(RichText::new(format!("{:.2}g", coffer.0)).color(Color32::GOLD));
                        ui.end_row();

                        ui.label(RichText::new("Map Color").color(Color32::LIGHT_GRAY));
                        let [r, g, b] = color.0.to_srgba().to_f32_array_no_alpha();
                        ui.color_edit_button_rgb(&mut [r, g, b]);
                        ui.end_row();
                    });
            });
    }
}
