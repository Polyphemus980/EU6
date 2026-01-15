use crate::country::{Country, DisplayName, MapColor};
use crate::player::Player;
use bevy::prelude::*;
use bevy_egui::egui::{Color32, RichText};
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<MenuState>()
            .add_systems(
                EguiPrimaryContextPass,
                display_main_menu.run_if(in_state(MenuState::MainMenu)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                display_country_selection.run_if(in_state(MenuState::CountrySelection)),
            )
            .add_systems(OnEnter(MenuState::InGame), hide_menu);
    }
}

/// Game menu state
#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum MenuState {
    #[default]
    MainMenu,
    CountrySelection,
    InGame,
}

fn display_main_menu(mut contexts: EguiContexts, mut next_state: ResMut<NextState<MenuState>>) {
    let ctx = match contexts.ctx_mut() {
        Ok(c) => c,
        Err(_) => return,
    };

    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(Color32::from_rgb(10, 10, 20)))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);

                ui.label(
                    RichText::new("EU6")
                        .font(egui::FontId::proportional(72.0))
                        .color(Color32::GOLD)
                        .strong(),
                );

                ui.add_space(20.0);

                ui.label(
                    RichText::new("A Grand Strategy Game")
                        .font(egui::FontId::proportional(24.0))
                        .color(Color32::LIGHT_GRAY)
                        .italics(),
                );

                ui.add_space(80.0);

                let button_size = egui::vec2(250.0, 50.0);

                if ui
                    .add_sized(
                        button_size,
                        egui::Button::new(
                            RichText::new("🎮 New Game")
                                .font(egui::FontId::proportional(24.0))
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(60, 80, 120)),
                    )
                    .clicked()
                {
                    next_state.set(MenuState::CountrySelection);
                }

                ui.add_space(20.0);

                if ui
                    .add_sized(
                        button_size,
                        egui::Button::new(
                            RichText::new("❌ Quit")
                                .font(egui::FontId::proportional(24.0))
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(120, 60, 60)),
                    )
                    .clicked()
                {
                    std::process::exit(0);
                }
            });
        });
}

fn display_country_selection(
    mut contexts: EguiContexts,
    mut next_state: ResMut<NextState<MenuState>>,
    countries: Query<(Entity, &DisplayName, &MapColor), With<Country>>,
    mut player: ResMut<Player>,
) {
    let ctx = match contexts.ctx_mut() {
        Ok(c) => c,
        Err(_) => return,
    };

    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(Color32::from_rgb(10, 10, 20)))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);

                ui.label(
                    RichText::new("Select Your Country")
                        .font(egui::FontId::proportional(48.0))
                        .color(Color32::WHITE)
                        .strong(),
                );

                ui.add_space(40.0);

                // Grid of countries
                let countries_vec: Vec<_> = countries.iter().collect();

                if countries_vec.is_empty() {
                    ui.label(
                        RichText::new("Loading countries...")
                            .font(egui::FontId::proportional(20.0))
                            .color(Color32::GRAY),
                    );
                } else {
                    egui::Grid::new("country_grid")
                        .num_columns(3)
                        .spacing([20.0, 20.0])
                        .show(ui, |ui| {
                            for (i, (entity, name, map_color)) in countries_vec.iter().enumerate() {
                                let color = map_color.0;
                                let [r, g, b, _] = color.to_srgba().to_u8_array();
                                let egui_color = Color32::from_rgb(r, g, b);

                                let button = egui::Button::new(
                                    RichText::new(&name.0)
                                        .font(egui::FontId::proportional(20.0))
                                        .color(Color32::WHITE),
                                )
                                .fill(egui_color)
                                .min_size(egui::vec2(180.0, 80.0));

                                if ui.add(button).clicked() {
                                    player.country = Some(*entity);
                                    info!("Player selected country: {}", name.0);
                                    next_state.set(MenuState::InGame);
                                }

                                if (i + 1) % 3 == 0 {
                                    ui.end_row();
                                }
                            }
                        });
                }

                ui.add_space(40.0);

                // Back button
                if ui
                    .add_sized(
                        egui::vec2(150.0, 40.0),
                        egui::Button::new(
                            RichText::new("← Back")
                                .font(egui::FontId::proportional(18.0))
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(80, 80, 80)),
                    )
                    .clicked()
                {
                    next_state.set(MenuState::MainMenu);
                }
            });
        });
}

fn hide_menu() {
    info!("Game started - hiding menu");
}
