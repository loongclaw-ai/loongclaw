use super::*;

fn assert_bridge_runtime_protocol_context(
    runtime: &Value,
    expected_request_id: &str,
    expected_required_capability: &str,
    expected_granted_capability: &str,
) {
    assert_eq!(runtime["request_method"], "tools/call");
    assert_eq!(runtime["request_id"], expected_request_id);
    assert_eq!(runtime["protocol_route"], "tools/call");
    assert_eq!(
        runtime["protocol_required_capability"],
        expected_required_capability
    );
    let capabilities = runtime["protocol_capabilities"]
        .as_array()
        .expect("protocol_capabilities should be an array")
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    assert!(
        capabilities
            .iter()
            .any(|capability| *capability == expected_granted_capability),
        "protocol_capabilities should include {expected_granted_capability}, got {capabilities:?}",
    );
}

fn assert_http_json_runtime_shape(runtime: &Value) {
    for key in [
        "executor",
        "method",
        "url",
        "timeout_ms",
        "enforce_protocol_contract",
        "request_method",
        "request_id",
        "protocol_route",
        "protocol_required_capability",
        "protocol_capabilities",
    ] {
        assert!(
            runtime.get(key).is_some(),
            "http_json runtime should include key `{key}`"
        );
    }
    assert_eq!(runtime["executor"], "http_json_reqwest");
}

fn assert_process_stdio_runtime_shape(runtime: &Value) {
    for key in [
        "executor",
        "transport_kind",
        "command",
        "args",
        "timeout_ms",
        "request_method",
        "request_id",
        "protocol_route",
        "protocol_required_capability",
        "protocol_capabilities",
    ] {
        assert!(
            runtime.get(key).is_some(),
            "process_stdio runtime should include key `{key}`"
        );
    }
    assert_eq!(runtime["executor"], "process_stdio_local");
    assert_eq!(runtime["transport_kind"], "json_line");
}

fn assert_runtime_keys_exact(runtime: &Value, expected_keys: &[&str]) {
    let runtime_object = runtime
        .as_object()
        .expect("runtime payload should be a JSON object");
    let mut actual = runtime_object.keys().cloned().collect::<Vec<_>>();
    actual.sort();
    let mut expected = expected_keys
        .iter()
        .map(|key| key.to_string())
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(actual, expected, "runtime key set should stay stable");
}

#[tokio::test]
async fn execute_spec_process_stdio_bridge_executes_when_enabled_and_allowed() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic")
        .as_nanos();
    let plugin_root =
        std::env::temp_dir().join(format!("loongclaw-plugin-process-stdio-run-{unique}"));
    fs::create_dir_all(&plugin_root).expect("create plugin root");

    fs::write(
        plugin_root.join("stdio_plugin.py"),
        r#"
# LOONGCLAW_PLUGIN_START
# {
#   "plugin_id": "stdio-plugin",
#   "provider_id": "stdio-provider",
#   "connector_name": "stdio-provider",
#   "channel_id": "primary",
#   "endpoint": "local://stdio-provider",
#   "capabilities": ["InvokeConnector"],
#   "metadata": {
#     "bridge_kind":"process_stdio",
#     "command":"cat",
#     "process_timeout_ms":"999999999",
#     "version":"1.0.0"
#   }
# }
# LOONGCLAW_PLUGIN_END
"#,
    )
    .expect("write stdio plugin");

    let spec = RunnerSpec {
        pack: VerticalPackManifest {
            pack_id: "spec-process-stdio-run".to_owned(),
            domain: "ops".to_owned(),
            version: "0.1.0".to_owned(),
            default_route: ExecutionRoute {
                harness_kind: HarnessKind::EmbeddedPi,
                adapter: Some("pi-local".to_owned()),
            },
            allowed_connectors: BTreeSet::new(),
            granted_capabilities: BTreeSet::new(),
            metadata: BTreeMap::new(),
        },
        agent_id: "agent-process-stdio-run".to_owned(),
        ttl_s: 120,
        approval: None,
        defaults: None,
        self_awareness: None,
        plugin_scan: Some(PluginScanSpec {
            enabled: true,
            roots: vec![plugin_root.display().to_string()],
        }),
        bridge_support: Some(BridgeSupportSpec {
            enabled: true,
            supported_bridges: vec![PluginBridgeKind::ProcessStdio],
            supported_adapter_families: Vec::new(),
            enforce_supported: true,
            policy_version: None,
            expected_checksum: None,
            expected_sha256: None,
            execute_process_stdio: true,
            execute_http_json: false,
            allowed_process_commands: vec!["cat".to_owned()],
            enforce_execution_success: true,
            security_scan: None,
        }),
        bootstrap: Some(BootstrapSpec {
            enabled: true,
            allow_http_json_auto_apply: Some(false),
            allow_process_stdio_auto_apply: Some(true),
            allow_native_ffi_auto_apply: Some(false),
            allow_wasm_component_auto_apply: Some(false),
            allow_mcp_server_auto_apply: Some(false),
            enforce_ready_execution: Some(true),
            max_tasks: Some(10),
        }),
        auto_provision: None,
        hotfixes: Vec::new(),
        operation: OperationSpec::ConnectorLegacy {
            connector_name: "stdio-provider".to_owned(),
            operation: "invoke".to_owned(),
            required_capabilities: BTreeSet::from([Capability::InvokeConnector]),
            payload: json!({"question":"ping"}),
        },
    };

    let report = execute_spec(spec, true).await;
    let runtime = &report.outcome["outcome"]["payload"]["bridge_execution"]["runtime"];
    assert_eq!(report.operation_kind, "connector_legacy");
    assert_eq!(report.outcome["outcome"]["status"], "ok");
    assert_eq!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["status"],
        "executed"
    );
    assert_process_stdio_runtime_shape(runtime);
    assert_runtime_keys_exact(
        runtime,
        &[
            "args",
            "command",
            "executor",
            "exit_code",
            "protocol_capabilities",
            "protocol_required_capability",
            "protocol_route",
            "request_id",
            "request_method",
            "response_id",
            "response_method",
            "stderr",
            "stdout",
            "stdout_json",
            "timeout_ms",
            "transport_kind",
        ],
    );
    assert_eq!(runtime["stdout_json"]["operation"], "invoke");
    assert!(runtime.get("exit_code").is_some());
    assert!(runtime.get("response_method").is_some());
    assert!(runtime.get("response_id").is_some());
    assert_bridge_runtime_protocol_context(
        runtime,
        "stdio-provider:primary:invoke",
        "invoke",
        "invoke",
    );
}

#[tokio::test]
async fn execute_spec_process_stdio_bridge_blocks_when_command_not_allowlisted() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic")
        .as_nanos();
    let plugin_root =
        std::env::temp_dir().join(format!("loongclaw-plugin-process-stdio-block-{unique}"));
    fs::create_dir_all(&plugin_root).expect("create plugin root");

    fs::write(
        plugin_root.join("stdio_plugin.py"),
        r#"
# LOONGCLAW_PLUGIN_START
# {
#   "plugin_id": "stdio-plugin",
#   "provider_id": "stdio-provider",
#   "connector_name": "stdio-provider",
#   "channel_id": "primary",
#   "endpoint": "local://stdio-provider",
#   "capabilities": ["InvokeConnector"],
#   "metadata": {
#     "bridge_kind":"process_stdio",
#     "command":"cat",
#     "process_timeout_ms":"999999999",
#     "version":"1.0.0"
#   }
# }
# LOONGCLAW_PLUGIN_END
"#,
    )
    .expect("write stdio plugin");

    let spec = RunnerSpec {
        pack: VerticalPackManifest {
            pack_id: "spec-process-stdio-block".to_owned(),
            domain: "ops".to_owned(),
            version: "0.1.0".to_owned(),
            default_route: ExecutionRoute {
                harness_kind: HarnessKind::EmbeddedPi,
                adapter: Some("pi-local".to_owned()),
            },
            allowed_connectors: BTreeSet::new(),
            granted_capabilities: BTreeSet::new(),
            metadata: BTreeMap::new(),
        },
        agent_id: "agent-process-stdio-block".to_owned(),
        ttl_s: 120,
        approval: None,
        defaults: None,
        self_awareness: None,
        plugin_scan: Some(PluginScanSpec {
            enabled: true,
            roots: vec![plugin_root.display().to_string()],
        }),
        bridge_support: Some(BridgeSupportSpec {
            enabled: true,
            supported_bridges: vec![PluginBridgeKind::ProcessStdio],
            supported_adapter_families: Vec::new(),
            enforce_supported: true,
            policy_version: None,
            expected_checksum: None,
            expected_sha256: None,
            execute_process_stdio: true,
            execute_http_json: false,
            allowed_process_commands: vec!["python3".to_owned()],
            enforce_execution_success: false,
            security_scan: None,
        }),
        bootstrap: Some(BootstrapSpec {
            enabled: true,
            allow_http_json_auto_apply: Some(false),
            allow_process_stdio_auto_apply: Some(true),
            allow_native_ffi_auto_apply: Some(false),
            allow_wasm_component_auto_apply: Some(false),
            allow_mcp_server_auto_apply: Some(false),
            enforce_ready_execution: Some(true),
            max_tasks: Some(10),
        }),
        auto_provision: None,
        hotfixes: Vec::new(),
        operation: OperationSpec::ConnectorLegacy {
            connector_name: "stdio-provider".to_owned(),
            operation: "invoke".to_owned(),
            required_capabilities: BTreeSet::from([Capability::InvokeConnector]),
            payload: json!({"question":"ping"}),
        },
    };

    let report = execute_spec(spec, true).await;
    assert_eq!(report.operation_kind, "connector_legacy");
    assert_eq!(report.outcome["outcome"]["status"], "ok");
    assert_eq!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["status"],
        "blocked"
    );
    assert!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["reason"]
            .as_str()
            .expect("blocked reason should be string")
            .contains("not allowed")
    );
}

#[tokio::test]
async fn execute_spec_process_stdio_bridge_fails_on_invalid_json_line_response() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic")
        .as_nanos();
    let plugin_root = std::env::temp_dir().join(format!(
        "loongclaw-plugin-process-stdio-invalid-frame-{unique}"
    ));
    fs::create_dir_all(&plugin_root).expect("create plugin root");

    fs::write(
        plugin_root.join("stdio_plugin.py"),
        r#"
# LOONGCLAW_PLUGIN_START
# {
#   "plugin_id": "stdio-invalid-frame-plugin",
#   "provider_id": "stdio-invalid-frame-provider",
#   "connector_name": "stdio-invalid-frame-provider",
#   "channel_id": "primary",
#   "endpoint": "local://stdio-invalid-frame-provider",
#   "capabilities": ["InvokeConnector"],
#   "metadata": {
#     "bridge_kind":"process_stdio",
#     "command":"printf",
#     "args_json":"[\"not-json\\n\"]",
#     "version":"1.0.0"
#   }
# }
# LOONGCLAW_PLUGIN_END
"#,
    )
    .expect("write stdio plugin");

    let spec = RunnerSpec {
        pack: VerticalPackManifest {
            pack_id: "spec-process-stdio-invalid-frame".to_owned(),
            domain: "ops".to_owned(),
            version: "0.1.0".to_owned(),
            default_route: ExecutionRoute {
                harness_kind: HarnessKind::EmbeddedPi,
                adapter: Some("pi-local".to_owned()),
            },
            allowed_connectors: BTreeSet::new(),
            granted_capabilities: BTreeSet::new(),
            metadata: BTreeMap::new(),
        },
        agent_id: "agent-process-stdio-invalid-frame".to_owned(),
        ttl_s: 120,
        approval: None,
        defaults: None,
        self_awareness: None,
        plugin_scan: Some(PluginScanSpec {
            enabled: true,
            roots: vec![plugin_root.display().to_string()],
        }),
        bridge_support: Some(BridgeSupportSpec {
            enabled: true,
            supported_bridges: vec![PluginBridgeKind::ProcessStdio],
            supported_adapter_families: Vec::new(),
            enforce_supported: true,
            policy_version: None,
            expected_checksum: None,
            expected_sha256: None,
            execute_process_stdio: true,
            execute_http_json: false,
            allowed_process_commands: vec!["printf".to_owned()],
            enforce_execution_success: false,
            security_scan: None,
        }),
        bootstrap: Some(BootstrapSpec {
            enabled: true,
            allow_http_json_auto_apply: Some(false),
            allow_process_stdio_auto_apply: Some(true),
            allow_native_ffi_auto_apply: Some(false),
            allow_wasm_component_auto_apply: Some(false),
            allow_mcp_server_auto_apply: Some(false),
            enforce_ready_execution: Some(true),
            max_tasks: Some(10),
        }),
        auto_provision: None,
        hotfixes: Vec::new(),
        operation: OperationSpec::ConnectorLegacy {
            connector_name: "stdio-invalid-frame-provider".to_owned(),
            operation: "invoke".to_owned(),
            required_capabilities: BTreeSet::from([Capability::InvokeConnector]),
            payload: json!({"question":"ping"}),
        },
    };

    let report = execute_spec(spec, true).await;
    assert_eq!(report.operation_kind, "connector_legacy");
    assert_eq!(report.outcome["outcome"]["status"], "ok");
    assert_eq!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["status"],
        "failed"
    );
    assert!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["reason"]
            .as_str()
            .expect("failed reason should be string")
            .contains("failed to decode inbound frame")
    );
    assert_eq!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["runtime"]["transport_kind"],
        "json_line"
    );
}

#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn execute_spec_process_stdio_bridge_fails_on_response_id_mismatch() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic")
        .as_nanos();
    let plugin_root = std::env::temp_dir().join(format!(
        "loongclaw-plugin-process-stdio-mismatch-id-{unique}"
    ));
    fs::create_dir_all(&plugin_root).expect("create plugin root");

    fs::write(
        plugin_root.join("stdio_plugin.py"),
        r#"
# LOONGCLAW_PLUGIN_START
# {
#   "plugin_id": "stdio-mismatch-id-plugin",
#   "provider_id": "stdio-mismatch-id-provider",
#   "connector_name": "stdio-mismatch-id-provider",
#   "channel_id": "primary",
#   "endpoint": "local://stdio-mismatch-id-provider",
#   "capabilities": ["InvokeConnector"],
#   "metadata": {
#     "bridge_kind":"process_stdio",
#     "command":"printf",
#     "args_json":"[\"{\\\"method\\\":\\\"tools/call\\\",\\\"id\\\":\\\"wrong-id\\\",\\\"payload\\\":{\\\"ok\\\":true}}\\n\"]",
#     "version":"1.0.0"
#   }
# }
# LOONGCLAW_PLUGIN_END
"#,
    )
    .expect("write stdio plugin");

    let spec = RunnerSpec {
        pack: VerticalPackManifest {
            pack_id: "spec-process-stdio-mismatch-id".to_owned(),
            domain: "ops".to_owned(),
            version: "0.1.0".to_owned(),
            default_route: ExecutionRoute {
                harness_kind: HarnessKind::EmbeddedPi,
                adapter: Some("pi-local".to_owned()),
            },
            allowed_connectors: BTreeSet::new(),
            granted_capabilities: BTreeSet::new(),
            metadata: BTreeMap::new(),
        },
        agent_id: "agent-process-stdio-mismatch-id".to_owned(),
        ttl_s: 120,
        approval: None,
        defaults: None,
        self_awareness: None,
        plugin_scan: Some(PluginScanSpec {
            enabled: true,
            roots: vec![plugin_root.display().to_string()],
        }),
        bridge_support: Some(BridgeSupportSpec {
            enabled: true,
            supported_bridges: vec![PluginBridgeKind::ProcessStdio],
            supported_adapter_families: Vec::new(),
            enforce_supported: true,
            policy_version: None,
            expected_checksum: None,
            expected_sha256: None,
            execute_process_stdio: true,
            execute_http_json: false,
            allowed_process_commands: vec!["printf".to_owned()],
            enforce_execution_success: false,
            security_scan: None,
        }),
        bootstrap: Some(BootstrapSpec {
            enabled: true,
            allow_http_json_auto_apply: Some(false),
            allow_process_stdio_auto_apply: Some(true),
            allow_native_ffi_auto_apply: Some(false),
            allow_wasm_component_auto_apply: Some(false),
            allow_mcp_server_auto_apply: Some(false),
            enforce_ready_execution: Some(true),
            max_tasks: Some(10),
        }),
        auto_provision: None,
        hotfixes: Vec::new(),
        operation: OperationSpec::ConnectorLegacy {
            connector_name: "stdio-mismatch-id-provider".to_owned(),
            operation: "invoke".to_owned(),
            required_capabilities: BTreeSet::from([Capability::InvokeConnector]),
            payload: json!({"question":"ping"}),
        },
    };

    let report = execute_spec(spec, true).await;
    assert_eq!(report.operation_kind, "connector_legacy");
    assert_eq!(report.outcome["outcome"]["status"], "ok");
    assert_eq!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["status"],
        "failed"
    );
    assert!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["reason"]
            .as_str()
            .expect("failed reason should be string")
            .contains("response id mismatch")
    );
}

#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn execute_spec_process_stdio_bridge_fails_on_response_method_mismatch() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic")
        .as_nanos();
    let plugin_root = std::env::temp_dir().join(format!(
        "loongclaw-plugin-process-stdio-mismatch-method-{unique}"
    ));
    fs::create_dir_all(&plugin_root).expect("create plugin root");

    fs::write(
        plugin_root.join("stdio_plugin.py"),
        r#"
# LOONGCLAW_PLUGIN_START
# {
#   "plugin_id": "stdio-mismatch-method-plugin",
#   "provider_id": "stdio-mismatch-method-provider",
#   "connector_name": "stdio-mismatch-method-provider",
#   "channel_id": "primary",
#   "endpoint": "local://stdio-mismatch-method-provider",
#   "capabilities": ["InvokeConnector"],
#   "metadata": {
#     "bridge_kind":"process_stdio",
#     "command":"printf",
#     "args_json":"[\"{\\\"method\\\":\\\"tools/list\\\",\\\"id\\\":\\\"stdio-mismatch-method-provider:primary:invoke\\\",\\\"payload\\\":{\\\"ok\\\":true}}\\n\"]",
#     "version":"1.0.0"
#   }
# }
# LOONGCLAW_PLUGIN_END
"#,
    )
    .expect("write stdio plugin");

    let spec = RunnerSpec {
        pack: VerticalPackManifest {
            pack_id: "spec-process-stdio-mismatch-method".to_owned(),
            domain: "ops".to_owned(),
            version: "0.1.0".to_owned(),
            default_route: ExecutionRoute {
                harness_kind: HarnessKind::EmbeddedPi,
                adapter: Some("pi-local".to_owned()),
            },
            allowed_connectors: BTreeSet::new(),
            granted_capabilities: BTreeSet::new(),
            metadata: BTreeMap::new(),
        },
        agent_id: "agent-process-stdio-mismatch-method".to_owned(),
        ttl_s: 120,
        approval: None,
        defaults: None,
        self_awareness: None,
        plugin_scan: Some(PluginScanSpec {
            enabled: true,
            roots: vec![plugin_root.display().to_string()],
        }),
        bridge_support: Some(BridgeSupportSpec {
            enabled: true,
            supported_bridges: vec![PluginBridgeKind::ProcessStdio],
            supported_adapter_families: Vec::new(),
            enforce_supported: true,
            policy_version: None,
            expected_checksum: None,
            expected_sha256: None,
            execute_process_stdio: true,
            execute_http_json: false,
            allowed_process_commands: vec!["printf".to_owned()],
            enforce_execution_success: false,
            security_scan: None,
        }),
        bootstrap: Some(BootstrapSpec {
            enabled: true,
            allow_http_json_auto_apply: Some(false),
            allow_process_stdio_auto_apply: Some(true),
            allow_native_ffi_auto_apply: Some(false),
            allow_wasm_component_auto_apply: Some(false),
            allow_mcp_server_auto_apply: Some(false),
            enforce_ready_execution: Some(true),
            max_tasks: Some(10),
        }),
        auto_provision: None,
        hotfixes: Vec::new(),
        operation: OperationSpec::ConnectorLegacy {
            connector_name: "stdio-mismatch-method-provider".to_owned(),
            operation: "invoke".to_owned(),
            required_capabilities: BTreeSet::from([Capability::InvokeConnector]),
            payload: json!({"question":"ping"}),
        },
    };

    let report = execute_spec(spec, true).await;
    assert_eq!(report.operation_kind, "connector_legacy");
    assert_eq!(report.outcome["outcome"]["status"], "ok");
    assert_eq!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["status"],
        "failed"
    );
    assert!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["reason"]
            .as_str()
            .expect("failed reason should be string")
            .contains("response method mismatch")
    );
}

#[tokio::test]
async fn execute_spec_process_stdio_bridge_blocks_when_protocol_authorization_fails() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic")
        .as_nanos();
    let plugin_root = std::env::temp_dir().join(format!(
        "loongclaw-plugin-process-stdio-authz-block-{unique}"
    ));
    fs::create_dir_all(&plugin_root).expect("create plugin root");

    fs::write(
        plugin_root.join("stdio_plugin.py"),
        r#"
# LOONGCLAW_PLUGIN_START
# {
#   "plugin_id": "stdio-authz-block-plugin",
#   "provider_id": "stdio-authz-block-provider",
#   "connector_name": "stdio-authz-block-provider",
#   "channel_id": "primary",
#   "endpoint": "local://stdio-authz-block-provider",
#   "capabilities": ["InvokeConnector"],
#   "metadata": {
#     "bridge_kind":"process_stdio",
#     "command":"cat",
#     "process_timeout_ms":"999999999",
#     "version":"1.0.0"
#   }
# }
# LOONGCLAW_PLUGIN_END
"#,
    )
    .expect("write stdio plugin");

    let spec = RunnerSpec {
        pack: VerticalPackManifest {
            pack_id: "spec-process-stdio-authz-block".to_owned(),
            domain: "ops".to_owned(),
            version: "0.1.0".to_owned(),
            default_route: ExecutionRoute {
                harness_kind: HarnessKind::EmbeddedPi,
                adapter: Some("pi-local".to_owned()),
            },
            allowed_connectors: BTreeSet::new(),
            granted_capabilities: BTreeSet::from([Capability::MemoryRead]),
            metadata: BTreeMap::new(),
        },
        agent_id: "agent-process-stdio-authz-block".to_owned(),
        ttl_s: 120,
        approval: None,
        defaults: None,
        self_awareness: None,
        plugin_scan: Some(PluginScanSpec {
            enabled: true,
            roots: vec![plugin_root.display().to_string()],
        }),
        bridge_support: Some(BridgeSupportSpec {
            enabled: true,
            supported_bridges: vec![PluginBridgeKind::ProcessStdio],
            supported_adapter_families: Vec::new(),
            enforce_supported: true,
            policy_version: None,
            expected_checksum: None,
            expected_sha256: None,
            execute_process_stdio: true,
            execute_http_json: false,
            allowed_process_commands: vec!["cat".to_owned()],
            enforce_execution_success: false,
            security_scan: None,
        }),
        bootstrap: Some(BootstrapSpec {
            enabled: true,
            allow_http_json_auto_apply: Some(false),
            allow_process_stdio_auto_apply: Some(true),
            allow_native_ffi_auto_apply: Some(false),
            allow_wasm_component_auto_apply: Some(false),
            allow_mcp_server_auto_apply: Some(false),
            enforce_ready_execution: Some(true),
            max_tasks: Some(10),
        }),
        auto_provision: None,
        hotfixes: Vec::new(),
        operation: OperationSpec::ConnectorLegacy {
            connector_name: "stdio-authz-block-provider".to_owned(),
            operation: "invoke".to_owned(),
            required_capabilities: BTreeSet::from([Capability::MemoryRead]),
            payload: json!({"question":"ping"}),
        },
    };

    let report = execute_spec(spec, true).await;
    assert_eq!(report.operation_kind, "connector_legacy");
    assert_eq!(report.outcome["outcome"]["status"], "ok");
    assert_eq!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["status"],
        "blocked"
    );
    assert!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["reason"]
            .as_str()
            .expect("blocked reason should be string")
            .contains("protocol route authorization failed")
    );
    let runtime = &report.outcome["outcome"]["payload"]["bridge_execution"]["runtime"];
    assert_process_stdio_runtime_shape(runtime);
    assert_runtime_keys_exact(
        runtime,
        &[
            "args",
            "command",
            "executor",
            "protocol_capabilities",
            "protocol_required_capability",
            "protocol_route",
            "request_id",
            "request_method",
            "timeout_ms",
            "transport_kind",
        ],
    );
    assert!(runtime.get("exit_code").is_none());
    assert!(runtime.get("response_method").is_none());
    assert!(runtime.get("response_id").is_none());
    assert_bridge_runtime_protocol_context(
        runtime,
        "stdio-authz-block-provider:primary:invoke",
        "invoke",
        "discover",
    );
    assert_eq!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["runtime"]["timeout_ms"],
        300000
    );
}

#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn execute_spec_process_stdio_bridge_fails_on_recv_timeout() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic")
        .as_nanos();
    let plugin_root =
        std::env::temp_dir().join(format!("loongclaw-plugin-process-stdio-timeout-{unique}"));
    fs::create_dir_all(&plugin_root).expect("create plugin root");

    fs::write(
        plugin_root.join("stdio_plugin.py"),
        r#"
# LOONGCLAW_PLUGIN_START
# {
#   "plugin_id": "stdio-timeout-plugin",
#   "provider_id": "stdio-timeout-provider",
#   "connector_name": "stdio-timeout-provider",
#   "channel_id": "primary",
#   "endpoint": "local://stdio-timeout-provider",
#   "capabilities": ["InvokeConnector"],
#   "metadata": {
#     "bridge_kind":"process_stdio",
#     "command":"sleep",
#     "args_json":"[\"0.2\"]",
#     "process_timeout_ms":"50",
#     "version":"1.0.0"
#   }
# }
# LOONGCLAW_PLUGIN_END
"#,
    )
    .expect("write stdio plugin");

    let spec = RunnerSpec {
        pack: VerticalPackManifest {
            pack_id: "spec-process-stdio-timeout".to_owned(),
            domain: "ops".to_owned(),
            version: "0.1.0".to_owned(),
            default_route: ExecutionRoute {
                harness_kind: HarnessKind::EmbeddedPi,
                adapter: Some("pi-local".to_owned()),
            },
            allowed_connectors: BTreeSet::new(),
            granted_capabilities: BTreeSet::new(),
            metadata: BTreeMap::new(),
        },
        agent_id: "agent-process-stdio-timeout".to_owned(),
        ttl_s: 120,
        approval: None,
        defaults: None,
        self_awareness: None,
        plugin_scan: Some(PluginScanSpec {
            enabled: true,
            roots: vec![plugin_root.display().to_string()],
        }),
        bridge_support: Some(BridgeSupportSpec {
            enabled: true,
            supported_bridges: vec![PluginBridgeKind::ProcessStdio],
            supported_adapter_families: Vec::new(),
            enforce_supported: true,
            policy_version: None,
            expected_checksum: None,
            expected_sha256: None,
            execute_process_stdio: true,
            execute_http_json: false,
            allowed_process_commands: vec!["sleep".to_owned()],
            enforce_execution_success: false,
            security_scan: None,
        }),
        bootstrap: Some(BootstrapSpec {
            enabled: true,
            allow_http_json_auto_apply: Some(false),
            allow_process_stdio_auto_apply: Some(true),
            allow_native_ffi_auto_apply: Some(false),
            allow_wasm_component_auto_apply: Some(false),
            allow_mcp_server_auto_apply: Some(false),
            enforce_ready_execution: Some(true),
            max_tasks: Some(10),
        }),
        auto_provision: None,
        hotfixes: Vec::new(),
        operation: OperationSpec::ConnectorLegacy {
            connector_name: "stdio-timeout-provider".to_owned(),
            operation: "invoke".to_owned(),
            required_capabilities: BTreeSet::from([Capability::InvokeConnector]),
            payload: json!({"question":"ping"}),
        },
    };

    let report = execute_spec(spec, true).await;
    assert_eq!(report.operation_kind, "connector_legacy");
    assert_eq!(report.outcome["outcome"]["status"], "ok");
    assert_eq!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["status"],
        "failed"
    );
    assert!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["reason"]
            .as_str()
            .expect("failed reason should be string")
            .contains("timed out")
    );
    assert_eq!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["runtime"]["timeout_ms"],
        50
    );
}

#[tokio::test]
async fn execute_spec_http_json_bridge_executes_against_local_server() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::{SystemTime, UNIX_EPOCH};

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test listener");
    let addr = listener.local_addr().expect("local addr");
    let server = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request_buf = [0_u8; 4096];
            let _ = stream.read(&mut request_buf);
            let body = r#"{"status":"ok","reply":"pong"}"#;
            let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
            let _ = stream.write_all(response.as_bytes());
        }
    });

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic")
        .as_nanos();
    let plugin_root = std::env::temp_dir().join(format!("loongclaw-http-runtime-{unique}"));
    fs::create_dir_all(&plugin_root).expect("create plugin root");

    fs::write(
        plugin_root.join("http_plugin.py"),
        format!(
            r#"
# LOONGCLAW_PLUGIN_START
# {{
#   "plugin_id": "http-runtime",
#   "provider_id": "http-runtime",
#   "connector_name": "http-runtime",
#   "channel_id": "primary",
#   "endpoint": "http://{addr}/invoke",
#   "capabilities": ["InvokeConnector"],
#   "metadata": {{
#     "bridge_kind":"http_json",
#     "http_method":"POST",
#     "http_timeout_ms":"3000",
#     "version":"1.0.0"
#   }}
# }}
# LOONGCLAW_PLUGIN_END
"#
        ),
    )
    .expect("write http plugin");

    let spec = RunnerSpec {
        pack: VerticalPackManifest {
            pack_id: "spec-http-runtime".to_owned(),
            domain: "ops".to_owned(),
            version: "0.1.0".to_owned(),
            default_route: ExecutionRoute {
                harness_kind: HarnessKind::EmbeddedPi,
                adapter: Some("pi-local".to_owned()),
            },
            allowed_connectors: BTreeSet::new(),
            granted_capabilities: BTreeSet::from([Capability::InvokeConnector]),
            metadata: BTreeMap::new(),
        },
        agent_id: "agent-http-runtime".to_owned(),
        ttl_s: 120,
        approval: Some(HumanApprovalSpec {
            mode: HumanApprovalMode::Disabled,
            ..HumanApprovalSpec::default()
        }),
        defaults: None,
        self_awareness: None,
        plugin_scan: Some(PluginScanSpec {
            enabled: true,
            roots: vec![plugin_root.display().to_string()],
        }),
        bridge_support: Some(BridgeSupportSpec {
            enabled: true,
            supported_bridges: vec![PluginBridgeKind::HttpJson],
            supported_adapter_families: Vec::new(),
            enforce_supported: true,
            policy_version: None,
            expected_checksum: None,
            expected_sha256: None,
            execute_process_stdio: false,
            execute_http_json: true,
            allowed_process_commands: Vec::new(),
            enforce_execution_success: true,
            security_scan: None,
        }),
        bootstrap: None,
        auto_provision: None,
        hotfixes: Vec::new(),
        operation: OperationSpec::ConnectorLegacy {
            connector_name: "http-runtime".to_owned(),
            operation: "invoke".to_owned(),
            required_capabilities: BTreeSet::from([Capability::InvokeConnector]),
            payload: json!({"query":"ping"}),
        },
    };

    let report = execute_spec(spec, true).await;
    server.join().expect("join local http server");
    let runtime = &report.outcome["outcome"]["payload"]["bridge_execution"]["runtime"];

    assert_eq!(report.operation_kind, "connector_legacy");
    assert_eq!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["status"],
        "executed"
    );
    assert_http_json_runtime_shape(runtime);
    assert_runtime_keys_exact(
        runtime,
        &[
            "enforce_protocol_contract",
            "executor",
            "method",
            "protocol_capabilities",
            "protocol_required_capability",
            "protocol_route",
            "request",
            "request_id",
            "request_method",
            "response_json",
            "response_text",
            "status_code",
            "timeout_ms",
            "url",
        ],
    );
    assert_eq!(runtime["response_json"]["reply"], "pong");
    assert!(runtime.get("status_code").is_some());
    assert!(runtime.get("response_text").is_some());
    assert!(runtime.get("response_method").is_none());
    assert!(runtime.get("response_id").is_none());
    assert_bridge_runtime_protocol_context(
        runtime,
        "http-runtime:primary:invoke",
        "invoke",
        "invoke",
    );
}

#[tokio::test]
async fn execute_spec_http_json_bridge_blocks_when_protocol_authorization_fails() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic")
        .as_nanos();
    let plugin_root = std::env::temp_dir().join(format!("loongclaw-http-authz-block-{unique}"));
    fs::create_dir_all(&plugin_root).expect("create plugin root");

    fs::write(
        plugin_root.join("http_plugin.py"),
        r#"
# LOONGCLAW_PLUGIN_START
# {
#   "plugin_id": "http-authz-block",
#   "provider_id": "http-authz-block",
#   "connector_name": "http-authz-block",
#   "channel_id": "primary",
#   "endpoint": "http://127.0.0.1:9/invoke",
#   "capabilities": ["InvokeConnector"],
#   "metadata": {
#     "bridge_kind":"http_json",
#     "http_method":"POST",
#     "http_timeout_ms":"999999999",
#     "version":"1.0.0"
#   }
# }
# LOONGCLAW_PLUGIN_END
"#,
    )
    .expect("write http plugin");

    let spec = RunnerSpec {
        pack: VerticalPackManifest {
            pack_id: "spec-http-authz-block".to_owned(),
            domain: "ops".to_owned(),
            version: "0.1.0".to_owned(),
            default_route: ExecutionRoute {
                harness_kind: HarnessKind::EmbeddedPi,
                adapter: Some("pi-local".to_owned()),
            },
            allowed_connectors: BTreeSet::new(),
            granted_capabilities: BTreeSet::from([Capability::MemoryRead]),
            metadata: BTreeMap::new(),
        },
        agent_id: "agent-http-authz-block".to_owned(),
        ttl_s: 120,
        approval: Some(HumanApprovalSpec {
            mode: HumanApprovalMode::Disabled,
            ..HumanApprovalSpec::default()
        }),
        defaults: None,
        self_awareness: None,
        plugin_scan: Some(PluginScanSpec {
            enabled: true,
            roots: vec![plugin_root.display().to_string()],
        }),
        bridge_support: Some(BridgeSupportSpec {
            enabled: true,
            supported_bridges: vec![PluginBridgeKind::HttpJson],
            supported_adapter_families: Vec::new(),
            enforce_supported: true,
            policy_version: None,
            expected_checksum: None,
            expected_sha256: None,
            execute_process_stdio: false,
            execute_http_json: true,
            allowed_process_commands: Vec::new(),
            enforce_execution_success: false,
            security_scan: None,
        }),
        bootstrap: None,
        auto_provision: None,
        hotfixes: Vec::new(),
        operation: OperationSpec::ConnectorLegacy {
            connector_name: "http-authz-block".to_owned(),
            operation: "invoke".to_owned(),
            required_capabilities: BTreeSet::from([Capability::MemoryRead]),
            payload: json!({"query":"ping"}),
        },
    };

    let report = execute_spec(spec, true).await;
    let runtime = &report.outcome["outcome"]["payload"]["bridge_execution"]["runtime"];
    assert_eq!(report.operation_kind, "connector_legacy");
    assert_eq!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["status"],
        "blocked"
    );
    assert!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["reason"]
            .as_str()
            .expect("blocked reason should be string")
            .contains("protocol route authorization failed")
    );
    assert_http_json_runtime_shape(runtime);
    assert_runtime_keys_exact(
        runtime,
        &[
            "enforce_protocol_contract",
            "executor",
            "method",
            "protocol_capabilities",
            "protocol_required_capability",
            "protocol_route",
            "request_id",
            "request_method",
            "timeout_ms",
            "url",
        ],
    );
    assert!(runtime.get("status_code").is_none());
    assert!(runtime.get("response_text").is_none());
    assert!(runtime.get("response_json").is_none());
    assert!(runtime.get("response_method").is_none());
    assert!(runtime.get("response_id").is_none());
    assert_bridge_runtime_protocol_context(
        runtime,
        "http-authz-block:primary:invoke",
        "invoke",
        "discover",
    );
    assert_eq!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["runtime"]["timeout_ms"],
        300000
    );
}

#[tokio::test]
async fn execute_spec_http_json_bridge_strict_contract_fails_on_method_mismatch() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::{SystemTime, UNIX_EPOCH};

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test listener");
    let addr = listener.local_addr().expect("local addr");
    let server = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request_buf = [0_u8; 4096];
            let _ = stream.read(&mut request_buf);
            let body = r#"{"method":"tools/list","id":"http-strict-method:primary:invoke","payload":{"reply":"pong"}}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic")
        .as_nanos();
    let plugin_root =
        std::env::temp_dir().join(format!("loongclaw-http-strict-method-mismatch-{unique}"));
    fs::create_dir_all(&plugin_root).expect("create plugin root");

    fs::write(
        plugin_root.join("http_plugin.py"),
        format!(
            r#"
# LOONGCLAW_PLUGIN_START
# {{
#   "plugin_id": "http-strict-method",
#   "provider_id": "http-strict-method",
#   "connector_name": "http-strict-method",
#   "channel_id": "primary",
#   "endpoint": "http://{addr}/invoke",
#   "capabilities": ["InvokeConnector"],
#   "metadata": {{
#     "bridge_kind":"http_json",
#     "http_method":"POST",
#     "http_enforce_protocol_contract":"true",
#     "http_timeout_ms":"3000",
#     "version":"1.0.0"
#   }}
# }}
# LOONGCLAW_PLUGIN_END
"#
        ),
    )
    .expect("write http plugin");

    let spec = RunnerSpec {
        pack: VerticalPackManifest {
            pack_id: "spec-http-strict-method".to_owned(),
            domain: "ops".to_owned(),
            version: "0.1.0".to_owned(),
            default_route: ExecutionRoute {
                harness_kind: HarnessKind::EmbeddedPi,
                adapter: Some("pi-local".to_owned()),
            },
            allowed_connectors: BTreeSet::new(),
            granted_capabilities: BTreeSet::from([Capability::InvokeConnector]),
            metadata: BTreeMap::new(),
        },
        agent_id: "agent-http-strict-method".to_owned(),
        ttl_s: 120,
        approval: Some(HumanApprovalSpec {
            mode: HumanApprovalMode::Disabled,
            ..HumanApprovalSpec::default()
        }),
        defaults: None,
        self_awareness: None,
        plugin_scan: Some(PluginScanSpec {
            enabled: true,
            roots: vec![plugin_root.display().to_string()],
        }),
        bridge_support: Some(BridgeSupportSpec {
            enabled: true,
            supported_bridges: vec![PluginBridgeKind::HttpJson],
            supported_adapter_families: Vec::new(),
            enforce_supported: true,
            policy_version: None,
            expected_checksum: None,
            expected_sha256: None,
            execute_process_stdio: false,
            execute_http_json: true,
            allowed_process_commands: Vec::new(),
            enforce_execution_success: false,
            security_scan: None,
        }),
        bootstrap: None,
        auto_provision: None,
        hotfixes: Vec::new(),
        operation: OperationSpec::ConnectorLegacy {
            connector_name: "http-strict-method".to_owned(),
            operation: "invoke".to_owned(),
            required_capabilities: BTreeSet::from([Capability::InvokeConnector]),
            payload: json!({"query":"ping"}),
        },
    };

    let report = execute_spec(spec, true).await;
    server.join().expect("join local http server");
    let runtime = &report.outcome["outcome"]["payload"]["bridge_execution"]["runtime"];

    assert_eq!(report.operation_kind, "connector_legacy");
    assert_eq!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["status"],
        "failed"
    );
    assert!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["reason"]
            .as_str()
            .expect("failed reason should be string")
            .contains("response method mismatch")
    );
    assert_runtime_keys_exact(
        runtime,
        &[
            "enforce_protocol_contract",
            "executor",
            "method",
            "protocol_capabilities",
            "protocol_required_capability",
            "protocol_route",
            "request",
            "request_id",
            "request_method",
            "timeout_ms",
            "url",
        ],
    );
}

#[tokio::test]
async fn execute_spec_http_json_bridge_strict_contract_fails_on_id_mismatch() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::{SystemTime, UNIX_EPOCH};

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test listener");
    let addr = listener.local_addr().expect("local addr");
    let server = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request_buf = [0_u8; 4096];
            let _ = stream.read(&mut request_buf);
            let body = r#"{"method":"tools/call","id":"http-strict-id:primary:other","payload":{"reply":"pong"}}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic")
        .as_nanos();
    let plugin_root =
        std::env::temp_dir().join(format!("loongclaw-http-strict-id-mismatch-{unique}"));
    fs::create_dir_all(&plugin_root).expect("create plugin root");

    fs::write(
        plugin_root.join("http_plugin.py"),
        format!(
            r#"
# LOONGCLAW_PLUGIN_START
# {{
#   "plugin_id": "http-strict-id",
#   "provider_id": "http-strict-id",
#   "connector_name": "http-strict-id",
#   "channel_id": "primary",
#   "endpoint": "http://{addr}/invoke",
#   "capabilities": ["InvokeConnector"],
#   "metadata": {{
#     "bridge_kind":"http_json",
#     "http_method":"POST",
#     "http_enforce_protocol_contract":"true",
#     "http_timeout_ms":"3000",
#     "version":"1.0.0"
#   }}
# }}
# LOONGCLAW_PLUGIN_END
"#
        ),
    )
    .expect("write http plugin");

    let spec = RunnerSpec {
        pack: VerticalPackManifest {
            pack_id: "spec-http-strict-id".to_owned(),
            domain: "ops".to_owned(),
            version: "0.1.0".to_owned(),
            default_route: ExecutionRoute {
                harness_kind: HarnessKind::EmbeddedPi,
                adapter: Some("pi-local".to_owned()),
            },
            allowed_connectors: BTreeSet::new(),
            granted_capabilities: BTreeSet::from([Capability::InvokeConnector]),
            metadata: BTreeMap::new(),
        },
        agent_id: "agent-http-strict-id".to_owned(),
        ttl_s: 120,
        approval: Some(HumanApprovalSpec {
            mode: HumanApprovalMode::Disabled,
            ..HumanApprovalSpec::default()
        }),
        defaults: None,
        self_awareness: None,
        plugin_scan: Some(PluginScanSpec {
            enabled: true,
            roots: vec![plugin_root.display().to_string()],
        }),
        bridge_support: Some(BridgeSupportSpec {
            enabled: true,
            supported_bridges: vec![PluginBridgeKind::HttpJson],
            supported_adapter_families: Vec::new(),
            enforce_supported: true,
            policy_version: None,
            expected_checksum: None,
            expected_sha256: None,
            execute_process_stdio: false,
            execute_http_json: true,
            allowed_process_commands: Vec::new(),
            enforce_execution_success: false,
            security_scan: None,
        }),
        bootstrap: None,
        auto_provision: None,
        hotfixes: Vec::new(),
        operation: OperationSpec::ConnectorLegacy {
            connector_name: "http-strict-id".to_owned(),
            operation: "invoke".to_owned(),
            required_capabilities: BTreeSet::from([Capability::InvokeConnector]),
            payload: json!({"query":"ping"}),
        },
    };

    let report = execute_spec(spec, true).await;
    server.join().expect("join local http server");
    let runtime = &report.outcome["outcome"]["payload"]["bridge_execution"]["runtime"];

    assert_eq!(report.operation_kind, "connector_legacy");
    assert_eq!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["status"],
        "failed"
    );
    assert!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["reason"]
            .as_str()
            .expect("failed reason should be string")
            .contains("response id mismatch")
    );
    assert_runtime_keys_exact(
        runtime,
        &[
            "enforce_protocol_contract",
            "executor",
            "method",
            "protocol_capabilities",
            "protocol_required_capability",
            "protocol_route",
            "request",
            "request_id",
            "request_method",
            "timeout_ms",
            "url",
        ],
    );
}

#[tokio::test]
async fn execute_spec_http_json_bridge_strict_contract_executes_on_matching_frame() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::{SystemTime, UNIX_EPOCH};

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test listener");
    let addr = listener.local_addr().expect("local addr");
    let expected_id = "http-strict-pass:primary:invoke".to_owned();
    let expected_id_for_server = expected_id.clone();
    let server = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request_buf = [0_u8; 4096];
            let _ = stream.read(&mut request_buf);
            let body = format!(
                "{{\"method\":\"tools/call\",\"id\":\"{}\",\"payload\":{{\"reply\":\"pong\"}}}}",
                expected_id_for_server
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic")
        .as_nanos();
    let plugin_root = std::env::temp_dir().join(format!("loongclaw-http-strict-pass-{unique}"));
    fs::create_dir_all(&plugin_root).expect("create plugin root");

    fs::write(
        plugin_root.join("http_plugin.py"),
        format!(
            r#"
# LOONGCLAW_PLUGIN_START
# {{
#   "plugin_id": "http-strict-pass",
#   "provider_id": "http-strict-pass",
#   "connector_name": "http-strict-pass",
#   "channel_id": "primary",
#   "endpoint": "http://{addr}/invoke",
#   "capabilities": ["InvokeConnector"],
#   "metadata": {{
#     "bridge_kind":"http_json",
#     "http_method":"POST",
#     "http_enforce_protocol_contract":"true",
#     "http_timeout_ms":"3000",
#     "version":"1.0.0"
#   }}
# }}
# LOONGCLAW_PLUGIN_END
"#
        ),
    )
    .expect("write http plugin");

    let spec = RunnerSpec {
        pack: VerticalPackManifest {
            pack_id: "spec-http-strict-pass".to_owned(),
            domain: "ops".to_owned(),
            version: "0.1.0".to_owned(),
            default_route: ExecutionRoute {
                harness_kind: HarnessKind::EmbeddedPi,
                adapter: Some("pi-local".to_owned()),
            },
            allowed_connectors: BTreeSet::new(),
            granted_capabilities: BTreeSet::from([Capability::InvokeConnector]),
            metadata: BTreeMap::new(),
        },
        agent_id: "agent-http-strict-pass".to_owned(),
        ttl_s: 120,
        approval: Some(HumanApprovalSpec {
            mode: HumanApprovalMode::Disabled,
            ..HumanApprovalSpec::default()
        }),
        defaults: None,
        self_awareness: None,
        plugin_scan: Some(PluginScanSpec {
            enabled: true,
            roots: vec![plugin_root.display().to_string()],
        }),
        bridge_support: Some(BridgeSupportSpec {
            enabled: true,
            supported_bridges: vec![PluginBridgeKind::HttpJson],
            supported_adapter_families: Vec::new(),
            enforce_supported: true,
            policy_version: None,
            expected_checksum: None,
            expected_sha256: None,
            execute_process_stdio: false,
            execute_http_json: true,
            allowed_process_commands: Vec::new(),
            enforce_execution_success: true,
            security_scan: None,
        }),
        bootstrap: None,
        auto_provision: None,
        hotfixes: Vec::new(),
        operation: OperationSpec::ConnectorLegacy {
            connector_name: "http-strict-pass".to_owned(),
            operation: "invoke".to_owned(),
            required_capabilities: BTreeSet::from([Capability::InvokeConnector]),
            payload: json!({"query":"ping"}),
        },
    };

    let report = execute_spec(spec, true).await;
    server.join().expect("join local http server");
    let runtime = &report.outcome["outcome"]["payload"]["bridge_execution"]["runtime"];

    assert_eq!(report.operation_kind, "connector_legacy");
    assert_eq!(
        report.outcome["outcome"]["payload"]["bridge_execution"]["status"],
        "executed"
    );
    assert_eq!(runtime["response_method"], "tools/call");
    assert_eq!(runtime["response_id"], expected_id);
    assert_runtime_keys_exact(
        runtime,
        &[
            "enforce_protocol_contract",
            "executor",
            "method",
            "protocol_capabilities",
            "protocol_required_capability",
            "protocol_route",
            "request",
            "request_id",
            "request_method",
            "response_id",
            "response_json",
            "response_method",
            "response_text",
            "status_code",
            "timeout_ms",
            "url",
        ],
    );
}
