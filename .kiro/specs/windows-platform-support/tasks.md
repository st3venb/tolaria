# Tasks: Windows Platform Support

## Task 1: MCP Server Path Resolution on Windows
- [x] 1.1 Update `mcp_server_dir()` in `src-tauri/src/mcp.rs` to add a Windows release candidate path (`<exe_dir>/mcp-server/`) alongside the existing macOS `.app` bundle path
- [x] 1.2 Update the error message to include all candidate paths (dev, macOS release, Windows release)
- [x] 1.3 Write unit tests for `mcp_server_dir()` covering dev mode, macOS release path, and Windows release path using temp directories with sentinel files

## Task 2: Node Binary Discovery on Windows
- [x] 2.1 Refactor `find_node()` in `src-tauri/src/mcp.rs` to use `where.exe node` on Windows and `which node` on macOS/Linux
- [x] 2.2 Add Windows-specific fallback paths to `fallback_node_path()`: `%ProgramFiles%\nodejs\node.exe`, `%LOCALAPPDATA%\Volta\node.exe`
- [x] 2.3 Write unit tests verifying the correct command dispatch (`which` vs `where.exe`) and Windows fallback path list

## Task 3: CLI Agent Detection on Windows
- [x] 3.1 Add Windows candidate paths to `claude_binary_candidates_for_home()` in `src-tauri/src/claude_cli.rs` with `.exe`/`.cmd` extensions, gated by `cfg!(target_os = "windows")`
- [x] 3.2 Add Windows candidate paths to `codex_binary_candidates()` in `src-tauri/src/ai_agents.rs` with `.exe`/`.cmd` extensions
- [x] 3.3 Update `find_claude_binary_on_path()` and `find_codex_binary_on_path()` to use `where.exe` on Windows instead of `which`
- [x] 3.4 Skip `find_claude_binary_in_user_shell()` on Windows (no zsh/bash login shell)
- [x] 3.5 Write unit tests verifying Windows candidate paths include proper extensions and macOS paths remain unchanged

## Task 4: Cache Subsystem Path Normalization
- [x] 4.1 Update `to_relative_path()` in `src-tauri/src/vault/cache.rs` to normalize backslashes to forward slashes before stripping the vault prefix
- [x] 4.2 Update `has_hidden_segment()` to split on both `/` and `\` separators
- [x] 4.3 Write unit tests for `to_relative_path()` with forward slashes, backslashes, and mixed separators
- [x] 4.4 Write unit tests for `collect_paths_from_diff()` and `collect_paths_from_porcelain()` with CRLF line endings
- [x] 4.5 [PBT] Write property test: for any vault path and file path with random separator styles, `to_relative_path()` produces consistent normalized output (Property 1)
- [x] 4.6 [PBT] Write property test: for any set of file paths, git output with CRLF endings parses identically to LF endings (Property 2)

## Task 5: Settings Path Verification
- [x] 5.1 Verify `app_config_dir()` in `src-tauri/src/settings.rs` uses `dirs::config_dir()` which resolves to `%APPDATA%` on Windows — no code change needed, add a unit test confirming the path is valid on the current platform
- [x] 5.2 [PBT] Write property test: for any valid vault path string, JSON serialization round-trip preserves the path (Property 3)

## Task 6: CI Pipeline — Windows Test Job
- [x] 6.1 Add a `test-windows` job to `.github/workflows/ci.yml` running on `windows-latest` with pnpm, Node.js 22, and Rust stable
- [x] 6.2 Add platform-specific Rust dependency caching with `Windows` in the cache key
- [x] 6.3 Run `pnpm test`, `tsc --noEmit`, and `cargo test` in the Windows job
- [x] 6.4 Verify the existing macOS test job remains unchanged

## Task 7: Alpha Release Pipeline — Windows Build
- [x] 7.1 Add a Windows matrix entry (`os: windows-latest`, `target: x86_64-pc-windows-msvc`) to the build job in `.github/workflows/release.yml`
- [x] 7.2 Conditionalize Apple certificate import and notarization steps to run only on macOS (`if: runner.os == 'macOS'`)
- [x] 7.3 Add Windows artifact upload step for `.msi`/`.nsis` installer files
- [x] 7.4 Update `alpha-latest.json` generation to include `windows-x86_64` platform entry with correct download URL and updater signature
- [x] 7.5 Add `TAURI_SIGNING_PRIVATE_KEY` env to the Windows build step for updater signature generation

## Task 8: Stable Release Pipeline — Windows Build
- [x] 8.1 Add a Windows matrix entry (`os: windows-latest`, `target: x86_64-pc-windows-msvc`) to the build job in `.github/workflows/release-stable.yml`
- [x] 8.2 Conditionalize Apple certificate import and notarization steps to run only on macOS
- [x] 8.3 Add Windows artifact upload step for `.msi`/`.nsis` installer files
- [x] 8.4 Update `stable-latest.json` generation to include `windows-x86_64` platform entry
- [x] 8.5 Update release notes generation to remove "Requires Apple Silicon" when Windows artifacts are present

## Task 9: Tauri Window Configuration Verification
- [x] 9.1 Verify `tauri.conf.json` window config (`titleBarStyle: "Overlay"`, `hiddenTitle: true`) works on Windows — no changes expected, document any needed adjustments
- [x] 9.2 Add `icon.ico` to the bundle icons list in `tauri.conf.json` if not already present (required for Windows)

## Task 10: Add `proptest` Dev Dependency
- [x] 10.1 Add `proptest` to `[dev-dependencies]` in `src-tauri/Cargo.toml` for property-based testing
