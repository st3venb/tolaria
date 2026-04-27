use std::io::{self, Write};

use tolaria_core::ai_agents::{
    self, AiAgentId, AiAgentStreamEvent, AiAgentStreamRequest,
};

use crate::output::{OutputContext, OutputFormat};

// ── ai status ───────────────────────────────────────────────────────

pub fn run_status(output: &OutputContext) {
    let status = ai_agents::get_ai_agents_status();
    match output.format {
        OutputFormat::Json => output.print_json_value(&status),
        OutputFormat::Human => {
            print_agent_availability("Claude Code", &status.claude_code, output);
            print_agent_availability("Codex", &status.codex, output);
        }
    }
}

fn print_agent_availability(
    name: &str,
    avail: &ai_agents::AiAgentAvailability,
    output: &OutputContext,
) {
    let mut out = output.stdout_stream();
    use termcolor::{Color, ColorSpec, WriteColor};

    let _ = out.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)));
    let _ = write!(out, "  {name}: ");
    let _ = out.reset();

    if avail.installed {
        let _ = out.set_color(ColorSpec::new().set_fg(Some(Color::Green)));
        let _ = write!(out, "available");
        let _ = out.reset();
        if let Some(ref ver) = avail.version {
            let _ = out.set_color(ColorSpec::new().set_dimmed(true));
            let _ = write!(out, " ({ver})");
            let _ = out.reset();
        }
    } else {
        let _ = out.set_color(ColorSpec::new().set_fg(Some(Color::Red)));
        let _ = write!(out, "not found");
        let _ = out.reset();
    }
    let _ = writeln!(out);
}

// ── ai chat ─────────────────────────────────────────────────────────

pub fn run_chat(
    vault_path: &str,
    message: &str,
    agent: Option<&str>,
    output: &OutputContext,
) {
    let agent_id = match resolve_agent(agent) {
        Ok(id) => id,
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    };

    let request = AiAgentStreamRequest {
        agent: agent_id,
        message: message.to_string(),
        system_prompt: None,
        vault_path: vault_path.to_string(),
    };

    let is_json = output.format == OutputFormat::Json;

    let result = ai_agents::run_ai_agent_stream(request, |event| {
        if is_json {
            emit_json_event(&event);
        } else {
            emit_human_event(&event);
        }
    });

    match result {
        Ok(_session_id) => {}
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    }
}

fn resolve_agent(name: Option<&str>) -> Result<AiAgentId, String> {
    match name.map(|s| s.to_ascii_lowercase()).as_deref() {
        None | Some("claude") | Some("claude_code") | Some("claude-code") => {
            Ok(AiAgentId::ClaudeCode)
        }
        Some("codex") => Ok(AiAgentId::Codex),
        Some(other) => Err(format!(
            "Unknown agent '{other}'. Available: claude, codex"
        )),
    }
}

fn emit_json_event(event: &AiAgentStreamEvent) {
    if let Ok(json) = serde_json::to_string(event) {
        println!("{json}");
    }
}

fn emit_human_event(event: &AiAgentStreamEvent) {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    match event {
        AiAgentStreamEvent::TextDelta { text } => {
            let _ = write!(out, "{text}");
            let _ = out.flush();
        }
        AiAgentStreamEvent::ThinkingDelta { text } => {
            let _ = write!(out, "{text}");
            let _ = out.flush();
        }
        AiAgentStreamEvent::ToolStart {
            tool_name,
            input,
            ..
        } => {
            let _ = writeln!(out, "\n⚙ {tool_name}");
            if let Some(ref inp) = input {
                let _ = writeln!(out, "  {inp}");
            }
        }
        AiAgentStreamEvent::ToolDone { output, .. } => {
            if let Some(ref out_text) = output {
                let truncated: String = out_text.chars().take(500).collect();
                let _ = writeln!(out, "  → {truncated}");
            }
        }
        AiAgentStreamEvent::Error { message } => {
            let _ = writeln!(out, "\nerror: {message}");
        }
        AiAgentStreamEvent::Done => {
            let _ = writeln!(out);
        }
        AiAgentStreamEvent::Init { .. } => {}
    }
}
