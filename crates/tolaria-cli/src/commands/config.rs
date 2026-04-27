use tolaria_core::settings::{self, Settings};

use crate::output::{OutputContext, OutputFormat};

// ── config get ──────────────────────────────────────────────────────

pub fn run_get(key: &str, output: &OutputContext) {
    let settings = match settings::get_settings() {
        Ok(s) => s,
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    };

    let value = extract_setting(&settings, key);

    match value {
        Some(val) => {
            match output.format {
                OutputFormat::Json => {
                    output.print_json_value(&serde_json::json!({
                        "key": key,
                        "value": val,
                    }));
                }
                OutputFormat::Human => println!("{val}"),
            }
        }
        None => {
            match output.format {
                OutputFormat::Json => {
                    output.print_json_value(&serde_json::json!({
                        "key": key,
                        "value": serde_json::Value::Null,
                    }));
                }
                OutputFormat::Human => {
                    output.info(&format!("{key}: (not set)"));
                }
            }
        }
    }
}

// ── config set ──────────────────────────────────────────────────────

pub fn run_set(key: &str, value: &str, output: &OutputContext) {
    let mut settings = match settings::get_settings() {
        Ok(s) => s,
        Err(msg) => {
            output.error(&msg);
            std::process::exit(1);
        }
    };

    if let Err(msg) = apply_setting(&mut settings, key, value) {
        output.error(&msg);
        std::process::exit(1);
    }

    if let Err(msg) = settings::save_settings(settings) {
        output.error(&msg);
        std::process::exit(1);
    }

    match output.format {
        OutputFormat::Json => {
            output.print_json_value(&serde_json::json!({
                "status": "saved",
                "key": key,
                "value": value,
            }));
        }
        OutputFormat::Human => {
            output.info(&format!("{key} = {value}"));
        }
    }
}

// ── helpers ─────────────────────────────────────────────────────────

fn extract_setting(settings: &Settings, key: &str) -> Option<String> {
    match key {
        "auto_pull_interval_minutes" => settings.auto_pull_interval_minutes.map(|v| v.to_string()),
        "autogit_enabled" => settings.autogit_enabled.map(|v| v.to_string()),
        "autogit_idle_threshold_seconds" => {
            settings.autogit_idle_threshold_seconds.map(|v| v.to_string())
        }
        "autogit_inactive_threshold_seconds" => {
            settings.autogit_inactive_threshold_seconds.map(|v| v.to_string())
        }
        "telemetry_consent" => settings.telemetry_consent.map(|v| v.to_string()),
        "crash_reporting_enabled" => settings.crash_reporting_enabled.map(|v| v.to_string()),
        "analytics_enabled" => settings.analytics_enabled.map(|v| v.to_string()),
        "anonymous_id" => settings.anonymous_id.clone(),
        "release_channel" => settings.release_channel.clone(),
        "initial_h1_auto_rename_enabled" => {
            settings.initial_h1_auto_rename_enabled.map(|v| v.to_string())
        }
        "default_ai_agent" => settings.default_ai_agent.clone(),
        _ => None,
    }
}

fn apply_setting(settings: &mut Settings, key: &str, value: &str) -> Result<(), String> {
    match key {
        "auto_pull_interval_minutes" => {
            settings.auto_pull_interval_minutes = Some(parse_u32(value)?);
        }
        "autogit_enabled" => {
            settings.autogit_enabled = Some(parse_bool(value)?);
        }
        "autogit_idle_threshold_seconds" => {
            settings.autogit_idle_threshold_seconds = Some(parse_u32(value)?);
        }
        "autogit_inactive_threshold_seconds" => {
            settings.autogit_inactive_threshold_seconds = Some(parse_u32(value)?);
        }
        "telemetry_consent" => {
            settings.telemetry_consent = Some(parse_bool(value)?);
        }
        "crash_reporting_enabled" => {
            settings.crash_reporting_enabled = Some(parse_bool(value)?);
        }
        "analytics_enabled" => {
            settings.analytics_enabled = Some(parse_bool(value)?);
        }
        "anonymous_id" => {
            settings.anonymous_id = Some(value.to_string());
        }
        "release_channel" => {
            settings.release_channel = Some(value.to_string());
        }
        "initial_h1_auto_rename_enabled" => {
            settings.initial_h1_auto_rename_enabled = Some(parse_bool(value)?);
        }
        "default_ai_agent" => {
            settings.default_ai_agent = Some(value.to_string());
        }
        _ => {
            return Err(format!("Unknown setting key: {key}"));
        }
    }
    Ok(())
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(format!("Invalid boolean value: '{value}' (use true/false)")),
    }
}

fn parse_u32(value: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .map_err(|_| format!("Invalid number: '{value}'"))
}
