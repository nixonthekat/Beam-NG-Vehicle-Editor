use egui::{RichText, ScrollArea, Ui, Vec2};

use crate::config::ENGINE_SLOT_NAMES;
use crate::engine::filter_engines;
use crate::gui::{accent_color, success_color};
use crate::state::{AppState, AppTab};

pub fn draw_editor(ui: &mut Ui, state: &mut AppState) {
    if state.edit_buffer.is_none() {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.heading("Select a vehicle from the Grid tab");
            if ui.button("Go to Grid").clicked() {
                state.tab = AppTab::Grid;
            }
        });
        return;
    }

    ui.horizontal(|ui| {
        if ui.button("Apply Changes").clicked() {
            state.request_apply = true;
        }
        if ui.button("Restore Latest").clicked() {
            state.request_restore_latest = true;
        }
        if ui.button("Browse Engines").clicked() {
            state.show_engine_browser = !state.show_engine_browser;
            if state.show_engine_browser && state.engines.is_empty() {
                state.request_engine_scan = true;
            }
        }
        ui.separator();
        let dirty_label = if state.dirty { "● Unsaved" } else { "Saved" };
        ui.label(RichText::new(dirty_label).color(if state.dirty {
            accent_color()
        } else {
            success_color()
        }));
    });

    ui.add_space(8.0);

    let vehicle_name = state
        .selected_vehicle()
        .map(|v| v.name.clone())
        .unwrap_or_else(|| "Vehicle".to_string());

    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(280.0);
            ui.heading(&vehicle_name);

            if let Some(vehicle) = state.selected_vehicle().cloned() {
                if let Some(path) = &vehicle.thumbnail_path {
                    let thumb_id = format!("editor_{}", vehicle.id);
                    state.thumbnails.queue_load(&thumb_id, path);
                    if let Some(tex) = state.thumbnails.get(&thumb_id) {
                        ui.image((tex.id(), Vec2::new(260.0, 180.0)));
                    }
                }
            }

            if let Some((slot, part)) = state.edit_buffer.as_ref().and_then(|c| c.engine_slot()) {
                ui.add_space(8.0);
                ui.group(|ui| {
                    ui.label(RichText::new("Engine slot").strong());
                    ui.label(format!("Slot: {slot}"));
                    ui.label(format!("Part: {part}"));
                });
            }
        });

        ui.separator();

        ui.vertical(|ui| {
            ui.heading("Parts / Slots");
            ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    let slots: Vec<String> = state.slot_edits.keys().cloned().collect();
                    for slot in slots {
                        let mut value = state.slot_edits.get(&slot).cloned().unwrap_or_default();
                        let is_engine = ENGINE_SLOT_NAMES
                            .iter()
                            .any(|s| s.eq_ignore_ascii_case(&slot))
                            || slot.to_ascii_lowercase().contains("engine");

                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&slot).monospace());
                            if is_engine {
                                ui.label(RichText::new("⚙").color(accent_color()));
                            }
                            let response = ui.add(
                                egui::TextEdit::singleline(&mut value)
                                    .desired_width(220.0),
                            );
                            if response.changed() {
                                let prev = state.slot_edits.get(&slot).cloned();
                                if prev.as_deref() != Some(value.as_str()) {
                                    if prev.is_some() {
                                        state.push_undo(format!("Edit slot {slot}"));
                                    }
                                    state.slot_edits.insert(slot.clone(), value);
                                    state.dirty = true;
                                }
                            }
                        });
                        ui.add_space(4.0);
                    }
                });

            ui.add_space(12.0);
            ui.heading("Config diff (vs loaded)");
            ScrollArea::vertical()
                .max_height(150.0)
                .show(ui, |ui| {
                    if let (Some(orig), Some(edit)) =
                        (&state.loaded_config, &state.edit_buffer)
                    {
                        let diffs = edit.diff_summary(orig);
                        if diffs.is_empty() {
                            ui.label(RichText::new("No changes").weak());
                        } else {
                            for (slot, old, new) in diffs {
                                ui.label(format!("{slot}: {old} → {new}"));
                            }
                        }
                    }
                });
        });
    });

    if state.show_engine_browser {
        draw_engine_browser(ui, state);
    }
}

fn draw_engine_browser(ui: &mut Ui, state: &mut AppState) {
    ui.add_space(12.0);
    ui.separator();
    ui.heading("Engine Browser");

    if state.engine_scanning {
        ui.horizontal(|ui| {
            ui.spinner();
            if let Some((n, msg)) = &state.engine_scan_progress {
                ui.label(format!("Scanning jbeam files… {n} — {msg}"));
            }
        });
    }

    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(&mut state.engine_search);
        if ui.button("Rescan engines").clicked() {
            state.request_engine_scan = true;
        }
    });

    let engines = filter_engines(
        &state.engines,
        &state.engine_search,
        state.engine_mod_filter.as_deref(),
    );

    ui.label(format!("{} engines found", engines.len()));

    ScrollArea::vertical()
        .max_height(250.0)
        .show(ui, |ui| {
            for engine in engines {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&engine.name).strong());
                        ui.label(
                            RichText::new(format!("{} · {}", engine.id, engine.mod_name))
                                .weak()
                                .size(11.0),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Assign").clicked() {
                            state.request_assign_engine = Some(engine.id.clone());
                        }
                    });
                });
                ui.separator();
            }
        });
}
