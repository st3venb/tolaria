use tolaria_core::vault_list;

use crate::output::OutputContext;

/// Resolved vault context for CLI operations.
pub struct CliContext {
    pub vault_path: String,
    pub output: OutputContext,
}

/// Resolve the active vault path using the priority chain:
/// 1. `--vault` CLI argument (highest priority)
/// 2. `active_vault` from `~/.config/com.tolaria.app/vaults.json`
/// 3. Error with usage instructions
pub fn resolve_vault_path(cli_vault: Option<&str>) -> Result<String, String> {
    if let Some(path) = cli_vault {
        return Ok(path.to_string());
    }

    match vault_list::load_vault_list() {
        Ok(list) => {
            if let Some(active) = list.active_vault {
                return Ok(active);
            }
        }
        Err(_) => {}
    }

    Err(
        "No vault specified. Use --vault <path> or set an active vault:\n\
         \n  tolaria --vault /path/to/vault <command>\n\
         \n  tolaria vault switch /path/to/vault"
            .to_string(),
    )
}

// Feature: linux-console-app, Property 3: Vault Path Resolution Priority
// **Validates: Requirements 2.3**
//
// For any combination of CLI --vault argument (present or absent) and
// active_vault in the vault list config (present or absent), the resolved
// vault path should follow the priority chain:
// CLI argument > config active_vault > error.
// When both are present, the CLI argument wins.
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Strategy for generating non-empty vault path strings.
    fn arb_vault_path() -> impl Strategy<Value = String> {
        "/[a-zA-Z0-9_/-]{1,40}".prop_filter("non-empty path", |s| !s.trim().is_empty())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        /// When a CLI vault argument is provided, it always wins —
        /// regardless of what the config file might contain.
        #[test]
        fn cli_arg_always_wins(cli_path in arb_vault_path()) {
            let result = resolve_vault_path(Some(&cli_path));
            prop_assert!(result.is_ok(), "Expected Ok when CLI arg is present");
            prop_assert_eq!(result.unwrap(), cli_path);
        }

        /// When a CLI vault argument is provided, the returned path is
        /// exactly the CLI argument — no normalization or mutation.
        #[test]
        fn cli_arg_returned_verbatim(cli_path in arb_vault_path()) {
            let result = resolve_vault_path(Some(&cli_path)).unwrap();
            prop_assert_eq!(
                result, cli_path,
                "resolve_vault_path must return the CLI arg verbatim"
            );
        }

        /// When CLI arg is None and no config file provides an active vault,
        /// the function must return an Err.
        /// Note: this test relies on the real config not having an active_vault
        /// set, which is true in CI / test environments. We use prop_assume
        /// to skip if a real config happens to exist.
        #[test]
        fn error_when_no_cli_arg_and_no_config(_dummy in 0..1u8) {
            let result = resolve_vault_path(None);
            // In a test/CI environment there is typically no vaults.json with
            // an active_vault. If one happens to exist, skip the assertion.
            if let Ok(list) = vault_list::load_vault_list() {
                prop_assume!(list.active_vault.is_none(),
                    "Skipping: real config has an active_vault set");
            }
            prop_assert!(result.is_err(),
                "Expected Err when no CLI arg and no active vault in config");
        }
    }
}
