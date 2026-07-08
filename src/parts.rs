use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use walkdir::WalkDir;

use crate::config::ENGINE_SLOT_NAMES;
use crate::error::{AppError, AppResult};
use crate::scan_util::mod_zip_dirs;
use crate::settings::AppSettings;

pub const CUSTOM_OPTION: &str = "__custom__";

#[derive(Debug, Clone)]
pub struct PartEntry {
    pub id: String,
    pub name: String,
    pub slot_type: String,
    pub mod_name: String,
    pub is_engine: bool,
    pub source_file: PathBuf,
}

#[derive(Debug, Clone)]
pub struct EngineModInfo {
    pub mod_name: String,
    pub engine_count: usize,
    pub is_engine_only_mod: bool,
}

#[derive(Debug, Clone, Default)]
pub struct PartsIndex {
    pub by_slot_type: HashMap<String, Vec<PartEntry>>,
    pub engines: Vec<PartEntry>,
    pub engine_mods: Vec<EngineModInfo>,
    pub all_by_id: HashMap<String, PartEntry>,
}

#[derive(Debug, Clone)]
pub enum PartsScanMessage {
    Progress { scanned: usize, message: String },
    Part(PartEntry),
    Finished { total: usize },
    Error(String),
}

impl PartsIndex {
    pub fn dropdown_options(&self, slot: &str, current: &str) -> Vec<PartEntry> {
        let mut seen = HashSet::new();
        let mut options = Vec::new();

        let mut add = |part: &PartEntry| {
            if seen.insert(part.id.clone()) {
                options.push(part.clone());
            }
        };

        if let Some(parts) = self.by_slot_type.get(slot) {
            for p in parts {
                add(p);
            }
        }

        let slot_lower = slot.to_ascii_lowercase();
        for (key, parts) in &self.by_slot_type {
            let key_lower = key.to_ascii_lowercase();
            if key != slot
                && (key_lower.contains(&slot_lower)
                    || slot_lower.contains(&key_lower)
                    || (slot_lower.contains("engine") && key_lower.contains("engine")))
            {
                for p in parts {
                    add(p);
                }
            }
        }

        if is_engine_slot_name(slot) {
            for engine in &self.engines {
                add(engine);
            }
        }

        if !current.is_empty() {
            if let Some(p) = self.all_by_id.get(current) {
                add(p);
            } else {
                add(&PartEntry {
                    id: current.to_string(),
                    name: format!("Current: {current}"),
                    slot_type: slot.to_string(),
                    mod_name: "installed".to_string(),
                    is_engine: is_engine_slot_name(slot),
                    source_file: PathBuf::new(),
                });
            }
        }

        options.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        options
    }

    pub fn finalize_from_parts(parts: Vec<PartEntry>) -> Self {
        let mut by_slot_type: HashMap<String, Vec<PartEntry>> = HashMap::new();
        let mut all_by_id = HashMap::new();
        let mut engines = Vec::new();
        let mut mod_stats: HashMap<String, (usize, usize, bool)> = HashMap::new();

        for part in parts {
            if part.is_engine {
                engines.push(part.clone());
            }
            all_by_id.insert(part.id.clone(), part.clone());
            by_slot_type
                .entry(part.slot_type.clone())
                .or_default()
                .push(part.clone());

            let stats = mod_stats.entry(part.mod_name.clone()).or_insert((0, 0, false));
            stats.1 += 1;
            if part.is_engine {
                stats.0 += 1;
            }
        }

        for list in by_slot_type.values_mut() {
            list.sort_by(|a, b| a.name.cmp(&b.name));
        }
        engines.sort_by(|a, b| a.name.cmp(&b.name));

        let engine_mods = mod_stats
            .into_iter()
            .filter(|(_, (eng, total, _))| *eng > 0 && (*eng >= 2 || *total <= 5))
            .map(|(mod_name, (engine_count, total, _))| {
                let lower = mod_name.to_ascii_lowercase();
                let is_engine_only_mod = lower.contains("engine")
                    || lower.contains("motor")
                    || (engine_count >= 2 && engine_count * 2 >= total);
                EngineModInfo {
                    mod_name,
                    engine_count,
                    is_engine_only_mod,
                }
            })
            .collect();

        Self {
            by_slot_type,
            engines,
            engine_mods,
            all_by_id,
        }
    }
}

pub struct PartsScanner;

impl PartsScanner {
    pub fn spawn_scan(settings: AppSettings) -> Receiver<PartsScanMessage> {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            if let Err(err) = Self::scan(&settings, &tx) {
                let _ = tx.send(PartsScanMessage::Error(err.to_string()));
            }
        });
        rx
    }

    fn scan(settings: &AppSettings, tx: &Sender<PartsScanMessage>) -> AppResult<()> {
        let mut roots = Vec::new();
        if let Some(mods) = settings.mods_dir() {
            roots.push(mods);
        }
        if let Some(game) = settings.game_loose_vehicles_dir() {
            roots.push(game);
        }

        if roots.is_empty() && settings.game_vehicles_dir().is_none() {
            return Err(AppError::msg("Set BeamNG user folder or BeamNG.exe in Settings"));
        }

        let mut scanned = 0usize;
        let mut seen_ids = BTreeSet::new();

        for root in roots {
            let mod_label = root
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("mod");
            scan_jbeam_tree(&root, &root, mod_label, tx, &mut scanned, &mut seen_ids)?;
            scan_unpacked_mods(&root, tx, &mut scanned, &mut seen_ids)?;
            for zip_dir in mod_zip_dirs(&root) {
                if zip_dir.file_name().and_then(|s| s.to_str()) == Some("unpacked") {
                    continue;
                }
                scan_zip_jbeams_in_dir(&zip_dir, tx, &mut scanned, &mut seen_ids)?;
            }
        }

        if let Some(stock_dir) = settings.game_vehicles_dir() {
            scan_stock_zip_jbeams(&stock_dir, tx, &mut scanned, &mut seen_ids)?;
        }

        let _ = tx.send(PartsScanMessage::Finished { total: scanned });
        Ok(())
    }
}

fn scan_jbeam_tree(
    _root: &Path,
    scan_root: &Path,
    mod_label: &str,
    tx: &Sender<PartsScanMessage>,
    scanned: &mut usize,
    seen_ids: &mut BTreeSet<String>,
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
        if path.extension().and_then(|e| e.to_str()) != Some("jbeam") {
            continue;
        }

        *scanned += 1;
        if *scanned % 40 == 0 {
            let _ = tx.send(PartsScanMessage::Progress {
                scanned: *scanned,
                message: path.display().to_string(),
            });
        }

        if let Ok(parts) = parse_jbeam_parts(path, mod_label) {
            for part in parts {
                if seen_ids.insert(part.id.clone()) {
                    let _ = tx.send(PartsScanMessage::Part(part));
                }
            }
        }
    }
    Ok(())
}

fn scan_unpacked_mods(
    mods_root: &Path,
    tx: &Sender<PartsScanMessage>,
    scanned: &mut usize,
    seen_ids: &mut BTreeSet<String>,
) -> AppResult<()> {
    let unpacked = mods_root.join("unpacked");
    if !unpacked.is_dir() {
        return Ok(());
    }
    if let Ok(read_dir) = std::fs::read_dir(&unpacked) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let mod_label = entry
                    .file_name()
                    .to_string_lossy()
                    .to_string();
                scan_jbeam_tree(&path, &path, &mod_label, tx, scanned, seen_ids)?;
            }
        }
    }
    Ok(())
}

fn scan_zip_jbeams_in_dir(
    dir: &Path,
    tx: &Sender<PartsScanMessage>,
    scanned: &mut usize,
    seen_ids: &mut BTreeSet<String>,
) -> AppResult<()> {
    if let Ok(read_dir) = std::fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("zip") {
                scan_jbeams_in_zip(
                    &path,
                    path.file_stem().and_then(|s| s.to_str()).unwrap_or("mod"),
                    tx,
                    scanned,
                    seen_ids,
                )?;
            }
        }
    }
    Ok(())
}

fn scan_stock_zip_jbeams(
    stock_dir: &Path,
    tx: &Sender<PartsScanMessage>,
    scanned: &mut usize,
    seen_ids: &mut BTreeSet<String>,
) -> AppResult<()> {
    if let Ok(read_dir) = std::fs::read_dir(stock_dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("zip") {
                scan_jbeams_in_zip(&path, "BeamNG", tx, scanned, seen_ids)?;
            }
        }
    }
    Ok(())
}

fn scan_jbeams_in_zip(
    zip_path: &Path,
    mod_name: &str,
    tx: &Sender<PartsScanMessage>,
    scanned: &mut usize,
    seen_ids: &mut BTreeSet<String>,
) -> AppResult<()> {
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().replace('\\', "/");
        if !name.ends_with(".jbeam") {
            continue;
        }
        *scanned += 1;
        let mut text = String::new();
        file.read_to_string(&mut text)?;
        if let Ok(parts) = parse_jbeam_text(&text, mod_name, zip_path) {
            for part in parts {
                if seen_ids.insert(part.id.clone()) {
                    let _ = tx.send(PartsScanMessage::Part(part));
                }
            }
        }
    }
    Ok(())
}

fn parse_jbeam_text(text: &str, mod_name: &str, source: &Path) -> AppResult<Vec<PartEntry>> {
    let value: serde_json::Value = serde_json::from_str(text)?;
    let mut parts = Vec::new();
    let Some(root_obj) = value.as_object() else {
        return Ok(parts);
    };
    for (part_id, part_val) in root_obj {
        if part_id.starts_with("__") {
            continue;
        }
        let Some(part_obj) = part_val.as_object() else {
            continue;
        };
        let slot_type = part_obj
            .get("slotType")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let name = part_obj
            .get("information")
            .and_then(|i| i.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or(part_id)
            .to_string();
        let is_engine = is_engine_slot_name(&slot_type)
            || part_id.to_ascii_lowercase().contains("engine")
            || name.to_ascii_lowercase().contains("engine");
        if slot_type.is_empty() && !is_engine {
            continue;
        }
        parts.push(PartEntry {
            id: part_id.clone(),
            name,
            slot_type: if slot_type.is_empty() {
                "engine".to_string()
            } else {
                slot_type
            },
            mod_name: mod_name.to_string(),
            is_engine,
            source_file: source.to_path_buf(),
        });
    }
    Ok(parts)
}

fn infer_mod_name(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .ok()
        .and_then(|p| p.components().next())
        .and_then(|c| c.as_os_str().to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn parse_jbeam_parts(path: &Path, mod_name: &str) -> AppResult<Vec<PartEntry>> {
    let text = std::fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&text)?;
    let mut parts = Vec::new();

    let Some(root_obj) = value.as_object() else {
        return Ok(parts);
    };

    for (part_id, part_val) in root_obj {
        if part_id.starts_with("__") {
            continue;
        }
        let Some(part_obj) = part_val.as_object() else {
            continue;
        };

        let slot_type = part_obj
            .get("slotType")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let name = part_obj
            .get("information")
            .and_then(|i| i.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or(part_id)
            .to_string();

        let is_engine = is_engine_slot_name(&slot_type)
            || part_id.to_ascii_lowercase().contains("engine")
            || name.to_ascii_lowercase().contains("engine");

        if slot_type.is_empty() && !is_engine {
            continue;
        }

        parts.push(PartEntry {
            id: part_id.clone(),
            name,
            slot_type: if slot_type.is_empty() {
                "engine".to_string()
            } else {
                slot_type
            },
            mod_name: mod_name.to_string(),
            is_engine,
            source_file: path.to_path_buf(),
        });
    }

    Ok(parts)
}

pub fn is_engine_slot_name(slot: &str) -> bool {
    let lower = slot.to_ascii_lowercase();
    ENGINE_SLOT_NAMES
        .iter()
        .any(|s| lower.contains(&s.to_ascii_lowercase()))
        || lower.contains("engine")
}

pub fn friendly_slot_label(slot: &str) -> String {
    let cleaned = slot
        .replace('_', " ")
        .split_whitespace()
        .map(capitalize_word)
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.to_ascii_lowercase().contains("engine") {
        return format!("⚙ {cleaned}");
    }
    cleaned
}

fn capitalize_word(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

pub fn engine_mods_sorted(index: &PartsIndex) -> Vec<&EngineModInfo> {
    let mut mods: Vec<_> = index.engine_mods.iter().collect();
    mods.sort_by(|a, b| {
        b.is_engine_only_mod
            .cmp(&a.is_engine_only_mod)
            .then(b.engine_count.cmp(&a.engine_count))
            .then(a.mod_name.cmp(&b.mod_name))
    });
    mods
}

pub fn filter_parts_for_mod_entry<'a>(
    index: &'a PartsIndex,
    mod_name: &str,
    query: &str,
) -> Vec<&'a PartEntry> {
    let q = query.trim().to_ascii_lowercase();
    let name_lower = mod_name.to_ascii_lowercase();
    index
        .engines
        .iter()
        .filter(|e| {
            e.mod_name.eq_ignore_ascii_case(mod_name)
                || e
                    .source_file
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .contains(&name_lower)
        })
        .filter(|e| {
            q.is_empty()
                || e.id.to_ascii_lowercase().contains(&q)
                || e.name.to_ascii_lowercase().contains(&q)
        })
        .collect()
}
