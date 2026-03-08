use std::collections::BTreeMap;
#[cfg(any(feature = "tool-shell", feature = "tool-file"))]
use std::path::PathBuf;
#[cfg(feature = "tool-shell")]
use std::{collections::BTreeSet, process::Command};
#[cfg(feature = "tool-file")]
use std::{fs, path::Path};

use kernel::{ToolCoreOutcome, ToolCoreRequest};
use serde_json::{json, Value};

pub fn execute_tool_core(request: ToolCoreRequest) -> Result<ToolCoreOutcome, String> {
    match request.tool_name.as_str() {
        "shell.exec" | "shell_exec" | "shell" => execute_shell_tool(request),
        "file.read" | "file_read" => execute_file_read_tool(request),
        "file.write" | "file_write" => execute_file_write_tool(request),
        _ => Ok(ToolCoreOutcome {
            status: "ok".to_owned(),
            payload: json!({
                "adapter": "core-tools",
                "tool_name": request.tool_name,
                "payload": request.payload,
            }),
        }),
    }
}

fn execute_shell_tool(request: ToolCoreRequest) -> Result<ToolCoreOutcome, String> {
    #[cfg(not(feature = "tool-shell"))]
    {
        let _ = request;
        return Err(
            "shell tool is disabled in this build (enable feature `tool-shell`)".to_owned(),
        );
    }

    #[cfg(feature = "tool-shell")]
    {
        let payload = request
            .payload
            .as_object()
            .ok_or_else(|| "shell.exec payload must be an object".to_owned())?;
        let command = payload
            .get("command")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "shell.exec requires payload.command".to_owned())?;
        let args = payload
            .get("args")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_owned))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let cwd = payload
            .get("cwd")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let allowlist = shell_allowlist();
        let normalized_command = command.to_ascii_lowercase();
        if !allowlist.contains(&normalized_command) {
            return Err(format!(
                "shell command `{command}` is not allowed (allowlist={})",
                allowlist.iter().cloned().collect::<Vec<_>>().join(",")
            ));
        }

        let output = Command::new(command)
            .args(&args)
            .current_dir(&cwd)
            .output()
            .map_err(|error| format!("shell command spawn failed: {error}"))?;

        Ok(ToolCoreOutcome {
            status: if output.status.success() {
                "ok".to_owned()
            } else {
                "failed".to_owned()
            },
            payload: json!({
                "adapter": "core-tools",
                "tool_name": request.tool_name,
                "command": command,
                "args": args,
                "cwd": cwd.display().to_string(),
                "exit_code": output.status.code(),
                "stdout": String::from_utf8_lossy(&output.stdout).trim().to_owned(),
                "stderr": String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            }),
        })
    }
}

fn execute_file_read_tool(request: ToolCoreRequest) -> Result<ToolCoreOutcome, String> {
    #[cfg(not(feature = "tool-file"))]
    {
        let _ = request;
        return Err("file tool is disabled in this build (enable feature `tool-file`)".to_owned());
    }

    #[cfg(feature = "tool-file")]
    {
        let payload = request
            .payload
            .as_object()
            .ok_or_else(|| "file.read payload must be an object".to_owned())?;
        let target = payload
            .get("path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "file.read requires payload.path".to_owned())?;

        let max_bytes = payload
            .get("max_bytes")
            .and_then(Value::as_u64)
            .unwrap_or(1_048_576)
            .min(8 * 1_048_576) as usize;

        let resolved = resolve_safe_file_path(target)?;
        let bytes = fs::read(&resolved)
            .map_err(|error| format!("failed to read file {}: {error}", resolved.display()))?;
        let clipped = bytes.len() > max_bytes;
        let content_slice = if clipped { &bytes[..max_bytes] } else { &bytes };

        Ok(ToolCoreOutcome {
            status: "ok".to_owned(),
            payload: json!({
                "adapter": "core-tools",
                "tool_name": request.tool_name,
                "path": resolved.display().to_string(),
                "bytes": bytes.len(),
                "truncated": clipped,
                "content": String::from_utf8_lossy(content_slice).to_string(),
            }),
        })
    }
}

fn execute_file_write_tool(request: ToolCoreRequest) -> Result<ToolCoreOutcome, String> {
    #[cfg(not(feature = "tool-file"))]
    {
        let _ = request;
        return Err("file tool is disabled in this build (enable feature `tool-file`)".to_owned());
    }

    #[cfg(feature = "tool-file")]
    {
        let payload = request
            .payload
            .as_object()
            .ok_or_else(|| "file.write payload must be an object".to_owned())?;
        let target = payload
            .get("path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "file.write requires payload.path".to_owned())?;
        let content = payload
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| "file.write requires payload.content".to_owned())?;
        let create_dirs = payload
            .get("create_dirs")
            .and_then(Value::as_bool)
            .unwrap_or(true);

        let resolved = resolve_safe_file_path(target)?;
        if create_dirs {
            if let Some(parent) = resolved.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    format!(
                        "failed to create parent directory {}: {error}",
                        parent.display()
                    )
                })?;
            }
        }
        fs::write(&resolved, content)
            .map_err(|error| format!("failed to write file {}: {error}", resolved.display()))?;

        Ok(ToolCoreOutcome {
            status: "ok".to_owned(),
            payload: json!({
                "adapter": "core-tools",
                "tool_name": request.tool_name,
                "path": resolved.display().to_string(),
                "bytes_written": content.len(),
            }),
        })
    }
}

#[cfg(feature = "tool-shell")]
fn shell_allowlist() -> BTreeSet<String> {
    let from_env = std::env::var("LOONGCLAW_SHELL_ALLOWLIST")
        .ok()
        .unwrap_or_else(|| "echo,cat,ls,pwd".to_owned());
    from_env
        .split([',', ';', ' '])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

#[cfg(feature = "tool-file")]
fn resolve_safe_file_path(raw: &str) -> Result<PathBuf, String> {
    let root = std::env::var("LOONGCLAW_FILE_ROOT")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let root = canonicalize_or_fallback(root)?;

    let candidate = Path::new(raw);
    let combined = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };
    let normalized = normalize_without_fs_access(&combined);
    if !normalized.starts_with(&root) {
        return Err(format!(
            "file path {} escapes configured file root {}",
            normalized.display(),
            root.display()
        ));
    }
    Ok(normalized)
}

#[cfg(feature = "tool-file")]
fn canonicalize_or_fallback(path: PathBuf) -> Result<PathBuf, String> {
    if path.exists() {
        return fs::canonicalize(&path)
            .map_err(|error| format!("failed to canonicalize {}: {error}", path.display()));
    }
    Ok(normalize_without_fs_access(&path))
}

#[cfg(feature = "tool-file")]
fn normalize_without_fs_access(path: &Path) -> PathBuf {
    let mut parts = Vec::new();
    for component in path.components() {
        use std::path::Component;
        match component {
            Component::ParentDir => {
                parts.pop();
            }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                parts.push(component.as_os_str().to_owned());
            }
        }
    }
    let mut normalized = PathBuf::new();
    for part in parts {
        normalized.push(part);
    }
    normalized
}

#[allow(dead_code)]
fn _shape_examples() -> BTreeMap<&'static str, Value> {
    BTreeMap::from([
        (
            "shell.exec",
            json!({
                "command": "echo",
                "args": ["hello"]
            }),
        ),
        (
            "file.read",
            json!({
                "path": "README.md",
                "max_bytes": 4096
            }),
        ),
        (
            "file.write",
            json!({
                "path": "notes.txt",
                "content": "hello",
                "create_dirs": true
            }),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_tool_keeps_backward_compatible_payload_shape() {
        let outcome = execute_tool_core(ToolCoreRequest {
            tool_name: "unknown".to_owned(),
            payload: json!({"hello":"world"}),
        })
        .expect("unknown tool should fallback to echo behavior");
        assert_eq!(outcome.status, "ok");
        assert_eq!(outcome.payload["adapter"], "core-tools");
    }
}
