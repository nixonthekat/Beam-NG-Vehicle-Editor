use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};
use crate::settings::AppSettings;

/// Where a vehicle `.pc` config lives (disk or inside a mod zip).
#[derive(Debug, Clone)]
pub enum VehicleLocation {
    Local {
        config_path: PathBuf,
    },
    Zip {
        archive_path: PathBuf,
        pc_entry: String,
        /// Extracted copy used for editing; populated after first load.
        cached_path: Option<PathBuf>,
    },
}

impl VehicleLocation {
    pub fn config_path(&self) -> Option<&Path> {
        match self {
            VehicleLocation::Local { config_path } => Some(config_path),
            VehicleLocation::Zip { cached_path, .. } => cached_path.as_deref(),
        }
    }

    pub fn display_label(&self) -> String {
        match self {
            VehicleLocation::Local { config_path } => config_path.display().to_string(),
            VehicleLocation::Zip {
                archive_path,
                pc_entry,
                ..
            } => format!("{}!{}", archive_path.display(), pc_entry),
        }
    }

    pub fn ensure_local_path(&mut self, settings: &AppSettings) -> AppResult<PathBuf> {
        match self {
            VehicleLocation::Local { config_path } => Ok(config_path.clone()),
            VehicleLocation::Zip {
                archive_path,
                pc_entry,
                cached_path,
            } => {
                if let Some(path) = cached_path {
                    if path.exists() {
                        return Ok(path.clone());
                    }
                }
                let extracted = extract_zip_entry(settings, archive_path, pc_entry)?;
                *cached_path = Some(extracted.clone());
                Ok(extracted)
            }
        }
    }

    pub fn write_back(&mut self, local_path: &Path) -> AppResult<()> {
        match self {
            VehicleLocation::Local { config_path } => {
                if config_path != local_path {
                    fs::copy(local_path, config_path)?;
                }
                Ok(())
            }
            VehicleLocation::Zip {
                archive_path,
                pc_entry,
                cached_path,
            } => {
                replace_zip_entry(archive_path, pc_entry, local_path)?;
                *cached_path = Some(local_path.to_path_buf());
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ThumbnailSource {
    File(PathBuf),
    Zip {
        archive_path: PathBuf,
        entry: String,
    },
}

pub fn cache_dir(settings: &AppSettings) -> AppResult<PathBuf> {
    let dir = settings.cache_root()?;
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn cache_key(archive: &Path, entry: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(archive.to_string_lossy().as_bytes());
    hasher.update(entry.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn extract_zip_entry(
    settings: &AppSettings,
    archive_path: &Path,
    entry: &str,
) -> AppResult<PathBuf> {
    let key = cache_key(archive_path, entry);
    let out_dir = cache_dir(settings)?.join(&key);
    fs::create_dir_all(&out_dir)?;
    let file_name = Path::new(entry)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("config.pc");
    let out_path = out_dir.join(file_name);

    let archive = zip::ZipArchive::new(
        fs::File::open(archive_path)
            .map_err(|e| AppError::msg(format!("Open zip {}: {e}", archive_path.display())))?,
    )
    .map_err(|e| AppError::msg(format!("Read zip {}: {e}", archive_path.display())))?;

    let mut zip = archive;
    let mut file = zip
        .by_name(entry)
        .map_err(|e| AppError::msg(format!("Entry '{entry}' in {}: {e}", archive_path.display())))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    fs::write(&out_path, bytes)?;
    Ok(out_path)
}

pub fn read_zip_entry_bytes(archive_path: &Path, entry: &str) -> AppResult<Vec<u8>> {
    let file = fs::File::open(archive_path)
        .map_err(|e| AppError::msg(format!("Open zip {}: {e}", archive_path.display())))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::msg(format!("Read zip {}: {e}", archive_path.display())))?;
    let mut zip_file = archive
        .by_name(entry)
        .map_err(|e| AppError::msg(format!("Entry '{entry}': {e}")))?;
    let mut bytes = Vec::new();
    zip_file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn replace_zip_entry(archive_path: &Path, entry: &str, new_content: &Path) -> AppResult<()> {
    let file = fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::msg(format!("Read zip: {e}")))?;

    let temp_path = archive_path.with_extension("zip.tmp");
    let out_file = fs::File::create(&temp_path)?;
    let mut writer = zip::ZipWriter::new(out_file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let new_bytes = fs::read(new_content)?;
    let entry_normalized = entry.replace('\\', "/");

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();
        let name_norm = name.replace('\\', "/");

        if name_norm == entry_normalized {
            continue;
        }

        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        writer.start_file(name_norm, options)?;
        writer.write_all(&buf)?;
    }

    writer.start_file(entry_normalized, options)?;
    writer.write_all(&new_bytes)?;
    writer.finish()?;

    fs::rename(&temp_path, archive_path)?;
    Ok(())
}

pub fn find_thumbnail_for_pc(
    pc_path: &Path,
    pc_folder: &Path,
    stem: &str,
    beamng_user_dir: Option<&Path>,
) -> Option<ThumbnailSource> {
    find_thumbnail_in_folder(pc_folder, stem).map(ThumbnailSource::File).or_else(|| {
        beamng_user_dir.and_then(|user| {
            let vehicle_name = pc_folder
                .file_name()
                .or_else(|| pc_path.parent()?.file_name())
                .and_then(|s| s.to_str())?;
            let user_vehicle = user.join("vehicles").join(vehicle_name);
            find_thumbnail_in_folder(&user_vehicle, stem).map(ThumbnailSource::File)
        })
    })
}

pub fn find_thumbnail_in_zip(
    archive_path: &Path,
    pc_entry: &str,
    stem: &str,
    beamng_user_dir: Option<&Path>,
) -> Option<ThumbnailSource> {
    let folder = zip_entry_folder(pc_entry);
    if let Ok(file) = fs::File::open(archive_path) {
        if let Ok(mut archive) = zip::ZipArchive::new(file) {
            if let Some(entry) = pick_thumbnail_from_zip_index(&mut archive, &folder, stem) {
                return Some(ThumbnailSource::Zip {
                    archive_path: archive_path.to_path_buf(),
                    entry,
                });
            }
        }
    }

    beamng_user_dir.and_then(|user| {
        let vehicle_name = Path::new(pc_entry)
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())?;
        let user_vehicle = user.join("vehicles").join(vehicle_name);
        find_thumbnail_in_folder(&user_vehicle, stem).map(ThumbnailSource::File)
    })
}

fn zip_entry_folder(entry: &str) -> String {
    let norm = entry.replace('\\', "/");
    Path::new(&norm)
        .parent()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

fn pick_thumbnail_from_zip_index(
    archive: &mut zip::ZipArchive<fs::File>,
    folder: &str,
    stem: &str,
) -> Option<String> {
    let folder_prefix = if folder.is_empty() {
        String::new()
    } else {
        format!("{}/", folder.trim_end_matches('/'))
    };

    let candidates = thumbnail_candidate_names(stem);
    let mut ui_images: Vec<String> = Vec::new();
    let mut any_image: Vec<String> = Vec::new();

    for i in 0..archive.len() {
        let file = archive.by_index(i).ok()?;
        let name = file.name().replace('\\', "/");
        if !name.starts_with(&folder_prefix) {
            continue;
        }
        let rel = name.strip_prefix(&folder_prefix).unwrap_or(&name);
        if rel.contains('/') {
            if rel.starts_with("ui/") {
                if is_image_name(rel) {
                    ui_images.push(name.clone());
                }
            }
            continue;
        }
        if is_image_name(rel) {
            any_image.push(name.clone());
        }
    }

    for candidate in &candidates {
        let target = format!("{folder_prefix}{candidate}");
        if any_image.iter().any(|n| n.eq_ignore_ascii_case(&target)) {
            return any_image
                .into_iter()
                .find(|n| n.eq_ignore_ascii_case(&target));
        }
    }

    ui_images.sort();
    ui_images.into_iter().next().or_else(|| {
        any_image.into_iter().find(|n| {
            let base = Path::new(n).file_name().and_then(|s| s.to_str()).unwrap_or("");
            base.eq_ignore_ascii_case("default.jpg") || base.eq_ignore_ascii_case("preview.jpg")
        })
    })
}

fn thumbnail_candidate_names(stem: &str) -> Vec<String> {
    vec![
        format!("{stem}.jpg"),
        format!("{stem}.jpeg"),
        format!("{stem}.png"),
        "default.jpg".to_string(),
        "default.png".to_string(),
        "preview.jpg".to_string(),
        "preview.png".to_string(),
        format!("{stem}.webp"),
    ]
}

pub fn find_thumbnail_in_folder(folder: &Path, stem: &str) -> Option<PathBuf> {
    if !folder.is_dir() {
        return None;
    }

    for candidate in thumbnail_candidate_names(stem) {
        let path = folder.join(&candidate);
        if path.is_file() {
            return Some(path);
        }
    }

    let ui_dir = folder.join("ui");
    if ui_dir.is_dir() {
        if let Ok(read_dir) = std::fs::read_dir(&ui_dir) {
            let mut images: Vec<PathBuf> = read_dir
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && is_image_path(p))
                .collect();
            images.sort();
            if let Some(first) = images.into_iter().next() {
                return Some(first);
            }
        }
    }

    if let Ok(read_dir) = std::fs::read_dir(folder) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_file() && is_image_path(&path) {
                return Some(path);
            }
        }
    }

    None
}

fn is_image_path(path: &Path) -> bool {
    is_image_name(
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(""),
    )
}

fn is_image_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".png")
        || lower.ends_with(".webp")
}
