use egui::{Color32, Rounding, Stroke, Visuals};

pub fn apply_dark_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();
    visuals.window_fill = Color32::from_rgb(18, 18, 22);
    visuals.panel_fill = Color32::from_rgb(24, 24, 30);
    visuals.extreme_bg_color = Color32::from_rgb(12, 12, 16);
    visuals.faint_bg_color = Color32::from_rgb(32, 32, 40);
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(30, 30, 38);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(40, 40, 50);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(55, 55, 70);
    visuals.widgets.active.bg_fill = Color32::from_rgb(70, 90, 140);
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(80, 120, 200, 80);
    visuals.window_rounding = Rounding::same(8.0);
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    ctx.set_style(style);
}

pub fn accent_color() -> Color32 {
    Color32::from_rgb(100, 160, 255)
}

pub fn success_color() -> Color32 {
    Color32::from_rgb(80, 200, 120)
}

pub fn error_color() -> Color32 {
    Color32::from_rgb(240, 90, 90)
}

pub fn card_stroke(selected: bool) -> Stroke {
    if selected {
        Stroke::new(2.0, accent_color())
    } else {
        Stroke::new(1.0, Color32::from_rgb(50, 50, 60))
    }
}
