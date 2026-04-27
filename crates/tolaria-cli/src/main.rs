mod commands;
mod context;
pub mod output;
pub mod resolve;

use clap::{Parser, Subcommand};
use context::{resolve_vault_path, CliContext};
use output::{OutputContext, OutputFormat};

/// Tolaria vault management CLI
#[derive(Parser)]
#[command(name = "tolaria", version, about)]
struct Cli {
    /// Path to the vault directory
    #[arg(long, global = true)]
    vault: Option<String>,

    /// Output as JSON
    #[arg(long, global = true)]
    json: bool,

    /// Suppress informational messages
    #[arg(long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List vault entries
    List {
        /// Filter by note type
        #[arg(long)]
        r#type: Option<String>,
        /// Filter by status
        #[arg(long)]
        status: Option<String>,
        /// Sort field (title, modified, created, type)
        #[arg(long, default_value = "modified")]
        sort: String,
    },
    /// Show a note's content
    Show {
        /// Note reference (filename stem, alias, or title)
        note: String,
    },
    /// Create a new note
    Create {
        /// Note title
        title: String,
        /// Note type to set in frontmatter
        #[arg(long)]
        r#type: Option<String>,
    },
    /// Open a note in $EDITOR
    Edit {
        /// Note reference (filename stem, alias, or title)
        note: String,
    },
    /// Delete a note
    Delete {
        /// Note reference (filename stem, alias, or title)
        note: String,
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
    /// Search the vault
    Search {
        /// Search query string
        query: String,
        /// Maximum number of results
        #[arg(long, default_value = "20")]
        limit: usize,
    },
    /// Manage frontmatter properties
    Prop {
        #[command(subcommand)]
        action: PropAction,
    },
    /// Git operations
    Git {
        #[command(subcommand)]
        action: GitAction,
    },
    /// MCP server management
    Mcp {
        #[command(subcommand)]
        action: McpAction,
    },
    /// AI agent operations
    Ai {
        #[command(subcommand)]
        action: AiAction,
    },
    /// Initialize a new vault
    Init {
        /// Directory path for the new vault
        path: String,
    },
    /// Clone a vault from a git repository
    Clone {
        /// Git repository URL
        url: String,
        /// Local directory path
        path: String,
    },
    /// Vault management
    Vault {
        #[command(subcommand)]
        action: VaultAction,
    },
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Show outgoing wikilinks from a note
    Links {
        /// Note reference (filename stem, alias, or title)
        note: String,
    },
    /// Show notes that link to a note
    Backlinks {
        /// Note reference (filename stem, alias, or title)
        note: String,
    },
    /// Show frontmatter relationships for a note
    Relationships {
        /// Note reference (filename stem, alias, or title)
        note: String,
    },
    /// Launch interactive TUI mode
    Tui,
}

#[derive(Subcommand)]
enum PropAction {
    /// Get a frontmatter property value
    Get {
        /// Note reference
        note: String,
        /// Property key
        key: String,
    },
    /// Set a frontmatter property value
    Set {
        /// Note reference
        note: String,
        /// Property key
        key: String,
        /// Property value
        value: String,
    },
    /// Delete a frontmatter property
    Delete {
        /// Note reference
        note: String,
        /// Property key
        key: String,
    },
    /// List all frontmatter properties
    List {
        /// Note reference
        note: String,
    },
}

#[derive(Subcommand)]
enum GitAction {
    /// Show modified files in the vault
    Status,
    /// Show unified diff for a file
    Diff {
        /// File path relative to vault root
        file: String,
    },
    /// Stage all changes and commit
    Commit {
        /// Commit message
        message: String,
    },
    /// Pull changes from remote (rebase)
    Pull,
    /// Push commits to remote
    Push,
    /// Show commit history
    Log {
        /// Show history for a specific file
        #[arg(long)]
        file: Option<String>,
    },
    /// Show remote and branch info
    Remote,
    /// Resolve a merge conflict
    Resolve {
        /// Conflicted file path
        file: String,
        /// Resolution strategy (ours, theirs, edit)
        #[arg(long)]
        strategy: String,
    },
}

#[derive(Subcommand)]
enum McpAction {
    /// Start the MCP WebSocket bridge
    Start,
    /// Stop the MCP bridge process
    Stop,
    /// Check MCP server status
    Status,
    /// Register MCP in AI tool configs
    Register,
    /// Remove MCP from AI tool configs
    Unregister,
}

#[derive(Subcommand)]
enum AiAction {
    /// Check AI agent availability
    Status,
    /// Chat with an AI agent
    Chat {
        /// Message to send
        message: String,
        /// Agent to use (e.g., claude, codex)
        #[arg(long)]
        agent: Option<String>,
    },
}

#[derive(Subcommand)]
enum VaultAction {
    /// List registered vaults
    List,
    /// Switch the active vault
    Switch {
        /// Vault path to activate
        path: String,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Get a configuration value
    Get {
        /// Setting key
        key: String,
    },
    /// Set a configuration value
    Set {
        /// Setting key
        key: String,
        /// Setting value
        value: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let output = OutputContext {
        format: if cli.json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        },
        is_tty: atty::is(atty::Stream::Stdout),
        quiet: cli.quiet,
    };

    // Commands that don't require a resolved vault
    match &cli.command {
        Command::Init { ref path } => {
            commands::vault_mgmt::run_init(path, &output);
            return;
        }
        Command::Clone { ref url, ref path } => {
            commands::vault_mgmt::run_clone(url, path, &output);
            return;
        }
        Command::Vault { ref action } => match action {
            VaultAction::List => {
                commands::vault_mgmt::run_vault_list(&output);
                return;
            }
            VaultAction::Switch { path } => {
                commands::vault_mgmt::run_vault_switch(path, &output);
                return;
            }
        },
        Command::Config { ref action } => match action {
            ConfigAction::Get { key } => {
                commands::config::run_get(key, &output);
                return;
            }
            ConfigAction::Set { key, value } => {
                commands::config::run_set(key, value, &output);
                return;
            }
        },
        Command::Tui => {
            #[cfg(feature = "tui")]
            {
                let vault_path = match resolve_vault_path(cli.vault.as_deref()) {
                    Ok(p) => p,
                    Err(msg) => {
                        output.error(&msg);
                        std::process::exit(1);
                    }
                };
                commands::tui::run(&vault_path);
                return;
            }
            #[cfg(not(feature = "tui"))]
            {
                eprintln!(
                    "TUI mode is not available. Rebuild with the `tui` feature:\n\
                     \n  cargo install tolaria-cli --features tui\n\
                     \n  cargo build -p tolaria-cli --features tui"
                );
                std::process::exit(1);
            }
        }
        _ => {}
    }

    // Resolve vault path for commands that need it
    let vault_path = match resolve_vault_path(cli.vault.as_deref()) {
        Ok(p) => p,
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    };

    let _ctx = CliContext {
        vault_path: vault_path.clone(),
        output,
    };

    match &cli.command {
        Command::List {
            ref r#type,
            ref status,
            ref sort,
        } => {
            commands::list::run(
                &vault_path,
                r#type.as_deref(),
                status.as_deref(),
                sort,
                &_ctx.output,
            );
        }
        Command::Show { ref note } => {
            commands::show::run(&vault_path, note, &_ctx.output);
        }
        Command::Create {
            ref title,
            ref r#type,
        } => {
            commands::create::run(&vault_path, title, r#type.as_deref(), &_ctx.output);
        }
        Command::Edit { ref note } => {
            commands::edit::run(&vault_path, note, &_ctx.output);
        }
        Command::Delete { ref note, force } => {
            commands::delete::run(&vault_path, note, *force, &_ctx.output);
        }
        Command::Search { ref query, limit } => {
            commands::search::run(&vault_path, query, *limit, &_ctx.output);
        }
        Command::Prop { ref action } => match action {
            PropAction::Get { note, key } => {
                commands::prop::run_get(&vault_path, note, key, &_ctx.output);
            }
            PropAction::Set { note, key, value } => {
                commands::prop::run_set(&vault_path, note, key, value, &_ctx.output);
            }
            PropAction::Delete { note, key } => {
                commands::prop::run_delete(&vault_path, note, key, &_ctx.output);
            }
            PropAction::List { note } => {
                commands::prop::run_list(&vault_path, note, &_ctx.output);
            }
        },
        Command::Git { ref action } => match action {
            GitAction::Status => {
                commands::git::run_status(&vault_path, &_ctx.output);
            }
            GitAction::Diff { file } => {
                commands::git::run_diff(&vault_path, file, &_ctx.output);
            }
            GitAction::Commit { message } => {
                commands::git::run_commit(&vault_path, message, &_ctx.output);
            }
            GitAction::Pull => {
                commands::git::run_pull(&vault_path, &_ctx.output);
            }
            GitAction::Push => {
                commands::git::run_push(&vault_path, &_ctx.output);
            }
            GitAction::Log { file } => {
                commands::git::run_log(&vault_path, file.as_deref(), &_ctx.output);
            }
            GitAction::Remote => {
                commands::git::run_remote(&vault_path, &_ctx.output);
            }
            GitAction::Resolve { file, strategy } => {
                commands::git::run_resolve(&vault_path, file, strategy, &_ctx.output);
            }
        },
        Command::Mcp { ref action } => match action {
            McpAction::Start => commands::mcp::run_start(&vault_path, &_ctx.output),
            McpAction::Stop => commands::mcp::run_stop(&_ctx.output),
            McpAction::Status => commands::mcp::run_status(&vault_path, &_ctx.output),
            McpAction::Register => commands::mcp::run_register(&vault_path, &_ctx.output),
            McpAction::Unregister => commands::mcp::run_unregister(&_ctx.output),
        },
        Command::Ai { ref action } => match action {
            AiAction::Status => commands::ai::run_status(&_ctx.output),
            AiAction::Chat { message, agent } => {
                commands::ai::run_chat(
                    &vault_path,
                    message,
                    agent.as_deref(),
                    &_ctx.output,
                );
            }
        },
        Command::Links { ref note } => {
            commands::links::run_links(&vault_path, note, &_ctx.output);
        }
        Command::Backlinks { ref note } => {
            commands::links::run_backlinks(&vault_path, note, &_ctx.output);
        }
        Command::Relationships { ref note } => {
            commands::links::run_relationships(&vault_path, note, &_ctx.output);
        }
        // Already handled above
        Command::Init { .. }
        | Command::Clone { .. }
        | Command::Vault { .. }
        | Command::Config { .. }
        | Command::Tui => unreachable!(),
    }
}
