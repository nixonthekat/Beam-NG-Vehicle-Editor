use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use walkdir::WalkDir;

use crate::config::VehicleConfig;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct VehicleEntry {
    pub id: String,
    pub name: String,
    pub config_path: PathBuf,
    pub folder_path: PathBuf,
    pub mod_name: String,
    pub is_stock: bool,
    pub thumbnail_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum ScanMessage {
    Progress { current: usize, message: String },
    Vehicle(VehicleEntry),
    Finished { total: usize },
    Error(String),
}

pub struct VehicleScanner;

impl VehicleScanner {
    pub fn spawn_scan(root: PathBuf) -> Receiver<ScanMessage> {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            if let Err(err) = Self::scan_root(&root, &tx) {
                let _ = tx.send(ScanMessage::Error(err.to_string()));
            }
        });
        rx
    }

    fn scan_root(root: &Path, tx: &Sender<ScanMessage>) -> AppResult<()> {
        if !root.is_dir() {
            return Err(AppError::msg(format!(
                "Mods/vehicles path does not exist: {}",
                root.display()
            )));
        }

        let mut count = 0usize;
        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("pc") {
                continue;
            }

            count += 1;
            let _ = tx.send(ScanMessage::Progress {
                current: count,
                message: path.display().to_string(),
            });

            match Self::build_entry(root, path) {
                Ok(vehicle) => {
                    let _ = tx.send(ScanMessage::Vehicle(vehicle));
                }
                Err(err) => {
                    let _ = tx.send(ScanMessage::Error(format!(
                        "Skipping {}: {}",
                        path.display(),
                        err
                    )));
                }
            }
        }

        let _ = tx.send(ScanMessage::Finished { total: count });
        Ok(())
    }

    fn build_entry(root: &Path, config_path: &Path) -> AppResult<VehicleEntry> {
        let folder_path = config_path
            .parent()
            .ok_or_else(|| AppError::msg("Config has no parent folder"))?
            .to_path_buf();

        let _config = VehicleConfig::load(config_path)?;

        let rel = config_path
            .strip_prefix(root)
            .unwrap_or(config_path)
            .to_path_buf();

        let mod_name = rel
            .components()
            .next()
            .and_then(|c| c.as_os_str().to_str())
            .unwrap_or("unknown")
            .to_string();

        let is_stock = mod_name.eq_ignore_ascii_case("vehicles")
            || mod_name.eq_ignore_ascii_case("stock")
            || mod_name.contains("game");

        let stem = config_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("vehicle");

        let name = stem.replace('_', " ");
        let thumbnail_path = find_thumbnail(&folder_path);

        Ok(VehicleEntry {
            id: format!("{}::{}", mod_name, stem),
            name,
            config_path: config_path.to_path_buf(),
            folder_path,
            mod_name,
            is_stock,
            thumbnail_path,
        })
    }
}

pub fn find_thumbnail(folder: &Path) -> Option<PathBuf> {
    const CANDIDATES: &[&str] = &[
        "preview.jpg",
        "preview.png",
        "default.jpg",
        "default.png",
    ];

    for name in CANDIDATES {
        let path = folder.join(name);
        if path.is_file() {
            return Some(path);
        }
    }

    let ui_dir = folder.join("ui");
    if ui_dir.is_dir() {
        if let Ok(read_dir) = std::fs::read_dir(&ui_dir) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        let ext_lower = ext.to_ascii_lowercase();
                        if ext_lower == "jpg" || ext_lower == "jpeg" || ext_lower == "png" {
                            return Some(path);
                        }
                    }
                }
            }
        }
    }

    None
}
