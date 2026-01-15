use crate::country::{Country, DisplayName, MapColor};
use crate::player::Player;
use crate::savegame::{save_exists, LoadGameEvent, SaveGameEvent};
use bevy::prelude::*;
use bevy_egui::egui::{Color32, RichText};
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<MenuState>()
            .insert_resource(PauseMenuOpen(false))
            .add_systems(
                EguiPrimaryContextPass,
                display_main_menu.run_if(in_state(MenuState::MainMenu)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                display_country_selection.run_if(in_state(MenuState::CountrySelection)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                display_pause_menu.run_if(in_state(MenuState::InGame)),
            )
            .add_systems(
                Update,
                handle_escape_key.run_if(in_state(MenuState::InGame)),
            )
            .add_systems(OnEnter(MenuState::InGame), hide_menu);
    }
}

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum MenuState {
    #[default]
    MainMenu,
    CountrySelection,
    InGame,
}

#[derive(Resource)]
pub struct PauseMenuOpen(pub bool);

fn handle_escape_key(keyboard: Res<ButtonInput<KeyCode>>, mut pause_menu: ResMut<PauseMenuOpen>) {
    if keyboard.just_pressed(KeyCode::Escape) {
        pause_menu.0 = !pause_menu.0;
    }
}

fn display_main_menu(
    mut contexts: EguiContexts,
    mut next_state: ResMut<NextState<MenuState>>,
    mut load_events: MessageWriter<LoadGameEvent>,
) {
    let ctx = match contexts.ctx_mut() {
        Ok(c) => c,
        Err(_) => return,
    };

    let has_save = save_exists();

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

                let load_button = egui::Button::new(
                    RichText::new("📂 Load Game")
                        .font(egui::FontId::proportional(24.0))
                        .color(if has_save {
                            Color32::WHITE
                        } else {
                            Color32::DARK_GRAY
                        }),
                )
                .fill(if has_save {
                    Color32::from_rgb(60, 120, 80)
                } else {
                    Color32::from_rgb(40, 40, 40)
                });

                let load_response = ui.add_sized(button_size, load_button);

                if has_save && load_response.clicked() {
                    load_events.write(LoadGameEvent);
                    next_state.set(MenuState::InGame);
                    info!("Loading saved game...");
                }

                if !has_save {
                    load_response.on_hover_text("No save file found");
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

fn display_pause_menu(
    mut contexts: EguiContexts,
    mut pause_menu: ResMut<PauseMenuOpen>,
    mut next_state: ResMut<NextState<MenuState>>,
    mut save_events: MessageWriter<SaveGameEvent>,
    mut load_events: MessageWriter<LoadGameEvent>,
) {
    if !pause_menu.0 {
        return;
    }

    let ctx = match contexts.ctx_mut() {
        Ok(c) => c,
        Err(_) => return,
    };

    let has_save = save_exists();

    egui::Area::new(egui::Id::new("pause_overlay"))
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(ctx, |ui| {
            let screen_rect = ui
                .ctx()
                .input(|i| i.viewport().inner_rect.unwrap_or(egui::Rect::NOTHING));
            ui.painter().rect_filled(
                screen_rect,
                0.0,
                Color32::from_rgba_unmultiplied(0, 0, 0, 180),
            );
        });

    egui::Window::new("Pause Menu")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .frame(
            egui::Frame::new()
                .fill(Color32::from_rgb(30, 30, 40))
                .inner_margin(30.0)
                .corner_radius(10.0),
        )
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("⏸ PAUSED")
                        .font(egui::FontId::proportional(36.0))
                        .color(Color32::WHITE)
                        .strong(),
                );

                ui.add_space(30.0);

                let button_size = egui::vec2(200.0, 45.0);

                if ui
                    .add_sized(
                        button_size,
                        egui::Button::new(
                            RichText::new("▶ Resume")
                                .font(egui::FontId::proportional(20.0))
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(60, 120, 80)),
                    )
                    .clicked()
                {
                    pause_menu.0 = false;
                }

                ui.add_space(15.0);

                if ui
                    .add_sized(
                        button_size,
                        egui::Button::new(
                            RichText::new("💾 Save Game")
                                .font(egui::FontId::proportional(20.0))
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(80, 80, 120)),
                    )
                    .clicked()
                {
                    save_events.write(SaveGameEvent);
                    info!("Game saved!");
                }

                ui.add_space(15.0);

                let load_button = egui::Button::new(
                    RichText::new("📂 Load Game")
                        .font(egui::FontId::proportional(20.0))
                        .color(if has_save {
                            Color32::WHITE
                        } else {
                            Color32::DARK_GRAY
                        }),
                )
                .fill(if has_save {
                    Color32::from_rgb(60, 100, 80)
                } else {
                    Color32::from_rgb(40, 40, 40)
                });

                let load_response = ui.add_sized(button_size, load_button);

                if has_save && load_response.clicked() {
                    load_events.write(LoadGameEvent);
                    pause_menu.0 = false;
                    info!("Loading saved game...");
                }

                if !has_save {
                    load_response.on_hover_text("No save file found");
                }

                ui.add_space(15.0);

                if ui
                    .add_sized(
                        button_size,
                        egui::Button::new(
                            RichText::new("🏠 Main Menu")
                                .font(egui::FontId::proportional(20.0))
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(120, 100, 60)),
                    )
                    .clicked()
                {
                    pause_menu.0 = false;
                    next_state.set(MenuState::MainMenu);
                }

                ui.add_space(15.0);

                if ui
                    .add_sized(
                        button_size,
                        egui::Button::new(
                            RichText::new("❌ Quit Game")
                                .font(egui::FontId::proportional(20.0))
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(120, 60, 60)),
                    )
                    .clicked()
                {
                    std::process::exit(0);
                }

                ui.add_space(10.0);

                ui.label(
                    RichText::new("Press ESC to resume")
                        .font(egui::FontId::proportional(14.0))
                        .color(Color32::GRAY)
                        .italics(),
                );
            });
        });
}

fn hide_menu(mut pause_menu: ResMut<PauseMenuOpen>) {
    pause_menu.0 = false;
    info!("Game started - hiding menu");
}
