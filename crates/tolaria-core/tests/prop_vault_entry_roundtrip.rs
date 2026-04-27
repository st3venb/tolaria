// Feature: linux-console-app, Property 1: VaultEntry Serialization Round-Trip
// **Validates: Requirements 1.9**
//
// For any valid VaultEntry instance, serializing it to JSON and deserializing
// it back should produce an equivalent VaultEntry with all fields preserved.

use proptest::prelude::*;
use std::collections::HashMap;
use tolaria_core::vault::VaultEntry;

/// Strategy for generating simple JSON scalar values (string, number, bool).
fn arb_json_value() -> impl Strategy<Value = serde_json::Value> {
    prop_oneof![
        any::<bool>().prop_map(serde_json::Value::Bool),
        any::<i64>().prop_map(|n| serde_json::Value::Number(n.into())),
        "[a-zA-Z0-9_ ]{0,20}".prop_map(|s| serde_json::Value::String(s)),
    ]
}

fn arb_properties() -> impl Strategy<Value = HashMap<String, serde_json::Value>> {
    proptest::collection::hash_map("[a-z_]{1,12}", arb_json_value(), 0..5)
}

fn arb_relationships() -> impl Strategy<Value = HashMap<String, Vec<String>>> {
    proptest::collection::hash_map(
        "[A-Za-z]{1,10}",
        proptest::collection::vec("[a-z-]{1,15}", 0..4),
        0..4,
    )
}

fn arb_opt_string() -> impl Strategy<Value = Option<String>> {
    proptest::option::of("[a-zA-Z0-9_ -]{1,20}")
}

fn arb_string_vec() -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec("[a-zA-Z0-9_-]{1,15}", 0..5)
}

/// Strategy for generating arbitrary VaultEntry instances.
///
/// Splits fields into small groups (≤10 per tuple) to stay within proptest's
/// tuple Strategy impl limit, then assembles the final struct.
fn arb_vault_entry() -> impl Strategy<Value = VaultEntry> {
    // Group A: identity fields (8 elements)
    let group_a = (
        "[a-z/]{1,30}",                     // path
        "[a-z-]{1,20}\\.md",                // filename
        "[A-Za-z0-9 ]{1,25}",              // title
        arb_opt_string(),                    // is_a
        arb_string_vec(),                    // aliases
        arb_string_vec(),                    // belongs_to
        arb_string_vec(),                    // related_to
        arb_opt_string(),                    // status
    );

    // Group B: metadata fields (8 elements)
    let group_b = (
        any::<bool>(),                       // archived
        proptest::option::of(any::<u64>()),  // modified_at
        proptest::option::of(any::<u64>()),  // created_at
        any::<u64>(),                        // file_size
        "[a-zA-Z0-9 .]{0,50}",             // snippet
        arb_relationships(),                 // relationships
        arb_opt_string(),                    // icon
        arb_opt_string(),                    // color
    );

    // Group C: type config fields (8 elements)
    let group_c = (
        proptest::option::of(any::<i64>()),  // order
        arb_opt_string(),                    // sidebar_label
        arb_opt_string(),                    // template
        arb_opt_string(),                    // sort
        arb_opt_string(),                    // view
        proptest::option::of(any::<bool>()), // visible
        any::<bool>(),                       // organized
        any::<bool>(),                       // favorite
    );

    // Group D: remaining fields (6 elements)
    let group_d = (
        proptest::option::of(any::<i64>()),  // favorite_index
        any::<u32>(),                        // word_count
        arb_string_vec(),                    // outgoing_links
        arb_properties(),                    // properties
        arb_string_vec(),                    // list_properties_display
        any::<bool>(),                       // has_h1
    );

    (group_a, group_b, group_c, group_d).prop_map(
        |(a, b, c, d)| {
            VaultEntry {
                path: a.0,
                filename: a.1,
                title: a.2,
                is_a: a.3,
                aliases: a.4,
                belongs_to: a.5,
                related_to: a.6,
                status: a.7,
                archived: b.0,
                modified_at: b.1,
                created_at: b.2,
                file_size: b.3,
                snippet: b.4,
                relationships: b.5,
                icon: b.6,
                color: b.7,
                order: c.0,
                sidebar_label: c.1,
                template: c.2,
                sort: c.3,
                view: c.4,
                visible: c.5,
                organized: c.6,
                favorite: c.7,
                favorite_index: d.0,
                word_count: d.1,
                outgoing_links: d.2,
                properties: d.3,
                list_properties_display: d.4,
                has_h1: d.5,
                file_kind: "markdown".to_string(),
            }
        },
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn vault_entry_roundtrip(entry in arb_vault_entry()) {
        let json = serde_json::to_string(&entry).unwrap();
        let decoded: VaultEntry = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(entry, decoded);
    }
}
