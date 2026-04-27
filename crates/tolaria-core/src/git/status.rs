use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Serialize, Clone)]
pub struct ModifiedFile {
    pub path: String,
    #[serde(rename = "relativePath")]
    pub relative_path: String,
    pub status: String,
    #[serde(rename = "addedLines")]
    pub added_lines: Option<usize>,
    #[serde(rename = "deletedLines")]
    pub deleted_lines: Option<usize>,
    pub binary: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct DiffStats {
    added_lines: Option<usize>,
    deleted_lines: Option<usize>,
    binary: bool,
}

fn status_label(status_code: &str) -> &'static str {
    match status_code.trim() {
        "M" | "MM" | "AM" => "modified",
        "A" => "added",
        "D" => "deleted",
        "??" => "untracked",
        "R" | "RM" => "renamed",
        _ => "modified",
    }
}

fn parse_status_path(raw_path: &str) -> String {
    raw_path
        .split(" -> ")
        .last()
        .unwrap_or(raw_path)
        .trim()
        .to_string()
}

fn parse_numstat_field(field: &str) -> Option<usize> {
    field.parse().ok()
}

fn parse_numstat_line(line: &str) -> Option<(String, DiffStats)> {
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() < 3 {
        return None;
    }

    let added_lines = parse_numstat_field(parts[0]);
    let deleted_lines = parse_numstat_field(parts[1]);
    let binary = parts[0] == "-" || parts[1] == "-";

    Some((
        parts.last()?.trim().to_string(),
        DiffStats { added_lines, deleted_lines, binary },
    ))
}

fn repo_has_head(vault: &Path) -> Result<bool, String> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(vault)
        .output()
        .map_err(|e| format!("Failed to run git rev-parse: {e}"))?;
    Ok(output.status.success())
}

fn load_diff_stats(vault: &Path) -> Result<HashMap<String, DiffStats>, String> {
    if !repo_has_head(vault)? {
        return Ok(HashMap::new());
    }
    let output = Command::new("git")
        .args(["diff", "--numstat", "--find-renames", "HEAD", "--"])
        .current_dir(vault)
        .output()
        .map_err(|e| format!("Failed to run git diff --numstat: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git diff --numstat failed: {}", stderr.trim()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().filter_map(parse_numstat_line).collect::<HashMap<_, _>>())
}

fn count_worktree_lines(vault: &Path, relative_path: &str) -> DiffStats {
    let full_path = vault.join(relative_path);
    let added_lines = std::fs::read_to_string(full_path)
        .ok()
        .map(|content| content.lines().count());
    DiffStats { added_lines, deleted_lines: None, binary: false }
}

fn resolve_diff_stats(
    vault: &Path,
    relative_path: &str,
    status: &str,
    diff_stats: &HashMap<String, DiffStats>,
) -> DiffStats {
    if status == "untracked" {
        return count_worktree_lines(vault, relative_path);
    }
    diff_stats.get(relative_path).copied().unwrap_or_default()
}

fn ensure_path_within_vault(vault: &Path, relative_path: &str, abs: &Path) -> Result<(), String> {
    for component in Path::new(relative_path).components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err("File path is outside the vault".into());
        }
    }
    if !abs.exists() {
        return Ok(());
    }
    let canonical_vault = vault.canonicalize().map_err(|e| format!("Cannot resolve vault path: {e}"))?;
    let canonical_file = abs.canonicalize().map_err(|e| format!("Cannot resolve file path: {e}"))?;
    if canonical_file.starts_with(&canonical_vault) { Ok(()) } else { Err("File path is outside the vault".into()) }
}

fn load_file_status(vault: &Path, relative_path: &str) -> Result<String, String> {
    let output = Command::new("git")
        .args(["status", "--porcelain", "--", relative_path])
        .current_dir(vault)
        .output()
        .map_err(|e| format!("Failed to run git status: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().find(|line| line.len() >= 4).map(|line| line[..2].trim().to_string()).unwrap_or_default())
}

fn restore_tracked_file(vault: &Path, relative_path: &str) -> Result<(), String> {
    let _ = Command::new("git").args(["reset", "HEAD", "--", relative_path]).current_dir(vault).output();
    let checkout = Command::new("git")
        .args(["checkout", "--", relative_path])
        .current_dir(vault)
        .output()
        .map_err(|e| format!("Failed to run git checkout: {e}"))?;
    if checkout.status.success() { return Ok(()); }
    let stderr = String::from_utf8_lossy(&checkout.stderr);
    Err(format!("git checkout failed: {}", stderr.trim()))
}

/// Get list of modified/added/deleted files in the vault (uncommitted changes).
pub fn get_modified_files(vault_path: &str) -> Result<Vec<ModifiedFile>, String> {
    let vault = Path::new(vault_path);
    let output = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=all"])
        .current_dir(vault)
        .output()
        .map_err(|e| format!("Failed to run git status: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git status failed: {}", stderr.trim()));
    }
    let diff_stats = load_diff_stats(vault)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let files = stdout
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            if line.len() < 4 { return None; }
            let status_code = &line[..2];
            let relative_path = parse_status_path(&line[3..]);
            if !relative_path.ends_with(".md") { return None; }
            let status = status_label(status_code);
            let full_path = vault.join(&relative_path).to_string_lossy().to_string();
            let stats = resolve_diff_stats(vault, &relative_path, status, &diff_stats);
            Some(ModifiedFile {
                path: full_path,
                relative_path,
                status: status.to_string(),
                added_lines: stats.added_lines,
                deleted_lines: stats.deleted_lines,
                binary: stats.binary,
            })
        })
        .collect();
    Ok(files)
}

/// Discard uncommitted changes to a single file.
pub fn discard_file_changes(vault_path: &str, relative_path: &str) -> Result<(), String> {
    let vault = Path::new(vault_path);
    let abs = vault.join(relative_path);
    ensure_path_within_vault(vault, relative_path, &abs)?;
    let status_code = load_file_status(vault, relative_path)?;
    match status_code.as_str() {
        "??" => {
            std::fs::remove_file(&abs).map_err(|e| format!("Failed to delete untracked file: {e}"))?;
        }
        _ => {
            restore_tracked_file(vault, relative_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::git_commit;
    use crate::git::tests::setup_git_repo;
    use std::fs;
    use std::process::Command;

    fn write_and_commit_markdown(vault: &Path, vp: &str, relative_path: &str, content: &str) {
        fs::write(vault.join(relative_path), content).unwrap();
        git_commit(vp, "initial").unwrap();
    }

    #[test]
    fn test_get_modified_files() {
        let dir = setup_git_repo();
        let vault = dir.path();
        fs::write(vault.join("note.md"), "# Note\n").unwrap();
        Command::new("git").args(["add", "note.md"]).current_dir(vault).output().unwrap();
        Command::new("git").args(["commit", "-m", "Add note"]).current_dir(vault).output().unwrap();
        fs::write(vault.join("note.md"), "# Note\n\nUpdated.").unwrap();
        fs::write(vault.join("new.md"), "# New\n").unwrap();
        let modified = get_modified_files(vault.to_str().unwrap()).unwrap();
        assert!(modified.len() >= 2);
        let statuses: Vec<&str> = modified.iter().map(|f| f.status.as_str()).collect();
        assert!(statuses.contains(&"modified"));
        assert!(statuses.contains(&"untracked"));
    }

    #[test]
    fn test_get_modified_files_untracked_in_subdirectory() {
        let dir = setup_git_repo();
        let vault = dir.path();
        fs::write(vault.join("init.md"), "# Init\n").unwrap();
        Command::new("git").args(["add", "init.md"]).current_dir(vault).output().unwrap();
        Command::new("git").args(["commit", "-m", "Initial"]).current_dir(vault).output().unwrap();
        fs::create_dir_all(vault.join("note")).unwrap();
        fs::write(vault.join("note/brand-new.md"), "# Brand New\n").unwrap();
        let modified = get_modified_files(vault.to_str().unwrap()).unwrap();
        assert_eq!(modified.len(), 1);
        assert_eq!(modified[0].status, "untracked");
        assert_eq!(modified[0].relative_path, "note/brand-new.md");
    }

    #[test]
    fn test_commit_flow_modified_files_then_commit_clears() {
        let dir = setup_git_repo();
        let vault = dir.path();
        let vp = vault.to_str().unwrap();
        fs::write(vault.join("flow.md"), "# Original\n").unwrap();
        git_commit(vp, "initial").unwrap();
        fs::write(vault.join("flow.md"), "# Modified\n").unwrap();
        let modified = get_modified_files(vp).unwrap();
        assert!(modified.iter().any(|f| f.relative_path == "flow.md"));
        git_commit(vp, "update flow").unwrap();
        let after = get_modified_files(vp).unwrap();
        assert!(after.is_empty());
    }

    #[test]
    fn test_discard_modified_file() {
        let dir = setup_git_repo();
        let vault = dir.path();
        let vp = vault.to_str().unwrap();
        write_and_commit_markdown(vault, vp, "note.md", "# Original\n");
        fs::write(vault.join("note.md"), "# Changed\n").unwrap();
        discard_file_changes(vp, "note.md").unwrap();
        let content = fs::read_to_string(vault.join("note.md")).unwrap();
        assert_eq!(content, "# Original\n");
    }

    #[test]
    fn test_discard_untracked_file() {
        let dir = setup_git_repo();
        let vault = dir.path();
        let vp = vault.to_str().unwrap();
        write_and_commit_markdown(vault, vp, "init.md", "# Init\n");
        fs::write(vault.join("new.md"), "# New\n").unwrap();
        discard_file_changes(vp, "new.md").unwrap();
        assert!(!vault.join("new.md").exists());
    }

    #[test]
    fn test_discard_deleted_file() {
        let dir = setup_git_repo();
        let vault = dir.path();
        let vp = vault.to_str().unwrap();
        write_and_commit_markdown(vault, vp, "note.md", "# Original\n");
        fs::remove_file(vault.join("note.md")).unwrap();
        discard_file_changes(vp, "note.md").unwrap();
        assert!(vault.join("note.md").exists());
        let content = fs::read_to_string(vault.join("note.md")).unwrap();
        assert_eq!(content, "# Original\n");
    }

    #[test]
    fn test_discard_rejects_path_outside_vault() {
        let dir = setup_git_repo();
        let vault = dir.path();
        let vp = vault.to_str().unwrap();
        write_and_commit_markdown(vault, vp, "init.md", "# Init\n");
        let result = discard_file_changes(vp, "../../../etc/passwd");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("outside the vault"));
    }
}
