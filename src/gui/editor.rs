use egui::{Color32, RichText, ScrollArea, Ui, Vec2};

use crate::gui::{accent_color, orange_button, success_color};
use crate::parts::{
    filter_parts_for_mod_entry, friendly_slot_label, CUSTOM_OPTION,
};
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
    let engine_panel_h = if state.show_engine_browser { 240.0 } else { 0.0 };
    let body_h = (avail_h - engine_panel_h).max(300.0);
    let main_w = (full_w - SIDEBAR_W - 24.0).max(400.0);

    ui.horizontal_top(|ui| {
        draw_left_panel(ui, state, &vehicle_name, body_h);

        ui.add_space(24.0);

        ui.allocate_ui_with_layout(
            Vec2::new(main_w, body_h),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                ui.set_width(main_w);
                draw_parts_panel(ui, state, main_w, body_h - 140.0);
                ui.add_space(16.0);
                draw_diff_panel(ui, state, main_w);
            },
        );
    });

    if state.show_engine_browser {
        ui.add_space(16.0);
        draw_engine_mod_panel(ui, state, full_w);
    }
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
        ui.add_space(8.0);
        if ui.button("Engine Mods").clicked() {
            state.show_engine_browser = !state.show_engine_browser;
            if state.parts_index.engines.is_empty() && !state.parts_scanning {
                state.request_parts_scan = true;
            }
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
                ui.label(RichText::new("Engine").strong().color(Color32::WHITE));
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
                    ui.label(RichText::new("Loading parts…").color(Color32::LIGHT_GRAY));
                });
            }
        },
    );
}

fn draw_parts_panel(ui: &mut Ui, state: &mut AppState, width: f32, scroll_h: f32) {
    ui.label(RichText::new("Parts & slots").strong().size(16.0).color(Color32::WHITE));
    ui.add_space(4.0);
    ui.label(
        RichText::new("Choose a part from the dropdown, or pick Custom to type an ID.")
            .color(Color32::LIGHT_GRAY)
            .size(13.0),
    );
    ui.add_space(12.0);

    ui.horizontal(|ui| {
        ui.label(RichText::new("Filter:").color(Color32::WHITE));
        ui.add(
            egui::TextEdit::singleline(&mut state.slot_filter)
                .hint_text("Search slots…")
                .desired_width(240.0),
        );
    });
    ui.add_space(16.0);

    let slots = filtered_slots(state);
    let combo_w = (width - 8.0).max(280.0);

    ScrollArea::vertical()
        .auto_shrink([false; 2])
        .max_height(scroll_h.max(200.0))
        .show(ui, |ui| {
            ui.set_width(width);
            for slot in slots {
                draw_slot_row(ui, state, &slot, combo_w);
                ui.add_space(20.0);
            }
        });
}

fn filtered_slots(state: &AppState) -> Vec<String> {
    let q = state.slot_filter.trim().to_ascii_lowercase();
    let mut slots: Vec<String> = state.slot_edits.keys().cloned().collect();
    slots.sort();
    if q.is_empty() {
        return slots;
    }
    slots
        .into_iter()
        .filter(|s| {
            s.to_ascii_lowercase().contains(&q)
                || friendly_slot_label(s).to_ascii_lowercase().contains(&q)
        })
        .collect()
}

fn draw_slot_row(ui: &mut Ui, state: &mut AppState, slot: &str, combo_w: f32) {
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
        ui.label(RichText::new("Custom part ID").color(Color32::LIGHT_GRAY).size(12.0));
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
        let options = state.parts_index.dropdown_options(slot, &current);
        let selected_text = if current.is_empty() {
            "(none)".to_string()
        } else if let Some(p) = state.parts_index.all_by_id.get(&current) {
            format!("{} ({})", p.name, p.mod_name)
        } else {
            current.clone()
        };

        egui::ComboBox::from_id_salt(format!("slot_{slot}"))
            .selected_text(RichText::new(selected_text).color(Color32::WHITE))
            .width(combo_w)
            .show_ui(ui, |ui| {
                for part in &options {
                    let label = format!("{} — {}", part.name, part.mod_name);
                    if ui.selectable_label(current == part.id, label).clicked() {
                        apply_slot_change(state, slot, part.id.clone());
                    }
                }
                ui.separator();
                if ui.selectable_label(false, "Custom…").clicked() {
                    state.slot_custom_mode.insert(slot.to_string());
                }
            });
    }

    ui.add_space(2.0);
    ui.separator();
}

fn apply_slot_change(state: &mut AppState, slot: &str, value: String) {
    if state.slot_edits.get(slot) == Some(&value) {
        return;
    }
    if !state.slot_undo_pending.contains(slot) {
        state.push_undo(format!("Edit slot {slot}"));
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
        let diffs = edit.diff_summary(orig);
        if diffs.is_empty() {
            ui.label(RichText::new("No changes yet.").color(Color32::LIGHT_GRAY));
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

fn draw_engine_mod_panel(ui: &mut Ui, state: &mut AppState, width: f32) {
    ui.set_width(width);
    ui.separator();
    ui.add_space(12.0);
    ui.label(RichText::new("Engine mods").strong().size(16.0).color(Color32::WHITE));
    ui.add_space(4.0);
    ui.label(
        RichText::new("Select a mod, install an engine, or add this car to the mod.")
            .color(Color32::LIGHT_GRAY),
    );
    ui.add_space(12.0);

    ui.horizontal(|ui| {
        ui.label(RichText::new("Search:").color(Color32::WHITE));
        ui.text_edit_singleline(&mut state.engine_search);
        if ui.button("Rescan parts").clicked() {
            state.request_parts_scan = true;
        }
    });
    ui.add_space(12.0);

    let engine_mods: Vec<_> = state.engine_mod_entries().into_iter().cloned().collect();

    if engine_mods.is_empty() && !state.parts_scanning {
        ui.label(
            RichText::new("No engine mods found. Rescan after setting your mods folder.")
                .color(Color32::LIGHT_GRAY),
        );
        return;
    }

    let current_model = state.current_vehicle_model();

    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(220.0);
            ui.label(RichText::new("Mods").strong().color(Color32::WHITE));
            ScrollArea::vertical()
                .max_height(180.0)
                .show(ui, |ui| {
                    for em in &engine_mods {
                        let selected =
                            state.selected_engine_mod.as_deref() == Some(em.name.as_str());
                        let kind = if em.kind == crate::mod_scanner::ModKind::EngineParts {
                            "Engine"
                        } else {
                            "Parts"
                        };
                        let label = format!("{} [{kind}] ({})", em.name, em.engine_count);
                        if ui.selectable_label(selected, label).clicked() {
                            state.selected_engine_mod = Some(em.name.clone());
                        }
                    }
                });
        });

        ui.add_space(24.0);

        ui.vertical(|ui| {
            ui.set_min_width((width - 260.0).max(300.0));
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
                let already_supported = mod_entry.target_vehicles.iter().any(|v| v == model);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("This car: {model}"))
                            .color(Color32::WHITE),
                    );
                    if already_supported {
                        ui.label(RichText::new("(already supported)").color(success_color()));
                    } else if orange_button(ui, "Add car to mod").clicked() {
                        state.request_add_car_to_mod_id = Some(mod_entry.id.clone());
                    }
                });
                ui.add_space(8.0);
            }

            if !crate::mod_scanner::is_editable(&mod_entry) {
                ui.label(
                    RichText::new("This mod is packed — unpack it on the Mods tab before editing.")
                        .color(accent_color()),
                );
                if ui.button("Go to Mods tab").clicked() {
                    state.tab = crate::state::AppTab::Mods;
                    state.selected_mod_id = Some(mod_entry.id.clone());
                }
                ui.add_space(8.0);
            }

            let parts =
                filter_parts_for_mod_entry(&state.parts_index, &mod_entry.name, &state.engine_search);
            ui.label(
                RichText::new(format!("{} — {} engines", mod_entry.name, parts.len()))
                    .strong()
                    .color(Color32::WHITE),
            );
            ui.add_space(8.0);

            ScrollArea::vertical()
                .max_height(180.0)
                .show(ui, |ui| {
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
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if orange_button(ui, "Use").clicked() {
                                    state.request_assign_engine = Some(part.id.clone());
                                }
                            });
                        });
                        ui.add_space(8.0);
                    }
                });
        });
    });
}

#[allow(dead_code)]
const _CUSTOM: &str = CUSTOM_OPTION;
