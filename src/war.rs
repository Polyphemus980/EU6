use crate::country::DisplayName;
use crate::egui_common;
use crate::map::{Owner, Province};
use crate::player::Player;
use bevy::prelude::*;
use bevy_egui::egui::{Align2, Color32, RichText};
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use std::collections::HashSet;

pub struct WarPlugin;

impl Plugin for WarPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Wars::default())
            .add_message::<DeclareWarEvent>()
            .add_message::<PeaceOfferEvent>()
            .add_message::<AcceptPeaceEvent>()
            .add_systems(Update, handle_declare_war)
            .add_systems(Update, handle_peace_offers)
            .add_systems(Update, handle_accept_peace)
            .add_systems(Update, ai_handle_peace_offers)
            .add_systems(EguiPrimaryContextPass, display_peace_offers_panel);
    }
}

// ============================================================================
// SIEGE SYSTEM
// ============================================================================

/// System to update siege progress and check for occupation.
pub(crate) fn update_siege_progress(
    mut commands: Commands,
    mut siege_provinces: Query<(Entity, &mut SiegeProgress, &Owner, Option<&Occupied>)>,
    armies: Query<(Entity, &crate::army::HexPos, &Owner), With<crate::army::Army>>,
    provinces: Query<(Entity, &Province, &Owner), Without<Occupied>>,
    province_hex_map: Res<crate::map::ProvinceHexMap>,
    war_relations: Query<&WarRelations>,
) {
    update_existing_sieges(
        &mut commands,
        &mut siege_provinces,
        &armies,
        &province_hex_map,
    );
    check_for_new_sieges(
        &mut commands,
        &armies,
        &provinces,
        &province_hex_map,
        &war_relations,
        &siege_provinces,
    );
}

fn update_existing_sieges(
    commands: &mut Commands,
    siege_provinces: &mut Query<(Entity, &mut SiegeProgress, &Owner, Option<&Occupied>)>,
    armies: &Query<(Entity, &crate::army::HexPos, &Owner), With<crate::army::Army>>,
    province_hex_map: &Res<crate::map::ProvinceHexMap>,
) {
    for (province_entity, mut siege, _, maybe_occupied) in siege_provinces.iter_mut() {
        if maybe_occupied.is_some() {
            commands.entity(province_entity).remove::<SiegeProgress>();
            continue;
        }

        let army_still_present =
            is_besieger_present(province_entity, &siege, armies, province_hex_map);

        if army_still_present {
            advance_siege(commands, province_entity, &mut siege);
        } else {
            lift_siege(commands, province_entity);
        }
    }
}

fn is_besieger_present(
    province_entity: Entity,
    siege: &SiegeProgress,
    armies: &Query<(Entity, &crate::army::HexPos, &Owner), With<crate::army::Army>>,
    province_hex_map: &Res<crate::map::ProvinceHexMap>,
) -> bool {
    armies.iter().any(|(_, pos, owner)| {
        province_hex_map
            .get_entity(&pos.0)
            .map(|&prov| prov == province_entity && owner.0 == siege.besieger_country)
            .unwrap_or(false)
    })
}

fn advance_siege(commands: &mut Commands, province_entity: Entity, siege: &mut SiegeProgress) {
    siege.progress += 1;
    info!(
        "Siege progress on {:?}: {}/{}",
        province_entity, siege.progress, SIEGE_TURNS_REQUIRED
    );

    if siege.progress >= SIEGE_TURNS_REQUIRED {
        commands
            .entity(province_entity)
            .remove::<SiegeProgress>()
            .insert(Occupied {
                occupier: siege.besieger_country,
            });
        info!(
            "Province {:?} occupied by {:?} after siege!",
            province_entity, siege.besieger_country
        );
    }
}

fn lift_siege(commands: &mut Commands, province_entity: Entity) {
    info!("Siege on {:?} lifted - army left", province_entity);
    commands.entity(province_entity).remove::<SiegeProgress>();
}

fn check_for_new_sieges(
    commands: &mut Commands,
    armies: &Query<(Entity, &crate::army::HexPos, &Owner), With<crate::army::Army>>,
    provinces: &Query<(Entity, &Province, &Owner), Without<Occupied>>,
    province_hex_map: &Res<crate::map::ProvinceHexMap>,
    war_relations: &Query<&WarRelations>,
    siege_provinces: &Query<(Entity, &mut SiegeProgress, &Owner, Option<&Occupied>)>,
) {
    for (_, army_pos, army_owner) in armies.iter() {
        if let Some(&province_entity) = province_hex_map.get_entity(&army_pos.0) {
            if siege_provinces.get(province_entity).is_ok() {
                continue;
            }
            try_start_siege(
                commands,
                province_entity,
                army_owner.0,
                provinces,
                war_relations,
            );
        }
    }
}

fn try_start_siege(
    commands: &mut Commands,
    province_entity: Entity,
    army_owner: Entity,
    provinces: &Query<(Entity, &Province, &Owner), Without<Occupied>>,
    war_relations: &Query<&WarRelations>,
) {
    if let Ok((_, province, province_owner)) = provinces.get(province_entity) {
        if are_at_war(army_owner, province_owner.0, war_relations) {
            commands.entity(province_entity).insert(SiegeProgress {
                besieger_country: army_owner,
                progress: 1,
            });
            info!("Siege started on {} by {:?}", province.name(), army_owner);
        }
    }
}

// ============================================================================
// DATA STRUCTURES
// ============================================================================

#[derive(Component)]
pub(crate) struct War {
    pub(crate) attacker: Entity,
    pub(crate) defender: Entity,
}

#[derive(Resource, Default)]
pub(crate) struct Wars {
    pub(crate) active_wars: Vec<Entity>,
}

impl Wars {
    pub(crate) fn add_war(&mut self, war_entity: Entity) {
        self.active_wars.push(war_entity);
    }

    pub(crate) fn remove_war(&mut self, war_entity: Entity) {
        self.active_wars.retain(|&e| e != war_entity);
    }
}

#[derive(Component, Default)]
pub(crate) struct WarRelations {
    pub(crate) at_war_with: HashSet<Entity>,
}

impl WarRelations {
    pub(crate) fn is_at_war_with(&self, other: Entity) -> bool {
        self.at_war_with.contains(&other)
    }

    pub(crate) fn add_enemy(&mut self, enemy: Entity) {
        self.at_war_with.insert(enemy);
    }

    pub(crate) fn remove_enemy(&mut self, enemy: Entity) {
        self.at_war_with.remove(&enemy);
    }
}

#[derive(Component)]
pub(crate) struct Occupied {
    pub(crate) occupier: Entity,
}

#[derive(Component)]
pub(crate) struct SiegeProgress {
    pub(crate) besieger_country: Entity,
    pub(crate) progress: u32,
}

pub(crate) const SIEGE_TURNS_REQUIRED: u32 = 3;

#[derive(Component)]
pub(crate) struct PeaceOffer {
    pub(crate) from: Entity,
    pub(crate) to: Entity,
    pub(crate) war_entity: Entity,
    pub(crate) provinces_to_cede: Vec<Entity>,
}

// ============================================================================
// EVENTS
// ============================================================================

#[derive(Message)]
pub(crate) struct DeclareWarEvent {
    pub(crate) attacker: Entity,
    pub(crate) defender: Entity,
}

impl DeclareWarEvent {
    pub(crate) fn new(attacker: Entity, defender: Entity) -> Self {
        Self { attacker, defender }
    }
}

#[derive(Message)]
pub(crate) struct PeaceOfferEvent {
    pub(crate) from: Entity,
    pub(crate) to: Entity,
    pub(crate) war_entity: Entity,
    pub(crate) provinces_to_cede: Vec<Entity>,
}

#[derive(Message)]
pub(crate) struct AcceptPeaceEvent {
    pub(crate) peace_offer_entity: Entity,
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

pub(crate) fn are_at_war(
    country1: Entity,
    country2: Entity,
    war_relations: &Query<&WarRelations>,
) -> bool {
    war_relations
        .get(country1)
        .map(|r| r.is_at_war_with(country2))
        .unwrap_or(false)
}

pub(crate) fn get_war_between(
    country1: Entity,
    country2: Entity,
    wars: &Res<Wars>,
    war_query: &Query<(Entity, &War)>,
) -> Option<Entity> {
    wars.active_wars.iter().find_map(|&war_entity| {
        war_query.get(war_entity).ok().and_then(|(_, war)| {
            let matches = (war.attacker == country1 && war.defender == country2)
                || (war.attacker == country2 && war.defender == country1);
            matches.then_some(war_entity)
        })
    })
}

pub(crate) fn occupy_province(commands: &mut Commands, province_entity: Entity, occupier: Entity) {
    commands
        .entity(province_entity)
        .insert(Occupied { occupier });
    info!("Province {:?} occupied by {:?}", province_entity, occupier);
}

// ============================================================================
// WAR DECLARATION
// ============================================================================

fn handle_declare_war(
    mut commands: Commands,
    mut events: MessageReader<DeclareWarEvent>,
    mut wars: ResMut<Wars>,
    mut war_relations: Query<&mut WarRelations>,
) {
    for event in events.read() {
        if !validate_war_declaration(&event, &war_relations) {
            continue;
        }
        let war_entity = create_war(&mut commands, &event);
        wars.add_war(war_entity);
        update_war_relations(&mut commands, &mut war_relations, &event);
        info!("War declared: {:?} vs {:?}", event.attacker, event.defender);
    }
}

fn validate_war_declaration(
    event: &DeclareWarEvent,
    war_relations: &Query<&mut WarRelations>,
) -> bool {
    if event.attacker == event.defender {
        warn!("Cannot declare war on yourself!");
        return false;
    }
    if let Ok(relations) = war_relations.get(event.attacker) {
        if relations.is_at_war_with(event.defender) {
            info!(
                "Countries {:?} and {:?} are already at war",
                event.attacker, event.defender
            );
            return false;
        }
    }
    true
}

fn create_war(commands: &mut Commands, event: &DeclareWarEvent) -> Entity {
    commands
        .spawn(War {
            attacker: event.attacker,
            defender: event.defender,
        })
        .id()
}

fn update_war_relations(
    commands: &mut Commands,
    war_relations: &mut Query<&mut WarRelations>,
    event: &DeclareWarEvent,
) {
    add_war_relation(commands, war_relations, event.attacker, event.defender);
    add_war_relation(commands, war_relations, event.defender, event.attacker);
}

fn add_war_relation(
    commands: &mut Commands,
    war_relations: &mut Query<&mut WarRelations>,
    country: Entity,
    enemy: Entity,
) {
    if let Ok(mut relations) = war_relations.get_mut(country) {
        relations.add_enemy(enemy);
    } else {
        let mut relations = WarRelations::default();
        relations.add_enemy(enemy);
        commands.entity(country).insert(relations);
    }
}

// ============================================================================
// PEACE OFFERS
// ============================================================================

fn handle_peace_offers(mut commands: Commands, mut events: MessageReader<PeaceOfferEvent>) {
    for event in events.read() {
        commands.spawn(PeaceOffer {
            from: event.from,
            to: event.to,
            war_entity: event.war_entity,
            provinces_to_cede: event.provinces_to_cede.clone(),
        });
        info!("Peace offer sent from {:?} to {:?}", event.from, event.to);
    }
}

fn ai_handle_peace_offers(
    mut commands: Commands,
    peace_offers: Query<(Entity, &PeaceOffer)>,
    player: Res<Player>,
    mut accept_peace_events: MessageWriter<AcceptPeaceEvent>,
    provinces: Query<&Owner, With<Province>>,
) {
    for (offer_entity, offer) in peace_offers.iter() {
        if Some(offer.to) == player.country {
            continue;
        }
        process_ai_peace_decision(
            &mut commands,
            offer_entity,
            offer,
            &mut accept_peace_events,
            &provinces,
        );
    }
}

fn process_ai_peace_decision(
    commands: &mut Commands,
    offer_entity: Entity,
    offer: &PeaceOffer,
    accept_peace_events: &mut MessageWriter<AcceptPeaceEvent>,
    provinces: &Query<&Owner, With<Province>>,
) {
    if evaluate_peace_offer(offer, provinces) {
        info!(
            "AI country {:?} accepts peace offer from {:?}",
            offer.to, offer.from
        );
        accept_peace_events.write(AcceptPeaceEvent {
            peace_offer_entity: offer_entity,
        });
    } else {
        info!(
            "AI country {:?} rejects peace offer from {:?}",
            offer.to, offer.from
        );
        commands.entity(offer_entity).despawn();
    }
}

fn evaluate_peace_offer(offer: &PeaceOffer, provinces: &Query<&Owner, With<Province>>) -> bool {
    let provinces_demanded = offer.provinces_to_cede.len();
    if provinces_demanded == 0 {
        return true;
    }

    let provinces_from_recipient = count_provinces_from_recipient(offer, provinces);
    if provinces_from_recipient <= 2 {
        return true;
    }

    let total_ai_provinces = provinces.iter().filter(|owner| owner.0 == offer.to).count();
    if total_ai_provinces > 0 {
        let loss_ratio = provinces_from_recipient as f32 / total_ai_provinces as f32;
        return loss_ratio < 0.3;
    }
    false
}

fn count_provinces_from_recipient(
    offer: &PeaceOffer,
    provinces: &Query<&Owner, With<Province>>,
) -> usize {
    offer
        .provinces_to_cede
        .iter()
        .filter(|&&prov| {
            provinces
                .get(prov)
                .map(|owner| owner.0 == offer.to)
                .unwrap_or(false)
        })
        .count()
}

// ============================================================================
// ACCEPT PEACE
// ============================================================================

fn handle_accept_peace(
    mut commands: Commands,
    mut events: MessageReader<AcceptPeaceEvent>,
    mut wars: ResMut<Wars>,
    mut war_relations: Query<&mut WarRelations>,
    peace_offers: Query<&PeaceOffer>,
    war_query: Query<&War>,
    occupied_provinces: Query<(Entity, &Occupied)>,
) {
    for event in events.read() {
        process_peace_acceptance(
            &mut commands,
            event,
            &mut wars,
            &mut war_relations,
            &peace_offers,
            &war_query,
            &occupied_provinces,
        );
    }
}

fn process_peace_acceptance(
    commands: &mut Commands,
    event: &AcceptPeaceEvent,
    wars: &mut ResMut<Wars>,
    war_relations: &mut Query<&mut WarRelations>,
    peace_offers: &Query<&PeaceOffer>,
    war_query: &Query<&War>,
    occupied_provinces: &Query<(Entity, &Occupied)>,
) {
    let Ok(peace_offer) = peace_offers.get(event.peace_offer_entity) else {
        warn!(
            "Peace offer entity not found: {:?}",
            event.peace_offer_entity
        );
        return;
    };

    let Ok(war) = war_query.get(peace_offer.war_entity) else {
        warn!("War entity not found: {:?}", peace_offer.war_entity);
        commands.entity(event.peace_offer_entity).despawn();
        return;
    };

    execute_peace_terms(
        commands,
        peace_offer,
        war,
        wars,
        war_relations,
        occupied_provinces,
    );
    cleanup_peace_entities(
        commands,
        wars,
        peace_offer.war_entity,
        event.peace_offer_entity,
    );
    info!(
        "Peace accepted between {:?} and {:?}",
        war.attacker, war.defender
    );
}

fn execute_peace_terms(
    commands: &mut Commands,
    peace_offer: &PeaceOffer,
    war: &War,
    _wars: &mut ResMut<Wars>,
    war_relations: &mut Query<&mut WarRelations>,
    occupied_provinces: &Query<(Entity, &Occupied)>,
) {
    transfer_provinces(commands, peace_offer);
    clear_occupations(commands, war, occupied_provinces);
    remove_war_relations(war_relations, war);
}

fn transfer_provinces(commands: &mut Commands, peace_offer: &PeaceOffer) {
    for &province_entity in &peace_offer.provinces_to_cede {
        commands
            .entity(province_entity)
            .remove::<Occupied>()
            .insert(Owner(peace_offer.from));
        info!(
            "Province {:?} ceded to {:?}",
            province_entity, peace_offer.from
        );
    }
}

fn clear_occupations(
    commands: &mut Commands,
    war: &War,
    occupied_provinces: &Query<(Entity, &Occupied)>,
) {
    for (province_entity, occupied) in occupied_provinces.iter() {
        if occupied.occupier == war.attacker || occupied.occupier == war.defender {
            commands.entity(province_entity).remove::<Occupied>();
        }
    }
}

fn remove_war_relations(war_relations: &mut Query<&mut WarRelations>, war: &War) {
    if let Ok(mut relations) = war_relations.get_mut(war.attacker) {
        relations.remove_enemy(war.defender);
    }
    if let Ok(mut relations) = war_relations.get_mut(war.defender) {
        relations.remove_enemy(war.attacker);
    }
}

fn cleanup_peace_entities(
    commands: &mut Commands,
    wars: &mut ResMut<Wars>,
    war_entity: Entity,
    offer_entity: Entity,
) {
    wars.remove_war(war_entity);
    commands.entity(war_entity).despawn();
    commands.entity(offer_entity).despawn();
}

// ============================================================================
// UI - PEACE OFFERS PANEL
// ============================================================================

pub(crate) fn display_peace_offers_panel(
    mut contexts: EguiContexts,
    player: Res<Player>,
    peace_offers: Query<(Entity, &PeaceOffer)>,
    countries: Query<&DisplayName>,
    provinces: Query<&Province>,
    mut accept_peace_events: MessageWriter<AcceptPeaceEvent>,
    mut commands: Commands,
) {
    let Some(player_country) = player.country else {
        return;
    };
    let player_offers: Vec<_> = peace_offers
        .iter()
        .filter(|(_, offer)| offer.to == player_country)
        .collect();

    if player_offers.is_empty() {
        return;
    }

    let ctx = match contexts.ctx_mut() {
        Ok(c) => c,
        Err(_) => return,
    };

    render_peace_offers_window(
        ctx,
        &player_offers,
        &countries,
        &provinces,
        &mut accept_peace_events,
        &mut commands,
    );
}

fn render_peace_offers_window(
    ctx: &egui::Context,
    player_offers: &[(Entity, &PeaceOffer)],
    countries: &Query<&DisplayName>,
    provinces: &Query<&Province>,
    accept_peace_events: &mut MessageWriter<AcceptPeaceEvent>,
    commands: &mut Commands,
) {
    egui::Window::new("Peace Offers")
        .frame(egui_common::default_frame())
        .title_bar(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .resizable(false)
        .default_width(350.0)
        .show(ctx, |ui| {
            ui.heading("☮ Peace Offer");
            ui.separator();
            for &(offer_entity, offer) in player_offers {
                render_single_peace_offer(
                    ui,
                    offer_entity,
                    offer,
                    countries,
                    provinces,
                    accept_peace_events,
                    commands,
                );
            }
        });
}

fn render_single_peace_offer(
    ui: &mut egui::Ui,
    offer_entity: Entity,
    offer: &PeaceOffer,
    countries: &Query<&DisplayName>,
    provinces: &Query<&Province>,
    accept_peace_events: &mut MessageWriter<AcceptPeaceEvent>,
    commands: &mut Commands,
) {
    let from_name = countries
        .get(offer.from)
        .map(|n| n.0.as_str())
        .unwrap_or("Unknown");
    ui.label(format!("{} offers peace:", from_name));
    ui.add_space(8.0);

    render_peace_terms(ui, offer, provinces);
    render_peace_buttons(ui, offer_entity, accept_peace_events, commands);
    ui.separator();
}

fn render_peace_terms(ui: &mut egui::Ui, offer: &PeaceOffer, provinces: &Query<&Province>) {
    if offer.provinces_to_cede.is_empty() {
        ui.label(RichText::new("White Peace").color(Color32::YELLOW));
        ui.label("No territorial changes.");
    } else {
        ui.label(RichText::new("Demands:").color(Color32::RED));
        for &province_entity in &offer.provinces_to_cede {
            if let Ok(province) = provinces.get(province_entity) {
                ui.label(format!("  • {}", province.name()));
            }
        }
    }
    ui.add_space(12.0);
}

fn render_peace_buttons(
    ui: &mut egui::Ui,
    offer_entity: Entity,
    accept_peace_events: &mut MessageWriter<AcceptPeaceEvent>,
    commands: &mut Commands,
) {
    ui.horizontal(|ui| {
        if ui.button("✓ Accept").clicked() {
            accept_peace_events.write(AcceptPeaceEvent {
                peace_offer_entity: offer_entity,
            });
        }
        if ui.button("✗ Decline").clicked() {
            commands.entity(offer_entity).despawn();
        }
    });
}

// ============================================================================
// UI - DIPLOMACY TAB
// ============================================================================

pub(crate) fn draw_diplomacy_tab(
    ui: &mut egui::Ui,
    player_country: Entity,
    target_country: Entity,
    war_relations: &Query<&WarRelations>,
    wars: &Res<Wars>,
    war_query: &Query<(Entity, &War)>,
    declare_war_events: &mut MessageWriter<DeclareWarEvent>,
    peace_offer_events: &mut MessageWriter<PeaceOfferEvent>,
    provinces: &Query<(Entity, &Province, &Owner, Option<&Occupied>)>,
    selected_provinces: &mut HashSet<Entity>,
) {
    let is_at_war = war_relations
        .get(player_country)
        .map(|r| r.is_at_war_with(target_country))
        .unwrap_or(false);

    if is_at_war {
        draw_war_diplomacy(
            ui,
            player_country,
            target_country,
            wars,
            war_query,
            peace_offer_events,
            provinces,
            selected_provinces,
        );
    } else {
        draw_peace_diplomacy(ui, player_country, target_country, declare_war_events);
    }
}

fn draw_war_diplomacy(
    ui: &mut egui::Ui,
    player_country: Entity,
    target_country: Entity,
    wars: &Res<Wars>,
    war_query: &Query<(Entity, &War)>,
    peace_offer_events: &mut MessageWriter<PeaceOfferEvent>,
    provinces: &Query<(Entity, &Province, &Owner, Option<&Occupied>)>,
    selected_provinces: &mut HashSet<Entity>,
) {
    ui.label(RichText::new("⚔ AT WAR").color(Color32::RED).strong());
    ui.add_space(8.0);

    let our_occupied = get_occupied_by(provinces, target_country, player_country);
    let their_occupied = get_occupied_by(provinces, player_country, target_country);

    draw_occupied_list(
        ui,
        "We occupy:",
        Color32::GREEN,
        &our_occupied,
        selected_provinces,
        true,
    );
    draw_occupied_list(
        ui,
        "They occupy:",
        Color32::RED,
        &their_occupied,
        &mut HashSet::new(),
        false,
    );

    ui.separator();
    draw_peace_offer_section(
        ui,
        player_country,
        target_country,
        wars,
        war_query,
        peace_offer_events,
        selected_provinces,
    );
}

fn get_occupied_by(
    provinces: &Query<(Entity, &Province, &Owner, Option<&Occupied>)>,
    owner: Entity,
    occupier: Entity,
) -> Vec<(Entity, String)> {
    provinces
        .iter()
        .filter(|(_, _, o, occ)| {
            o.0 == owner && occ.map(|x| x.occupier == occupier).unwrap_or(false)
        })
        .map(|(e, p, _, _)| (e, p.name().to_string()))
        .collect()
}

fn draw_occupied_list(
    ui: &mut egui::Ui,
    label: &str,
    color: Color32,
    occupied: &[(Entity, String)],
    selected: &mut HashSet<Entity>,
    selectable: bool,
) {
    if occupied.is_empty() {
        return;
    }

    ui.label(RichText::new(label).color(color));
    for (entity, name) in occupied {
        if selectable {
            let is_selected = selected.contains(entity);
            if ui
                .selectable_label(is_selected, format!("  • {}", name))
                .clicked()
            {
                if is_selected {
                    selected.remove(entity);
                } else {
                    selected.insert(*entity);
                }
            }
        } else {
            ui.label(format!("  • {}", name));
        }
    }
    ui.add_space(4.0);
}

fn draw_peace_offer_section(
    ui: &mut egui::Ui,
    player_country: Entity,
    target_country: Entity,
    wars: &Res<Wars>,
    war_query: &Query<(Entity, &War)>,
    peace_offer_events: &mut MessageWriter<PeaceOfferEvent>,
    selected_provinces: &mut HashSet<Entity>,
) {
    ui.label(RichText::new("Peace Terms:").strong());

    if selected_provinces.is_empty() {
        ui.label("White peace (select provinces above to demand them)");
    } else {
        ui.label(format!(
            "Demanding {} province(s)",
            selected_provinces.len()
        ));
    }

    ui.add_space(8.0);

    if ui.button("📜 Offer Peace").clicked() {
        if let Some(war_entity) = get_war_between(player_country, target_country, wars, war_query) {
            peace_offer_events.write(PeaceOfferEvent {
                from: player_country,
                to: target_country,
                war_entity,
                provinces_to_cede: selected_provinces.iter().copied().collect(),
            });
            selected_provinces.clear();
        }
    }
}

fn draw_peace_diplomacy(
    ui: &mut egui::Ui,
    player_country: Entity,
    target_country: Entity,
    declare_war_events: &mut MessageWriter<DeclareWarEvent>,
) {
    ui.label(RichText::new("☮ AT PEACE").color(Color32::GREEN).strong());
    ui.add_space(16.0);

    if ui.button("⚔ Declare War").clicked() {
        declare_war_events.write(DeclareWarEvent::new(player_country, target_country));
    }
}
