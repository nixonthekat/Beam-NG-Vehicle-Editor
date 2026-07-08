use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};

/// Known BeamNG engine slot names (checked in order).
pub const ENGINE_SLOT_NAMES: &[&str] = &[
    "mainEngine",
    "engine",
    "main_engine",
    "engineMain",
    "engineSlot",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VehicleConfig {
    pub path: PathBuf,
    pub raw: Value,
    pub parts: BTreeMap<String, String>,
}

impl VehicleConfig {
    pub fn load(path: &Path) -> AppResult<Self> {
        let text = fs::read_to_string(path)?;
        let raw = crate::json_util::parse_beamng_json(&text)?;
        let parts = extract_parts(&raw)?;
        Ok(Self {
            path: path.to_path_buf(),
            raw,
            parts,
        })
    }

    pub fn reload_if_changed(&mut self) -> AppResult<bool> {
        let updated = Self::load(&self.path)?;
        let changed = updated.raw != self.raw;
        if changed {
            *self = updated;
        }
        Ok(changed)
    }

    pub fn engine_slot(&self) -> Option<(&str, &str)> {
        for slot in ENGINE_SLOT_NAMES {
            if let Some(part) = self.parts.get(*slot) {
                return Some((slot, part.as_str()));
            }
        }
        self.parts
            .iter()
            .find(|(slot, _)| slot.to_ascii_lowercase().contains("engine"))
            .map(|(slot, part)| (slot.as_str(), part.as_str()))
    }

    pub fn set_part(&mut self, slot: &str, part_id: &str) {
        self.parts.insert(slot.to_string(), part_id.to_string());
        if let Some(parts_obj) = self.raw.get_mut("parts").and_then(|v| v.as_object_mut()) {
            parts_obj.insert(
                slot.to_string(),
                Value::String(part_id.to_string()),
            );
        } else {
            let mut parts_obj = serde_json::Map::new();
            for (k, v) in &self.parts {
                parts_obj.insert(k.clone(), Value::String(v.clone()));
            }
            if let Some(obj) = self.raw.as_object_mut() {
                obj.insert("parts".to_string(), Value::Object(parts_obj));
            }
        }
    }

    pub fn to_pretty_json(&self) -> AppResult<String> {
        Ok(serde_json::to_string_pretty(&self.raw)?)
    }

    pub fn save(&self) -> AppResult<()> {
        validate_json(&self.raw)?;
        let text = self.to_pretty_json()?;
        fs::write(&self.path, text)?;
        Ok(())
    }

    pub fn diff_summary(&self, other: &Self) -> Vec<(String, String, String)> {
        let mut changes = Vec::new();
        let mut keys: Vec<_> = self.parts.keys().chain(other.parts.keys()).collect();
        keys.sort();
        keys.dedup();
        for key in keys {
            let a = self.parts.get(key).map(String::as_str).unwrap_or("");
            let b = other.parts.get(key).map(String::as_str).unwrap_or("");
            if a != b {
                changes.push((key.clone(), a.to_string(), b.to_string()));
            }
        }
        changes
    }
}

fn extract_parts(raw: &Value) -> AppResult<BTreeMap<String, String>> {
    let mut parts = BTreeMap::new();
    if let Some(obj) = raw.get("parts").and_then(|v| v.as_object()) {
        for (k, v) in obj {
            if let Some(s) = v.as_str() {
                parts.insert(k.clone(), s.to_string());
            }
        }
    }
    Ok(parts)
}

pub fn validate_json(value: &Value) -> AppResult<()> {
    serde_json::to_string(value)?;
    if let Some(parts) = value.get("parts") {
        if !parts.is_object() {
            return Err(AppError::msg("'parts' must be a JSON object"));
        }
    }
    Ok(())
}

pub fn vehicle_key_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}
