use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::backup::{BackupIndex, BackupManager};
use crate::config::VehicleConfig;
use crate::engine::{EngineScanMessage, EngineScanner};
use crate::gui::{apply_dark_theme, draw_confirm_dialogs, draw_toasts, render_tab, top_bar};
use crate::scanner::{ScanMessage, VehicleScanner};
use crate::settings::AppSettings;
use crate::state::{AppState, Toast};

pub struct BeamNgVehicleEditor {
    state: AppState,
}

impl BeamNgVehicleEditor {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_dark_theme(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let settings = AppSettings::load().unwrap_or_default();
        let backup_index = BackupIndex::load(&settings).unwrap_or_default();

        let mut editor = Self {
            state: AppState::new(settings, backup_index),
        };

        if editor.state.settings.mods_path().is_some() {
            editor.state.request_rescan = true;
        }

        editor
    }

    fn clear_action_flags(&mut self) {
        self.state.request_rescan = false;
        self.state.request_mods_browse = false;
        self.state.request_exe_browse = false;
        self.state.request_save_settings = false;
        self.state.request_load_vehicle = None;
        self.state.request_apply = false;
        self.state.request_restore_latest = false;
        self.state.request_engine_scan = false;
        self.state.request_assign_engine = None;
        self.state.confirm_restore = false;
        self.state.confirm_restore_all = false;
    }

    fn process_actions(&mut self) {
        if self.state.request_mods_browse {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Select BeamNG mods folder (or vehicles subfolder)")
                .pick_folder()
            {
                self.state.settings.mods_vehicles_path = Some(path);
                let _ = self.state.settings.save();
                self.state.request_rescan = true;
            }
        }

        if self.state.request_exe_browse {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Select BeamNG executable")
                .add_filter("Executable", &["exe"])
                .pick_file()
            {
                self.state.settings.beamng_exe_path = Some(path);
                let _ = self.state.settings.save();
                self.state.push_toast(Toast::info("BeamNG exe path saved"));
            }
        }

        if self.state.request_save_settings {
            match self.state.settings.save() {
                Ok(()) => self.state.push_toast(Toast::info("Settings saved")),
                Err(e) => self.state.push_toast(Toast::error(e.to_string())),
            }
        }

        if self.state.request_rescan {
            self.start_scan();
        }

        if let Some(path) = self.state.request_load_vehicle.take() {
            self.load_vehicle(&path);
        }

        if self.state.request_engine_scan {
            self.start_engine_scan();
        }

        if let Some(engine_id) = self.state.request_assign_engine.take() {
            self.assign_engine(&engine_id);
        }

        if self.state.request_apply {
            self.apply_changes();
        }

        if self.state.request_restore_latest {
            self.restore_latest_for_current();
        }

        if self.state.confirm_restore {
            if let Some(meta) = self.state.pending_restore.take() {
                self.do_restore(&meta);
            }
        }

        if self.state.confirm_restore_all {
            self.do_restore_all();
        }

        self.clear_action_flags();
    }

    fn start_scan(&mut self) {
        let Some(root) = self.state.settings.mods_vehicles_path.clone() else {
            self.state.push_toast(Toast::error("Set mods folder in Settings first"));
            return;
        };

        self.state.scanning = true;
        self.state.vehicles.clear();
        self.state.thumbnails.clear();
        self.state.scan_rx = Some(VehicleScanner::spawn_scan(root));
        self.state.status_message = "Scanning…".to_string();
    }

    fn poll_scan(&mut self) {
        let mut finished = false;
        let mut messages = Vec::new();

        if let Some(rx) = &self.state.scan_rx {
            while let Ok(msg) = rx.try_recv() {
                messages.push(msg);
            }
        }

        for msg in messages {
            match msg {
                ScanMessage::Progress { current, message } => {
                    self.state.scan_progress = Some((current, message));
                }
                ScanMessage::Vehicle(v) => {
                    self.state.vehicles.push(v);
                }
                ScanMessage::Finished { total } => {
                    finished = true;
                    self.state.status_message = format!(
                        "Found {} .pc configs ({} vehicles loaded)",
                        total,
                        self.state.vehicles.len()
                    );
                    self.state.push_toast(Toast::info(format!(
                        "Scan complete: {} vehicles",
                        self.state.vehicles.len()
                    )));
                }
                ScanMessage::Error(err) => {
                    self.state.push_toast(Toast::error(err));
                }
            }
        }

        if finished {
            self.state.scanning = false;
            self.state.scan_rx = None;
            self.state.scan_progress = None;
        }
    }

    fn start_engine_scan(&mut self) {
        let Some(root) = self.state.settings.mods_vehicles_path.clone() else {
            self.state.push_toast(Toast::error("Set mods folder first"));
            return;
        };

        self.state.engine_scanning = true;
        self.state.engines.clear();
        self.state.engine_scan_rx = Some(EngineScanner::spawn_scan(root));
    }

    fn poll_engine_scan(&mut self) {
        let mut finished = false;
        let mut messages = Vec::new();

        if let Some(rx) = &self.state.engine_scan_rx {
            while let Ok(msg) = rx.try_recv() {
                messages.push(msg);
            }
        }

        for msg in messages {
            match msg {
                EngineScanMessage::Progress { scanned, message } => {
                    self.state.engine_scan_progress = Some((scanned, message));
                }
                EngineScanMessage::Engine(e) => {
                    self.state.engines.push(e);
                }
                EngineScanMessage::Finished { total } => {
                    finished = true;
                    self.state.push_toast(Toast::info(format!(
                        "Engine scan: {} engines from {total} jbeam files",
                        self.state.engines.len()
                    )));
                }
                EngineScanMessage::Error(err) => {
                    self.state.push_toast(Toast::error(err));
                }
            }
        }

        if finished {
            self.state.engine_scanning = false;
            self.state.engine_scan_rx = None;
            self.state.engine_scan_progress = None;
        }
    }

    fn load_vehicle(&mut self, path: &PathBuf) {
        match VehicleConfig::load(path) {
            Ok(config) => {
                self.state.loaded_config = Some(config.clone());
                self.state.edit_buffer = Some(config);
                self.state.sync_slot_edits();
                self.state.dirty = false;
                self.state.undo_stack.clear();
                self.state.redo_stack.clear();
                self.state.status_message = format!("Loaded {}", path.display());
                self.state.push_toast(Toast::info("Vehicle loaded"));
            }
            Err(e) => {
                self.state.push_toast(Toast::error(format!("Load failed: {e}")));
            }
        }
    }

    fn assign_engine(&mut self, engine_id: &str) {
        let Some(buf) = &self.state.edit_buffer else {
            return;
        };

        let slot = buf
            .engine_slot()
            .map(|(s, _)| s.to_string())
            .unwrap_or_else(|| "mainEngine".to_string());

        self.state.push_undo(format!("Assign engine {engine_id}"));
        self.state
            .slot_edits
            .insert(slot.clone(), engine_id.to_string());
        self.state.apply_slot_edits();
        self.state.dirty = true;
        self.state
            .push_toast(Toast::info(format!("Assigned {engine_id} to {slot}")));
    }

    fn apply_changes(&mut self) {
        self.state.apply_slot_edits();

        let Some(buf) = self.state.edit_buffer.clone() else {
            return;
        };

        let vehicle_name = self
            .state
            .selected_vehicle()
            .map(|v| v.name.clone())
            .unwrap_or_else(|| "vehicle".to_string());

        let vehicle_key = crate::config::vehicle_key_from_path(&buf.path);
        let existing = self.state.backup_index.backups_for(&vehicle_key).len();
        let save_path = buf.path.clone();

        if let Err(e) = BackupManager::ensure_initial_backup(
            &self.state.settings,
            &buf.path,
            &vehicle_name,
            &self.state.backup_index,
        ) {
            self.state
                .push_toast(Toast::error(format!("Initial backup failed: {e}")));
            return;
        }

        match BackupManager::create_backup(
            &self.state.settings,
            &buf.path,
            &vehicle_name,
            "before apply",
            existing,
        ) {
            Ok(meta) => {
                self.state
                    .backup_index
                    .by_vehicle
                    .entry(meta.vehicle_key.clone())
                    .or_default()
                    .push(meta);
            }
            Err(e) => {
                self.state
                    .push_toast(Toast::error(format!("Backup failed: {e}")));
                return;
            }
        }

        match buf.save() {
            Ok(()) => {
                self.state.loaded_config = Some(buf.clone());
                if let Some(edit) = &mut self.state.edit_buffer {
                    *edit = buf;
                }
                self.state.dirty = false;
                self.state.push_toast(Toast::info("Changes applied and saved"));
                self.state.status_message = format!("Saved {}", save_path.display());

                if let Some(exe) = &self.state.settings.beamng_exe_path {
                    if exe.exists() {
                        let _ = Command::new(exe).spawn();
                        self.state.push_toast(Toast::info("Launched BeamNG"));
                    }
                }
            }
            Err(e) => {
                self.state
                    .push_toast(Toast::error(format!("Save failed: {e}")));
            }
        }
    }

    fn restore_latest_for_current(&mut self) {
        let Some(key) = self.state.vehicle_key() else {
            return;
        };
        if let Some(meta) = self.state.backup_index.latest_for(&key).cloned() {
            self.do_restore(&meta);
        } else {
            self.state
                .push_toast(Toast::error("No backups for this vehicle"));
        }
    }

    fn do_restore(&mut self, meta: &crate::backup::BackupMetadata) {
        match BackupManager::restore_backup(meta) {
            Ok(()) => {
                self.state.push_toast(Toast::info(format!(
                    "Restored backup v{:03}",
                    meta.version
                )));
                if meta.original_path.exists() {
                    self.load_vehicle(&meta.original_path);
                }
            }
            Err(e) => {
                self.state
                    .push_toast(Toast::error(format!("Restore failed: {e}")));
            }
        }
    }

    fn do_restore_all(&mut self) {
        match BackupManager::restore_all(&self.state.backup_index) {
            Ok(results) => {
                let mut ok = 0usize;
                let mut fail = 0usize;
                for (_, r) in results {
                    if r.is_ok() {
                        ok += 1;
                    } else {
                        fail += 1;
                    }
                }
                self.state.push_toast(Toast::info(format!(
                    "Restore all: {ok} ok, {fail} failed"
                )));
                self.state.request_rescan = true;
            }
            Err(e) => {
                self.state
                    .push_toast(Toast::error(format!("Restore all failed: {e}")));
            }
        }
    }

    fn poll_external_changes(&mut self) {
        if !self.state.settings.hot_reload_external_changes {
            return;
        }
        if self.state.last_external_check.elapsed() < Duration::from_secs(2) {
            return;
        }
        self.state.last_external_check = Instant::now();

        if let Some(buf) = &mut self.state.edit_buffer {
            if !self.state.dirty {
                if let Ok(changed) = buf.reload_if_changed() {
                    if changed {
                        self.state.loaded_config = Some(buf.clone());
                        self.state.sync_slot_edits();
                        self.state.push_toast(Toast::info("Reloaded external changes"));
                    }
                }
            }
        }
    }

    fn handle_hotkeys(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if i.modifiers.command || i.modifiers.ctrl {
                if i.key_pressed(egui::Key::S) {
                    self.state.request_apply = true;
                }
                if i.key_pressed(egui::Key::Z) {
                    if i.modifiers.shift {
                        self.state.redo();
                    } else {
                        self.state.undo();
                    }
                }
            }
        });
    }
}

impl eframe::App for BeamNgVehicleEditor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_hotkeys(ctx);
        self.process_actions();
        self.poll_scan();
        self.poll_engine_scan();
        self.poll_external_changes();
        self.state.thumbnails.upload_pending(ctx);

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            top_bar(ui, &mut self.state);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            render_tab(ui, &mut self.state);
        });

        draw_confirm_dialogs(ctx, &mut self.state);
        draw_toasts(ctx, &mut self.state);

        if self.state.scanning || self.state.engine_scanning {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}
