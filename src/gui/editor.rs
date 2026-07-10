use egui::{Color32, RichText, ScrollArea, Ui, Vec2};

use crate::gui::{accent_color, orange_button, success_color};
use crate::parts::{filter_parts_for_mod_entry, friendly_slot_label, is_engine_slot_name};
use crate::state::{AppState, AppTab};

const SIDEBAR_W: f32 = 280.0;

pub fn draw_editor(ui: &mut Ui, state: &mut AppState) {
    if state.edit_buffer.is_none() {
        ui.vertical(|ui| {
            ui.add_space(60.0);
            ui.label(RichText::new("Pick a vehicle from the Grid").size(18.0));
            ui.add_space(8.0);
            ui.label(RichText::new("Click the ⚙ on any card to start editing.").color(Color32::LIGHT_GRAY));
            ui.add_space(16.0);
            if orange_button(ui, "Go to Grid").clicked() {
                state.tab = AppTab::Grid;
            }
        });
        return;
    }

    let full_w = ui.available_width();
    ui.set_width(full_w);

    draw_toolbar(ui, state);
    ui.add_space(16.0);

    let vehicle_name = state
        .selected_vehicle()
        .map(|v| v.name.clone())
        .unwrap_or_else(|| "Vehicle".to_string());

    let avail_h = ui.available_height();
    let engine_panel_h = (avail_h * 0.42).clamp(280.0, 420.0);
    let body_h = (avail_h - engine_panel_h - 20.0).max(180.0);
    let main_w = (full_w - SIDEBAR_W - 24.0).max(400.0);

    ui.horizontal_top(|ui| {
        draw_left_panel(ui, state, &vehicle_name, body_h);

        ui.add_space(24.0);

        ui.allocate_ui_with_layout(
            Vec2::new(main_w, body_h),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                ui.set_width(main_w);
                draw_engine_slot_panel(ui, state, main_w);
                ui.add_space(16.0);
                draw_diff_panel(ui, state, main_w);
            },
        );
    });

    ui.add_space(12.0);
    draw_engine_mod_panel(ui, state, full_w, engine_panel_h);
}

fn draw_toolbar(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        if orange_button(ui, "Apply Changes").clicked() {
            state.request_apply = true;
        }
        ui.add_space(8.0);
        if ui.button("Restore Latest").clicked() {
            state.request_restore_latest = true;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let dirty_label = if state.dirty { "Unsaved changes" } else { "Saved" };
            ui.label(RichText::new(dirty_label).color(if state.dirty {
                accent_color()
            } else {
                success_color()
            }));
        });
    });
}

fn draw_left_panel(ui: &mut Ui, state: &mut AppState, vehicle_name: &str, height: f32) {
    ui.allocate_ui_with_layout(
        Vec2::new(SIDEBAR_W, height),
        egui::Layout::top_down(egui::Align::LEFT),
        |ui| {
            ui.set_width(SIDEBAR_W);
            ui.label(RichText::new(vehicle_name).strong().size(18.0).color(Color32::WHITE));
            ui.add_space(12.0);

            if let Some(vehicle) = state.selected_vehicle().cloned() {
                if let Some(thumb) = &vehicle.thumbnail {
                    let thumb_id = format!("editor_{}", vehicle.id);
                    state.thumbnails.queue_load(&thumb_id, thumb);
                    if let Some(tex) = state.thumbnails.get(&thumb_id) {
                        ui.image((tex.id(), Vec2::new(SIDEBAR_W - 8.0, 180.0)));
                    }
                }
            }

            ui.add_space(16.0);

            if let Some((slot, part)) = state.edit_buffer.as_ref().and_then(|c| c.engine_slot()) {
                ui.label(RichText::new("Installed engine").strong().color(Color32::WHITE));
                ui.add_space(4.0);
                ui.label(RichText::new(friendly_slot_label(slot)).color(Color32::LIGHT_GRAY));
                ui.label(RichText::new(part).monospace().color(Color32::from_rgb(200, 200, 200)));
                ui.add_space(12.0);
            }

            let mod_count = state.engine_mod_entries().len();
            if mod_count > 0 {
                ui.label(
                    RichText::new(format!("{mod_count} engine mod(s) available"))
                        .color(Color32::LIGHT_GRAY)
                        .size(12.0),
                );
            }

            if state.parts_scanning {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.spinner();
                    let progress = state
                        .parts_scan_progress
                        .as_ref()
                        .map(|(n, path)| format!("Loading engines… ({n} scanned) {path}"))
                        .unwrap_or_else(|| "Loading engines…".to_string());
                    ui.label(RichText::new(progress).color(Color32::LIGHT_GRAY));
                });
            } else if !state.parts_scan_done && state.parts_index.engines.is_empty() {
                ui.add_space(8.0);
                if ui.button("Load engines").clicked() {
                    state.request_parts_scan = true;
                }
            }
        },
    );
}

fn draw_engine_slot_panel(ui: &mut Ui, state: &mut AppState, width: f32) {
    ui.label(RichText::new("Engine").strong().size(16.0).color(Color32::WHITE));
    ui.add_space(4.0);
    ui.label(
        RichText::new("Swap the engine on this config. Other parts are not edited here.")
            .color(Color32::LIGHT_GRAY)
            .size(13.0),
    );
    ui.add_space(16.0);

    let Some(slot) = engine_slot_key(state) else {
        ui.label(
            RichText::new("No engine slot found in this vehicle config.")
                .color(accent_color()),
        );
        return;
    };

    let combo_w = (width - 8.0).max(280.0);
    draw_engine_slot_row(ui, state, &slot, combo_w);
}

fn engine_slot_key(state: &AppState) -> Option<String> {
    if let Some(buf) = &state.edit_buffer {
        if let Some((slot, _)) = buf.engine_slot() {
            return Some(slot.to_string());
        }
    }
    state
        .slot_edits
        .keys()
        .find(|s| is_engine_slot_name(s))
        .cloned()
}

fn draw_engine_slot_row(ui: &mut Ui, state: &mut AppState, slot: &str, combo_w: f32) {
    let current = state.slot_edits.get(slot).cloned().unwrap_or_default();
    let is_custom = state.slot_custom_mode.contains(slot);

    ui.label(RichText::new(friendly_slot_label(slot)).strong().color(Color32::WHITE));
    ui.label(
        RichText::new(slot)
            .monospace()
            .size(11.0)
            .color(Color32::from_rgb(185, 185, 190)),
    );
    ui.add_space(6.0);

    if is_custom {
        ui.label(RichText::new("Custom engine ID").color(Color32::LIGHT_GRAY).size(12.0));
        let mut value = current.clone();
        let response = ui.add(
            egui::TextEdit::singleline(&mut value)
                .desired_width(combo_w)
                .font(egui::TextStyle::Monospace),
        );
        if response.changed() {
            apply_slot_change(state, slot, value);
        }
        if ui.link("Back to dropdown").clicked() {
            state.slot_custom_mode.remove(slot);
        }
    } else {
        let groups = state.engine_dropdown_groups(slot, &current);
        let option_count: usize = groups.iter().map(|(_, _, e)| e.len()).sum();

        let selected_text = if current.is_empty() {
            "(none)".to_string()
        } else if let Some(p) = state.parts_index.all_by_id.get(&current) {
            format!("{} ({})", p.name, p.mod_name)
        } else {
            current.clone()
        };

        if option_count == 0 && !state.parts_scanning {
            ui.label(
                RichText::new("No engines loaded — click Rescan engines below or Load engines.")
                    .color(accent_color())
                    .size(12.0),
            );
            ui.add_space(6.0);
        }

        egui::ComboBox::from_id_salt(format!("engine_slot_{slot}"))
            .selected_text(RichText::new(selected_text).color(Color32::WHITE))
            .width(combo_w)
            .show_ui(ui, |ui| {
                for (mod_name, compatible, engines) in &groups {
                    let header = if *compatible {
                        format!("{mod_name} (compatible)")
                    } else {
                        mod_name.clone()
                    };
                    ui.label(RichText::new(header).strong().color(Color32::LIGHT_GRAY));
                    for part in engines {
                        let label = format!("{} — {}", part.name, part.id);
                        if ui.selectable_label(current == part.id, label).clicked() {
                            apply_slot_change(state, slot, part.id.clone());
                        }
                    }
                    ui.separator();
                }
                if ui.selectable_label(false, "Custom…").clicked() {
                    state.slot_custom_mode.insert(slot.to_string());
                }
            });
    }
}

fn apply_slot_change(state: &mut AppState, slot: &str, value: String) {
    if state.slot_edits.get(slot) == Some(&value) {
        return;
    }
    if !state.slot_undo_pending.contains(slot) {
        state.push_undo(format!("Edit engine {slot}"));
        state.slot_undo_pending.insert(slot.to_string());
    }
    state.slot_edits.insert(slot.to_string(), value);
    state.dirty = true;
}

fn draw_diff_panel(ui: &mut Ui, state: &AppState, width: f32) {
    ui.set_width(width);
    ui.label(RichText::new("Changes").strong().color(Color32::WHITE));
    ui.add_space(6.0);

    if let (Some(orig), Some(edit)) = (&state.loaded_config, &state.edit_buffer) {
        let diffs: Vec<_> = edit
            .diff_summary(orig)
            .into_iter()
            .filter(|(slot, _, _)| is_engine_slot_name(slot))
            .collect();
        if diffs.is_empty() {
            ui.label(RichText::new("No engine changes yet.").color(Color32::LIGHT_GRAY));
        } else {
            for (slot, old, new) in diffs {
                ui.label(
                    RichText::new(format!(
                        "{}: {} → {}",
                        friendly_slot_label(&slot),
                        old,
                        new
                    ))
                    .color(Color32::from_rgb(220, 220, 220)),
                );
            }
        }
    }
}

fn draw_engine_mod_panel(ui: &mut Ui, state: &mut AppState, width: f32, panel_height: f32) {
    ui.set_width(width);
    ui.set_min_height(panel_height);
    ui.separator();
    ui.add_space(8.0);
    ui.label(RichText::new("Engine mods").strong().size(16.0).color(Color32::WHITE));
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Pick an engine mod, install an engine, or create an adapter mod for this car.",
        )
        .color(Color32::LIGHT_GRAY),
    );
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label(RichText::new("Search:").color(Color32::WHITE));
        ui.text_edit_singleline(&mut state.engine_search);
        if ui.button("Rescan engines").clicked() {
            state.request_parts_scan = true;
        }
    });
    ui.add_space(8.0);

    let engine_mods: Vec<_> = state.engine_mod_entries().into_iter().cloned().collect();

    if engine_mods.is_empty() && !state.parts_scanning {
        ui.label(
            RichText::new("No engine mods found. Rescan after setting your mods folder.")
                .color(Color32::LIGHT_GRAY),
        );
        return;
    }

    let current_model = state.current_vehicle_model();
    let list_height = (panel_height - 88.0).max(160.0);

    ui.allocate_ui_with_layout(
        Vec2::new(width, list_height),
        egui::Layout::left_to_right(egui::Align::TOP),
        |ui| {
            ui.set_height(list_height);

            ui.vertical(|ui| {
                ui.set_width(240.0);
                ui.set_height(list_height);
                ui.label(RichText::new("Mods").strong().color(Color32::WHITE));
                ui.add_space(4.0);
                ScrollArea::vertical()
                    .id_salt("editor_engine_mod_list")
                    .auto_shrink([false; 2])
                    .max_height(list_height - 24.0)
                    .show(ui, |ui| {
                        ui.set_width(228.0);
                        for em in &engine_mods {
                            let selected =
                                state.selected_engine_mod.as_deref() == Some(em.name.as_str());
                            let label = format!("{} ({})", em.name, em.engine_count);
                            if ui.selectable_label(selected, label).clicked() {
                                state.selected_engine_mod = Some(em.name.clone());
                            }
                        }
                    });
            });

            ui.add_space(16.0);

            ui.vertical(|ui| {
                ui.set_min_width((width - 280.0).max(320.0));
                ui.set_height(list_height);
                let Some(mod_entry) = state
                    .selected_engine_mod
                    .as_ref()
                    .and_then(|name| state.mod_by_name(name))
                    .cloned()
                else {
                    ui.label(RichText::new("Select a mod on the left.").color(Color32::LIGHT_GRAY));
                    return;
                };

                if let Some(model) = &current_model {
                    let built_in = mod_entry.target_vehicles.iter().any(|v| v == model);
                    let adapter_linked = state.adapter_supports_vehicle(&mod_entry.name, model);
                    let supported = built_in || adapter_linked;
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("This car: {model}"))
                                .color(Color32::WHITE),
                        );
                        if supported {
                            let note = if built_in {
                                "(built into mod)"
                            } else {
                                "(adapter mod)"
                            };
                            ui.label(RichText::new(note).color(success_color()));
                        } else if orange_button(ui, "Create adapter").clicked() {
                            state.request_add_car_to_mod_id = Some(mod_entry.id.clone());
                        }
                    });
                    ui.add_space(4.0);
                    if !supported {
                        ui.label(
                            RichText::new(format!(
                                "Creates vehicles/{model}/ in {}",
                                crate::mod_scanner::ADAPTER_MOD_FOLDER
                            ))
                            .color(Color32::LIGHT_GRAY)
                            .size(12.0),
                        );
                    }
                    ui.add_space(6.0);
                }

                let parts = filter_parts_for_mod_entry(
                    &state.parts_index,
                    &mod_entry.name,
                    &state.engine_search,
                );
                ui.label(
                    RichText::new(format!("{} — {} engines", mod_entry.name, parts.len()))
                        .strong()
                        .color(Color32::WHITE),
                );
                ui.add_space(4.0);

                ScrollArea::vertical()
                    .id_salt("editor_engine_parts_list")
                    .auto_shrink([false; 2])
                    .max_height(list_height - 72.0)
                    .show(ui, |ui| {
                        if parts.is_empty() {
                            ui.label(
                                RichText::new("No engines loaded for this mod — try Rescan engines.")
                                    .color(Color32::LIGHT_GRAY),
                            );
                        }
                        for part in parts {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&part.name).color(Color32::WHITE));
                                    ui.label(
                                        RichText::new(&part.id)
                                            .monospace()
                                            .size(11.0)
                                            .color(Color32::LIGHT_GRAY),
                                    );
                                });
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if orange_button(ui, "Use").clicked() {
                                            state.request_assign_engine = Some(part.id.clone());
                                        }
                                    },
                                );
                            });
                            ui.add_space(6.0);
                        }
                    });
            });
        },
    );
}
