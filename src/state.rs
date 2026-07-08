use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use crate::backup::{BackupIndex, BackupMetadata};
use crate::config::VehicleConfig;
use crate::mod_scanner::{ModEntry, ModKind, ModScanMessage, ModStorage};
use crate::scanner::{ScanMessage, VehicleCategory, VehicleEntry};
use crate::settings::AppSettings;
use crate::thumbnail::ThumbnailCache;
use crate::vehicle_source::VehicleLocation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTab {
    Grid,
    Mods,
    Editor,
    Backups,
    Settings,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub is_error: bool,
    pub created: Instant,
}

impl Toast {
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            is_error: false,
            created: Instant::now(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            is_error: true,
            created: Instant::now(),
        }
    }

    pub fn expired(&self) -> bool {
        self.created.elapsed() > Duration::from_secs(4)
    }
}

#[derive(Debug, Clone)]
pub struct UndoSnapshot {
    pub config: VehicleConfig,
    pub label: String,
}

pub struct AppState {
    pub settings: AppSettings,
    pub tab: AppTab,
    pub vehicles: Vec<VehicleEntry>,
    pub selected_id: Option<String>,
    pub search_query: String,
    pub mod_filter: Option<String>,
    pub stock_only: bool,
    pub mod_only: bool,
    pub saved_only: bool,

    pub mods: Vec<ModEntry>,
    pub selected_mod_id: Option<String>,
    pub mod_add_vehicle: String,
    pub mod_template_vehicle: String,
    pub mod_scan_rx: Option<Receiver<ModScanMessage>>,
    pub mod_scanning: bool,
    pub mod_scan_progress: Option<(usize, String)>,
    pub pending_mod_remove: Option<String>,

    pub loaded_config: Option<VehicleConfig>,
    pub loaded_location: Option<VehicleLocation>,
    pub edit_buffer: Option<VehicleConfig>,
    pub slot_edits: HashMap<String, String>,
    pub slot_undo_pending: std::collections::HashSet<String>,

    pub backup_index: BackupIndex,
    pub pending_restore: Option<BackupMetadata>,
    pub pending_restore_all: bool,

    pub parts_index: crate::parts::PartsIndex,
    pub parts_collect: Vec<crate::parts::PartEntry>,
    pub parts_scan_rx: Option<Receiver<crate::parts::PartsScanMessage>>,
    pub parts_scanning: bool,
    pub parts_scan_progress: Option<(usize, String)>,

    pub engine_search: String,
    pub engine_mod_filter: Option<String>,
    pub show_engine_browser: bool,
    pub selected_engine_mod: Option<String>,

    pub slot_custom_mode: std::collections::HashSet<String>,
    pub slot_filter: String,

    pub scan_rx: Option<Receiver<ScanMessage>>,
    pub scan_progress: Option<(usize, String)>,
    pub scanning: bool,

    pub thumbnails: ThumbnailCache,
    pub toasts: VecDeque<Toast>,
    pub undo_stack: Vec<UndoSnapshot>,
    pub redo_stack: Vec<UndoSnapshot>,

    pub dirty: bool,
    pub last_external_check: Instant,
    pub status_message: String,

    // UI action requests (processed by app each frame)
    pub request_user_browse: bool,
    pub request_rescan: bool,
    pub request_mods_browse: bool,
    pub request_exe_browse: bool,
    pub request_save_settings: bool,
    pub request_load_vehicle_id: Option<String>,
    pub request_apply: bool,
    pub request_restore_latest: bool,
    pub request_parts_scan: bool,
    pub request_engine_scan: bool,
    pub request_assign_engine: Option<String>,
    pub request_unpack_mod_id: Option<String>,
    pub request_pack_mod_id: Option<String>,
    pub request_add_car_to_mod_id: Option<String>,
    pub confirm_restore: bool,
    pub confirm_restore_all: bool,
    pub backup_vehicle_filter: Option<String>,
    pub request_drive: bool,
}

impl AppState {
    pub fn new(settings: AppSettings, backup_index: BackupIndex) -> Self {
        Self {
            settings,
            tab: AppTab::Grid,
            vehicles: Vec::new(),
            selected_id: None,
            search_query: String::new(),
            mod_filter: None,
            stock_only: false,
            mod_only: false,
            saved_only: false,
            mods: Vec::new(),
            selected_mod_id: None,
            mod_add_vehicle: String::new(),
            mod_template_vehicle: String::new(),
            mod_scan_rx: None,
            mod_scanning: false,
            mod_scan_progress: None,
            pending_mod_remove: None,
            loaded_config: None,
            loaded_location: None,
            edit_buffer: None,
            slot_edits: HashMap::new(),
            slot_undo_pending: std::collections::HashSet::new(),
            backup_index,
            pending_restore: None,
            pending_restore_all: false,
            parts_index: crate::parts::PartsIndex::default(),
            parts_collect: Vec::new(),
            parts_scan_rx: None,
            parts_scanning: false,
            parts_scan_progress: None,
            engine_search: String::new(),
            engine_mod_filter: None,
            show_engine_browser: false,
            selected_engine_mod: None,
            slot_custom_mode: std::collections::HashSet::new(),
            slot_filter: String::new(),
            scan_rx: None,
            scan_progress: None,
            scanning: false,
            thumbnails: ThumbnailCache::new(),
            toasts: VecDeque::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            dirty: false,
            last_external_check: Instant::now(),
            status_message: "Set BeamNG user folder and exe in Settings, then Rescan.".to_string(),
            request_user_browse: false,
            request_rescan: false,
            request_mods_browse: false,
            request_exe_browse: false,
            request_save_settings: false,
            request_load_vehicle_id: None,
            request_apply: false,
            request_restore_latest: false,
            request_parts_scan: false,
            request_engine_scan: false,
            request_assign_engine: None,
            request_unpack_mod_id: None,
            request_pack_mod_id: None,
            request_add_car_to_mod_id: None,
            confirm_restore: false,
            confirm_restore_all: false,
            backup_vehicle_filter: None,
            request_drive: false,
        }
    }

    pub fn push_toast(&mut self, toast: Toast) {
        self.toasts.push_back(toast);
        while self.toasts.len() > 5 {
            self.toasts.pop_front();
        }
    }

    pub fn selected_vehicle(&self) -> Option<&VehicleEntry> {
        let id = self.selected_id.as_ref()?;
        self.vehicles.iter().find(|v| &v.id == id)
    }

    pub fn filtered_vehicles(&self) -> Vec<&VehicleEntry> {
        let q = self.search_query.trim().to_ascii_lowercase();
        self.vehicles
            .iter()
            .filter(|v| {
                if self.saved_only && v.category != VehicleCategory::Saved {
                    return false;
                }
                if self.stock_only && v.category != VehicleCategory::Stock {
                    return false;
                }
                if self.mod_only && v.category != VehicleCategory::Mod {
                    return false;
                }
                if let Some(ref m) = self.mod_filter {
                    if !v.mod_name.eq_ignore_ascii_case(m) {
                        return false;
                    }
                }
                if q.is_empty() {
                    return true;
                }
                v.name.to_ascii_lowercase().contains(&q)
                    || v.id.to_ascii_lowercase().contains(&q)
                    || v.mod_name.to_ascii_lowercase().contains(&q)
                    || v.model_key.to_ascii_lowercase().contains(&q)
            })
            .collect()
    }

    pub fn selected_mod(&self) -> Option<&ModEntry> {
        let id = self.selected_mod_id.as_ref()?;
        self.mods.iter().find(|m| &m.id == id)
    }

    pub fn filtered_mods(&self) -> Vec<&ModEntry> {
        let q = self.search_query.trim().to_ascii_lowercase();
        self.mods
            .iter()
            .filter(|m| {
                q.is_empty()
                    || m.name.to_ascii_lowercase().contains(&q)
                    || m.id.to_ascii_lowercase().contains(&q)
                    || m.target_vehicles.iter().any(|v| v.to_ascii_lowercase().contains(&q))
            })
            .collect()
    }

    pub fn filtered_mods_by_storage(&self, storage: ModStorage) -> Vec<&ModEntry> {
        self.filtered_mods()
            .into_iter()
            .filter(|m| m.storage == storage)
            .collect()
    }

    pub fn engine_mod_entries(&self) -> Vec<&ModEntry> {
        self.mods
            .iter()
            .filter(|m| m.kind == ModKind::EngineParts || m.engine_count > 0)
            .collect()
    }

    pub fn current_vehicle_model(&self) -> Option<String> {
        if let Some(v) = self.selected_vehicle() {
            if !v.model_key.is_empty() {
                return Some(v.model_key.clone());
            }
        }
        if let Some(buf) = &self.edit_buffer {
            if let Some(model) = buf.raw.get("model").and_then(|v| v.as_str()) {
                return Some(model.to_string());
            }
            if let Some(model) = buf.raw.get("mainPartName").and_then(|v| v.as_str()) {
                return Some(model.to_string());
            }
        }
        None
    }

    pub fn mod_by_name(&self, name: &str) -> Option<&ModEntry> {
        self.mods
            .iter()
            .filter(|m| m.name.eq_ignore_ascii_case(name))
            .max_by_key(|m| match m.storage {
                ModStorage::Unpacked => 1,
                ModStorage::Packed => 0,
            })
    }

    pub fn mod_names(&self) -> Vec<String> {
        let mut mods: Vec<_> = self
            .vehicles
            .iter()
            .map(|v| v.mod_name.clone())
            .collect();
        mods.sort();
        mods.dedup();
        mods
    }

    pub fn vehicle_key(&self) -> Option<String> {
        self.edit_buffer
            .as_ref()
            .map(|c| crate::config::vehicle_key_from_path(&c.path))
    }

    pub fn push_undo(&mut self, label: impl Into<String>) {
        if let Some(buf) = self.edit_buffer.clone() {
            self.undo_stack.push(UndoSnapshot {
                config: buf,
                label: label.into(),
            });
            self.redo_stack.clear();
            if self.undo_stack.len() > 50 {
                self.undo_stack.remove(0);
            }
        }
    }

    pub fn undo(&mut self) {
        if let Some(snapshot) = self.undo_stack.pop() {
            if let Some(current) = self.edit_buffer.take() {
                self.redo_stack.push(UndoSnapshot {
                    config: current,
                    label: "redo point".to_string(),
                });
            }
            self.edit_buffer = Some(snapshot.config);
            self.sync_slot_edits();
            self.dirty = true;
            self.push_toast(Toast::info(format!("Undo: {}", snapshot.label)));
        }
    }

    pub fn redo(&mut self) {
        if let Some(snapshot) = self.redo_stack.pop() {
            if let Some(current) = self.edit_buffer.take() {
                self.undo_stack.push(UndoSnapshot {
                    config: current,
                    label: "undo point".to_string(),
                });
            }
            self.edit_buffer = Some(snapshot.config);
            self.sync_slot_edits();
            self.dirty = true;
            self.push_toast(Toast::info("Redo"));
        }
    }

    pub fn sync_slot_edits(&mut self) {
        self.slot_edits.clear();
        if let Some(buf) = &self.edit_buffer {
            for (k, v) in &buf.parts {
                self.slot_edits.insert(k.clone(), v.clone());
            }
        }
    }

    pub fn apply_slot_edits(&mut self) {
        if let Some(buf) = &mut self.edit_buffer {
            for (slot, part) in &self.slot_edits {
                buf.set_part(slot, part);
            }
        }
    }
}
