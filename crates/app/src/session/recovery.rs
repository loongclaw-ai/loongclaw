#![allow(dead_code)]

use serde_json::{Value, json};

use crate::session::repository::{FinalizeSessionTerminalRequest, SessionEventRecord};

pub(crate) const RECOVERY_EVENT_KIND: &str = "delegate_recovery_applied";
pub(crate) const RECOVERY_SOURCE_EVENT: &str = "event";
pub(crate) const RECOVERY_SOURCE_LAST_ERROR: &str = "last_error";
pub(crate) const RECOVERY_SOURCE_NONE: &str = "none";

pub(crate) const RECOVERY_KIND_UNKNOWN: &str = "unknown";
pub(crate) const RECOVERY_KIND_TERMINAL_FINALIZE_PERSIST_FAILED: &str =
    "terminal_finalize_persist_failed";
pub(crate) const RECOVERY_KIND_ASYNC_SPAWN_FAILURE_PERSIST_FAILED: &str =
    "async_spawn_failure_persist_failed";
pub(crate) const RECOVERY_KIND_QUEUED_ASYNC_OVERDUE_MARKED_FAILED: &str =
    "queued_async_overdue_marked_failed";
pub(crate) const RECOVERY_KIND_RUNNING_ASYNC_OVERDUE_MARKED_FAILED: &str =
    "running_async_overdue_marked_failed";

const TERMINAL_FINALIZE_LAST_ERROR_PREFIX: &str = "delegate_terminal_finalize_failed:";
const ASYNC_SPAWN_PERSIST_LAST_ERROR_PREFIX: &str = "delegate_async_spawn_failure_persist_failed:";
const QUEUED_ASYNC_OVERDUE_LAST_ERROR_PREFIX: &str = "delegate_async_queued_overdue_marked_failed:";
const RUNNING_ASYNC_OVERDUE_LAST_ERROR_PREFIX: &str =
    "delegate_async_running_overdue_marked_failed:";

const RECOVERY_KIND_FIELD: &str = "recovery_kind";
const RECOVERED_STATE_FIELD: &str = "recovered_state";
const RECOVERY_ERROR_FIELD: &str = "recovery_error";
const ORIGINAL_ERROR_FIELD: &str = "original_error";
const ATTEMPTED_TERMINAL_EVENT_KIND_FIELD: &str = "attempted_terminal_event_kind";
const ATTEMPTED_OUTCOME_STATUS_FIELD: &str = "attempted_outcome_status";

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SessionRecoveryRecord {
    pub source: String,
    pub kind: String,
    pub event_kind: String,
    pub recovered_state: Option<String>,
    pub recovery_error: Option<String>,
    pub original_error: Option<String>,
    pub attempted_terminal_event_kind: Option<String>,
    pub attempted_outcome_status: Option<String>,
    pub ts: i64,
}

pub(crate) fn build_terminal_finalize_recovery_payload(
    request: &FinalizeSessionTerminalRequest,
    recovery_error: &str,
) -> Value {
    json!({
        RECOVERY_KIND_FIELD: RECOVERY_KIND_TERMINAL_FINALIZE_PERSIST_FAILED,
        RECOVERED_STATE_FIELD: "failed",
        RECOVERY_ERROR_FIELD: recovery_error,
        ATTEMPTED_TERMINAL_EVENT_KIND_FIELD: request.event_kind,
        ATTEMPTED_OUTCOME_STATUS_FIELD: request.outcome_status,
        "attempted_last_error": request.last_error,
    })
}

pub(crate) fn build_async_spawn_failure_recovery_payload(
    label: Option<&str>,
    original_error: &str,
    recovery_error: &str,
) -> Value {
    json!({
        RECOVERY_KIND_FIELD: RECOVERY_KIND_ASYNC_SPAWN_FAILURE_PERSIST_FAILED,
        RECOVERED_STATE_FIELD: "failed",
        RECOVERY_ERROR_FIELD: recovery_error,
        ORIGINAL_ERROR_FIELD: original_error,
        "label": label,
    })
}

pub(crate) fn build_queued_async_overdue_recovery_payload(
    label: Option<&str>,
    queued_at: i64,
    elapsed_seconds: u64,
    timeout_seconds: u64,
    deadline_at: i64,
    recovery_error: &str,
) -> Value {
    json!({
        RECOVERY_KIND_FIELD: RECOVERY_KIND_QUEUED_ASYNC_OVERDUE_MARKED_FAILED,
        RECOVERED_STATE_FIELD: "failed",
        RECOVERY_ERROR_FIELD: recovery_error,
        "label": label,
        "queued_at": queued_at,
        "elapsed_seconds": elapsed_seconds,
        "timeout_seconds": timeout_seconds,
        "deadline_at": deadline_at,
        "reference": "queued",
    })
}

pub(crate) fn build_running_async_overdue_recovery_payload(
    label: Option<&str>,
    queued_at: Option<i64>,
    started_at: Option<i64>,
    reference: &str,
    elapsed_seconds: u64,
    timeout_seconds: u64,
    deadline_at: i64,
    recovery_error: &str,
) -> Value {
    json!({
        RECOVERY_KIND_FIELD: RECOVERY_KIND_RUNNING_ASYNC_OVERDUE_MARKED_FAILED,
        RECOVERED_STATE_FIELD: "failed",
        RECOVERY_ERROR_FIELD: recovery_error,
        "label": label,
        "queued_at": queued_at,
        "started_at": started_at,
        "elapsed_seconds": elapsed_seconds,
        "timeout_seconds": timeout_seconds,
        "deadline_at": deadline_at,
        "reference": reference,
    })
}

pub(crate) fn observe_missing_recovery(
    recent_events: &[SessionEventRecord],
    last_error: Option<&str>,
) -> SessionRecoveryRecord {
    recent_events
        .iter()
        .rev()
        .find_map(parse_recovery_event)
        .unwrap_or_else(|| synthesize_recovery_from_last_error(last_error))
}

pub(crate) fn recovery_json(recovery: SessionRecoveryRecord) -> Value {
    json!({
        "source": recovery.source,
        "kind": recovery.kind,
        "event_kind": if recovery.event_kind.is_empty() {
            Value::Null
        } else {
            Value::String(recovery.event_kind)
        },
        "recovered_state": recovery.recovered_state,
        "recovery_error": recovery.recovery_error,
        "original_error": recovery.original_error,
        "attempted_terminal_event_kind": recovery.attempted_terminal_event_kind,
        "attempted_outcome_status": recovery.attempted_outcome_status,
        "ts": if recovery.ts == 0 { Value::Null } else { Value::from(recovery.ts) },
    })
}

fn parse_recovery_event(event: &SessionEventRecord) -> Option<SessionRecoveryRecord> {
    if event.event_kind != RECOVERY_EVENT_KIND {
        return None;
    }
    let kind = event
        .payload_json
        .get(RECOVERY_KIND_FIELD)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_owned();
    Some(SessionRecoveryRecord {
        source: RECOVERY_SOURCE_EVENT.to_owned(),
        kind,
        event_kind: event.event_kind.clone(),
        recovered_state: normalized_payload_string(&event.payload_json, RECOVERED_STATE_FIELD),
        recovery_error: normalized_payload_string(&event.payload_json, RECOVERY_ERROR_FIELD),
        original_error: normalized_payload_string(&event.payload_json, ORIGINAL_ERROR_FIELD),
        attempted_terminal_event_kind: normalized_payload_string(
            &event.payload_json,
            ATTEMPTED_TERMINAL_EVENT_KIND_FIELD,
        ),
        attempted_outcome_status: normalized_payload_string(
            &event.payload_json,
            ATTEMPTED_OUTCOME_STATUS_FIELD,
        ),
        ts: event.ts,
    })
}

fn normalized_payload_string(payload_json: &Value, field: &str) -> Option<String> {
    let field_value = payload_json.get(field)?;
    let field_str = field_value.as_str()?;
    let normalized = field_str.trim();
    if normalized.is_empty() {
        return None;
    }

    Some(field_str.to_owned())
}

fn synthesize_recovery_from_last_error(last_error: Option<&str>) -> SessionRecoveryRecord {
    let recovery_error = last_error
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    SessionRecoveryRecord {
        source: if recovery_error.is_some() {
            RECOVERY_SOURCE_LAST_ERROR.to_owned()
        } else {
            RECOVERY_SOURCE_NONE.to_owned()
        },
        kind: recovery_kind_from_last_error(recovery_error.as_deref()).to_owned(),
        event_kind: String::new(),
        recovered_state: None,
        recovery_error,
        original_error: None,
        attempted_terminal_event_kind: None,
        attempted_outcome_status: None,
        ts: 0,
    }
}

fn recovery_kind_from_last_error(last_error: Option<&str>) -> &'static str {
    match last_error {
        Some(last_error) if last_error.starts_with(TERMINAL_FINALIZE_LAST_ERROR_PREFIX) => {
            RECOVERY_KIND_TERMINAL_FINALIZE_PERSIST_FAILED
        }
        Some(last_error) if last_error.starts_with(ASYNC_SPAWN_PERSIST_LAST_ERROR_PREFIX) => {
            RECOVERY_KIND_ASYNC_SPAWN_FAILURE_PERSIST_FAILED
        }
        Some(last_error) if last_error.starts_with(QUEUED_ASYNC_OVERDUE_LAST_ERROR_PREFIX) => {
            RECOVERY_KIND_QUEUED_ASYNC_OVERDUE_MARKED_FAILED
        }
        Some(last_error) if last_error.starts_with(RUNNING_ASYNC_OVERDUE_LAST_ERROR_PREFIX) => {
            RECOVERY_KIND_RUNNING_ASYNC_OVERDUE_MARKED_FAILED
        }
        Some(_) | None => RECOVERY_KIND_UNKNOWN,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        RECOVERY_EVENT_KIND, RECOVERY_KIND_ASYNC_SPAWN_FAILURE_PERSIST_FAILED,
        RECOVERY_KIND_QUEUED_ASYNC_OVERDUE_MARKED_FAILED,
        RECOVERY_KIND_RUNNING_ASYNC_OVERDUE_MARKED_FAILED,
        RECOVERY_KIND_TERMINAL_FINALIZE_PERSIST_FAILED, RECOVERY_KIND_UNKNOWN,
        RECOVERY_SOURCE_EVENT, RECOVERY_SOURCE_LAST_ERROR, RECOVERY_SOURCE_NONE,
        SessionRecoveryRecord, build_async_spawn_failure_recovery_payload,
        observe_missing_recovery, recovery_json, recovery_kind_from_last_error,
    };
    use crate::session::repository::SessionEventRecord;

    fn recovery_event(payload_json: serde_json::Value, ts: i64) -> SessionEventRecord {
        SessionEventRecord {
            id: 1,
            session_id: "child-session".to_owned(),
            event_kind: RECOVERY_EVENT_KIND.to_owned(),
            actor_session_id: Some("root-session".to_owned()),
            payload_json,
            ts,
        }
    }

    #[test]
    fn build_async_spawn_failure_recovery_payload_keeps_expected_fields() {
        let payload = build_async_spawn_failure_recovery_payload(
            Some("Child"),
            "spawn panic",
            "persist failed",
        );

        assert_eq!(
            payload["recovery_kind"],
            RECOVERY_KIND_ASYNC_SPAWN_FAILURE_PERSIST_FAILED
        );
        assert_eq!(payload["recovered_state"], "failed");
        assert_eq!(payload["recovery_error"], "persist failed");
        assert_eq!(payload["original_error"], "spawn panic");
        assert_eq!(payload["label"], "Child");
    }

    #[test]
    fn observe_missing_recovery_prefers_newest_recovery_event_over_last_error() {
        let older_payload = json!({
            "recovery_kind": "terminal_finalize_persist_failed",
            "recovered_state": "failed",
            "recovery_error": "older"
        });
        let older_event = recovery_event(older_payload, 11);
        let newer_payload = json!({
            "recovery_kind": "async_spawn_failure_persist_failed",
            "recovered_state": "failed",
            "recovery_error": "newer",
            "original_error": "spawn failure"
        });
        let newer_event = recovery_event(newer_payload, 22);
        let recent_events = vec![older_event, newer_event];
        let recovery = observe_missing_recovery(
            &recent_events,
            Some("delegate_terminal_finalize_failed: fallback"),
        );

        assert_eq!(recovery.source, RECOVERY_SOURCE_EVENT);
        assert_eq!(
            recovery.kind,
            RECOVERY_KIND_ASYNC_SPAWN_FAILURE_PERSIST_FAILED
        );
        assert_eq!(recovery.recovery_error.as_deref(), Some("newer"));
        assert_eq!(recovery.original_error.as_deref(), Some("spawn failure"));
        assert_eq!(recovery.ts, 22);
    }

    #[test]
    fn observe_missing_recovery_falls_back_to_last_error_when_event_missing() {
        let recent_events = Vec::new();
        let recovery = observe_missing_recovery(
            &recent_events,
            Some("delegate_async_queued_overdue_marked_failed: timed out"),
        );

        assert_eq!(recovery.source, RECOVERY_SOURCE_LAST_ERROR);
        assert_eq!(
            recovery.kind,
            RECOVERY_KIND_QUEUED_ASYNC_OVERDUE_MARKED_FAILED
        );
        assert_eq!(
            recovery.recovery_error.as_deref(),
            Some("delegate_async_queued_overdue_marked_failed: timed out")
        );
        assert!(recovery.event_kind.is_empty());
        assert_eq!(recovery.ts, 0);
    }

    #[test]
    fn observe_missing_recovery_uses_none_source_when_metadata_is_missing() {
        let recent_events = Vec::new();
        let recovery = observe_missing_recovery(&recent_events, None);

        assert_eq!(recovery.source, RECOVERY_SOURCE_NONE);
        assert_eq!(recovery.kind, RECOVERY_KIND_UNKNOWN);
        assert!(recovery.recovery_error.is_none());
        assert!(recovery.event_kind.is_empty());
        assert_eq!(recovery.ts, 0);
    }

    #[test]
    fn recovery_kind_from_last_error_maps_known_prefixes() {
        let terminal_kind =
            recovery_kind_from_last_error(Some("delegate_terminal_finalize_failed: busy"));
        let async_spawn_kind = recovery_kind_from_last_error(Some(
            "delegate_async_spawn_failure_persist_failed: busy",
        ));
        let queued_kind = recovery_kind_from_last_error(Some(
            "delegate_async_queued_overdue_marked_failed: busy",
        ));
        let running_kind = recovery_kind_from_last_error(Some(
            "delegate_async_running_overdue_marked_failed: busy",
        ));
        let unknown_kind = recovery_kind_from_last_error(Some("opaque_failure"));

        assert_eq!(
            terminal_kind,
            RECOVERY_KIND_TERMINAL_FINALIZE_PERSIST_FAILED
        );
        assert_eq!(
            async_spawn_kind,
            RECOVERY_KIND_ASYNC_SPAWN_FAILURE_PERSIST_FAILED
        );
        assert_eq!(
            queued_kind,
            RECOVERY_KIND_QUEUED_ASYNC_OVERDUE_MARKED_FAILED
        );
        assert_eq!(
            running_kind,
            RECOVERY_KIND_RUNNING_ASYNC_OVERDUE_MARKED_FAILED
        );
        assert_eq!(unknown_kind, RECOVERY_KIND_UNKNOWN);
    }

    #[test]
    fn recovery_json_projects_null_for_empty_event_kind_and_zero_timestamp() {
        let recovery = SessionRecoveryRecord {
            source: RECOVERY_SOURCE_LAST_ERROR.to_owned(),
            kind: RECOVERY_KIND_UNKNOWN.to_owned(),
            event_kind: String::new(),
            recovered_state: None,
            recovery_error: Some("opaque_failure".to_owned()),
            original_error: None,
            attempted_terminal_event_kind: None,
            attempted_outcome_status: None,
            ts: 0,
        };
        let payload = recovery_json(recovery);

        assert_eq!(payload["source"], RECOVERY_SOURCE_LAST_ERROR);
        assert_eq!(payload["kind"], RECOVERY_KIND_UNKNOWN);
        assert!(payload["event_kind"].is_null());
        assert!(payload["ts"].is_null());
    }

    #[test]
    fn observe_missing_recovery_normalizes_blank_optional_event_fields() {
        let payload = json!({
            "recovery_kind": "terminal_finalize_persist_failed",
            "recovered_state": "",
            "recovery_error": "",
            "original_error": "",
            "attempted_terminal_event_kind": "",
            "attempted_outcome_status": ""
        });
        let event = recovery_event(payload, 33);
        let recent_events = vec![event];
        let recovery = observe_missing_recovery(&recent_events, None);

        assert_eq!(recovery.source, RECOVERY_SOURCE_EVENT);
        assert_eq!(
            recovery.kind,
            RECOVERY_KIND_TERMINAL_FINALIZE_PERSIST_FAILED
        );
        assert!(recovery.recovered_state.is_none());
        assert!(recovery.recovery_error.is_none());
        assert!(recovery.original_error.is_none());
        assert!(recovery.attempted_terminal_event_kind.is_none());
        assert!(recovery.attempted_outcome_status.is_none());
    }

    #[test]
    fn observe_missing_recovery_preserves_non_blank_optional_event_fields() {
        let payload = json!({
            "recovery_kind": "terminal_finalize_persist_failed",
            "recovered_state": " failed ",
            "recovery_error": " persist failed ",
            "original_error": " original failure ",
            "attempted_terminal_event_kind": " terminal ",
            "attempted_outcome_status": " error "
        });
        let event = recovery_event(payload, 44);
        let recent_events = vec![event];
        let recovery = observe_missing_recovery(&recent_events, None);

        assert_eq!(recovery.recovered_state.as_deref(), Some(" failed "));
        assert_eq!(recovery.recovery_error.as_deref(), Some(" persist failed "));
        assert_eq!(
            recovery.original_error.as_deref(),
            Some(" original failure ")
        );
        assert_eq!(
            recovery.attempted_terminal_event_kind.as_deref(),
            Some(" terminal ")
        );
        assert_eq!(
            recovery.attempted_outcome_status.as_deref(),
            Some(" error ")
        );
    }
}
