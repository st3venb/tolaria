use tolaria_core::mcp;

use crate::output::{OutputContext, OutputFormat};

// ── mcp start ───────────────────────────────────────────────────────

pub fn run_start(vault_path: &str, output: &OutputContext) {
    match mcp::spawn_ws_bridge(vault_path) {
        Ok(child) => {
            let pid = child.id();
            match output.format {
                OutputFormat::Json => {
                    output.print_json_value(&serde_json::json!({
                        "status": "started",
                        "pid": pid,
                        "vault": vault_path,
                    }));
                }
                OutputFormat::Human => {
                    output.info(&format!(
                        "MCP WebSocket bridge started (pid: {pid})"
                    ));
                }
            }
        }
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    }
}

// ── mcp stop ────────────────────────────────────────────────────────

pub fn run_stop(output: &OutputContext) {
    // The bridge is a detached child process; we look for it by name.
    let result = find_and_kill_bridge();
    match result {
        Ok(msg) => {
            match output.format {
                OutputFormat::Json => {
                    output.print_json_value(&serde_json::json!({
                        "status": "stopped",
                        "message": msg,
                    }));
                }
                OutputFormat::Human => output.info(&msg),
            }
        }
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    }
}

/// Find and kill the ws-bridge process by searching for it via `pgrep`.
fn find_and_kill_bridge() -> Result<String, String> {
    let pgrep = std::process::Command::new("pgrep")
        .args(["-f", "ws-bridge.js"])
        .output()
        .map_err(|e| format!("Failed to run pgrep: {e}"))?;

    if !pgrep.status.success() {
        return Err("MCP bridge is not running.".into());
    }

    let stdout_str = String::from_utf8_lossy(&pgrep.stdout).to_string();
    let pids: Vec<&str> = stdout_str
        .trim()
        .lines()
        .filter(|s| !s.is_empty())
        .collect();

    if pids.is_empty() {
        return Err("MCP bridge is not running.".into());
    }

    for pid in &pids {
        let _ = std::process::Command::new("kill")
            .arg(pid)
            .output();
    }

    Ok(format!("Stopped MCP bridge (pid: {}).", pids.join(", ")))
}

// ── mcp status ──────────────────────────────────────────────────────

pub fn run_status(vault_path: &str, output: &OutputContext) {
    let status = mcp::check_mcp_status(vault_path);
    match output.format {
        OutputFormat::Json => output.print_json_value(&status),
        OutputFormat::Human => {
            let label = match status {
                mcp::McpStatus::Installed => "installed (registered and server files present)",
                mcp::McpStatus::NotInstalled => "not installed",
            };
            output.info(&format!("MCP status: {label}"));
        }
    }
}

// ── mcp register ────────────────────────────────────────────────────

pub fn run_register(vault_path: &str, output: &OutputContext) {
    match mcp::register_mcp(vault_path) {
        Ok(result) => {
            match output.format {
                OutputFormat::Json => {
                    output.print_json_value(&serde_json::json!({
                        "status": result,
                        "vault": vault_path,
                    }));
                }
                OutputFormat::Human => {
                    output.info(&format!("MCP server {result} for vault: {vault_path}"));
                }
            }
        }
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    }
}

// ── mcp unregister ──────────────────────────────────────────────────

pub fn run_unregister(output: &OutputContext) {
    let result = mcp::remove_mcp();
    match output.format {
        OutputFormat::Json => {
            output.print_json_value(&serde_json::json!({
                "status": result,
            }));
        }
        OutputFormat::Human => {
            let msg = match result.as_str() {
                "removed" => "MCP server entries removed from AI tool configs.",
                "already_absent" => "MCP server was not registered in any config.",
                _ => &result,
            };
            output.info(msg);
        }
    }
}
