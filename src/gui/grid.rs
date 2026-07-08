use egui::{Color32, FontId, ScrollArea, Sense, Ui, Vec2};

use crate::gui::{accent_color, card_bg, card_stroke, truncate_label};
use crate::scanner::{VehicleCategory, VehicleEntry};
use crate::state::{AppState, AppTab};

const CARD_W: f32 = 200.0;
const CARD_GAP: f32 = 14.0;
const THUMB_H: f32 = 118.0;
const FOOTER_H: f32 = 46.0;
const GEAR_W: f32 = 34.0;
const TITLE_MAX_CHARS: usize = 28;

pub fn draw_grid(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(&mut state.search_query);

        ui.separator();
        ui.checkbox(&mut state.saved_only, "Saved");
        ui.checkbox(&mut state.stock_only, "Stock");
        ui.checkbox(&mut state.mod_only, "Mods");

        ui.separator();
        ui.label("Mod:");
        egui::ComboBox::from_id_salt("mod_filter")
            .selected_text(state.mod_filter.as_deref().unwrap_or("All"))
            .show_ui(ui, |ui| {
                if ui.selectable_label(state.mod_filter.is_none(), "All").clicked() {
                    state.mod_filter = None;
                }
                for m in state.mod_names() {
                    let selected = state.mod_filter.as_deref() == Some(m.as_str());
                    if ui.selectable_label(selected, &m).clicked() {
                        state.mod_filter = Some(m);
                    }
                }
            });
    });

    ui.add_space(8.0);

    let vehicles: Vec<VehicleEntry> = state
        .filtered_vehicles()
        .into_iter()
        .cloned()
        .collect();
    ui.label(format!("{} vehicles", vehicles.len()));

    ScrollArea::vertical().show(ui, |ui| {
        let available_w = ui.available_width();
        let cols = column_count(available_w);
        let card_h = THUMB_H + FOOTER_H;

        egui::Grid::new("vehicle_grid")
            .num_columns(cols)
            .spacing([CARD_GAP, CARD_GAP])
            .min_col_width(CARD_W)
            .show(ui, |ui| {
                for (i, vehicle) in vehicles.iter().enumerate() {
                    draw_card(ui, state, vehicle, card_h);
                    if (i + 1) % cols == 0 {
                        ui.end_row();
                    }
                }
            });
    });
}

fn column_count(available_w: f32) -> usize {
    let cols = ((available_w + CARD_GAP) / (CARD_W + CARD_GAP)).floor() as usize;
    cols.max(1)
}

fn draw_card(ui: &mut Ui, state: &mut AppState, vehicle: &VehicleEntry, card_h: f32) {
    let selected = state.selected_id.as_deref() == Some(vehicle.id.as_str());

    if let Some(thumb) = &vehicle.thumbnail {
        state.thumbnails.queue_load(&vehicle.id, thumb);
    }

    let (rect, response) = ui.allocate_exact_size(Vec2::new(CARD_W, card_h), Sense::click());
    let hovered = response.hovered();
    let painter = ui.painter();

    painter.rect(rect, 4.0, card_bg(), card_stroke(selected, hovered));

    let thumb_rect = egui::Rect::from_min_size(rect.min, Vec2::new(CARD_W, THUMB_H));
    painter.rect_filled(thumb_rect, 0.0, Color32::from_rgb(30, 30, 30));

    if let Some(tex) = state.thumbnails.get(&vehicle.id) {
        painter.image(
            tex.id(),
            thumb_rect.shrink(2.0),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else if state.thumbnails.is_loading(&vehicle.id) {
        painter.text(
            thumb_rect.center(),
            egui::Align2::CENTER_CENTER,
            "Loading…",
            FontId::proportional(11.0),
            Color32::GRAY,
        );
    } else {
        painter.text(
            thumb_rect.center(),
            egui::Align2::CENTER_CENTER,
            "No preview",
            FontId::proportional(11.0),
            Color32::DARK_GRAY,
        );
    }

    if vehicle.config_count > 1 {
        let badge_text = format!("{}", vehicle.config_count);
        let badge_font = FontId::proportional(11.0);
        let galley = painter.layout_no_wrap(badge_text, badge_font, Color32::WHITE);
        let badge_size = galley.size() + egui::vec2(10.0, 4.0);
        let badge_rect = egui::Rect::from_min_size(
            egui::pos2(rect.max.x - badge_size.x - 6.0, rect.min.y + 6.0),
            badge_size,
        );
        painter.rect_filled(badge_rect, 3.0, Color32::from_rgba_unmultiplied(0, 0, 0, 140));
        painter.galley(
            badge_rect.min + egui::vec2(5.0, 2.0),
            galley,
            Color32::WHITE,
        );
    }

    let badge = match vehicle.category {
        VehicleCategory::Saved => "SAVED",
        VehicleCategory::Stock => "STOCK",
        VehicleCategory::Mod if vehicle.in_zip => "ZIP",
        VehicleCategory::Mod => "MOD",
    };
    painter.text(
        egui::pos2(rect.min.x + 8.0, rect.min.y + 8.0),
        egui::Align2::LEFT_TOP,
        badge,
        FontId::proportional(9.0),
        accent_color(),
    );

    let footer_top = rect.min.y + THUMB_H;
    let name_width = CARD_W - 16.0 - GEAR_W;
    let display_name = truncate_label(&vehicle.name, TITLE_MAX_CHARS);
    let name_galley = painter.layout(
        display_name,
        FontId::proportional(12.0),
        Color32::WHITE,
        name_width,
    );
    let name_pos = egui::pos2(rect.min.x + 8.0, footer_top + 6.0);
    painter.galley(name_pos, name_galley, Color32::WHITE);

    let name_rect = egui::Rect::from_min_size(
        name_pos,
        Vec2::new(name_width, FOOTER_H - 8.0),
    );
    let name_response = ui.interact(
        name_rect,
        ui.id().with(format!("name_{}", vehicle.id)),
        Sense::hover(),
    );
    if vehicle.name.chars().count() > TITLE_MAX_CHARS {
        name_response.on_hover_text(&vehicle.name);
    }

    let edit_rect = egui::Rect::from_min_size(
        egui::pos2(rect.max.x - GEAR_W - 2.0, footer_top + 8.0),
        Vec2::new(GEAR_W, FOOTER_H - 10.0),
    );
    let edit_response = ui.interact(edit_rect, ui.id().with(vehicle.id.as_str()), Sense::click());

    let edit_color = if edit_response.hovered() {
        accent_color()
    } else {
        Color32::from_rgb(200, 200, 200)
    };
    painter.text(
        edit_rect.center(),
        egui::Align2::CENTER_CENTER,
        "⚙",
        FontId::proportional(16.0),
        edit_color,
    );

    if response.clicked() {
        state.selected_id = Some(vehicle.id.clone());
    }

    if edit_response.clicked() {
        open_vehicle(state, vehicle);
    }

    if response.double_clicked() {
        open_vehicle(state, vehicle);
    }

    if edit_response.hovered() {
        edit_response.on_hover_text("Edit vehicle config");
    }
}

fn open_vehicle(state: &mut AppState, vehicle: &VehicleEntry) {
    state.selected_id = Some(vehicle.id.clone());
    state.request_load_vehicle_id = Some(vehicle.id.clone());
    state.tab = AppTab::Editor;
}
