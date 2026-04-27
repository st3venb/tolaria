// Vault boundary enforcement: path validation
//
// Pure path-validation logic shared by both the Tauri GUI and the CLI.
// Tauri-specific concerns (state extraction, `from_request`, `expand_tilde`)
// remain in `src-tauri/src/commands/vault/boundary.rs`.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

pub const ACTIVE_VAULT_PATH_ERROR: &str = "Path must stay inside the active vault";
pub const INVALID_VIEW_FILENAME_ERROR: &str = "Invalid view filename";

/// Verify that `target` is a canonical descendant of `vault_root`.
///
/// Both paths should already be canonicalized before calling this function.
/// Returns `Ok(())` when `target` is inside the vault, or an error string
/// when it escapes the boundary.
pub fn validate_path_within_vault(vault_root: &Path, target: &Path) -> Result<(), String> {
    target
        .strip_prefix(vault_root)
        .map(|_| ())
        .map_err(|_| ACTIVE_VAULT_PATH_ERROR.to_string())
}

/// Validate that `relative_path` is a safe, downward-only relative path.
///
/// Rejects empty strings, absolute paths, and any component that is `.`,
/// `..`, a root dir, or a Windows prefix.
pub fn validate_relative_child_path(relative_path: &str) -> Result<(), String> {
    if relative_path.trim().is_empty() {
        return Err(ACTIVE_VAULT_PATH_ERROR.to_string());
    }

    let path = Path::new(relative_path);
    if path.is_absolute() {
        return Err(ACTIVE_VAULT_PATH_ERROR.to_string());
    }

    if path.components().any(|component| {
        matches!(
            component,
            Component::CurDir | Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(ACTIVE_VAULT_PATH_ERROR.to_string());
    }

    Ok(())
}

/// Canonicalize a path that may not fully exist yet (for write operations).
///
/// Walks up from the leaf until an existing ancestor is found, canonicalizes
/// that ancestor, then re-appends the missing tail segments.
pub fn canonicalize_candidate_for_write(path: &Path) -> Result<PathBuf, String> {
    let (ancestor, tail) = find_existing_ancestor(path)?;
    Ok(tail
        .into_iter()
        .fold(ancestor, |current, segment| current.join(segment)))
}

/// Validate that `filename` is a single `.yml` file name with no path separators.
pub fn validate_view_filename(filename: &str) -> Result<(), String> {
    if !filename.ends_with(".yml") {
        return Err("Filename must end with .yml".to_string());
    }

    let path = Path::new(filename);
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(()),
        _ => Err(INVALID_VIEW_FILENAME_ERROR.to_string()),
    }
}

fn find_existing_ancestor(path: &Path) -> Result<(PathBuf, Vec<OsString>), String> {
    let mut current = path;
    let mut tail = Vec::new();

    loop {
        if current.exists() {
            let canonical = current
                .canonicalize()
                .map_err(|_| ACTIVE_VAULT_PATH_ERROR.to_string())?;
            tail.reverse();
            return Ok((canonical, tail));
        }

        let file_name = current
            .file_name()
            .ok_or_else(|| ACTIVE_VAULT_PATH_ERROR.to_string())?;
        tail.push(file_name.to_os_string());
        current = current
            .parent()
            .ok_or_else(|| ACTIVE_VAULT_PATH_ERROR.to_string())?;
    }
}
