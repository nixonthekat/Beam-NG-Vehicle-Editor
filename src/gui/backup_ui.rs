use egui::{RichText, ScrollArea, Ui};

use crate::backup::{format_size, BackupMetadata};
use crate::gui::{accent_color, success_color};
use crate::state::AppState;

pub fn draw_backups(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.heading("Backup & Restore");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(RichText::new("Restore All (latest per vehicle)").color(accent_color()))
                .clicked()
            {
                state.pending_restore_all = true;
            }
        });
    });

    ui.label(
        RichText::new("Backups are never auto-deleted. Every apply creates a versioned copy.")
            .weak(),
    );
    ui.add_space(8.0);

    if state.backup_index.by_vehicle.is_empty() {
        ui.label("No backups yet. Edit a vehicle to create automatic backups.");
        return;
    }

    let vehicle_keys: Vec<String> = state.backup_index.by_vehicle.keys().cloned().collect();

    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(200.0);
            ui.heading("Vehicles");
            ScrollArea::vertical().show(ui, |ui| {
                for key in &vehicle_keys {
                    let count = state.backup_index.backups_for(key).len();
                    let selected = state.backup_vehicle_filter.as_deref() == Some(key.as_str());
                    if ui
                        .selectable_label(selected, format!("{key} ({count})"))
                        .clicked()
                    {
                        state.backup_vehicle_filter = Some(key.clone());
                    }
                }
                if ui.selectable_label(state.backup_vehicle_filter.is_none(), "All").clicked() {
                    state.backup_vehicle_filter = None;
                }
            });
        });

        ui.separator();

        ui.vertical(|ui| {
            ScrollArea::vertical().show(ui, |ui| {
                let entries: Vec<BackupMetadata> = if let Some(ref filter) = state.backup_vehicle_filter {
                    state
                        .backup_index
                        .backups_for(filter)
                        .iter()
                        .cloned()
                        .collect()
                } else {
                    state
                        .backup_index
                        .all_backups()
                        .into_iter()
                        .cloned()
                        .collect()
                };

                if entries.is_empty() {
                    ui.label("No backups for selection.");
                    return;
                }

                for meta in entries.iter().rev() {
                    draw_backup_entry(ui, state, meta);
                    ui.add_space(8.0);
                }
            });
        });
    });
}

fn draw_backup_entry(ui: &mut Ui, state: &mut AppState, meta: &BackupMetadata) {
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(28, 28, 34))
        .inner_margin(12.0)
        .rounding(6.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    let local_time = meta.created_at.with_timezone(&chrono::Local);
                    ui.label(
                        RichText::new(local_time.format("%Y-%m-%d %H:%M:%S").to_string())
                            .strong(),
                    );
                    ui.label(format!("Vehicle: {}", meta.vehicle_name));
                    ui.label(format!("Version: v{:03}", meta.version));
                    ui.label(format!("Size: {}", format_size(meta.file_size)));
                    ui.label(RichText::new(&meta.reason).weak().size(11.0));
                    ui.label(
                        RichText::new(meta.backup_path.display().to_string())
                            .monospace()
                            .size(10.0)
                            .weak(),
                    );
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(RichText::new("Restore").size(16.0).color(success_color()))
                        .clicked()
                    {
                        state.pending_restore = Some(meta.clone());
                    }
                });
            });
        });
}
