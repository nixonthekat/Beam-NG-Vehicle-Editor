use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    pub mods_vehicles_path: Option<PathBuf>,
    pub beamng_exe_path: Option<PathBuf>,
    pub hot_reload_external_changes: bool,
}

impl AppSettings {
    pub fn settings_dir() -> AppResult<PathBuf> {
        let dir = directories::ProjectDirs::from("", "", "beam-ng-vehicle-editor")
            .ok_or_else(|| AppError::msg("Could not resolve settings directory"))?
            .config_dir()
            .to_path_buf();
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    pub fn settings_path() -> AppResult<PathBuf> {
        Ok(Self::settings_dir()?.join(SETTINGS_FILE))
    }

    pub fn load() -> AppResult<Self> {
        let path = Self::settings_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = fs::read_to_string(path)?;
        let settings: Self = serde_json::from_str(&text)?;
        Ok(settings)
    }

    pub fn save(&self) -> AppResult<()> {
        let path = Self::settings_path()?;
        let text = serde_json::to_string_pretty(self)?;
        fs::write(path, text)?;
        Ok(())
    }

    pub fn backups_root(&self) -> AppResult<PathBuf> {
        let dir = Self::settings_dir()?.join("backups");
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    pub fn mods_path(&self) -> Option<&Path> {
        self.mods_vehicles_path.as_deref()
    }
}
