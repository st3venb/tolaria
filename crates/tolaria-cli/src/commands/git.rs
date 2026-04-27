use std::io::Write;
use std::path::Path;
use std::process::Command;

use termcolor::{Color, ColorSpec, WriteColor};
use tolaria_core::git::{
    get_file_diff, get_file_history, get_modified_files, git_commit, git_pull, git_push,
    git_remote_status, git_resolve_conflict, GitCommit,
};

use crate::output::{OutputContext, OutputFormat};

// ── git status ──────────────────────────────────────────────────────

pub fn run_status(vault_path: &str, output: &OutputContext) {
    match get_modified_files(vault_path) {
        Ok(files) => output.print_modified_files(&files),
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    }
}

// ── git diff ────────────────────────────────────────────────────────

pub fn run_diff(vault_path: &str, file: &str, output: &OutputContext) {
    let full_path = Path::new(vault_path).join(file);
    let full_str = full_path.to_string_lossy().to_string();

    match get_file_diff(vault_path, &full_str) {
        Ok(diff) => {
            if diff.is_empty() {
                output.info("No changes.");
            } else {
                println!("{diff}");
            }
        }
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    }
}

// ── git commit ──────────────────────────────────────────────────────

pub fn run_commit(vault_path: &str, message: &str, output: &OutputContext) {
    match git_commit(vault_path, message) {
        Ok(result) => output.info(&result.trim_end().to_string()),
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    }
}

// ── git pull ────────────────────────────────────────────────────────

pub fn run_pull(vault_path: &str, output: &OutputContext) {
    let result = match git_pull(vault_path) {
        Ok(r) => r,
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    };

    match output.format {
        OutputFormat::Json => output.print_json_value(&result),
        OutputFormat::Human => {
            match result.status.as_str() {
                "up_to_date" => output.info("Already up to date."),
                "no_remote" => output.info("No remote configured."),
                "updated" => {
                    output.info(&result.message);
                    for f in &result.updated_files {
                        println!("  {f}");
                    }
                }
                "conflict" => {
                    output.error(&result.message);
                    println!();
                    println!("Conflicted files:");
                    for f in &result.conflict_files {
                        println!("  {f}");
                    }
                    println!();
                    println!("Resolve with:");
                    println!("  tolaria git resolve <file> --strategy ours");
                    println!("  tolaria git resolve <file> --strategy theirs");
                    println!("  tolaria git resolve <file> --strategy edit");
                }
                _ => {
                    output.error(&result.message);
                }
            }
        }
    }
}

// ── git push ────────────────────────────────────────────────────────

pub fn run_push(vault_path: &str, output: &OutputContext) {
    let result = match git_push(vault_path) {
        Ok(r) => r,
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    };

    match output.format {
        OutputFormat::Json => output.print_json_value(&result),
        OutputFormat::Human => {
            if result.status == "ok" {
                output.info(&result.message);
            } else {
                output.error(&result.message);
                std::process::exit(1);
            }
        }
    }
}

// ── git log ─────────────────────────────────────────────────────────

pub fn run_log(vault_path: &str, file: Option<&str>, output: &OutputContext) {
    let commits = match file {
        Some(f) => {
            let full_path = Path::new(vault_path).join(f);
            let full_str = full_path.to_string_lossy().to_string();
            get_file_history(vault_path, &full_str)
        }
        None => get_recent_commits(vault_path),
    };

    match commits {
        Ok(commits) => print_commits(&commits, output),
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    }
}

fn get_recent_commits(vault_path: &str) -> Result<Vec<GitCommit>, String> {
    let vault = Path::new(vault_path);
    let out = Command::new("git")
        .args(["log", "--format=%H|%h|%an|%aI|%s", "-n", "20"])
        .current_dir(vault)
        .output()
        .map_err(|e| format!("Failed to run git log: {e}"))?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("does not have any commits yet") {
            return Ok(Vec::new());
        }
        return Err(format!("git log failed: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    let commits = stdout
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(parse_log_line)
        .collect();
    Ok(commits)
}

fn parse_log_line(line: &str) -> Option<GitCommit> {
    let parts: Vec<&str> = line.splitn(5, '|').collect();
    if parts.len() != 5 {
        return None;
    }
    let date = chrono::DateTime::parse_from_rfc3339(parts[3])
        .map(|dt| dt.timestamp())
        .unwrap_or(0);
    Some(GitCommit {
        hash: parts[0].to_string(),
        short_hash: parts[1].to_string(),
        author: parts[2].to_string(),
        date,
        message: parts[4].to_string(),
    })
}

fn print_commits(commits: &[GitCommit], output: &OutputContext) {
    match output.format {
        OutputFormat::Json => output.print_json_value(commits),
        OutputFormat::Human => {
            if commits.is_empty() {
                output.info("No commits found.");
                return;
            }
            print_commits_human(commits, output);
        }
    }
}

fn print_commits_human(commits: &[GitCommit], output: &OutputContext) {
    let mut out = output.stdout_stream();
    for (i, commit) in commits.iter().enumerate() {
        let _ = out.set_color(ColorSpec::new().set_fg(Some(Color::Yellow)));
        let _ = write!(out, "{}", commit.short_hash);
        let _ = out.reset();
        let _ = write!(out, " {}", commit.message);

        let _ = out.set_color(ColorSpec::new().set_dimmed(true));
        let date_str = format_commit_date(commit.date);
        let _ = write!(out, "  ({}, {})", commit.author, date_str);
        let _ = out.reset();
        let _ = writeln!(out);

        if i < commits.len() - 1 {
            // no extra spacing needed between compact log lines
        }
    }
}

fn format_commit_date(timestamp: i64) -> String {
    chrono::DateTime::from_timestamp(timestamp, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "-".to_string())
}

// ── git remote ──────────────────────────────────────────────────────

pub fn run_remote(vault_path: &str, output: &OutputContext) {
    let status = match git_remote_status(vault_path) {
        Ok(s) => s,
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    };

    match output.format {
        OutputFormat::Json => output.print_json_value(&status),
        OutputFormat::Human => {
            let mut out = output.stdout_stream();

            let _ = out.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)));
            let _ = write!(out, "  Branch: ");
            let _ = out.reset();
            let _ = writeln!(out, "{}", status.branch);

            let _ = out.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)));
            let _ = write!(out, "  Remote: ");
            let _ = out.reset();
            if status.has_remote {
                let _ = writeln!(out, "yes");

                let _ = out.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)));
                let _ = write!(out, "  Ahead:  ");
                let _ = out.reset();
                let _ = writeln!(out, "{}", status.ahead);

                let _ = out.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)));
                let _ = write!(out, "  Behind: ");
                let _ = out.reset();
                let _ = writeln!(out, "{}", status.behind);
            } else {
                let _ = writeln!(out, "no");
            }
        }
    }
}

// ── git resolve ─────────────────────────────────────────────────────

pub fn run_resolve(vault_path: &str, file: &str, strategy: &str, output: &OutputContext) {
    match strategy {
        "ours" | "theirs" => {
            match git_resolve_conflict(vault_path, file, strategy) {
                Ok(()) => output.info(&format!("Resolved {file} using '{strategy}' strategy.")),
                Err(msg) => {
                    output.error(&msg);
                    std::process::exit(1);
                }
            }
        }
        "edit" => {
            let editor = crate::commands::edit::resolve_editor(output);
            let full_path = Path::new(vault_path).join(file);

            let status = Command::new(&editor)
                .arg(&full_path)
                .status()
                .map_err(|e| format!("Failed to launch editor '{}': {}", editor, e));

            match status {
                Ok(s) if s.success() => {
                    output.info(&format!(
                        "Opened {file} in editor. Stage and commit when ready."
                    ));
                }
                Ok(s) => {
                    let code = s.code().unwrap_or(-1);
                    output.error(&format!("Editor exited with status {code}"));
                    std::process::exit(1);
                }
                Err(msg) => {
                    output.error(&msg);
                    std::process::exit(1);
                }
            }
        }
        _ => {
            output.error(&format!(
                "Invalid strategy '{strategy}': must be 'ours', 'theirs', or 'edit'"
            ));
            std::process::exit(1);
        }
    }
}
