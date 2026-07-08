use egui::{Color32, Rounding, Response, RichText, Stroke, Ui, Visuals};

/// BeamNG UI orange accent (~ #FF6600).
pub fn beamng_orange() -> Color32 {
    Color32::from_rgb(255, 102, 0)
}

pub fn apply_dark_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();
    visuals.window_fill = Color32::from_rgb(35, 35, 35);
    visuals.panel_fill = Color32::from_rgb(45, 45, 45);
    visuals.extreme_bg_color = Color32::from_rgb(25, 25, 25);
    visuals.faint_bg_color = Color32::from_rgb(55, 55, 55);
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(50, 50, 50);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(60, 60, 60);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(75, 75, 75);
    visuals.widgets.active.bg_fill = Color32::from_rgb(90, 55, 20);
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(255, 102, 0, 60);
    visuals.override_text_color = Some(Color32::from_rgb(230, 230, 230));
    visuals.hyperlink_color = beamng_orange();
    visuals.window_rounding = Rounding::same(4.0);
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(14.0, 7.0);
    style.spacing.indent = 18.0;
    style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(55, 55, 55);
    ctx.set_style(style);
}

pub fn accent_color() -> Color32 {
    beamng_orange()
}

pub fn success_color() -> Color32 {
    Color32::from_rgb(120, 200, 100)
}

pub fn error_color() -> Color32 {
    Color32::from_rgb(240, 80, 60)
}

pub fn card_bg() -> Color32 {
    Color32::from_rgb(40, 40, 40)
}

pub fn card_stroke(selected: bool, hovered: bool) -> Stroke {
    if selected {
        Stroke::new(2.5, beamng_orange())
    } else if hovered {
        Stroke::new(1.5, Color32::from_rgb(180, 80, 20))
    } else {
        Stroke::new(1.0, Color32::from_rgb(65, 65, 65))
    }
}

/// Orange filled button with white label (nav tabs, Drive, primary actions).
pub fn orange_button(ui: &mut Ui, label: &str) -> Response {
    let text = RichText::new(label).color(Color32::WHITE).strong();
    ui.add(
        egui::Button::new(text)
            .fill(beamng_orange())
            .stroke(Stroke::NONE)
            .rounding(4.0),
    )
}

/// Nav tab: orange + white when selected, muted text when not.
pub fn nav_tab(ui: &mut Ui, label: &str, selected: bool) -> Response {
    if selected {
        orange_button(ui, label)
    } else {
        ui.add(
            egui::Button::new(RichText::new(label).color(Color32::from_rgb(190, 190, 190)))
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE),
        )
    }
}

/// Truncate long labels with ".." suffix for single-line display.
pub fn truncate_label(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }
    let take = max_chars.saturating_sub(2);
    let mut out: String = text.chars().take(take).collect();
    out.push_str("..");
    out
}
