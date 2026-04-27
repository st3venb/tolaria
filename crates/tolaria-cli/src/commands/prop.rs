use std::path::Path;

use gray_matter::engine::YAML;
use gray_matter::Matter;
use tolaria_core::boundary::{canonicalize_candidate_for_write, validate_path_within_vault};
use tolaria_core::frontmatter::{delete_frontmatter_property, update_frontmatter, FrontmatterValue};
use tolaria_core::vault::scan_vault_cached;

use crate::output::OutputContext;
use crate::resolve::resolve_note;

/// `prop get <note> <key>` — display a single frontmatter property value.
pub fn run_get(vault_path: &str, note_ref: &str, key: &str, output: &OutputContext) {
    let (content, _) = resolve_and_read(vault_path, note_ref, output);
    let matter = Matter::<YAML>::new();
    let parsed = matter.parse(&content);

    let value = parsed
        .data
        .and_then(|pod| extract_pod_key(&pod, key));

    match value {
        Some(v) => println!("{}", v),
        None => {
            output.error(&format!("Property not found: {}", key));
            std::process::exit(1);
        }
    }
}

/// `prop set <note> <key> <value>` — update a frontmatter property.
pub fn run_set(vault_path: &str, note_ref: &str, key: &str, value: &str, output: &OutputContext) {
    let (_, note_path) = resolve_and_read(vault_path, note_ref, output);
    validate_boundary(vault_path, &note_path, output);

    match update_frontmatter(&note_path, key, FrontmatterValue::String(value.to_string())) {
        Ok(_) => output.info(&format!("Set {}={}", key, value)),
        Err(e) => {
            output.error(&e);
            std::process::exit(1);
        }
    }
}

/// `prop delete <note> <key>` — remove a frontmatter property.
pub fn run_delete(vault_path: &str, note_ref: &str, key: &str, output: &OutputContext) {
    let (_, note_path) = resolve_and_read(vault_path, note_ref, output);
    validate_boundary(vault_path, &note_path, output);

    match delete_frontmatter_property(&note_path, key) {
        Ok(_) => output.info(&format!("Deleted property: {}", key)),
        Err(e) => {
            output.error(&e);
            std::process::exit(1);
        }
    }
}

/// `prop list <note>` — display all frontmatter key-value pairs.
pub fn run_list(vault_path: &str, note_ref: &str, output: &OutputContext) {
    let (content, _) = resolve_and_read(vault_path, note_ref, output);
    let matter = Matter::<YAML>::new();
    let parsed = matter.parse(&content);

    match parsed.data {
        Some(gray_matter::Pod::Hash(map)) => {
            if map.is_empty() {
                output.info("No frontmatter properties.");
                return;
            }
            for (key, pod) in &map {
                println!("{}: {}", key, format_pod(pod));
            }
        }
        _ => {
            output.info("No frontmatter properties.");
        }
    }
}

/// Resolve a note reference and read its file content.
/// Returns (file_content, absolute_path).
fn resolve_and_read(vault_path: &str, note_ref: &str, output: &OutputContext) -> (String, String) {
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

    let content = match std::fs::read_to_string(&entry.path) {
        Ok(c) => c,
        Err(e) => {
            output.error(&format!("Failed to read {}: {}", entry.path, e));
            std::process::exit(1);
        }
    };

    (content, entry.path.clone())
}

/// Validate that the note path is within the vault boundary.
fn validate_boundary(vault_path: &str, note_path: &str, output: &OutputContext) {
    let vault_root = match Path::new(vault_path).canonicalize() {
        Ok(p) => p,
        Err(e) => {
            output.error(&format!("Cannot resolve vault path: {}", e));
            std::process::exit(1);
        }
    };

    let target = match canonicalize_candidate_for_write(Path::new(note_path)) {
        Ok(p) => p,
        Err(e) => {
            output.error(&e);
            std::process::exit(1);
        }
    };

    if let Err(e) = validate_path_within_vault(&vault_root, &target) {
        output.error(&e);
        std::process::exit(1);
    }
}

/// Extract a single key's value from a gray_matter Pod.
fn extract_pod_key(pod: &gray_matter::Pod, key: &str) -> Option<String> {
    match pod {
        gray_matter::Pod::Hash(map) => map.get(key).map(format_pod),
        _ => None,
    }
}

/// Format a gray_matter Pod value as a display string.
fn format_pod(pod: &gray_matter::Pod) -> String {
    match pod {
        gray_matter::Pod::String(s) => s.clone(),
        gray_matter::Pod::Integer(n) => n.to_string(),
        gray_matter::Pod::Float(f) => f.to_string(),
        gray_matter::Pod::Boolean(b) => b.to_string(),
        gray_matter::Pod::Null => "null".to_string(),
        gray_matter::Pod::Array(arr) => {
            let items: Vec<String> = arr.iter().map(format_pod).collect();
            format!("[{}]", items.join(", "))
        }
        gray_matter::Pod::Hash(map) => {
            let pairs: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{}: {}", k, format_pod(v)))
                .collect();
            format!("{{{}}}", pairs.join(", "))
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::fs;
    use tempfile::TempDir;

    /// Create a temp vault with a single note file containing the given frontmatter keys.
    /// Returns (vault_dir, note_path, filename).
    fn create_temp_note(
        frontmatter_pairs: &[(&str, &str)],
    ) -> (TempDir, String, String) {
        let dir = TempDir::new().unwrap();
        let filename = "test-note.md";
        let note_path = dir.path().join(filename);

        let mut fm_lines = Vec::new();
        for (k, v) in frontmatter_pairs {
            fm_lines.push(format!("{}: {}", k, v));
        }

        let content = if fm_lines.is_empty() {
            "---\n---\n# Test Note\n".to_string()
        } else {
            format!("---\n{}\n---\n# Test Note\n", fm_lines.join("\n"))
        };

        fs::write(&note_path, &content).unwrap();
        (dir, note_path.to_string_lossy().to_string(), filename.to_string())
    }

    /// Read a frontmatter property from a file using gray_matter.
    fn read_property(path: &str, key: &str) -> Option<String> {
        let content = fs::read_to_string(path).unwrap();
        let matter = Matter::<YAML>::new();
        let parsed = matter.parse(&content);
        parsed.data.and_then(|pod| extract_pod_key(&pod, key))
    }

    /// Read all frontmatter keys from a file.
    fn read_all_keys(path: &str) -> Vec<String> {
        let content = fs::read_to_string(path).unwrap();
        let matter = Matter::<YAML>::new();
        let parsed = matter.parse(&content);
        match parsed.data {
            Some(gray_matter::Pod::Hash(map)) => map.keys().cloned().collect(),
            _ => Vec::new(),
        }
    }

    // ── Property 9: Frontmatter Property Set/Get Round-Trip ─────────
    // **Validates: Requirements 5.1, 5.2**
    //
    // For any note and valid key-value pair, set then get returns the
    // same value.

    /// Strategy for valid YAML property keys (alphanumeric + underscores).
    fn arb_prop_key() -> impl Strategy<Value = String> {
        "[a-zA-Z][a-zA-Z0-9_]{0,12}"
    }

    /// Strategy for simple string values that survive YAML round-trip.
    fn arb_prop_value() -> impl Strategy<Value = String> {
        "[a-zA-Z][a-zA-Z0-9 ]{0,30}"
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_frontmatter_set_get_roundtrip(
            key in arb_prop_key(),
            value in arb_prop_value(),
        ) {
            let (_dir, note_path, _) = create_temp_note(&[("type", "Note")]);

            // Set the property
            update_frontmatter(&note_path, &key, FrontmatterValue::String(value.clone()))
                .expect("update_frontmatter should succeed");

            // Get the property back
            let retrieved = read_property(&note_path, &key);
            prop_assert!(
                retrieved.is_some(),
                "Property '{}' not found after set",
                key
            );
            let retrieved_val = retrieved.unwrap();
            prop_assert_eq!(
                retrieved_val.trim(),
                value.trim(),
                "Round-trip failed for key='{}'",
                key
            );
        }
    }

    // ── Property 10: Frontmatter Property Deletion ──────────────────
    // **Validates: Requirements 5.3**
    //
    // For any note with a property, deleting it makes it absent while
    // preserving other properties.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_frontmatter_deletion_preserves_others(
            target_key in arb_prop_key(),
            target_value in arb_prop_value(),
            other_key in arb_prop_key(),
            other_value in arb_prop_value(),
        ) {
            // Ensure keys are distinct
            prop_assume!(target_key != other_key);

            let (_dir, note_path, _) = create_temp_note(&[]);

            // Set both properties
            update_frontmatter(
                &note_path,
                &other_key,
                FrontmatterValue::String(other_value.clone()),
            )
            .expect("set other_key");
            update_frontmatter(
                &note_path,
                &target_key,
                FrontmatterValue::String(target_value.clone()),
            )
            .expect("set target_key");

            // Verify both exist
            prop_assert!(read_property(&note_path, &target_key).is_some());
            prop_assert!(read_property(&note_path, &other_key).is_some());

            // Delete the target property
            delete_frontmatter_property(&note_path, &target_key)
                .expect("delete should succeed");

            // Target key should be absent
            let after_delete = read_property(&note_path, &target_key);
            prop_assert!(
                after_delete.is_none(),
                "Property '{}' should be absent after deletion, got: {:?}",
                target_key,
                after_delete
            );

            // Other key should still be present with its value
            let other_after = read_property(&note_path, &other_key);
            prop_assert!(
                other_after.is_some(),
                "Property '{}' should be preserved after deleting '{}'",
                other_key,
                target_key
            );
            let other_after_val = other_after.unwrap();
            prop_assert_eq!(
                other_after_val.trim(),
                other_value.trim(),
                "Other property value changed after deletion"
            );
        }
    }
}
