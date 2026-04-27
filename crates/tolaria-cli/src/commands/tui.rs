use std::io;
use std::path::Path;
use std::process::Command;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use tolaria_core::vault::{get_note_content, scan_vault_cached, VaultEntry};

use super::edit::resolve_editor;
use crate::output::{OutputContext, OutputFormat};

/// A group of notes sharing the same type.
struct TypeGroup {
    type_name: String,
    entries: Vec<VaultEntry>,
}

/// Application state for the TUI.
struct App {
    /// Flat list of displayable rows (headers + entries).
    rows: Vec<Row>,
    /// Index into `rows` of the currently highlighted row.
    cursor: usize,
    /// Search filter string (empty = show all).
    filter: String,
    /// Whether the search input bar is active.
    searching: bool,
    /// All vault entries (unfiltered).
    all_entries: Vec<VaultEntry>,
    /// Vault path for reading note content.
    vault_path: String,
}

/// A single row in the note list panel.
enum Row {
    /// Type group header (not selectable for editing).
    Header(String),
    /// A note entry.
    Entry(VaultEntry),
}

impl App {
    fn new(vault_path: String, entries: Vec<VaultEntry>) -> Self {
        let mut app = App {
            rows: Vec::new(),
            cursor: 0,
            filter: String::new(),
            searching: false,
            all_entries: entries,
            vault_path,
        };
        app.rebuild_rows();
        app
    }

    /// Rebuild the flat row list from entries, applying the current filter
    /// and grouping by type.
    fn rebuild_rows(&mut self) {
        let filtered: Vec<&VaultEntry> = if self.filter.is_empty() {
            self.all_entries.iter().collect()
        } else {
            let q = self.filter.to_lowercase();
            self.all_entries
                .iter()
                .filter(|e| e.title.to_lowercase().contains(&q))
                .collect()
        };

        let groups = group_by_type(&filtered);

        self.rows.clear();
        for group in &groups {
            self.rows
                .push(Row::Header(format!("── {} ──", group.type_name)));
            for entry in &group.entries {
                self.rows.push(Row::Entry(entry.clone()));
            }
        }

        // Clamp cursor
        if self.rows.is_empty() {
            self.cursor = 0;
        } else if self.cursor >= self.rows.len() {
            self.cursor = self.rows.len().saturating_sub(1);
        }
        // Skip to first entry if cursor lands on a header
        self.skip_to_entry_forward();
    }

    fn move_up(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        if self.cursor > 0 {
            self.cursor -= 1;
            self.skip_to_entry_backward();
        }
    }

    fn move_down(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        if self.cursor + 1 < self.rows.len() {
            self.cursor += 1;
            self.skip_to_entry_forward();
        }
    }

    /// If cursor is on a header, move forward to the next entry.
    fn skip_to_entry_forward(&mut self) {
        while self.cursor < self.rows.len() {
            if matches!(self.rows[self.cursor], Row::Entry(_)) {
                return;
            }
            self.cursor += 1;
        }
        // Wrapped past end — try backward
        if self.cursor >= self.rows.len() {
            self.cursor = self.rows.len().saturating_sub(1);
            self.skip_to_entry_backward();
        }
    }

    /// If cursor is on a header, move backward to the previous entry.
    fn skip_to_entry_backward(&mut self) {
        loop {
            if matches!(self.rows.get(self.cursor), Some(Row::Entry(_))) {
                return;
            }
            if self.cursor == 0 {
                // No entries before — try forward
                self.skip_to_entry_forward();
                return;
            }
            self.cursor -= 1;
        }
    }

    /// Get the currently selected VaultEntry, if any.
    fn selected_entry(&self) -> Option<&VaultEntry> {
        match self.rows.get(self.cursor) {
            Some(Row::Entry(e)) => Some(e),
            _ => None,
        }
    }
}

/// Group entries by type, sorted by type name. Entries without a type go
/// into an "Uncategorized" group at the end. Within each group, entries
/// are sorted by modified date descending (most recent first).
fn group_by_type(entries: &[&VaultEntry]) -> Vec<TypeGroup> {
    use std::collections::BTreeMap;

    let mut map: BTreeMap<String, Vec<VaultEntry>> = BTreeMap::new();
    for entry in entries {
        let key = entry
            .is_a
            .as_deref()
            .unwrap_or("Uncategorized")
            .to_string();
        map.entry(key).or_default().push((*entry).clone());
    }

    let mut groups: Vec<TypeGroup> = map
        .into_iter()
        .map(|(type_name, mut entries)| {
            entries.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
            TypeGroup { type_name, entries }
        })
        .collect();

    // Move "Uncategorized" to the end
    if let Some(pos) = groups.iter().position(|g| g.type_name == "Uncategorized") {
        let uncat = groups.remove(pos);
        groups.push(uncat);
    }

    groups
}

/// Launch the interactive TUI.
pub fn run(vault_path: &str) {
    let entries = match scan_vault_cached(Path::new(vault_path)) {
        Ok(e) => e,
        Err(msg) => {
            eprintln!("error: {}", msg);
            std::process::exit(1);
        }
    };

    let mut app = App::new(vault_path.to_string(), entries);

    // Set up terminal
    enable_raw_mode().expect("Failed to enable raw mode");
    io::stdout()
        .execute(EnterAlternateScreen)
        .expect("Failed to enter alternate screen");

    let backend = ratatui::backend::CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

    let result = run_event_loop(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode().ok();
    io::stdout().execute(LeaveAlternateScreen).ok();

    if let Err(e) = result {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}

/// Main event loop — draws UI and handles keyboard input.
fn run_event_loop(
    terminal: &mut Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        terminal.draw(|frame| draw_ui(frame, app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                if app.searching {
                    match key.code {
                        KeyCode::Esc => {
                            app.searching = false;
                            app.filter.clear();
                            app.rebuild_rows();
                        }
                        KeyCode::Enter => {
                            app.searching = false;
                        }
                        KeyCode::Backspace => {
                            app.filter.pop();
                            app.rebuild_rows();
                        }
                        KeyCode::Char(c) => {
                            app.filter.push(c);
                            app.rebuild_rows();
                        }
                        _ => {}
                    }
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                    KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                    KeyCode::Char('/') => {
                        app.searching = true;
                        app.filter.clear();
                    }
                    KeyCode::Enter | KeyCode::Char('e') => {
                        if let Some(entry) = app.selected_entry().cloned() {
                            open_in_editor(terminal, &entry.path);
                            // Reload vault after editor closes
                            if let Ok(entries) =
                                scan_vault_cached(Path::new(&app.vault_path))
                            {
                                app.all_entries = entries;
                                app.rebuild_rows();
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Temporarily leave the TUI, open a file in $EDITOR, then restore.
fn open_in_editor(
    terminal: &mut Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>,
    path: &str,
) {
    // Create a dummy output context just for editor resolution
    let output = OutputContext {
        format: OutputFormat::Human,
        is_tty: true,
        quiet: true,
    };
    let editor = resolve_editor(&output);

    // Leave alternate screen so the editor gets a normal terminal
    disable_raw_mode().ok();
    io::stdout().execute(LeaveAlternateScreen).ok();

    let _ = Command::new(&editor).arg(path).status();

    // Re-enter TUI mode
    io::stdout().execute(EnterAlternateScreen).ok();
    enable_raw_mode().ok();
    terminal.clear().ok();
}

/// Draw the two-panel layout.
fn draw_ui(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Split horizontally: 35% note list, 65% preview
    let chunks = Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    draw_note_list(frame, app, chunks[0]);
    draw_preview(frame, app, chunks[1]);

    // Draw search bar at the bottom if searching
    if app.searching {
        let search_area = Rect {
            x: area.x,
            y: area.height.saturating_sub(1),
            width: area.width,
            height: 1,
        };
        let search_text = format!("/{}", app.filter);
        let search_bar = Paragraph::new(search_text)
            .style(Style::default().fg(Color::Yellow).bg(Color::DarkGray));
        frame.render_widget(search_bar, search_area);
    }
}

/// Draw the left panel: note list grouped by type.
fn draw_note_list(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| match row {
            Row::Header(label) => {
                let style = Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD);
                ListItem::new(Line::from(Span::styled(label.clone(), style)))
            }
            Row::Entry(entry) => {
                let status_indicator = entry
                    .status
                    .as_deref()
                    .map(|s| format!(" [{}]", s))
                    .unwrap_or_default();
                let text = format!("  {}{}", entry.title, status_indicator);

                let style = if i == app.cursor {
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::Blue)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(Line::from(Span::styled(text, style)))
            }
        })
        .collect();

    let title = if app.filter.is_empty() {
        " Notes ".to_string()
    } else {
        format!(" Notes (filter: {}) ", app.filter)
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::Gray)),
    );

    // Use ListState to handle scrolling
    let mut state = ListState::default();
    state.select(Some(app.cursor));
    frame.render_stateful_widget(list, area, &mut state);
}

/// Draw the right panel: note preview with basic markdown rendering.
fn draw_preview(frame: &mut Frame, app: &App, area: Rect) {
    let content = match app.selected_entry() {
        Some(entry) => match get_note_content(Path::new(&entry.path)) {
            Ok(c) => c,
            Err(_) => "(unable to read note)".to_string(),
        },
        None => "(no note selected)".to_string(),
    };

    let title = app
        .selected_entry()
        .map(|e| format!(" {} ", e.title))
        .unwrap_or_else(|| " Preview ".to_string());

    let lines = render_markdown(&content);

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(Color::Gray)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

/// Basic markdown rendering: convert markdown text into styled ratatui Lines.
/// Handles headings, bold, italic, lists, and code blocks.
fn render_markdown(content: &str) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut in_code_block = false;
    let mut in_frontmatter = false;
    let mut frontmatter_count = 0;

    for raw_line in content.lines() {
        // Track frontmatter delimiters
        if raw_line.trim() == "---" {
            frontmatter_count += 1;
            if frontmatter_count == 1 {
                in_frontmatter = true;
                continue;
            } else if frontmatter_count == 2 {
                in_frontmatter = false;
                continue;
            }
        }

        // Skip frontmatter content
        if in_frontmatter {
            continue;
        }

        // Code block toggle
        if raw_line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            let style = Style::default().fg(Color::DarkGray);
            lines.push(Line::from(Span::styled(
                raw_line.to_string(),
                style,
            )));
            continue;
        }

        if in_code_block {
            let style = Style::default().fg(Color::Green);
            lines.push(Line::from(Span::styled(
                raw_line.to_string(),
                style,
            )));
            continue;
        }

        // Headings
        if let Some(heading) = parse_heading(raw_line) {
            lines.push(heading);
            continue;
        }

        // Unordered list items
        let trimmed = raw_line.trim_start();
        if trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || trimmed.starts_with("+ ")
        {
            let indent = raw_line.len() - trimmed.len();
            let prefix = " ".repeat(indent);
            let bullet_content = &trimmed[2..];
            let mut spans = vec![Span::raw(format!("{}• ", prefix))];
            spans.extend(parse_inline(bullet_content));
            lines.push(Line::from(spans));
            continue;
        }

        // Ordered list items
        if let Some(pos) = trimmed.find(". ") {
            if pos > 0 && trimmed[..pos].chars().all(|c| c.is_ascii_digit()) {
                let indent = raw_line.len() - trimmed.len();
                let prefix = " ".repeat(indent);
                let number = &trimmed[..pos];
                let rest = &trimmed[pos + 2..];
                let mut spans = vec![Span::raw(format!("{}{}. ", prefix, number))];
                spans.extend(parse_inline(rest));
                lines.push(Line::from(spans));
                continue;
            }
        }

        // Regular paragraph line with inline formatting
        let spans = parse_inline(raw_line);
        lines.push(Line::from(spans));
    }

    lines
}

/// Parse a heading line (# through ######) into a styled Line.
fn parse_heading(line: &str) -> Option<Line<'static>> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return None;
    }

    let level = trimmed.chars().take_while(|&c| c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }

    // Must have a space after the hashes
    if trimmed.len() <= level || trimmed.as_bytes()[level] != b' ' {
        return None;
    }

    let text = &trimmed[level + 1..];
    let color = match level {
        1 => Color::Magenta,
        2 => Color::Blue,
        3 => Color::Cyan,
        _ => Color::White,
    };

    let style = Style::default()
        .fg(color)
        .add_modifier(Modifier::BOLD);

    Some(Line::from(Span::styled(
        format!("{} {}", "#".repeat(level), text),
        style,
    )))
}

/// Parse inline markdown formatting: **bold**, *italic*, `code`, [[wikilinks]].
fn parse_inline(text: &str) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        // Find the earliest special marker
        let bold_pos = remaining.find("**");
        let italic_pos = remaining.find('*').filter(|&p| {
            // Only match single * not preceded/followed by another *
            let at_bold = bold_pos == Some(p);
            !at_bold
        });
        let code_pos = remaining.find('`');
        let link_pos = remaining.find("[[");

        // Find the earliest marker
        let positions: Vec<(usize, &str)> = [
            bold_pos.map(|p| (p, "**")),
            italic_pos.map(|p| (p, "*")),
            code_pos.map(|p| (p, "`")),
            link_pos.map(|p| (p, "[[")),
        ]
        .into_iter()
        .flatten()
        .collect();

        let earliest = positions.iter().min_by_key(|(pos, _)| *pos);

        match earliest {
            None => {
                // No more markers — emit the rest as plain text
                spans.push(Span::raw(remaining.to_string()));
                break;
            }
            Some(&(pos, marker)) => {
                // Emit text before the marker
                if pos > 0 {
                    spans.push(Span::raw(remaining[..pos].to_string()));
                }

                let after_marker = &remaining[pos + marker.len()..];

                match marker {
                    "**" => {
                        if let Some(end) = after_marker.find("**") {
                            let content = &after_marker[..end];
                            spans.push(Span::styled(
                                content.to_string(),
                                Style::default().add_modifier(Modifier::BOLD),
                            ));
                            remaining = &after_marker[end + 2..];
                        } else {
                            spans.push(Span::raw("**".to_string()));
                            remaining = after_marker;
                        }
                    }
                    "*" => {
                        if let Some(end) = after_marker.find('*') {
                            let content = &after_marker[..end];
                            spans.push(Span::styled(
                                content.to_string(),
                                Style::default().add_modifier(Modifier::ITALIC),
                            ));
                            remaining = &after_marker[end + 1..];
                        } else {
                            spans.push(Span::raw("*".to_string()));
                            remaining = after_marker;
                        }
                    }
                    "`" => {
                        if let Some(end) = after_marker.find('`') {
                            let content = &after_marker[..end];
                            spans.push(Span::styled(
                                content.to_string(),
                                Style::default().fg(Color::Green),
                            ));
                            remaining = &after_marker[end + 1..];
                        } else {
                            spans.push(Span::raw("`".to_string()));
                            remaining = after_marker;
                        }
                    }
                    "[[" => {
                        if let Some(end) = after_marker.find("]]") {
                            let content = &after_marker[..end];
                            // Handle [[target|display]] syntax
                            let display = content
                                .split('|')
                                .nth(1)
                                .unwrap_or(content);
                            spans.push(Span::styled(
                                format!("[[{}]]", display),
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::UNDERLINED),
                            ));
                            remaining = &after_marker[end + 2..];
                        } else {
                            spans.push(Span::raw("[[".to_string()));
                            remaining = after_marker;
                        }
                    }
                    _ => {
                        spans.push(Span::raw(marker.to_string()));
                        remaining = after_marker;
                    }
                }
            }
        }
    }

    if spans.is_empty() {
        spans.push(Span::raw(String::new()));
    }

    spans
}
