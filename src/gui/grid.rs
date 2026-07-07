use egui::{Color32, RichText, ScrollArea, Ui, Vec2};

use crate::gui::card_stroke;
use crate::scanner::VehicleEntry;
use crate::state::{AppState, AppTab};

pub fn draw_grid(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(&mut state.search_query);

        ui.separator();
        ui.checkbox(&mut state.stock_only, "Stock only");
        ui.checkbox(&mut state.mod_only, "Mods only");

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
        egui::Grid::new("vehicle_grid")
            .num_columns(4)
            .spacing([12.0, 12.0])
            .show(ui, |ui| {
                for (i, vehicle) in vehicles.iter().enumerate() {
                    draw_card(ui, state, vehicle);
                    if (i + 1) % 4 == 0 {
                        ui.end_row();
                    }
                }
            });
    });
}

fn draw_card(ui: &mut Ui, state: &mut AppState, vehicle: &VehicleEntry) {
    let selected = state.selected_id.as_deref() == Some(vehicle.id.as_str());
    let card_size = Vec2::new(180.0, 220.0);

    if let Some(path) = &vehicle.thumbnail_path {
        state.thumbnails.queue_load(&vehicle.id, path);
    }

    let response = egui::Frame::none()
        .fill(Color32::from_rgb(28, 28, 34))
        .stroke(card_stroke(selected))
        .inner_margin(8.0)
        .rounding(8.0)
        .show(ui, |ui| {
            ui.set_width(card_size.x);
            ui.set_min_height(card_size.y);

            ui.vertical(|ui| {
                ui.allocate_ui(Vec2::new(164.0, 120.0), |ui| {
                    if let Some(tex) = state.thumbnails.get(&vehicle.id) {
                        ui.image((tex.id(), Vec2::new(164.0, 120.0)));
                    } else {
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(164.0, 120.0), egui::Sense::hover());
                        ui.painter().rect_filled(
                            rect,
                            4.0,
                            Color32::from_rgb(40, 40, 48),
                        );
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "No preview",
                            egui::FontId::proportional(12.0),
                            Color32::GRAY,
                        );
                    }
                });

                ui.add_space(4.0);
                ui.label(RichText::new(&vehicle.name).strong().size(14.0));
                ui.label(
                    RichText::new(format!(
                        "{} · {}",
                        vehicle.mod_name,
                        if vehicle.is_stock { "stock" } else { "mod" }
                    ))
                    .weak()
                    .size(11.0),
                );
            });
        })
        .response;

    if response.clicked() {
        state.selected_id = Some(vehicle.id.clone());
        state.request_load_vehicle = Some(vehicle.config_path.clone());
        state.tab = AppTab::Editor;
    }

    if response.double_clicked() {
        state.selected_id = Some(vehicle.id.clone());
        state.request_load_vehicle = Some(vehicle.config_path.clone());
        state.tab = AppTab::Editor;
    }
}
