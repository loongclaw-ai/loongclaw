#![forbid(unsafe_code)]

//! Transitional Phase 2 runtime spine.
//! Delete the legacy `crates/app` runtime ownership only after Phase 3 retargets
//! a concrete product path onto these runtime boundaries.

use serde::{Deserialize, Serialize};

pub use loong_core::{
    ChildBudgetPolicy, ExecutionArtifact, Session, SessionBudgetOverlay, SessionEvent, Task,
    TaskBudget, TaskEvent, TaskExecutionMode, TaskLifecycle, WorkspaceContext,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeSurface {
    ProviderAdapters,
    DefaultCodingTools,
    BrowserAdapter,
    SessionStorage,
    ArtifactStorage,
    ProjectionCompaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSurfaceBinding {
    pub surface: RuntimeSurface,
    pub ownership_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSpine {
    pub core_contract: String,
    pub detached_execution_supported: bool,
    pub surfaces: Vec<RuntimeSurfaceBinding>,
}

impl Default for RuntimeSpine {
    fn default() -> Self {
        Self {
            core_contract: "loong-core".to_owned(),
            detached_execution_supported: true,
            surfaces: vec![
                RuntimeSurfaceBinding {
                    surface: RuntimeSurface::ProviderAdapters,
                    ownership_summary: "Official model/provider adapters stay above loong-core"
                        .to_owned(),
                },
                RuntimeSurfaceBinding {
                    surface: RuntimeSurface::DefaultCodingTools,
                    ownership_summary: "Default shell/file/browser tools live in the runtime layer"
                        .to_owned(),
                },
                RuntimeSurfaceBinding {
                    surface: RuntimeSurface::BrowserAdapter,
                    ownership_summary:
                        "Browser automation stays a runtime adapter, not a kernel concern"
                            .to_owned(),
                },
                RuntimeSurfaceBinding {
                    surface: RuntimeSurface::SessionStorage,
                    ownership_summary:
                        "Session truth storage binds durable facts without redefining the kernel"
                            .to_owned(),
                },
                RuntimeSurfaceBinding {
                    surface: RuntimeSurface::ArtifactStorage,
                    ownership_summary:
                        "Artifact references and durability storage are runtime-owned adapters"
                            .to_owned(),
                },
                RuntimeSurfaceBinding {
                    surface: RuntimeSurface::ProjectionCompaction,
                    ownership_summary:
                        "Projection compaction is optional runtime optimization only".to_owned(),
                },
            ],
        }
    }
}
