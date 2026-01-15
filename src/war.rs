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

/// System to update siege progress and check for occupation.
/// Should be called each turn.
pub(crate) fn update_siege_progress(
    mut commands: Commands,
    mut siege_provinces: Query<(Entity, &mut SiegeProgress, &Owner, Option<&Occupied>)>,
    armies: Query<(Entity, &crate::army::HexPos, &Owner), With<crate::army::Army>>,
    provinces: Query<(Entity, &Province, &Owner), Without<Occupied>>,
    province_hex_map: Res<crate::map::ProvinceHexMap>,
    war_relations: Query<&WarRelations>,
) {
    // First, update existing sieges
    for (province_entity, mut siege, _province_owner, maybe_occupied) in siege_provinces.iter_mut()
    {
        // Skip if already occupied
        if maybe_occupied.is_some() {
            commands.entity(province_entity).remove::<SiegeProgress>();
            continue;
        }

        // Check if besieger army is still on this province
        let army_still_present = armies.iter().any(|(_, pos, owner)| {
            if let Some(&prov_entity) = province_hex_map.get_entity(&pos.0) {
                prov_entity == province_entity && owner.0 == siege.besieger_country
            } else {
                false
            }
        });

        if army_still_present {
            siege.progress += 1;
            info!(
                "Siege progress on {:?}: {}/{}",
                province_entity, siege.progress, SIEGE_TURNS_REQUIRED
            );

            if siege.progress >= SIEGE_TURNS_REQUIRED {
                // Province is now occupied!
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
        } else {
            // Army left, siege is lifted
            info!("Siege on {:?} lifted - army left", province_entity);
            commands.entity(province_entity).remove::<SiegeProgress>();
        }
    }

    // Check for new sieges - armies standing on enemy provinces
    for (army_entity, army_pos, army_owner) in armies.iter() {
        if let Some(&province_entity) = province_hex_map.get_entity(&army_pos.0) {
            // Skip if province already has siege or is occupied
            if siege_provinces.get(province_entity).is_ok() {
                continue;
            }

            if let Ok((_, province, province_owner)) = provinces.get(province_entity) {
                // Check if at war with province owner
                if are_at_war(army_owner.0, province_owner.0, &war_relations) {
                    // Start siege
                    commands.entity(province_entity).insert(SiegeProgress {
                        besieger: army_entity,
                        besieger_country: army_owner.0,
                        progress: 1, // First turn counts
                    });
                    info!("Siege started on {} by {:?}", province.name(), army_owner.0);
                }
            }
        }
    }
}

/// Component representing an active war between two countries
#[derive(Component)]
pub(crate) struct War {
    pub(crate) attacker: Entity,
    pub(crate) defender: Entity,
    pub(crate) started_turn: u32,
}

/// Resource tracking all active wars
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

/// Component added to countries to track their war relations
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

/// Component marking a province as occupied by another country during war.
/// The original owner remains in Owner component, but the occupier controls it.
#[derive(Component)]
pub(crate) struct Occupied {
    pub(crate) occupier: Entity,
}

/// Component tracking siege progress on a province.
/// When progress reaches threshold, the province becomes occupied.
#[derive(Component)]
pub(crate) struct SiegeProgress {
    pub(crate) besieger: Entity,
    pub(crate) besieger_country: Entity,
    pub(crate) progress: u32,
}

/// Number of turns required to occupy a province
pub(crate) const SIEGE_TURNS_REQUIRED: u32 = 3;

/// Component for pending peace offers
#[derive(Component)]
pub(crate) struct PeaceOffer {
    pub(crate) from: Entity,
    pub(crate) to: Entity,
    pub(crate) war_entity: Entity,
    /// Provinces that will change ownership (from loser to winner)
    pub(crate) provinces_to_cede: Vec<Entity>,
}

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

/// Check if two countries are at war
pub(crate) fn are_at_war(
    country1: Entity,
    country2: Entity,
    war_relations: &Query<&WarRelations>,
) -> bool {
    if let Ok(relations) = war_relations.get(country1) {
        relations.is_at_war_with(country2)
    } else {
        false
    }
}

/// Get the war entity between two countries
pub(crate) fn get_war_between(
    country1: Entity,
    country2: Entity,
    wars: &Res<Wars>,
    war_query: &Query<(Entity, &War)>,
) -> Option<Entity> {
    for &war_entity in &wars.active_wars {
        if let Ok((_, war)) = war_query.get(war_entity) {
            if (war.attacker == country1 && war.defender == country2)
                || (war.attacker == country2 && war.defender == country1)
            {
                return Some(war_entity);
            }
        }
    }
    None
}

/// Occupy a province (called when winning a battle on enemy territory)
pub(crate) fn occupy_province(commands: &mut Commands, province_entity: Entity, occupier: Entity) {
    commands
        .entity(province_entity)
        .insert(Occupied { occupier });
    info!("Province {:?} occupied by {:?}", province_entity, occupier);
}

/// Get all provinces occupied by a country from an enemy
pub(crate) fn get_occupied_provinces(
    occupier: Entity,
    enemy: Entity,
    provinces: &Query<(Entity, &Owner, Option<&Occupied>), With<Province>>,
) -> Vec<Entity> {
    provinces
        .iter()
        .filter(|(_, owner, occupied)| {
            owner.0 == enemy && occupied.map(|o| o.occupier == occupier).unwrap_or(false)
        })
        .map(|(e, _, _)| e)
        .collect()
}

fn handle_declare_war(
    mut commands: Commands,
    mut events: MessageReader<DeclareWarEvent>,
    mut wars: ResMut<Wars>,
    mut war_relations: Query<&mut WarRelations>,
    turn: Res<crate::turns::Turn>,
) {
    for event in events.read() {
        // Cannot declare war on yourself
        if event.attacker == event.defender {
            warn!("Cannot declare war on yourself!");
            continue;
        }

        // Check if already at war
        if let Ok(relations) = war_relations.get(event.attacker) {
            if relations.is_at_war_with(event.defender) {
                info!(
                    "Countries {:?} and {:?} are already at war",
                    event.attacker, event.defender
                );
                continue;
            }
        }

        // Create war entity
        let war_entity = commands
            .spawn(War {
                attacker: event.attacker,
                defender: event.defender,
                started_turn: turn.current_turn(),
            })
            .id();

        wars.add_war(war_entity);

        // Update war relations for attacker
        if let Ok(mut relations) = war_relations.get_mut(event.attacker) {
            relations.add_enemy(event.defender);
        } else {
            let mut relations = WarRelations::default();
            relations.add_enemy(event.defender);
            commands.entity(event.attacker).insert(relations);
        }

        // Update war relations for defender
        if let Ok(mut relations) = war_relations.get_mut(event.defender) {
            relations.add_enemy(event.attacker);
        } else {
            let mut relations = WarRelations::default();
            relations.add_enemy(event.attacker);
            commands.entity(event.defender).insert(relations);
        }

        info!("War declared: {:?} vs {:?}", event.attacker, event.defender);
    }
}

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

/// AI system to automatically handle peace offers for non-player countries.
/// AI will accept white peace or peace offers where they don't lose too much.
fn ai_handle_peace_offers(
    mut commands: Commands,
    peace_offers: Query<(Entity, &PeaceOffer)>,
    player: Res<Player>,
    mut accept_peace_events: MessageWriter<AcceptPeaceEvent>,
    provinces: Query<&Owner, With<Province>>,
    war_query: Query<&War>,
) {
    for (offer_entity, offer) in peace_offers.iter() {
        // Skip offers to player - they handle it via UI
        if Some(offer.to) == player.country {
            continue;
        }

        // AI decision logic
        let should_accept = evaluate_peace_offer(offer, &provinces, &war_query);

        if should_accept {
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
            // Reject by despawning the offer
            commands.entity(offer_entity).despawn();
        }
    }
}

/// Evaluate whether AI should accept a peace offer.
/// Returns true if AI should accept.
fn evaluate_peace_offer(
    offer: &PeaceOffer,
    provinces: &Query<&Owner, With<Province>>,
    _war_query: &Query<&War>,
) -> bool {
    // Count how many provinces each side would lose
    let provinces_demanded = offer.provinces_to_cede.len();

    // White peace - always accept
    if provinces_demanded == 0 {
        return true;
    }

    // Check if the demanded provinces actually belong to the recipient (offer.to)
    let provinces_from_recipient: usize = offer
        .provinces_to_cede
        .iter()
        .filter(|&&prov| {
            provinces
                .get(prov)
                .map(|owner| owner.0 == offer.to)
                .unwrap_or(false)
        })
        .count();

    // If demanding our provinces, be more reluctant
    // Accept if demanding 2 or fewer of our provinces
    if provinces_from_recipient <= 2 {
        return true;
    }

    // Count total provinces owned by the AI
    let total_ai_provinces = provinces.iter().filter(|owner| owner.0 == offer.to).count();

    // Accept if losing less than 30% of total provinces
    if total_ai_provinces > 0 {
        let loss_ratio = provinces_from_recipient as f32 / total_ai_provinces as f32;
        if loss_ratio < 0.3 {
            return true;
        }
    }

    false
}

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
        let Ok(peace_offer) = peace_offers.get(event.peace_offer_entity) else {
            warn!(
                "Peace offer entity not found: {:?}",
                event.peace_offer_entity
            );
            continue;
        };

        let Ok(war) = war_query.get(peace_offer.war_entity) else {
            warn!("War entity not found: {:?}", peace_offer.war_entity);
            // Clean up the peace offer anyway
            commands.entity(event.peace_offer_entity).despawn();
            continue;
        };

        // Transfer provinces that are being ceded
        for &province_entity in &peace_offer.provinces_to_cede {
            // Remove occupation and change owner
            commands
                .entity(province_entity)
                .remove::<Occupied>()
                .insert(Owner(peace_offer.from)); // provinces go to the one who demanded them (the 'from' in peace offer)
            info!(
                "Province {:?} ceded to {:?}",
                province_entity, peace_offer.from
            );
        }

        // Remove ALL occupations between these two countries
        for (province_entity, occupied) in occupied_provinces.iter() {
            if occupied.occupier == war.attacker || occupied.occupier == war.defender {
                commands.entity(province_entity).remove::<Occupied>();
            }
        }

        // Remove war relations
        if let Ok(mut relations) = war_relations.get_mut(war.attacker) {
            relations.remove_enemy(war.defender);
        }
        if let Ok(mut relations) = war_relations.get_mut(war.defender) {
            relations.remove_enemy(war.attacker);
        }

        // Remove war entity
        wars.remove_war(peace_offer.war_entity);
        commands.entity(peace_offer.war_entity).despawn();

        // Remove peace offer entity
        commands.entity(event.peace_offer_entity).despawn();

        info!(
            "Peace accepted between {:?} and {:?}",
            war.attacker, war.defender
        );
    }
}

/// UI for displaying incoming peace offers (popup)
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

    // Check if there are any peace offers for the player
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

    let frame = egui_common::default_frame();

    egui::Window::new("Peace Offers")
        .frame(frame)
        .title_bar(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .resizable(false)
        .default_width(350.0)
        .show(ctx, |ui| {
            ui.heading("☮ Peace Offer");
            ui.separator();

            for (offer_entity, offer) in player_offers {
                let from_name = countries
                    .get(offer.from)
                    .map(|n| n.0.as_str())
                    .unwrap_or("Unknown");

                ui.label(format!("{} offers peace:", from_name));
                ui.add_space(8.0);

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

                ui.separator();
            }
        });
}

/// Diplomacy tab content - to be called from country panel
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
    let player_relations = war_relations.get(player_country).ok();
    let is_at_war = player_relations
        .map(|r| r.is_at_war_with(target_country))
        .unwrap_or(false);

    if is_at_war {
        ui.label(RichText::new("⚔ AT WAR").color(Color32::RED).strong());
        ui.add_space(8.0);

        // Show occupied provinces
        let our_occupied: Vec<_> = provinces
            .iter()
            .filter(|(_, _, owner, occupied)| {
                owner.0 == target_country
                    && occupied
                        .map(|o| o.occupier == player_country)
                        .unwrap_or(false)
            })
            .collect();

        let their_occupied: Vec<_> = provinces
            .iter()
            .filter(|(_, _, owner, occupied)| {
                owner.0 == player_country
                    && occupied
                        .map(|o| o.occupier == target_country)
                        .unwrap_or(false)
            })
            .collect();

        if !our_occupied.is_empty() {
            ui.label(RichText::new("We occupy:").color(Color32::GREEN));
            for (entity, province, _, _) in &our_occupied {
                let is_selected = selected_provinces.contains(entity);
                if ui
                    .selectable_label(is_selected, format!("  • {}", province.name()))
                    .clicked()
                {
                    if is_selected {
                        selected_provinces.remove(entity);
                    } else {
                        selected_provinces.insert(*entity);
                    }
                }
            }
            ui.add_space(4.0);
        }

        if !their_occupied.is_empty() {
            ui.label(RichText::new("They occupy:").color(Color32::RED));
            for (_, province, _, _) in &their_occupied {
                ui.label(format!("  • {}", province.name()));
            }
            ui.add_space(4.0);
        }

        ui.separator();

        // Peace offer section
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
            if let Some(war_entity) =
                get_war_between(player_country, target_country, wars, war_query)
            {
                peace_offer_events.write(PeaceOfferEvent {
                    from: player_country,
                    to: target_country,
                    war_entity,
                    provinces_to_cede: selected_provinces.iter().copied().collect(),
                });
                selected_provinces.clear();
            }
        }
    } else {
        ui.label(RichText::new("☮ AT PEACE").color(Color32::GREEN).strong());
        ui.add_space(16.0);

        if ui.button("⚔ Declare War").clicked() {
            declare_war_events.write(DeclareWarEvent::new(player_country, target_country));
        }
    }
}
