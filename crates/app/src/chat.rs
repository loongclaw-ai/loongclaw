use std::io::{self, Write};

use crate::context::{bootstrap_kernel_context_for_config, DEFAULT_TOKEN_TTL_S};
use crate::tools::runtime_tool_view_for_config;
use crate::CliResult;

use super::config::{self, LoongClawConfig};
use super::conversation::{ConversationTurnLoop, ProviderErrorMode, SessionContext};
#[cfg(feature = "memory-sqlite")]
use super::memory;
#[cfg(feature = "memory-sqlite")]
use super::memory::runtime_config::MemoryRuntimeConfig;

#[allow(clippy::print_stdout)] // CLI REPL output
pub async fn run_cli_chat(config_path: Option<&str>, session_hint: Option<&str>) -> CliResult<()> {
    let (resolved_path, config) = config::load(config_path)?;
    if !config.cli.enabled {
        return Err("CLI channel is disabled by config.cli.enabled=false".to_owned());
    }

    crate::runtime_env::initialize_runtime_environment(&config, Some(&resolved_path));
    let kernel_ctx = bootstrap_kernel_context_for_config("cli-chat", DEFAULT_TOKEN_TTL_S, &config)?;

    #[cfg(feature = "memory-sqlite")]
    let memory_config = MemoryRuntimeConfig {
        sqlite_path: Some(config.memory.resolved_sqlite_path()),
    };

    #[cfg(feature = "memory-sqlite")]
    {
        let sqlite_path = config.memory.resolved_sqlite_path();
        let initialized = memory::ensure_memory_db_ready(Some(sqlite_path.clone()), &memory_config)
            .map_err(|error| format!("failed to initialize sqlite memory: {error}"))?;
        println!(
            "loongclaw chat started (config={}, memory={})",
            resolved_path.display(),
            initialized.display()
        );
    }
    #[cfg(not(feature = "memory-sqlite"))]
    {
        println!(
            "loongclaw chat started (config={}, memory=disabled)",
            resolved_path.display()
        );
    }

    let session_id = session_hint
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("default")
        .to_owned();
    let session_context = SessionContext::root_with_tool_view(
        &session_id,
        runtime_tool_view_for_config(&config.tools),
    );
    println!("session={session_id} (type /help for commands, /exit to quit)");
    let turn_loop = ConversationTurnLoop::new();

    loop {
        print!("you> ");
        io::stdout()
            .flush()
            .map_err(|error| format!("flush stdout failed: {error}"))?;
        let mut line = String::new();
        let read = io::stdin()
            .read_line(&mut line)
            .map_err(|error| format!("read stdin failed: {error}"))?;
        if read == 0 {
            println!();
            break;
        }
        let input = line.trim();
        if input.is_empty() {
            continue;
        }
        if is_exit_command(&config, input) {
            break;
        }
        if input == "/help" {
            print_help();
            continue;
        }
        if input == "/history" {
            #[cfg(feature = "memory-sqlite")]
            print_history(&session_id, config.memory.sliding_window, &memory_config)?;
            #[cfg(not(feature = "memory-sqlite"))]
            print_history(&session_id, config.memory.sliding_window)?;
            continue;
        }

        let assistant_text = turn_loop
            .handle_turn_in_session(
                &config,
                &session_context,
                input,
                ProviderErrorMode::InlineMessage,
                Some(&kernel_ctx),
            )
            .await?;

        println!("loongclaw> {assistant_text}");
    }

    println!("bye.");
    Ok(())
}

fn is_exit_command(config: &LoongClawConfig, input: &str) -> bool {
    let lower = input.to_ascii_lowercase();
    config
        .cli
        .exit_commands
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .any(|value| !value.is_empty() && value == lower)
}

#[allow(clippy::print_stdout)] // CLI output
fn print_help() {
    println!("/help    show this help");
    println!("/history print current session sliding window");
    println!("/exit    quit chat");
}

#[allow(clippy::print_stdout)] // CLI output
fn print_history(
    session_id: &str,
    limit: usize,
    #[cfg(feature = "memory-sqlite")] memory_config: &MemoryRuntimeConfig,
) -> CliResult<()> {
    #[cfg(feature = "memory-sqlite")]
    {
        let turns = memory::window_direct(session_id, limit, memory_config)
            .map_err(|error| format!("load history failed: {error}"))?;
        if turns.is_empty() {
            println!("(no history yet)");
            return Ok(());
        }
        for turn in turns {
            println!("[{}] {}: {}", turn.ts, turn.role, turn.content);
        }
        Ok(())
    }

    #[cfg(not(feature = "memory-sqlite"))]
    {
        let _ = (session_id, limit);
        println!("history unavailable: memory-sqlite feature disabled");
        Ok(())
    }
}
