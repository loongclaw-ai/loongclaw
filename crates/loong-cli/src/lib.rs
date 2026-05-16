#![forbid(unsafe_code)]

//! Transitional Phase 2 CLI shell spine.
//! Delete legacy `crates/daemon` task shell ownership after Phase 3 moves one
//! real entry path onto `loong-app-protocol`.

use serde::{Deserialize, Serialize};

pub use loong_app_protocol::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CliCommandKind {
    StartTask,
    ResumeTask,
    ForkTask,
    SendInput,
    Approve,
    Interrupt,
    SwitchWorktree,
    ReadArtifacts,
    ListTasks,
    ListSessions,
    StreamEvents,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliCommandSurface {
    pub namespace: String,
    pub command: String,
    pub kind: CliCommandKind,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstPartyCliSpine {
    pub interactive_task_shell: CliCommandSurface,
    pub namespaces: Vec<CliCommandSurface>,
}

impl Default for FirstPartyCliSpine {
    fn default() -> Self {
        Self {
            interactive_task_shell: CliCommandSurface {
                namespace: "task".to_owned(),
                command: "chat".to_owned(),
                kind: CliCommandKind::SendInput,
                summary: "Interactive task shell delegates through the app protocol".to_owned(),
            },
            namespaces: vec![
                CliCommandSurface {
                    namespace: "task".to_owned(),
                    command: "start".to_owned(),
                    kind: CliCommandKind::StartTask,
                    summary: "Start a task through the protocol-owned runtime".to_owned(),
                },
                CliCommandSurface {
                    namespace: "task".to_owned(),
                    command: "approve".to_owned(),
                    kind: CliCommandKind::Approve,
                    summary: "Approve a protocol-surfaced checkpoint".to_owned(),
                },
                CliCommandSurface {
                    namespace: "session".to_owned(),
                    command: "switch-worktree".to_owned(),
                    kind: CliCommandKind::SwitchWorktree,
                    summary: "Apply durable same-repo worktree switches at session scope"
                        .to_owned(),
                },
                CliCommandSurface {
                    namespace: "admin".to_owned(),
                    command: "events".to_owned(),
                    kind: CliCommandKind::StreamEvents,
                    summary: "Read the fact event stream instead of shell-local summaries"
                        .to_owned(),
                },
            ],
        }
    }
}
