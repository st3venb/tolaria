use std::io::{self, BufRead, Write};
use std::path::Path;

use tolaria_core::vault::{delete_note, scan_vault_cached};

use crate::output::OutputContext;
use crate::resolve::resolve_note;

/// Delete a note, prompting for confirmation unless --force or non-TTY stdin.
pub fn run(vault_path: &str, note_ref: &str, force: bool, output: &OutputContext) {
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

    let skip_prompt = force || !atty::is(atty::Stream::Stdin);

    if !skip_prompt {
        eprint!(
            "Delete \"{}\" ({})? [y/N] ",
            entry.title, entry.filename
        );
        io::stderr().flush().ok();

        let mut answer = String::new();
        if io::stdin().lock().read_line(&mut answer).is_err() {
            output.error("Failed to read confirmation input");
            std::process::exit(1);
        }
        let answer = answer.trim().to_lowercase();
        if answer != "y" && answer != "yes" {
            output.info("Cancelled.");
            return;
        }
    }

    match delete_note(&entry.path) {
        Ok(_) => {
            output.info(&format!("Deleted {}", entry.filename));
        }
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    }
}
