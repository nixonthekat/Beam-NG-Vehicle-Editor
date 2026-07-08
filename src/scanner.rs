use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use walkdir::WalkDir;

use crate::error::{AppError, AppResult};
use crate::json_util::validate_beamng_json;
use crate::scan_util::mod_zip_dirs;
use crate::settings::AppSettings;
use crate::vehicle_source::{
    find_thumbnail_for_pc, find_thumbnail_in_zip, ThumbnailSource, VehicleLocation,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehicleCategory {
    Saved,
    Stock,
    Mod,
}

#[derive(Debug, Clone)]
pub struct VehicleEntry {
    pub id: String,
    pub name: String,
    pub model_key: String,
    pub location: VehicleLocation,
    pub mod_name: String,
    pub category: VehicleCategory,
    pub is_stock: bool,
    pub thumbnail: Option<ThumbnailSource>,
    pub config_count: usize,
    pub in_zip: bool,
}

#[derive(Debug, Clone)]
pub enum ScanMessage {
    Progress { current: usize, message: String },
    Vehicle(VehicleEntry),
    Finished { total: usize, skipped: usize },
    Skipped,
    Error(String),
}

struct ScanContext<'a> {
    user_dir: Option<&'a Path>,
    tx: &'a Sender<ScanMessage>,
    count: &'a mut usize,
    skipped: &'a mut usize,
    seen_ids: &'a mut BTreeSet<String>,
}

pub struct VehicleScanner;

impl VehicleScanner {
    pub fn spawn_scan(settings: AppSettings) -> Receiver<ScanMessage> {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            if let Err(err) = Self::scan_all(&settings, &tx) {
                let _ = tx.send(ScanMessage::Error(err.to_string()));
            }
        });
        rx
    }

    fn scan_all(settings: &AppSettings, tx: &Sender<ScanMessage>) -> AppResult<()> {
        if !settings.can_scan() {
            return Err(AppError::msg(
                "Set BeamNG user folder or BeamNG.exe in Settings to begin scanning",
            ));
        }

        let user_dir = settings.beamng_user_dir();
        let mut count = 0usize;
        let mut skipped = 0usize;
        let mut seen_ids = BTreeSet::new();
        let mut ctx = ScanContext {
            user_dir: user_dir.as_deref(),
            tx,
            count: &mut count,
            skipped: &mut skipped,
            seen_ids: &mut seen_ids,
        };

        if let Some(saved) = settings.saved_vehicles_dir() {
            let _ = tx.send(ScanMessage::Progress {
                current: *ctx.count,
                message: format!("Saved configs: {}", saved.display()),
            });
            Self::scan_filesystem(
                &mut ctx,
                &saved,
                &saved,
                ScanKind::Saved,
                "Saved",
            )?;
        }

        if let Some(mods) = settings.mods_dir() {
            let _ = tx.send(ScanMessage::Progress {
                current: *ctx.count,
                message: format!("Mods: {}", mods.display()),
            });
            Self::scan_mods_tree(&mut ctx, &mods)?;
        }

        if let Some(game_loose) = settings.game_loose_vehicles_dir() {
            let _ = tx.send(ScanMessage::Progress {
                current: *ctx.count,
                message: format!("Game vehicles: {}", game_loose.display()),
            });
            Self::scan_filesystem(
                &mut ctx,
                &game_loose,
                &game_loose,
                ScanKind::Mod,
                "Game",
            )?;
        }

        if let Some(stock_dir) = settings.game_vehicles_dir() {
            let _ = tx.send(ScanMessage::Progress {
                current: *ctx.count,
                message: format!("Stock packs: {}", stock_dir.display()),
            });
            Self::scan_stock_zips(&mut ctx, &stock_dir)?;
        }

        let _ = tx.send(ScanMessage::Finished {
            total: count,
            skipped,
        });
        Ok(())
    }

    fn scan_mods_tree(ctx: &mut ScanContext<'_>, mods_root: &Path) -> AppResult<()> {
        for zip_dir in mod_zip_dirs(mods_root) {
            if zip_dir.file_name().and_then(|s| s.to_str()) == Some("unpacked") {
                continue;
            }
            Self::scan_zip_files_in_dir(ctx, mods_root, &zip_dir, ScanKind::Mod)?;
        }

        Self::scan_filesystem(ctx, mods_root, mods_root, ScanKind::Mod, "Mod")?;

        let unpacked = mods_root.join("unpacked");
        if unpacked.is_dir() {
            if let Ok(read_dir) = fs::read_dir(&unpacked) {
                for entry in read_dir.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let mod_name = entry
                            .file_name()
                            .to_string_lossy()
                            .to_string();
                        Self::scan_filesystem(ctx, &path, &path, ScanKind::Mod, &mod_name)?;
                        Self::scan_zip_files_in_dir(ctx, &path, &path, ScanKind::Mod)?;
                    } else if path.is_file()
                        && path.extension().and_then(|e| e.to_str()) == Some("zip")
                    {
                        Self::scan_zip_file(ctx, mods_root, &path, ScanKind::Mod)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn scan_stock_zips(ctx: &mut ScanContext<'_>, stock_dir: &Path) -> AppResult<()> {
        if let Ok(read_dir) = fs::read_dir(stock_dir) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("zip") {
                    Self::scan_zip_file(ctx, stock_dir, &path, ScanKind::Stock)?;
                }
            }
        }
        Ok(())
    }

    fn scan_filesystem(
        ctx: &mut ScanContext<'_>,
        root: &Path,
        scan_root: &Path,
        kind: ScanKind,
        mod_label: &str,
    ) -> AppResult<()> {
        for entry in WalkDir::new(scan_root)
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

            *ctx.count += 1;
            let _ = ctx.tx.send(ScanMessage::Progress {
                current: *ctx.count,
                message: path.display().to_string(),
            });

            match build_local_entry(root, path, ctx.user_dir, kind, mod_label) {
                Ok(vehicle) => {
                    if ctx.seen_ids.insert(vehicle.id.clone()) {
                        let _ = ctx.tx.send(ScanMessage::Vehicle(vehicle));
                    }
                }
                Err(err) => {
                    *ctx.skipped += 1;
                    let _ = ctx.tx.send(ScanMessage::Skipped);
                    let _ = err;
                }
            }
        }
        Ok(())
    }

    fn scan_zip_files_in_dir(
        ctx: &mut ScanContext<'_>,
        root: &Path,
        dir: &Path,
        kind: ScanKind,
    ) -> AppResult<()> {
        if let Ok(read_dir) = fs::read_dir(dir) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("zip") {
                    Self::scan_zip_file(ctx, root, &path, kind)?;
                }
            }
        }
        Ok(())
    }

    fn scan_zip_file(
        ctx: &mut ScanContext<'_>,
        root: &Path,
        zip_path: &Path,
        kind: ScanKind,
    ) -> AppResult<()> {
        let file = fs::File::open(zip_path)
            .map_err(|e| AppError::msg(format!("Open {}: {e}", zip_path.display())))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| AppError::msg(format!("Zip {}: {e}", zip_path.display())))?;

        let mod_name = match kind {
            ScanKind::Stock => zip_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("BeamNG")
                .to_string(),
            _ => zip_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("mod")
                .to_string(),
        };

        let mut pc_entries: Vec<String> = Vec::new();
        for i in 0..archive.len() {
            let file = archive.by_index(i)?;
            let name = file.name().replace('\\', "/");
            if name.ends_with(".pc")
                && (name.contains("/vehicles/") || name.starts_with("vehicles/"))
            {
                pc_entries.push(name);
            }
        }

        let configs_in_folder = count_configs_per_folder(&pc_entries);

        for pc_entry in pc_entries {
            *ctx.count += 1;
            let _ = ctx.tx.send(ScanMessage::Progress {
                current: *ctx.count,
                message: format!("{}!{}", zip_path.display(), pc_entry),
            });

            match build_zip_entry(
                root,
                zip_path,
                &pc_entry,
                &mod_name,
                ctx.user_dir,
                configs_in_folder
                    .get(&zip_entry_folder(&pc_entry))
                    .copied()
                    .unwrap_or(1),
                kind,
            ) {
                Ok(vehicle) => {
                    if ctx.seen_ids.insert(vehicle.id.clone()) {
                        let _ = ctx.tx.send(ScanMessage::Vehicle(vehicle));
                    }
                }
                Err(err) => {
                    *ctx.skipped += 1;
                    let _ = ctx.tx.send(ScanMessage::Skipped);
                    let _ = err;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
enum ScanKind {
    Saved,
    Stock,
    Mod,
}

fn build_local_entry(
    root: &Path,
    config_path: &Path,
    user_dir: Option<&Path>,
    kind: ScanKind,
    mod_label: &str,
) -> AppResult<VehicleEntry> {
    let folder_path = config_path
        .parent()
        .ok_or_else(|| AppError::msg("Config has no parent folder"))?
        .to_path_buf();

    validate_pc_file(config_path)?;

    let vehicle_folder = folder_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("vehicle")
        .to_string();

    let stem = config_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("vehicle")
        .to_string();

    let (category, mod_name, is_stock) = match kind {
        ScanKind::Saved => (VehicleCategory::Saved, "Saved".to_string(), false),
        ScanKind::Stock => (
            VehicleCategory::Stock,
            "BeamNG".to_string(),
            true,
        ),
        ScanKind::Mod => {
            let rel = config_path.strip_prefix(root).unwrap_or(config_path);
            let label = rel
                .components()
                .next()
                .and_then(|c| c.as_os_str().to_str())
                .unwrap_or(mod_label)
                .to_string();
            (VehicleCategory::Mod, label, false)
        }
    };

    let name = match kind {
        ScanKind::Saved => format!("{} (saved)", format_display_name(&stem)),
        ScanKind::Stock => format!(
            "{} ({})",
            format_display_name(&stem),
            format_display_name(&vehicle_folder)
        ),
        ScanKind::Mod => format_display_name(&stem),
    };

    let config_count = count_local_configs(&folder_path);
    let thumbnail = find_thumbnail_for_pc(config_path, &folder_path, &stem, user_dir);

    let id = match kind {
        ScanKind::Saved => format!("saved::{vehicle_folder}::{stem}"),
        ScanKind::Stock => format!("stock::{vehicle_folder}::{stem}"),
        ScanKind::Mod => format!("{mod_name}::{vehicle_folder}::{stem}"),
    };

    Ok(VehicleEntry {
        id,
        name,
        model_key: vehicle_folder,
        location: VehicleLocation::Local {
            config_path: config_path.to_path_buf(),
        },
        mod_name,
        category,
        is_stock,
        thumbnail,
        config_count,
        in_zip: false,
    })
}

fn build_zip_entry(
    _root: &Path,
    zip_path: &Path,
    pc_entry: &str,
    mod_name: &str,
    user_dir: Option<&Path>,
    config_count: usize,
    kind: ScanKind,
) -> AppResult<VehicleEntry> {
    validate_pc_in_zip(zip_path, pc_entry)?;

    let stem = Path::new(pc_entry)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("vehicle")
        .to_string();

    let vehicle_folder = vehicle_folder_from_pc_entry(pc_entry).unwrap_or_else(|| mod_name.to_string());

    let name = match kind {
        ScanKind::Stock => config_name_from_stock_zip(zip_path, pc_entry, &vehicle_folder, &stem),
        _ => format_display_name(&stem),
    };

    let thumbnail = find_thumbnail_in_zip(zip_path, pc_entry, &stem, user_dir);
    let (category, is_stock) = match kind {
        ScanKind::Stock => (VehicleCategory::Stock, true),
        ScanKind::Saved => (VehicleCategory::Saved, false),
        ScanKind::Mod => (VehicleCategory::Mod, false),
    };

    let id = match kind {
        ScanKind::Stock => format!("stock::{vehicle_folder}::{stem}"),
        _ => format!("{mod_name}::{vehicle_folder}::{stem}::{}", zip_path.display()),
    };

    Ok(VehicleEntry {
        id,
        name,
        model_key: vehicle_folder,
        location: VehicleLocation::Zip {
            archive_path: zip_path.to_path_buf(),
            pc_entry: pc_entry.to_string(),
            cached_path: None,
        },
        mod_name: if is_stock {
            "BeamNG".to_string()
        } else {
            mod_name.to_string()
        },
        category,
        is_stock,
        thumbnail,
        config_count,
        in_zip: true,
    })
}

fn config_name_from_stock_zip(
    zip_path: &Path,
    _pc_entry: &str,
    vehicle_folder: &str,
    stem: &str,
) -> String {
    let file = match fs::File::open(zip_path) {
        Ok(f) => f,
        Err(_) => return format_display_name(stem),
    };
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(_) => return format_display_name(stem),
    };

    let candidates = [
        format!("vehicles/{vehicle_folder}/info_{stem}.json"),
        format!("vehicles/{vehicle_folder}/info.json"),
    ];

    for candidate in candidates {
        if let Ok(mut file) = archive.by_name(&candidate) {
            let mut text = String::new();
            if file.read_to_string(&mut text).is_ok() {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(conf) = value.get("Configuration").and_then(|v| v.as_str()) {
                        return conf.to_string();
                    }
                }
            }
        }
    }

    format_display_name(stem)
}

fn vehicle_folder_from_pc_entry(entry: &str) -> Option<String> {
    let normalized = entry.replace('\\', "/");
    let parts: Vec<&str> = normalized.split('/').collect();
    if let Some(idx) = parts.iter().position(|p| *p == "vehicles") {
        return parts.get(idx + 1).map(|s| (*s).to_string());
    }
    None
}

fn format_display_name(stem: &str) -> String {
    stem.replace('_', " ")
}

fn zip_entry_folder(entry: &str) -> String {
    Path::new(entry)
        .parent()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

fn count_configs_per_folder(entries: &[String]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for entry in entries {
        let folder = zip_entry_folder(entry);
        *map.entry(folder).or_insert(0) += 1;
    }
    map
}

fn count_local_configs(folder: &Path) -> usize {
    fs::read_dir(folder)
        .map(|read_dir| {
            read_dir
                .flatten()
                .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("pc"))
                .count()
        })
        .unwrap_or(1)
        .max(1)
}

fn validate_pc_file(path: &Path) -> AppResult<()> {
    let text = fs::read_to_string(path)?;
    validate_beamng_json(&text)?;
    Ok(())
}

fn validate_pc_in_zip(zip_path: &Path, entry: &str) -> AppResult<()> {
    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut zip_file = archive.by_name(entry)?;
    let mut text = String::new();
    zip_file.read_to_string(&mut text)?;
    validate_beamng_json(&text)?;
    Ok(())
}

pub fn find_thumbnail(folder: &Path) -> Option<PathBuf> {
    crate::vehicle_source::find_thumbnail_in_folder(
        folder,
        folder
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(""),
    )
}
