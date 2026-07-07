use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use egui::{ColorImage, TextureHandle, TextureOptions};

use crate::backup::{BackupIndex, BackupMetadata};
use crate::config::VehicleConfig;
use crate::engine::EnginePart;
use crate::scanner::{ScanMessage, VehicleEntry};
use crate::settings::AppSettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTab {
    Grid,
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

pub struct ThumbnailCache {
    textures: HashMap<String, TextureHandle>,
    pending: HashMap<String, ColorImage>,
}

impl ThumbnailCache {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            pending: HashMap::new(),
        }
    }

    pub fn queue_load(&mut self, id: &str, path: &PathBuf) {
        if self.textures.contains_key(id) || self.pending.contains_key(id) {
            return;
        }
        if let Ok(img) = load_image_color(path) {
            self.pending.insert(id.to_string(), img);
        }
    }

    pub fn upload_pending(&mut self, ctx: &egui::Context) {
        let pending: Vec<_> = self.pending.drain().collect();
        for (id, color) in pending {
            let texture = ctx.load_texture(
                format!("thumb_{id}"),
                color,
                TextureOptions::LINEAR,
            );
            self.textures.insert(id, texture);
        }
    }

    pub fn get(&self, id: &str) -> Option<&TextureHandle> {
        self.textures.get(id)
    }

    pub fn clear(&mut self) {
        self.textures.clear();
        self.pending.clear();
    }
}

fn load_image_color(path: &PathBuf) -> Result<ColorImage, image::ImageError> {
    let img = image::open(path)?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Ok(ColorImage::from_rgba_unmultiplied(size, &img))
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

    pub loaded_config: Option<VehicleConfig>,
    pub edit_buffer: Option<VehicleConfig>,
    pub slot_edits: HashMap<String, String>,

    pub backup_index: BackupIndex,
    pub pending_restore: Option<BackupMetadata>,
    pub pending_restore_all: bool,

    pub engines: Vec<EnginePart>,
    pub engine_search: String,
    pub engine_mod_filter: Option<String>,
    pub show_engine_browser: bool,

    pub scan_rx: Option<Receiver<ScanMessage>>,
    pub scan_progress: Option<(usize, String)>,
    pub scanning: bool,

    pub engine_scan_rx: Option<Receiver<crate::engine::EngineScanMessage>>,
    pub engine_scanning: bool,
    pub engine_scan_progress: Option<(usize, String)>,

    pub thumbnails: ThumbnailCache,
    pub toasts: VecDeque<Toast>,
    pub undo_stack: Vec<UndoSnapshot>,
    pub redo_stack: Vec<UndoSnapshot>,

    pub dirty: bool,
    pub last_external_check: Instant,
    pub status_message: String,

    // UI action requests (processed by app each frame)
    pub request_rescan: bool,
    pub request_mods_browse: bool,
    pub request_exe_browse: bool,
    pub request_save_settings: bool,
    pub request_load_vehicle: Option<PathBuf>,
    pub request_apply: bool,
    pub request_restore_latest: bool,
    pub request_engine_scan: bool,
    pub request_assign_engine: Option<String>,
    pub confirm_restore: bool,
    pub confirm_restore_all: bool,
    pub backup_vehicle_filter: Option<String>,
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
            loaded_config: None,
            edit_buffer: None,
            slot_edits: HashMap::new(),
            backup_index,
            pending_restore: None,
            pending_restore_all: false,
            engines: Vec::new(),
            engine_search: String::new(),
            engine_mod_filter: None,
            show_engine_browser: false,
            scan_rx: None,
            scan_progress: None,
            scanning: false,
            engine_scan_rx: None,
            engine_scanning: false,
            engine_scan_progress: None,
            thumbnails: ThumbnailCache::new(),
            toasts: VecDeque::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            dirty: false,
            last_external_check: Instant::now(),
            status_message: "Set your BeamNG mods folder in Settings to begin.".to_string(),
            request_rescan: false,
            request_mods_browse: false,
            request_exe_browse: false,
            request_save_settings: false,
            request_load_vehicle: None,
            request_apply: false,
            request_restore_latest: false,
            request_engine_scan: false,
            request_assign_engine: None,
            confirm_restore: false,
            confirm_restore_all: false,
            backup_vehicle_filter: None,
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
                if self.stock_only && !v.is_stock {
                    return false;
                }
                if self.mod_only && v.is_stock {
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
            })
            .collect()
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
