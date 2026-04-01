use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use crate::mvp;
use crate::{Capability, OperationSpec};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;

pub use crate::SecurityScanProfile;

static DAEMON_TEST_ENV_LOCK: Mutex<()> = Mutex::new(());

pub fn lock_daemon_test_environment() -> MutexGuard<'static, ()> {
    DAEMON_TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn set_test_env_var(key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) {
    // SAFETY: daemon tests serialize process env mutations behind
    // `lock_daemon_test_environment`, so no concurrent env readers or writers
    // observe racy updates while these helpers are active.
    #[allow(unsafe_code, clippy::disallowed_methods)]
    unsafe {
        std::env::set_var(key, value);
    }
}

fn remove_test_env_var(key: impl AsRef<OsStr>) {
    // SAFETY: daemon tests serialize process env mutations behind
    // `lock_daemon_test_environment`, so removals are coordinated with all
    // other daemon-side env mutation helpers.
    #[allow(unsafe_code, clippy::disallowed_methods)]
    unsafe {
        std::env::remove_var(key);
    }
}

pub struct ScopedEnv {
    saved: Vec<(String, Option<OsString>)>,
    _lock: MutexGuard<'static, ()>,
}

impl ScopedEnv {
    // Daemon-side tests must share this guard so every env mutation in the
    // daemon test process is serialized behind one lock.
    pub fn new() -> Self {
        let lock = lock_daemon_test_environment();
        let saved = Vec::new();
        Self { saved, _lock: lock }
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl AsRef<OsStr>) {
        let key = key.into();
        self.capture_original(&key);
        set_test_env_var(&key, value);
    }

    pub fn remove(&mut self, key: impl Into<String>) {
        let key = key.into();
        self.capture_original(&key);
        remove_test_env_var(&key);
    }

    fn capture_original(&mut self, key: &str) {
        let already_saved = self.saved.iter().any(|(saved_key, _)| saved_key == key);
        if already_saved {
            return;
        }

        let original_value = std::env::var_os(key);
        let saved_key = key.to_owned();
        self.saved.push((saved_key, original_value));
    }
}

impl Drop for ScopedEnv {
    fn drop(&mut self) {
        while let Some((key, original_value)) = self.saved.pop() {
            match original_value {
                Some(value) => set_test_env_var(&key, value),
                None => remove_test_env_var(&key),
            }
        }
    }
}

pub fn catalog_entry(raw: &str) -> mvp::channel::ChannelCatalogEntry {
    mvp::channel::resolve_channel_catalog_entry(raw).expect("channel catalog entry")
}

pub fn catalog_command_family(raw: &str) -> mvp::channel::ChannelCatalogCommandFamilyDescriptor {
    mvp::channel::resolve_channel_catalog_command_family_descriptor(raw)
        .expect("channel catalog command family")
}

pub fn channel_send_command(raw: &str) -> &'static str {
    catalog_command_family(raw).send.command
}

pub fn channel_serve_command(raw: &str) -> &'static str {
    catalog_command_family(raw).serve.command
}

pub fn channel_capability_ids(raw: &str) -> Vec<&'static str> {
    catalog_entry(raw)
        .capabilities
        .into_iter()
        .map(|capability| capability.as_str())
        .collect()
}

pub fn channel_supported_target_kinds(raw: &str) -> Vec<&'static str> {
    catalog_entry(raw)
        .supported_target_kinds
        .into_iter()
        .map(|kind| kind.as_str())
        .collect()
}

pub fn approval_test_operation(tool_name: &str, payload: Value) -> OperationSpec {
    OperationSpec::ToolCore {
        tool_name: tool_name.to_owned(),
        required_capabilities: BTreeSet::from([Capability::InvokeTool]),
        payload,
        core: None,
    }
}

pub fn write_temp_risk_profile(path: &Path, body: &str) {
    fs::create_dir_all(
        path.parent()
            .expect("temp risk profile path should have parent directory"),
    )
    .expect("create temp risk profile directory");
    fs::write(path, body).expect("write temp risk profile");
}

pub fn sign_security_scan_profile_for_test(profile: &SecurityScanProfile) -> (String, String) {
    use crate::security_scan_profile_message;
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
    use ed25519_dalek::{Signer, SigningKey};

    let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
    let signature = signing_key.sign(&security_scan_profile_message(profile));
    let public_key_base64 = BASE64_STANDARD.encode(signing_key.verifying_key().to_bytes());
    let signature_base64 = BASE64_STANDARD.encode(signature.to_bytes());
    (public_key_base64, signature_base64)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::ScopedEnv;

    fn daemon_source_uses_forbidden_env_guard(source: &str) -> bool {
        let normalized_source = normalize_source_for_guard_scan(source);
        let direct_base_path = ["mvp", "::test_support"].concat();
        let crate_base_path = ["crate::", direct_base_path.as_str()].concat();
        let scoped_env_name = ["Scoped", "Env"].concat();
        let direct_path = [direct_base_path.as_str(), "::", scoped_env_name.as_str()].concat();
        let crate_path = [crate_base_path.as_str(), "::", scoped_env_name.as_str()].concat();
        let uses_direct_path = normalized_source.contains(&direct_path);

        if uses_direct_path {
            return true;
        }

        let uses_crate_path = normalized_source.contains(&crate_path);

        if uses_crate_path {
            return true;
        }

        let uses_direct_brace_import = brace_import_contains_scoped_env(
            &normalized_source,
            &direct_base_path,
            &scoped_env_name,
        );

        if uses_direct_brace_import {
            return true;
        }

        brace_import_contains_scoped_env(&normalized_source, &crate_base_path, &scoped_env_name)
    }

    fn normalize_source_for_guard_scan(source: &str) -> String {
        let mut normalized = String::with_capacity(source.len());

        for ch in source.chars() {
            let is_whitespace = ch.is_ascii_whitespace();

            if is_whitespace {
                continue;
            }

            normalized.push(ch);
        }

        normalized
    }

    fn brace_import_contains_scoped_env(
        normalized_source: &str,
        test_support_prefix: &str,
        scoped_env_name: &str,
    ) -> bool {
        let brace_import_prefix = [test_support_prefix, "::{"].concat();
        let mut search_start = 0;

        while let Some(relative_start) =
            normalized_source[search_start..].find(&brace_import_prefix)
        {
            let absolute_start = search_start + relative_start;
            let import_list_start = absolute_start + brace_import_prefix.len();
            let remaining_source = &normalized_source[import_list_start..];
            let closing_brace_offset = remaining_source.find('}');

            let Some(closing_brace_offset) = closing_brace_offset else {
                return false;
            };

            let import_list = &remaining_source[..closing_brace_offset];
            let has_scoped_env = import_list.contains(scoped_env_name);

            if has_scoped_env {
                return true;
            }

            search_start = import_list_start + closing_brace_offset + 1;
        }

        false
    }

    fn collect_rust_source_paths(root: &Path) -> Vec<PathBuf> {
        let mut pending_paths = vec![root.to_path_buf()];
        let mut rust_source_paths = Vec::new();

        while let Some(current_path) = pending_paths.pop() {
            let read_dir = fs::read_dir(&current_path).expect("read daemon source directory");
            let mut child_paths = Vec::new();

            for entry in read_dir {
                let entry = entry.expect("daemon source directory entry");
                let child_path = entry.path();
                child_paths.push(child_path);
            }

            child_paths.sort();

            for child_path in child_paths {
                if child_path.is_dir() {
                    pending_paths.push(child_path);
                    continue;
                }

                let extension = child_path.extension();
                let rust_extension = Some(std::ffi::OsStr::new("rs"));

                if extension == rust_extension {
                    rust_source_paths.push(child_path);
                }
            }
        }

        rust_source_paths.sort();
        rust_source_paths
    }

    #[test]
    fn scoped_env_remove_restores_original_value() {
        let key = "LOONGCLAW_SCOPED_ENV_REMOVE_TEST_KEY";
        let sentinel_value = "scoped-env-sentinel";
        let mut env = ScopedEnv::new();
        let original_value = std::env::var_os(key);

        env.set(key, sentinel_value);
        env.remove(key);

        assert!(
            std::env::var_os(key).is_none(),
            "ScopedEnv::remove should clear the environment variable while the guard is alive"
        );

        drop(env);

        assert_eq!(
            std::env::var_os(key),
            original_value,
            "ScopedEnv should restore the original environment value when dropped"
        );
    }

    #[test]
    fn daemon_source_guard_flags_mvp_scoped_env_reference() {
        let base_path = ["mvp", "::test_support"].concat();
        let scoped_env_name = ["Scoped", "Env"].concat();
        let sample_source = format!("let mut env = {base_path}::{scoped_env_name}::new();");

        assert!(
            daemon_source_uses_forbidden_env_guard(&sample_source),
            "daemon source guard should flag direct mvp scoped env references"
        );
    }

    #[test]
    fn daemon_source_guard_accepts_daemon_scoped_env_reference() {
        let sample_source = "let mut env = crate::test_support::ScopedEnv::new();";

        assert!(
            !daemon_source_uses_forbidden_env_guard(sample_source),
            "daemon source guard should allow daemon scoped env references"
        );
    }

    #[test]
    fn daemon_source_guard_flags_mvp_scoped_env_brace_import() {
        let prefix = ["crate::", "mvp", "::test_support"].concat();
        let scoped_env_name = ["Scoped", "Env"].concat();
        let sample_source = format!("use {prefix}::{{self,{scoped_env_name}}};");

        assert!(
            daemon_source_uses_forbidden_env_guard(&sample_source),
            "daemon source guard should flag mvp scoped env brace imports"
        );
    }

    #[test]
    fn daemon_source_guard_flags_multiline_mvp_scoped_env_brace_import() {
        let prefix = ["crate::", "mvp", "::test_support"].concat();
        let scoped_env_name = ["Scoped", "Env"].concat();
        let sample_source = format!(
            "use {prefix}::{{
                self,
                {scoped_env_name},
            }};"
        );

        assert!(
            daemon_source_uses_forbidden_env_guard(&sample_source),
            "daemon source guard should flag multiline mvp scoped env brace imports"
        );
    }

    #[test]
    fn daemon_test_env_source_files_do_not_use_app_scoped_env_guard() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let daemon_source_root = manifest_dir.join("src");
        let rust_source_paths = collect_rust_source_paths(&daemon_source_root);
        let mut violating_paths = Vec::new();

        for rust_source_path in rust_source_paths {
            let source = fs::read_to_string(&rust_source_path).expect("read daemon source file");
            let has_forbidden_env_guard = daemon_source_uses_forbidden_env_guard(&source);

            if has_forbidden_env_guard {
                violating_paths.push(rust_source_path);
            }
        }

        assert!(
            violating_paths.is_empty(),
            "daemon source files must not use the app scoped env guard: {violating_paths:?}"
        );
    }
}
