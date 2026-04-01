#[cfg(feature = "memory-sqlite")]
use crate::config::ToolConfig;
#[cfg(feature = "memory-sqlite")]
use crate::conversation::{ConstrainedSubagentExecution, ConstrainedSubagentMode};
#[cfg(feature = "memory-sqlite")]
use crate::operator::session_graph::OperatorSessionGraph;
#[cfg(feature = "memory-sqlite")]
use crate::runtime_self_continuity::RuntimeSelfContinuity;
#[cfg(feature = "memory-sqlite")]
use crate::session::repository::{
    CreateSessionWithEventRequest, NewSessionRecord, SessionKind, SessionRepository, SessionState,
};

#[cfg(feature = "memory-sqlite")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperatorDelegateToolViewDecision {
    Root,
    DelegateChild { allow_nested_delegate: bool },
}

#[cfg(feature = "memory-sqlite")]
pub(crate) struct PrepareDelegateChildSessionRequest<'a> {
    pub parent_session_id: &'a str,
    pub child_session_id: &'a str,
    pub child_label: Option<&'a str>,
    pub task: &'a str,
    pub timeout_seconds: u64,
    pub mode: ConstrainedSubagentMode,
    pub tool_config: &'a ToolConfig,
    pub kernel_bound: bool,
    pub runtime_self_continuity: Option<&'a RuntimeSelfContinuity>,
}

#[cfg(feature = "memory-sqlite")]
pub(crate) struct PreparedDelegateChildSession {
    pub execution: ConstrainedSubagentExecution,
}

#[cfg(feature = "memory-sqlite")]
pub(crate) struct OperatorDelegateRuntime<'a> {
    repo: &'a SessionRepository,
    session_graph: OperatorSessionGraph<'a>,
}

#[cfg(feature = "memory-sqlite")]
impl<'a> OperatorDelegateRuntime<'a> {
    pub(crate) fn new(repo: &'a SessionRepository) -> Self {
        let session_graph = OperatorSessionGraph::new(repo);

        Self {
            repo,
            session_graph,
        }
    }

    pub(crate) fn tool_view_decision(
        &self,
        session_id: &str,
        max_depth: usize,
    ) -> Result<OperatorDelegateToolViewDecision, String> {
        let stored_session = self.repo.load_session(session_id)?;

        if let Some(stored_session) = stored_session {
            if stored_session.parent_session_id.is_some() {
                let lineage_depth = self.session_graph.lineage_depth(session_id);
                let allow_nested_delegate = match lineage_depth {
                    Ok(lineage_depth) => lineage_depth < max_depth,
                    Err(error) if Self::is_fail_closed_lineage_error(error.as_str()) => false,
                    Err(error) => return Err(error),
                };

                let decision = OperatorDelegateToolViewDecision::DelegateChild {
                    allow_nested_delegate,
                };
                return Ok(decision);
            }

            return Ok(OperatorDelegateToolViewDecision::Root);
        }

        let legacy_session_summary = self
            .repo
            .load_session_summary_with_legacy_fallback(session_id)?;
        let is_legacy_delegate_child = legacy_session_summary
            .is_some_and(|session_summary| session_summary.kind == SessionKind::DelegateChild);

        if is_legacy_delegate_child {
            let decision = OperatorDelegateToolViewDecision::DelegateChild {
                allow_nested_delegate: false,
            };
            return Ok(decision);
        }

        Ok(OperatorDelegateToolViewDecision::Root)
    }

    pub(crate) fn create_delegate_child_session(
        &self,
        request: PrepareDelegateChildSessionRequest<'_>,
    ) -> Result<PreparedDelegateChildSession, String> {
        let parent_session_id = request.parent_session_id.to_owned();
        let child_session_id = request.child_session_id.to_owned();
        let child_label = request.child_label.map(str::to_owned);
        let task = request.task.to_owned();
        let timeout_seconds = request.timeout_seconds;
        let mode = request.mode;
        let tool_config = request.tool_config;
        let delegate_config = &tool_config.delegate;
        let kernel_bound = request.kernel_bound;
        let runtime_self_continuity = request.runtime_self_continuity.cloned();
        let next_child_depth = self
            .session_graph
            .next_delegate_child_depth(&parent_session_id, delegate_config.max_depth)?;

        let (_, execution) = self
            .repo
            .create_delegate_child_session_with_event_if_within_limit(
                &parent_session_id,
                delegate_config.max_active_children,
                |active_children| {
                    let execution = Self::build_constrained_subagent_execution(
                        tool_config,
                        mode,
                        timeout_seconds,
                        next_child_depth,
                        active_children,
                        kernel_bound,
                    );
                    let session_state = Self::session_state_for_mode(mode);
                    let event_kind = Self::spawn_event_kind_for_mode(mode);
                    let event_payload_json = execution.spawn_payload_with_runtime_self_continuity(
                        &task,
                        child_label.as_deref(),
                        runtime_self_continuity.as_ref(),
                    );
                    let session_record = NewSessionRecord {
                        session_id: child_session_id.clone(),
                        kind: SessionKind::DelegateChild,
                        parent_session_id: Some(parent_session_id.clone()),
                        label: child_label.clone(),
                        state: session_state,
                    };
                    let create_request = CreateSessionWithEventRequest {
                        session: session_record,
                        event_kind: event_kind.to_owned(),
                        actor_session_id: Some(parent_session_id.clone()),
                        event_payload_json,
                    };

                    Ok((create_request, execution))
                },
            )?;
        let prepared_session = PreparedDelegateChildSession { execution };

        Ok(prepared_session)
    }

    fn build_constrained_subagent_execution(
        tool_config: &ToolConfig,
        mode: ConstrainedSubagentMode,
        timeout_seconds: u64,
        next_child_depth: usize,
        active_children: usize,
        kernel_bound: bool,
    ) -> ConstrainedSubagentExecution {
        let delegate_config = &tool_config.delegate;
        let runtime_narrowing = delegate_config.child_runtime.runtime_narrowing();

        ConstrainedSubagentExecution {
            mode,
            depth: next_child_depth,
            max_depth: delegate_config.max_depth,
            active_children,
            max_active_children: delegate_config.max_active_children,
            timeout_seconds,
            allow_shell_in_child: delegate_config.allow_shell_in_child,
            child_tool_allowlist: delegate_config.child_tool_allowlist.clone(),
            runtime_narrowing,
            kernel_bound,
        }
    }

    fn session_state_for_mode(mode: ConstrainedSubagentMode) -> SessionState {
        match mode {
            ConstrainedSubagentMode::Inline => SessionState::Running,
            ConstrainedSubagentMode::Async => SessionState::Ready,
        }
    }

    fn spawn_event_kind_for_mode(mode: ConstrainedSubagentMode) -> &'static str {
        match mode {
            ConstrainedSubagentMode::Inline => "delegate_started",
            ConstrainedSubagentMode::Async => "delegate_queued",
        }
    }

    fn is_fail_closed_lineage_error(error: &str) -> bool {
        error.starts_with("session_lineage_broken:")
            || error.starts_with("session_lineage_cycle_detected:")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        OperatorDelegateRuntime, OperatorDelegateToolViewDecision,
        PrepareDelegateChildSessionRequest,
    };

    use crate::config::ToolConfig;
    use crate::conversation::ConstrainedSubagentMode;
    use crate::memory::runtime_config::MemoryRuntimeConfig;
    use crate::session::repository::{
        NewSessionRecord, SessionKind, SessionRepository, SessionState,
    };

    fn isolated_memory_config(test_name: &str) -> MemoryRuntimeConfig {
        let process_id = std::process::id();
        let temp_dir = std::env::temp_dir();
        let directory_name =
            format!("loongclaw-operator-delegate-runtime-{test_name}-{process_id}");
        let base_dir = temp_dir.join(directory_name);
        let _ = std::fs::create_dir_all(&base_dir);

        let db_path = base_dir.join("memory.sqlite3");
        let _ = std::fs::remove_file(&db_path);

        MemoryRuntimeConfig {
            sqlite_path: Some(db_path),
            ..MemoryRuntimeConfig::default()
        }
    }

    fn seed_root_session(repo: &SessionRepository) {
        let session_record = NewSessionRecord {
            session_id: "root-session".to_owned(),
            kind: SessionKind::Root,
            parent_session_id: None,
            label: Some("Root".to_owned()),
            state: SessionState::Ready,
        };

        repo.create_session(session_record)
            .expect("create root session");
    }

    #[test]
    fn operator_delegate_runtime_creates_inline_child_session_with_started_event() {
        let memory_config = isolated_memory_config("inline-started");
        let repo = SessionRepository::new(&memory_config).expect("create repository");
        let runtime = OperatorDelegateRuntime::new(&repo);
        let mut tool_config = ToolConfig::default();

        tool_config.delegate.child_runtime.web.timeout_seconds = Some(5);
        tool_config.delegate.child_runtime.browser.max_sessions = Some(1);
        seed_root_session(&repo);

        let request = PrepareDelegateChildSessionRequest {
            parent_session_id: "root-session",
            child_session_id: "child-inline",
            child_label: Some("Child Inline"),
            task: "inspect inline child",
            timeout_seconds: 30,
            mode: ConstrainedSubagentMode::Inline,
            tool_config: &tool_config,
            kernel_bound: true,
            runtime_self_continuity: None,
        };
        let prepared = runtime
            .create_delegate_child_session(request)
            .expect("create inline child session");
        let stored_session = repo
            .load_session("child-inline")
            .expect("load inline child session")
            .expect("inline child session should exist");
        let lifecycle_events = repo
            .list_delegate_lifecycle_events("child-inline")
            .expect("list inline child lifecycle events");

        assert_eq!(stored_session.state, SessionState::Running);
        assert_eq!(prepared.execution.mode, ConstrainedSubagentMode::Inline);
        assert_eq!(prepared.execution.depth, 1);
        assert_eq!(
            prepared
                .execution
                .runtime_narrowing
                .web_fetch
                .timeout_seconds,
            Some(5)
        );
        assert_eq!(
            prepared.execution.runtime_narrowing.browser.max_sessions,
            Some(1)
        );
        assert!(
            lifecycle_events
                .iter()
                .any(|event| event.event_kind == "delegate_started")
        );
    }

    #[test]
    fn operator_delegate_runtime_creates_async_child_session_with_queued_event() {
        let memory_config = isolated_memory_config("async-queued");
        let repo = SessionRepository::new(&memory_config).expect("create repository");
        let runtime = OperatorDelegateRuntime::new(&repo);
        let mut tool_config = ToolConfig::default();

        tool_config.delegate.allow_shell_in_child = true;
        seed_root_session(&repo);

        let request = PrepareDelegateChildSessionRequest {
            parent_session_id: "root-session",
            child_session_id: "child-async",
            child_label: Some("Child Async"),
            task: "inspect async child",
            timeout_seconds: 45,
            mode: ConstrainedSubagentMode::Async,
            tool_config: &tool_config,
            kernel_bound: false,
            runtime_self_continuity: None,
        };
        let prepared = runtime
            .create_delegate_child_session(request)
            .expect("create async child session");
        let stored_session = repo
            .load_session("child-async")
            .expect("load async child session")
            .expect("async child session should exist");
        let lifecycle_events = repo
            .list_delegate_lifecycle_events("child-async")
            .expect("list async child lifecycle events");

        assert_eq!(stored_session.state, SessionState::Ready);
        assert_eq!(prepared.execution.mode, ConstrainedSubagentMode::Async);
        assert_eq!(prepared.execution.depth, 1);
        assert!(prepared.execution.allow_shell_in_child);
        assert!(
            lifecycle_events
                .iter()
                .any(|event| event.event_kind == "delegate_queued")
        );
    }

    #[test]
    fn operator_delegate_runtime_fail_closes_tool_view_for_broken_lineage_child() {
        let memory_config = isolated_memory_config("broken-lineage");
        let repo = SessionRepository::new(&memory_config).expect("create repository");
        let runtime = OperatorDelegateRuntime::new(&repo);

        repo.create_session(NewSessionRecord {
            session_id: "child-session".to_owned(),
            kind: SessionKind::DelegateChild,
            parent_session_id: Some("missing-parent".to_owned()),
            label: Some("Child".to_owned()),
            state: SessionState::Ready,
        })
        .expect("create broken child session");

        let decision = runtime
            .tool_view_decision("child-session", 2)
            .expect("resolve tool view decision");

        assert_eq!(
            decision,
            OperatorDelegateToolViewDecision::DelegateChild {
                allow_nested_delegate: false,
            }
        );
    }
}
