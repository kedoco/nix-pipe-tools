use crate::config::Config;
use crate::db::Database;
use chrono::Utc;
use std::path::Path;

pub fn generate_zsh_hook() -> String {
    r#"# prv shell integration for zsh
# Add to ~/.zshrc: eval "$(prv init --zsh)"

__prv_preexec() {
    export __PRV_CMD="$1"
    export __PRV_START=$(date +%s%3N)
}

__prv_precmd() {
    local exit_code=$?
    if [ -n "$__PRV_CMD" ]; then
        prv record --exit-code "$exit_code" -- "$__PRV_CMD"
        unset __PRV_CMD __PRV_START
    fi
}

autoload -Uz add-zsh-hook
add-zsh-hook preexec __prv_preexec
add-zsh-hook precmd __prv_precmd
"#
    .to_string()
}

pub fn generate_bash_hook() -> String {
    r#"# prv shell integration for bash
# Add to ~/.bashrc: eval "$(prv init --bash)"

__prv_debug() {
    if [ -z "$__PRV_CMD" ]; then
        export __PRV_CMD="$BASH_COMMAND"
        export __PRV_START=$(date +%s%3N)
    fi
}

__prv_prompt() {
    local exit_code=$?
    if [ -n "$__PRV_CMD" ]; then
        prv record --exit-code "$exit_code" -- "$__PRV_CMD"
        unset __PRV_CMD __PRV_START
    fi
}

trap '__prv_debug' DEBUG
PROMPT_COMMAND="__prv_prompt${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
"#
    .to_string()
}

pub fn record_command(
    command_string: &str,
    exit_code: Option<i32>,
    db: &Database,
    config: &Config,
) -> anyhow::Result<()> {
    let timestamp = Utc::now().to_rfc3339();
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let parts = parse_command(command_string);
    if parts.is_empty() {
        return Ok(());
    }

    let (cmd, args) = (&parts[0], &parts[1..]);
    let cmd_id = db.insert_command(cmd, args, &cwd, &timestamp, None, exit_code)?;

    // Heuristically detect file arguments
    let event_ts = Utc::now().to_rfc3339();
    let mut seen_redirect_out = false;
    let mut seen_redirect_in = false;

    for (i, arg) in parts.iter().enumerate() {
        if arg == ">" || arg == ">>" {
            seen_redirect_out = true;
            continue;
        }
        if arg == "<" {
            seen_redirect_in = true;
            continue;
        }

        // Handle >file (no space)
        if let Some(path) = arg.strip_prefix(">>").or_else(|| arg.strip_prefix('>')) {
            let resolved = resolve_path(path, &cwd);
            if !config.should_ignore(&resolved) {
                db.insert_file_event(cmd_id, &resolved, "write", &event_ts)?;
            }
            continue;
        }
        if let Some(path) = arg.strip_prefix('<') {
            let resolved = resolve_path(path, &cwd);
            if !config.should_ignore(&resolved) {
                db.insert_file_event(cmd_id, &resolved, "read", &event_ts)?;
            }
            continue;
        }

        if seen_redirect_out {
            seen_redirect_out = false;
            let resolved = resolve_path(arg, &cwd);
            if !config.should_ignore(&resolved) {
                db.insert_file_event(cmd_id, &resolved, "write", &event_ts)?;
            }
            continue;
        }
        if seen_redirect_in {
            seen_redirect_in = false;
            let resolved = resolve_path(arg, &cwd);
            if !config.should_ignore(&resolved) {
                db.insert_file_event(cmd_id, &resolved, "read", &event_ts)?;
            }
            continue;
        }

        // Skip the command name itself
        if i == 0 {
            continue;
        }

        // Skip flags
        if arg.starts_with('-') {
            continue;
        }

        // Check if arg looks like an existing file
        let resolved = resolve_path(arg, &cwd);
        if Path::new(&resolved).exists() {
            // Determine event type based on command
            let event_type = classify_file_access(cmd, &resolved);
            if !config.should_ignore(&resolved) {
                db.insert_file_event(cmd_id, &resolved, event_type, &event_ts)?;
            }
        }
    }

    Ok(())
}

fn resolve_path(path: &str, cwd: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("{}/{}", cwd, path)
    }
}

fn parse_command(cmd: &str) -> Vec<String> {
    // Simple shell-like word splitting (handles basic quoting)
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escaped = false;

    for c in cmd.chars() {
        if escaped {
            current.push(c);
            escaped = false;
            continue;
        }
        if c == '\\' && !in_single_quote {
            escaped = true;
            continue;
        }
        if c == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            continue;
        }
        if c == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            continue;
        }
        if c.is_whitespace() && !in_single_quote && !in_double_quote {
            if !current.is_empty() {
                parts.push(std::mem::take(&mut current));
            }
            continue;
        }
        current.push(c);
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

fn classify_file_access(command: &str, _path: &str) -> &'static str {
    // Heuristic: commands that typically write
    match command {
        "cp" | "mv" | "install" | "sed" | "patch" | "tee" => "write",
        "rm" | "unlink" => "delete",
        _ => "read",
    }
}
