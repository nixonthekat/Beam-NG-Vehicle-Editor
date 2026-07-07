use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use walkdir::WalkDir;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct EnginePart {
    pub id: String,
    pub name: String,
    pub slot_types: Vec<String>,
    pub source_file: PathBuf,
    pub mod_name: String,
}

#[derive(Debug, Clone)]
pub enum EngineScanMessage {
    Progress { scanned: usize, message: String },
    Engine(EnginePart),
    Finished { total: usize },
    Error(String),
}

pub struct EngineScanner;

impl EngineScanner {
    pub fn spawn_scan(mods_root: PathBuf) -> Receiver<EngineScanMessage> {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            if let Err(err) = Self::scan_engines(&mods_root, &tx) {
                let _ = tx.send(EngineScanMessage::Error(err.to_string()));
            }
        });
        rx
    }

    fn scan_engines(root: &Path, tx: &mpsc::Sender<EngineScanMessage>) -> AppResult<()> {
        if !root.is_dir() {
            return Err(AppError::msg("Mods root is not a directory"));
        }

        let mut scanned = 0usize;
        let mut seen_ids = BTreeSet::new();

        for entry in WalkDir::new(root)
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

            scanned += 1;
            if scanned % 25 == 0 {
                let _ = tx.send(EngineScanMessage::Progress {
                    scanned,
                    message: path.display().to_string(),
                });
            }

            let mod_name = path
                .strip_prefix(root)
                .ok()
                .and_then(|p| p.components().next())
                .and_then(|c| c.as_os_str().to_str())
                .unwrap_or("unknown")
                .to_string();

            match parse_jbeam_engines(path, &mod_name) {
                Ok(parts) => {
                    for part in parts {
                        if seen_ids.insert(part.id.clone()) {
                            let _ = tx.send(EngineScanMessage::Engine(part));
                        }
                    }
                }
                Err(_) => continue,
            }
        }

        let _ = tx.send(EngineScanMessage::Finished { total: scanned });
        Ok(())
    }
}

fn parse_jbeam_engines(path: &Path, mod_name: &str) -> AppResult<Vec<EnginePart>> {
    let text = std::fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&text)?;
    let mut engines = Vec::new();

    let Some(root_obj) = value.as_object() else {
        return Ok(engines);
    };

    for (part_id, part_val) in root_obj {
        if part_id.starts_with("__") || part_id.starts_with("//") {
            continue;
        }
        let Some(part_obj) = part_val.as_object() else {
            continue;
        };

        let slot_types = collect_slot_types(part_obj);
        let is_engine = slot_types.iter().any(|s| is_engine_slot_type(s))
            || part_id.to_ascii_lowercase().contains("engine");

        let name = part_obj
            .get("information")
            .and_then(|i| i.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or(part_id)
            .to_string();

        let part_type = part_obj
            .get("slotType")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let looks_like_engine = is_engine
            || is_engine_slot_type(part_type)
            || (part_type.is_empty()
                && (name.to_ascii_lowercase().contains("engine")
                    || part_id.to_ascii_lowercase().contains("engine")));

        if !looks_like_engine {
            continue;
        }

        engines.push(EnginePart {
            id: part_id.clone(),
            name,
            slot_types: if slot_types.is_empty() && !part_type.is_empty() {
                vec![part_type.to_string()]
            } else {
                slot_types
            },
            source_file: path.to_path_buf(),
            mod_name: mod_name.to_string(),
        });
    }

    Ok(engines)
}

fn collect_slot_types(part_obj: &serde_json::Map<String, serde_json::Value>) -> Vec<String> {
    let mut slots = Vec::new();

    if let Some(st) = part_obj.get("slotType").and_then(|v| v.as_str()) {
        slots.push(st.to_string());
    }

    if let Some(slots_val) = part_obj.get("slots") {
        collect_slots_recursive(slots_val, &mut slots);
    }

    slots.sort();
    slots.dedup();
    slots
}

fn collect_slots_recursive(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Array(arr) => {
            for item in arr {
                if let Some(slot_type) = item.get("type").and_then(|v| v.as_str()) {
                    out.push(slot_type.to_string());
                }
                if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                    if is_engine_slot_type(name) {
                        out.push(name.to_string());
                    }
                }
                collect_slots_recursive(item, out);
            }
        }
        serde_json::Value::Object(obj) => {
            for (k, v) in obj {
                if k == "type" {
                    if let Some(s) = v.as_str() {
                        out.push(s.to_string());
                    }
                } else {
                    collect_slots_recursive(v, out);
                }
            }
        }
        _ => {}
    }
}

fn is_engine_slot_type(slot: &str) -> bool {
    let lower = slot.to_ascii_lowercase();
    lower.contains("engine") || lower == "mainengine" || lower == "main_engine"
}

pub fn filter_engines<'a>(
    engines: &'a [EnginePart],
    query: &str,
    mod_filter: Option<&str>,
) -> Vec<&'a EnginePart> {
    let q = query.trim().to_ascii_lowercase();
    engines
        .iter()
        .filter(|e| {
            if let Some(m) = mod_filter {
                if !e.mod_name.eq_ignore_ascii_case(m) {
                    return false;
                }
            }
            if q.is_empty() {
                return true;
            }
            e.id.to_ascii_lowercase().contains(&q)
                || e.name.to_ascii_lowercase().contains(&q)
                || e.mod_name.to_ascii_lowercase().contains(&q)
        })
        .collect()
}

pub fn engines_by_mod(engines: &[EnginePart]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for e in engines {
        *map.entry(e.mod_name.clone()).or_insert(0) += 1;
    }
    map
}
