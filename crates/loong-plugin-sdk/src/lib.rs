#![forbid(unsafe_code)]

//! Transitional Phase 2 plugin spine.
//! Delete any overlapping built-in plugin ownership inside `crates/kernel` only
//! after later phases move loading and contract enforcement onto this SDK layer.

use serde::{Deserialize, Serialize};

use loong_core::ArtifactDurabilityClass;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginExecutionModel {
    OutOfProcessFirst,
    InProcessOptIn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginAttachTarget {
    Session,
    Task,
    Subtask,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginToolContract {
    pub name: String,
    pub summary: String,
    pub default_artifact_durability: ArtifactDurabilityClass,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub plugin_id: String,
    pub execution_model: PluginExecutionModel,
    pub attach_targets: Vec<PluginAttachTarget>,
    pub tools: Vec<PluginToolContract>,
}
