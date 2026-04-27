use std::path::Path;

use tolaria_core::vault::{get_note_content, scan_vault_cached};

use crate::output::OutputContext;
use crate::resolve::resolve_note;

/// Display a note's frontmatter properties and markdown body.
pub fn run(vault_path: &str, note_ref: &str, output: &OutputContext) {
    let entries = match scan_vault_cached(Path::new(vault_path)) {
        Ok(e) => e,
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    };

    let entry = match resolve_note(&entries, note_ref) {
        Some(e) => e.clone(),
        None => {
            output.error(&format!("Note not found: {}", note_ref));
            std::process::exit(1);
        }
    };

    let content = match get_note_content(Path::new(&entry.path)) {
        Ok(c) => c,
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    };

    output.print_entry_detail(&entry, &content);
}
