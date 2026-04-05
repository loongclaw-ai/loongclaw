use std::collections::{BTreeMap, BTreeSet};

use crate::CliResult;
use crate::config::{AcpxBackendConfig, AcpxMcpServerConfig, LoongClawConfig};

use super::config::{McpServerConfig, McpServerTransportConfig};
use super::types::{
    McpAuthStatus, McpRuntimeServerSnapshot, McpRuntimeSnapshot, McpServerOrigin,
    McpServerOriginKind, McpServerStatus, McpServerStatusKind, McpStdioServerLaunchSpec,
    McpTransportSnapshot,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct McpRegistryEntry {
    snapshot: McpRuntimeServerSnapshot,
    stdio_launch_spec: Option<McpStdioServerLaunchSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct McpRegistry {
    servers: BTreeMap<String, McpRegistryEntry>,
    missing_selected_servers: Vec<String>,
}

impl McpRegistry {
    pub fn from_config(config: &LoongClawConfig) -> CliResult<Self> {
        let mut registry = Self::default();

        for (raw_name, server) in &config.mcp.servers {
            let name = canonical_server_name(raw_name)?;
            let origin = McpServerOrigin {
                kind: McpServerOriginKind::Config,
                source_id: None,
            };

            registry.upsert_server_config(name, server, origin);
        }

        if let Some(profile) = config.acp.acpx_profile() {
            registry.ingest_acpx_profile(profile)?;
        }

        let bootstrap_names = config.acp.dispatch.bootstrap_mcp_server_names()?;

        for selected_name in bootstrap_names {
            let maybe_server = registry.servers.get_mut(&selected_name);

            if let Some(server) = maybe_server {
                server.snapshot.selected_for_acp_bootstrap = true;

                let origin = McpServerOrigin {
                    kind: McpServerOriginKind::AcpBootstrapSelection,
                    source_id: None,
                };

                push_origin(&mut server.snapshot.origins, origin);
                continue;
            }

            registry.missing_selected_servers.push(selected_name);
        }

        Ok(registry)
    }

    pub fn snapshot(&self) -> McpRuntimeSnapshot {
        let servers = self
            .servers
            .values()
            .map(|entry| entry.snapshot.clone())
            .collect();

        McpRuntimeSnapshot {
            servers,
            missing_selected_servers: self.missing_selected_servers.clone(),
        }
    }

    pub fn injectable_stdio_server_count(&self) -> usize {
        let mut count = 0;

        for entry in self.servers.values() {
            let is_stdio = entry.stdio_launch_spec.is_some();
            let is_enabled = entry.snapshot.enabled;

            if is_stdio && is_enabled {
                count += 1;
            }
        }

        count
    }

    pub fn resolve_selected_server_names(
        &self,
        requested_names: &[String],
    ) -> CliResult<Vec<String>> {
        let mut selected_names = Vec::new();
        let mut seen_names = BTreeSet::new();
        let mut missing_names = Vec::new();

        for raw_name in requested_names {
            let normalized_name = canonical_server_name(raw_name)?;
            let inserted = seen_names.insert(normalized_name.clone());

            if !inserted {
                continue;
            }

            let contains_server = self.servers.contains_key(&normalized_name);

            if contains_server {
                selected_names.push(normalized_name);
                continue;
            }

            missing_names.push(normalized_name);
        }

        if missing_names.is_empty() {
            return Ok(selected_names);
        }

        let rendered_names = missing_names.join(", ");
        let message = format!(
            "ACPX requested mcp_servers are not configured in the shared MCP registry ([mcp.servers] or [acp.backends.acpx.mcp_servers]): {rendered_names}"
        );

        Err(message)
    }

    pub fn resolve_injectable_stdio_launch_specs(
        &self,
        selected_names: &[String],
    ) -> CliResult<Vec<McpStdioServerLaunchSpec>> {
        let mut launch_specs = Vec::new();

        for name in selected_names {
            let maybe_entry = self.servers.get(name);

            let Some(entry) = maybe_entry else {
                let message = format!(
                    "ACPX requested mcp_servers are not configured in the shared MCP registry ([mcp.servers] or [acp.backends.acpx.mcp_servers]): {name}"
                );

                return Err(message);
            };

            let is_enabled = entry.snapshot.enabled;

            if !is_enabled {
                let message = format!(
                    "ACPX requested mcp_server `{name}` exists, but it is disabled in the shared MCP registry"
                );

                return Err(message);
            }

            let maybe_launch_spec = entry.stdio_launch_spec.clone();

            let Some(launch_spec) = maybe_launch_spec else {
                let transport_kind = transport_kind_label(&entry.snapshot.transport);
                let message = format!(
                    "ACPX requested mcp_server `{name}` exists, but its transport `{transport_kind}` is not compatible with ACPX injection; only stdio MCP servers can be proxied"
                );

                return Err(message);
            };

            launch_specs.push(launch_spec);
        }

        Ok(launch_specs)
    }

    fn ingest_acpx_profile(&mut self, profile: &AcpxBackendConfig) -> CliResult<()> {
        for (raw_name, server) in &profile.mcp_servers {
            let name = canonical_server_name(raw_name)?;
            let entry = registry_entry_from_acpx_profile(name, server);
            self.merge_or_insert(entry);
        }

        Ok(())
    }

    fn upsert_server_config(
        &mut self,
        name: String,
        server: &McpServerConfig,
        origin: McpServerOrigin,
    ) {
        let entry = registry_entry_from_config(name, server, origin);
        self.merge_or_insert(entry);
    }

    fn merge_or_insert(&mut self, mut next: McpRegistryEntry) {
        let maybe_existing = self.servers.get_mut(&next.snapshot.name);

        if let Some(existing) = maybe_existing {
            for origin in next.snapshot.origins.drain(..) {
                push_origin(&mut existing.snapshot.origins, origin);
            }

            let should_adopt_stdio_launch_spec = existing.stdio_launch_spec.is_none();

            if should_adopt_stdio_launch_spec {
                existing.stdio_launch_spec = next.stdio_launch_spec;
            }

            return;
        }

        let name = next.snapshot.name.clone();
        self.servers.insert(name, next);
    }
}

pub fn collect_mcp_runtime_snapshot(config: &LoongClawConfig) -> CliResult<McpRuntimeSnapshot> {
    let registry = McpRegistry::from_config(config)?;
    let snapshot = registry.snapshot();
    Ok(snapshot)
}

fn registry_entry_from_config(
    name: String,
    server: &McpServerConfig,
    origin: McpServerOrigin,
) -> McpRegistryEntry {
    let transport = transport_snapshot(&server.transport);
    let status_kind = if server.enabled {
        McpServerStatusKind::Pending
    } else {
        McpServerStatusKind::Disabled
    };

    let snapshot = McpRuntimeServerSnapshot {
        name,
        enabled: server.enabled,
        required: server.required,
        selected_for_acp_bootstrap: false,
        origins: vec![origin],
        status: McpServerStatus {
            kind: status_kind,
            auth: McpAuthStatus::Unknown,
            last_error: None,
        },
        transport,
        enabled_tools: server.enabled_tools.clone(),
        disabled_tools: server.disabled_tools.clone(),
        startup_timeout_ms: server.startup_timeout_ms,
        tool_timeout_ms: server.tool_timeout_ms,
    };

    let stdio_launch_spec = stdio_launch_spec_from_config(&snapshot, &server.transport);

    McpRegistryEntry {
        snapshot,
        stdio_launch_spec,
    }
}

fn registry_entry_from_acpx_profile(
    name: String,
    server: &AcpxMcpServerConfig,
) -> McpRegistryEntry {
    let transport = McpTransportSnapshot::Stdio {
        command: server.command.clone(),
        args: server.args.clone(),
        cwd: None,
        env_var_names: server.env.keys().cloned().collect(),
    };

    let snapshot = McpRuntimeServerSnapshot {
        name: name.clone(),
        enabled: true,
        required: false,
        selected_for_acp_bootstrap: false,
        origins: vec![McpServerOrigin {
            kind: McpServerOriginKind::AcpBackendProfile,
            source_id: Some("acpx".to_owned()),
        }],
        status: McpServerStatus {
            kind: McpServerStatusKind::Pending,
            auth: McpAuthStatus::Unknown,
            last_error: None,
        },
        transport,
        enabled_tools: Vec::new(),
        disabled_tools: Vec::new(),
        startup_timeout_ms: None,
        tool_timeout_ms: None,
    };

    let stdio_launch_spec = McpStdioServerLaunchSpec {
        name,
        command: server.command.clone(),
        args: server.args.clone(),
        env: server.env.clone(),
        cwd: None,
        startup_timeout_ms: None,
        tool_timeout_ms: None,
    };

    McpRegistryEntry {
        snapshot,
        stdio_launch_spec: Some(stdio_launch_spec),
    }
}

fn canonical_server_name(raw: &str) -> CliResult<String> {
    let normalized = raw.trim().to_ascii_lowercase();

    if normalized.is_empty() {
        return Err("MCP server names must not be empty".to_owned());
    }

    Ok(normalized)
}

fn push_origin(origins: &mut Vec<McpServerOrigin>, candidate: McpServerOrigin) {
    let already_present = origins.iter().any(|origin| origin == &candidate);

    if already_present {
        return;
    }

    origins.push(candidate);
}

fn transport_snapshot(transport: &McpServerTransportConfig) -> McpTransportSnapshot {
    match transport {
        McpServerTransportConfig::Stdio {
            command,
            args,
            env,
            cwd,
        } => McpTransportSnapshot::Stdio {
            command: command.clone(),
            args: args.clone(),
            cwd: cwd.as_ref().map(|value| value.display().to_string()),
            env_var_names: env.keys().cloned().collect(),
        },
        McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var,
            http_headers,
            env_http_headers,
        } => McpTransportSnapshot::StreamableHttp {
            url: url.clone(),
            bearer_token_env_var: bearer_token_env_var.clone(),
            http_header_names: http_headers.keys().cloned().collect(),
            env_http_header_names: env_http_headers.keys().cloned().collect(),
        },
    }
}

fn transport_kind_label(transport: &McpTransportSnapshot) -> &'static str {
    match transport {
        McpTransportSnapshot::Stdio { .. } => "stdio",
        McpTransportSnapshot::StreamableHttp { .. } => "streamable_http",
    }
}

fn stdio_launch_spec_from_config(
    snapshot: &McpRuntimeServerSnapshot,
    transport: &McpServerTransportConfig,
) -> Option<McpStdioServerLaunchSpec> {
    let McpServerTransportConfig::Stdio {
        command,
        args,
        env,
        cwd,
    } = transport
    else {
        return None;
    };

    let cwd_string = cwd.as_ref().map(|value| value.display().to_string());

    let launch_spec = McpStdioServerLaunchSpec {
        name: snapshot.name.clone(),
        command: command.clone(),
        args: args.clone(),
        env: env.clone(),
        cwd: cwd_string,
        startup_timeout_ms: snapshot.startup_timeout_ms,
        tool_timeout_ms: snapshot.tool_timeout_ms,
    };

    Some(launch_spec)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::config::{AcpConfig, AcpxMcpServerConfig};

    use super::*;
    use crate::mcp::config::{McpConfig, McpServerConfig, McpServerTransportConfig};

    fn configured_stdio_server() -> McpServerConfig {
        McpServerConfig {
            transport: McpServerTransportConfig::Stdio {
                command: "uvx".to_owned(),
                args: vec!["context7-mcp".to_owned()],
                env: BTreeMap::from([("API_TOKEN".to_owned(), "secret".to_owned())]),
                cwd: Some(PathBuf::from("/workspace/repo")),
            },
            enabled: true,
            required: false,
            startup_timeout_ms: Some(15_000),
            tool_timeout_ms: Some(120_000),
            enabled_tools: vec!["search".to_owned()],
            disabled_tools: vec!["write".to_owned()],
        }
    }

    #[test]
    fn collect_mcp_runtime_snapshot_includes_config_servers_and_bootstrap_selection() {
        let config = LoongClawConfig {
            mcp: McpConfig {
                servers: BTreeMap::from([("Docs".to_owned(), configured_stdio_server())]),
            },
            acp: AcpConfig {
                dispatch: crate::config::AcpDispatchConfig {
                    bootstrap_mcp_servers: vec![" docs ".to_owned()],
                    ..crate::config::AcpDispatchConfig::default()
                },
                ..AcpConfig::default()
            },
            ..LoongClawConfig::default()
        };

        let snapshot = collect_mcp_runtime_snapshot(&config).expect("collect MCP snapshot");

        assert!(snapshot.missing_selected_servers.is_empty());
        assert_eq!(snapshot.servers.len(), 1);

        let server = &snapshot.servers[0];

        assert_eq!(server.name, "docs");
        assert!(server.selected_for_acp_bootstrap);
        assert_eq!(server.status.kind, McpServerStatusKind::Pending);
        assert_eq!(server.enabled_tools, vec!["search".to_owned()]);
        assert_eq!(server.disabled_tools, vec!["write".to_owned()]);

        let has_config_origin = server
            .origins
            .iter()
            .any(|origin| origin.kind == McpServerOriginKind::Config && origin.source_id.is_none());
        let has_bootstrap_origin = server.origins.iter().any(|origin| {
            origin.kind == McpServerOriginKind::AcpBootstrapSelection && origin.source_id.is_none()
        });

        assert!(has_config_origin);
        assert!(has_bootstrap_origin);
        assert_eq!(
            server.transport,
            McpTransportSnapshot::Stdio {
                command: "uvx".to_owned(),
                args: vec!["context7-mcp".to_owned()],
                cwd: Some("/workspace/repo".to_owned()),
                env_var_names: vec!["API_TOKEN".to_owned()],
            }
        );
    }

    #[test]
    fn collect_mcp_runtime_snapshot_includes_acpx_profile_servers() {
        let config = LoongClawConfig {
            acp: AcpConfig {
                backends: crate::config::AcpBackendProfilesConfig {
                    acpx: Some(crate::config::AcpxBackendConfig {
                        mcp_servers: BTreeMap::from([(
                            "filesystem".to_owned(),
                            AcpxMcpServerConfig {
                                command: "npx".to_owned(),
                                args: vec![
                                    "-y".to_owned(),
                                    "@modelcontextprotocol/server-filesystem".to_owned(),
                                ],
                                env: BTreeMap::from([(
                                    "NODE_ENV".to_owned(),
                                    "production".to_owned(),
                                )]),
                            },
                        )]),
                        ..crate::config::AcpxBackendConfig::default()
                    }),
                },
                ..AcpConfig::default()
            },
            ..LoongClawConfig::default()
        };

        let snapshot = collect_mcp_runtime_snapshot(&config).expect("collect MCP snapshot");

        assert_eq!(snapshot.servers.len(), 1);

        let server = &snapshot.servers[0];

        assert_eq!(server.name, "filesystem");

        let has_acpx_origin = server.origins.iter().any(|origin| {
            origin.kind == McpServerOriginKind::AcpBackendProfile
                && origin.source_id.as_deref() == Some("acpx")
        });

        assert!(has_acpx_origin);
        assert_eq!(server.status.kind, McpServerStatusKind::Pending);
    }

    #[test]
    fn collect_mcp_runtime_snapshot_reports_missing_bootstrap_names() {
        let config = LoongClawConfig {
            acp: AcpConfig {
                dispatch: crate::config::AcpDispatchConfig {
                    bootstrap_mcp_servers: vec!["missing".to_owned()],
                    ..crate::config::AcpDispatchConfig::default()
                },
                ..AcpConfig::default()
            },
            ..LoongClawConfig::default()
        };

        let snapshot = collect_mcp_runtime_snapshot(&config).expect("collect MCP snapshot");

        assert!(snapshot.servers.is_empty());
        assert_eq!(
            snapshot.missing_selected_servers,
            vec!["missing".to_owned()]
        );
    }

    #[test]
    fn registry_merges_same_server_across_config_and_acpx_profile() {
        let config = LoongClawConfig {
            mcp: McpConfig {
                servers: BTreeMap::from([("filesystem".to_owned(), configured_stdio_server())]),
            },
            acp: AcpConfig {
                backends: crate::config::AcpBackendProfilesConfig {
                    acpx: Some(crate::config::AcpxBackendConfig {
                        mcp_servers: BTreeMap::from([(
                            "filesystem".to_owned(),
                            AcpxMcpServerConfig {
                                command: "npx".to_owned(),
                                args: Vec::new(),
                                env: BTreeMap::new(),
                            },
                        )]),
                        ..crate::config::AcpxBackendConfig::default()
                    }),
                },
                dispatch: crate::config::AcpDispatchConfig {
                    bootstrap_mcp_servers: vec!["filesystem".to_owned()],
                    ..crate::config::AcpDispatchConfig::default()
                },
                ..AcpConfig::default()
            },
            ..LoongClawConfig::default()
        };

        let snapshot = collect_mcp_runtime_snapshot(&config).expect("collect MCP snapshot");

        assert_eq!(snapshot.servers.len(), 1);

        let server = &snapshot.servers[0];

        assert_eq!(server.name, "filesystem");
        assert_eq!(server.origins.len(), 3);
        assert!(server.selected_for_acp_bootstrap);
    }

    #[test]
    fn registry_resolves_injectable_stdio_launch_specs_from_shared_mcp_config() {
        let config = LoongClawConfig {
            mcp: McpConfig {
                servers: BTreeMap::from([("Docs".to_owned(), configured_stdio_server())]),
            },
            ..LoongClawConfig::default()
        };

        let registry = McpRegistry::from_config(&config).expect("registry");
        let requested_names = vec![" docs ".to_owned(), "docs".to_owned()];
        let selected_names = registry
            .resolve_selected_server_names(&requested_names)
            .expect("selected names");
        let launch_specs = registry
            .resolve_injectable_stdio_launch_specs(&selected_names)
            .expect("launch specs");

        assert_eq!(selected_names, vec!["docs".to_owned()]);
        assert_eq!(launch_specs.len(), 1);
        assert_eq!(launch_specs[0].name, "docs");
        assert_eq!(launch_specs[0].command, "uvx");
        assert_eq!(launch_specs[0].args, vec!["context7-mcp".to_owned()]);
        assert_eq!(
            launch_specs[0].env,
            BTreeMap::from([("API_TOKEN".to_owned(), "secret".to_owned())])
        );
    }

    #[test]
    fn registry_rejects_disabled_servers_for_acpx_injection() {
        let disabled_server = McpServerConfig {
            enabled: false,
            ..configured_stdio_server()
        };
        let config = LoongClawConfig {
            mcp: McpConfig {
                servers: BTreeMap::from([("docs".to_owned(), disabled_server)]),
            },
            ..LoongClawConfig::default()
        };

        let registry = McpRegistry::from_config(&config).expect("registry");
        let requested_names = vec!["docs".to_owned()];
        let selected_names = registry
            .resolve_selected_server_names(&requested_names)
            .expect("selected names");
        let error = registry
            .resolve_injectable_stdio_launch_specs(&selected_names)
            .expect_err("disabled server must be rejected for ACPX injection");

        assert!(error.contains("disabled"), "error={error}");
    }

    #[test]
    fn registry_rejects_non_stdio_servers_for_acpx_injection() {
        let config = LoongClawConfig {
            mcp: McpConfig {
                servers: BTreeMap::from([(
                    "remote".to_owned(),
                    McpServerConfig {
                        transport: McpServerTransportConfig::StreamableHttp {
                            url: "https://mcp.example.com".to_owned(),
                            bearer_token_env_var: Some("MCP_TOKEN".to_owned()),
                            http_headers: BTreeMap::new(),
                            env_http_headers: BTreeMap::new(),
                        },
                        enabled: true,
                        required: false,
                        startup_timeout_ms: None,
                        tool_timeout_ms: None,
                        enabled_tools: Vec::new(),
                        disabled_tools: Vec::new(),
                    },
                )]),
            },
            ..LoongClawConfig::default()
        };

        let registry = McpRegistry::from_config(&config).expect("registry");
        let requested_names = vec!["remote".to_owned()];
        let selected_names = registry
            .resolve_selected_server_names(&requested_names)
            .expect("selected names");
        let error = registry
            .resolve_injectable_stdio_launch_specs(&selected_names)
            .expect_err("http server must be rejected for ACPX injection");

        assert!(error.contains("streamable_http"), "error={error}");
    }
}
