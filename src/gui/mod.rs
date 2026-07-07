mod backup_ui;
mod editor;
mod grid;
mod theme;

pub use theme::{accent_color, apply_dark_theme, card_stroke, error_color, success_color};

use egui::{Align, Layout, RichText, Ui};

use crate::state::{AppState, AppTab};

pub fn top_bar(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.heading(RichText::new("BeamNG Vehicle Editor").strong().size(18.0));
        ui.separator();
        tab_button(ui, state, AppTab::Grid, "Grid");
        tab_button(ui, state, AppTab::Editor, "Editor");
        tab_button(ui, state, AppTab::Backups, "Backups");
        tab_button(ui, state, AppTab::Settings, "Settings");

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if state.scanning {
                ui.spinner();
                if let Some((n, msg)) = &state.scan_progress {
                    ui.label(format!("Scanning… {n} — {msg}"));
                }
            } else {
                ui.label(RichText::new(&state.status_message).weak());
            }
            if ui.button("Rescan").clicked() {
                state.request_rescan = true;
            }
        });
    });
}

fn tab_button(ui: &mut Ui, state: &mut AppState, tab: AppTab, label: &str) {
    let selected = state.tab == tab;
    if ui
        .selectable_label(selected, RichText::new(label).strong())
        .clicked()
    {
        state.tab = tab;
    }
}

pub fn draw_toasts(ctx: &egui::Context, state: &mut AppState) {
    state.toasts.retain(|t| !t.expired());
    if state.toasts.is_empty() {
        return;
    }

    egui::Area::new(egui::Id::new("toasts"))
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0))
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
        AppTab::Editor => editor::draw_editor(ui, state),
        AppTab::Backups => backup_ui::draw_backups(ui, state),
        AppTab::Settings => draw_settings(ui, state),
    }
}

fn draw_settings(ui: &mut Ui, state: &mut AppState) {
    ui.heading("Settings");
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label("Mods / vehicles folder:");
        let path_text = state
            .settings
            .mods_vehicles_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(not set)".to_string());
        ui.label(RichText::new(path_text).monospace());
        if ui.button("Browse…").clicked() {
            state.request_mods_browse = true;
        }
    });

    ui.horizontal(|ui| {
        ui.label("BeamNG executable (optional):");
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
    });

    ui.checkbox(
        &mut state.settings.hot_reload_external_changes,
        "Hot-reload when .pc file changes externally",
    );

    if ui.button("Save Settings").clicked() {
        state.request_save_settings = true;
    }

    ui.add_space(16.0);
    ui.separator();
    ui.label(RichText::new("Typical paths").strong());
    ui.label("Windows: …\\Documents\\BeamNG.drive\\mods");
    ui.label("Also scan: …\\BeamNG.drive\\mods\\unpacked or vehicle mod folders");
    ui.label("Backups stored in app config dir under backups/");
}
