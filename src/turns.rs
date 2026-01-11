use crate::buildings::Income;
use crate::country::{Coffer, Country};
use crate::map::{Owner, Province};
use bevy::log::info;
use bevy::prelude::{NextState, Plugin, Query, Res, ResMut, Resource, State, States, With};
use bevy_egui::egui::Align2;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use std::collections::HashMap;

pub struct TurnsPlugin;

impl Plugin for TurnsPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        use bevy::prelude::*;
        app.insert_resource(Turn::default())
            .init_state::<GameState>()
            .add_systems(OnEnter(GameState::Processing), handle_new_turn)
            .add_systems(EguiPrimaryContextPass, display_turn_button);
    }
}

/// Resource for keeping track of current turn. Only cosmetic for now (or forever?).
#[derive(Resource, Default)]
pub(crate) struct Turn {
    current_turn: u32,
}

impl Turn {
    pub(crate) fn advance(&mut self) {
        self.current_turn += 1;
    }
}

/// Different states the game can be in.
#[derive(States, Default, Debug, Hash, PartialEq, Eq, Clone)]
pub(crate) enum GameState {
    #[default]
    /// Player's turn, waiting for input.
    PlayerTurn,
    /// AI turn, updating various systems.
    Processing,
}

/// Handles updating resources, movements of armies (TBD) after the end of each turn.
pub(crate) fn handle_new_turn(
    mut turn: ResMut<Turn>,
    mut next_state: ResMut<NextState<GameState>>,
    incomes: Query<(&Income, &Owner)>,
    mut coffers: Query<(&mut Coffer)>,
) {
    info!("Ending turn {}", turn.current_turn);

    // Those aren't necessarily countries since e.g. rebels can have incomes (but are they owners? IDK).
    // But the owner thing is nice since we can make building have owners and collect income the same
    // way as base income from provinces.
    let mut faction_incomes = HashMap::new();

    // Sum up income for each faction from each source.
    for (income, owner) in incomes.iter() {
        faction_incomes
            .entry(&owner.0)
            .and_modify(|curr_income| *curr_income += income.get())
            .or_insert(income.get());
    }

    for (faction, faction_entity) in faction_incomes.into_iter() {
        if let Ok(mut coffer) = coffers.get_mut(*faction) {
            coffer.add_ducats(faction_entity);
        }
    }

    turn.advance();
    info!("Starting turn {}", turn.current_turn);
    next_state.set(GameState::PlayerTurn);
}

/// Egui system for showing 'End turn' button. Moves the system into [`GameState::Processing`] state.
pub(crate) fn display_turn_button(
    mut contexts: EguiContexts,
    turn: Res<Turn>,
    curr_state: Res<State<GameState>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let ctx = match contexts.ctx_mut() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    let frame = crate::egui_common::default_frame();

    egui::Window::new("Turn")
        .frame(frame)
        .title_bar(false)
        .resizable(false)
        .default_width(150.0)
        .anchor(Align2::LEFT_BOTTOM, [20.0, -20.0])
        .show(ctx, |ui| match curr_state.get() {
            GameState::PlayerTurn => {
                if ui
                    .add(egui::Button::new(format!(
                        "End Turn ({})",
                        turn.current_turn
                    )))
                    .clicked()
                {
                    next_state.set(GameState::Processing);
                }
            }
            GameState::Processing => {
                ui.spinner();
            }
        });
}
