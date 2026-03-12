use std::collections::BTreeSet;

use loongclaw_contracts::PolicyError;
use loongclaw_kernel::{PolicyExtension, PolicyExtensionContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellPolicyDefault {
    Deny,
    Allow,
}

impl ShellPolicyDefault {
    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "allow" => Self::Allow,
            _ => Self::Deny,
        }
    }
}

pub struct ToolPolicyExtension {
    hard_deny: BTreeSet<String>,
    approval_required: BTreeSet<String>,
    allow: BTreeSet<String>,
    default_mode: ShellPolicyDefault,
}

impl ToolPolicyExtension {
    pub fn new(
        hard_deny: BTreeSet<String>,
        approval_required: BTreeSet<String>,
        allow: BTreeSet<String>,
        default_mode: ShellPolicyDefault,
    ) -> Self {
        Self {
            hard_deny,
            approval_required,
            allow,
            default_mode,
        }
    }

    pub fn default_rules() -> Self {
        Self {
            hard_deny: [
                "rm", "dd", "mkfs", "shutdown", "reboot", "poweroff", "halt", "init",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            approval_required: [
                "bash",
                "sh",
                "zsh",
                "fish",
                "sudo",
                "su",
                "curl",
                "wget",
                "ssh",
                "scp",
                "sftp",
                "nc",
                "ncat",
                "netcat",
                "python",
                "python3",
                "node",
                "perl",
                "ruby",
                "php",
                "pwsh",
                "powershell",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            allow: [
                "echo", "cat", "ls", "pwd", "dir", "where", "whoami", "hostname", "date", "head",
                "tail", "wc", "sort", "uniq", "find", "grep", "rg", "git", "cargo", "rustc", "npm",
                "pip",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            default_mode: ShellPolicyDefault::Deny,
        }
    }

    /// Build from runtime config, merging user overrides with hardcoded defaults.
    pub fn from_config(rt: &super::runtime_config::ToolRuntimeConfig) -> Self {
        let mut ext = Self::default_rules();
        if !rt.shell_deny.is_empty() {
            ext.hard_deny.extend(rt.shell_deny.iter().cloned());
        }
        if !rt.shell_approval_required.is_empty() {
            ext.approval_required
                .extend(rt.shell_approval_required.iter().cloned());
        }
        if !rt.shell_allow.is_empty() {
            ext.allow.extend(rt.shell_allow.iter().cloned());
        }
        ext.default_mode = rt.shell_default_mode;
        ext
    }

    fn normalize_tool_name(raw: &str) -> &str {
        match raw {
            "shell_exec" | "shell" => "shell.exec",
            "file_read" => "file.read",
            "file_write" => "file.write",
            other => other,
        }
    }
}

impl PolicyExtension for ToolPolicyExtension {
    fn name(&self) -> &str {
        "tool-policy"
    }

    fn authorize_extension(&self, context: &PolicyExtensionContext<'_>) -> Result<(), PolicyError> {
        let Some(params) = context.request_parameters else {
            return Ok(());
        };

        let raw_tool_name = params
            .get("tool_name")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let tool_name = Self::normalize_tool_name(raw_tool_name);

        if tool_name != "shell.exec" {
            return Ok(());
        }

        let command = params
            .get("payload")
            .and_then(|p| p.get("command"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_ascii_lowercase);

        let Some(command) = command else {
            return Ok(());
        };

        // Extract basename so absolute paths like "/usr/bin/rm" match "rm".
        let basename = command
            .rsplit('/')
            .next()
            .and_then(|s| s.rsplit('\\').next())
            .unwrap_or(&command);

        if self.hard_deny.contains(basename) {
            return Err(PolicyError::ToolCallDenied {
                tool_name: tool_name.to_owned(),
                reason: format!("command `{basename}` is blocked by shell policy"),
            });
        }

        if self.approval_required.contains(basename) {
            return Err(PolicyError::ToolCallApprovalRequired {
                tool_name: tool_name.to_owned(),
                prompt: format!("command `{basename}` requires approval by shell policy"),
            });
        }

        if self.allow.contains(basename) {
            return Ok(());
        }

        // Default mode for unknown commands
        match self.default_mode {
            ShellPolicyDefault::Allow => Ok(()),
            ShellPolicyDefault::Deny => Err(PolicyError::ToolCallDenied {
                tool_name: tool_name.to_owned(),
                reason: format!(
                    "command `{basename}` is not in the allow list (default-deny policy)"
                ),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loongclaw_contracts::{Capability, CapabilityToken, ExecutionRoute, HarnessKind};
    use loongclaw_kernel::{PolicyExtensionContext, VerticalPackManifest};
    use serde_json::json;
    use std::collections::{BTreeMap, BTreeSet};

    fn test_pack() -> VerticalPackManifest {
        VerticalPackManifest {
            pack_id: "test-pack".into(),
            domain: "test".into(),
            version: "0.1.0".into(),
            default_route: ExecutionRoute {
                harness_kind: HarnessKind::EmbeddedPi,
                adapter: None,
            },
            allowed_connectors: BTreeSet::new(),
            granted_capabilities: BTreeSet::from([Capability::InvokeTool]),
            metadata: BTreeMap::new(),
        }
    }

    fn test_token() -> CapabilityToken {
        CapabilityToken {
            token_id: "tok-1".into(),
            agent_id: "agent-1".into(),
            pack_id: "test-pack".into(),
            issued_at_epoch_s: 1000,
            expires_at_epoch_s: 2000,
            allowed_capabilities: BTreeSet::from([Capability::InvokeTool]),
            generation: 1,
            membrane: None,
        }
    }

    fn make_context<'a>(
        pack: &'a loongclaw_kernel::VerticalPackManifest,
        token: &'a CapabilityToken,
        caps: &'a BTreeSet<Capability>,
        params: Option<&'a serde_json::Value>,
    ) -> PolicyExtensionContext<'a> {
        PolicyExtensionContext {
            pack,
            token,
            now_epoch_s: 1500,
            required_capabilities: caps,
            request_parameters: params,
        }
    }

    #[test]
    fn denies_destructive_shell_commands() {
        let ext = ToolPolicyExtension::default_rules();
        let pack = test_pack();
        let token = test_token();
        let caps = BTreeSet::from([Capability::InvokeTool]);
        let params = json!({"tool_name": "shell.exec", "payload": {"command": "rm"}});
        let ctx = make_context(&pack, &token, &caps, Some(&params));
        let result = ext.authorize_extension(&ctx);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PolicyError::ToolCallDenied { .. }
        ));
    }

    #[test]
    fn requires_approval_for_high_risk_commands() {
        let ext = ToolPolicyExtension::default_rules();
        let pack = test_pack();
        let token = test_token();
        let caps = BTreeSet::from([Capability::InvokeTool]);
        let params = json!({"tool_name": "shell.exec", "payload": {"command": "curl"}});
        let ctx = make_context(&pack, &token, &caps, Some(&params));
        let result = ext.authorize_extension(&ctx);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PolicyError::ToolCallApprovalRequired { .. }
        ));
    }

    #[test]
    fn allows_safe_shell_commands() {
        let ext = ToolPolicyExtension::default_rules();
        let pack = test_pack();
        let token = test_token();
        let caps = BTreeSet::from([Capability::InvokeTool]);
        let params = json!({"tool_name": "shell.exec", "payload": {"command": "echo"}});
        let ctx = make_context(&pack, &token, &caps, Some(&params));
        assert!(ext.authorize_extension(&ctx).is_ok());
    }

    #[test]
    fn normalizes_underscore_shell_alias() {
        let ext = ToolPolicyExtension::default_rules();
        let pack = test_pack();
        let token = test_token();
        let caps = BTreeSet::from([Capability::InvokeTool]);
        let params = json!({"tool_name": "shell_exec", "payload": {"command": "curl"}});
        let ctx = make_context(&pack, &token, &caps, Some(&params));
        let result = ext.authorize_extension(&ctx);
        assert!(matches!(
            result.unwrap_err(),
            PolicyError::ToolCallApprovalRequired { .. }
        ));
    }

    #[test]
    fn keeps_non_shell_tools_allowed() {
        let ext = ToolPolicyExtension::default_rules();
        let pack = test_pack();
        let token = test_token();
        let caps = BTreeSet::from([Capability::InvokeTool]);
        let params = json!({"tool_name": "file.read", "payload": {"path": "/etc/passwd"}});
        let ctx = make_context(&pack, &token, &caps, Some(&params));
        assert!(ext.authorize_extension(&ctx).is_ok());
    }

    #[test]
    fn allows_when_no_request_parameters() {
        let ext = ToolPolicyExtension::default_rules();
        let pack = test_pack();
        let token = test_token();
        let caps = BTreeSet::from([Capability::InvokeTool]);
        let ctx = make_context(&pack, &token, &caps, None);
        assert!(ext.authorize_extension(&ctx).is_ok());
    }

    #[test]
    fn denies_absolute_path_command() {
        let ext = ToolPolicyExtension::default_rules();
        let pack = test_pack();
        let token = test_token();
        let caps = BTreeSet::from([Capability::InvokeTool]);
        let params = json!({"tool_name": "shell.exec", "payload": {"command": "/usr/bin/rm"}});
        let ctx = make_context(&pack, &token, &caps, Some(&params));
        let result = ext.authorize_extension(&ctx);
        assert!(matches!(
            result.unwrap_err(),
            PolicyError::ToolCallDenied { .. }
        ));
    }

    #[test]
    fn denies_windows_absolute_path_command() {
        let ext = ToolPolicyExtension::default_rules();
        let pack = test_pack();
        let token = test_token();
        let caps = BTreeSet::from([Capability::InvokeTool]);
        let params = json!({"tool_name": "shell.exec", "payload": {"command": "C:\\Windows\\System32\\rm.exe"}});
        let ctx = make_context(&pack, &token, &caps, Some(&params));
        // basename is "rm.exe" which doesn't match "rm" in deny list,
        // but also not in allow list — default-deny blocks it.
        assert!(matches!(
            ext.authorize_extension(&ctx).unwrap_err(),
            PolicyError::ToolCallDenied { .. }
        ));
    }

    #[test]
    fn normalizes_bare_shell_alias() {
        let ext = ToolPolicyExtension::default_rules();
        let pack = test_pack();
        let token = test_token();
        let caps = BTreeSet::from([Capability::InvokeTool]);
        let params = json!({"tool_name": "shell", "payload": {"command": "rm"}});
        let ctx = make_context(&pack, &token, &caps, Some(&params));
        assert!(matches!(
            ext.authorize_extension(&ctx).unwrap_err(),
            PolicyError::ToolCallDenied { .. }
        ));
    }

    #[test]
    fn case_insensitive_command_matching() {
        let ext = ToolPolicyExtension::default_rules();
        let pack = test_pack();
        let token = test_token();
        let caps = BTreeSet::from([Capability::InvokeTool]);
        let params = json!({"tool_name": "shell.exec", "payload": {"command": "RM"}});
        let ctx = make_context(&pack, &token, &caps, Some(&params));
        assert!(matches!(
            ext.authorize_extension(&ctx).unwrap_err(),
            PolicyError::ToolCallDenied { .. }
        ));
    }

    #[test]
    fn default_deny_blocks_unknown_command() {
        let ext = ToolPolicyExtension::default_rules();
        let pack = test_pack();
        let token = test_token();
        let caps = BTreeSet::from([Capability::InvokeTool]);
        let params =
            json!({"tool_name": "shell.exec", "payload": {"command": "some_unknown_tool"}});
        let ctx = make_context(&pack, &token, &caps, Some(&params));
        let err = ext.authorize_extension(&ctx).unwrap_err();
        assert!(matches!(err, PolicyError::ToolCallDenied { .. }));
    }

    #[test]
    fn allow_listed_command_passes() {
        let ext = ToolPolicyExtension::default_rules();
        let pack = test_pack();
        let token = test_token();
        let caps = BTreeSet::from([Capability::InvokeTool]);
        let params = json!({"tool_name": "shell.exec", "payload": {"command": "git"}});
        let ctx = make_context(&pack, &token, &caps, Some(&params));
        assert!(ext.authorize_extension(&ctx).is_ok());
    }

    #[test]
    fn allow_mode_passes_unknown_command() {
        let ext = ToolPolicyExtension::new(
            BTreeSet::new(),
            BTreeSet::new(),
            BTreeSet::new(),
            ShellPolicyDefault::Allow,
        );
        let pack = test_pack();
        let token = test_token();
        let caps = BTreeSet::from([Capability::InvokeTool]);
        let params = json!({"tool_name": "shell.exec", "payload": {"command": "anything"}});
        let ctx = make_context(&pack, &token, &caps, Some(&params));
        assert!(ext.authorize_extension(&ctx).is_ok());
    }
}
