use crate::CliResult;
#[cfg(feature = "memory-sqlite")]
use crate::session::repository::{ApprovalRequestRecord, SessionRepository};
#[cfg(feature = "memory-sqlite")]
use crate::session::store::SessionStoreConfig;
use serde_json::Value;

pub(crate) const CHAT_SESSION_KIND_DELEGATE_CHILD: &str = "delegate_child";
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ChatControlPlaneApprovalSummary {
    pub(crate) approval_request_id: String,
    pub(crate) status: String,
    pub(crate) tool_name: String,
    pub(crate) visible_tool_name: String,
    pub(crate) request_summary: Value,
    pub(crate) turn_id: String,
    pub(crate) requested_at: i64,
    pub(crate) reason: Option<String>,
    pub(crate) rule_id: Option<String>,
    pub(crate) last_error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ChatControlPlaneSessionSummary {
    pub(crate) session_id: String,
    pub(crate) label: String,
    pub(crate) explicit_label: Option<String>,
    pub(crate) state: String,
    pub(crate) kind: String,
    pub(crate) parent_session_id: Option<String>,
    pub(crate) turn_count: usize,
    pub(crate) updated_at: i64,
    pub(crate) last_turn_at: Option<i64>,
    pub(crate) first_user_message: Option<String>,
    pub(crate) terminal_status: Option<String>,
    pub(crate) last_turn_excerpt: Option<String>,
    pub(crate) last_error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ChatControlPlaneSessionDetails {
    pub(crate) lineage_root_session_id: Option<String>,
    pub(crate) lineage_depth: usize,
    pub(crate) trajectory_turn_count: usize,
    pub(crate) event_count: usize,
    pub(crate) approval_count: usize,
    pub(crate) terminal_status: Option<String>,
    pub(crate) terminal_recorded_at: Option<i64>,
    pub(crate) last_turn_role: Option<String>,
    pub(crate) last_turn_excerpt: Option<String>,
    pub(crate) last_turn_ts: Option<i64>,
    pub(crate) recent_events: Vec<String>,
    pub(crate) delegate_events: Vec<String>,
}

#[cfg(feature = "memory-sqlite")]
pub(crate) struct ChatControlPlaneStore {
    repo: SessionRepository,
}

#[cfg(not(feature = "memory-sqlite"))]
pub(crate) struct ChatControlPlaneStore;

impl ChatControlPlaneApprovalSummary {
    #[cfg(feature = "memory-sqlite")]
    fn from_record(record: &ApprovalRequestRecord) -> Self {
        let reason = record
            .governance_snapshot_json
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let rule_id = record
            .governance_snapshot_json
            .get("rule_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let status = record.status.as_str().to_owned();
        let raw_request = record
            .request_payload_json
            .as_object()
            .and_then(|payload| payload.get("args_json"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let request_summary = crate::tools::summarize_tool_request_for_display(
            record.tool_name.as_str(),
            raw_request,
        );

        Self {
            approval_request_id: record.approval_request_id.clone(),
            status,
            tool_name: record.tool_name.clone(),
            visible_tool_name: crate::tools::user_visible_tool_name(record.tool_name.as_str()),
            request_summary,
            turn_id: record.turn_id.clone(),
            requested_at: record.requested_at,
            reason,
            rule_id,
            last_error: record.last_error.clone(),
        }
    }
}

impl ChatControlPlaneSessionSummary {
    #[cfg(feature = "memory-sqlite")]
    fn from_session_summary(summary: &crate::session::repository::SessionSummaryRecord) -> Self {
        let label = derive_resume_session_label(summary);
        let explicit_label = summary
            .label
            .as_deref()
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .map(ToOwned::to_owned);
        let state = summary.state.as_str().to_owned();
        let kind = summary.kind.as_str().to_owned();

        Self {
            session_id: summary.session_id.clone(),
            label,
            explicit_label,
            state,
            kind,
            parent_session_id: summary.parent_session_id.clone(),
            turn_count: summary.turn_count,
            updated_at: summary.updated_at,
            last_turn_at: summary.last_turn_at,
            first_user_message: summary.first_user_message.clone(),
            terminal_status: None,
            last_turn_excerpt: None,
            last_error: summary.last_error.clone(),
        }
    }
}

fn derive_resume_session_label(
    summary: &crate::session::repository::SessionSummaryRecord,
) -> String {
    if let Some(label) = summary
        .label
        .as_deref()
        .map(str::trim)
        .filter(|label| !label.is_empty())
    {
        return label.to_owned();
    }
    if summary.kind == crate::session::repository::SessionKind::Root {
        if summary.session_id == "default" {
            return "legacy chat".to_owned();
        }
        if summary.session_id.starts_with("chat-") {
            return "chat".to_owned();
        }
        return "session".to_owned();
    }
    summary.kind.as_str().to_owned()
}

impl ChatControlPlaneStore {
    #[cfg(feature = "memory-sqlite")]
    pub(crate) fn new(memory_config: &SessionStoreConfig) -> CliResult<Self> {
        let repo = SessionRepository::new(memory_config)?;
        Ok(Self { repo })
    }

    #[cfg(not(feature = "memory-sqlite"))]
    pub(crate) fn new<T>(_memory_config: &T) -> CliResult<Self> {
        Err("control plane requires memory-sqlite support".to_owned())
    }

    #[cfg(feature = "memory-sqlite")]
    pub(crate) fn visible_sessions(
        &self,
        scope_session_id: &str,
        limit: usize,
    ) -> CliResult<Vec<ChatControlPlaneSessionSummary>> {
        let visible_sessions = self.repo.list_visible_sessions(scope_session_id)?;
        let limited_sessions = visible_sessions
            .into_iter()
            .filter(|session| session.archived_at.is_none())
            .take(limit);
        let mut summaries = Vec::new();

        for session in limited_sessions {
            let summary = ChatControlPlaneSessionSummary::from_session_summary(&session);
            summaries.push(summary);
        }

        Ok(summaries)
    }

    #[cfg(feature = "memory-sqlite")]
    pub(crate) fn recent_resumable_root_sessions(
        &self,
        limit: usize,
    ) -> CliResult<Vec<ChatControlPlaneSessionSummary>> {
        let sessions = self.repo.list_recent_resumable_root_session_summaries(limit)?;
        let mut summaries = Vec::new();

        for session in sessions {
            let mut summary = ChatControlPlaneSessionSummary::from_session_summary(&session);
            if let Some(details) = self.session_details(session.session_id.as_str(), false)? {
                summary.terminal_status = details.terminal_status;
                summary.last_turn_excerpt = details.last_turn_excerpt;
            }
            summaries.push(summary);
        }

        Ok(summaries)
    }

    #[cfg(not(feature = "memory-sqlite"))]
    pub(crate) fn visible_sessions(
        &self,
        _scope_session_id: &str,
        _limit: usize,
    ) -> CliResult<Vec<ChatControlPlaneSessionSummary>> {
        Err("control plane requires memory-sqlite support".to_owned())
    }

    #[cfg(not(feature = "memory-sqlite"))]
    pub(crate) fn recent_resumable_root_sessions(
        &self,
        _limit: usize,
    ) -> CliResult<Vec<ChatControlPlaneSessionSummary>> {
        Err("control plane requires memory-sqlite support".to_owned())
    }

    #[cfg(feature = "memory-sqlite")]
    pub(crate) fn visible_worker_sessions(
        &self,
        scope_session_id: &str,
        limit: usize,
    ) -> CliResult<Vec<ChatControlPlaneSessionSummary>> {
        let visible_sessions = self.visible_sessions(scope_session_id, usize::MAX)?;
        let mut workers = Vec::new();

        for session in visible_sessions {
            if session.kind != CHAT_SESSION_KIND_DELEGATE_CHILD {
                continue;
            }
            workers.push(session);
            if workers.len() >= limit {
                break;
            }
        }

        Ok(workers)
    }

    #[cfg(not(feature = "memory-sqlite"))]
    pub(crate) fn visible_worker_sessions(
        &self,
        _scope_session_id: &str,
        _limit: usize,
    ) -> CliResult<Vec<ChatControlPlaneSessionSummary>> {
        Err("control plane requires memory-sqlite support".to_owned())
    }

    #[cfg(feature = "memory-sqlite")]
    pub(crate) fn approval_queue(
        &self,
        session_id: &str,
        limit: usize,
    ) -> CliResult<Vec<ChatControlPlaneApprovalSummary>> {
        let records = self
            .repo
            .list_approval_requests_for_session(session_id, None)?;
        let limited_records = records.into_iter().take(limit);
        let mut approvals = Vec::new();

        for record in limited_records {
            let summary = ChatControlPlaneApprovalSummary::from_record(&record);
            approvals.push(summary);
        }

        Ok(approvals)
    }

    #[cfg(not(feature = "memory-sqlite"))]
    pub(crate) fn approval_queue(
        &self,
        _session_id: &str,
        _limit: usize,
    ) -> CliResult<Vec<ChatControlPlaneApprovalSummary>> {
        Err("control plane requires memory-sqlite support".to_owned())
    }

    #[cfg(feature = "memory-sqlite")]
    pub(crate) fn session_details(
        &self,
        session_id: &str,
        include_delegate_lifecycle: bool,
    ) -> CliResult<Option<ChatControlPlaneSessionDetails>> {
        let snapshot_result = self
            .repo
            .load_session_trajectory_read_snapshot(session_id, 12)?;
        let snapshot = match snapshot_result {
            Some(snapshot) => snapshot,
            None => return Ok(None),
        };

        let trajectory_turn_count = snapshot.turns.len();
        let event_count = snapshot.events.len();
        let approval_count = snapshot.approval_requests.len();
        let terminal_status = snapshot
            .terminal_outcome
            .as_ref()
            .map(|outcome| outcome.status.clone());
        let terminal_recorded_at = snapshot
            .terminal_outcome
            .as_ref()
            .map(|outcome| outcome.recorded_at);
        let last_turn_role = snapshot.turns.last().map(|turn| turn.role.clone());
        let last_turn_ts = snapshot.turns.last().map(|turn| turn.ts);
        let last_turn_excerpt = snapshot
            .turns
            .iter()
            .rev()
            .find_map(|turn| visible_resume_excerpt(turn.role.as_str(), turn.content.as_str()));
        let recent_event_lines = snapshot
            .events
            .iter()
            .map(|event| {
                let event_id = event.id;
                let event_kind = event.event_kind.as_str();
                format!("event#{event_id}={event_kind}")
            })
            .collect::<Vec<_>>();
        let recent_events = build_recent_event_lines(recent_event_lines);

        let delegate_events = if include_delegate_lifecycle {
            let lifecycle_events = self.repo.list_delegate_lifecycle_events(session_id)?;
            let delegate_event_lines = lifecycle_events
                .iter()
                .map(|event| {
                    let event_id = event.id;
                    let event_kind = event.event_kind.as_str();
                    format!("delegate_event#{event_id}={event_kind}")
                })
                .collect::<Vec<_>>();
            build_recent_event_lines(delegate_event_lines)
        } else {
            Vec::new()
        };

        let details = ChatControlPlaneSessionDetails {
            lineage_root_session_id: snapshot.lineage_root_session_id,
            lineage_depth: snapshot.lineage_depth,
            trajectory_turn_count,
            event_count,
            approval_count,
            terminal_status,
            terminal_recorded_at,
            last_turn_role,
            last_turn_excerpt,
            last_turn_ts,
            recent_events,
            delegate_events,
        };

        Ok(Some(details))
    }

    #[cfg(not(feature = "memory-sqlite"))]
    pub(crate) fn session_details(
        &self,
        _session_id: &str,
        _include_delegate_lifecycle: bool,
    ) -> CliResult<Option<ChatControlPlaneSessionDetails>> {
        Err("control plane requires memory-sqlite support".to_owned())
    }
}

fn build_recent_event_lines(lines: Vec<String>) -> Vec<String> {
    let reversed_items = lines.into_iter().rev().take(4);
    let mut recent_items = Vec::new();

    for line in reversed_items {
        recent_items.push(line);
    }

    recent_items.reverse();
    recent_items
}

fn truncate_excerpt(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_owned();
    }

    let keep_count = max_chars.saturating_sub(1);
    let mut excerpt = text.chars().take(keep_count).collect::<String>();
    excerpt.push('…');
    excerpt
}

fn visible_resume_excerpt(role: &str, content: &str) -> Option<String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }
    if role == "assistant" && looks_like_internal_persisted_payload(trimmed) {
        return None;
    }
    Some(truncate_excerpt(trimmed, 96))
}

fn looks_like_internal_persisted_payload(text: &str) -> bool {
    let Ok(parsed) = serde_json::from_str::<Value>(text) else {
        return false;
    };
    parsed
        .get(crate::memory::INTERNAL_PERSISTED_RECORD_MARKER)
        .and_then(Value::as_bool)
        == Some(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_recent_event_lines_keeps_latest_four_in_display_order() {
        let items = [
            "event#1=a".to_owned(),
            "event#2=b".to_owned(),
            "event#3=c".to_owned(),
            "event#4=d".to_owned(),
            "event#5=e".to_owned(),
        ];
        let recent = build_recent_event_lines(items.to_vec());

        assert_eq!(recent.len(), 4);
        assert_eq!(recent[0], "event#2=b");
        assert_eq!(recent[3], "event#5=e");
    }

    #[cfg(feature = "memory-sqlite")]
    #[test]
    fn recent_resumable_root_sessions_returns_recent_roots_only() {
        let (_config, memory_config, sqlite_path) =
            crate::chat::tests::init_chat_test_memory("recent-resume-roots");
        let repo = crate::session::repository::SessionRepository::new(&memory_config)
            .expect("repository");
        repo.create_session(crate::session::repository::NewSessionRecord {
            session_id: "root-a".to_owned(),
            kind: crate::session::repository::SessionKind::Root,
            parent_session_id: None,
            label: Some("Root A".to_owned()),
            state: crate::session::repository::SessionState::Ready,
        })
        .expect("root a");
        repo.create_session(crate::session::repository::NewSessionRecord {
            session_id: "root-b".to_owned(),
            kind: crate::session::repository::SessionKind::Root,
            parent_session_id: None,
            label: Some("Root B".to_owned()),
            state: crate::session::repository::SessionState::Ready,
        })
        .expect("root b");
        repo.create_session(crate::session::repository::NewSessionRecord {
            session_id: "child-c".to_owned(),
            kind: crate::session::repository::SessionKind::DelegateChild,
            parent_session_id: Some("root-b".to_owned()),
            label: Some("Child C".to_owned()),
            state: crate::session::repository::SessionState::Ready,
        })
        .expect("child c");
        crate::session::store::append_session_turn_direct("root-a", "user", "older", &memory_config)
            .expect("turn a");
        crate::session::store::append_session_turn_direct("root-b", "user", "newer", &memory_config)
            .expect("turn b");
        crate::session::store::append_session_turn_direct("child-c", "assistant", "child", &memory_config)
            .expect("turn c");
        let conn = rusqlite::Connection::open(sqlite_path.as_path()).expect("open sqlite");
        conn.execute(
            "UPDATE sessions SET updated_at = 100 WHERE session_id = 'root-a'",
            [],
        )
        .expect("update root-a");
        conn.execute(
            "UPDATE sessions SET updated_at = 200 WHERE session_id = 'root-b'",
            [],
        )
        .expect("update root-b");
        conn.execute(
            "UPDATE turns SET ts = 100 WHERE session_id = 'root-a'",
            [],
        )
        .expect("update turns root-a");
        conn.execute(
            "UPDATE turns SET ts = 200 WHERE session_id = 'root-b'",
            [],
        )
        .expect("update turns root-b");
        conn.execute(
            "UPDATE turns SET ts = 150 WHERE session_id = 'child-c'",
            [],
        )
        .expect("update turns child-c");

        let store = ChatControlPlaneStore::new(&memory_config).expect("store");
        let sessions = store
            .recent_resumable_root_sessions(8)
            .expect("recent sessions");

        assert_eq!(
            sessions
                .iter()
                .map(|session| session.session_id.as_str())
                .collect::<Vec<_>>(),
            vec!["root-b", "root-a"]
        );

        crate::chat::tests::cleanup_chat_test_memory(&sqlite_path);
    }

    #[cfg(feature = "memory-sqlite")]
    #[test]
    fn visible_sessions_omits_archived_lineage_entries() {
        let (_config, memory_config, sqlite_path) =
            crate::chat::tests::init_chat_test_memory("visible-sessions-omit-archived");
        let repo = crate::session::repository::SessionRepository::new(&memory_config)
            .expect("repository");
        repo.create_session(crate::session::repository::NewSessionRecord {
            session_id: "root-session".to_owned(),
            kind: crate::session::repository::SessionKind::Root,
            parent_session_id: None,
            label: Some("Root".to_owned()),
            state: crate::session::repository::SessionState::Ready,
        })
        .expect("root");
        repo.create_session(crate::session::repository::NewSessionRecord {
            session_id: "child-live".to_owned(),
            kind: crate::session::repository::SessionKind::DelegateChild,
            parent_session_id: Some("root-session".to_owned()),
            label: Some("Live Child".to_owned()),
            state: crate::session::repository::SessionState::Ready,
        })
        .expect("child live");
        repo.create_session(crate::session::repository::NewSessionRecord {
            session_id: "child-archived".to_owned(),
            kind: crate::session::repository::SessionKind::DelegateChild,
            parent_session_id: Some("root-session".to_owned()),
            label: Some("Archived Child".to_owned()),
            state: crate::session::repository::SessionState::Completed,
        })
        .expect("child archived");
        crate::session::store::append_session_turn_direct(
            "root-session",
            "user",
            "root",
            &memory_config,
        )
        .expect("root turn");
        crate::session::store::append_session_turn_direct(
            "child-live",
            "assistant",
            "live",
            &memory_config,
        )
        .expect("live turn");
        crate::session::store::append_session_turn_direct(
            "child-archived",
            "assistant",
            "archived",
            &memory_config,
        )
        .expect("archived turn");
        repo.append_event(crate::session::repository::NewSessionEvent {
            session_id: "child-archived".to_owned(),
            event_kind: "session_archived".to_owned(),
            actor_session_id: Some("root-session".to_owned()),
            payload_json: serde_json::json!({}),
        })
        .expect("archive event");

        let store = ChatControlPlaneStore::new(&memory_config).expect("store");
        let sessions = store
            .visible_sessions("root-session", 8)
            .expect("visible sessions");
        let ids = sessions
            .iter()
            .map(|session| session.session_id.as_str())
            .collect::<Vec<_>>();

        assert!(ids.contains(&"root-session"));
        assert!(ids.contains(&"child-live"));
        assert!(!ids.contains(&"child-archived"));

        crate::chat::tests::cleanup_chat_test_memory(&sqlite_path);
    }

    #[test]
    fn truncate_excerpt_adds_ellipsis_when_needed() {
        let excerpt = truncate_excerpt("abcdef", 4);
        let short = truncate_excerpt("abc", 4);

        assert_eq!(excerpt, "abc…");
        assert_eq!(short, "abc");
    }

    #[cfg(feature = "memory-sqlite")]
    #[test]
    fn derive_resume_session_label_softens_unnamed_root_sessions() {
        let root_default = crate::session::repository::SessionSummaryRecord {
            session_id: "default".to_owned(),
            kind: crate::session::repository::SessionKind::Root,
            parent_session_id: None,
            label: None,
            state: crate::session::repository::SessionState::Ready,
            created_at: 0,
            updated_at: 0,
            archived_at: None,
            turn_count: 1,
            last_turn_at: Some(1),
            first_user_message: None,
            last_error: None,
        };
        let root_chat = crate::session::repository::SessionSummaryRecord {
            session_id: "chat-123".to_owned(),
            kind: crate::session::repository::SessionKind::Root,
            parent_session_id: None,
            label: None,
            state: crate::session::repository::SessionState::Ready,
            created_at: 0,
            updated_at: 0,
            archived_at: None,
            turn_count: 1,
            last_turn_at: Some(1),
            first_user_message: None,
            last_error: None,
        };
        let root_generic = crate::session::repository::SessionSummaryRecord {
            session_id: "refactor-audit-gh-1777983772".to_owned(),
            kind: crate::session::repository::SessionKind::Root,
            parent_session_id: None,
            label: None,
            state: crate::session::repository::SessionState::Ready,
            created_at: 0,
            updated_at: 0,
            archived_at: None,
            turn_count: 1,
            last_turn_at: Some(1),
            first_user_message: None,
            last_error: None,
        };

        assert_eq!(derive_resume_session_label(&root_default), "legacy chat");
        assert_eq!(derive_resume_session_label(&root_chat), "chat");
        assert_eq!(derive_resume_session_label(&root_generic), "session");
    }

    #[cfg(feature = "memory-sqlite")]
    #[test]
    fn recent_resumable_root_sessions_include_first_user_message() {
        let (_config, memory_config, sqlite_path) =
            crate::chat::tests::init_chat_test_memory("recent-resume-first-user");
        let repo = crate::session::repository::SessionRepository::new(&memory_config)
            .expect("repository");
        repo.create_session(crate::session::repository::NewSessionRecord {
            session_id: "root-a".to_owned(),
            kind: crate::session::repository::SessionKind::Root,
            parent_session_id: None,
            label: None,
            state: crate::session::repository::SessionState::Ready,
        })
        .expect("root a");
        crate::session::store::append_session_turn_direct(
            "root-a",
            "user",
            "Look at github.com/chumyin and summarize it",
            &memory_config,
        )
        .expect("user turn");
        crate::session::store::append_session_turn_direct(
            "root-a",
            "assistant",
            "summary answer",
            &memory_config,
        )
        .expect("assistant turn");

        let store = ChatControlPlaneStore::new(&memory_config).expect("store");
        let sessions = store
            .recent_resumable_root_sessions(8)
            .expect("recent sessions");
        let root = sessions
            .iter()
            .find(|session| session.session_id == "root-a")
            .expect("root session");

        assert_eq!(
            root.first_user_message.as_deref(),
            Some("Look at github.com/chumyin and summarize it")
        );

        crate::chat::tests::cleanup_chat_test_memory(&sqlite_path);
    }

    #[cfg(feature = "memory-sqlite")]
    #[test]
    fn control_plane_summary_preserves_explicit_label_separately_from_fallback_title() {
        let root_named = crate::session::repository::SessionSummaryRecord {
            session_id: "chat-1".to_owned(),
            kind: crate::session::repository::SessionKind::Root,
            parent_session_id: None,
            label: Some("Named Session".to_owned()),
            state: crate::session::repository::SessionState::Ready,
            created_at: 0,
            updated_at: 0,
            archived_at: None,
            turn_count: 1,
            last_turn_at: Some(1),
            first_user_message: Some("hello".to_owned()),
            last_error: None,
        };
        let root_fallback = crate::session::repository::SessionSummaryRecord {
            session_id: "chat-2".to_owned(),
            kind: crate::session::repository::SessionKind::Root,
            parent_session_id: None,
            label: None,
            state: crate::session::repository::SessionState::Ready,
            created_at: 0,
            updated_at: 0,
            archived_at: None,
            turn_count: 1,
            last_turn_at: Some(1),
            first_user_message: Some("hello".to_owned()),
            last_error: None,
        };

        let named = ChatControlPlaneSessionSummary::from_session_summary(&root_named);
        let fallback = ChatControlPlaneSessionSummary::from_session_summary(&root_fallback);

        assert_eq!(named.label, "Named Session");
        assert_eq!(named.explicit_label.as_deref(), Some("Named Session"));
        assert_eq!(fallback.label, "chat");
        assert_eq!(fallback.explicit_label, None);
    }

    #[cfg(feature = "memory-sqlite")]
    #[test]
    fn recent_resumable_root_sessions_skip_internal_tail_when_deriving_excerpt() {
        let (_config, memory_config, sqlite_path) =
            crate::chat::tests::init_chat_test_memory("recent-resume-visible-excerpt");
        let repo = crate::session::repository::SessionRepository::new(&memory_config)
            .expect("repository");
        repo.create_session(crate::session::repository::NewSessionRecord {
            session_id: "root-a".to_owned(),
            kind: crate::session::repository::SessionKind::Root,
            parent_session_id: None,
            label: Some("Root A".to_owned()),
            state: crate::session::repository::SessionState::Ready,
        })
        .expect("root a");
        crate::session::store::append_session_turn_direct(
            "root-a",
            "assistant",
            "Visible answer body",
            &memory_config,
        )
        .expect("visible turn");
        crate::session::store::append_session_turn_direct(
            "root-a",
            "assistant",
            r#"{"_loong_internal":true,"event":"turn_checkpoint","payload":{"kind":"persist_reply"}}"#,
            &memory_config,
        )
        .expect("internal tail turn");

        let store = ChatControlPlaneStore::new(&memory_config).expect("store");
        let sessions = store
            .recent_resumable_root_sessions(8)
            .expect("recent sessions");
        let root = sessions
            .iter()
            .find(|session| session.session_id == "root-a")
            .expect("root session");

        assert_eq!(root.last_turn_excerpt.as_deref(), Some("Visible answer body"));

        crate::chat::tests::cleanup_chat_test_memory(&sqlite_path);
    }

    #[cfg(feature = "memory-sqlite")]
    #[test]
    fn approval_summary_from_record_pairs_raw_and_visible_tool_names() {
        let record = ApprovalRequestRecord {
            approval_request_id: "apr-1".to_owned(),
            session_id: "child-session".to_owned(),
            turn_id: "turn-1".to_owned(),
            tool_call_id: "call-1".to_owned(),
            tool_name: "shell.exec".to_owned(),
            approval_key: "tool:shell.exec".to_owned(),
            status: crate::session::repository::ApprovalRequestStatus::Pending,
            decision: None,
            request_payload_json: serde_json::json!({}),
            governance_snapshot_json: serde_json::json!({
                "reason": "operator approval required",
                "rule_id": "shell-gate",
            }),
            requested_at: 42,
            resolved_at: None,
            resolved_by_session_id: None,
            executed_at: None,
            last_error: None,
        };

        let summary = ChatControlPlaneApprovalSummary::from_record(&record);

        assert_eq!(summary.tool_name, "shell.exec");
        assert_eq!(summary.visible_tool_name, "bash");
        assert_eq!(summary.request_summary, serde_json::json!({}));
        assert_eq!(
            summary.reason.as_deref(),
            Some("operator approval required")
        );
        assert_eq!(summary.rule_id.as_deref(), Some("shell-gate"));
    }
}
