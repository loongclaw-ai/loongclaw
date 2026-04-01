#[cfg(feature = "memory-sqlite")]
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

#[cfg(feature = "memory-sqlite")]
use crate::conversation::ConstrainedSubagentExecution;
#[cfg(feature = "memory-sqlite")]
use crate::session::recovery::{SessionRecoveryRecord, observe_missing_recovery, recovery_json};
#[cfg(feature = "memory-sqlite")]
use crate::session::repository::{
    SessionEventRecord, SessionKind, SessionObservationRecord, SessionRepository, SessionState,
    SessionSummaryRecord, SessionTerminalOutcomeRecord,
};
#[cfg(feature = "memory-sqlite")]
use crate::session::{
    DELEGATE_CANCEL_REASON_OPERATOR_REQUESTED, DELEGATE_CANCEL_REQUESTED_EVENT_KIND,
};

#[cfg(feature = "memory-sqlite")]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SessionInspectionSnapshot {
    pub session: SessionSummaryRecord,
    pub terminal_outcome: Option<SessionTerminalOutcomeRecord>,
    pub recent_events: Vec<SessionEventRecord>,
    pub delegate_events: Vec<SessionEventRecord>,
}

#[cfg(feature = "memory-sqlite")]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SessionObservationSnapshot {
    pub inspection: SessionInspectionSnapshot,
    pub tail_events: Vec<SessionEventRecord>,
}

#[cfg(feature = "memory-sqlite")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionDelegateLifecycleRecord {
    pub mode: &'static str,
    pub phase: &'static str,
    pub queued_at: Option<i64>,
    pub started_at: Option<i64>,
    pub timeout_seconds: Option<u64>,
    pub execution: Option<ConstrainedSubagentExecution>,
    pub staleness: Option<SessionDelegateStalenessRecord>,
    pub cancellation: Option<SessionDelegateCancellationRecord>,
}

#[cfg(feature = "memory-sqlite")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionDelegateStalenessRecord {
    pub state: &'static str,
    pub reference: &'static str,
    pub elapsed_seconds: u64,
    pub threshold_seconds: u64,
    pub deadline_at: i64,
}

#[cfg(feature = "memory-sqlite")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionDelegateCancellationRecord {
    pub state: &'static str,
    pub reference: String,
    pub requested_at: i64,
    pub reason: String,
}

#[cfg(feature = "memory-sqlite")]
pub(crate) fn load_session_observation_snapshot(
    repo: &SessionRepository,
    target_session_id: &str,
    recent_event_limit: usize,
    tail_after_id: Option<i64>,
    tail_page_limit: usize,
) -> Result<SessionObservationSnapshot, String> {
    let observation = repo.load_session_observation(
        target_session_id,
        recent_event_limit,
        tail_after_id,
        tail_page_limit,
    )?;
    let SessionObservationRecord {
        session,
        terminal_outcome,
        recent_events,
        tail_events,
    } = observation.ok_or_else(|| format!("session_not_found: `{target_session_id}`"))?;
    let delegate_events = load_delegate_lifecycle_events(repo, &session)?;
    let inspection = SessionInspectionSnapshot {
        session,
        terminal_outcome,
        recent_events,
        delegate_events,
    };

    Ok(SessionObservationSnapshot {
        inspection,
        tail_events,
    })
}

#[cfg(feature = "memory-sqlite")]
pub(crate) fn session_state_is_terminal(state: SessionState) -> bool {
    matches!(
        state,
        SessionState::Completed | SessionState::Failed | SessionState::TimedOut
    )
}

#[cfg(feature = "memory-sqlite")]
pub(crate) fn session_inspection_payload(snapshot: SessionInspectionSnapshot) -> Value {
    let now_ts = current_unix_ts();

    session_inspection_payload_at(snapshot, now_ts)
}

#[cfg(feature = "memory-sqlite")]
pub(crate) fn session_inspection_payload_at(
    snapshot: SessionInspectionSnapshot,
    now_ts: i64,
) -> Value {
    let terminal_outcome_state =
        session_terminal_outcome_state(snapshot.session.state, snapshot.terminal_outcome.is_some());
    let delegate_lifecycle = session_delegate_lifecycle_at(
        &snapshot.session,
        snapshot.delegate_events.as_slice(),
        now_ts,
    );
    let recovery = match terminal_outcome_state {
        "missing" => Some(observe_missing_recovery(
            snapshot.recent_events.as_slice(),
            snapshot.session.last_error.as_deref(),
        )),
        _ => None,
    };
    let terminal_outcome_missing_reason = match terminal_outcome_state {
        "missing" => session_terminal_outcome_missing_reason(recovery.as_ref()),
        _ => None,
    };

    json!({
        "session": {
            "session_id": snapshot.session.session_id,
            "kind": snapshot.session.kind.as_str(),
            "parent_session_id": snapshot.session.parent_session_id,
            "label": snapshot.session.label,
            "state": snapshot.session.state.as_str(),
            "created_at": snapshot.session.created_at,
            "updated_at": snapshot.session.updated_at,
            "archived": snapshot.session.archived_at.is_some(),
            "archived_at": snapshot.session.archived_at,
            "last_error": snapshot.session.last_error,
        },
        "terminal_outcome_state": terminal_outcome_state,
        "terminal_outcome_missing_reason": terminal_outcome_missing_reason,
        "delegate_lifecycle": delegate_lifecycle.map(session_delegate_lifecycle_json),
        "recovery": recovery.map(recovery_json),
        "terminal_outcome": snapshot.terminal_outcome.map(session_terminal_outcome_json),
        "recent_events": snapshot
            .recent_events
            .into_iter()
            .map(session_event_json)
            .collect::<Vec<_>>(),
    })
}

#[cfg(feature = "memory-sqlite")]
pub(crate) fn session_delegate_lifecycle_at(
    session: &SessionSummaryRecord,
    recent_events: &[SessionEventRecord],
    now_ts: i64,
) -> Option<SessionDelegateLifecycleRecord> {
    if session.kind != SessionKind::DelegateChild {
        return None;
    }

    let mut queued_at = None;
    let mut started_at = None;
    let mut queued_timeout_seconds = None;
    let mut started_timeout_seconds = None;
    let mut execution = None;
    let mut cancellation = None;

    for event in recent_events {
        match event.event_kind.as_str() {
            "delegate_queued" => {
                queued_at = Some(event.ts);
                execution = execution.or_else(|| {
                    ConstrainedSubagentExecution::from_event_payload(&event.payload_json)
                });
                queued_timeout_seconds = event
                    .payload_json
                    .get("timeout_seconds")
                    .and_then(Value::as_u64)
                    .or_else(|| {
                        execution
                            .as_ref()
                            .map(|execution| execution.timeout_seconds)
                    });
            }
            "delegate_started" => {
                started_at = Some(event.ts);
                execution = execution.or_else(|| {
                    ConstrainedSubagentExecution::from_event_payload(&event.payload_json)
                });
                started_timeout_seconds = event
                    .payload_json
                    .get("timeout_seconds")
                    .and_then(Value::as_u64)
                    .or_else(|| {
                        execution
                            .as_ref()
                            .map(|execution| execution.timeout_seconds)
                    });
            }
            DELEGATE_CANCEL_REQUESTED_EVENT_KIND => {
                let reason = event
                    .payload_json
                    .get("cancel_reason")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or(DELEGATE_CANCEL_REASON_OPERATOR_REQUESTED)
                    .to_owned();
                let reference = event
                    .payload_json
                    .get("reference")
                    .and_then(Value::as_str)
                    .filter(|value| *value == "running")
                    .unwrap_or("running");
                cancellation = Some(SessionDelegateCancellationRecord {
                    state: "requested",
                    reference: reference.to_owned(),
                    requested_at: event.ts,
                    reason,
                });
            }
            _ => {}
        }
    }

    if session.parent_session_id.is_none() && queued_at.is_none() && started_at.is_none() {
        return None;
    }

    let phase = match session.state {
        SessionState::Ready => "queued",
        SessionState::Running => "running",
        SessionState::Completed => "completed",
        SessionState::Failed => "failed",
        SessionState::TimedOut => "timed_out",
    };
    let timeout_seconds = started_timeout_seconds.or(queued_timeout_seconds);
    let mode = execution
        .as_ref()
        .map(|execution| match execution.mode {
            crate::conversation::ConstrainedSubagentMode::Async => "async",
            crate::conversation::ConstrainedSubagentMode::Inline => "inline",
        })
        .unwrap_or_else(|| {
            if queued_at.is_some() || matches!(session.state, SessionState::Ready) {
                "async"
            } else {
                "inline"
            }
        });
    let staleness = match session.state {
        SessionState::Ready => {
            session_delegate_staleness_at("queued", queued_at, timeout_seconds, now_ts)
        }
        SessionState::Running => session_delegate_staleness_at(
            if started_at.is_some() {
                "started"
            } else {
                "queued"
            },
            started_at.or(queued_at),
            timeout_seconds,
            now_ts,
        ),
        SessionState::Completed | SessionState::Failed | SessionState::TimedOut => None,
    };
    let cancellation = if session.state == SessionState::Running {
        cancellation
    } else {
        None
    };

    Some(SessionDelegateLifecycleRecord {
        mode,
        phase,
        queued_at,
        started_at,
        timeout_seconds,
        execution,
        staleness,
        cancellation,
    })
}

#[cfg(feature = "memory-sqlite")]
pub(crate) fn session_delegate_lifecycle_json(lifecycle: SessionDelegateLifecycleRecord) -> Value {
    json!({
        "mode": lifecycle.mode,
        "phase": lifecycle.phase,
        "queued_at": lifecycle.queued_at,
        "started_at": lifecycle.started_at,
        "timeout_seconds": lifecycle.timeout_seconds,
        "execution": lifecycle.execution,
        "staleness": lifecycle.staleness.map(session_delegate_staleness_json),
        "cancellation": lifecycle
            .cancellation
            .map(session_delegate_cancellation_json),
    })
}

#[cfg(feature = "memory-sqlite")]
pub(crate) fn session_event_json(event: SessionEventRecord) -> Value {
    json!({
        "id": event.id,
        "session_id": event.session_id,
        "event_kind": event.event_kind,
        "actor_session_id": event.actor_session_id,
        "payload_json": event.payload_json,
        "ts": event.ts,
    })
}

#[cfg(feature = "memory-sqlite")]
pub(crate) fn session_terminal_outcome_json(outcome: SessionTerminalOutcomeRecord) -> Value {
    json!({
        "session_id": outcome.session_id,
        "status": outcome.status,
        "payload": outcome.payload_json,
        "recorded_at": outcome.recorded_at,
    })
}

#[cfg(feature = "memory-sqlite")]
pub(crate) fn load_delegate_lifecycle_events(
    repo: &SessionRepository,
    session: &SessionSummaryRecord,
) -> Result<Vec<SessionEventRecord>, String> {
    if session.kind != SessionKind::DelegateChild {
        return Ok(Vec::new());
    }

    repo.list_delegate_lifecycle_events(&session.session_id)
}

#[cfg(feature = "memory-sqlite")]
fn session_terminal_outcome_state(state: SessionState, has_terminal_outcome: bool) -> &'static str {
    if has_terminal_outcome {
        return "present";
    }
    if session_state_is_terminal(state) {
        return "missing";
    }

    "not_terminal"
}

#[cfg(feature = "memory-sqlite")]
fn session_terminal_outcome_missing_reason(
    recovery: Option<&SessionRecoveryRecord>,
) -> Option<String> {
    recovery.map(|recovery| recovery.kind.clone())
}

#[cfg(feature = "memory-sqlite")]
fn session_delegate_staleness_at(
    reference: &'static str,
    reference_at: Option<i64>,
    timeout_seconds: Option<u64>,
    now_ts: i64,
) -> Option<SessionDelegateStalenessRecord> {
    let reference_at = reference_at?;
    let threshold_seconds = timeout_seconds?;
    let elapsed_seconds = now_ts.saturating_sub(reference_at).max(0) as u64;
    let deadline_at = reference_at.saturating_add(threshold_seconds.min(i64::MAX as u64) as i64);
    let state = if elapsed_seconds > threshold_seconds {
        "overdue"
    } else {
        "fresh"
    };

    Some(SessionDelegateStalenessRecord {
        state,
        reference,
        elapsed_seconds,
        threshold_seconds,
        deadline_at,
    })
}

#[cfg(feature = "memory-sqlite")]
fn session_delegate_staleness_json(staleness: SessionDelegateStalenessRecord) -> Value {
    json!({
        "state": staleness.state,
        "reference": staleness.reference,
        "elapsed_seconds": staleness.elapsed_seconds,
        "threshold_seconds": staleness.threshold_seconds,
        "deadline_at": staleness.deadline_at,
    })
}

#[cfg(feature = "memory-sqlite")]
fn session_delegate_cancellation_json(cancellation: SessionDelegateCancellationRecord) -> Value {
    json!({
        "state": cancellation.state,
        "reference": cancellation.reference,
        "requested_at": cancellation.requested_at,
        "reason": cancellation.reason,
    })
}

#[cfg(feature = "memory-sqlite")]
fn current_unix_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        SessionInspectionSnapshot, session_inspection_payload_at, session_state_is_terminal,
    };
    use crate::session::recovery::RECOVERY_KIND_TERMINAL_FINALIZE_PERSIST_FAILED;
    use crate::session::repository::{
        SessionEventRecord, SessionKind, SessionState, SessionSummaryRecord,
    };

    fn summary(state: SessionState, last_error: Option<&str>) -> SessionSummaryRecord {
        SessionSummaryRecord {
            session_id: "child-session".to_owned(),
            kind: SessionKind::DelegateChild,
            parent_session_id: Some("root-session".to_owned()),
            label: Some("Child".to_owned()),
            state,
            created_at: 10,
            updated_at: 20,
            archived_at: None,
            turn_count: 0,
            last_turn_at: None,
            last_error: last_error.map(str::to_owned),
        }
    }

    #[test]
    fn session_state_is_terminal_only_reports_terminal_states() {
        assert!(!session_state_is_terminal(SessionState::Ready));
        assert!(!session_state_is_terminal(SessionState::Running));
        assert!(session_state_is_terminal(SessionState::Completed));
        assert!(session_state_is_terminal(SessionState::Failed));
        assert!(session_state_is_terminal(SessionState::TimedOut));
    }

    #[test]
    fn session_inspection_payload_synthesizes_recovery_for_missing_terminal_outcome() {
        let snapshot = SessionInspectionSnapshot {
            session: summary(
                SessionState::Failed,
                Some("delegate_terminal_finalize_failed: persist failed"),
            ),
            terminal_outcome: None,
            recent_events: Vec::<SessionEventRecord>::new(),
            delegate_events: Vec::<SessionEventRecord>::new(),
        };

        let payload = session_inspection_payload_at(snapshot, 200);

        assert_eq!(payload["terminal_outcome_state"], "missing");
        assert_eq!(
            payload["terminal_outcome_missing_reason"],
            RECOVERY_KIND_TERMINAL_FINALIZE_PERSIST_FAILED
        );
        assert_eq!(
            payload["recovery"]["kind"],
            RECOVERY_KIND_TERMINAL_FINALIZE_PERSIST_FAILED
        );
        assert_eq!(
            payload["recovery"]["recovery_error"],
            "delegate_terminal_finalize_failed: persist failed"
        );
    }

    #[test]
    fn session_inspection_payload_omits_recovery_for_non_terminal_session() {
        let snapshot = SessionInspectionSnapshot {
            session: summary(
                SessionState::Ready,
                Some("delegate_terminal_finalize_failed: persist failed"),
            ),
            terminal_outcome: None,
            recent_events: Vec::<SessionEventRecord>::new(),
            delegate_events: Vec::<SessionEventRecord>::new(),
        };

        let payload = session_inspection_payload_at(snapshot, 200);

        assert_eq!(payload["terminal_outcome_state"], "not_terminal");
        assert!(payload["terminal_outcome_missing_reason"].is_null());
        assert!(payload["recovery"].is_null());
    }
}
