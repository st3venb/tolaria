use tolaria_core::vault::VaultEntry;

/// Resolve a user-provided note reference to a VaultEntry.
///
/// Resolution tries these passes in order, returning the first match:
/// 1. Exact filename stem match (e.g. `my-project` → `my-project.md`)
/// 2. Alias match (from frontmatter `aliases:`)
/// 3. Exact title match
/// 4. Humanized title match (kebab-case → words, case-insensitive)
/// 5. Last segment of path-style references (e.g. `person/alice` → `alice`)
pub fn resolve_note<'a>(entries: &'a [VaultEntry], reference: &str) -> Option<&'a VaultEntry> {
    // Pass 1: exact filename stem
    let stem = filename_stem(reference);
    if let Some(entry) = entries.iter().find(|e| filename_stem(&e.filename) == stem) {
        return Some(entry);
    }

    // Pass 2: alias match (case-insensitive)
    let ref_lower = reference.to_lowercase();
    if let Some(entry) = entries
        .iter()
        .find(|e| e.aliases.iter().any(|a| a.to_lowercase() == ref_lower))
    {
        return Some(entry);
    }

    // Pass 3: exact title match (case-insensitive)
    if let Some(entry) = entries
        .iter()
        .find(|e| e.title.to_lowercase() == ref_lower)
    {
        return Some(entry);
    }

    // Pass 4: humanized title match — convert kebab-case reference to words
    let humanized = humanize(reference);
    if let Some(entry) = entries
        .iter()
        .find(|e| e.title.to_lowercase() == humanized)
    {
        return Some(entry);
    }

    // Pass 5: last segment of path-style reference
    if let Some(segment) = reference.rsplit('/').next() {
        if segment != reference {
            return resolve_note(entries, segment);
        }
    }

    None
}

/// Extract the filename stem (without extension).
fn filename_stem(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, _)) => stem.to_lowercase(),
        None => name.to_lowercase(),
    }
}

/// Convert a kebab-case string to lowercase space-separated words.
fn humanize(s: &str) -> String {
    s.replace('-', " ").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_entry(filename: &str, title: &str, aliases: &[&str]) -> VaultEntry {
        VaultEntry {
            filename: filename.to_string(),
            title: title.to_string(),
            aliases: aliases.iter().map(|a| a.to_string()).collect(),
            ..VaultEntry::default()
        }
    }

    #[test]
    fn resolve_by_filename_stem() {
        let entries = vec![make_entry("my-project.md", "My Project", &[])];
        let result = resolve_note(&entries, "my-project");
        assert_eq!(result.unwrap().filename, "my-project.md");
    }

    #[test]
    fn resolve_by_filename_stem_with_extension() {
        let entries = vec![make_entry("my-project.md", "My Project", &[])];
        let result = resolve_note(&entries, "my-project.md");
        assert_eq!(result.unwrap().filename, "my-project.md");
    }

    #[test]
    fn resolve_by_alias() {
        let entries = vec![make_entry("note.md", "A Note", &["shortcut"])];
        let result = resolve_note(&entries, "shortcut");
        assert_eq!(result.unwrap().filename, "note.md");
    }

    #[test]
    fn resolve_by_alias_case_insensitive() {
        let entries = vec![make_entry("note.md", "A Note", &["ShortCut"])];
        let result = resolve_note(&entries, "shortcut");
        assert_eq!(result.unwrap().filename, "note.md");
    }

    #[test]
    fn resolve_by_exact_title() {
        let entries = vec![make_entry("file.md", "My Great Note", &[])];
        let result = resolve_note(&entries, "My Great Note");
        assert_eq!(result.unwrap().filename, "file.md");
    }

    #[test]
    fn resolve_by_title_case_insensitive() {
        let entries = vec![make_entry("file.md", "My Great Note", &[])];
        let result = resolve_note(&entries, "my great note");
        assert_eq!(result.unwrap().filename, "file.md");
    }

    #[test]
    fn resolve_by_humanized_title() {
        let entries = vec![make_entry("file.md", "my great note", &[])];
        let result = resolve_note(&entries, "my-great-note");
        assert_eq!(result.unwrap().filename, "file.md");
    }

    #[test]
    fn resolve_by_last_path_segment() {
        let entries = vec![make_entry("alice.md", "Alice", &[])];
        let result = resolve_note(&entries, "person/alice");
        assert_eq!(result.unwrap().filename, "alice.md");
    }

    #[test]
    fn resolve_returns_none_for_no_match() {
        let entries = vec![make_entry("note.md", "A Note", &[])];
        let result = resolve_note(&entries, "nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn resolve_priority_filename_over_alias() {
        let entries = vec![
            make_entry("alpha.md", "Alpha Title", &[]),
            make_entry("beta.md", "Beta Title", &["alpha"]),
        ];
        // "alpha" matches filename stem of first entry (pass 1)
        // and alias of second entry (pass 2) — filename wins
        let result = resolve_note(&entries, "alpha");
        assert_eq!(result.unwrap().filename, "alpha.md");
    }

    #[test]
    fn resolve_priority_alias_over_title() {
        let entries = vec![
            make_entry("first.md", "Unrelated", &["target"]),
            make_entry("second.md", "target", &[]),
        ];
        // "target" doesn't match any filename stem (pass 1)
        // matches alias of first entry (pass 2) — alias wins over title
        let result = resolve_note(&entries, "target");
        assert_eq!(result.unwrap().filename, "first.md");
    }

    // ---------------------------------------------------------------
    // Feature: linux-console-app, Property 7: Note Resolution Multi-Pass
    // **Validates: Requirements 3.11**
    //
    // For any vault entry, resolving by filename stem, any alias, or
    // exact title should all return the same entry. Resolution should
    // be deterministic and consistent across all valid reference forms.
    // ---------------------------------------------------------------

    /// Strategy: a lowercase kebab-case stem that is a valid filename
    /// component and does NOT collide with the title or aliases.
    fn arb_stem() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9]{0,5}(-[a-z][a-z0-9]{0,5}){0,3}"
    }

    /// Strategy: a mixed-case title that is NOT a valid filename stem
    /// (contains at least one space) so it won't collide with pass 1.
    fn arb_title() -> impl Strategy<Value = String> {
        "[A-Z][a-z]{1,6}( [A-Z][a-z]{1,6}){1,3}"
    }

    /// Strategy: an alias that contains at least one uppercase letter
    /// so it won't collide with the lowercase filename stem in pass 1.
    fn arb_alias() -> impl Strategy<Value = String> {
        "[A-Z][a-zA-Z0-9]{1,8}"
    }

    /// Build a single VaultEntry with known stem, title, and aliases,
    /// plus a set of "other" entries that must NOT match any of those
    /// reference forms.
    fn arb_entry_with_others() -> impl Strategy<Value = (VaultEntry, Vec<String>, Vec<VaultEntry>)>
    {
        (
            arb_stem(),
            arb_title(),
            proptest::collection::vec(arb_alias(), 0..4),
        )
            .prop_map(|(stem, title, aliases)| {
                let filename = format!("{}.md", stem);
                let entry = VaultEntry {
                    filename: filename.clone(),
                    title: title.clone(),
                    aliases: aliases.clone(),
                    ..VaultEntry::default()
                };

                // Build a few "other" entries whose filenames, titles,
                // and aliases are guaranteed not to collide.
                let others: Vec<VaultEntry> = (0..3)
                    .map(|i| VaultEntry {
                        filename: format!("other-{}.md", i),
                        title: format!("Other Title {}", i),
                        aliases: vec![format!("OtherAlias{}", i)],
                        ..VaultEntry::default()
                    })
                    .collect();

                (entry, aliases, others)
            })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn note_resolution_multi_pass(
            (entry, aliases, others) in arb_entry_with_others(),
        ) {
            // Place the target entry among other entries
            let mut entries = others;
            entries.push(entry.clone());

            let expected_filename = &entry.filename;

            // --- Pass 1: resolve by filename stem ---
            let stem_ref = entry.filename.replace(".md", "");
            let by_stem = resolve_note(&entries, &stem_ref);
            prop_assert!(
                by_stem.is_some(),
                "resolve by filename stem failed for {:?}",
                expected_filename
            );
            prop_assert_eq!(
                &by_stem.unwrap().filename,
                expected_filename,
                "filename stem resolved to wrong entry"
            );

            // --- Pass 2: resolve by each alias ---
            for alias in &aliases {
                let by_alias = resolve_note(&entries, alias);
                prop_assert!(
                    by_alias.is_some(),
                    "resolve by alias {:?} failed",
                    alias
                );
                prop_assert_eq!(
                    &by_alias.unwrap().filename,
                    expected_filename,
                    "alias {:?} resolved to wrong entry",
                    alias
                );
            }

            // --- Pass 3: resolve by exact title ---
            let by_title = resolve_note(&entries, &entry.title);
            prop_assert!(
                by_title.is_some(),
                "resolve by title {:?} failed",
                entry.title
            );
            prop_assert_eq!(
                &by_title.unwrap().filename,
                expected_filename,
                "title resolved to wrong entry"
            );
        }
    }
}
