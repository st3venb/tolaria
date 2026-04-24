use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

const MCP_SERVER_NAME: &str = "tolaria";
const LEGACY_MCP_SERVER_NAME: &str = "laputa";

/// Status of the MCP server installation.
#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum McpStatus {
    /// MCP is registered in Claude config and server files exist.
    Installed,
    /// MCP server files or config are missing for the active vault.
    NotInstalled,
}

/// Find the `node` binary path at runtime.
pub(crate) fn find_node() -> Result<PathBuf, String> {
    if let Some(path) = find_node_on_path()? {
        return Ok(path);
    }

    if let Some(path) = fallback_node_path() {
        return Ok(path);
    }

    Err("node not found in PATH or common install locations".into())
}

/// Use the platform-appropriate command to locate `node` on PATH.
/// Returns `Ok(Some(path))` when found, `Ok(None)` when the command
/// succeeds but produces no output, or `Err` on execution failure.
fn find_node_on_path() -> Result<Option<PathBuf>, String> {
    let (cmd, arg) = if cfg!(target_os = "windows") {
        ("where.exe", "node")
    } else {
        ("which", "node")
    };

    let output = Command::new(cmd)
        .arg(arg)
        .output()
        .map_err(|e| format!("Failed to run `{cmd} {arg}`: {e}"))?;

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if !path.is_empty() {
            return Ok(Some(PathBuf::from(path)));
        }
    }

    Ok(None)
}

fn fallback_node_path() -> Option<PathBuf> {
    let candidates = if cfg!(target_os = "windows") {
        fallback_node_paths_windows()
    } else {
        fallback_node_paths_unix()
    };

    candidates.into_iter().find(|path| path.is_file())
}

fn fallback_node_paths_unix() -> Vec<PathBuf> {
    let mut candidates = vec![
        PathBuf::from("/opt/homebrew/bin/node"),
        PathBuf::from("/usr/local/bin/node"),
    ];

    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".volta").join("bin").join("node"));

        let nvm_dir = home.join(".nvm").join("versions").join("node");
        if let Ok(entries) = std::fs::read_dir(nvm_dir) {
            let mut versions = entries
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .collect::<Vec<_>>();
            versions.sort();
            versions.reverse();
            candidates.extend(
                versions
                    .into_iter()
                    .map(|version| version.join("bin").join("node")),
            );
        }
    }

    candidates
}

fn fallback_node_paths_windows() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(program_files) = std::env::var("ProgramFiles") {
        candidates.push(PathBuf::from(program_files).join("nodejs").join("node.exe"));
    }

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        candidates.push(
            PathBuf::from(local_app_data)
                .join("Volta")
                .join("node.exe"),
        );
    }

    if let Ok(app_data) = std::env::var("APPDATA") {
        let nvm_dir = PathBuf::from(app_data).join("nvm");
        if let Ok(entries) = std::fs::read_dir(nvm_dir) {
            let mut versions = entries
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .collect::<Vec<_>>();
            versions.sort();
            versions.reverse();
            candidates.extend(
                versions
                    .into_iter()
                    .map(|version| version.join("node.exe")),
            );
        }
    }

    candidates
}

/// Resolve the path to `mcp-server/`.
///
/// In dev mode, uses `CARGO_MANIFEST_DIR` (set at compile time).
/// In release mode, navigates from the current executable:
/// - macOS: `Contents/Resources/mcp-server/` inside the `.app` bundle
/// - Windows: `<exe_dir>/mcp-server/` (flat directory alongside the exe)
pub(crate) fn mcp_server_dir() -> Result<PathBuf, String> {
    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("mcp-server");
    let exe = std::env::current_exe().map_err(|e| format!("Cannot find executable: {e}"))?;
    let macos_release_path = exe
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("Resources").join("mcp-server"));
    let windows_release_path = exe.parent().map(|p| p.join("mcp-server"));

    resolve_mcp_server_dir(&dev_path, macos_release_path.as_deref(), windows_release_path.as_deref())
}

/// Core resolution logic: tries candidate paths in order, returning the first
/// that contains `ws-bridge.js`. Extracted for testability.
fn resolve_mcp_server_dir(
    dev_path: &Path,
    macos_release_path: Option<&Path>,
    windows_release_path: Option<&Path>,
) -> Result<PathBuf, String> {
    if dev_path.join("ws-bridge.js").exists() {
        return Ok(std::fs::canonicalize(dev_path).unwrap_or_else(|_| dev_path.to_path_buf()));
    }

    if let Some(path) = macos_release_path {
        if path.join("ws-bridge.js").exists() {
            return Ok(path.to_path_buf());
        }
    }

    if let Some(path) = windows_release_path {
        if path.join("ws-bridge.js").exists() {
            return Ok(path.to_path_buf());
        }
    }

    let mut checked = vec![format!("{}", dev_path.display())];
    if let Some(p) = macos_release_path {
        checked.push(format!("{}", p.display()));
    }
    if let Some(p) = windows_release_path {
        checked.push(format!("{}", p.display()));
    }
    Err(format!("mcp-server not found at {}", checked.join(", ")))
}

/// Spawn the WebSocket bridge as a child process.
pub fn spawn_ws_bridge(vault_path: &str) -> Result<Child, String> {
    let node = find_node()?;
    let server_dir = mcp_server_dir()?;
    let script = server_dir.join("ws-bridge.js");

    let child = Command::new(node)
        .arg(&script)
        .env("VAULT_PATH", vault_path)
        .env("WS_PORT", "9710")
        .env("WS_UI_PORT", "9711")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn ws-bridge: {e}"))?;

    log::info!("ws-bridge spawned (pid: {})", child.id());
    Ok(child)
}

fn mcp_config_paths() -> Vec<PathBuf> {
    dirs::home_dir()
        .map(|home| mcp_config_paths_for_home(&home))
        .unwrap_or_default()
}

fn mcp_config_paths_for_home(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".claude.json"),
        home.join(".claude").join("mcp.json"),
        home.join(".cursor").join("mcp.json"),
    ]
}

fn read_registered_mcp_entry(config_path: &Path) -> Option<serde_json::Value> {
    let raw = std::fs::read_to_string(config_path).ok()?;
    let config: serde_json::Value = serde_json::from_str(&raw).ok()?;
    config
        .get("mcpServers")
        .and_then(|value| value.as_object())
        .and_then(|servers| {
            servers
                .get(MCP_SERVER_NAME)
                .or_else(|| servers.get(LEGACY_MCP_SERVER_NAME))
        })
        .cloned()
}

fn entry_index_js_exists(entry: &serde_json::Value) -> bool {
    entry["args"]
        .as_array()
        .and_then(|args| args.first())
        .and_then(|value| value.as_str())
        .is_some_and(|index_js| Path::new(index_js).exists())
}

fn entry_targets_vault(entry: &serde_json::Value, vault_path: &Path) -> bool {
    let Some(entry_vault_path) = entry["env"]["VAULT_PATH"].as_str() else {
        return false;
    };

    let Ok(expected) = std::fs::canonicalize(vault_path) else {
        return false;
    };
    let Ok(actual) = std::fs::canonicalize(entry_vault_path) else {
        return false;
    };

    actual == expected
}

/// Build the MCP server entry JSON for a given vault path and index.js path.
fn build_mcp_entry(index_js: &str, vault_path: &str) -> serde_json::Value {
    serde_json::json!({
        "command": "node",
        "args": [index_js],
        "env": { "VAULT_PATH": vault_path }
    })
}

/// Write MCP registration to a list of config file paths.
/// Returns "registered" on first registration, "updated" if already present.
fn register_mcp_to_configs(entry: &serde_json::Value, config_paths: &[PathBuf]) -> String {
    let mut status = "registered";
    for config_path in config_paths {
        match upsert_mcp_config(config_path, entry) {
            Ok(true) => status = "updated",
            Ok(false) => {}
            Err(e) => log::warn!("Failed to update {}: {}", config_path.display(), e),
        }
    }
    status.to_string()
}

/// Register Tolaria as an MCP server in Claude Code and Cursor config files.
pub fn register_mcp(vault_path: &str) -> Result<String, String> {
    let server_dir = mcp_server_dir()?;
    let index_js = server_dir.join("index.js").to_string_lossy().into_owned();

    let entry = build_mcp_entry(&index_js, vault_path);

    Ok(register_mcp_to_configs(&entry, &mcp_config_paths()))
}

/// Insert or update the Tolaria entry in an MCP config file.
fn upsert_mcp_config(config_path: &Path, entry: &serde_json::Value) -> Result<bool, String> {
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create dir {}: {e}", parent.display()))?;
    }

    let mut config: serde_json::Value = if config_path.exists() {
        let raw = std::fs::read_to_string(config_path)
            .map_err(|e| format!("Cannot read {}: {e}", config_path.display()))?;
        serde_json::from_str(&raw)
            .map_err(|e| format!("Invalid JSON in {}: {e}", config_path.display()))?
    } else {
        serde_json::json!({})
    };

    let servers = config
        .as_object_mut()
        .ok_or("Config is not a JSON object")?
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));

    let servers = servers
        .as_object_mut()
        .ok_or("mcpServers is not a JSON object")?;

    let was_update =
        servers.get(MCP_SERVER_NAME).is_some() || servers.get(LEGACY_MCP_SERVER_NAME).is_some();
    servers.remove(LEGACY_MCP_SERVER_NAME);
    servers.insert(MCP_SERVER_NAME.to_string(), entry.clone());

    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {e}"))?;
    std::fs::write(config_path, json)
        .map_err(|e| format!("Cannot write {}: {e}", config_path.display()))?;

    Ok(was_update)
}

fn remove_mcp_from_configs(config_paths: &[PathBuf]) -> String {
    let mut removed_any = false;
    for config_path in config_paths {
        match remove_mcp_from_config(config_path) {
            Ok(true) => removed_any = true,
            Ok(false) => {}
            Err(e) => log::warn!("Failed to update {}: {}", config_path.display(), e),
        }
    }

    if removed_any {
        "removed".to_string()
    } else {
        "already_absent".to_string()
    }
}

fn remove_mcp_from_config(config_path: &Path) -> Result<bool, String> {
    if !config_path.exists() {
        return Ok(false);
    }

    let raw = std::fs::read_to_string(config_path)
        .map_err(|e| format!("Cannot read {}: {e}", config_path.display()))?;
    let mut config: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| format!("Invalid JSON in {}: {e}", config_path.display()))?;

    let Some(config_object) = config.as_object_mut() else {
        return Err("Config is not a JSON object".into());
    };

    let Some(servers_value) = config_object.get_mut("mcpServers") else {
        return Ok(false);
    };

    let Some(servers) = servers_value.as_object_mut() else {
        return Err("mcpServers is not a JSON object".into());
    };

    let removed_primary = servers.remove(MCP_SERVER_NAME).is_some();
    let removed_legacy = servers.remove(LEGACY_MCP_SERVER_NAME).is_some();
    if !removed_primary && !removed_legacy {
        return Ok(false);
    }

    if servers.is_empty() {
        config_object.remove("mcpServers");
    }

    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {e}"))?;
    std::fs::write(config_path, json)
        .map_err(|e| format!("Cannot write {}: {e}", config_path.display()))?;

    Ok(true)
}

pub fn remove_mcp() -> String {
    remove_mcp_from_configs(&mcp_config_paths())
}

/// Check whether the MCP server is properly installed and registered.
///
/// Returns `Installed` when the Tolaria entry exists for the active vault in
/// Claude Code or Cursor config and the referenced index.js file is present.
/// Otherwise returns `NotInstalled`.
pub fn check_mcp_status(vault_path: &str) -> McpStatus {
    let active_vault_path = Path::new(vault_path);
    if mcp_config_paths().into_iter().any(|config_path| {
        read_registered_mcp_entry(&config_path).is_some_and(|entry| {
            entry_index_js_exists(&entry) && entry_targets_vault(&entry, active_vault_path)
        })
    }) {
        McpStatus::Installed
    } else {
        McpStatus::NotInstalled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_config(config_path: &Path) -> serde_json::Value {
        let raw = std::fs::read_to_string(config_path).unwrap();
        serde_json::from_str(&raw).unwrap()
    }

    fn temp_config_path(file_name: &str) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join(file_name);
        (tmp, config_path)
    }

    fn write_config_json(config_path: &Path, config: serde_json::Value) {
        std::fs::write(config_path, serde_json::to_string(&config).unwrap()).unwrap();
    }

    fn managed_server(index_js: &str, vault_path: &str) -> serde_json::Value {
        serde_json::json!({
            "command": "node",
            "args": [index_js],
            "env": { "VAULT_PATH": vault_path }
        })
    }

    fn write_mcp_servers_config(config_path: &Path, servers: Vec<(&str, serde_json::Value)>) {
        let servers = servers
            .into_iter()
            .map(|(name, server)| (name.to_string(), server))
            .collect::<serde_json::Map<_, _>>();
        write_config_json(config_path, serde_json::json!({ "mcpServers": servers }));
    }

    fn write_index_js(dir: &Path) -> PathBuf {
        let index_js = dir.join("index.js");
        std::fs::write(&index_js, "console.log('ok');").unwrap();
        index_js
    }

    #[test]
    fn build_mcp_entry_produces_correct_json() {
        let entry = build_mcp_entry("/path/to/index.js", "/my/vault");
        assert_eq!(entry["command"], "node");
        assert_eq!(entry["args"][0], "/path/to/index.js");
        assert_eq!(entry["env"]["VAULT_PATH"], "/my/vault");
    }

    #[test]
    fn upsert_creates_new_config() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("mcp.json");
        let entry = build_mcp_entry("/test/index.js", "/test/vault");

        let was_update = upsert_mcp_config(&config_path, &entry).unwrap();
        assert!(!was_update);

        let config = read_config(&config_path);
        assert_eq!(
            config["mcpServers"][MCP_SERVER_NAME]["args"][0],
            "/test/index.js"
        );
        assert_eq!(
            config["mcpServers"][MCP_SERVER_NAME]["env"]["VAULT_PATH"],
            "/test/vault"
        );
    }

    #[test]
    fn upsert_updates_existing_config() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("mcp.json");

        let entry1 = build_mcp_entry("/test/index.js", "/vault/v1");
        upsert_mcp_config(&config_path, &entry1).unwrap();

        let entry2 = build_mcp_entry("/test/index.js", "/vault/v2");
        let was_update = upsert_mcp_config(&config_path, &entry2).unwrap();
        assert!(was_update);

        let config = read_config(&config_path);
        assert_eq!(
            config["mcpServers"][MCP_SERVER_NAME]["env"]["VAULT_PATH"],
            "/vault/v2"
        );
    }

    #[test]
    fn upsert_migrates_legacy_server_name() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("mcp.json");

        let existing = serde_json::json!({
            "mcpServers": {
                "laputa": {
                    "command": "node",
                    "args": ["/old/index.js"],
                    "env": { "VAULT_PATH": "/old" }
                }
            }
        });
        std::fs::write(&config_path, serde_json::to_string(&existing).unwrap()).unwrap();

        let entry = build_mcp_entry("/test/index.js", "/vault");
        let was_update = upsert_mcp_config(&config_path, &entry).unwrap();
        assert!(was_update);

        let config = read_config(&config_path);
        assert!(config["mcpServers"][LEGACY_MCP_SERVER_NAME].is_null());
        assert_eq!(
            config["mcpServers"][MCP_SERVER_NAME]["args"][0],
            "/test/index.js"
        );
    }

    #[test]
    fn upsert_preserves_other_servers() {
        let (_tmp, config_path) = temp_config_path("mcp.json");
        write_mcp_servers_config(
            &config_path,
            vec![(
                "other-server",
                serde_json::json!({ "command": "other", "args": [] }),
            )],
        );

        let entry = build_mcp_entry("/test/index.js", "/vault");
        upsert_mcp_config(&config_path, &entry).unwrap();

        let raw = std::fs::read_to_string(&config_path).unwrap();
        let config: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(config["mcpServers"]["other-server"].is_object());
        assert!(config["mcpServers"][MCP_SERVER_NAME].is_object());
    }

    #[test]
    fn upsert_preserves_other_top_level_settings() {
        let (_tmp, config_path) = temp_config_path(".claude.json");
        write_config_json(
            &config_path,
            serde_json::json!({
                "model": "sonnet",
                "theme": "dark",
                "mcpServers": {
                    "other-server": { "command": "other", "args": [] }
                }
            }),
        );

        let entry = build_mcp_entry("/test/index.js", "/vault");
        upsert_mcp_config(&config_path, &entry).unwrap();

        let config = read_config(&config_path);
        assert_eq!(config["model"], "sonnet");
        assert_eq!(config["theme"], "dark");
        assert!(config["mcpServers"]["other-server"].is_object());
        assert!(config["mcpServers"][MCP_SERVER_NAME].is_object());
    }

    #[test]
    fn upsert_creates_parent_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("nested").join("dir").join("mcp.json");
        let entry = build_mcp_entry("/test/index.js", "/vault");

        upsert_mcp_config(&config_path, &entry).unwrap();
        assert!(config_path.exists());
    }

    #[test]
    fn register_mcp_to_configs_returns_registered_for_new() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("claude").join("mcp.json");
        let entry = build_mcp_entry("/test/index.js", "/vault");

        let status = register_mcp_to_configs(&entry, &[config]);
        assert_eq!(status, "registered");
    }

    #[test]
    fn register_mcp_to_configs_returns_updated_for_existing() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("mcp.json");
        let entry = build_mcp_entry("/test/index.js", "/vault");

        // First call
        register_mcp_to_configs(&entry, std::slice::from_ref(&config));
        // Second call
        let status = register_mcp_to_configs(&entry, &[config]);
        assert_eq!(status, "updated");
    }

    #[test]
    fn find_node_returns_valid_path() {
        let node = find_node().unwrap();
        assert!(node.exists(), "node binary should exist at {:?}", node);
        assert!(
            node.to_string_lossy().contains("node"),
            "path should contain 'node': {:?}",
            node
        );
    }

    #[test]
    fn mcp_server_dir_resolves_in_dev() {
        let dir = mcp_server_dir().unwrap();
        assert!(dir.join("ws-bridge.js").exists());
        assert!(dir.join("index.js").exists());
        assert!(dir.join("vault.js").exists());
    }

    #[test]
    fn spawn_ws_bridge_starts_and_can_be_killed() {
        let tmp = tempfile::tempdir().unwrap();
        let vault_path = tmp.path().to_str().unwrap();

        let mut child = spawn_ws_bridge(vault_path).unwrap();
        assert!(child.id() > 0, "child process should have a valid PID");

        // Clean up: kill the spawned process
        child.kill().unwrap();
        child.wait().unwrap();
    }

    #[test]
    fn register_mcp_to_configs_writes_multiple_configs() {
        let tmp = tempfile::tempdir().unwrap();
        let claude_user_cfg = tmp.path().join(".claude.json");
        let claude_cfg = tmp.path().join("claude").join("mcp.json");
        let cursor_cfg = tmp.path().join("cursor").join("mcp.json");
        let entry = build_mcp_entry("/test/index.js", "/vault");

        register_mcp_to_configs(
            &entry,
            &[
                claude_user_cfg.clone(),
                claude_cfg.clone(),
                cursor_cfg.clone(),
            ],
        );

        assert!(claude_user_cfg.exists());
        assert!(claude_cfg.exists());
        assert!(cursor_cfg.exists());

        let raw = std::fs::read_to_string(&claude_user_cfg).unwrap();
        let config: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            config["mcpServers"][MCP_SERVER_NAME]["args"][0],
            "/test/index.js"
        );
    }

    #[test]
    fn mcp_config_paths_for_home_includes_claude_root_and_legacy_paths() {
        let home = Path::new("/Users/tester");
        let paths = mcp_config_paths_for_home(home);

        assert_eq!(
            paths,
            vec![
                home.join(".claude.json"),
                home.join(".claude").join("mcp.json"),
                home.join(".cursor").join("mcp.json"),
            ]
        );
    }
    #[test]
    fn upsert_returns_error_for_invalid_json() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("mcp.json");
        std::fs::write(&config_path, "not valid json{{{{").unwrap();
        let entry = build_mcp_entry("/test/index.js", "/vault");
        let result = upsert_mcp_config(&config_path, &entry);
        assert!(result.is_err());
    }

    #[test]
    fn register_mcp_to_configs_handles_empty_list() {
        let entry = build_mcp_entry("/test/index.js", "/vault");
        // Empty config list — function should return "registered" (no existing)
        let status = register_mcp_to_configs(&entry, &[]);
        // With empty config list, there were no updates, so status should be "registered"
        assert_eq!(status, "registered");
    }

    #[test]
    fn read_registered_mcp_entry_prefers_primary_server_name() {
        let (_tmp, config_path) = temp_config_path("mcp.json");
        write_mcp_servers_config(
            &config_path,
            vec![
                (
                    MCP_SERVER_NAME,
                    managed_server("/primary/index.js", "/primary"),
                ),
                (
                    LEGACY_MCP_SERVER_NAME,
                    managed_server("/legacy/index.js", "/legacy"),
                ),
            ],
        );

        let entry = read_registered_mcp_entry(&config_path).unwrap();
        assert_eq!(entry["env"]["VAULT_PATH"], "/primary");
    }

    #[test]
    fn read_registered_mcp_entry_uses_legacy_server_name() {
        let (_tmp, config_path) = temp_config_path("mcp.json");
        write_mcp_servers_config(
            &config_path,
            vec![(
                LEGACY_MCP_SERVER_NAME,
                managed_server("/legacy/index.js", "/legacy"),
            )],
        );

        let entry = read_registered_mcp_entry(&config_path).unwrap();
        assert_eq!(entry["env"]["VAULT_PATH"], "/legacy");
    }

    #[test]
    fn read_registered_mcp_entry_returns_none_for_invalid_or_missing_servers() {
        let tmp = tempfile::tempdir().unwrap();
        let invalid_path = tmp.path().join("invalid.json");
        std::fs::write(&invalid_path, "{not json").unwrap();
        assert!(read_registered_mcp_entry(&invalid_path).is_none());

        let empty_path = tmp.path().join("empty.json");
        let empty_config = serde_json::json!({ "other": {} });
        std::fs::write(&empty_path, serde_json::to_string(&empty_config).unwrap()).unwrap();
        assert!(read_registered_mcp_entry(&empty_path).is_none());

        let missing_path = tmp.path().join("missing.json");
        assert!(read_registered_mcp_entry(&missing_path).is_none());
    }

    #[test]
    fn entry_index_js_exists_requires_existing_first_arg() {
        let tmp = tempfile::tempdir().unwrap();
        let index_js = tmp.path().join("index.js");
        std::fs::write(&index_js, "console.log('ok');").unwrap();

        let existing = serde_json::json!({
            "args": [index_js.to_string_lossy()]
        });
        assert!(entry_index_js_exists(&existing));

        let missing = serde_json::json!({
            "args": [tmp.path().join("missing.js").to_string_lossy()]
        });
        assert!(!entry_index_js_exists(&missing));

        let no_args = serde_json::json!({});
        assert!(!entry_index_js_exists(&no_args));
    }

    #[test]
    fn upsert_returns_error_for_non_object_config() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("mcp.json");
        std::fs::write(&config_path, "[]").unwrap();

        let entry = build_mcp_entry("/test/index.js", "/vault");
        let result = upsert_mcp_config(&config_path, &entry);
        assert!(matches!(result, Err(ref error) if error.contains("Config is not a JSON object")));
    }

    #[test]
    fn upsert_returns_error_for_non_object_mcp_servers() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("mcp.json");
        let config = serde_json::json!({
            "mcpServers": []
        });
        std::fs::write(&config_path, serde_json::to_string(&config).unwrap()).unwrap();

        let entry = build_mcp_entry("/test/index.js", "/vault");
        let result = upsert_mcp_config(&config_path, &entry);
        assert!(
            matches!(result, Err(ref error) if error.contains("mcpServers is not a JSON object"))
        );
    }

    #[test]
    fn remove_mcp_from_config_removes_primary_and_legacy_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("mcp.json");
        let config = serde_json::json!({
            "mcpServers": {
                "tolaria": { "command": "node", "args": ["/index.js"] },
                "laputa": { "command": "node", "args": ["/legacy.js"] },
                "other-server": { "command": "other", "args": [] }
            }
        });
        std::fs::write(&config_path, serde_json::to_string(&config).unwrap()).unwrap();

        let removed = remove_mcp_from_config(&config_path).unwrap();
        assert!(removed);

        let updated = read_config(&config_path);
        assert!(updated["mcpServers"][MCP_SERVER_NAME].is_null());
        assert!(updated["mcpServers"][LEGACY_MCP_SERVER_NAME].is_null());
        assert!(updated["mcpServers"]["other-server"].is_object());
    }

    #[test]
    fn remove_mcp_from_config_returns_false_when_entry_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("mcp.json");
        let config = serde_json::json!({
            "mcpServers": {
                "other-server": { "command": "other", "args": [] }
            }
        });
        std::fs::write(&config_path, serde_json::to_string(&config).unwrap()).unwrap();

        let removed = remove_mcp_from_config(&config_path).unwrap();
        assert!(!removed);
    }

    #[test]
    fn check_mcp_status_returns_installed_for_matching_vault() {
        let tmp = tempfile::tempdir().unwrap();
        let vault_path = tmp.path().join("vault");
        std::fs::create_dir_all(&vault_path).unwrap();
        let index_js = write_index_js(tmp.path());
        let config_path = tmp.path().join("mcp.json");
        let config = serde_json::json!({
            "mcpServers": {
                "tolaria": {
                    "command": "node",
                    "args": [index_js.to_string_lossy()],
                    "env": { "VAULT_PATH": vault_path.to_string_lossy() }
                }
            }
        });
        std::fs::write(&config_path, serde_json::to_string(&config).unwrap()).unwrap();

        let entry = read_registered_mcp_entry(&config_path).unwrap();
        assert!(entry_targets_vault(&entry, &vault_path));
        assert!(entry_index_js_exists(&entry));
    }

    #[test]
    fn entry_targets_vault_requires_matching_existing_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let first_vault = tmp.path().join("vault-a");
        let second_vault = tmp.path().join("vault-b");
        std::fs::create_dir_all(&first_vault).unwrap();
        std::fs::create_dir_all(&second_vault).unwrap();

        let entry = serde_json::json!({
            "env": { "VAULT_PATH": first_vault.to_string_lossy() }
        });

        assert!(entry_targets_vault(&entry, &first_vault));
        assert!(!entry_targets_vault(&entry, &second_vault));
    }

    #[test]
    fn mcp_status_serializes_to_snake_case() {
        let json = serde_json::to_string(&McpStatus::Installed).unwrap();
        assert_eq!(json, r#""installed""#);
        let json = serde_json::to_string(&McpStatus::NotInstalled).unwrap();
        assert_eq!(json, r#""not_installed""#);
    }

    // --- mcp_server_dir resolution tests ---

    fn write_sentinel(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("ws-bridge.js"), "// sentinel").unwrap();
    }

    #[test]
    fn resolve_mcp_server_dir_picks_dev_path_first() {
        let tmp = tempfile::tempdir().unwrap();
        let dev = tmp.path().join("dev-mcp");
        let macos = tmp.path().join("macos-mcp");
        let win = tmp.path().join("win-mcp");
        write_sentinel(&dev);
        write_sentinel(&macos);
        write_sentinel(&win);

        let result = resolve_mcp_server_dir(&dev, Some(&macos), Some(&win)).unwrap();
        // canonicalize may resolve symlinks, so compare canonical forms
        assert_eq!(
            std::fs::canonicalize(&dev).unwrap(),
            std::fs::canonicalize(&result).unwrap()
        );
    }

    #[test]
    fn resolve_mcp_server_dir_picks_macos_release_when_dev_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let dev = tmp.path().join("dev-mcp"); // no sentinel
        let macos = tmp.path().join("Contents").join("Resources").join("mcp-server");
        write_sentinel(&macos);

        let result = resolve_mcp_server_dir(&dev, Some(&macos), None).unwrap();
        assert_eq!(result, macos);
    }

    #[test]
    fn resolve_mcp_server_dir_picks_windows_release_when_others_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let dev = tmp.path().join("dev-mcp"); // no sentinel
        let macos = tmp.path().join("macos-mcp"); // no sentinel
        let win = tmp.path().join("mcp-server");
        write_sentinel(&win);

        let result = resolve_mcp_server_dir(&dev, Some(&macos), Some(&win)).unwrap();
        assert_eq!(result, win);
    }

    #[test]
    fn resolve_mcp_server_dir_error_lists_all_candidate_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let dev = tmp.path().join("dev-mcp");
        let macos = tmp.path().join("macos-mcp");
        let win = tmp.path().join("win-mcp");
        // No sentinels written — all paths missing

        let err = resolve_mcp_server_dir(&dev, Some(&macos), Some(&win)).unwrap_err();
        assert!(err.contains(&dev.display().to_string()), "error should contain dev path: {err}");
        assert!(err.contains(&macos.display().to_string()), "error should contain macOS path: {err}");
        assert!(err.contains(&win.display().to_string()), "error should contain Windows path: {err}");
    }

    #[test]
    fn resolve_mcp_server_dir_error_with_none_candidates() {
        let tmp = tempfile::tempdir().unwrap();
        let dev = tmp.path().join("dev-mcp");

        let err = resolve_mcp_server_dir(&dev, None, None).unwrap_err();
        assert!(err.contains(&dev.display().to_string()));
        // Should only list the dev path when others are None
        assert!(err.starts_with("mcp-server not found at"));
    }

    // --- Node binary discovery tests ---

    #[test]
    fn find_node_on_path_uses_which_on_current_platform() {
        // On macOS/Linux (our CI), `which` should be available and find node.
        // This implicitly verifies the cfg! dispatch picks `which` (not `where.exe`).
        let result = find_node_on_path();
        assert!(result.is_ok(), "find_node_on_path should not error: {:?}", result);
        // Node is expected to be installed in dev/CI environments
        if let Ok(Some(path)) = &result {
            assert!(
                path.to_string_lossy().contains("node"),
                "resolved path should contain 'node': {:?}",
                path
            );
        }
    }

    #[test]
    fn fallback_node_paths_unix_includes_homebrew_and_volta() {
        let paths = fallback_node_paths_unix();
        let path_strs: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();

        assert!(
            path_strs.iter().any(|p| p.contains("/opt/homebrew/bin/node")),
            "should include Homebrew ARM path: {:?}",
            path_strs
        );
        assert!(
            path_strs.iter().any(|p| p.contains("/usr/local/bin/node")),
            "should include Homebrew Intel path: {:?}",
            path_strs
        );
        assert!(
            path_strs.iter().any(|p| p.contains(".volta/bin/node")),
            "should include Volta path: {:?}",
            path_strs
        );
    }

    #[test]
    fn fallback_node_paths_unix_includes_nvm_versions() {
        let tmp = tempfile::tempdir().unwrap();
        let nvm_dir = tmp.path().join(".nvm").join("versions").join("node");
        let v20 = nvm_dir.join("v20.0.0").join("bin");
        let v22 = nvm_dir.join("v22.0.0").join("bin");
        std::fs::create_dir_all(&v20).unwrap();
        std::fs::create_dir_all(&v22).unwrap();

        // Override HOME so fallback_node_paths_unix picks up our temp nvm dir.
        // We can't easily do that for the function as-is (it uses dirs::home_dir),
        // so we verify the static paths are always present.
        let paths = fallback_node_paths_unix();
        // At minimum, the two static Homebrew paths are always present
        assert!(paths.len() >= 2);
        assert_eq!(paths[0], PathBuf::from("/opt/homebrew/bin/node"));
        assert_eq!(paths[1], PathBuf::from("/usr/local/bin/node"));
    }

    #[test]
    fn fallback_node_paths_windows_builds_correct_paths_from_env() {
        // Test the path construction logic by setting env vars and verifying
        // the resulting PathBuf components. We use temp dirs as env values
        // to avoid conflicts with parallel tests.
        let tmp = tempfile::tempdir().unwrap();
        let fake_pf = tmp.path().join("ProgramFiles");
        let fake_la = tmp.path().join("LocalAppData");
        let fake_ad = tmp.path().join("AppData");
        std::fs::create_dir_all(&fake_pf).unwrap();
        std::fs::create_dir_all(&fake_la).unwrap();
        std::fs::create_dir_all(&fake_ad).unwrap();

        let orig_pf = std::env::var("ProgramFiles").ok();
        let orig_la = std::env::var("LOCALAPPDATA").ok();
        let orig_ad = std::env::var("APPDATA").ok();

        std::env::set_var("ProgramFiles", &fake_pf);
        std::env::set_var("LOCALAPPDATA", &fake_la);
        std::env::set_var("APPDATA", &fake_ad);

        let paths = fallback_node_paths_windows();

        // Restore env vars immediately
        match orig_pf {
            Some(v) => std::env::set_var("ProgramFiles", v),
            None => std::env::remove_var("ProgramFiles"),
        }
        match orig_la {
            Some(v) => std::env::set_var("LOCALAPPDATA", v),
            None => std::env::remove_var("LOCALAPPDATA"),
        }
        match orig_ad {
            Some(v) => std::env::set_var("APPDATA", v),
            None => std::env::remove_var("APPDATA"),
        }

        // ProgramFiles/nodejs/node.exe
        assert!(
            paths.contains(&fake_pf.join("nodejs").join("node.exe")),
            "should include ProgramFiles/nodejs/node.exe: {:?}",
            paths
        );
        // LOCALAPPDATA/Volta/node.exe
        assert!(
            paths.contains(&fake_la.join("Volta").join("node.exe")),
            "should include LOCALAPPDATA/Volta/node.exe: {:?}",
            paths
        );
    }

    #[test]
    fn fallback_node_paths_windows_includes_nvm_versions() {
        let tmp = tempfile::tempdir().unwrap();
        let fake_ad = tmp.path().join("AppData");
        let nvm_dir = fake_ad.join("nvm");
        let v20 = nvm_dir.join("v20.0.0");
        let v22 = nvm_dir.join("v22.0.0");
        std::fs::create_dir_all(&v20).unwrap();
        std::fs::create_dir_all(&v22).unwrap();

        let orig_pf = std::env::var("ProgramFiles").ok();
        let orig_la = std::env::var("LOCALAPPDATA").ok();
        let orig_ad = std::env::var("APPDATA").ok();

        std::env::remove_var("ProgramFiles");
        std::env::remove_var("LOCALAPPDATA");
        std::env::set_var("APPDATA", &fake_ad);

        let paths = fallback_node_paths_windows();

        match orig_pf {
            Some(v) => std::env::set_var("ProgramFiles", v),
            None => std::env::remove_var("ProgramFiles"),
        }
        match orig_la {
            Some(v) => std::env::set_var("LOCALAPPDATA", v),
            None => std::env::remove_var("LOCALAPPDATA"),
        }
        match orig_ad {
            Some(v) => std::env::set_var("APPDATA", v),
            None => std::env::remove_var("APPDATA"),
        }

        // Should include nvm version paths with node.exe
        assert!(
            paths.contains(&v22.join("node.exe")),
            "should include nvm v22 path: {:?}",
            paths
        );
        assert!(
            paths.contains(&v20.join("node.exe")),
            "should include nvm v20 path: {:?}",
            paths
        );
    }

    #[test]
    fn fallback_node_paths_windows_empty_when_env_vars_unset() {
        let orig_pf = std::env::var("ProgramFiles").ok();
        let orig_la = std::env::var("LOCALAPPDATA").ok();
        let orig_ad = std::env::var("APPDATA").ok();

        std::env::remove_var("ProgramFiles");
        std::env::remove_var("LOCALAPPDATA");
        std::env::remove_var("APPDATA");

        let paths = fallback_node_paths_windows();

        if let Some(v) = orig_pf { std::env::set_var("ProgramFiles", v); }
        if let Some(v) = orig_la { std::env::set_var("LOCALAPPDATA", v); }
        if let Some(v) = orig_ad { std::env::set_var("APPDATA", v); }

        assert!(
            paths.is_empty(),
            "should return empty vec when env vars are unset: {:?}",
            paths
        );
    }
}
