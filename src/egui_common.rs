use bevy_egui::egui;
use bevy_egui::egui::Color32;

/// Reusable stylized frame (basically a Flutter 'Container' widget) for usage in most egui
/// widgets in game.
pub(crate) fn default_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::from_rgb(25, 35, 60))
        .stroke(egui::Stroke::new(2.0, Color32::from_rgb(180, 150, 80)))
        .inner_margin(egui::Margin::same(20))
        .corner_radius(egui::CornerRadius::same(12))
        .shadow(egui::Shadow {
            offset: [4, 4],
            blur: 10,
            spread: 0,
            color: Color32::from_black_alpha(150),
        })
}

/// Reusable stylized close button, adjusted to the right of the rect.
pub(crate) fn close_button(ui: &mut egui::Ui) -> bool {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.add(egui::Button::new("X").fill(Color32::from_rgb(200, 50, 50)))
            .on_hover_text("Close")
            .clicked()
    })
    .inner
}
