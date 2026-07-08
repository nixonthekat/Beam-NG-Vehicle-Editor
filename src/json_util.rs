use serde_json::Value;

use crate::error::{AppError, AppResult};

/// Parse BeamNG `.pc` JSON (allows trailing commas and repairs common typos).
pub fn parse_beamng_json(text: &str) -> AppResult<Value> {
    if let Ok(value) = serde_json::from_str(text) {
        return Ok(value);
    }
    if let Ok(value) = json5::from_str(text) {
        return Ok(value);
    }
    let repaired = repair_missing_commas(text);
    if let Ok(value) = json5::from_str(&repaired) {
        return Ok(value);
    }
    json5::from_str(text).map_err(|err| AppError::msg(format!("JSON error: {err}")))
}

/// Inserts missing commas between consecutive `"key": "value"` lines (seen in some stock configs).
fn repair_missing_commas(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_end();
        if i + 1 < lines.len() {
            let next = lines[i + 1].trim();
            if trimmed.ends_with('"')
                && !trimmed.ends_with(',')
                && !trimmed.ends_with('{')
                && !trimmed.ends_with('[')
                && next.starts_with('"')
            {
                out.push(format!("{trimmed},"));
                continue;
            }
        }
        out.push((*line).to_string());
    }
    out.join("\n")
}

pub fn validate_beamng_json(text: &str) -> AppResult<()> {
    parse_beamng_json(text)?;
    Ok(())
}
