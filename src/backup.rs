use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::{vehicle_key_from_path, VehicleConfig};
use crate::error::{AppError, AppResult};
use crate::settings::AppSettings;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub vehicle_key: String,
    pub vehicle_name: String,
    pub original_path: PathBuf,
    pub backup_path: PathBuf,
    pub created_at: DateTime<Utc>,
    pub file_size: u64,
    pub checksum: String,
    pub reason: String,
    pub version: u32,
}

#[derive(Debug, Clone, Default)]
pub struct BackupIndex {
    pub by_vehicle: BTreeMap<String, Vec<BackupMetadata>>,
}

impl BackupIndex {
    pub fn load(settings: &AppSettings) -> AppResult<Self> {
        let root = settings.backups_root()?;
        let mut index = Self::default();

        if !root.exists() {
            return Ok(index);
        }

        for entry in fs::read_dir(&root)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let vehicle_key = entry
                .file_name()
                .to_string_lossy()
                .to_string();
            let mut backups = Vec::new();
            for file in fs::read_dir(&path)?.flatten() {
                let backup_path = file.path();
                if backup_path.extension().and_then(|e| e.to_str()) != Some("pc") {
                    continue;
                }
                let meta_path = backup_path.with_extension("meta.json");
                if meta_path.exists() {
                    if let Ok(text) = fs::read_to_string(&meta_path) {
                        if let Ok(meta) = serde_json::from_str::<BackupMetadata>(&text) {
                            backups.push(meta);
                            continue;
                        }
                    }
                }
                if let Ok(meta) = BackupManager::metadata_from_file(&backup_path, &vehicle_key) {
                    backups.push(meta);
                }
            }
            backups.sort_by_key(|b| b.created_at);
            index.by_vehicle.insert(vehicle_key, backups);
        }

        Ok(index)
    }

    pub fn backups_for(&self, vehicle_key: &str) -> &[BackupMetadata] {
        self.by_vehicle
            .get(vehicle_key)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn latest_for(&self, vehicle_key: &str) -> Option<&BackupMetadata> {
        self.backups_for(vehicle_key).last()
    }

    pub fn all_backups(&self) -> Vec<&BackupMetadata> {
        self.by_vehicle
            .values()
            .flat_map(|v| v.iter())
            .collect()
    }
}

pub struct BackupManager;

impl BackupManager {
    pub fn vehicle_backup_dir(settings: &AppSettings, vehicle_key: &str) -> AppResult<PathBuf> {
        let safe_key = sanitize_key(vehicle_key);
        let dir = settings.backups_root()?.join(safe_key);
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    pub fn create_backup(
        settings: &AppSettings,
        config_path: &Path,
        vehicle_name: &str,
        reason: &str,
        existing_count: usize,
    ) -> AppResult<BackupMetadata> {
        let vehicle_key = vehicle_key_from_path(config_path);
        let dir = Self::vehicle_backup_dir(settings, &vehicle_key)?;
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let version = (existing_count + 1) as u32;
        let filename = format!("{}_{}_v{:03}.pc", vehicle_key, timestamp, version);
        let backup_path = dir.join(&filename);

        fs::copy(config_path, &backup_path)?;

        let meta = Self::build_metadata(
            &backup_path,
            config_path,
            &vehicle_key,
            vehicle_name,
            reason,
            version,
        )?;
        Self::write_metadata(&meta)?;
        Ok(meta)
    }

    pub fn ensure_initial_backup(
        settings: &AppSettings,
        config_path: &Path,
        vehicle_name: &str,
        index: &BackupIndex,
    ) -> AppResult<Option<BackupMetadata>> {
        let vehicle_key = vehicle_key_from_path(config_path);
        if index.backups_for(&vehicle_key).is_empty() {
            let meta = Self::create_backup(
                settings,
                config_path,
                vehicle_name,
                "initial auto-backup before first edit",
                0,
            )?;
            return Ok(Some(meta));
        }
        Ok(None)
    }

    pub fn restore_backup(meta: &BackupMetadata) -> AppResult<()> {
        if !meta.backup_path.exists() {
            return Err(AppError::msg(format!(
                "Backup file missing: {}",
                meta.backup_path.display()
            )));
        }
        validate_backup_file(&meta.backup_path)?;
        fs::copy(&meta.backup_path, &meta.original_path)?;
        Ok(())
    }

    pub fn restore_all(index: &BackupIndex) -> AppResult<Vec<(String, AppResult<()>)>> {
        let mut results = Vec::new();
        for (vehicle_key, backups) in &index.by_vehicle {
            if let Some(latest) = backups.last() {
                let result = Self::restore_backup(latest);
                results.push((vehicle_key.clone(), result));
            }
        }
        Ok(results)
    }

    fn build_metadata(
        backup_path: &Path,
        original_path: &Path,
        vehicle_key: &str,
        vehicle_name: &str,
        reason: &str,
        version: u32,
    ) -> AppResult<BackupMetadata> {
        let file_size = fs::metadata(backup_path)?.len();
        let checksum = checksum_file(backup_path)?;
        Ok(BackupMetadata {
            vehicle_key: vehicle_key.to_string(),
            vehicle_name: vehicle_name.to_string(),
            original_path: original_path.to_path_buf(),
            backup_path: backup_path.to_path_buf(),
            created_at: Utc::now(),
            file_size,
            checksum,
            reason: reason.to_string(),
            version,
        })
    }

    fn write_metadata(meta: &BackupMetadata) -> AppResult<()> {
        let meta_path = meta.backup_path.with_extension("meta.json");
        let text = serde_json::to_string_pretty(meta)?;
        fs::write(meta_path, text)?;
        Ok(())
    }

    fn metadata_from_file(backup_path: &Path, vehicle_key: &str) -> AppResult<BackupMetadata> {
        let file_size = fs::metadata(backup_path)?.len();
        let checksum = checksum_file(backup_path)?;
        let created_at = fs::metadata(backup_path)?
            .modified()
            .ok()
            .map(|t| DateTime::<Utc>::from(t))
            .unwrap_or_else(Utc::now);

        Ok(BackupMetadata {
            vehicle_key: vehicle_key.to_string(),
            vehicle_name: vehicle_key.to_string(),
            original_path: PathBuf::new(),
            backup_path: backup_path.to_path_buf(),
            created_at,
            file_size,
            checksum,
            reason: "imported".to_string(),
            version: 0,
        })
    }
}

fn sanitize_key(key: &str) -> String {
    key.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn checksum_file(path: &Path) -> AppResult<String> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

fn validate_backup_file(path: &Path) -> AppResult<()> {
    let _ = VehicleConfig::load(path)?;
    Ok(())
}

pub fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let b = bytes as f64;
    if b >= MB {
        format!("{:.2} MB", b / MB)
    } else if b >= KB {
        format!("{:.2} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}
