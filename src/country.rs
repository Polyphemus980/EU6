use crate::egui_common;
use crate::map::{MapData, Owner, Province};
use crate::menu::MenuState;
use crate::player::Player;
use crate::war::{
    draw_diplomacy_tab, DeclareWarEvent, Occupied, PeaceOfferEvent, War, WarRelations, Wars,
};
use bevy::prelude::*;
use bevy_egui::egui::{Color32, RichText, TextureId};
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass, EguiTextureHandle};
use std::collections::{HashMap, HashSet};

pub struct CountryPlugin;

impl Plugin for CountryPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SelectedCountry::default())
            .insert_resource(CountryFlags::default())
            .add_systems(
                Startup,
                setup_countries_from_map.after(crate::map::generate_map),
            )
            .add_systems(
                Startup,
                assign_province_ownership
                    .after(crate::map::generate_map)
                    .after(setup_countries_from_map),
            )
            .add_systems(
                EguiPrimaryContextPass,
                display_country_panel.run_if(in_state(MenuState::InGame)),
            );
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

/// Component storing the flag texture handle for a country
#[derive(Component)]
pub(crate) struct Flag(pub(crate) Handle<Image>);

/// Resource storing egui texture IDs for country flags
#[derive(Resource, Default)]
pub(crate) struct CountryFlags {
    pub(crate) textures: HashMap<Entity, TextureId>,
}

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

    pub(crate) fn get_ducats(&self) -> f32 {
        self.0
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

/// Setup countries from map data - creates country entities based on what's in the map file
pub(crate) fn setup_countries_from_map(
    mut commands: Commands,
    map_data: Res<MapData>,
    asset_server: Res<AssetServer>,
) {
    info!(
        "Setting up {} countries from map data",
        map_data.countries.len()
    );

    for country_def in &map_data.countries {
        let color = Color::srgb(
            country_def.color[0],
            country_def.color[1],
            country_def.color[2],
        );

        // Load flag texture
        let flag_handle: Handle<Image> = asset_server.load(&country_def.flag);

        let entity = commands
            .spawn(CountryBundle::new(&country_def.name, color))
            .insert(Flag(flag_handle))
            .id();

        info!(
            "Created country: {} ({:?}) with flag: {}",
            country_def.name, entity, country_def.flag
        );
    }
}

/// System to assign province ownership to countries based on map data.
/// This runs after both countries and provinces have been spawned.
pub(crate) fn assign_province_ownership(
    mut commands: Commands,
    provinces: Query<(Entity, &Province)>,
    countries: Query<(Entity, &DisplayName), With<Country>>,
    map_data: Res<MapData>,
) {
    // Create a lookup from country name to entity
    let country_lookup: HashMap<&str, Entity> = countries
        .iter()
        .map(|(entity, name)| (name.0.as_str(), entity))
        .collect();

    if country_lookup.is_empty() {
        warn!("No countries found for province assignment!");
        return;
    }

    // Assign provinces based on map data
    for (province_entity, province) in provinces.iter() {
        let hex = province.get_hex();

        // Look up the owner from map data
        if let Some(owner_name) = map_data.province_owners.get(hex) {
            if let Some(&owner_entity) = country_lookup.get(owner_name.as_str()) {
                commands.entity(province_entity).insert(Owner(owner_entity));
            } else {
                warn!(
                    "Unknown country '{}' for province '{}'",
                    owner_name,
                    province.name()
                );
            }
        }
        // If no owner in map data, province stays unowned
    }

    info!("Province ownership assigned from map data");
}

/// Enum for country panel tabs
#[derive(Default, PartialEq, Clone, Copy)]
pub(crate) enum CountryTab {
    #[default]
    Info,
    Diplomacy,
}

pub(crate) fn display_country_panel(
    mut contexts: EguiContexts,
    mut selected_country: ResMut<SelectedCountry>,
    countries: Query<(Entity, &DisplayName, &Coffer, &MapColor, Option<&Flag>), With<Country>>,
    player: Res<Player>,
    war_relations: Query<&WarRelations>,
    wars: Res<Wars>,
    war_query: Query<(Entity, &War)>,
    mut declare_war_events: MessageWriter<DeclareWarEvent>,
    mut peace_offer_events: MessageWriter<PeaceOfferEvent>,
    provinces: Query<(Entity, &Province, &Owner, Option<&Occupied>)>,
    mut current_tab: Local<CountryTab>,
    mut selected_provinces_for_peace: Local<HashSet<Entity>>,
    mut country_flags: ResMut<CountryFlags>,
    images: Res<Assets<Image>>,
) {
    let Some(country) = selected_country.get() else {
        selected_provinces_for_peace.clear();
        return;
    };

    let Ok((country_entity, name, coffer, color, maybe_flag)) = countries.get(country) else {
        return;
    };

    let is_player = Some(country) == player.country;
    let player_country = player.country;
    let flag_texture_id = get_flag_texture(
        &mut contexts,
        &mut country_flags,
        &images,
        country_entity,
        maybe_flag,
    );

    let ctx = match contexts.ctx_mut() {
        Ok(c) => c,
        Err(_) => return,
    };

    render_country_window(
        ctx,
        &name.0,
        coffer,
        color,
        is_player,
        player_country,
        country_entity,
        flag_texture_id,
        &mut selected_country,
        &mut selected_provinces_for_peace,
        &mut current_tab,
        &war_relations,
        &wars,
        &war_query,
        &mut declare_war_events,
        &mut peace_offer_events,
        &provinces,
    );
}

fn get_flag_texture(
    contexts: &mut EguiContexts,
    country_flags: &mut ResMut<CountryFlags>,
    images: &Res<Assets<Image>>,
    country_entity: Entity,
    maybe_flag: Option<&Flag>,
) -> Option<TextureId> {
    let flag = maybe_flag?;

    if let Some(&texture_id) = country_flags.textures.get(&country_entity) {
        return Some(texture_id);
    }

    if images.get(&flag.0).is_some() {
        let texture_id = contexts.add_image(EguiTextureHandle::Strong(flag.0.clone()));
        country_flags.textures.insert(country_entity, texture_id);
        return Some(texture_id);
    }

    None
}

fn render_country_window(
    ctx: &egui::Context,
    name: &str,
    coffer: &Coffer,
    color: &MapColor,
    is_player: bool,
    player_country: Option<Entity>,
    country_entity: Entity,
    flag_texture_id: Option<TextureId>,
    selected_country: &mut ResMut<SelectedCountry>,
    selected_provinces_for_peace: &mut Local<HashSet<Entity>>,
    current_tab: &mut Local<CountryTab>,
    war_relations: &Query<&WarRelations>,
    wars: &Res<Wars>,
    war_query: &Query<(Entity, &War)>,
    declare_war_events: &mut MessageWriter<DeclareWarEvent>,
    peace_offer_events: &mut MessageWriter<PeaceOfferEvent>,
    provinces: &Query<(Entity, &Province, &Owner, Option<&Occupied>)>,
) {
    egui::Window::new("Country")
        .frame(egui_common::default_frame())
        .title_bar(false)
        .anchor(egui::Align2::RIGHT_TOP, [-20.0, 20.0])
        .resizable(false)
        .default_width(280.0)
        .show(ctx, |ui| {
            render_country_header(
                ui,
                name,
                is_player,
                flag_texture_id,
                selected_country,
                selected_provinces_for_peace,
            );
            render_country_tabs(ui, current_tab, is_player, player_country.is_some());
            render_country_content(
                ui,
                coffer,
                color,
                is_player,
                player_country,
                country_entity,
                current_tab,
                war_relations,
                wars,
                war_query,
                declare_war_events,
                peace_offer_events,
                provinces,
                selected_provinces_for_peace,
            );
        });
}

fn render_country_header(
    ui: &mut egui::Ui,
    name: &str,
    is_player: bool,
    flag_texture_id: Option<TextureId>,
    selected_country: &mut ResMut<SelectedCountry>,
    selected_provinces_for_peace: &mut Local<HashSet<Entity>>,
) {
    ui.horizontal(|ui| {
        if let Some(texture_id) = flag_texture_id {
            ui.add(egui::Image::new(egui::load::SizedTexture::new(
                texture_id,
                egui::vec2(48.0, 32.0),
            )));
            ui.add_space(8.0);
        }

        ui.add(egui::Label::new(
            RichText::new(name)
                .font(egui::FontId::proportional(22.0))
                .color(Color32::WHITE)
                .strong(),
        ));

        if is_player {
            ui.add(egui::Label::new(
                RichText::new("(You)").color(Color32::GREEN).italics(),
            ));
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if egui_common::close_button(ui) {
                selected_country.clear();
                selected_provinces_for_peace.clear();
            }
        });
    });
    ui.add_space(8.0);
    ui.separator();
}

fn render_country_tabs(
    ui: &mut egui::Ui,
    current_tab: &mut Local<CountryTab>,
    is_player: bool,
    has_player: bool,
) {
    let show_diplomacy = !is_player && has_player;

    ui.horizontal(|ui| {
        if ui
            .selectable_label(**current_tab == CountryTab::Info, "📊 Info")
            .clicked()
        {
            **current_tab = CountryTab::Info;
        }
        if show_diplomacy
            && ui
                .selectable_label(**current_tab == CountryTab::Diplomacy, "⚔ Diplomacy")
                .clicked()
        {
            **current_tab = CountryTab::Diplomacy;
        }
    });
    ui.separator();
    ui.add_space(8.0);
}

fn render_country_content(
    ui: &mut egui::Ui,
    coffer: &Coffer,
    color: &MapColor,
    _is_player: bool,
    player_country: Option<Entity>,
    country_entity: Entity,
    current_tab: &mut Local<CountryTab>,
    war_relations: &Query<&WarRelations>,
    wars: &Res<Wars>,
    war_query: &Query<(Entity, &War)>,
    declare_war_events: &mut MessageWriter<DeclareWarEvent>,
    peace_offer_events: &mut MessageWriter<PeaceOfferEvent>,
    provinces: &Query<(Entity, &Province, &Owner, Option<&Occupied>)>,
    selected_provinces_for_peace: &mut Local<HashSet<Entity>>,
) {
    match **current_tab {
        CountryTab::Info => render_info_tab(ui, coffer, color),
        CountryTab::Diplomacy => {
            if let Some(player_country) = player_country {
                draw_diplomacy_tab(
                    ui,
                    player_country,
                    country_entity,
                    war_relations,
                    wars,
                    war_query,
                    declare_war_events,
                    peace_offer_events,
                    provinces,
                    selected_provinces_for_peace,
                );
            }
        }
    }
}

fn render_info_tab(ui: &mut egui::Ui, coffer: &Coffer, color: &MapColor) {
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
}
