use std::path::Path;

use tolaria_core::vault::{scan_vault_cached, VaultEntry};

use crate::output::OutputContext;

/// Apply optional type and status filters, then sort by the given field.
pub fn run(
    vault_path: &str,
    type_filter: Option<&str>,
    status_filter: Option<&str>,
    sort_field: &str,
    output: &OutputContext,
) {
    let entries = match scan_vault_cached(Path::new(vault_path)) {
        Ok(e) => e,
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    };

    let mut filtered = filter_entries(&entries, type_filter, status_filter);
    sort_entries(&mut filtered, sort_field);
    output.print_entries(&filtered);
}

/// Filter entries by optional type and status predicates.
pub fn filter_entries(
    entries: &[VaultEntry],
    type_filter: Option<&str>,
    status_filter: Option<&str>,
) -> Vec<VaultEntry> {
    entries
        .iter()
        .filter(|e| {
            if let Some(tf) = type_filter {
                let matches = e
                    .is_a
                    .as_deref()
                    .is_some_and(|t| t.eq_ignore_ascii_case(tf));
                if !matches {
                    return false;
                }
            }
            if let Some(sf) = status_filter {
                let matches = e
                    .status
                    .as_deref()
                    .is_some_and(|s| s.eq_ignore_ascii_case(sf));
                if !matches {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect()
}

/// Sort entries in-place by the specified field.
/// Supported fields: title, modified, created, type.
/// Defaults to modified (descending) for unknown fields.
pub fn sort_entries(entries: &mut [VaultEntry], field: &str) {
    match field {
        "title" => entries.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase())),
        "created" => entries.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
        "type" => entries.sort_by(|a, b| {
            let a_type = a.is_a.as_deref().unwrap_or("");
            let b_type = b.is_a.as_deref().unwrap_or("");
            a_type
                .to_lowercase()
                .cmp(&b_type.to_lowercase())
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        }),
        // "modified" or anything else — most-recently-modified first
        _ => entries.sort_by(|a, b| b.modified_at.cmp(&a.modified_at)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn entry(title: &str, is_a: Option<&str>, status: Option<&str>) -> VaultEntry {
        VaultEntry {
            title: title.to_string(),
            is_a: is_a.map(|s| s.to_string()),
            status: status.map(|s| s.to_string()),
            ..VaultEntry::default()
        }
    }

    #[test]
    fn filter_by_type() {
        let entries = vec![
            entry("A", Some("project"), None),
            entry("B", Some("note"), None),
            entry("C", Some("project"), None),
        ];
        let filtered = filter_entries(&entries, Some("project"), None);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|e| e.is_a.as_deref() == Some("project")));
    }

    #[test]
    fn filter_by_status() {
        let entries = vec![
            entry("A", None, Some("active")),
            entry("B", None, Some("done")),
            entry("C", None, Some("active")),
        ];
        let filtered = filter_entries(&entries, None, Some("active"));
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_by_type_and_status() {
        let entries = vec![
            entry("A", Some("project"), Some("active")),
            entry("B", Some("project"), Some("done")),
            entry("C", Some("note"), Some("active")),
        ];
        let filtered = filter_entries(&entries, Some("project"), Some("active"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "A");
    }

    #[test]
    fn filter_case_insensitive() {
        let entries = vec![entry("A", Some("Project"), Some("Active"))];
        let filtered = filter_entries(&entries, Some("project"), Some("active"));
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn filter_no_filters_returns_all() {
        let entries = vec![entry("A", None, None), entry("B", Some("x"), Some("y"))];
        let filtered = filter_entries(&entries, None, None);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn sort_by_title() {
        let mut entries = vec![entry("Zebra", None, None), entry("Alpha", None, None)];
        sort_entries(&mut entries, "title");
        assert_eq!(entries[0].title, "Alpha");
        assert_eq!(entries[1].title, "Zebra");
    }

    #[test]
    fn sort_by_type() {
        let mut entries = vec![
            entry("B", Some("note"), None),
            entry("A", Some("area"), None),
            entry("C", Some("note"), None),
        ];
        sort_entries(&mut entries, "type");
        assert_eq!(entries[0].is_a.as_deref(), Some("area"));
        // Within same type, sorted by title
        assert_eq!(entries[1].title, "B");
        assert_eq!(entries[2].title, "C");
    }

    #[test]
    fn sort_by_modified_descending() {
        let mut entries = vec![
            VaultEntry {
                title: "Old".into(),
                modified_at: Some(100),
                ..VaultEntry::default()
            },
            VaultEntry {
                title: "New".into(),
                modified_at: Some(200),
                ..VaultEntry::default()
            },
        ];
        sort_entries(&mut entries, "modified");
        assert_eq!(entries[0].title, "New");
    }

    #[test]
    fn sort_by_created_descending() {
        let mut entries = vec![
            VaultEntry {
                title: "Old".into(),
                created_at: Some(100),
                ..VaultEntry::default()
            },
            VaultEntry {
                title: "New".into(),
                created_at: Some(200),
                ..VaultEntry::default()
            },
        ];
        sort_entries(&mut entries, "created");
        assert_eq!(entries[0].title, "New");
    }

    // ── Arbitrary generators ────────────────────────────────────────

    fn arb_opt_type() -> impl Strategy<Value = Option<String>> {
        proptest::option::of(prop_oneof![
            Just("project".to_string()),
            Just("note".to_string()),
            Just("area".to_string()),
            Just("person".to_string()),
            Just("topic".to_string()),
        ])
    }

    fn arb_opt_status() -> impl Strategy<Value = Option<String>> {
        proptest::option::of(prop_oneof![
            Just("active".to_string()),
            Just("done".to_string()),
            Just("draft".to_string()),
            Just("paused".to_string()),
        ])
    }

    fn arb_vault_entry() -> impl Strategy<Value = VaultEntry> {
        (
            "[A-Za-z ]{1,20}",
            arb_opt_type(),
            arb_opt_status(),
            proptest::option::of(0u64..2_000_000_000u64),
            proptest::option::of(0u64..2_000_000_000u64),
        )
            .prop_map(|(title, is_a, status, modified_at, created_at)| VaultEntry {
                title,
                is_a,
                status,
                modified_at,
                created_at,
                ..VaultEntry::default()
            })
    }

    fn arb_entries() -> impl Strategy<Value = Vec<VaultEntry>> {
        proptest::collection::vec(arb_vault_entry(), 0..20)
    }

    // ── Property 4: List Filtering Correctness ──────────────────────
    // **Validates: Requirements 3.2, 3.3**
    //
    // For any set of entries and filter, filtered list contains exactly
    // matching entries — no matching entries excluded, no non-matching
    // entries included.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        #[test]
        fn prop_filter_type_correctness(
            entries in arb_entries(),
            filter_type in arb_opt_type(),
        ) {
            let filtered = filter_entries(&entries, filter_type.as_deref(), None);

            match filter_type.as_deref() {
                Some(tf) => {
                    // Every filtered entry must match the type
                    for e in &filtered {
                        prop_assert!(
                            e.is_a.as_deref().is_some_and(|t| t.eq_ignore_ascii_case(tf)),
                            "Filtered entry {:?} does not match type filter {:?}",
                            e.title, tf
                        );
                    }
                    // Every matching entry in the original must be in the filtered set
                    let expected_count = entries.iter().filter(|e| {
                        e.is_a.as_deref().is_some_and(|t| t.eq_ignore_ascii_case(tf))
                    }).count();
                    prop_assert_eq!(filtered.len(), expected_count);
                }
                None => {
                    prop_assert_eq!(filtered.len(), entries.len());
                }
            }
        }

        #[test]
        fn prop_filter_status_correctness(
            entries in arb_entries(),
            filter_status in arb_opt_status(),
        ) {
            let filtered = filter_entries(&entries, None, filter_status.as_deref());

            match filter_status.as_deref() {
                Some(sf) => {
                    for e in &filtered {
                        prop_assert!(
                            e.status.as_deref().is_some_and(|s| s.eq_ignore_ascii_case(sf)),
                            "Filtered entry {:?} does not match status filter {:?}",
                            e.title, sf
                        );
                    }
                    let expected_count = entries.iter().filter(|e| {
                        e.status.as_deref().is_some_and(|s| s.eq_ignore_ascii_case(sf))
                    }).count();
                    prop_assert_eq!(filtered.len(), expected_count);
                }
                None => {
                    prop_assert_eq!(filtered.len(), entries.len());
                }
            }
        }

        #[test]
        fn prop_filter_combined_correctness(
            entries in arb_entries(),
            filter_type in arb_opt_type(),
            filter_status in arb_opt_status(),
        ) {
            let filtered = filter_entries(
                &entries,
                filter_type.as_deref(),
                filter_status.as_deref(),
            );

            // Every filtered entry must match both predicates
            for e in &filtered {
                if let Some(tf) = filter_type.as_deref() {
                    prop_assert!(
                        e.is_a.as_deref().is_some_and(|t| t.eq_ignore_ascii_case(tf)),
                    );
                }
                if let Some(sf) = filter_status.as_deref() {
                    prop_assert!(
                        e.status.as_deref().is_some_and(|s| s.eq_ignore_ascii_case(sf)),
                    );
                }
            }

            // Count must match manual filter
            let expected_count = entries.iter().filter(|e| {
                let type_ok = match filter_type.as_deref() {
                    Some(tf) => e.is_a.as_deref().is_some_and(|t| t.eq_ignore_ascii_case(tf)),
                    None => true,
                };
                let status_ok = match filter_status.as_deref() {
                    Some(sf) => e.status.as_deref().is_some_and(|s| s.eq_ignore_ascii_case(sf)),
                    None => true,
                };
                type_ok && status_ok
            }).count();
            prop_assert_eq!(filtered.len(), expected_count);
        }
    }

    // ── Property 5: List Sorting Correctness ────────────────────────
    // **Validates: Requirements 3.4**
    //
    // For any set of entries and sort field, output is correctly ordered.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        #[test]
        fn prop_sort_title_ascending(entries in arb_entries()) {
            let mut sorted = entries.clone();
            sort_entries(&mut sorted, "title");
            for w in sorted.windows(2) {
                prop_assert!(
                    w[0].title.to_lowercase() <= w[1].title.to_lowercase(),
                    "Title sort violated: {:?} > {:?}",
                    w[0].title, w[1].title
                );
            }
        }

        #[test]
        fn prop_sort_modified_descending(entries in arb_entries()) {
            let mut sorted = entries.clone();
            sort_entries(&mut sorted, "modified");
            for w in sorted.windows(2) {
                prop_assert!(
                    w[0].modified_at >= w[1].modified_at,
                    "Modified sort violated: {:?} < {:?}",
                    w[0].modified_at, w[1].modified_at
                );
            }
        }

        #[test]
        fn prop_sort_created_descending(entries in arb_entries()) {
            let mut sorted = entries.clone();
            sort_entries(&mut sorted, "created");
            for w in sorted.windows(2) {
                prop_assert!(
                    w[0].created_at >= w[1].created_at,
                    "Created sort violated: {:?} < {:?}",
                    w[0].created_at, w[1].created_at
                );
            }
        }

        #[test]
        fn prop_sort_type_ascending(entries in arb_entries()) {
            let mut sorted = entries.clone();
            sort_entries(&mut sorted, "type");
            for w in sorted.windows(2) {
                let a = w[0].is_a.as_deref().unwrap_or("").to_lowercase();
                let b = w[1].is_a.as_deref().unwrap_or("").to_lowercase();
                prop_assert!(
                    a <= b || (a == b && w[0].title.to_lowercase() <= w[1].title.to_lowercase()),
                    "Type sort violated: ({:?}, {:?}) > ({:?}, {:?})",
                    a, w[0].title, b, w[1].title
                );
            }
        }

        #[test]
        fn prop_sort_preserves_length(
            entries in arb_entries(),
            field in prop_oneof![
                Just("title"),
                Just("modified"),
                Just("created"),
                Just("type"),
            ],
        ) {
            let mut sorted = entries.clone();
            sort_entries(&mut sorted, field);
            prop_assert_eq!(sorted.len(), entries.len());
        }
    }
}
