# Requirements Document

## Introduction

Tolaria is a Tauri v2 + React + Rust desktop application currently shipping only on macOS (Apple Silicon). This feature adds Windows as a supported build target, producing unsigned `.exe` / `.msi` installers from the same codebase. The application is already ~95% cross-platform; the work focuses on the small set of macOS-specific code paths, CI/CD pipeline additions, and platform-aware resource resolution. Windows code signing is explicitly out of scope for this initial release.

## Glossary

- **Build_Pipeline**: The GitHub Actions CI/CD workflows that compile, test, and publish Tolaria releases
- **MCP_Server_Resolver**: The `mcp_server_dir()` function in `src-tauri/src/mcp.rs` that locates the bundled MCP server directory relative to the running executable
- **Node_Resolver**: The `find_node()` function in `src-tauri/src/mcp.rs` that discovers the `node` binary on the host system
- **CLI_Agent_Detector**: The functions `claude_binary_candidates()` in `claude_cli.rs` and `codex_binary_candidates()` in `ai_agents.rs` that probe filesystem paths for installed AI CLI agents
- **Cache_Subsystem**: The vault cache module in `src-tauri/src/vault/cache.rs` that accelerates vault scanning using git-based incremental updates
- **Title_Bar**: The Tauri window chrome configuration controlling the application title bar style and drag regions
- **Updater_Manifest**: The JSON file (`latest.json`) served via GitHub Pages that the Tauri updater plugin reads to check for new versions
- **Windows_Target**: The `x86_64-pc-windows-msvc` Rust compilation target for 64-bit Windows

## Requirements

### Requirement 1: MCP Server Path Resolution on Windows

**User Story:** As a Windows user, I want the bundled MCP server to be located correctly at runtime, so that AI agent integration works the same as on macOS.

#### Acceptance Criteria

1. WHEN Tolaria runs on Windows in release mode, THE MCP_Server_Resolver SHALL resolve the MCP server directory using the flat directory structure where resources sit alongside the executable (e.g., `<exe_dir>/mcp-server/`)
2. WHEN Tolaria runs on macOS in release mode, THE MCP_Server_Resolver SHALL continue to resolve the MCP server directory using the `.app` bundle structure (`Contents/Resources/mcp-server/`)
3. WHEN Tolaria runs in dev mode on either platform, THE MCP_Server_Resolver SHALL resolve the MCP server directory via `CARGO_MANIFEST_DIR` as it does today
4. IF the MCP server directory cannot be found at any candidate path, THEN THE MCP_Server_Resolver SHALL return a descriptive error including all paths that were checked

### Requirement 2: Node Binary Discovery on Windows

**User Story:** As a Windows user, I want Tolaria to find my installed Node.js runtime, so that the MCP WebSocket bridge can be spawned.

#### Acceptance Criteria

1. WHEN Tolaria runs on Windows, THE Node_Resolver SHALL use `where.exe node` instead of `which node` to locate the Node.js binary
2. WHEN `where.exe` does not find Node.js on Windows, THE Node_Resolver SHALL check common Windows installation paths including `%ProgramFiles%\nodejs\node.exe` and `%LOCALAPPDATA%\Volta\node.exe`
3. WHEN Tolaria runs on macOS, THE Node_Resolver SHALL continue to use `which node` and the existing Homebrew/nvm/Volta fallback paths
4. IF Node.js cannot be found on either platform, THEN THE Node_Resolver SHALL return a descriptive error message

### Requirement 3: CLI Agent Detection on Windows

**User Story:** As a Windows user, I want Tolaria to detect installed AI CLI agents (Claude Code, Codex), so that the AI Agent panel works on Windows.

#### Acceptance Criteria

1. WHEN Tolaria runs on Windows, THE CLI_Agent_Detector SHALL probe Windows-appropriate paths for Claude Code including `%LOCALAPPDATA%\Programs\claude\claude.exe`, `%APPDATA%\npm\claude.cmd`, and `%USERPROFILE%\.claude\local\claude.exe`
2. WHEN Tolaria runs on Windows, THE CLI_Agent_Detector SHALL probe Windows-appropriate paths for Codex including `%APPDATA%\npm\codex.cmd` and `%LOCALAPPDATA%\Programs\codex\codex.exe`
3. WHEN Tolaria runs on macOS, THE CLI_Agent_Detector SHALL continue to use the existing Unix-style candidate paths
4. THE CLI_Agent_Detector SHALL append `.exe` or `.cmd` extensions to candidate paths on Windows

### Requirement 4: CI Pipeline — Windows Test Job

**User Story:** As a developer, I want the CI pipeline to run tests on Windows, so that regressions on the Windows platform are caught before merge.

#### Acceptance Criteria

1. THE Build_Pipeline SHALL include a `windows-latest` runner job in the CI workflow (`ci.yml`) that runs frontend tests (`pnpm test`), TypeScript type checking, and Rust tests (`cargo test`)
2. THE Build_Pipeline SHALL cache Rust dependencies on the Windows runner using a platform-specific cache key
3. WHEN a CI run completes on the Windows runner, THE Build_Pipeline SHALL report test results with the same pass/fail semantics as the existing macOS runner
4. THE Build_Pipeline SHALL continue to run the existing macOS test job unchanged

### Requirement 5: Release Pipeline — Windows Build Job

**User Story:** As a developer, I want the release pipeline to produce Windows installers, so that Windows users can download and install Tolaria.

#### Acceptance Criteria

1. THE Build_Pipeline SHALL include a `windows-latest` runner job in the alpha release workflow (`release.yml`) that builds Tolaria with `--target x86_64-pc-windows-msvc`
2. THE Build_Pipeline SHALL produce `.msi` and/or `.exe` installer artifacts from the Windows build job
3. THE Build_Pipeline SHALL upload Windows installer artifacts to the GitHub Release alongside the existing macOS artifacts
4. THE Build_Pipeline SHALL skip Windows code signing (no Authenticode certificate) for this initial release
5. THE Build_Pipeline SHALL include the Windows platform entry in the Updater_Manifest (`alpha-latest.json`) with the correct download URL and updater signature

### Requirement 6: Release Pipeline — Windows Stable Build

**User Story:** As a developer, I want the stable release pipeline to also produce Windows builds, so that stable releases are available on both platforms.

#### Acceptance Criteria

1. THE Build_Pipeline SHALL include a `windows-latest` runner job in the stable release workflow (`release-stable.yml`) that builds Tolaria with `--target x86_64-pc-windows-msvc`
2. THE Build_Pipeline SHALL produce `.msi` and/or `.exe` installer artifacts from the stable Windows build job
3. THE Build_Pipeline SHALL upload Windows stable artifacts to the GitHub Release alongside the existing macOS artifacts
4. THE Build_Pipeline SHALL include the Windows platform entry in the stable Updater_Manifest (`stable-latest.json`)
5. THE Build_Pipeline SHALL skip Windows code signing for this initial release

### Requirement 7: Tauri Window Configuration for Windows

**User Story:** As a Windows user, I want the application window to look and behave correctly on Windows, so that the app feels native.

#### Acceptance Criteria

1. THE Title_Bar SHALL render correctly on Windows, where `titleBarStyle: "Overlay"` is supported by Tauri v2 on Windows
2. THE Title_Bar SHALL support window dragging via the existing `data-tauri-drag-region` attributes on Windows
3. WHEN Tolaria runs on Windows, THE Title_Bar SHALL display window control buttons (minimize, maximize, close) in the platform-native position (top-right)

### Requirement 8: Cache Subsystem Cross-Platform Compatibility

**User Story:** As a Windows user, I want the vault cache to work correctly on Windows, so that vault loading is fast.

#### Acceptance Criteria

1. THE Cache_Subsystem SHALL store cache files under the platform-appropriate config directory on Windows (via the `dirs` crate, which resolves to `%LOCALAPPDATA%` or equivalent)
2. THE Cache_Subsystem SHALL use the existing `#[cfg(not(unix))]` no-op stub for `sync_parent_directory` on Windows, which is already implemented
3. THE Cache_Subsystem SHALL handle Windows-style path separators (`\`) when computing relative paths from git output

### Requirement 9: Git Integration on Windows

**User Story:** As a Windows user, I want git operations (commit, sync, clone) to work correctly, so that vault synchronization functions on Windows.

#### Acceptance Criteria

1. WHEN Tolaria runs on Windows, THE Cache_Subsystem SHALL invoke `git.exe` (found via PATH) for all git operations
2. THE Cache_Subsystem SHALL handle Windows line endings (CRLF) in git command output without breaking path parsing
3. WHEN a vault is opened on Windows, THE Cache_Subsystem SHALL correctly normalize path separators when comparing git diff output against cached entry paths

### Requirement 10: Application Settings Path on Windows

**User Story:** As a Windows user, I want my application settings to be stored in the standard Windows location, so that settings persist correctly.

#### Acceptance Criteria

1. WHEN Tolaria runs on Windows, THE settings module SHALL store settings under the platform-appropriate config directory (the `dirs` crate resolves `config_dir()` to `%APPDATA%` on Windows)
2. WHEN Tolaria runs on Windows, THE vault list module SHALL store `vaults.json` under the same platform-appropriate config directory
3. THE settings module SHALL use forward slashes or platform-native separators consistently when storing vault paths in configuration files
