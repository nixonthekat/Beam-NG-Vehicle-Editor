use std::process::Command;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::backup::{BackupIndex, BackupManager};
use crate::config::VehicleConfig;
use crate::mod_scanner::{ModScanMessage, ModScanner};
use crate::parts::{PartsScanMessage, PartsScanner};
use crate::gui::{apply_dark_theme, draw_confirm_dialogs, draw_drive_button, draw_toasts, render_tab, top_bar};
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

        if editor.state.settings.can_scan() {
            editor.state.request_rescan = true;
        }

        editor
    }

    fn clear_action_flags(&mut self) {
        self.state.request_user_browse = false;
        self.state.request_rescan = false;
        self.state.request_mods_browse = false;
        self.state.request_exe_browse = false;
        self.state.request_save_settings = false;
        self.state.request_load_vehicle_id = None;
        self.state.request_apply = false;
        self.state.request_restore_latest = false;
        self.state.request_parts_scan = false;
        self.state.request_engine_scan = false;
        self.state.request_assign_engine = None;
        self.state.request_unpack_mod_id = None;
        self.state.request_pack_mod_id = None;
        self.state.request_add_car_to_mod_id = None;
        self.state.confirm_restore = false;
        self.state.confirm_restore_all = false;
        self.state.request_drive = false;
    }

    fn process_actions(&mut self) {
        if self.state.request_user_browse {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Select BeamNG user folder (…/BeamNG.drive/current)")
                .pick_folder()
            {
                self.state.settings.beamng_user_path = Some(path);
                let _ = self.state.settings.save();
                self.state.request_rescan = true;
            }
        }

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
                self.state.push_toast(Toast::info("BeamNG exe saved — rescanning for stock vehicles"));
                self.state.request_rescan = true;
            }
        }

        if self.state.request_drive {
            self.launch_beamng();
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

        if let Some(id) = self.state.request_load_vehicle_id.take() {
            self.load_vehicle_by_id(&id);
        }

        if self.state.request_parts_scan || self.state.request_engine_scan {
            self.start_parts_scan();
        }

        if let Some(engine_id) = self.state.request_assign_engine.take() {
            self.assign_engine(&engine_id);
        }

        if let Some(mod_id) = self.state.request_unpack_mod_id.take() {
            self.unpack_mod_by_id(&mod_id);
        }

        if let Some(mod_id) = self.state.request_pack_mod_id.take() {
            self.pack_mod_by_id(&mod_id);
        }

        if let Some(mod_id) = self.state.request_add_car_to_mod_id.take() {
            self.add_car_to_mod_by_id(&mod_id);
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
        if !self.state.settings.can_scan() {
            self.state.push_toast(Toast::error(
                "Set BeamNG user folder or BeamNG.exe in Settings first",
            ));
            return;
        }

        self.state.scanning = true;
        self.state.mod_scanning = true;
        self.state.vehicles.clear();
        self.state.mods.clear();
        self.state.thumbnails.clear();
        self.state.scan_rx = Some(VehicleScanner::spawn_scan(self.state.settings.clone()));
        self.state.mod_scan_rx = Some(ModScanner::spawn_scan(self.state.settings.clone()));
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
                ScanMessage::Finished { total, skipped } => {
                    finished = true;
                    self.state.status_message = format!(
                        "Found {} .pc configs ({} vehicles loaded)",
                        total,
                        self.state.vehicles.len()
                    );
                    if skipped > 0 {
                        self.state.push_toast(Toast::info(format!(
                            "Scan complete: {} vehicles ({} configs skipped)",
                            self.state.vehicles.len(),
                            skipped
                        )));
                    } else {
                        self.state.push_toast(Toast::info(format!(
                            "Scan complete: {} vehicles",
                            self.state.vehicles.len()
                        )));
                    }
                    if !self.state.parts_scan_done
                        && !self.state.parts_scanning
                        && self.state.parts_index.all_by_id.is_empty()
                    {
                        self.state.request_parts_scan = true;
                    }
                }
                ScanMessage::Skipped => {}
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

    fn poll_mod_scan(&mut self) {
        let mut finished = false;
        let mut messages = Vec::new();

        if let Some(rx) = &self.state.mod_scan_rx {
            while let Ok(msg) = rx.try_recv() {
                messages.push(msg);
            }
        }

        for msg in messages {
            match msg {
                ModScanMessage::Progress { current, message } => {
                    self.state.mod_scan_progress = Some((current, message));
                }
                ModScanMessage::Mod(m) => {
                    self.state.mods.push(m);
                }
                ModScanMessage::Finished { total: _ } => {
                    finished = true;
                    self.state.mods.sort_by(|a, b| a.name.cmp(&b.name));
                    if self.state.selected_mod_id.is_none() {
                        if let Some(first) = self.state.mods.first() {
                            self.state.selected_mod_id = Some(first.id.clone());
                        }
                    }
                    if self.state.selected_engine_mod.is_none() {
                        if let Some(first) = self.state.engine_mod_entries().first() {
                            self.state.selected_engine_mod = Some(first.name.clone());
                        }
                    }
                }
                ModScanMessage::Error(err) => {
                    self.state.push_toast(Toast::error(err));
                }
            }
        }

        if finished {
            self.state.mod_scanning = false;
            self.state.mod_scan_rx = None;
            self.state.mod_scan_progress = None;
        }
    }

    fn start_parts_scan(&mut self) {
        if !self.state.settings.can_scan() {
            self.state
                .push_toast(Toast::error("Set BeamNG user folder or BeamNG.exe in Settings"));
            return;
        }
        if self.state.parts_scanning {
            return;
        }

        self.state.parts_scanning = true;
        self.state.parts_scan_done = false;
        self.state.parts_collect.clear();
        self.state.parts_scan_rx = Some(PartsScanner::spawn_scan(self.state.settings.clone()));
    }

    fn poll_parts_scan(&mut self) {
        let mut finished = false;
        let mut errored = false;
        let mut messages = Vec::new();

        if let Some(rx) = &self.state.parts_scan_rx {
            while let Ok(msg) = rx.try_recv() {
                messages.push(msg);
            }
        }

        for msg in messages {
            match msg {
                PartsScanMessage::Progress { scanned, message } => {
                    self.state.parts_scan_progress = Some((scanned, message));
                }
                PartsScanMessage::Part(p) => {
                    self.state.parts_collect.push(p);
                }
                PartsScanMessage::Finished { total } => {
                    finished = true;
                    self.state.parts_index = crate::parts::PartsIndex::finalize_from_parts(
                        std::mem::take(&mut self.state.parts_collect),
                    );
                    let engine_count = self.state.parts_index.engines.len();
                    self.state.push_toast(Toast::info(format!(
                        "Loaded {engine_count} engines from mods"
                    )));
                    if self.state.selected_engine_mod.is_none() {
                        if let Some(first) = self.state.engine_mod_entries().first() {
                            self.state.selected_engine_mod = Some(first.name.clone());
                        }
                    }
                    let _ = total;
                }
                PartsScanMessage::Error(err) => {
                    errored = true;
                    self.state.push_toast(Toast::error(err));
                }
            }
        }

        if finished || errored {
            self.state.parts_scanning = false;
            self.state.parts_scan_done = true;
            self.state.parts_scan_rx = None;
            self.state.parts_scan_progress = None;
        }
    }

    #[allow(dead_code)]
    fn start_engine_scan(&mut self) {
        self.start_parts_scan();
    }

    #[allow(dead_code)]
    fn poll_engine_scan(&mut self) {
        self.poll_parts_scan();
    }

    fn load_vehicle_by_id(&mut self, id: &str) {
        let Some(entry) = self.state.vehicles.iter().find(|v| v.id == id).cloned() else {
            self.state
                .push_toast(Toast::error(format!("Vehicle not found: {id}")));
            return;
        };

        let mut location = entry.location.clone();
        let path = match location.ensure_local_path(&self.state.settings) {
            Ok(p) => p,
            Err(e) => {
                self.state
                    .push_toast(Toast::error(format!("Extract/load failed: {e}")));
                return;
            }
        };

        match VehicleConfig::load(&path) {
            Ok(config) => {
                self.state.loaded_config = Some(config.clone());
                self.state.edit_buffer = Some(config);
                self.state.loaded_location = Some(location);
                self.state.sync_slot_edits();
                self.state.slot_undo_pending.clear();
                self.state.slot_custom_mode.clear();
                self.state.slot_filter.clear();
                self.state.dirty = false;
                self.state.undo_stack.clear();
                self.state.redo_stack.clear();
                self.state.status_message = format!("Loaded {}", entry.name);
                self.state.push_toast(Toast::info(format!("Editing {}", entry.name)));
                self.state.tab = crate::state::AppTab::Editor;
                if !self.state.parts_scan_done
                    && !self.state.parts_scanning
                    && self.state.parts_index.all_by_id.is_empty()
                {
                    self.state.request_parts_scan = true;
                }
            }
            Err(e) => {
                self.state
                    .push_toast(Toast::error(format!("Load failed: {e}")));
            }
        }
    }

    fn add_car_to_mod_by_id(&mut self, mod_id: &str) {
        let Some(mod_entry) = self.state.mods.iter().find(|m| m.id == mod_id).cloned() else {
            self.state.push_toast(Toast::error("Mod not found"));
            return;
        };
        let Some(vehicle) = self.state.current_vehicle_model() else {
            self.state
                .push_toast(Toast::error("No vehicle loaded — open a car in the editor first"));
            return;
        };

        let template = if self.state.mod_template_vehicle.is_empty() {
            mod_entry.target_vehicles.first().map(|s| s.as_str())
        } else {
            Some(self.state.mod_template_vehicle.as_str())
        };

        match crate::mod_scanner::add_vehicle_adapter_for_engine_mod(
            &self.state.settings,
            &mod_entry,
            &vehicle,
            template,
        ) {
            Ok(()) => {
                self.state.push_toast(Toast::info(format!(
                    "Created adapter for {vehicle} → {} (enable {} in BeamNG)",
                    mod_entry.name,
                    crate::mod_scanner::ADAPTER_MOD_FOLDER
                )));
                self.state.request_rescan = true;
            }
            Err(e) => self.state.push_toast(Toast::error(e.to_string())),
        }
    }

    fn unpack_mod_by_id(&mut self, mod_id: &str) {
        let Some(mod_entry) = self.state.mods.iter().find(|m| m.id == mod_id).cloned() else {
            self.state.push_toast(Toast::error("Mod not found"));
            return;
        };
        let crate::mod_scanner::ModLocation::Zip { archive_path } = mod_entry.location else {
            self.state.push_toast(Toast::error("Mod is already unpacked"));
            return;
        };
        let mods_root = match crate::mod_scanner::mods_dir(&self.state.settings) {
            Ok(p) => p,
            Err(e) => {
                self.state.push_toast(Toast::error(e.to_string()));
                return;
            }
        };
        match crate::mod_scanner::unpack_mod(&archive_path, &mods_root) {
            Ok(dest) => {
                self.state.push_toast(Toast::info(format!("Unpacked to {}", dest.display())));
                self.state.selected_mod_id = Some(format!(
                    "unpacked::{}",
                    dest.file_name().and_then(|s| s.to_str()).unwrap_or("mod")
                ));
                self.state.request_rescan = true;
            }
            Err(e) => self.state.push_toast(Toast::error(e.to_string())),
        }
    }

    fn pack_mod_by_id(&mut self, mod_id: &str) {
        let Some(mod_entry) = self.state.mods.iter().find(|m| m.id == mod_id).cloned() else {
            self.state.push_toast(Toast::error("Mod not found"));
            return;
        };
        let Some(root) = crate::mod_scanner::mod_root_path(&mod_entry) else {
            self.state.push_toast(Toast::error("Only unpacked mods can be packed"));
            return;
        };
        let mods_root = match crate::mod_scanner::mods_dir(&self.state.settings) {
            Ok(p) => p,
            Err(e) => {
                self.state.push_toast(Toast::error(e.to_string()));
                return;
            }
        };
        match crate::mod_scanner::pack_mod(&root, &mods_root) {
            Ok(zip_path) => {
                self.state.push_toast(Toast::info(format!(
                    "Packed to {}",
                    zip_path.display()
                )));
                self.state.request_rescan = true;
            }
            Err(e) => self.state.push_toast(Toast::error(e.to_string())),
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
                if let Some(mut location) = self.state.loaded_location.take() {
                    if let Err(e) = location.write_back(&save_path) {
                        self.state.loaded_location = Some(location);
                        self.state
                            .push_toast(Toast::error(format!("Write-back failed: {e}")));
                        return;
                    }
                    self.state.loaded_location = Some(location);
                }

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
                    let id = self.state.selected_id.clone();
                    if let Some(id) = id {
                        self.load_vehicle_by_id(&id);
                    }
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

    fn launch_beamng(&mut self) {
        let Some(exe) = &self.state.settings.beamng_exe_path else {
            self.state
                .push_toast(Toast::error("Set BeamNG.exe in Settings first"));
            return;
        };
        if !exe.exists() {
            self.state
                .push_toast(Toast::error(format!("BeamNG exe not found: {}", exe.display())));
            return;
        }
        match Command::new(exe).spawn() {
            Ok(_) => self.state.push_toast(Toast::info("Launching BeamNG…")),
            Err(e) => self
                .state
                .push_toast(Toast::error(format!("Launch failed: {e}"))),
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
        self.poll_scan();
        self.poll_mod_scan();
        self.poll_parts_scan();
        self.poll_external_changes();
        self.state.thumbnails.poll();
        self.state.thumbnails.upload_pending(ctx);

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            top_bar(ui, &mut self.state);
        });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .inner_margin(egui::Margin::symmetric(20.0, 16.0))
                    .fill(egui::Color32::from_rgb(32, 32, 36)),
            )
            .show(ctx, |ui| {
                render_tab(ui, &mut self.state);
            });

        // Process UI actions after widgets so clicks apply same frame
        self.process_actions();

        draw_confirm_dialogs(ctx, &mut self.state);
        draw_drive_button(ctx, &mut self.state);
        draw_toasts(ctx, &mut self.state);

        if self.state.scanning
            || self.state.mod_scanning
            || self.state.parts_scanning
            || self.state.thumbnails.has_pending()
        {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}
