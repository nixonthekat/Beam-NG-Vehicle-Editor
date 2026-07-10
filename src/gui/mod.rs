mod backup_ui;
mod editor;
mod grid;
mod mods_ui;
mod theme;

pub use theme::{
    accent_color, apply_dark_theme, card_bg, card_stroke, error_color, nav_tab,
    orange_button, success_color, truncate_label,
};

use egui::{Align, Color32, Layout, RichText, Ui};

use crate::settings::AppSettings;
use crate::state::{AppState, AppTab};

pub fn top_bar(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.heading(RichText::new("BeamNG Vehicle Editor").strong().size(18.0));
            ui.label(
                RichText::new("by nixonthekat")
                    .weak()
                    .size(11.0)
                    .color(Color32::from_rgb(180, 180, 180)),
            );
        });
        ui.add_space(8.0);
        ui.separator();
        if nav_tab(ui, "Grid", state.tab == AppTab::Grid).clicked() {
            state.tab = AppTab::Grid;
        }
        if nav_tab(ui, "Mods", state.tab == AppTab::Mods).clicked() {
            state.tab = AppTab::Mods;
        }
        if nav_tab(ui, "Editor", state.tab == AppTab::Editor).clicked() {
            state.tab = AppTab::Editor;
        }
        if nav_tab(ui, "Backups", state.tab == AppTab::Backups).clicked() {
            state.tab = AppTab::Backups;
        }
        if nav_tab(ui, "Settings", state.tab == AppTab::Settings).clicked() {
            state.tab = AppTab::Settings;
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if state.scanning {
                ui.spinner();
                if let Some((n, msg)) = &state.scan_progress {
                    ui.label(format!("Scanning… {n} — {msg}"));
                }
            } else {
                ui.label(RichText::new(&state.status_message).weak());
            }
            if orange_button(ui, "Rescan").clicked() {
                state.request_rescan = true;
            }
        });
    });
}

pub fn draw_drive_button(ctx: &egui::Context, state: &mut AppState) {
    egui::Area::new(egui::Id::new("drive_button"))
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-20.0, -20.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            if orange_button(ui, "Drive").on_hover_text("Launch BeamNG").clicked() {
                state.request_drive = true;
            }
        });
}

pub fn draw_toasts(ctx: &egui::Context, state: &mut AppState) {
    state.toasts.retain(|t| !t.expired());
    if state.toasts.is_empty() {
        return;
    }

    egui::Area::new(egui::Id::new("toasts"))
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -72.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.with_layout(Layout::top_down(egui::Align::RIGHT), |ui| {
                for toast in state.toasts.iter().rev() {
                    let color = if toast.is_error {
                        error_color()
                    } else {
                        success_color()
                    };
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(30, 30, 36))
                        .stroke(egui::Stroke::new(1.0, color))
                        .inner_margin(10.0)
                        .rounding(6.0)
                        .show(ui, |ui| {
                            ui.label(RichText::new(&toast.message).color(color));
                        });
                }
            });
        });
}

pub fn draw_confirm_dialogs(ctx: &egui::Context, state: &mut AppState) {
    if let Some(meta) = state.pending_restore.clone() {
        let mut open = true;
        egui::Window::new("Confirm Restore")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(format!(
                    "Restore backup from {}?",
                    meta.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                ));
                ui.label(format!("Vehicle: {}", meta.vehicle_name));
                ui.label(format!(
                    "This will overwrite: {}",
                    meta.original_path.display()
                ));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Restore").color(success_color())).clicked() {
                        state.confirm_restore = true;
                        state.pending_restore = None;
                    }
                    if ui.button("Cancel").clicked() {
                        state.pending_restore = None;
                    }
                });
            });
        if !open {
            state.pending_restore = None;
        }
    }

    if state.pending_restore_all {
        let mut open = true;
        egui::Window::new("Restore All Backups")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("Restore the latest backup for every vehicle?");
                ui.label(RichText::new("This cannot be undone automatically.").weak());
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Restore All").color(success_color())).clicked() {
                        state.confirm_restore_all = true;
                        state.pending_restore_all = false;
                    }
                    if ui.button("Cancel").clicked() {
                        state.pending_restore_all = false;
                    }
                });
            });
        if !open {
            state.pending_restore_all = false;
        }
    }
}

pub fn render_tab(ui: &mut Ui, state: &mut AppState) {
    match state.tab {
        AppTab::Grid => grid::draw_grid(ui, state),
        AppTab::Mods => mods_ui::draw_mods(ui, state),
        AppTab::Editor => editor::draw_editor(ui, state),
        AppTab::Backups => backup_ui::draw_backups(ui, state),
        AppTab::Settings => draw_settings(ui, state),
    }
}

fn draw_settings(ui: &mut Ui, state: &mut AppState) {
    ui.heading("Settings");
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label("BeamNG user folder (current):");
        let path_text = state
            .settings
            .effective_user_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(not set)".to_string());
        ui.label(RichText::new(path_text).monospace());
        if ui.button("Browse…").clicked() {
            state.request_user_browse = true;
        }
        if ui.button("Open").clicked() {
            if let Some(path) = state.settings.effective_user_dir() {
                match AppSettings::open_in_file_manager(&path) {
                    Ok(()) => {}
                    Err(e) => state.push_toast(crate::state::Toast::error(e.to_string())),
                }
            } else {
                state.push_toast(crate::state::Toast::error("User folder not set"));
            }
        }
    });

    ui.horizontal(|ui| {
        ui.label("Mods folder (optional override):");
        let path_text = state
            .settings
            .mods_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(auto from user folder)".to_string());
        ui.label(RichText::new(path_text).monospace());
        if ui.button("Browse…").clicked() {
            state.request_mods_browse = true;
        }
        if ui.button("Open").clicked() {
            if let Some(path) = state.settings.mods_dir() {
                match AppSettings::open_in_file_manager(&path) {
                    Ok(()) => {}
                    Err(e) => state.push_toast(crate::state::Toast::error(e.to_string())),
                }
            } else {
                state.push_toast(crate::state::Toast::error("Mods folder not found"));
            }
        }
    });

    ui.horizontal(|ui| {
        ui.label("BeamNG executable:");
        let exe_text = state
            .settings
            .beamng_exe_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(not set)".to_string());
        ui.label(RichText::new(exe_text).monospace());
        if ui.button("Browse…").clicked() {
            state.request_exe_browse = true;
        }
        if ui.button("Open").clicked() {
            if let Some(path) = state.settings.beamng_exe_path.as_ref() {
                let open_path = path.parent().unwrap_or(path.as_path());
                match AppSettings::open_in_file_manager(open_path) {
                    Ok(()) => {}
                    Err(e) => state.push_toast(crate::state::Toast::error(e.to_string())),
                }
            } else {
                state.push_toast(crate::state::Toast::error("BeamNG exe not set"));
            }
        }
    });

    ui.checkbox(
        &mut state.settings.hot_reload_external_changes,
        "Hot-reload when .pc file changes externally",
    );

    if ui.button("Save Settings").clicked() {
        state.request_save_settings = true;
    }

    if let Some(game_dir) = state.settings.game_vehicles_dir() {
        ui.label(RichText::new(format!("Stock vehicle packs: {}", game_dir.display())).weak());
    }
    if let Some(loose) = state.settings.game_loose_vehicles_dir() {
        ui.label(RichText::new(format!("Game loose vehicles: {}", loose.display())).weak());
    }
    if let Some(saved) = state.settings.saved_vehicles_dir() {
        ui.label(RichText::new(format!("Saved configs: {}", saved.display())).weak());
    }
    if !state.settings.can_scan() {
        ui.label(
            RichText::new("Set user folder and/or BeamNG.exe to scan vehicles")
                .color(error_color()),
        );
    }

    ui.add_space(16.0);
    ui.separator();
    ui.label(RichText::new("Typical paths").strong());
    ui.label("%LocalAppData%\\BeamNG\\BeamNG.drive\\current — user folder (mods, saved configs)");
    ui.label("…\\current\\mods\\packed — zip mods (BeamNG default)");
    ui.label("…\\current\\mods\\unpacked — extracted mods");
    ui.label("…\\current\\vehicles — your saved .pc configs");
    ui.label("Steam\\…\\BeamNG.drive\\Bin64\\BeamNG.exe — stock vehicles from content/vehicles/*.zip");
    ui.label("Backups stored in app config dir under backups/");
}
