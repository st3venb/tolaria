# Requirements Document

## Introduction

This document specifies the requirements for converting Tolaria from a Tauri v2 + React desktop GUI application into a console/CLI application that runs on Linux. The core Rust backend — vault scanning/caching, frontmatter parsing, git operations, search, settings management, MCP server integration, and AI agent integration — is already cleanly separated from the Tauri command layer. The goal is to extract this domain logic into a standalone Rust binary with a terminal-based interface, preserving full vault compatibility so the same vault can be used interchangeably between the GUI and console versions.

## Glossary

- **CLI**: The new console application binary, compiled as a standalone Rust executable without Tauri dependencies
- **Core_Library**: The extracted Rust crate containing vault, git, frontmatter, search, settings, and MCP domain logic — shared between the GUI and CLI
- **Vault**: A directory of markdown files with YAML frontmatter that serves as the single source of truth for all data
- **VaultEntry**: The core data type representing a single parsed note (path, title, type, relationships, properties, etc.)
- **Frontmatter**: YAML metadata block between `---` delimiters at the top of a markdown file
- **Wikilink**: A `[[target]]` reference linking one note to another
- **MCP_Server**: The Model Context Protocol server (Node.js) that exposes vault tools for AI assistants
- **TUI**: Terminal User Interface — an interactive text-based interface rendered in the terminal
- **Note_Viewer**: The console component that renders markdown note content in the terminal
- **Cache**: The git-based incremental vault index stored at `~/.laputa/cache/<vault-hash>.json`

## Requirements

### Requirement 1: Extract Core Library from Tauri Shell

**User Story:** As a developer, I want the domain logic separated into a standalone Rust crate, so that both the GUI and CLI can share the same vault operations without code duplication.

#### Acceptance Criteria

1. THE Core_Library SHALL expose all vault operations (scan, parse, cache, reload) as public Rust functions without any Tauri dependency
2. THE Core_Library SHALL expose all git operations (commit, pull, push, status, history, diff, conflict resolution, clone) as public Rust functions without any Tauri dependency
3. THE Core_Library SHALL expose all frontmatter operations (parse, update, delete property) as public Rust functions without any Tauri dependency
4. THE Core_Library SHALL expose search operations (keyword search across vault files) as public Rust functions without any Tauri dependency
5. THE Core_Library SHALL expose settings management (read, write, normalize) as public Rust functions without any Tauri dependency
6. THE Core_Library SHALL expose MCP server management (spawn, register, remove, status check) as public Rust functions without any Tauri dependency
7. THE Core_Library SHALL expose AI agent operations (availability detection, streaming agent execution) as public Rust functions without any Tauri dependency
8. WHEN the Core_Library is compiled, THE Core_Library SHALL produce no compilation errors on Linux x86_64 and aarch64 targets
9. THE Core_Library SHALL preserve the existing `VaultEntry` data model, including all fields defined in `vault/entry.rs`
10. THE Core_Library SHALL preserve the vault boundary enforcement logic that prevents file operations outside the active vault

### Requirement 2: Build Linux-Compatible CLI Binary

**User Story:** As a Linux user, I want a standalone console binary I can run from my terminal, so that I can manage my Tolaria vault without a graphical desktop environment.

#### Acceptance Criteria

1. THE CLI SHALL compile into a single statically-linked binary for Linux x86_64
2. THE CLI SHALL compile into a single statically-linked binary for Linux aarch64
3. THE CLI SHALL accept a vault path as a command-line argument or read it from the existing vault list at `~/.config/com.tolaria.app/vaults.json`
4. WHEN no vault path is provided and no vault list exists, THE CLI SHALL display an error message with usage instructions
5. THE CLI SHALL use a subcommand-based interface (e.g., `tolaria list`, `tolaria search`, `tolaria git status`)
6. THE CLI SHALL exit with status code 0 on success and a non-zero status code on failure
7. THE CLI SHALL write normal output to stdout and error messages to stderr

### Requirement 3: Vault Browsing and Note Management

**User Story:** As a user, I want to browse, read, create, edit, and delete notes from the terminal, so that I can manage my knowledge vault without a GUI.

#### Acceptance Criteria

1. WHEN the `list` subcommand is invoked, THE CLI SHALL display all vault entries with title, type, status, and modified date
2. WHEN the `list` subcommand is invoked with a `--type` filter, THE CLI SHALL display only entries matching the specified type
3. WHEN the `list` subcommand is invoked with a `--status` filter, THE CLI SHALL display only entries matching the specified status
4. WHEN the `list` subcommand is invoked with a `--sort` option, THE CLI SHALL sort results by the specified field (title, modified, created, type)
5. WHEN the `show` subcommand is invoked with a note path or title, THE CLI SHALL display the note's frontmatter properties and markdown body
6. WHEN the `create` subcommand is invoked with a title, THE CLI SHALL create a new markdown file at the vault root with the title as an H1 heading
7. WHEN the `create` subcommand is invoked with a `--type` option, THE CLI SHALL include the specified type in the new note's frontmatter
8. WHEN the `edit` subcommand is invoked with a note path, THE CLI SHALL open the note in the user's `$EDITOR` (falling back to `vim`)
9. WHEN the `delete` subcommand is invoked with a note path, THE CLI SHALL prompt for confirmation before permanently deleting the file
10. WHEN the `delete` subcommand is invoked with the `--force` flag, THE CLI SHALL delete the file without prompting for confirmation
11. THE CLI SHALL resolve note references by filename stem, alias, or title — matching the same multi-pass resolution logic used by the GUI

### Requirement 4: Search

**User Story:** As a user, I want to search my vault from the terminal, so that I can quickly find notes by keyword.

#### Acceptance Criteria

1. WHEN the `search` subcommand is invoked with a query string, THE CLI SHALL return matching notes ranked by relevance score
2. WHEN search results are displayed, THE CLI SHALL show the note title, type, relevance score, and a contextual snippet for each match
3. WHEN the `search` subcommand is invoked with a `--limit` option, THE CLI SHALL return at most the specified number of results
4. THE CLI SHALL use the same keyword-based scoring algorithm as the GUI (title word match > title substring > content frequency)

### Requirement 5: Frontmatter Property Management

**User Story:** As a user, I want to read and modify note properties from the command line, so that I can update metadata without opening an editor.

#### Acceptance Criteria

1. WHEN the `prop get` subcommand is invoked with a note path and property key, THE CLI SHALL display the current value of that frontmatter property
2. WHEN the `prop set` subcommand is invoked with a note path, key, and value, THE CLI SHALL update the frontmatter property in the file on disk
3. WHEN the `prop delete` subcommand is invoked with a note path and key, THE CLI SHALL remove the frontmatter property from the file on disk
4. WHEN the `prop list` subcommand is invoked with a note path, THE CLI SHALL display all frontmatter properties as key-value pairs
5. THE CLI SHALL use the same line-by-line YAML editing logic from `frontmatter/ops.rs` to preserve formatting and comments in the frontmatter block
6. IF a property update targets a note outside the active vault boundary, THEN THE CLI SHALL reject the operation with an error message

### Requirement 6: Git Operations

**User Story:** As a user, I want to perform git operations on my vault from the terminal, so that I can commit, sync, and review changes without the GUI.

#### Acceptance Criteria

1. WHEN the `git status` subcommand is invoked, THE CLI SHALL display modified, added, deleted, and untracked markdown files in the vault
2. WHEN the `git diff` subcommand is invoked with a file path, THE CLI SHALL display the unified diff for that file
3. WHEN the `git commit` subcommand is invoked with a message, THE CLI SHALL stage all changes and create a commit (matching the GUI's `git add -A && git commit` behavior)
4. WHEN the `git pull` subcommand is invoked, THE CLI SHALL perform a `git pull --rebase` from the remote
5. WHEN the `git push` subcommand is invoked, THE CLI SHALL push commits to the remote
6. WHEN the `git log` subcommand is invoked, THE CLI SHALL display recent commits with hash, message, author, and date
7. WHEN the `git log` subcommand is invoked with a `--file` option, THE CLI SHALL display the commit history for that specific file
8. IF a merge conflict is detected after a pull, THEN THE CLI SHALL list conflicted files and offer resolution options (ours, theirs, manual edit)
9. WHEN the `git remote` subcommand is invoked, THE CLI SHALL display the current branch, remote URL, and ahead/behind counts

### Requirement 7: MCP Server Management

**User Story:** As a user, I want to start and manage the MCP server from the console, so that AI assistants can interact with my vault on a headless Linux system.

#### Acceptance Criteria

1. WHEN the `mcp start` subcommand is invoked, THE CLI SHALL spawn the MCP WebSocket bridge process with the active vault path
2. WHEN the `mcp stop` subcommand is invoked, THE CLI SHALL terminate the running MCP bridge process
3. WHEN the `mcp status` subcommand is invoked, THE CLI SHALL report whether the MCP server is running and which vault it is serving
4. WHEN the `mcp register` subcommand is invoked, THE CLI SHALL write the Tolaria MCP entry to Claude Code and Cursor configuration files
5. WHEN the `mcp unregister` subcommand is invoked, THE CLI SHALL remove the Tolaria MCP entry from Claude Code and Cursor configuration files
6. THE CLI SHALL locate the Node.js runtime using the same multi-path detection logic as the GUI (PATH, Homebrew, Volta, nvm, npm-global)

### Requirement 8: AI Agent Integration

**User Story:** As a user, I want to invoke AI agents (Claude Code, Codex) from the console, so that I can use AI-assisted vault operations on a headless Linux system.

#### Acceptance Criteria

1. WHEN the `ai status` subcommand is invoked, THE CLI SHALL report the availability and version of each supported AI agent (Claude Code, Codex)
2. WHEN the `ai chat` subcommand is invoked with a message, THE CLI SHALL stream the selected agent's response to stdout, including reasoning blocks and tool actions
3. WHEN the `ai chat` subcommand is invoked with a `--agent` option, THE CLI SHALL use the specified agent instead of the default
4. THE CLI SHALL detect AI agent binaries using the same multi-path search logic as the GUI (PATH, login shell, local installs, Mise/asdf shims, Homebrew, npm-global)
5. WHEN an AI agent executes file operations on the vault, THE CLI SHALL report which files were created or modified

### Requirement 9: Vault Initialization and Configuration

**User Story:** As a user, I want to create and configure vaults from the console, so that I can set up Tolaria on a new Linux system without a GUI.

#### Acceptance Criteria

1. WHEN the `init` subcommand is invoked with a directory path, THE CLI SHALL create a new empty vault with git initialization, default `.gitignore`, `AGENTS.md`, `CLAUDE.md`, and starter type scaffolding
2. WHEN the `clone` subcommand is invoked with a git URL and target path, THE CLI SHALL clone the repository and register it as a vault
3. WHEN the `vault list` subcommand is invoked, THE CLI SHALL display all registered vaults from `~/.config/com.tolaria.app/vaults.json`
4. WHEN the `vault switch` subcommand is invoked with a vault path, THE CLI SHALL set the specified vault as the active vault
5. WHEN the `config get` subcommand is invoked with a setting key, THE CLI SHALL display the current value from `~/.config/com.tolaria.app/settings.json`
6. WHEN the `config set` subcommand is invoked with a key and value, THE CLI SHALL update the setting and apply the same normalization rules as the GUI

### Requirement 10: Output Formatting

**User Story:** As a user, I want structured output options, so that I can use the CLI in scripts and pipelines.

#### Acceptance Criteria

1. WHEN the `--json` global flag is provided, THE CLI SHALL output results as JSON to stdout
2. WHEN the `--json` flag is not provided, THE CLI SHALL output results in a human-readable tabular format
3. WHEN stdout is not a TTY (piped), THE CLI SHALL suppress color codes and interactive prompts
4. WHEN stdout is a TTY, THE CLI SHALL use color to distinguish note types, statuses, and git change indicators
5. THE CLI SHALL support a `--quiet` flag that suppresses informational messages, outputting only the primary result

### Requirement 11: Vault Cache Compatibility

**User Story:** As a user, I want the CLI and GUI to share the same vault cache, so that switching between them does not trigger unnecessary full rescans.

#### Acceptance Criteria

1. THE CLI SHALL read and write the vault cache at the same location as the GUI (`~/.laputa/cache/<vault-hash>.json`)
2. THE CLI SHALL use the same cache version identifier as the GUI so that caches are interchangeable
3. THE CLI SHALL use the same three-strategy caching logic as the GUI (full scan, cache hit with uncommitted changes, incremental git diff update)
4. WHEN the CLI modifies vault files, THE CLI SHALL invalidate or update the cache entry for the affected files

### Requirement 12: Wikilink Operations

**User Story:** As a user, I want to inspect and manage note relationships from the terminal, so that I can navigate my knowledge graph without a GUI.

#### Acceptance Criteria

1. WHEN the `links` subcommand is invoked with a note path, THE CLI SHALL display all outgoing wikilinks from the note body
2. WHEN the `backlinks` subcommand is invoked with a note path, THE CLI SHALL display all notes that reference the specified note via wikilinks
3. WHEN the `relationships` subcommand is invoked with a note path, THE CLI SHALL display all frontmatter relationship fields (belongs_to, related_to, has, and custom relationship keys) with their resolved targets
4. WHEN a note is renamed via the CLI, THE CLI SHALL update all wikilinks across the vault that reference the old name

### Requirement 13: Interactive TUI Mode (Optional)

**User Story:** As a user, I want an optional interactive terminal UI mode, so that I can browse and navigate my vault with keyboard-driven navigation similar to the GUI experience.

#### Acceptance Criteria

1. WHEN the `tui` subcommand is invoked, THE CLI SHALL launch an interactive terminal interface with a note list panel and a note preview panel
2. WHILE the TUI is active, THE CLI SHALL allow keyboard navigation through the note list (arrow keys, j/k, search filtering)
3. WHILE the TUI is active and a note is selected, THE Note_Viewer SHALL render the note's markdown content with basic formatting (headings, bold, italic, lists, code blocks)
4. WHILE the TUI is active, THE CLI SHALL support opening the selected note in `$EDITOR` via a keyboard shortcut
5. WHERE the TUI mode is enabled, THE CLI SHALL display the same type-based grouping and sorting as the GUI sidebar
