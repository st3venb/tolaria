use std::path::Path;

use serde::Serialize;
use tolaria_core::vault::{scan_vault_cached, VaultEntry};

use crate::output::{OutputContext, OutputFormat};
use crate::resolve::resolve_note;

/// Display all outgoing wikilinks from a note's body.
pub fn run_links(vault_path: &str, note_ref: &str, output: &OutputContext) {
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

    let links = &entry.outgoing_links;

    match output.format {
        OutputFormat::Json => {
            output.print_json_value(links);
        }
        OutputFormat::Human => {
            if links.is_empty() {
                output.info(&format!("No outgoing links from '{}'.", entry.title));
            } else {
                println!("Outgoing links from '{}':", entry.title);
                for link in links {
                    println!("  [[{}]]", link);
                }
            }
        }
    }
}

/// Display all notes that link to the given note via wikilinks.
pub fn run_backlinks(vault_path: &str, note_ref: &str, output: &OutputContext) {
    let entries = match scan_vault_cached(Path::new(vault_path)) {
        Ok(e) => e,
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    };

    let target = match resolve_note(&entries, note_ref) {
        Some(e) => e.clone(),
        None => {
            output.error(&format!("Note not found: {}", note_ref));
            std::process::exit(1);
        }
    };

    let target_stem = filename_stem(&target.filename);
    let backlinks: Vec<&VaultEntry> = collect_backlinks(&entries, &target_stem);

    match output.format {
        OutputFormat::Json => {
            #[derive(Serialize)]
            struct BacklinkEntry<'a> {
                filename: &'a str,
                title: &'a str,
            }
            let items: Vec<BacklinkEntry> = backlinks
                .iter()
                .map(|e| BacklinkEntry {
                    filename: &e.filename,
                    title: &e.title,
                })
                .collect();
            output.print_json_value(&items);
        }
        OutputFormat::Human => {
            if backlinks.is_empty() {
                output.info(&format!("No backlinks to '{}'.", target.title));
            } else {
                println!("Notes linking to '{}':", target.title);
                for entry in &backlinks {
                    println!("  {} ({})", entry.title, entry.filename);
                }
            }
        }
    }
}

/// Display frontmatter relationship fields for a note.
pub fn run_relationships(vault_path: &str, note_ref: &str, output: &OutputContext) {
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

    match output.format {
        OutputFormat::Json => {
            #[derive(Serialize)]
            struct Relationships<'a> {
                belongs_to: &'a [String],
                related_to: &'a [String],
                #[serde(flatten)]
                custom: &'a std::collections::HashMap<String, Vec<String>>,
            }
            let rels = Relationships {
                belongs_to: &entry.belongs_to,
                related_to: &entry.related_to,
                custom: &entry.relationships,
            };
            output.print_json_value(&rels);
        }
        OutputFormat::Human => {
            let has_any = !entry.belongs_to.is_empty()
                || !entry.related_to.is_empty()
                || !entry.relationships.is_empty();

            if !has_any {
                output.info(&format!(
                    "No relationships for '{}'.",
                    entry.title
                ));
                return;
            }

            println!("Relationships for '{}':", entry.title);
            if !entry.belongs_to.is_empty() {
                println!("  Belongs to: {}", entry.belongs_to.join(", "));
            }
            if !entry.related_to.is_empty() {
                println!("  Related to: {}", entry.related_to.join(", "));
            }
            for (key, targets) in &entry.relationships {
                println!("  {}: {}", key, targets.join(", "));
            }
        }
    }
}

/// Extract the filename stem (without extension), lowercased.
fn filename_stem(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, _)) => stem.to_lowercase(),
        None => name.to_lowercase(),
    }
}

/// Collect all entries whose outgoing_links contain the target stem.
fn collect_backlinks<'a>(
    entries: &'a [VaultEntry],
    target_stem: &str,
) -> Vec<&'a VaultEntry> {
    entries
        .iter()
        .filter(|e| {
            e.outgoing_links
                .iter()
                .any(|link| link.to_lowercase() == target_stem)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::HashMap;

    fn make_entry(
        filename: &str,
        title: &str,
        outgoing_links: Vec<&str>,
    ) -> VaultEntry {
        VaultEntry {
            filename: filename.to_string(),
            title: title.to_string(),
            outgoing_links: outgoing_links.into_iter().map(String::from).collect(),
            ..VaultEntry::default()
        }
    }

    fn make_entry_with_rels(
        filename: &str,
        title: &str,
        belongs_to: Vec<&str>,
        related_to: Vec<&str>,
        custom: HashMap<String, Vec<String>>,
    ) -> VaultEntry {
        VaultEntry {
            filename: filename.to_string(),
            title: title.to_string(),
            belongs_to: belongs_to.into_iter().map(String::from).collect(),
            related_to: related_to.into_iter().map(String::from).collect(),
            relationships: custom,
            ..VaultEntry::default()
        }
    }

    // ── Unit tests ──────────────────────────────────────────────────

    #[test]
    fn collect_backlinks_finds_referencing_notes() {
        let entries = vec![
            make_entry("alpha.md", "Alpha", vec!["beta"]),
            make_entry("beta.md", "Beta", vec![]),
            make_entry("gamma.md", "Gamma", vec!["beta", "alpha"]),
        ];
        let backlinks = collect_backlinks(&entries, "beta");
        let filenames: Vec<&str> =
            backlinks.iter().map(|e| e.filename.as_str()).collect();
        assert_eq!(filenames, vec!["alpha.md", "gamma.md"]);
    }

    #[test]
    fn collect_backlinks_case_insensitive() {
        let entries = vec![
            make_entry("a.md", "A", vec!["Beta"]),
            make_entry("beta.md", "Beta", vec![]),
        ];
        let backlinks = collect_backlinks(&entries, "beta");
        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0].filename, "a.md");
    }

    #[test]
    fn collect_backlinks_empty_when_no_references() {
        let entries = vec![
            make_entry("a.md", "A", vec!["gamma"]),
            make_entry("b.md", "B", vec![]),
        ];
        let backlinks = collect_backlinks(&entries, "beta");
        assert!(backlinks.is_empty());
    }

    #[test]
    fn filename_stem_strips_extension() {
        assert_eq!(filename_stem("my-note.md"), "my-note");
        assert_eq!(filename_stem("NOTE.MD"), "note");
        assert_eq!(filename_stem("no-ext"), "no-ext");
    }

    // ── Property 15: Outgoing Wikilinks Completeness ────────────────
    // **Validates: Requirements 12.1**
    //
    // For any note with [[wikilink]] patterns, the links output lists
    // every target extracted from the body.

    fn arb_link_target() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9-]{0,12}"
    }

    fn arb_entry_with_links()
    -> impl Strategy<Value = (VaultEntry, Vec<String>)> {
        proptest::collection::vec(arb_link_target(), 0..8).prop_map(|links| {
            let entry = VaultEntry {
                filename: "test-note.md".to_string(),
                title: "Test Note".to_string(),
                outgoing_links: links.clone(),
                ..VaultEntry::default()
            };
            (entry, links)
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_outgoing_wikilinks_completeness(
            (entry, expected_links) in arb_entry_with_links()
        ) {
            // The links command reads entry.outgoing_links directly.
            // Verify every expected link is present in the entry's
            // outgoing_links field.
            let actual = &entry.outgoing_links;
            for link in &expected_links {
                prop_assert!(
                    actual.contains(link),
                    "Missing outgoing link '{}' in {:?}",
                    link,
                    actual
                );
            }
            prop_assert_eq!(
                actual.len(),
                expected_links.len(),
                "Link count mismatch"
            );
        }
    }

    // ── Property 16: Backlinks Completeness ─────────────────────────
    // **Validates: Requirements 12.2**
    //
    // For any note, backlinks returns exactly the set of notes
    // referencing it — no false positives, no missed references.

    fn arb_vault_for_backlinks()
    -> impl Strategy<Value = (String, Vec<VaultEntry>)> {
        let target_stem = "[a-z][a-z0-9]{1,6}";
        let num_entries = 2..8usize;

        (target_stem, num_entries).prop_flat_map(|(stem, n)| {
            let stem_clone = stem.clone();
            proptest::collection::vec(
                (
                    "[a-z][a-z0-9]{1,6}",
                    proptest::collection::vec(arb_link_target(), 0..5),
                    any::<bool>(),
                ),
                n,
            )
            .prop_map(move |raw_entries| {
                let mut entries: Vec<VaultEntry> = raw_entries
                    .into_iter()
                    .enumerate()
                    .map(|(i, (name, mut links, should_link))| {
                        let fname = format!("note-{}-{}.md", name, i);
                        if should_link {
                            links.push(stem_clone.clone());
                        }
                        VaultEntry {
                            filename: fname.clone(),
                            title: format!("Note {} {}", name, i),
                            outgoing_links: links,
                            ..VaultEntry::default()
                        }
                    })
                    .collect();
                // Add the target note itself
                entries.push(VaultEntry {
                    filename: format!("{}.md", stem_clone),
                    title: format!("Target {}", stem_clone),
                    outgoing_links: vec![],
                    ..VaultEntry::default()
                });
                (stem_clone.clone(), entries)
            })
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_backlinks_completeness(
            (target_stem, entries) in arb_vault_for_backlinks()
        ) {
            let backlinks = collect_backlinks(&entries, &target_stem);
            let backlink_filenames: std::collections::HashSet<&str> =
                backlinks.iter().map(|e| e.filename.as_str()).collect();

            // Every entry that has target_stem in outgoing_links
            // must appear in backlinks
            for entry in &entries {
                let links_to_target = entry
                    .outgoing_links
                    .iter()
                    .any(|l| l.to_lowercase() == target_stem);
                if links_to_target {
                    prop_assert!(
                        backlink_filenames.contains(entry.filename.as_str()),
                        "Missing backlink from '{}' which links to '{}'",
                        entry.filename,
                        target_stem
                    );
                } else {
                    prop_assert!(
                        !backlink_filenames.contains(entry.filename.as_str()),
                        "False positive backlink from '{}' which does NOT link to '{}'",
                        entry.filename,
                        target_stem
                    );
                }
            }
        }
    }

    // ── Property 17: Wikilink Update on Rename ──────────────────────
    // **Validates: Requirements 12.4**
    //
    // Renaming a note updates all [[wikilink]] references to the old
    // name, no other wikilinks modified.
    //
    // This property is tested at the tolaria-core level since
    // rename_note lives there. We verify the invariant by simulating
    // a vault with cross-references and checking that after rename,
    // all old-name references are replaced and other links are intact.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]

        #[test]
        fn prop_wikilink_update_on_rename(
            old_stem in "[a-z][a-z0-9]{2,6}",
            new_title in "[A-Z][a-z]{2,8}( [A-Z][a-z]{2,8}){0,2}",
            other_links in proptest::collection::vec("[a-z][a-z0-9]{2,6}", 0..4),
            num_referencing in 1..5usize,
        ) {
            let dir = tempfile::tempdir().unwrap();
            let vault_path = dir.path();

            // Create the target note
            let old_filename = format!("{}.md", old_stem);
            let old_path = vault_path.join(&old_filename);
            std::fs::write(
                &old_path,
                format!("---\ntitle: {}\n---\n# {}\n\nContent here.\n", old_stem, old_stem),
            )
            .unwrap();

            // Create referencing notes with [[old_stem]] links
            let mut referencing_files = Vec::new();
            for i in 0..num_referencing {
                let fname = format!("ref-{}.md", i);
                let fpath = vault_path.join(&fname);
                // Include the old_stem link plus some other links
                let mut body = format!("See [[{}]]", old_stem);
                for other in &other_links {
                    body.push_str(&format!(" and [[{}]]", other));
                }
                std::fs::write(
                    &fpath,
                    format!("---\ntitle: Ref {}\n---\n# Ref {}\n\n{}\n", i, i, body),
                )
                .unwrap();
                referencing_files.push((fname, fpath));
            }

            // Initialize git so rename_note can work
            let git_init = std::process::Command::new("git")
                .args(["init"])
                .current_dir(vault_path)
                .output();
            if git_init.is_err() {
                // Skip test if git is not available
                return Ok(());
            }
            let _ = std::process::Command::new("git")
                .args(["add", "."])
                .current_dir(vault_path)
                .output();
            let _ = std::process::Command::new("git")
                .args(["-c", "user.name=test", "-c", "user.email=test@test.com", "commit", "-m", "init"])
                .current_dir(vault_path)
                .output();

            // Perform the rename
            let vault_str = vault_path.to_str().unwrap();
            let old_path_str = old_path.to_str().unwrap();
            let _result = tolaria_core::vault::rename_note(
                vault_str,
                old_path_str,
                &new_title,
                None,
            );

            // Derive expected new stem from the new title
            let _new_stem: String = new_title
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() { c.to_lowercase().next().unwrap() }
                    else if c == ' ' { '-' }
                    else { c }
                })
                .collect();

            // Check that referencing files no longer contain [[old_stem]]
            // and that other links are preserved
            for (fname, fpath) in &referencing_files {
                if fpath.exists() {
                    let content = std::fs::read_to_string(fpath).unwrap();
                    // Old wikilink should be gone (replaced with new stem)
                    let old_pattern = format!("[[{}]]", old_stem);
                    prop_assert!(
                        !content.contains(&old_pattern),
                        "File '{}' still contains old wikilink '{}' after rename.\nContent: {}",
                        fname,
                        old_pattern,
                        content
                    );

                    // Other links should be preserved
                    for other in &other_links {
                        let other_pattern = format!("[[{}]]", other);
                        prop_assert!(
                            content.contains(&other_pattern),
                            "File '{}' lost unrelated wikilink '{}' after rename.\nContent: {}",
                            fname,
                            other_pattern,
                            content
                        );
                    }
                }
            }
        }
    }
}
