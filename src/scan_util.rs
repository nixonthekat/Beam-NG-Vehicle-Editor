use std::path::{Path, PathBuf};

/// Subfolders under `mods/` that may contain zip archives.
pub fn mod_zip_dirs(mods_root: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if mods_root.is_dir() {
        dirs.push(mods_root.to_path_buf());
    }
    for name in ["packed", "unpacked"] {
        let sub = mods_root.join(name);
        if sub.is_dir() {
            dirs.push(sub);
        }
    }
    dirs
}
