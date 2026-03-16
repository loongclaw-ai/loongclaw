#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    unused_imports,
    dead_code,
    unsafe_code
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

struct ScopedEnv {
    saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl ScopedEnv {
    fn new() -> Self {
        Self { saved: Vec::new() }
    }

    fn remove(&mut self, key: &'static str) {
        self.saved.push((key, std::env::var_os(key)));
        unsafe { std::env::remove_var(key) };
    }
}

impl Drop for ScopedEnv {
    fn drop(&mut self) {
        for (key, value) in self.saved.drain(..).rev() {
            match value {
                Some(value) => unsafe { std::env::set_var(key, value) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
    }
}

#[path = "integration/mod.rs"]
mod integration;
