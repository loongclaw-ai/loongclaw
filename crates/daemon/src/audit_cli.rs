use std::collections::{BTreeMap, VecDeque};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use clap::Subcommand;

use crate::kernel::{AuditEvent, AuditEventKind};
use loongclaw_spec::CliResult;
use serde_json::{Value, json};

const MAX_AUDIT_WINDOW: usize = 10_000;

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum AuditCommands {
    /// Print the last N retained audit events
    Recent {
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Print a compact rollup over the last N retained audit events
    Summary {
        #[arg(long, default_value_t = 200)]
        limit: usize,
    },
}

#[derive(Debug, Clone)]
pub struct AuditCommandOptions {
    pub config: Option<String>,
    pub json: bool,
    pub command: AuditCommands,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditCommandExecution {
    pub resolved_config_path: String,
    pub journal_path: String,
    pub result: AuditCommandResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditCommandResult {
    Recent {
        limit: usize,
        events: Vec<AuditEvent>,
    },
    Summary {
        limit: usize,
        loaded_events: usize,
        event_kind_counts: BTreeMap<String, usize>,
        triage_counts: BTreeMap<String, usize>,
        first_timestamp_epoch_s: Option<u64>,
        last_event_id: Option<String>,
        last_timestamp_epoch_s: Option<u64>,
        last_agent_id: Option<String>,
        last_triage_event_id: Option<String>,
        last_triage_event_kind: Option<String>,
        last_triage_timestamp_epoch_s: Option<u64>,
        last_triage_agent_id: Option<String>,
    },
}

pub fn run_audit_cli(options: AuditCommandOptions) -> CliResult<()> {
    let as_json = options.json;
    let execution = execute_audit_command(options)?;
    if as_json {
        let pretty = serde_json::to_string_pretty(&audit_cli_json(&execution))
            .map_err(|error| format!("serialize audit CLI output failed: {error}"))?;
        println!("{pretty}");
        return Ok(());
    }

    println!("{}", render_audit_cli_text(&execution)?);
    Ok(())
}

pub fn execute_audit_command(options: AuditCommandOptions) -> CliResult<AuditCommandExecution> {
    let AuditCommandOptions {
        config,
        json: _,
        command,
    } = options;

    let (limit, command_name) = match &command {
        AuditCommands::Recent { limit } => (*limit, "audit recent"),
        AuditCommands::Summary { limit } => (*limit, "audit summary"),
    };
    let limit = validate_audit_limit(limit, command_name)?;

    let (resolved_path, config) = crate::mvp::config::load(config.as_deref())?;
    let journal_path = config.audit.resolved_path();
    let events = load_audit_event_window(&config.audit, &journal_path, limit)?;
    let result = match command {
        AuditCommands::Recent { limit } => AuditCommandResult::Recent { limit, events },
        AuditCommands::Summary { limit } => summarize_audit_events(limit, &events),
    };

    Ok(AuditCommandExecution {
        resolved_config_path: resolved_path.display().to_string(),
        journal_path: journal_path.display().to_string(),
        result,
    })
}

pub fn audit_cli_json(execution: &AuditCommandExecution) -> Value {
    match &execution.result {
        AuditCommandResult::Recent { limit, events } => json!({
            "command": "recent",
            "config": execution.resolved_config_path,
            "journal_path": execution.journal_path,
            "limit": limit,
            "loaded_events": events.len(),
            "events": events,
        }),
        AuditCommandResult::Summary {
            limit,
            loaded_events,
            event_kind_counts,
            triage_counts,
            first_timestamp_epoch_s,
            last_event_id,
            last_timestamp_epoch_s,
            last_agent_id,
            last_triage_event_id,
            last_triage_event_kind,
            last_triage_timestamp_epoch_s,
            last_triage_agent_id,
        } => json!({
            "command": "summary",
            "config": execution.resolved_config_path,
            "journal_path": execution.journal_path,
            "limit": limit,
            "loaded_events": loaded_events,
            "event_kind_counts": event_kind_counts,
            "triage_counts": triage_counts,
            "first_timestamp_epoch_s": first_timestamp_epoch_s,
            "last_event_id": last_event_id,
            "last_timestamp_epoch_s": last_timestamp_epoch_s,
            "last_agent_id": last_agent_id,
            "last_triage_event_id": last_triage_event_id,
            "last_triage_event_kind": last_triage_event_kind,
            "last_triage_timestamp_epoch_s": last_triage_timestamp_epoch_s,
            "last_triage_agent_id": last_triage_agent_id,
        }),
    }
}

pub fn render_audit_cli_text(execution: &AuditCommandExecution) -> CliResult<String> {
    let mut lines = Vec::new();
    match &execution.result {
        AuditCommandResult::Recent { limit, events } => {
            lines.push(format!(
                "audit recent config={} journal={} limit={} loaded_events={}",
                execution.resolved_config_path,
                execution.journal_path,
                limit,
                events.len()
            ));
            for event in events {
                let detail = format_audit_event_detail(&event.kind);
                lines.push(format!(
                    "- ts={} event_id={} agent_id={} kind={} {}",
                    event.timestamp_epoch_s,
                    event.event_id,
                    event.agent_id.as_deref().unwrap_or("-"),
                    audit_event_kind_label(&event.kind),
                    detail
                ));
            }
        }
        AuditCommandResult::Summary {
            limit,
            loaded_events,
            event_kind_counts,
            triage_counts,
            first_timestamp_epoch_s,
            last_event_id,
            last_timestamp_epoch_s,
            last_agent_id,
            last_triage_event_id,
            last_triage_event_kind,
            last_triage_timestamp_epoch_s,
            last_triage_agent_id,
        } => {
            lines.push(format!(
                "audit summary config={} journal={} limit={} loaded_events={}",
                execution.resolved_config_path, execution.journal_path, limit, loaded_events
            ));
            lines.push(format!(
                "event_kind_counts={}",
                format_equals_rollup(event_kind_counts)
            ));
            lines.push(format!(
                "triage_counts={}",
                format_equals_rollup(triage_counts)
            ));
            lines.push(format!(
                "first_timestamp_epoch_s={}",
                first_timestamp_epoch_s
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_owned())
            ));
            lines.push(format!(
                "last_event_id={} last_timestamp_epoch_s={} last_agent_id={}",
                last_event_id.as_deref().unwrap_or("-"),
                last_timestamp_epoch_s
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_owned()),
                last_agent_id.as_deref().unwrap_or("-")
            ));
            lines.push(format!(
                "last_triage_event_id={} last_triage_event_kind={} last_triage_timestamp_epoch_s={} last_triage_agent_id={}",
                last_triage_event_id.as_deref().unwrap_or("-"),
                last_triage_event_kind.as_deref().unwrap_or("-"),
                last_triage_timestamp_epoch_s
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_owned()),
                last_triage_agent_id.as_deref().unwrap_or("-")
            ));
        }
    }
    Ok(lines.join("\n"))
}

fn validate_audit_limit(limit: usize, command_name: &str) -> CliResult<usize> {
    if !(1..=MAX_AUDIT_WINDOW).contains(&limit) {
        return Err(format!(
            "{command_name} limit must be between 1 and {MAX_AUDIT_WINDOW}"
        ));
    }

    Ok(limit)
}

fn load_audit_event_window(
    audit: &crate::mvp::config::AuditConfig,
    journal_path: &Path,
    limit: usize,
) -> CliResult<Vec<AuditEvent>> {
    if !journal_path.exists() {
        let hint = if audit.mode == crate::mvp::config::AuditMode::InMemory {
            "durable audit retention is disabled because [audit].mode = \"in_memory\""
        } else {
            "journal is created on first audit write"
        };
        return Err(format!(
            "audit journal not found at {} ({hint})",
            journal_path.display()
        ));
    }
    if !journal_path.is_file() {
        return Err(format!(
            "audit journal path {} exists but is not a file",
            journal_path.display()
        ));
    }

    let file = File::open(journal_path).map_err(|error| {
        format!(
            "open audit journal {} failed: {error}",
            journal_path.display()
        )
    })?;
    file.lock_shared().map_err(|error| {
        format!(
            "lock audit journal {} for reading failed: {error}",
            journal_path.display()
        )
    })?;
    let reader = BufReader::new(file);
    let mut window = VecDeque::new();
    for (index, line_result) in reader.lines().enumerate() {
        let line_number = index + 1;
        let line = line_result.map_err(|error| {
            format!(
                "read audit journal {} failed at line {}: {error}",
                journal_path.display(),
                line_number
            )
        })?;
        let event = serde_json::from_str::<AuditEvent>(&line).map_err(|error| {
            format!(
                "decode audit journal {} failed at line {}: {error}",
                journal_path.display(),
                line_number
            )
        })?;
        if window.len() == limit {
            let _ = window.pop_front();
        }
        window.push_back(event);
    }
    Ok(window.into_iter().collect())
}

fn summarize_audit_events(limit: usize, events: &[AuditEvent]) -> AuditCommandResult {
    let mut event_kind_counts = BTreeMap::new();
    let mut triage_counts = BTreeMap::new();
    for event in events {
        let label = audit_event_kind_label(&event.kind).to_owned();
        *event_kind_counts.entry(label).or_insert(0) += 1;
        if let Some(triage_label) = triage_event_label(&event.kind) {
            *triage_counts.entry(triage_label.to_owned()).or_insert(0) += 1;
        }
    }
    let first = events.first();
    let last = events.last();
    let last_triage = events
        .iter()
        .rev()
        .find(|event| triage_event_label(&event.kind).is_some());

    AuditCommandResult::Summary {
        limit,
        loaded_events: events.len(),
        event_kind_counts,
        triage_counts,
        first_timestamp_epoch_s: first.map(|event| event.timestamp_epoch_s),
        last_event_id: last.map(|event| event.event_id.clone()),
        last_timestamp_epoch_s: last.map(|event| event.timestamp_epoch_s),
        last_agent_id: last.and_then(|event| event.agent_id.clone()),
        last_triage_event_id: last_triage.map(|event| event.event_id.clone()),
        last_triage_event_kind: last_triage
            .map(|event| audit_event_kind_label(&event.kind).to_owned()),
        last_triage_timestamp_epoch_s: last_triage.map(|event| event.timestamp_epoch_s),
        last_triage_agent_id: last_triage.and_then(|event| event.agent_id.clone()),
    }
}

fn triage_event_label(kind: &AuditEventKind) -> Option<&'static str> {
    match kind {
        AuditEventKind::AuthorizationDenied { .. } => Some("authorization_denied"),
        AuditEventKind::ProviderFailover { .. } => Some("provider_failover"),
        AuditEventKind::SecurityScanEvaluated { blocked: true, .. } => {
            Some("security_scan_blocked")
        }
        AuditEventKind::TokenIssued { .. }
        | AuditEventKind::TokenRevoked { .. }
        | AuditEventKind::TaskDispatched { .. }
        | AuditEventKind::ConnectorInvoked { .. }
        | AuditEventKind::PlaneInvoked { .. }
        | AuditEventKind::SecurityScanEvaluated { .. } => None,
        _ => None,
    }
}

fn audit_event_kind_label(kind: &AuditEventKind) -> &'static str {
    match kind {
        AuditEventKind::TokenIssued { .. } => "TokenIssued",
        AuditEventKind::TokenRevoked { .. } => "TokenRevoked",
        AuditEventKind::TaskDispatched { .. } => "TaskDispatched",
        AuditEventKind::ConnectorInvoked { .. } => "ConnectorInvoked",
        AuditEventKind::PlaneInvoked { .. } => "PlaneInvoked",
        AuditEventKind::SecurityScanEvaluated { .. } => "SecurityScanEvaluated",
        AuditEventKind::ProviderFailover { .. } => "ProviderFailover",
        AuditEventKind::AuthorizationDenied { .. } => "AuthorizationDenied",
        // AuditEventKind is non_exhaustive in loongclaw-contracts, so keep a visible
        // fallback label instead of silently collapsing future variants into "Unknown".
        _ => "UnknownAuditEventKind",
    }
}

fn format_audit_event_detail(kind: &AuditEventKind) -> String {
    match kind {
        AuditEventKind::TokenIssued { token } => format!(
            "pack_id={} token_id={} expires_at_epoch_s={}",
            token.pack_id, token.token_id, token.expires_at_epoch_s
        ),
        AuditEventKind::TokenRevoked { token_id } => format!("token_id={token_id}"),
        AuditEventKind::TaskDispatched {
            pack_id,
            task_id,
            route,
            ..
        } => format!(
            "pack_id={} task_id={} harness={:?} adapter={}",
            pack_id,
            task_id,
            route.harness_kind,
            route.adapter.as_deref().unwrap_or("-")
        ),
        AuditEventKind::ConnectorInvoked {
            pack_id,
            connector_name,
            operation,
            ..
        } => format!(
            "pack_id={} connector={} operation={}",
            pack_id, connector_name, operation
        ),
        AuditEventKind::PlaneInvoked {
            pack_id,
            plane,
            tier,
            primary_adapter,
            operation,
            ..
        } => format!(
            "pack_id={} plane={:?} tier={:?} adapter={} operation={}",
            pack_id, plane, tier, primary_adapter, operation
        ),
        AuditEventKind::SecurityScanEvaluated {
            pack_id,
            total_findings,
            blocked,
            ..
        } => format!(
            "pack_id={} total_findings={} blocked={}",
            pack_id, total_findings, blocked
        ),
        AuditEventKind::ProviderFailover {
            pack_id,
            provider_id,
            reason,
            attempt,
            max_attempts,
            ..
        } => format!(
            "pack_id={} provider_id={} reason={} attempt={}/{}",
            pack_id, provider_id, reason, attempt, max_attempts
        ),
        AuditEventKind::AuthorizationDenied {
            pack_id,
            token_id,
            reason,
        } => format!(
            "pack_id={} token_id={} reason={}",
            pack_id, token_id, reason
        ),
        _ => "detail unavailable for unknown/non-exhaustive audit event variant".to_owned(),
    }
}

fn format_equals_rollup(counts: &BTreeMap<String, usize>) -> String {
    if counts.is_empty() {
        return "-".to_owned();
    }
    counts
        .iter()
        .map(|(label, count)| format!("{label}={count}"))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::fs::OpenOptions;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use crate::kernel::{AuditEvent, AuditEventKind, CapabilityToken, ExecutionPlane, PlaneTier};
    use crate::mvp::test_support::ScopedEnv;

    use super::*;

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        static UNIQUE_COUNTER: AtomicU64 = AtomicU64::new(0);

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let counter = UNIQUE_COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("{prefix}-{}-{nanos}-{counter}", std::process::id()))
    }

    fn write_audit_config_with_mode(
        root: &Path,
        journal_path: &Path,
        mode: crate::mvp::config::AuditMode,
    ) -> PathBuf {
        fs::create_dir_all(root).expect("create config root");
        let config_path = root.join("loongclaw.toml");
        let mut config = crate::mvp::config::LoongClawConfig::default();
        config.audit.mode = mode;
        config.audit.path = journal_path.display().to_string();
        crate::mvp::config::write(Some(config_path.to_string_lossy().as_ref()), &config, true)
            .expect("write audit config");
        config_path
    }

    fn write_audit_config(root: &Path, journal_path: &Path) -> PathBuf {
        write_audit_config_with_mode(root, journal_path, crate::mvp::config::AuditMode::Fanout)
    }

    fn sample_audit_event(
        event_id: &str,
        timestamp_epoch_s: u64,
        agent_id: Option<&str>,
        kind: AuditEventKind,
    ) -> AuditEvent {
        AuditEvent {
            event_id: event_id.to_owned(),
            timestamp_epoch_s,
            agent_id: agent_id.map(str::to_owned),
            kind,
        }
    }

    fn write_journal(path: &Path, events: &[AuditEvent]) {
        let parent = path.parent().expect("journal path should have parent");
        fs::create_dir_all(parent).expect("create journal parent");
        let encoded = events
            .iter()
            .map(|event| serde_json::to_string(event).expect("serialize audit event"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(path, format!("{encoded}\n")).expect("write audit journal");
    }

    #[test]
    fn audit_recent_execution_keeps_last_events_in_order() {
        let root = unique_temp_dir("loongclaw-audit-cli-recent");
        let journal_path = root.join("audit").join("events.jsonl");
        let config_path = write_audit_config(&root, &journal_path);
        write_journal(
            &journal_path,
            &[
                sample_audit_event(
                    "evt-1",
                    1_700_010_001,
                    Some("agent-a"),
                    AuditEventKind::TokenRevoked {
                        token_id: "token-1".to_owned(),
                    },
                ),
                sample_audit_event(
                    "evt-2",
                    1_700_010_002,
                    Some("agent-b"),
                    AuditEventKind::AuthorizationDenied {
                        pack_id: "sales-intel".to_owned(),
                        token_id: "token-2".to_owned(),
                        reason: "missing capability".to_owned(),
                    },
                ),
                sample_audit_event(
                    "evt-3",
                    1_700_010_003,
                    Some("agent-c"),
                    AuditEventKind::PlaneInvoked {
                        pack_id: "sales-intel".to_owned(),
                        plane: ExecutionPlane::Tool,
                        tier: PlaneTier::Core,
                        primary_adapter: "mvp-tools".to_owned(),
                        delegated_core_adapter: None,
                        operation: "tool.call".to_owned(),
                        required_capabilities: Vec::new(),
                    },
                ),
            ],
        );

        let execution = execute_audit_command(AuditCommandOptions {
            config: Some(config_path.display().to_string()),
            json: false,
            command: AuditCommands::Recent { limit: 2 },
        })
        .expect("execute audit recent");

        assert_eq!(execution.journal_path, journal_path.display().to_string());
        match execution.result {
            AuditCommandResult::Recent { limit, events } => {
                assert_eq!(limit, 2);
                let ids = events
                    .iter()
                    .map(|event| event.event_id.as_str())
                    .collect::<Vec<_>>();
                assert_eq!(ids, vec!["evt-2", "evt-3"]);
            }
            other => panic!("unexpected audit command result: {other:?}"),
        }
    }

    #[test]
    fn audit_recent_json_includes_loaded_events_and_journal_path() {
        let root = unique_temp_dir("loongclaw-audit-cli-recent-json");
        let journal_path = root.join("audit").join("events.jsonl");
        let config_path = write_audit_config(&root, &journal_path);
        write_journal(
            &journal_path,
            &[sample_audit_event(
                "evt-json",
                1_700_010_010,
                Some("agent-json"),
                AuditEventKind::TokenRevoked {
                    token_id: "token-json".to_owned(),
                },
            )],
        );

        let execution = execute_audit_command(AuditCommandOptions {
            config: Some(config_path.display().to_string()),
            json: true,
            command: AuditCommands::Recent { limit: 5 },
        })
        .expect("execute audit recent");
        let payload = audit_cli_json(&execution);

        assert_eq!(payload["journal_path"], journal_path.display().to_string());
        assert_eq!(payload["limit"], 5);
        assert_eq!(payload["loaded_events"], 1);
        assert_eq!(payload["events"][0]["event_id"], "evt-json");
    }

    #[test]
    fn audit_recent_waits_for_existing_audit_journal_lock_before_reading() {
        let root = unique_temp_dir("loongclaw-audit-cli-recent-lock");
        let journal_path = root.join("audit").join("events.jsonl");
        let config_path = write_audit_config(&root, &journal_path);
        write_journal(
            &journal_path,
            &[sample_audit_event(
                "evt-lock",
                1_700_010_020,
                Some("agent-lock"),
                AuditEventKind::TokenRevoked {
                    token_id: "token-lock".to_owned(),
                },
            )],
        );

        let external_lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&journal_path)
            .expect("open external audit journal handle");
        external_lock
            .lock()
            .expect("hold external audit journal lock");

        let (tx, rx) = mpsc::channel();
        let config = config_path.display().to_string();
        let handle = thread::spawn(move || {
            let result = execute_audit_command(AuditCommandOptions {
                config: Some(config),
                json: false,
                command: AuditCommands::Recent { limit: 10 },
            });
            tx.send(result).expect("send audit recent result");
        });

        match rx.recv_timeout(Duration::from_millis(100)) {
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Ok(result) => {
                panic!("audit recent should block on external journal lock, got {result:?}")
            }
            Err(error) => panic!("audit recent channel closed unexpectedly: {error:?}"),
        }

        external_lock
            .unlock()
            .expect("release external audit journal lock");
        let execution = rx
            .recv_timeout(Duration::from_secs(1))
            .expect("audit recent should finish after lock release")
            .expect("audit recent should succeed after lock release");
        handle.join().expect("join audit recent reader");

        match execution.result {
            AuditCommandResult::Recent { events, .. } => {
                assert_eq!(events.len(), 1);
                assert_eq!(events[0].event_id, "evt-lock");
            }
            other => panic!("unexpected audit command result after lock release: {other:?}"),
        }
    }

    #[test]
    fn audit_recent_rejects_zero_limit() {
        let mut env = ScopedEnv::new();
        env.set("HOME", unique_temp_dir("loongclaw-audit-cli-missing-home"));

        let error = execute_audit_command(AuditCommandOptions {
            config: None,
            json: false,
            command: AuditCommands::Recent { limit: 0 },
        })
        .expect_err("zero recent limit should fail");

        assert!(error.contains("audit recent limit must be between 1 and 10000"));
    }

    #[test]
    fn audit_recent_rejects_excessive_limit() {
        let mut env = ScopedEnv::new();
        env.set(
            "HOME",
            unique_temp_dir("loongclaw-audit-cli-large-limit-home"),
        );

        let error = execute_audit_command(AuditCommandOptions {
            config: None,
            json: false,
            command: AuditCommands::Recent { limit: 10_001 },
        })
        .expect_err("excessive recent limit should fail");

        assert!(error.contains("audit recent limit must be between 1 and 10000"));
    }

    #[test]
    fn audit_recent_reports_missing_journal_with_first_write_hint() {
        let root = unique_temp_dir("loongclaw-audit-cli-missing");
        let journal_path = root.join("audit").join("events.jsonl");
        let config_path = write_audit_config(&root, &journal_path);

        let error = execute_audit_command(AuditCommandOptions {
            config: Some(config_path.display().to_string()),
            json: false,
            command: AuditCommands::Recent { limit: 10 },
        })
        .expect_err("missing journal should fail");

        assert!(error.contains("audit journal not found"));
        assert!(error.contains("first audit write"));
    }

    #[test]
    fn audit_recent_reports_in_memory_mode_when_journal_is_missing() {
        let root = unique_temp_dir("loongclaw-audit-cli-in-memory");
        let journal_path = root.join("audit").join("events.jsonl");
        let config_path = write_audit_config_with_mode(
            &root,
            &journal_path,
            crate::mvp::config::AuditMode::InMemory,
        );

        let error = execute_audit_command(AuditCommandOptions {
            config: Some(config_path.display().to_string()),
            json: false,
            command: AuditCommands::Recent { limit: 10 },
        })
        .expect_err("missing in-memory journal should fail");

        assert!(error.contains("audit journal not found"));
        assert!(error.contains("durable audit retention is disabled"));
        assert!(error.contains("[audit].mode = \"in_memory\""));
    }

    #[test]
    fn audit_summary_rolls_up_event_kinds_and_last_seen_fields() {
        let root = unique_temp_dir("loongclaw-audit-cli-summary");
        let journal_path = root.join("audit").join("events.jsonl");
        let config_path = write_audit_config(&root, &journal_path);
        write_journal(
            &journal_path,
            &[
                sample_audit_event(
                    "evt-1",
                    1_700_010_100,
                    Some("agent-a"),
                    AuditEventKind::TokenIssued {
                        token: CapabilityToken {
                            token_id: "token-0".to_owned(),
                            pack_id: "sales-intel".to_owned(),
                            agent_id: "agent-a".to_owned(),
                            allowed_capabilities: Default::default(),
                            issued_at_epoch_s: 1_700_010_100,
                            expires_at_epoch_s: 1_700_010_200,
                            generation: 0,
                        },
                    },
                ),
                sample_audit_event(
                    "evt-2",
                    1_700_010_101,
                    Some("agent-b"),
                    AuditEventKind::AuthorizationDenied {
                        pack_id: "sales-intel".to_owned(),
                        token_id: "token-1".to_owned(),
                        reason: "missing capability".to_owned(),
                    },
                ),
                sample_audit_event(
                    "evt-3",
                    1_700_010_102,
                    Some("agent-c"),
                    AuditEventKind::ProviderFailover {
                        pack_id: "sales-intel".to_owned(),
                        provider_id: "openai".to_owned(),
                        reason: "rate_limited".to_owned(),
                        stage: "response".to_owned(),
                        model: "gpt-5.1".to_owned(),
                        attempt: 1,
                        max_attempts: 3,
                        status_code: Some(429),
                        try_next_model: true,
                        auto_model_mode: true,
                        candidate_index: 0,
                        candidate_count: 2,
                    },
                ),
                sample_audit_event(
                    "evt-4",
                    1_700_010_103,
                    Some("agent-d"),
                    AuditEventKind::SecurityScanEvaluated {
                        pack_id: "sales-intel".to_owned(),
                        scanned_plugins: 1,
                        total_findings: 2,
                        high_findings: 1,
                        medium_findings: 1,
                        low_findings: 0,
                        blocked: true,
                        block_reason: Some("unsigned plugin".to_owned()),
                        categories: vec!["signature".to_owned()],
                        finding_ids: vec!["finding-1".to_owned()],
                    },
                ),
                sample_audit_event(
                    "evt-5",
                    1_700_010_104,
                    Some("agent-e"),
                    AuditEventKind::PlaneInvoked {
                        pack_id: "sales-intel".to_owned(),
                        plane: ExecutionPlane::Runtime,
                        tier: PlaneTier::Core,
                        primary_adapter: "runtime".to_owned(),
                        delegated_core_adapter: None,
                        operation: "turn.complete".to_owned(),
                        required_capabilities: Vec::new(),
                    },
                ),
            ],
        );

        let execution = execute_audit_command(AuditCommandOptions {
            config: Some(config_path.display().to_string()),
            json: false,
            command: AuditCommands::Summary { limit: 10 },
        })
        .expect("execute audit summary");

        match execution.result {
            AuditCommandResult::Summary {
                limit,
                loaded_events,
                event_kind_counts,
                triage_counts,
                first_timestamp_epoch_s,
                last_event_id,
                last_timestamp_epoch_s,
                last_agent_id,
                last_triage_event_id,
                last_triage_event_kind,
                last_triage_timestamp_epoch_s,
                last_triage_agent_id,
            } => {
                assert_eq!(limit, 10);
                assert_eq!(loaded_events, 5);
                assert_eq!(
                    event_kind_counts,
                    BTreeMap::from([
                        ("AuthorizationDenied".to_owned(), 1_usize),
                        ("PlaneInvoked".to_owned(), 1_usize),
                        ("ProviderFailover".to_owned(), 1_usize),
                        ("SecurityScanEvaluated".to_owned(), 1_usize),
                        ("TokenIssued".to_owned(), 1_usize),
                    ])
                );
                assert_eq!(
                    triage_counts,
                    BTreeMap::from([
                        ("authorization_denied".to_owned(), 1_usize),
                        ("provider_failover".to_owned(), 1_usize),
                        ("security_scan_blocked".to_owned(), 1_usize),
                    ])
                );
                assert_eq!(first_timestamp_epoch_s, Some(1_700_010_100));
                assert_eq!(last_event_id.as_deref(), Some("evt-5"));
                assert_eq!(last_timestamp_epoch_s, Some(1_700_010_104));
                assert_eq!(last_agent_id.as_deref(), Some("agent-e"));
                assert_eq!(last_triage_event_id.as_deref(), Some("evt-4"));
                assert_eq!(
                    last_triage_event_kind.as_deref(),
                    Some("SecurityScanEvaluated")
                );
                assert_eq!(last_triage_timestamp_epoch_s, Some(1_700_010_103));
                assert_eq!(last_triage_agent_id.as_deref(), Some("agent-d"));
            }
            other => panic!("unexpected audit command result: {other:?}"),
        }
    }

    #[test]
    fn audit_summary_ignores_non_blocking_security_scan_for_triage_rollups() {
        let root = unique_temp_dir("loongclaw-audit-cli-summary-non-blocking-scan");
        let journal_path = root.join("audit").join("events.jsonl");
        let config_path = write_audit_config(&root, &journal_path);
        write_journal(
            &journal_path,
            &[
                sample_audit_event(
                    "evt-1",
                    1_700_010_150,
                    Some("agent-a"),
                    AuditEventKind::AuthorizationDenied {
                        pack_id: "sales-intel".to_owned(),
                        token_id: "token-1".to_owned(),
                        reason: "missing capability".to_owned(),
                    },
                ),
                sample_audit_event(
                    "evt-2",
                    1_700_010_151,
                    Some("agent-b"),
                    AuditEventKind::SecurityScanEvaluated {
                        pack_id: "sales-intel".to_owned(),
                        scanned_plugins: 1,
                        total_findings: 1,
                        high_findings: 0,
                        medium_findings: 1,
                        low_findings: 0,
                        blocked: false,
                        block_reason: None,
                        categories: vec!["signature".to_owned()],
                        finding_ids: vec!["finding-1".to_owned()],
                    },
                ),
                sample_audit_event(
                    "evt-3",
                    1_700_010_152,
                    Some("agent-c"),
                    AuditEventKind::PlaneInvoked {
                        pack_id: "sales-intel".to_owned(),
                        plane: ExecutionPlane::Runtime,
                        tier: PlaneTier::Core,
                        primary_adapter: "runtime".to_owned(),
                        delegated_core_adapter: None,
                        operation: "turn.complete".to_owned(),
                        required_capabilities: Vec::new(),
                    },
                ),
            ],
        );

        let execution = execute_audit_command(AuditCommandOptions {
            config: Some(config_path.display().to_string()),
            json: false,
            command: AuditCommands::Summary { limit: 10 },
        })
        .expect("execute audit summary");

        match execution.result {
            AuditCommandResult::Summary {
                triage_counts,
                last_triage_event_id,
                last_triage_event_kind,
                last_triage_timestamp_epoch_s,
                last_triage_agent_id,
                ..
            } => {
                assert_eq!(
                    triage_counts,
                    BTreeMap::from([("authorization_denied".to_owned(), 1_usize)])
                );
                assert_eq!(last_triage_event_id.as_deref(), Some("evt-1"));
                assert_eq!(
                    last_triage_event_kind.as_deref(),
                    Some("AuthorizationDenied")
                );
                assert_eq!(last_triage_timestamp_epoch_s, Some(1_700_010_150));
                assert_eq!(last_triage_agent_id.as_deref(), Some("agent-a"));
            }
            other => panic!("unexpected audit command result: {other:?}"),
        }
    }

    #[test]
    fn audit_summary_rejects_excessive_limit() {
        let mut env = ScopedEnv::new();
        env.set(
            "HOME",
            unique_temp_dir("loongclaw-audit-cli-large-summary-limit-home"),
        );

        let error = execute_audit_command(AuditCommandOptions {
            config: None,
            json: false,
            command: AuditCommands::Summary { limit: 10_001 },
        })
        .expect_err("excessive summary limit should fail");

        assert!(error.contains("audit summary limit must be between 1 and 10000"));
    }

    #[test]
    fn audit_summary_text_includes_triage_counts_and_last_seen_fields() {
        let execution = AuditCommandExecution {
            resolved_config_path: "/tmp/loongclaw.toml".to_owned(),
            journal_path: "/tmp/audit/events.jsonl".to_owned(),
            result: AuditCommandResult::Summary {
                limit: 50,
                loaded_events: 3,
                event_kind_counts: BTreeMap::from([
                    ("AuthorizationDenied".to_owned(), 2_usize),
                    ("PlaneInvoked".to_owned(), 1_usize),
                ]),
                triage_counts: BTreeMap::from([
                    ("authorization_denied".to_owned(), 2_usize),
                    ("security_scan_blocked".to_owned(), 1_usize),
                ]),
                first_timestamp_epoch_s: Some(1_700_010_100),
                last_event_id: Some("evt-3".to_owned()),
                last_timestamp_epoch_s: Some(1_700_010_102),
                last_agent_id: Some("agent-c".to_owned()),
                last_triage_event_id: Some("evt-2".to_owned()),
                last_triage_event_kind: Some("AuthorizationDenied".to_owned()),
                last_triage_timestamp_epoch_s: Some(1_700_010_101),
                last_triage_agent_id: Some("agent-b".to_owned()),
            },
        };

        let rendered = render_audit_cli_text(&execution).expect("render audit summary");

        assert!(rendered.contains("audit summary"));
        assert!(rendered.contains("loaded_events=3"));
        assert!(rendered.contains("first_timestamp_epoch_s=1700010100"));
        assert!(rendered.contains("AuthorizationDenied=2"));
        assert!(rendered.contains("PlaneInvoked=1"));
        assert!(rendered.contains("triage_counts=authorization_denied=2,security_scan_blocked=1"));
        assert!(rendered.contains("last_event_id=evt-3"));
        assert!(rendered.contains("last_agent_id=agent-c"));
        assert!(rendered.contains("last_triage_event_id=evt-2"));
        assert!(rendered.contains("last_triage_event_kind=AuthorizationDenied"));
        assert!(rendered.contains("last_triage_agent_id=agent-b"));
    }

    #[test]
    fn audit_summary_json_includes_triage_fields() {
        let execution = AuditCommandExecution {
            resolved_config_path: "/tmp/loongclaw.toml".to_owned(),
            journal_path: "/tmp/audit/events.jsonl".to_owned(),
            result: AuditCommandResult::Summary {
                limit: 25,
                loaded_events: 2,
                event_kind_counts: BTreeMap::from([
                    ("AuthorizationDenied".to_owned(), 1_usize),
                    ("PlaneInvoked".to_owned(), 1_usize),
                ]),
                triage_counts: BTreeMap::from([("authorization_denied".to_owned(), 1_usize)]),
                first_timestamp_epoch_s: Some(1_700_010_200),
                last_event_id: Some("evt-2".to_owned()),
                last_timestamp_epoch_s: Some(1_700_010_201),
                last_agent_id: Some("agent-b".to_owned()),
                last_triage_event_id: Some("evt-1".to_owned()),
                last_triage_event_kind: Some("AuthorizationDenied".to_owned()),
                last_triage_timestamp_epoch_s: Some(1_700_010_200),
                last_triage_agent_id: Some("agent-a".to_owned()),
            },
        };

        let payload = audit_cli_json(&execution);

        assert_eq!(payload["first_timestamp_epoch_s"], 1_700_010_200_u64);
        assert_eq!(payload["triage_counts"]["authorization_denied"], 1);
        assert_eq!(payload["last_triage_event_id"], "evt-1");
        assert_eq!(payload["last_triage_event_kind"], "AuthorizationDenied");
        assert_eq!(payload["last_triage_timestamp_epoch_s"], 1_700_010_200_u64);
        assert_eq!(payload["last_triage_agent_id"], "agent-a");
    }

    #[test]
    fn audit_summary_json_uses_empty_and_null_triage_fields_when_no_triage_events_exist() {
        let execution = AuditCommandExecution {
            resolved_config_path: "/tmp/loongclaw.toml".to_owned(),
            journal_path: "/tmp/audit/events.jsonl".to_owned(),
            result: AuditCommandResult::Summary {
                limit: 10,
                loaded_events: 1,
                event_kind_counts: BTreeMap::from([("TokenIssued".to_owned(), 1_usize)]),
                triage_counts: BTreeMap::new(),
                first_timestamp_epoch_s: Some(1_700_010_300),
                last_event_id: Some("evt-1".to_owned()),
                last_timestamp_epoch_s: Some(1_700_010_300),
                last_agent_id: Some("agent-a".to_owned()),
                last_triage_event_id: None,
                last_triage_event_kind: None,
                last_triage_timestamp_epoch_s: None,
                last_triage_agent_id: None,
            },
        };

        let payload = audit_cli_json(&execution);

        assert_eq!(
            payload["triage_counts"].as_object(),
            Some(&serde_json::Map::new())
        );
        assert_eq!(payload["last_triage_event_id"], Value::Null);
        assert_eq!(payload["last_triage_event_kind"], Value::Null);
        assert_eq!(payload["last_triage_timestamp_epoch_s"], Value::Null);
        assert_eq!(payload["last_triage_agent_id"], Value::Null);
    }

    #[test]
    fn audit_summary_text_uses_placeholders_when_no_triage_events_exist() {
        let execution = AuditCommandExecution {
            resolved_config_path: "/tmp/loongclaw.toml".to_owned(),
            journal_path: "/tmp/audit/events.jsonl".to_owned(),
            result: AuditCommandResult::Summary {
                limit: 10,
                loaded_events: 1,
                event_kind_counts: BTreeMap::from([("TokenIssued".to_owned(), 1_usize)]),
                triage_counts: BTreeMap::new(),
                first_timestamp_epoch_s: Some(1_700_010_300),
                last_event_id: Some("evt-1".to_owned()),
                last_timestamp_epoch_s: Some(1_700_010_300),
                last_agent_id: Some("agent-a".to_owned()),
                last_triage_event_id: None,
                last_triage_event_kind: None,
                last_triage_timestamp_epoch_s: None,
                last_triage_agent_id: None,
            },
        };

        let rendered = render_audit_cli_text(&execution).expect("render audit summary");

        assert!(rendered.contains("triage_counts=-"));
        assert!(rendered.contains("last_triage_event_id=-"));
        assert!(rendered.contains("last_triage_event_kind=-"));
    }
}
