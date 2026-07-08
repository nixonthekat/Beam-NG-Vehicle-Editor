use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use walkdir::WalkDir;

use crate::error::{AppError, AppResult};
use crate::scan_util::mod_zip_dirs;
use crate::settings::AppSettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModStorage {
    Packed,
    Unpacked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModKind {
    EngineParts,
    PartsPack,
    FullVehicle,
}

#[derive(Debug, Clone)]
pub enum ModLocation {
    Unpacked { root: PathBuf },
    Zip { archive_path: PathBuf },
}

#[derive(Debug, Clone)]
pub struct ModEntry {
    pub id: String,
    pub name: String,
    pub location: ModLocation,
    pub storage: ModStorage,
    pub kind: ModKind,
    pub target_vehicles: Vec<String>,
    pub engine_count: usize,
    pub jbeam_count: usize,
    pub pc_count: usize,
}

#[derive(Debug, Clone)]
pub enum ModScanMessage {
    Progress { current: usize, message: String },
    Mod(ModEntry),
    Finished { total: usize },
    Error(String),
}

pub struct ModScanner;

impl ModScanner {
    pub fn spawn_scan(settings: AppSettings) -> Receiver<ModScanMessage> {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            if let Err(err) = Self::scan_all(&settings, &tx) {
                let _ = tx.send(ModScanMessage::Error(err.to_string()));
            }
        });
        rx
    }

    fn scan_all(settings: &AppSettings, tx: &Sender<ModScanMessage>) -> AppResult<()> {
        let mut count = 0usize;
        let mut seen = BTreeSet::new();

        if let Some(mods) = settings.mods_dir() {
            Self::scan_mods_dir(&mods, tx, &mut count, &mut seen)?;
        }

        if let Some(game) = settings.game_loose_vehicles_dir() {
            Self::scan_loose_mods_in_game(&game, tx, &mut count, &mut seen)?;
        }

        let _ = tx.send(ModScanMessage::Finished { total: count });
        Ok(())
    }

    fn scan_mods_dir(
        mods_root: &Path,
        tx: &Sender<ModScanMessage>,
        count: &mut usize,
        seen: &mut BTreeSet<String>,
    ) -> AppResult<()> {
        for zip_dir in mod_zip_dirs(mods_root) {
            if zip_dir.file_name().and_then(|s| s.to_str()) == Some("unpacked") {
                continue;
            }
            if let Ok(read_dir) = fs::read_dir(&zip_dir) {
                for entry in read_dir.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("zip") {
                        Self::emit_zip_mod(&path, tx, count, seen)?;
                    }
                }
            }
        }

        let unpacked = mods_root.join("unpacked");
        if unpacked.is_dir() {
            if let Ok(read_dir) = fs::read_dir(&unpacked) {
                for entry in read_dir.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        Self::emit_unpacked_mod(&path, tx, count, seen)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn scan_loose_mods_in_game(
        game_vehicles: &Path,
        tx: &Sender<ModScanMessage>,
        count: &mut usize,
        seen: &mut BTreeSet<String>,
    ) -> AppResult<()> {
        if let Ok(read_dir) = fs::read_dir(game_vehicles) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                if name.eq_ignore_ascii_case("common") {
                    continue;
                }
                let stats = analyze_mod_root(&path)?;
                if stats.jbeam_count == 0 {
                    continue;
                }
                let id = format!("game_loose::{name}");
                if !seen.insert(id.clone()) {
                    continue;
                }
                *count += 1;
                let _ = tx.send(ModScanMessage::Progress {
                    current: *count,
                    message: path.display().to_string(),
                });
                let _ = tx.send(ModScanMessage::Mod(build_mod_entry(
                    id,
                    name,
                    ModLocation::Unpacked { root: path },
                    ModStorage::Unpacked,
                    stats,
                )));
            }
        }
        Ok(())
    }

    fn emit_unpacked_mod(
        mod_root: &Path,
        tx: &Sender<ModScanMessage>,
        count: &mut usize,
        seen: &mut BTreeSet<String>,
    ) -> AppResult<()> {
        let name = mod_root
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("mod")
            .to_string();
        let stats = analyze_mod_root(mod_root)?;
        if stats.jbeam_count == 0 && stats.pc_count == 0 {
            return Ok(());
        }
        let id = format!("unpacked::{name}");
        if !seen.insert(id.clone()) {
            return Ok(());
        }
        *count += 1;
        let _ = tx.send(ModScanMessage::Progress {
            current: *count,
            message: mod_root.display().to_string(),
        });
        let _ = tx.send(ModScanMessage::Mod(build_mod_entry(
            id,
            name,
            ModLocation::Unpacked {
                root: mod_root.to_path_buf(),
            },
            ModStorage::Unpacked,
            stats,
        )));
        Ok(())
    }

    fn emit_zip_mod(
        zip_path: &Path,
        tx: &Sender<ModScanMessage>,
        count: &mut usize,
        seen: &mut BTreeSet<String>,
    ) -> AppResult<()> {
        let name = zip_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("mod")
            .to_string();
        let stats = analyze_mod_zip(zip_path)?;
        if stats.jbeam_count == 0 && stats.pc_count == 0 {
            return Ok(());
        }
        let id = format!("zip::{}::{}", name, zip_path.display());
        if !seen.insert(id.clone()) {
            return Ok(());
        }
        *count += 1;
        let _ = tx.send(ModScanMessage::Progress {
            current: *count,
            message: zip_path.display().to_string(),
        });
        let _ = tx.send(ModScanMessage::Mod(build_mod_entry(
            id,
            name,
            ModLocation::Zip {
                archive_path: zip_path.to_path_buf(),
            },
            ModStorage::Packed,
            stats,
        )));
        Ok(())
    }
}

struct ModStats {
    target_vehicles: Vec<String>,
    engine_count: usize,
    jbeam_count: usize,
    pc_count: usize,
}

fn build_mod_entry(
    id: String,
    name: String,
    location: ModLocation,
    storage: ModStorage,
    stats: ModStats,
) -> ModEntry {
    let kind = classify_mod(&stats);
    ModEntry {
        id,
        name,
        location,
        storage,
        kind,
        target_vehicles: stats.target_vehicles,
        engine_count: stats.engine_count,
        jbeam_count: stats.jbeam_count,
        pc_count: stats.pc_count,
    }
}

fn classify_mod(stats: &ModStats) -> ModKind {
    if stats.pc_count == 0 && stats.engine_count > 0 {
        return ModKind::EngineParts;
    }
    if stats.pc_count > 0 {
        return ModKind::FullVehicle;
    }
    if stats.engine_count > 0 {
        return ModKind::EngineParts;
    }
    ModKind::PartsPack
}

fn analyze_mod_root(root: &Path) -> AppResult<ModStats> {
    let mut target_set = BTreeSet::new();
    let mut engine_count = 0usize;
    let mut jbeam_count = 0usize;
    let mut pc_count = 0usize;

    let vehicles_dir = root.join("vehicles");
    if vehicles_dir.is_dir() {
        collect_vehicle_targets(&vehicles_dir, &mut target_set);
    }

    for entry in WalkDir::new(root).follow_links(false).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        match path.extension().and_then(|e| e.to_str()) {
            Some("jbeam") => {
                jbeam_count += 1;
                if is_engine_jbeam(path)? {
                    engine_count += 1;
                }
            }
            Some("pc") => {
                pc_count += 1;
            }
            _ => {}
        }
    }

    let mut target_vehicles: Vec<String> = target_set.into_iter().collect();
    target_vehicles.sort();

    if let Ok(extra) = read_compat_file(root) {
        for vehicle in extra {
            if !target_vehicles.iter().any(|v| v == &vehicle) {
                target_vehicles.push(vehicle);
            }
        }
        target_vehicles.sort();
    }

    Ok(ModStats {
        target_vehicles,
        engine_count,
        jbeam_count,
        pc_count,
    })
}

fn analyze_mod_zip(zip_path: &Path) -> AppResult<ModStats> {
    let file = fs::File::open(zip_path)
        .map_err(|e| AppError::msg(format!("Open {}: {e}", zip_path.display())))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::msg(format!("Zip {}: {e}", zip_path.display())))?;

    let mut target_set = BTreeSet::new();
    let mut engine_count = 0usize;
    let mut jbeam_count = 0usize;
    let mut pc_count = 0usize;
    let mut jbeam_texts: Vec<String> = Vec::new();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().replace('\\', "/");
        if name.contains("/vehicles/") {
            if let Some(vehicle) = vehicle_folder_from_entry(&name) {
                if vehicle != "common" {
                    target_set.insert(vehicle);
                }
            }
        }
        if name.ends_with(".jbeam") {
            jbeam_count += 1;
            let mut text = String::new();
            file.read_to_string(&mut text)?;
            jbeam_texts.push(text);
        } else if name.ends_with(".pc") {
            pc_count += 1;
        }
    }

    for text in jbeam_texts {
        let lower = text.to_ascii_lowercase();
        if lower.contains("\"slottype\"")
            && (lower.contains("engine") || text.contains("Engine"))
        {
            engine_count += 1;
        }
    }

    let mut target_vehicles: Vec<String> = target_set.into_iter().collect();
    target_vehicles.sort();

    Ok(ModStats {
        target_vehicles,
        engine_count,
        jbeam_count,
        pc_count,
    })
}

fn collect_vehicle_targets(vehicles_dir: &Path, out: &mut BTreeSet<String>) {
    if let Ok(read_dir) = fs::read_dir(vehicles_dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.eq_ignore_ascii_case("common") {
                continue;
            }
            out.insert(name);
        }
    }
}

fn vehicle_folder_from_entry(entry: &str) -> Option<String> {
    let parts: Vec<&str> = entry.split('/').collect();
    if let Some(idx) = parts.iter().position(|p| *p == "vehicles") {
        if let Some(vehicle) = parts.get(idx + 1) {
            return Some((*vehicle).to_string());
        }
    }
    None
}

fn is_engine_jbeam(path: &Path) -> AppResult<bool> {
    let text = fs::read_to_string(path)?;
    let lower = text.to_ascii_lowercase();
    Ok(lower.contains("\"slottype\"")
        && (lower.contains("engine") || path.to_string_lossy().to_ascii_lowercase().contains("engine")))
}

const COMPAT_FILE: &str = ".beamng_editor_compat.json";

pub fn read_compat_file(mod_root: &Path) -> AppResult<Vec<String>> {
    let path = mod_root.join(COMPAT_FILE);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&text)?;
    let mut out = Vec::new();
    if let Some(arr) = value.get("target_vehicles").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(s) = item.as_str() {
                out.push(s.to_string());
            }
        }
    }
    Ok(out)
}

pub fn write_compat_file(mod_root: &Path, vehicles: &[String]) -> AppResult<()> {
    let path = mod_root.join(COMPAT_FILE);
    let value = serde_json::json!({ "target_vehicles": vehicles });
    fs::write(path, serde_json::to_string_pretty(&value)?)?;
    Ok(())
}

pub fn mod_root_path(entry: &ModEntry) -> Option<PathBuf> {
    match &entry.location {
        ModLocation::Unpacked { root } => Some(root.clone()),
        ModLocation::Zip { .. } => None,
    }
}

pub fn add_vehicle_folder(mod_root: &Path, vehicle: &str, template: Option<&str>) -> AppResult<()> {
    let vehicles_dir = mod_root.join("vehicles");
    fs::create_dir_all(&vehicles_dir)?;
    let dst = vehicles_dir.join(vehicle);
    if dst.exists() {
        return Err(AppError::msg(format!("Vehicle folder already exists: {vehicle}")));
    }

    if let Some(template_name) = template {
        let src = vehicles_dir.join(template_name);
        if src.is_dir() {
            copy_engine_jbeams(&src, &dst)?;
            return Ok(());
        }
    }

    fs::create_dir_all(&dst)?;
    let stub = format!(
        "{{\n  \"{vehicle}_engine_adapter\": {{\n    \"slotType\": \"mainEngine\",\n    \"information\": {{ \"name\": \"Engine adapter for {vehicle}\" }}\n  }}\n}}\n"
    );
    fs::write(dst.join(format!("{vehicle}_engine_adapter.jbeam")), stub)?;
    Ok(())
}

pub fn remove_vehicle_folder(mod_root: &Path, vehicle: &str) -> AppResult<()> {
    let path = mod_root.join("vehicles").join(vehicle);
    if !path.is_dir() {
        return Err(AppError::msg(format!("Folder not found: {vehicle}")));
    }
    fs::remove_dir_all(path)?;
    Ok(())
}

fn copy_engine_jbeams(src: &Path, dst: &Path) -> AppResult<()> {
    fs::create_dir_all(dst)?;
    for entry in WalkDir::new(src).follow_links(false).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("jbeam") {
            continue;
        }
        let name = path.file_name().ok_or_else(|| AppError::msg("Missing file name"))?;
        let name_str = name.to_string_lossy().to_lowercase();
        if !name_str.contains("engine") {
            continue;
        }
        fs::copy(path, dst.join(name))?;
    }
    Ok(())
}

pub fn all_known_models(entries: &[ModEntry], vehicles: &[crate::scanner::VehicleEntry]) -> Vec<String> {
    let mut models: BTreeSet<String> = BTreeSet::new();
    for v in vehicles {
        if !v.model_key.is_empty() {
            models.insert(v.model_key.clone());
        }
    }
    for m in entries {
        for t in &m.target_vehicles {
            models.insert(t.clone());
        }
    }
    models.into_iter().collect()
}

pub fn is_editable(entry: &ModEntry) -> bool {
    entry.storage == ModStorage::Unpacked
}

pub fn mods_dir(settings: &AppSettings) -> AppResult<PathBuf> {
    settings
        .mods_dir()
        .ok_or_else(|| AppError::msg("Mods folder not configured"))
}

pub fn unpack_mod(zip_path: &Path, mods_root: &Path) -> AppResult<PathBuf> {
    let unpacked_root = mods_root.join("unpacked");
    fs::create_dir_all(&unpacked_root)?;
    let stem = zip_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("mod");
    let dest = unpacked_root.join(stem);
    if dest.exists() {
        return Err(AppError::msg(format!(
            "Already unpacked at {}",
            dest.display()
        )));
    }
    extract_zip_to_dir(zip_path, &dest)?;
    Ok(dest)
}

pub fn pack_mod(folder: &Path, mods_root: &Path) -> AppResult<PathBuf> {
    let packed_root = mods_root.join("packed");
    fs::create_dir_all(&packed_root)?;
    let name = folder
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("mod");
    let zip_path = packed_root.join(format!("{name}.zip"));
    create_zip_from_dir(folder, &zip_path)?;
    Ok(zip_path)
}

pub fn add_vehicle_to_mod(
    entry: &ModEntry,
    vehicle: &str,
    template: Option<&str>,
) -> AppResult<()> {
    let root = mod_root_path(entry).ok_or_else(|| {
        AppError::msg("Unpack this mod before editing vehicle compatibility")
    })?;
    add_vehicle_folder(&root, vehicle, template)?;
    let mut vehicles = entry.target_vehicles.clone();
    if !vehicles.iter().any(|v| v == vehicle) {
        vehicles.push(vehicle.to_string());
        vehicles.sort();
        let _ = write_compat_file(&root, &vehicles);
    }
    Ok(())
}

fn extract_zip_to_dir(zip_path: &Path, dest_dir: &Path) -> AppResult<()> {
    let file = fs::File::open(zip_path)
        .map_err(|e| AppError::msg(format!("Open {}: {e}", zip_path.display())))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::msg(format!("Zip {}: {e}", zip_path.display())))?;
    fs::create_dir_all(dest_dir)?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().replace('\\', "/");
        if name.contains("..") {
            continue;
        }
        let out_path = dest_dir.join(&name);
        if name.ends_with('/') {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut outfile = fs::File::create(&out_path)?;
        std::io::copy(&mut file, &mut outfile)?;
    }
    Ok(())
}

fn create_zip_from_dir(source: &Path, zip_path: &Path) -> AppResult<()> {
    let out_file = fs::File::create(zip_path)?;
    let mut writer = zip::ZipWriter::new(out_file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for entry in WalkDir::new(source).follow_links(false).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        let rel = path
            .strip_prefix(source)
            .map_err(|e| AppError::msg(e.to_string()))?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        let name = rel.to_string_lossy().replace('\\', "/");
        if path.is_dir() {
            writer.add_directory(format!("{name}/"), options)?;
        } else {
            writer.start_file(name, options)?;
            let mut f = fs::File::open(path)?;
            io::copy(&mut f, &mut writer)?;
        }
    }
    writer.finish()?;
    Ok(())
}
