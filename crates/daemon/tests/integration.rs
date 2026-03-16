#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    unused_imports,
    dead_code
)]
use std::time::Duration;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use loongclaw_daemon::kernel::ConnectorCommand;
use loongclaw_daemon::kernel::{
    AuditEventKind, Capability, ExecutionRoute, HarnessKind, PluginBridgeKind, VerticalPackManifest,
};
use loongclaw_daemon::test_support::*;
use loongclaw_daemon::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::time::sleep;

#[path = "integration/mod.rs"]
mod integration;
