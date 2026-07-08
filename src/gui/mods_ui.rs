use egui::{Color32, RichText, ScrollArea, Ui};

use crate::gui::{accent_color, orange_button};
use crate::mod_scanner::{
    add_vehicle_to_mod, all_known_models, is_editable, mod_root_path, remove_vehicle_folder,
    write_compat_file, ModKind, ModLocation, ModStorage,
};
use crate::state::AppState;

pub fn draw_mods(ui: &mut Ui, state: &mut AppState) {
    ui.label(
        RichText::new("Engine & parts mods")
            .strong()
            .size(16.0)
            .color(Color32::WHITE),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Packed mods are zip files. Unpack before editing, then pack again when done.",
        )
        .color(Color32::LIGHT_GRAY)
        .size(13.0),
    );
    ui.add_space(12.0);

    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(&mut state.search_query);
        if ui.button("Rescan mods").clicked() {
            state.request_rescan = true;
        }
    });

    ui.add_space(12.0);

    let packed: Vec<_> = state
        .filtered_mods_by_storage(ModStorage::Packed)
        .into_iter()
        .cloned()
        .collect();
    let unpacked: Vec<_> = state
        .filtered_mods_by_storage(ModStorage::Unpacked)
        .into_iter()
        .cloned()
        .collect();
    ui.label(format!(
        "{} packed · {} unpacked",
        packed.len(),
        unpacked.len()
    ));

    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(280.0);
            ScrollArea::vertical()
                .max_height(ui.available_height())
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("Packed (zip)")
                            .strong()
                            .color(Color32::WHITE),
                    );
                    ui.add_space(6.0);
                    for m in &packed {
                        draw_mod_list_item(ui, state, m);
                    }
                    ui.add_space(16.0);
                    ui.label(
                        RichText::new("Unpacked (editable)")
                            .strong()
                            .color(Color32::WHITE),
                    );
                    ui.add_space(6.0);
                    for m in &unpacked {
                        draw_mod_list_item(ui, state, m);
                    }
                });
        });

        ui.add_space(24.0);

        ui.vertical(|ui| {
            ui.set_min_width((ui.available_width() - 300.0).max(400.0));
            let Some(mod_entry) = state.selected_mod().cloned() else {
                ui.label(RichText::new("Select a mod on the left.").color(Color32::LIGHT_GRAY));
                return;
            };
            draw_mod_detail(ui, state, &mod_entry);
        });
    });
}

fn draw_mod_list_item(ui: &mut Ui, state: &mut AppState, m: &crate::mod_scanner::ModEntry) {
    let selected = state.selected_mod_id.as_deref() == Some(m.id.as_str());
    let kind = match m.kind {
        ModKind::EngineParts => "Engine",
        ModKind::PartsPack => "Parts",
        ModKind::FullVehicle => "Vehicle",
    };
    let storage = if m.storage == ModStorage::Packed {
        "zip"
    } else {
        "dir"
    };
    let label = format!("{} [{kind}] ({}) · {storage}", m.name, m.target_vehicles.len());
    if ui.selectable_label(selected, label).clicked() {
        state.selected_mod_id = Some(m.id.clone());
        if state.mod_template_vehicle.is_empty() {
            if let Some(first) = m.target_vehicles.first() {
                state.mod_template_vehicle = first.clone();
            }
        }
    }
}

fn draw_mod_detail(ui: &mut Ui, state: &mut AppState, mod_entry: &crate::mod_scanner::ModEntry) {
    ui.label(
        RichText::new(&mod_entry.name)
            .strong()
            .size(18.0)
            .color(Color32::WHITE),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(format!(
            "{} engines · {} jbeam · {} configs",
            mod_entry.engine_count, mod_entry.jbeam_count, mod_entry.pc_count
        ))
        .color(Color32::LIGHT_GRAY),
    );

    match &mod_entry.location {
        ModLocation::Unpacked { root } => {
            ui.label(RichText::new(root.display().to_string()).monospace().size(11.0));
            if ui.button("Pack to mods/packed").clicked() {
                state.request_pack_mod_id = Some(mod_entry.id.clone());
            }
        }
        ModLocation::Zip { archive_path } => {
            ui.label(
                RichText::new(format!("Zip: {}", archive_path.display()))
                    .color(Color32::LIGHT_GRAY)
                    .size(11.0),
            );
            if orange_button(ui, "Unpack to mods/unpacked").clicked() {
                state.request_unpack_mod_id = Some(mod_entry.id.clone());
            }
            ui.label(
                RichText::new("Unpack before editing vehicle compatibility.")
                    .color(accent_color())
                    .size(12.0),
            );
        }
    }

        if let Some(model) = state.current_vehicle_model() {
            ui.add_space(8.0);
            let add_label = format!("Add current car ({model}) to this mod");
            if orange_button(ui, &add_label).clicked() {
                state.request_add_car_to_mod_id = Some(mod_entry.id.clone());
            }
        }

    ui.add_space(16.0);
    ui.label(RichText::new("Supported vehicles").strong().color(Color32::WHITE));
    ui.add_space(8.0);

    if mod_entry.target_vehicles.is_empty() {
        ui.label(RichText::new("No vehicle folders found under vehicles/.").color(Color32::LIGHT_GRAY));
    } else if is_editable(mod_entry) {
        for vehicle in &mod_entry.target_vehicles {
            ui.horizontal(|ui| {
                ui.label(RichText::new(vehicle).monospace().color(Color32::WHITE));
                if ui.button("Remove").clicked() {
                    state.pending_mod_remove = Some(vehicle.clone());
                }
            });
        }
    } else {
        for vehicle in &mod_entry.target_vehicles {
            ui.label(RichText::new(vehicle).monospace().color(Color32::LIGHT_GRAY));
        }
    }

    if is_editable(mod_entry) {
        if let Some(root) = mod_root_path(mod_entry) {
            ui.add_space(16.0);
            ui.separator();
            ui.add_space(12.0);
            ui.label(RichText::new("Add vehicle support").strong().color(Color32::WHITE));
            ui.add_space(8.0);

            if let Some(model) = state.current_vehicle_model() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Current car: {model}")).color(Color32::WHITE));
                    if orange_button(ui, "Use current car").clicked() {
                        state.mod_add_vehicle = model.clone();
                    }
                });
                ui.add_space(8.0);
            }

            ui.horizontal(|ui| {
                ui.label("Vehicle model:");
                ui.text_edit_singleline(&mut state.mod_add_vehicle);
            });

            ui.horizontal(|ui| {
                ui.label("Copy adapters from:");
                egui::ComboBox::from_id_salt("mod_template_vehicle")
                    .selected_text(if state.mod_template_vehicle.is_empty() {
                        "(none)".to_string()
                    } else {
                        state.mod_template_vehicle.clone()
                    })
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(state.mod_template_vehicle.is_empty(), "(none)")
                            .clicked()
                        {
                            state.mod_template_vehicle.clear();
                        }
                        for v in &mod_entry.target_vehicles {
                            if ui
                                .selectable_label(state.mod_template_vehicle == *v, v)
                                .clicked()
                            {
                                state.mod_template_vehicle = v.clone();
                            }
                        }
                        let known = all_known_models(&state.mods, &state.vehicles);
                        for v in known {
                            if mod_entry.target_vehicles.iter().any(|t| t == &v) {
                                continue;
                            }
                            if ui
                                .selectable_label(state.mod_template_vehicle == v, &v)
                                .clicked()
                            {
                                state.mod_template_vehicle = v;
                            }
                        }
                    });
            });

            ui.add_space(8.0);
            if orange_button(ui, "Add vehicle").clicked() {
                let vehicle = state.mod_add_vehicle.trim().to_string();
                if vehicle.is_empty() {
                    state.push_toast(crate::state::Toast::error("Enter a vehicle model name"));
                } else {
                    let template = if state.mod_template_vehicle.is_empty() {
                        None
                    } else {
                        Some(state.mod_template_vehicle.as_str())
                    };
                    match add_vehicle_to_mod(mod_entry, &vehicle, template) {
                        Ok(()) => {
                            state.push_toast(crate::state::Toast::info(format!(
                                "Added support for {vehicle}"
                            )));
                            state.mod_add_vehicle.clear();
                            state.request_rescan = true;
                        }
                        Err(e) => state.push_toast(crate::state::Toast::error(e.to_string())),
                    }
                }
            }
            let _ = root;
        }
    }

    if let Some(remove) = state.pending_mod_remove.take() {
        if let Some(root) = mod_root_path(mod_entry) {
            match remove_vehicle_folder(&root, &remove) {
                Ok(()) => {
                    let vehicles: Vec<String> = mod_entry
                        .target_vehicles
                        .iter()
                        .filter(|v| *v != &remove)
                        .cloned()
                        .collect();
                    let _ = write_compat_file(&root, &vehicles);
                    state.push_toast(crate::state::Toast::info(format!("Removed {remove}")));
                    state.request_rescan = true;
                }
                Err(e) => state.push_toast(crate::state::Toast::error(e.to_string())),
            }
        }
    }
}
