use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// BeamNG user folder, e.g. `%LocalAppData%/BeamNG/BeamNG.drive/current`
    pub beamng_user_path: Option<PathBuf>,
    /// Legacy / optional override for mods folder
    pub mods_vehicles_path: Option<PathBuf>,
    pub beamng_exe_path: Option<PathBuf>,
    pub hot_reload_external_changes: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            beamng_user_path: Self::detect_user_dir(),
            mods_vehicles_path: None,
            beamng_exe_path: None,
            hot_reload_external_changes: false,
        }
    }
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
        let mut settings: Self = serde_json::from_str(&text)?;
        if settings.beamng_user_path.is_none() {
            settings.beamng_user_path = Self::detect_user_dir();
        }
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

    pub fn cache_root(&self) -> AppResult<PathBuf> {
        let dir = Self::settings_dir()?.join("cache");
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    pub fn detect_user_dir() -> Option<PathBuf> {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|local| local.join("BeamNG").join("BeamNG.drive").join("current"))
            .filter(|p| p.is_dir())
    }

    pub fn effective_user_dir(&self) -> Option<PathBuf> {
        self.beamng_user_path
            .clone()
            .filter(|p| p.is_dir())
            .or_else(Self::detect_user_dir)
    }

    pub fn beamng_user_dir(&self) -> Option<PathBuf> {
        self.effective_user_dir()
    }

    pub fn mods_dir(&self) -> Option<PathBuf> {
        if let Some(user) = self.effective_user_dir() {
            let mods = user.join("mods");
            if mods.is_dir() {
                return Some(mods);
            }
        }
        if let Some(path) = &self.mods_vehicles_path {
            if path.is_dir() {
                if path.file_name().and_then(|s| s.to_str()) == Some("mods") {
                    return Some(path.clone());
                }
                let mods = path.join("mods");
                if mods.is_dir() {
                    return Some(mods);
                }
                return Some(path.clone());
            }
        }
        None
    }

    pub fn saved_vehicles_dir(&self) -> Option<PathBuf> {
        self.effective_user_dir()
            .map(|u| u.join("vehicles"))
            .filter(|p| p.is_dir())
    }

    pub fn game_root(&self) -> Option<PathBuf> {
        self.beamng_exe_path
            .as_ref()
            .and_then(|exe| resolve_game_root(exe))
    }

    /// Stock vehicle packs shipped as zips (`content/vehicles/*.zip`).
    pub fn game_vehicles_dir(&self) -> Option<PathBuf> {
        self.game_root()
            .map(|root| root.join("content").join("vehicles"))
            .filter(|p| p.is_dir())
    }

    /// Loose mod installs dropped into the game folder (`vehicles/`).
    pub fn game_loose_vehicles_dir(&self) -> Option<PathBuf> {
        self.game_root()
            .map(|root| root.join("vehicles"))
            .filter(|p| p.is_dir())
    }

    pub fn mods_path(&self) -> Option<PathBuf> {
        self.mods_dir()
    }

    pub fn can_scan(&self) -> bool {
        self.mods_dir().is_some()
            || self.saved_vehicles_dir().is_some()
            || self.game_vehicles_dir().is_some()
            || self.game_loose_vehicles_dir().is_some()
    }
}

fn resolve_game_root(exe: &Path) -> Option<PathBuf> {
    let mut dir = exe.parent()?;
    if let Some(name) = dir.file_name().and_then(|s| s.to_str()) {
        if name.eq_ignore_ascii_case("Bin64")
            || name.eq_ignore_ascii_case("BinWin64")
            || name.eq_ignore_ascii_case("BinLinux")
        {
            dir = dir.parent()?;
        }
    }
    Some(dir.to_path_buf())
}
