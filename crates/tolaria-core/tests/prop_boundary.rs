// Feature: linux-console-app, Property 2: Vault Boundary Enforcement
// **Validates: Requirements 1.10, 5.6**
//
// For any vault root path and any candidate file path, the boundary validator
// should accept the path if and only if the candidate's canonical path is a
// descendant of the vault root's canonical path. Paths containing `..`
// traversals, symlinks escaping the vault, or absolute paths outside the root
// should be rejected.

use proptest::prelude::*;
use std::path::Path;
use tolaria_core::boundary::{validate_path_within_vault, validate_relative_child_path};

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

/// Generate a clean relative path segment (no `.`, `..`, or separators).
fn arb_path_segment() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_-]{1,12}"
}

/// Generate a clean multi-segment relative path (1–4 segments joined by `/`).
fn arb_relative_path() -> impl Strategy<Value = String> {
    proptest::collection::vec(arb_path_segment(), 1..=4)
        .prop_map(|segs| segs.join("/"))
}

/// Generate a vault root as an absolute path (e.g. `/tmp/vault_abc`).
fn arb_vault_root() -> impl Strategy<Value = String> {
    arb_path_segment().prop_map(|seg| format!("/tmp/vault_{seg}"))
}

// ---------------------------------------------------------------------------
// Property tests for validate_path_within_vault
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// A candidate that is a child of the vault root must be accepted.
    #[test]
    fn accepts_descendant_paths(
        root in arb_vault_root(),
        child in arb_relative_path(),
    ) {
        let vault_root = Path::new(&root);
        let candidate = vault_root.join(&child);
        prop_assert!(
            validate_path_within_vault(vault_root, &candidate).is_ok(),
            "Expected descendant path to be accepted: root={root}, candidate={}",
            candidate.display()
        );
    }

    /// A candidate that is NOT a prefix-descendant of the vault root must be
    /// rejected. We construct this by using a completely different root.
    #[test]
    fn rejects_paths_outside_vault(
        root in arb_vault_root(),
        other_seg in arb_path_segment(),
        child in arb_relative_path(),
    ) {
        let vault_root = Path::new(&root);
        // Build a sibling directory that is guaranteed to differ from root.
        let outside = Path::new("/tmp").join(format!("other_{other_seg}")).join(&child);
        // Only assert rejection when the outside path truly isn't under root.
        prop_assume!(!outside.starts_with(vault_root));
        prop_assert!(
            validate_path_within_vault(vault_root, &outside).is_err(),
            "Expected path outside vault to be rejected: root={root}, candidate={}",
            outside.display()
        );
    }

    /// Paths that attempt `..` traversal out of the vault root must be
    /// rejected once the traversal escapes the root prefix.
    #[test]
    fn rejects_dotdot_traversal_escaping_root(
        root in arb_vault_root(),
        child in arb_path_segment(),
    ) {
        let vault_root = Path::new(&root);
        // e.g. /tmp/vault_abc/child/../../escape  →  /tmp/escape
        let _traversal = vault_root.join(&child).join("..").join("..").join("escape");
        // After logical resolution the path escapes the root.
        // validate_path_within_vault works on already-canonicalized paths,
        // so we simulate what canonicalization would produce.
        let mut resolved = vault_root.join(&child);
        // Go up twice: once back to root, once above root.
        if let Some(p) = resolved.parent() {
            resolved = p.to_path_buf();
        }
        if let Some(p) = resolved.parent() {
            resolved = p.to_path_buf();
        }
        let resolved = resolved.join("escape");
        // The resolved path should be outside the vault.
        if !resolved.starts_with(vault_root) {
            prop_assert!(
                validate_path_within_vault(vault_root, &resolved).is_err(),
                "Expected ..‐traversal to be rejected: root={root}, resolved={}",
                resolved.display()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Property tests for validate_relative_child_path
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Clean relative paths (no `.`, `..`, absolute prefix) must be accepted.
    #[test]
    fn accepts_clean_relative_paths(path in arb_relative_path()) {
        prop_assert!(
            validate_relative_child_path(&path).is_ok(),
            "Expected clean relative path to be accepted: {path}"
        );
    }

    /// Paths containing `..` components must be rejected.
    #[test]
    fn rejects_dotdot_components(
        prefix in arb_relative_path(),
        suffix in arb_relative_path(),
    ) {
        let bad = format!("{prefix}/../{suffix}");
        prop_assert!(
            validate_relative_child_path(&bad).is_err(),
            "Expected path with .. to be rejected: {bad}"
        );
    }

    /// Paths that are exactly `.` or start with `./` must be rejected,
    /// because `Path::components()` yields `CurDir` for a leading `.`.
    #[test]
    fn rejects_leading_dot_component(suffix in arb_relative_path()) {
        let bad = format!("./{suffix}");
        prop_assert!(
            validate_relative_child_path(&bad).is_err(),
            "Expected path starting with ./ to be rejected: {bad}"
        );
    }

    /// A bare `.` path must be rejected.
    #[test]
    fn rejects_bare_dot(_dummy in 0..1u8) {
        prop_assert!(
            validate_relative_child_path(".").is_err(),
            "Expected bare '.' to be rejected"
        );
    }

    /// Absolute paths (starting with `/`) must be rejected.
    #[test]
    fn rejects_absolute_paths(path in arb_relative_path()) {
        let abs = format!("/{path}");
        prop_assert!(
            validate_relative_child_path(&abs).is_err(),
            "Expected absolute path to be rejected: {abs}"
        );
    }

    /// Empty and whitespace-only strings must be rejected.
    #[test]
    fn rejects_empty_strings(spaces in " {0,5}") {
        prop_assert!(
            validate_relative_child_path(&spaces).is_err(),
            "Expected empty/whitespace string to be rejected"
        );
    }
}
