use comfy_table::{Cell, Color as TableColor, Table};
use serde::Serialize;
use std::io::Write;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use tolaria_core::git::ModifiedFile;
use tolaria_core::search::SearchResult;
use tolaria_core::vault::VaultEntry;

/// Output format selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
}

/// Shared output context threaded through all CLI commands.
pub struct OutputContext {
    pub format: OutputFormat,
    pub is_tty: bool,
    pub quiet: bool,
}

impl OutputContext {
    fn color_choice(&self) -> ColorChoice {
        if self.is_tty {
            ColorChoice::Auto
        } else {
            ColorChoice::Never
        }
    }

    fn stdout(&self) -> StandardStream {
        StandardStream::stdout(self.color_choice())
    }

    fn stderr(&self) -> StandardStream {
        StandardStream::stderr(self.color_choice())
    }

    // ── Entries (list) ──────────────────────────────────────────────

    /// Print a list of vault entries as a table (human) or JSON array (json).
    pub fn print_entries(&self, entries: &[VaultEntry]) {
        match self.format {
            OutputFormat::Json => {
                self.print_json(&entries);
            }
            OutputFormat::Human => {
                self.print_entries_table(entries);
            }
        }
    }

    fn print_entries_table(&self, entries: &[VaultEntry]) {
        if entries.is_empty() {
            self.info("No entries found.");
            return;
        }

        let mut table = Table::new();
        table.set_header(vec!["Title", "Type", "Status", "Modified"]);

        for entry in entries {
            let type_str = entry.is_a.as_deref().unwrap_or("-");
            let status_str = entry.status.as_deref().unwrap_or("-");
            let modified = format_timestamp(entry.modified_at);

            let mut title_cell = Cell::new(&entry.title);
            let mut type_cell = Cell::new(type_str);
            let status_cell = Cell::new(status_str);
            let modified_cell = Cell::new(&modified);

            if self.is_tty {
                title_cell = title_cell.fg(TableColor::White);
                type_cell = type_cell.fg(TableColor::Cyan);
            }

            table.add_row(vec![title_cell, type_cell, status_cell, modified_cell]);
        }

        println!("{table}");
    }

    // ── Entry detail (show) ─────────────────────────────────────────

    /// Print a single entry's detail view with frontmatter + body.
    pub fn print_entry_detail(&self, entry: &VaultEntry, content: &str) {
        match self.format {
            OutputFormat::Json => {
                #[derive(Serialize)]
                struct Detail<'a> {
                    entry: &'a VaultEntry,
                    content: &'a str,
                }
                self.print_json(&Detail { entry, content });
            }
            OutputFormat::Human => {
                self.print_entry_detail_human(entry, content);
            }
        }
    }

    fn print_entry_detail_human(&self, entry: &VaultEntry, content: &str) {
        let mut out = self.stdout();

        // Title
        let _ = out.set_color(ColorSpec::new().set_fg(Some(Color::White)).set_bold(true));
        let _ = writeln!(out, "{}", entry.title);
        let _ = out.reset();

        // Frontmatter properties
        if let Some(ref t) = entry.is_a {
            self.write_field(&mut out, "Type", t);
        }
        if let Some(ref s) = entry.status {
            self.write_field(&mut out, "Status", s);
        }
        if !entry.aliases.is_empty() {
            self.write_field(&mut out, "Aliases", &entry.aliases.join(", "));
        }
        if !entry.belongs_to.is_empty() {
            self.write_field(&mut out, "Belongs to", &entry.belongs_to.join(", "));
        }
        if !entry.related_to.is_empty() {
            self.write_field(&mut out, "Related to", &entry.related_to.join(", "));
        }
        for (key, targets) in &entry.relationships {
            self.write_field(&mut out, key, &targets.join(", "));
        }
        for (key, value) in &entry.properties {
            let display = match value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            self.write_field(&mut out, key, &display);
        }

        let _ = writeln!(out);

        // Body
        let _ = writeln!(out, "{content}");
    }

    fn write_field(&self, out: &mut StandardStream, label: &str, value: &str) {
        let _ = out.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)));
        let _ = write!(out, "  {label}: ");
        let _ = out.reset();
        let _ = writeln!(out, "{value}");
    }

    // ── Search results ──────────────────────────────────────────────

    /// Print search results with score and snippet.
    pub fn print_search_results(&self, results: &[SearchResult]) {
        match self.format {
            OutputFormat::Json => {
                self.print_json(&results);
            }
            OutputFormat::Human => {
                self.print_search_results_human(results);
            }
        }
    }

    fn print_search_results_human(&self, results: &[SearchResult]) {
        if results.is_empty() {
            self.info("No results found.");
            return;
        }

        let mut out = self.stdout();

        for (i, result) in results.iter().enumerate() {
            // Title line with score
            let _ = out.set_color(ColorSpec::new().set_fg(Some(Color::White)).set_bold(true));
            let _ = write!(out, "{}. {}", i + 1, result.title);
            let _ = out.reset();

            let _ = out.set_color(ColorSpec::new().set_fg(Some(Color::Yellow)));
            let _ = writeln!(out, "  ({:.1})", result.score);
            let _ = out.reset();

            // Type if present
            if let Some(ref note_type) = result.note_type {
                let _ = out.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)));
                let _ = write!(out, "   Type: ");
                let _ = out.reset();
                let _ = writeln!(out, "{note_type}");
            }

            // Path
            let _ = out.set_color(ColorSpec::new().set_dimmed(true));
            let _ = writeln!(out, "   {}", result.path);
            let _ = out.reset();

            // Snippet
            if !result.snippet.is_empty() {
                let _ = writeln!(out, "   {}", result.snippet);
            }

            if i < results.len() - 1 {
                let _ = writeln!(out);
            }
        }
    }

    // ── Modified files (git status) ─────────────────────────────────

    /// Print git status display of modified files.
    pub fn print_modified_files(&self, files: &[ModifiedFile]) {
        match self.format {
            OutputFormat::Json => {
                self.print_json(&files);
            }
            OutputFormat::Human => {
                self.print_modified_files_human(files);
            }
        }
    }

    fn print_modified_files_human(&self, files: &[ModifiedFile]) {
        if files.is_empty() {
            self.info("No modified files.");
            return;
        }

        let mut out = self.stdout();

        for file in files {
            let (indicator, color) = match file.status.as_str() {
                "modified" => ("M", Color::Yellow),
                "added" => ("A", Color::Green),
                "deleted" => ("D", Color::Red),
                "untracked" => ("?", Color::Cyan),
                "renamed" => ("R", Color::Magenta),
                _ => (" ", Color::White),
            };

            let _ = out.set_color(ColorSpec::new().set_fg(Some(color)).set_bold(true));
            let _ = write!(out, " {indicator} ");
            let _ = out.reset();
            let _ = write!(out, "{}", file.relative_path);

            // Line stats
            let mut stats_parts: Vec<String> = Vec::new();
            if let Some(added) = file.added_lines {
                stats_parts.push(format!("+{added}"));
            }
            if let Some(deleted) = file.deleted_lines {
                stats_parts.push(format!("-{deleted}"));
            }
            if !stats_parts.is_empty() {
                let _ = out.set_color(ColorSpec::new().set_dimmed(true));
                let _ = write!(out, "  ({})", stats_parts.join(", "));
                let _ = out.reset();
            }

            let _ = writeln!(out);
        }
    }

    // ── Info / Error ────────────────────────────────────────────────

    /// Print an informational message. Suppressed in quiet mode.
    pub fn info(&self, msg: &str) {
        if self.quiet {
            return;
        }
        let mut out = self.stdout();
        let _ = out.set_color(ColorSpec::new().set_fg(Some(Color::Blue)));
        let _ = write!(out, "info: ");
        let _ = out.reset();
        let _ = writeln!(out, "{msg}");
    }

    /// Print an error message.
    /// - Human mode: colored `error:` prefix (red when TTY) + message to stderr
    /// - JSON mode: `{ "error": "message" }` to stdout + raw message to stderr
    pub fn error(&self, msg: &str) {
        match self.format {
            OutputFormat::Json => {
                #[derive(Serialize)]
                struct ErrPayload<'a> {
                    error: &'a str,
                }
                let json = serde_json::to_string(&ErrPayload { error: msg })
                    .unwrap_or_else(|_| format!("{{\"error\":\"{msg}\"}}"));
                println!("{json}");
                // JSON mode: also write raw message to stderr for logging
                eprintln!("{msg}");
            }
            OutputFormat::Human => {
                let mut err = self.stderr();
                let _ = err.set_color(ColorSpec::new().set_fg(Some(Color::Red)).set_bold(true));
                let _ = write!(err, "error: ");
                let _ = err.reset();
                let _ = writeln!(err, "{msg}");
            }
        }
    }

    // ── Helpers ─────────────────────────────────────────────────────

    /// Get a stdout stream with the appropriate color choice.
    pub fn stdout_stream(&self) -> StandardStream {
        self.stdout()
    }

    /// Print any serializable value as JSON.
    pub fn print_json_value<T: Serialize + ?Sized>(&self, value: &T) {
        self.print_json(value);
    }

    fn print_json<T: Serialize + ?Sized>(&self, value: &T) {
        match serde_json::to_string_pretty(value) {
            Ok(json) => println!("{json}"),
            Err(e) => {
                self.error(&format!("Failed to serialize JSON: {e}"));
            }
        }
    }

    // ── Buffer-based helpers (for testing) ──────────────────────────

    /// Serialize a value to JSON and write to the provided buffer.
    /// Returns Ok(()) on success, Err with the serialization error message.
    pub(crate) fn format_json_to_buf<T: Serialize>(
        &self,
        value: &T,
        buf: &mut Vec<u8>,
    ) -> Result<(), String> {
        let json = serde_json::to_string_pretty(value)
            .map_err(|e| format!("Failed to serialize JSON: {e}"))?;
        writeln!(buf, "{json}").map_err(|e| e.to_string())
    }

    /// Write an informational message to the provided buffer.
    /// Respects `quiet` mode (writes nothing when quiet).
    /// When `is_tty` is false, no ANSI codes are emitted.
    pub(crate) fn format_info_to_buf(&self, msg: &str, buf: &mut Vec<u8>) {
        if self.quiet {
            return;
        }
        let choice = self.color_choice();
        let mut out = termcolor::Buffer::ansi();
        if choice == ColorChoice::Never {
            out = termcolor::Buffer::no_color();
        }
        let _ = out.set_color(ColorSpec::new().set_fg(Some(Color::Blue)));
        let _ = write!(out, "info: ");
        let _ = out.reset();
        let _ = writeln!(out, "{msg}");
        buf.extend_from_slice(out.as_slice());
    }

    /// Format a list of vault entries in human-readable form to a buffer.
    /// Uses comfy-table without color when `is_tty` is false.
    pub(crate) fn format_entries_human_to_buf(
        &self,
        entries: &[VaultEntry],
        buf: &mut Vec<u8>,
    ) {
        if entries.is_empty() {
            self.format_info_to_buf("No entries found.", buf);
            return;
        }

        let mut table = Table::new();
        table.set_header(vec!["Title", "Type", "Status", "Modified"]);

        for entry in entries {
            let type_str = entry.is_a.as_deref().unwrap_or("-");
            let status_str = entry.status.as_deref().unwrap_or("-");
            let modified = format_timestamp(entry.modified_at);

            table.add_row(vec![
                Cell::new(&entry.title),
                Cell::new(type_str),
                Cell::new(status_str),
                Cell::new(&modified),
            ]);
        }

        let _ = writeln!(buf, "{table}");
    }

    /// Format a single entry detail in human-readable form to a buffer.
    /// No ANSI codes when `is_tty` is false.
    pub(crate) fn format_entry_detail_human_to_buf(
        &self,
        entry: &VaultEntry,
        content: &str,
        buf: &mut Vec<u8>,
    ) {
        let choice = self.color_choice();
        let mut out = if choice == ColorChoice::Never {
            termcolor::Buffer::no_color()
        } else {
            termcolor::Buffer::ansi()
        };

        // Title
        let _ = out.set_color(ColorSpec::new().set_fg(Some(Color::White)).set_bold(true));
        let _ = writeln!(out, "{}", entry.title);
        let _ = out.reset();

        if let Some(ref t) = entry.is_a {
            Self::write_field_to_buf(&mut out, choice, "Type", t);
        }
        if let Some(ref s) = entry.status {
            Self::write_field_to_buf(&mut out, choice, "Status", s);
        }
        let modified = format_timestamp(entry.modified_at);
        Self::write_field_to_buf(&mut out, choice, "Modified", &modified);

        let _ = writeln!(out);
        let _ = writeln!(out, "{content}");

        buf.extend_from_slice(out.as_slice());
    }

    fn write_field_to_buf(
        out: &mut termcolor::Buffer,
        _choice: ColorChoice,
        label: &str,
        value: &str,
    ) {
        let _ = out.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)));
        let _ = write!(out, "  {label}: ");
        let _ = out.reset();
        let _ = writeln!(out, "{value}");
    }
}

/// Format a unix timestamp (seconds) into a human-readable date string.
fn format_timestamp(ts: Option<u64>) -> String {
    match ts {
        Some(secs) => {
            let dt = chrono::DateTime::from_timestamp(secs as i64, 0);
            match dt {
                Some(d) => d.format("%Y-%m-%d %H:%M").to_string(),
                None => "-".to_string(),
            }
        }
        None => "-".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::HashMap;

    #[test]
    fn test_format_timestamp_none() {
        assert_eq!(format_timestamp(None), "-");
    }

    #[test]
    fn test_format_timestamp_some() {
        // 2024-01-15 12:00:00 UTC = 1705320000
        let result = format_timestamp(Some(1705320000));
        assert!(result.starts_with("2024-01-15"));
    }

    #[test]
    fn test_output_format_equality() {
        assert_eq!(OutputFormat::Human, OutputFormat::Human);
        assert_eq!(OutputFormat::Json, OutputFormat::Json);
        assert_ne!(OutputFormat::Human, OutputFormat::Json);
    }

    // ── Arbitrary generators ────────────────────────────────────────

    fn arb_opt_string() -> impl Strategy<Value = Option<String>> {
        proptest::option::of("[a-zA-Z0-9 _-]{1,20}")
    }

    fn arb_string_vec() -> impl Strategy<Value = Vec<String>> {
        proptest::collection::vec("[a-zA-Z0-9_-]{1,15}", 0..3)
    }

    fn arb_vault_entry() -> impl Strategy<Value = VaultEntry> {
        (
            (
                "[a-z/]{1,30}",
                "[a-z-]{1,20}\\.md",
                "[A-Za-z0-9 ]{1,25}",
                arb_opt_string(),
                arb_string_vec(),
                arb_string_vec(),
                arb_string_vec(),
                arb_opt_string(),
            ),
            (
                any::<bool>(),
                proptest::option::of(0u64..2_000_000_000u64),
                proptest::option::of(0u64..2_000_000_000u64),
                0u64..1_000_000u64,
                "[a-zA-Z0-9 .]{0,30}",
                any::<bool>(),
                any::<bool>(),
                any::<u32>(),
            ),
        )
            .prop_map(|(a, b)| VaultEntry {
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
                organized: b.5,
                favorite: b.6,
                word_count: b.7,
                relationships: HashMap::new(),
                properties: HashMap::new(),
                icon: None,
                color: None,
                order: None,
                sidebar_label: None,
                template: None,
                sort: None,
                view: None,
                visible: None,
                favorite_index: None,
                outgoing_links: Vec::new(),
                list_properties_display: Vec::new(),
                has_h1: true,
                file_kind: "markdown".to_string(),
            })
    }

    fn arb_search_result() -> impl Strategy<Value = SearchResult> {
        (
            "[A-Za-z0-9 ]{1,25}",
            "[a-z/]{1,30}\\.md",
            "[a-zA-Z0-9 .]{0,30}",
            0.0f64..100.0f64,
            arb_opt_string(),
        )
            .prop_map(|(title, path, snippet, score, note_type)| SearchResult {
                title,
                path,
                snippet,
                score,
                note_type,
            })
    }

    fn arb_modified_file() -> impl Strategy<Value = ModifiedFile> {
        (
            "[a-z/]{1,30}\\.md",
            "[a-z/]{1,20}\\.md",
            prop_oneof![
                Just("modified".to_string()),
                Just("added".to_string()),
                Just("deleted".to_string()),
                Just("untracked".to_string()),
            ],
            proptest::option::of(0usize..500usize),
            proptest::option::of(0usize..500usize),
            any::<bool>(),
        )
            .prop_map(
                |(path, relative_path, status, added_lines, deleted_lines, binary)| {
                    ModifiedFile {
                        path,
                        relative_path,
                        status,
                        added_lines,
                        deleted_lines,
                        binary,
                    }
                },
            )
    }

    // ── Property 12: JSON Output Validity ───────────────────────────
    // **Validates: Requirements 10.1**
    //
    // For any domain object, formatting with OutputFormat::Json produces
    // valid parseable JSON.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_json_output_validity_entries(entries in proptest::collection::vec(arb_vault_entry(), 0..5)) {
            let ctx = OutputContext {
                format: OutputFormat::Json,
                is_tty: false,
                quiet: false,
            };
            let mut buf = Vec::new();
            ctx.format_json_to_buf(&entries, &mut buf).unwrap();
            let output = String::from_utf8(buf).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
            prop_assert!(parsed.is_array());
        }

        #[test]
        fn prop_json_output_validity_search(results in proptest::collection::vec(arb_search_result(), 0..5)) {
            let ctx = OutputContext {
                format: OutputFormat::Json,
                is_tty: false,
                quiet: false,
            };
            let mut buf = Vec::new();
            ctx.format_json_to_buf(&results, &mut buf).unwrap();
            let output = String::from_utf8(buf).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
            prop_assert!(parsed.is_array());
        }

        #[test]
        fn prop_json_output_validity_modified(files in proptest::collection::vec(arb_modified_file(), 0..5)) {
            let ctx = OutputContext {
                format: OutputFormat::Json,
                is_tty: false,
                quiet: false,
            };
            let mut buf = Vec::new();
            ctx.format_json_to_buf(&files, &mut buf).unwrap();
            let output = String::from_utf8(buf).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
            prop_assert!(parsed.is_array());
        }

        #[test]
        fn prop_json_output_validity_entry_detail(entry in arb_vault_entry(), content in "[a-zA-Z0-9 \n.]{0,100}") {
            let ctx = OutputContext {
                format: OutputFormat::Json,
                is_tty: false,
                quiet: false,
            };
            #[derive(Serialize)]
            struct Detail<'a> {
                entry: &'a VaultEntry,
                content: &'a str,
            }
            let mut buf = Vec::new();
            ctx.format_json_to_buf(&Detail { entry: &entry, content: &content }, &mut buf).unwrap();
            let output = String::from_utf8(buf).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
            prop_assert!(parsed.is_object());
            prop_assert!(parsed.get("entry").is_some());
            prop_assert!(parsed.get("content").is_some());
        }
    }

    // ── Property 13: Non-TTY Output Has No ANSI Codes ───────────────
    // **Validates: Requirements 10.3**
    //
    // For any output produced when is_tty = false, verify no ANSI escape
    // sequences present.

    fn contains_ansi(bytes: &[u8]) -> bool {
        let s = String::from_utf8_lossy(bytes);
        s.contains("\x1b[")
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_non_tty_no_ansi_info(msg in "[a-zA-Z0-9 ]{1,50}") {
            let ctx = OutputContext {
                format: OutputFormat::Human,
                is_tty: false,
                quiet: false,
            };
            let mut buf = Vec::new();
            ctx.format_info_to_buf(&msg, &mut buf);
            prop_assert!(!contains_ansi(&buf), "ANSI codes found in non-TTY info output");
        }

        #[test]
        fn prop_non_tty_no_ansi_entries(entries in proptest::collection::vec(arb_vault_entry(), 1..4)) {
            let ctx = OutputContext {
                format: OutputFormat::Human,
                is_tty: false,
                quiet: false,
            };
            let mut buf = Vec::new();
            ctx.format_entries_human_to_buf(&entries, &mut buf);
            prop_assert!(!contains_ansi(&buf), "ANSI codes found in non-TTY entries output");
        }

        #[test]
        fn prop_non_tty_no_ansi_detail(entry in arb_vault_entry(), content in "[a-zA-Z0-9 ]{0,50}") {
            let ctx = OutputContext {
                format: OutputFormat::Human,
                is_tty: false,
                quiet: false,
            };
            let mut buf = Vec::new();
            ctx.format_entry_detail_human_to_buf(&entry, &content, &mut buf);
            prop_assert!(!contains_ansi(&buf), "ANSI codes found in non-TTY detail output");
        }
    }

    // ── Property 14: Quiet Mode Suppresses Informational Messages ───
    // **Validates: Requirements 10.5**
    //
    // For any operation with quiet = true, info() produces no output.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_quiet_mode_suppresses_info(msg in "[a-zA-Z0-9 ]{1,50}") {
            let ctx = OutputContext {
                format: OutputFormat::Human,
                is_tty: false,
                quiet: true,
            };
            let mut buf = Vec::new();
            ctx.format_info_to_buf(&msg, &mut buf);
            prop_assert!(buf.is_empty(), "Quiet mode should suppress info messages, got: {:?}", String::from_utf8_lossy(&buf));
        }

        #[test]
        fn prop_quiet_mode_suppresses_info_tty(msg in "[a-zA-Z0-9 ]{1,50}") {
            let ctx = OutputContext {
                format: OutputFormat::Human,
                is_tty: true,
                quiet: true,
            };
            let mut buf = Vec::new();
            ctx.format_info_to_buf(&msg, &mut buf);
            prop_assert!(buf.is_empty(), "Quiet mode should suppress info even on TTY");
        }

        #[test]
        fn prop_quiet_mode_suppresses_info_json(msg in "[a-zA-Z0-9 ]{1,50}") {
            let ctx = OutputContext {
                format: OutputFormat::Json,
                is_tty: false,
                quiet: true,
            };
            let mut buf = Vec::new();
            ctx.format_info_to_buf(&msg, &mut buf);
            prop_assert!(buf.is_empty(), "Quiet mode should suppress info in JSON mode too");
        }
    }

    // ── Property 11: Output Completeness ────────────────────────────
    // **Validates: Requirements 3.1, 4.2, 6.1, 6.6, 6.9, 9.3**
    //
    // For any domain object, human-readable format includes all required
    // display fields (title, type, status, modified date for entries).

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_output_completeness_entries(entry in arb_vault_entry()) {
            let ctx = OutputContext {
                format: OutputFormat::Human,
                is_tty: false,
                quiet: false,
            };
            let mut buf = Vec::new();
            ctx.format_entries_human_to_buf(&[entry.clone()], &mut buf);
            let output = String::from_utf8(buf).unwrap();

            // Title must appear in the table
            prop_assert!(output.contains(&entry.title),
                "Entry table missing title '{}' in output:\n{}", entry.title, output);

            // Type column: shows the type or "-" placeholder
            let type_str = entry.is_a.as_deref().unwrap_or("-");
            prop_assert!(output.contains(type_str),
                "Entry table missing type '{}' in output:\n{}", type_str, output);

            // Status column: shows the status or "-" placeholder
            let status_str = entry.status.as_deref().unwrap_or("-");
            prop_assert!(output.contains(status_str),
                "Entry table missing status '{}' in output:\n{}", status_str, output);

            // Modified column: shows formatted date or "-"
            let modified = format_timestamp(entry.modified_at);
            prop_assert!(output.contains(&modified),
                "Entry table missing modified '{}' in output:\n{}", modified, output);
        }

        #[test]
        fn prop_output_completeness_detail(entry in arb_vault_entry(), content in "[a-zA-Z0-9 ]{0,50}") {
            let ctx = OutputContext {
                format: OutputFormat::Human,
                is_tty: false,
                quiet: false,
            };
            let mut buf = Vec::new();
            ctx.format_entry_detail_human_to_buf(&entry, &content, &mut buf);
            let output = String::from_utf8(buf).unwrap();

            // Title must appear
            prop_assert!(output.contains(&entry.title),
                "Detail view missing title '{}' in output:\n{}", entry.title, output);

            // Type must appear if present
            if let Some(ref t) = entry.is_a {
                prop_assert!(output.contains(t),
                    "Detail view missing type '{}' in output:\n{}", t, output);
            }

            // Status must appear if present
            if let Some(ref s) = entry.status {
                prop_assert!(output.contains(s),
                    "Detail view missing status '{}' in output:\n{}", s, output);
            }

            // Modified date must appear
            let modified = format_timestamp(entry.modified_at);
            prop_assert!(output.contains(&modified),
                "Detail view missing modified '{}' in output:\n{}", modified, output);
        }
    }
}
