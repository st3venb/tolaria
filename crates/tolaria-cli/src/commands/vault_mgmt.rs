use std::path::Path;

use tolaria_core::git;
use tolaria_core::vault::seed_config_files;
use tolaria_core::vault_list::{self, VaultEntry as VaultListEntry};

use crate::output::{OutputContext, OutputFormat};

// ── init ────────────────────────────────────────────────────────────

pub fn run_init(path: &str, output: &OutputContext) {
    let dir = Path::new(path);

    // Create the vault directory if it doesn't exist
    if let Err(e) = std::fs::create_dir_all(dir) {
        output.error(&format!("Failed to create directory: {e}"));
        std::process::exit(1);
    }

    // Initialize git repo
    if let Err(e) = git::init_repo(path) {
        output.error(&format!("Failed to initialize git repo: {e}"));
        std::process::exit(1);
    }

    // Seed default config files (AGENTS.md, type definitions, etc.)
    seed_config_files(path);

    // Register in vault list
    register_vault(path, output);

    match output.format {
        OutputFormat::Json => {
            output.print_json_value(&serde_json::json!({
                "status": "created",
                "path": path,
            }));
        }
        OutputFormat::Human => {
            output.info(&format!("Vault initialized at: {path}"));
        }
    }
}

// ── clone ───────────────────────────────────────────────────────────

pub fn run_clone(url: &str, path: &str, output: &OutputContext) {
    match git::clone_repo(url, path) {
        Ok(msg) => {
            // Register the cloned vault
            register_vault(path, output);

            match output.format {
                OutputFormat::Json => {
                    output.print_json_value(&serde_json::json!({
                        "status": "cloned",
                        "path": path,
                        "message": msg,
                    }));
                }
                OutputFormat::Human => {
                    output.info(&format!("Vault cloned to: {path}"));
                }
            }
        }
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    }
}

// ── vault list ──────────────────────────────────────────────────────

pub fn run_vault_list(output: &OutputContext) {
    let list = match vault_list::load_vault_list() {
        Ok(l) => l,
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    };

    match output.format {
        OutputFormat::Json => output.print_json_value(&list),
        OutputFormat::Human => {
            if list.vaults.is_empty() {
                output.info("No registered vaults.");
                return;
            }
            for vault in &list.vaults {
                let active = list
                    .active_vault
                    .as_deref()
                    .is_some_and(|a| a == vault.path);
                let marker = if active { "* " } else { "  " };
                println!("{marker}{} ({})", vault.label, vault.path);
            }
        }
    }
}

// ── vault switch ────────────────────────────────────────────────────

pub fn run_vault_switch(path: &str, output: &OutputContext) {
    let mut list = match vault_list::load_vault_list() {
        Ok(l) => l,
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    };

    list.active_vault = Some(path.to_string());

    if let Err(msg) = vault_list::save_vault_list(&list) {
        output.error(&msg);
        std::process::exit(1);
    }

    match output.format {
        OutputFormat::Json => {
            output.print_json_value(&serde_json::json!({
                "status": "switched",
                "active_vault": path,
            }));
        }
        OutputFormat::Human => {
            output.info(&format!("Active vault set to: {path}"));
        }
    }
}

// ── helpers ─────────────────────────────────────────────────────────

fn register_vault(path: &str, output: &OutputContext) {
    let mut list = vault_list::load_vault_list().unwrap_or_default();

    // Don't add duplicates
    if list.vaults.iter().any(|v| v.path == path) {
        return;
    }

    let label = Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());

    list.vaults.push(VaultListEntry {
        label,
        path: path.to_string(),
    });

    if list.active_vault.is_none() {
        list.active_vault = Some(path.to_string());
    }

    if let Err(e) = vault_list::save_vault_list(&list) {
        output.error(&format!("Warning: failed to register vault: {e}"));
    }
}
