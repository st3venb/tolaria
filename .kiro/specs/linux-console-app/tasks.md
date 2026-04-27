# Implementation Plan: Linux Console App

## Overview

Convert Tolaria from a single Tauri crate into a Cargo workspace with three members: `tolaria-core` (shared domain library), `tolaria-cli` (standalone Linux binary), and the existing `tolaria` Tauri app. The implementation proceeds incrementally: workspace scaffolding → core extraction → Tauri rewiring → CLI binary build-out → output formatting → testing → CI pipeline.

## Tasks

- [x] 1. Set up Cargo workspace and crate scaffolding
  - [x] 1.1 Create workspace-level `Cargo.toml` at the repo root with `members = ["crates/tolaria-core", "crates/tolaria-cli", "src-tauri"]`
    - Move shared dependencies (`serde`, `serde_json`, `serde_yaml`, `chrono`, `walkdir`, `gray_matter`, `regex`, `uuid`, `tokio`, `base64`, `dirs`, `tempfile`, `log`) into `[workspace.dependencies]`
    - Update `src-tauri/Cargo.toml` to use `workspace = true` references for shared deps
    - _Requirements: 1.1, 1.8_

  - [x] 1.2 Create `crates/tolaria-core/Cargo.toml` and `crates/tolaria-core/src/lib.rs`
    - Declare the core crate with shared workspace dependencies, no Tauri dependency
    - `lib.rs` should declare all public modules: `vault`, `frontmatter`, `git`, `search`, `settings`, `vault_list`, `mcp`, `ai_agents`, `claude_cli`, `boundary`
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7_

  - [x] 1.3 Create `crates/tolaria-cli/Cargo.toml` and `crates/tolaria-cli/src/main.rs`
    - Depend on `tolaria-core`, `clap` (with derive feature), `comfy-table`, `termcolor`, `atty`
    - Add `ratatui` and `crossterm` as optional dependencies behind a `tui` feature flag
    - Stub `main.rs` with a minimal clap `Cli` struct that parses `--vault`, `--json`, `--quiet` and prints help
    - _Requirements: 2.1, 2.2, 2.5_

- [x] 2. Extract domain modules into tolaria-core
  - [x] 2.1 Move vault modules to tolaria-core
    - Move `src-tauri/src/vault/` → `crates/tolaria-core/src/vault/` (all files: `mod.rs`, `entry.rs`, `cache.rs`, `file.rs`, `folders.rs`, `frontmatter.rs`, `parsing.rs`, `rename.rs`, `rename_transaction.rs`, `title_sync.rs`, `trash.rs`, `views.rs`, `image.rs`, `migration.rs`, `config_seed.rs`, `getting_started.rs`, and all test files)
    - Update internal `crate::` references to `crate::` within tolaria-core
    - Ensure `VaultEntry` and all vault types are `pub` and derive `Serialize`, `Deserialize`
    - _Requirements: 1.1, 1.9_

  - [x] 2.2 Move frontmatter modules to tolaria-core
    - Move `src-tauri/src/frontmatter/` → `crates/tolaria-core/src/frontmatter/` (all files: `mod.rs`, `ops.rs`, `yaml.rs`, `ops_update_tests.rs`)
    - _Requirements: 1.3_

  - [x] 2.3 Move git modules to tolaria-core
    - Move `src-tauri/src/git/` → `crates/tolaria-core/src/git/` (all files: `mod.rs`, `commit.rs`, `clone.rs`, `conflict.rs`, `connect.rs`, `dates.rs`, `history.rs`, `pulse.rs`, `remote.rs`, `status.rs`)
    - _Requirements: 1.2_

  - [x] 2.4 Move standalone domain files to tolaria-core
    - Move `src-tauri/src/search.rs` → `crates/tolaria-core/src/search.rs`
    - Move `src-tauri/src/settings.rs` → `crates/tolaria-core/src/settings.rs`
    - Move `src-tauri/src/vault_list.rs` → `crates/tolaria-core/src/vault_list.rs`
    - Move `src-tauri/src/mcp.rs` → `crates/tolaria-core/src/mcp.rs`
    - Move `src-tauri/src/ai_agents.rs` → `crates/tolaria-core/src/ai_agents.rs`
    - Move `src-tauri/src/claude_cli.rs` → `crates/tolaria-core/src/claude_cli.rs`
    - Update all `crate::` references to resolve within tolaria-core
    - _Requirements: 1.4, 1.5, 1.6, 1.7_

  - [x] 2.5 Extract boundary validation into tolaria-core
    - Move path validation logic from `src-tauri/src/commands/vault/boundary.rs` → `crates/tolaria-core/src/boundary.rs`
    - Extract `validate_path_within_vault` and `validate_relative_child_path` as public functions
    - Leave Tauri-specific `from_request` / state extraction in the Tauri crate's boundary module
    - _Requirements: 1.10_

  - [x] 2.6 Write property test for VaultEntry serialization round-trip
    - **Property 1: VaultEntry Serialization Round-Trip**
    - Generate arbitrary `VaultEntry` instances, serialize to JSON, deserialize back, assert equality
    - Add to `crates/tolaria-core/src/vault/entry.rs` or a dedicated `tests/` file
    - **Validates: Requirements 1.9**

  - [x] 2.7 Write property test for vault boundary enforcement
    - **Property 2: Vault Boundary Enforcement**
    - Generate arbitrary vault root + candidate paths, verify accept iff canonical descendant
    - Test `..` traversals, symlinks, absolute paths outside root are rejected
    - **Validates: Requirements 1.10, 5.6**

- [x] 3. Rewire Tauri crate to depend on tolaria-core
  - [x] 3.1 Update `src-tauri/Cargo.toml` to depend on `tolaria-core`
    - Add `tolaria-core = { path = "../crates/tolaria-core" }` to dependencies
    - Remove direct dependencies that are now re-exported from tolaria-core (e.g., `gray_matter`, `walkdir`)
    - _Requirements: 1.1_

  - [x] 3.2 Rewrite Tauri `commands/` to import from `tolaria_core`
    - Update all `commands/*.rs` and `commands/vault/*.rs` to use `tolaria_core::vault`, `tolaria_core::frontmatter`, `tolaria_core::git`, etc.
    - Replace `crate::vault::*` with `tolaria_core::vault::*` in command files
    - Keep Tauri-specific wrappers (state extraction, `#[tauri::command]` attributes) in place
    - _Requirements: 1.1_

  - [x] 3.3 Update `src-tauri/src/lib.rs` to import from tolaria-core
    - Replace `mod vault;`, `mod frontmatter;`, `mod git;`, etc. with `use tolaria_core::*` where appropriate
    - Keep Tauri-only modules (`menu.rs`, `app_updater.rs`, `telemetry.rs`) as local `mod` declarations
    - _Requirements: 1.1_

  - [x] 3.4 Verify Tauri app compiles and existing tests pass
    - Run `cargo build -p tolaria` and `cargo test -p tolaria` from workspace root
    - Fix any remaining import or visibility issues
    - _Requirements: 1.8_

- [x] 4. Checkpoint — Workspace compiles, Tauri app works
  - Ensure `cargo build --workspace` and `cargo test --workspace` pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 5. Implement CLI argument parsing and vault resolution
  - [x] 5.1 Implement full clap argument structure in `tolaria-cli`
    - Define `Cli`, `Command`, `PropAction`, `GitAction`, `McpAction`, `AiAction`, `VaultAction`, `ConfigAction` enums as specified in the design
    - Include all subcommands: `list`, `show`, `create`, `edit`, `delete`, `search`, `prop`, `git`, `mcp`, `ai`, `init`, `clone`, `vault`, `config`, `links`, `backlinks`, `relationships`, `tui`
    - Include global flags: `--vault`, `--json`, `--quiet`
    - _Requirements: 2.5, 3.1–3.11, 4.1–4.3, 5.1–5.5, 6.1–6.9, 7.1–7.6, 8.1–8.5, 9.1–9.6, 12.1–12.4, 13.1_

  - [x] 5.2 Implement vault path resolution logic
    - Priority chain: `--vault` CLI arg → `active_vault` from `~/.config/com.tolaria.app/vaults.json` → error with usage instructions
    - Create `CliContext` struct holding resolved `vault_path` and `OutputContext`
    - _Requirements: 2.3, 2.4_

  - [x] 5.3 Write property test for vault path resolution priority
    - **Property 3: Vault Path Resolution Priority**
    - Generate combinations of CLI arg present/absent and config active_vault present/absent
    - Verify CLI arg wins when both present, config used when CLI absent, error when neither
    - **Validates: Requirements 2.3**

  - [x] 5.4 Implement note resolution logic
    - Implement `resolve_note` function with multi-pass resolution: filename stem → alias → exact title → humanized title → last path segment
    - Place in `crates/tolaria-cli/src/resolve.rs` or similar
    - _Requirements: 3.11_

  - [x] 5.5 Write property test for note resolution multi-pass
    - **Property 7: Note Resolution Multi-Pass**
    - For any vault entry, resolving by filename stem, any alias, or exact title should all return the same entry
    - **Validates: Requirements 3.11**

- [x] 6. Implement output formatting layer
  - [x] 6.1 Implement `OutputContext` and `OutputFormat` in `crates/tolaria-cli/src/output.rs`
    - `OutputFormat::Human` and `OutputFormat::Json` enum
    - `OutputContext` with `format`, `is_tty` (via `atty`), `quiet` fields
    - `print_entries` — tabular list via `comfy-table` (human) or JSON array (json)
    - `print_entry_detail` — note detail view with frontmatter + body
    - `print_search_results` — search hits with score and snippet
    - `print_modified_files` — git status display
    - `info` — suppressed in quiet mode
    - `error` — always to stderr
    - TTY-aware coloring via `termcolor`; no ANSI codes when piped
    - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5_

  - [x] 6.2 Write property test for JSON output validity
    - **Property 12: JSON Output Validity**
    - For any domain object, formatting with `OutputFormat::Json` produces valid parseable JSON
    - **Validates: Requirements 10.1**

  - [x] 6.3 Write property test for non-TTY output has no ANSI codes
    - **Property 13: Non-TTY Output Has No ANSI Codes**
    - For any output produced when `is_tty = false`, verify no ANSI escape sequences present
    - **Validates: Requirements 10.3**

  - [x] 6.4 Write property test for quiet mode suppresses info messages
    - **Property 14: Quiet Mode Suppresses Informational Messages**
    - For any operation with `quiet = true`, stdout contains only primary result data
    - **Validates: Requirements 10.5**

  - [x] 6.5 Write property test for output completeness
    - **Property 11: Output Completeness**
    - For any domain object, human-readable format includes all required display fields (title, type, status, modified date for entries; hash, message, author, date for commits; etc.)
    - **Validates: Requirements 3.1, 4.2, 6.1, 6.6, 6.9, 9.3**

- [x] 7. Implement core CLI commands — vault browsing and note management
  - [x] 7.1 Implement `list` command
    - Call `tolaria_core::vault::scan_vault_cached`, apply `--type` and `--status` filters, sort by `--sort` field
    - Output via `OutputContext::print_entries`
    - _Requirements: 3.1, 3.2, 3.3, 3.4_

  - [x] 7.2 Write property test for list filtering correctness
    - **Property 4: List Filtering Correctness**
    - For any set of entries and filter, filtered list contains exactly matching entries
    - **Validates: Requirements 3.2, 3.3**

  - [x] 7.3 Write property test for list sorting correctness
    - **Property 5: List Sorting Correctness**
    - For any set of entries and sort field, output is correctly ordered
    - **Validates: Requirements 3.4**

  - [x] 7.4 Implement `show` command
    - Resolve note reference, read file content, display frontmatter + body via `OutputContext::print_entry_detail`
    - _Requirements: 3.5_

  - [x] 7.5 Implement `create` command
    - Create markdown file at vault root with title as H1, optional `--type` in frontmatter
    - Use `tolaria_core::frontmatter` for YAML generation
    - _Requirements: 3.6, 3.7_

  - [x] 7.6 Write property test for note creation produces valid file
    - **Property 6: Note Creation Produces Valid File**
    - For any valid title and optional type, created file has H1 heading and correct frontmatter
    - **Validates: Requirements 3.6, 3.7**

  - [x] 7.7 Implement `edit` command
    - Resolve note, open in `$EDITOR` (fallback to `vim`), wait for editor to exit
    - _Requirements: 3.8_

  - [x] 7.8 Implement `delete` command
    - Resolve note, prompt for confirmation (unless `--force`), delete file
    - Suppress prompt when stdin is not a TTY or `--force` is set
    - _Requirements: 3.9, 3.10_

- [x] 8. Implement search command
  - [x] 8.1 Implement `search` command
    - Call `tolaria_core::search_vault` with query, mode, and limit
    - Output via `OutputContext::print_search_results`
    - _Requirements: 4.1, 4.2, 4.3, 4.4_

  - [x] 8.2 Write property test for search results ordering and limit
    - **Property 8: Search Results Ordering and Limit**
    - For any query and limit, results are sorted by score descending and count ≤ limit
    - **Validates: Requirements 4.1, 4.3**

- [x] 9. Implement frontmatter property commands
  - [x] 9.1 Implement `prop get`, `prop set`, `prop delete`, `prop list` subcommands
    - `prop get <note> <key>` — read and display single property value
    - `prop set <note> <key> <value>` — update property via `tolaria_core::frontmatter::update_frontmatter`
    - `prop delete <note> <key>` — remove property via `tolaria_core::frontmatter::delete_frontmatter_property`
    - `prop list <note>` — display all frontmatter key-value pairs
    - Enforce vault boundary check before any write operation
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6_

  - [x] 9.2 Write property test for frontmatter set/get round-trip
    - **Property 9: Frontmatter Property Set/Get Round-Trip**
    - For any note and valid key-value pair, set then get returns the same value
    - **Validates: Requirements 5.1, 5.2**

  - [x] 9.3 Write property test for frontmatter property deletion
    - **Property 10: Frontmatter Property Deletion**
    - For any note with a property, deleting it makes it absent while preserving other properties
    - **Validates: Requirements 5.3**

- [x] 10. Implement git commands
  - [x] 10.1 Implement `git status`, `git diff`, `git commit`, `git pull`, `git push`, `git log`, `git remote` subcommands
    - `git status` — call `tolaria_core::git::get_modified_files`, display via output layer
    - `git diff <file>` — call `tolaria_core::git::get_file_diff`, print unified diff
    - `git commit <message>` — call `tolaria_core::git::git_commit`
    - `git pull` — call `tolaria_core::git::git_pull`, handle conflict detection
    - `git push` — call `tolaria_core::git::git_push`
    - `git log [--file <path>]` — call `tolaria_core::git::get_file_history`, display commits
    - `git remote` — call `tolaria_core::git::git_remote_status`, display branch/remote info
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7, 6.9_

  - [x] 10.2 Implement git conflict resolution flow
    - After pull detects conflicts, list conflicted files via `tolaria_core::git::get_conflict_files`
    - Offer resolution: `git resolve <file> --strategy ours|theirs|edit`
    - `edit` opens the file in `$EDITOR`
    - _Requirements: 6.8_

- [x] 11. Checkpoint — Core CLI commands work
  - Ensure `cargo build -p tolaria-cli` succeeds
  - Ensure all tests pass, ask the user if questions arise.

- [x] 12. Implement MCP, AI, vault management, and config commands
  - [x] 12.1 Implement `mcp start`, `mcp stop`, `mcp status`, `mcp register`, `mcp unregister` subcommands
    - `mcp start` — call `tolaria_core::spawn_ws_bridge`
    - `mcp stop` — terminate the bridge process
    - `mcp status` — call `tolaria_core::check_mcp_status`
    - `mcp register` / `mcp unregister` — call `tolaria_core::register_mcp` / `tolaria_core::remove_mcp`
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 7.6_

  - [x] 12.2 Implement `ai status` and `ai chat` subcommands
    - `ai status` — call `tolaria_core::get_ai_agents_status`, display availability
    - `ai chat <message> [--agent <name>]` — call `tolaria_core::run_ai_agent_stream`, stream output to stdout with reasoning blocks and tool actions
    - Report file operations performed by the agent
    - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5_

  - [x] 12.3 Implement `init`, `clone`, `vault list`, `vault switch` subcommands
    - `init <path>` — create vault dir, call `tolaria_core::git::init_repo`, seed default files
    - `clone <url> <path>` — call `tolaria_core::git::clone_repo`, register in vault list
    - `vault list` — call `tolaria_core::load_vault_list`, display registered vaults
    - `vault switch <path>` — update active_vault in vault list config
    - _Requirements: 9.1, 9.2, 9.3, 9.4_

  - [x] 12.4 Implement `config get` and `config set` subcommands
    - `config get <key>` — call `tolaria_core::get_settings`, display specific key
    - `config set <key> <value>` — call `tolaria_core::save_settings` with normalization
    - _Requirements: 9.5, 9.6_

- [x] 13. Implement wikilink commands
  - [x] 13.1 Implement `links`, `backlinks`, `relationships` subcommands
    - `links <note>` — scan note body for `[[wikilink]]` patterns, list all outgoing targets
    - `backlinks <note>` — scan all vault notes for `[[wikilink]]` references to the target note
    - `relationships <note>` — display frontmatter relationship fields (`belongs_to`, `related_to`, `has`, custom) with resolved targets
    - _Requirements: 12.1, 12.2, 12.3_

  - [x] 13.2 Ensure note rename updates wikilinks across vault
    - Wire CLI rename operations through `tolaria_core::vault::rename_note` which already handles cross-vault wikilink updates
    - _Requirements: 12.4_

  - [x] 13.3 Write property test for outgoing wikilinks completeness
    - **Property 15: Outgoing Wikilinks Completeness**
    - For any note with `[[wikilink]]` patterns, `links` output lists every target
    - **Validates: Requirements 12.1**

  - [x] 13.4 Write property test for backlinks completeness
    - **Property 16: Backlinks Completeness**
    - For any note, `backlinks` returns exactly the set of notes referencing it — no false positives, no missed references
    - **Validates: Requirements 12.2**

  - [x] 13.5 Write property test for wikilink update on rename
    - **Property 17: Wikilink Update on Rename**
    - Renaming a note updates all `[[wikilink]]` references to the old name, no other wikilinks modified
    - **Validates: Requirements 12.4**

- [x] 14. Implement error handling and exit codes
  - [x] 14.1 Implement consistent error handling across all commands
    - Human mode: colored `error:` prefix (red when TTY) + message to stderr
    - JSON mode: `{ "error": "message" }` to stdout + raw message to stderr
    - Exit code 1 for runtime errors, exit code 2 for invalid arguments (clap default)
    - Graceful degradation: corrupt cache → full scan, missing `$EDITOR` → helpful error, missing git/node → clear error for affected commands only
    - _Requirements: 2.6, 2.7_

- [x] 15. Implement vault cache compatibility
  - [x] 15.1 Ensure CLI uses the same cache path and version as the GUI
    - CLI reads/writes `~/.laputa/cache/<vault-hash>.json` using `tolaria_core::vault::scan_vault_cached`
    - Same cache version identifier (v13) and three-strategy logic (full scan, cache hit, incremental git diff)
    - Invalidate/update cache entries when CLI modifies vault files
    - _Requirements: 11.1, 11.2, 11.3, 11.4_

- [x] 16. Checkpoint — Full CLI feature set complete
  - Ensure `cargo test --workspace` passes
  - Ensure all tests pass, ask the user if questions arise.

- [x] 17. Implement optional TUI mode
  - [x] 17.1 Implement `tui` subcommand with ratatui
    - Two-panel layout: note list (left) + note preview (right)
    - Keyboard navigation: arrow keys, j/k, `/` for search filtering
    - Type-based grouping and sorting matching GUI sidebar
    - Open selected note in `$EDITOR` via keyboard shortcut (e.g., `e` or `Enter`)
    - Basic markdown rendering: headings, bold, italic, lists, code blocks
    - Gate behind `tui` feature flag in `Cargo.toml`
    - _Requirements: 13.1, 13.2, 13.3, 13.4, 13.5_

- [x] 18. Set up Linux CI pipeline for static builds
  - [x] 18.1 Create GitHub Actions workflow for Linux musl builds
    - Add workflow file for building `tolaria-cli` targeting `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`
    - Run `cargo test --workspace` in CI
    - Verify static linking with `file` command on the output binary
    - Upload release artifacts
    - _Requirements: 2.1, 2.2_

- [x] 19. Final checkpoint — All tests pass, static binaries build
  - Run `cargo test --workspace` — all unit, integration, and property tests pass
  - Run `cargo build --target x86_64-unknown-linux-musl -p tolaria-cli --release`
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate the 17 universal correctness properties from the design document
- The Tauri app must remain fully functional throughout — task 3.4 and checkpoint 4 verify this
- All domain logic lives in `tolaria-core`; the CLI and Tauri app are thin consumers
