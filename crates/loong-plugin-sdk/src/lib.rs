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
pub struct PluginChannelBridgeReadiness {
    pub ready: bool,
    #[serde(default)]
    pub missing_fields: Vec<String>,
}

impl Default for PluginChannelBridgeReadiness {
    fn default() -> Self {
        Self {
            ready: true,
            missing_fields: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginChannelBridgeContract {
    #[serde(default)]
    pub channel_id: Option<String>,
    #[serde(default)]
    pub setup_surface: Option<String>,
    #[serde(default)]
    pub transport_family: Option<String>,
    #[serde(default)]
    pub target_contract: Option<String>,
    #[serde(default)]
    pub account_scope: Option<String>,
    #[serde(default)]
    pub runtime_contract: Option<String>,
    #[serde(default)]
    pub runtime_operations: Vec<String>,
    #[serde(default)]
    pub runtime_metadata_issues: Vec<String>,
    #[serde(default)]
    pub readiness: PluginChannelBridgeReadiness,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub plugin_id: String,
    pub execution_model: PluginExecutionModel,
    pub attach_targets: Vec<PluginAttachTarget>,
    pub tools: Vec<PluginToolContract>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_channel_bridge_contract_round_trips_with_runtime_metadata() {
        let contract = PluginChannelBridgeContract {
            channel_id: Some("weixin".to_owned()),
            setup_surface: Some("channel".to_owned()),
            transport_family: Some("wechat_clawbot_ilink_bridge".to_owned()),
            target_contract: Some("weixin_reply_loop".to_owned()),
            account_scope: Some("per_account".to_owned()),
            runtime_contract: Some("loong_channel_bridge_v1".to_owned()),
            runtime_operations: vec![
                "send_message".to_owned(),
                "receive_batch".to_owned(),
                "ack_inbound".to_owned(),
            ],
            runtime_metadata_issues: Vec::new(),
            readiness: PluginChannelBridgeReadiness::default(),
        };

        let encoded = serde_json::to_string(&contract).expect("serialize channel bridge contract");
        let decoded: PluginChannelBridgeContract =
            serde_json::from_str(&encoded).expect("deserialize channel bridge contract");

        assert_eq!(decoded, contract);
    }

    #[test]
    fn plugin_channel_bridge_readiness_defaults_to_ready_without_missing_fields() {
        let readiness = PluginChannelBridgeReadiness::default();

        assert!(readiness.ready);
        assert!(readiness.missing_fields.is_empty());
    }
}
