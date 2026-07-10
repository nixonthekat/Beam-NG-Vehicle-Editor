use egui::{Color32, RichText, ScrollArea, Ui};

use crate::gui::{accent_color, orange_button};
use crate::mod_scanner::{
    add_vehicle_adapter_for_engine_mod, all_known_models,
    is_adapter_mod, is_listable_mod, read_adapter_compat,
    remove_vehicle_adapter_for_engine_mod, ModLocation, ModStorage, ADAPTER_MOD_FOLDER,
};
use crate::state::AppState;

pub fn draw_mods(ui: &mut Ui, state: &mut AppState) {
    ui.label(
        RichText::new("Engine mods")
            .strong()
            .size(16.0)
            .color(Color32::WHITE),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Engine packs stay untouched. Vehicle adapters live in a separate mod you enable in BeamNG.",
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

    draw_adapter_mod_summary(ui, state);
    ui.add_space(16.0);

    let packed: Vec<_> = state
        .filtered_mods_by_storage(ModStorage::Packed)
        .into_iter()
        .filter(|m| is_listable_mod(m))
        .cloned()
        .collect();
    let unpacked: Vec<_> = state
        .filtered_mods_by_storage(ModStorage::Unpacked)
        .into_iter()
        .filter(|m| is_listable_mod(m))
        .cloned()
        .collect();
    ui.label(format!(
        "{} packed · {} unpacked mods",
        packed.len(),
        unpacked.len()
    ));

    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(280.0);
            ScrollArea::vertical()
                .max_height(ui.available_height())
                .show(ui, |ui| {
                    if let Some(adapter) = state.adapter_mod_entry().cloned() {
                        ui.label(
                            RichText::new("Your adapter mod")
                                .strong()
                                .color(Color32::WHITE),
                        );
                        ui.add_space(6.0);
                        draw_mod_list_item(ui, state, &adapter);
                        ui.add_space(16.0);
                    }
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
                        RichText::new("Unpacked")
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
                ui.label(RichText::new("Select an engine mod on the left.").color(Color32::LIGHT_GRAY));
                return;
            };
            if is_adapter_mod(&mod_entry.name) {
                draw_adapter_mod_detail(ui, state, &mod_entry);
            } else {
                draw_engine_mod_detail(ui, state, &mod_entry);
            }
        });
    });
}

fn draw_adapter_mod_summary(ui: &mut Ui, state: &AppState) {
    ui.label(
        RichText::new(format!("Adapter mod: {ADAPTER_MOD_FOLDER}"))
            .strong()
            .color(Color32::WHITE),
    );
    ui.add_space(4.0);
    if let Some(adapter) = state.adapter_mod_entry() {
        let ModLocation::Unpacked { root } = &adapter.location else {
            ui.label(RichText::new("Adapter mod path unavailable.").color(Color32::LIGHT_GRAY));
            return;
        };
        let compat = read_adapter_compat(root).unwrap_or_default();
        ui.label(
            RichText::new(format!(
                "{} vehicle adapter(s) · {}",
                compat.target_vehicles.len(),
                root.display()
            ))
            .color(Color32::LIGHT_GRAY)
            .size(12.0),
        );
        ui.label(
            RichText::new("Enable this mod in BeamNG alongside your engine pack(s).")
                .color(accent_color())
                .size(12.0),
        );
    } else {
        ui.label(
            RichText::new("Not created yet — use Create adapter on a car in the Editor.")
                .color(Color32::LIGHT_GRAY)
                .size(12.0),
        );
    }
}

fn draw_mod_list_item(ui: &mut Ui, state: &mut AppState, m: &crate::mod_scanner::ModEntry) {
    let selected = state.selected_mod_id.as_deref() == Some(m.id.as_str());
    let storage = if m.storage == ModStorage::Packed {
        "zip"
    } else {
        "dir"
    };
    let label = if is_adapter_mod(&m.name) {
        format!("{} [Adapter] · {storage}", m.name)
    } else {
        let kind = match m.kind {
            crate::mod_scanner::ModKind::EngineParts => "Engine",
            crate::mod_scanner::ModKind::PartsPack => "Parts",
            crate::mod_scanner::ModKind::FullVehicle => "Vehicle",
        };
        if m.engine_count > 0 {
            format!("{} [{kind}] ({}) · {storage}", m.name, m.engine_count)
        } else {
            format!("{} [{kind}] · {storage}", m.name)
        }
    };
    if ui.selectable_label(selected, label).clicked() {
        state.selected_mod_id = Some(m.id.clone());
        if state.mod_template_vehicle.is_empty() {
            if let Some(first) = m.target_vehicles.first() {
                state.mod_template_vehicle = first.clone();
            }
        }
    }
}

fn draw_engine_mod_detail(ui: &mut Ui, state: &mut AppState, mod_entry: &crate::mod_scanner::ModEntry) {
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
        }
    }

    if let Some(model) = state.current_vehicle_model() {
        ui.add_space(8.0);
        let built_in = mod_entry.target_vehicles.iter().any(|v| v == &model);
        let adapter = state.adapter_supports_vehicle(&mod_entry.name, &model);
        if built_in {
            ui.label(
                RichText::new(format!("{model} is built into this engine mod."))
                    .color(Color32::LIGHT_GRAY),
            );
        } else if adapter {
            ui.label(
                RichText::new(format!("{model} has an adapter in {ADAPTER_MOD_FOLDER}."))
                    .color(Color32::LIGHT_GRAY),
            );
        } else {
            let add_label = format!("Create adapter for {model}");
            if orange_button(ui, &add_label).clicked() {
                state.request_add_car_to_mod_id = Some(mod_entry.id.clone());
            }
        }
    }

    ui.add_space(16.0);
    ui.label(RichText::new("Built-in vehicle support").strong().color(Color32::WHITE));
    ui.add_space(8.0);
    if mod_entry.target_vehicles.is_empty() {
        ui.label(RichText::new("None in this mod's vehicles/ folder.").color(Color32::LIGHT_GRAY));
    } else {
        for vehicle in &mod_entry.target_vehicles {
            ui.label(RichText::new(vehicle).monospace().color(Color32::LIGHT_GRAY));
        }
    }

    ui.add_space(16.0);
    ui.label(RichText::new("Adapter mod support").strong().color(Color32::WHITE));
    ui.add_space(8.0);
    let adapter_vehicles = adapter_vehicles_for_mod(state, &mod_entry.name);
    if adapter_vehicles.is_empty() {
        ui.label(
            RichText::new(format!("No adapters linked to {} yet.", mod_entry.name))
                .color(Color32::LIGHT_GRAY),
        );
    } else {
        for vehicle in &adapter_vehicles {
            ui.horizontal(|ui| {
                ui.label(RichText::new(vehicle).monospace().color(Color32::WHITE));
                if ui.button("Remove adapter").clicked() {
                    state.pending_adapter_remove = Some((mod_entry.name.clone(), vehicle.clone()));
                }
            });
        }
    }

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(12.0);
    ui.label(RichText::new("Add adapter for another vehicle").strong().color(Color32::WHITE));
    ui.add_space(8.0);
    draw_add_adapter_form(ui, state, mod_entry);

    if let Some((engine_mod, vehicle)) = state.pending_adapter_remove.take() {
        match remove_vehicle_adapter_for_engine_mod(&state.settings, &engine_mod, &vehicle) {
            Ok(()) => {
                state.push_toast(crate::state::Toast::info(format!(
                    "Removed {vehicle} adapter for {engine_mod}"
                )));
                state.request_rescan = true;
            }
            Err(e) => state.push_toast(crate::state::Toast::error(e.to_string())),
        }
    }
}

fn draw_adapter_mod_detail(ui: &mut Ui, _state: &AppState, adapter: &crate::mod_scanner::ModEntry) {
    ui.label(
        RichText::new(ADAPTER_MOD_FOLDER)
            .strong()
            .size(18.0)
            .color(Color32::WHITE),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new("Your custom vehicle adapter mod — enable it in BeamNG with engine packs.")
            .color(Color32::LIGHT_GRAY),
    );

    let ModLocation::Unpacked { root } = &adapter.location else {
        ui.label(RichText::new("Adapter mod must be unpacked.").color(accent_color()));
        return;
    };
    ui.label(RichText::new(root.display().to_string()).monospace().size(11.0));

    let compat = read_adapter_compat(root).unwrap_or_default();
    ui.add_space(16.0);
    ui.label(RichText::new("Vehicles").strong().color(Color32::WHITE));
    ui.add_space(8.0);
    if compat.target_vehicles.is_empty() {
        ui.label(RichText::new("No adapters yet.").color(Color32::LIGHT_GRAY));
        return;
    }
    for vehicle in &compat.target_vehicles {
        let mods = compat
            .engine_mod_links
            .get(vehicle)
            .map(|v| v.join(", "))
            .unwrap_or_else(|| "(unlinked)".to_string());
        ui.label(
            RichText::new(format!("{vehicle} → {mods}"))
                .monospace()
                .color(Color32::WHITE),
        );
    }
}

fn draw_add_adapter_form(ui: &mut Ui, state: &mut AppState, mod_entry: &crate::mod_scanner::ModEntry) {
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
    if orange_button(ui, "Create adapter").clicked() {
        let vehicle = state.mod_add_vehicle.trim().to_string();
        if vehicle.is_empty() {
            state.push_toast(crate::state::Toast::error("Enter a vehicle model name"));
        } else {
            let template = if state.mod_template_vehicle.is_empty() {
                None
            } else {
                Some(state.mod_template_vehicle.as_str())
            };
            match add_vehicle_adapter_for_engine_mod(&state.settings, mod_entry, &vehicle, template) {
                Ok(()) => {
                    state.push_toast(crate::state::Toast::info(format!(
                        "Created adapter for {vehicle} → {}",
                        mod_entry.name
                    )));
                    state.mod_add_vehicle.clear();
                    state.request_rescan = true;
                }
                Err(e) => state.push_toast(crate::state::Toast::error(e.to_string())),
            }
        }
    }
}

fn adapter_vehicles_for_mod(state: &AppState, engine_mod: &str) -> Vec<String> {
    let Some(adapter) = state.adapter_mod_entry() else {
        return Vec::new();
    };
    let ModLocation::Unpacked { root } = &adapter.location else {
        return Vec::new();
    };
    crate::mod_scanner::vehicles_linked_to_engine_mod(root, engine_mod)
}
