use std::path::Path;
use std::process::Command;

use tolaria_core::vault::scan_vault_cached;

use crate::output::OutputContext;
use crate::resolve::resolve_note;

/// Open a note in $EDITOR (fallback to vim), wait for the editor to exit.
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

    let editor = resolve_editor(output);

    let status = Command::new(&editor)
        .arg(&entry.path)
        .status()
        .map_err(|e| format!("Failed to launch editor '{}': {}", editor, e));

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            let code = s.code().unwrap_or(-1);
            output.error(&format!("Editor exited with status {}", code));
            std::process::exit(1);
        }
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    }
}

/// Resolve the editor to use: $EDITOR, then vim, with a helpful error if
/// neither is available.
pub(crate) fn resolve_editor(output: &OutputContext) -> String {
    if let Ok(editor) = std::env::var("EDITOR") {
        if !editor.is_empty() {
            return editor;
        }
    }

    // Check if vim is available on PATH
    if Command::new("vim").arg("--version").output().is_ok() {
        return "vim".to_string();
    }

    output.error(
        "No editor found. Set the $EDITOR environment variable:\n  export EDITOR=vim\n  export EDITOR=nano\n  export EDITOR=code",
    );
    std::process::exit(1);
}
