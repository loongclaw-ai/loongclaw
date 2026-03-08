#[cfg(feature = "memory-sqlite")]
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use kernel::{MemoryCoreOutcome, MemoryCoreRequest};
#[cfg(feature = "memory-sqlite")]
use serde::{Deserialize, Serialize};
use serde_json::json;
#[cfg(feature = "memory-sqlite")]
use serde_json::Value;

#[cfg(feature = "memory-sqlite")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTurn {
    pub role: String,
    pub content: String,
    pub ts: i64,
}

pub fn execute_memory_core(request: MemoryCoreRequest) -> Result<MemoryCoreOutcome, String> {
    match request.operation.as_str() {
        "append_turn" => append_turn(request),
        "window" => load_window(request),
        "clear_session" => clear_session(request),
        _ => Ok(MemoryCoreOutcome {
            status: "ok".to_owned(),
            payload: json!({
                "adapter": "kv-core",
                "operation": request.operation,
                "payload": request.payload,
            }),
        }),
    }
}

#[cfg(feature = "memory-sqlite")]
fn append_turn(request: MemoryCoreRequest) -> Result<MemoryCoreOutcome, String> {
    let payload = request
        .payload
        .as_object()
        .ok_or_else(|| "memory.append_turn payload must be an object".to_owned())?;
    let session_id = payload
        .get("session_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "memory.append_turn requires payload.session_id".to_owned())?;
    let role = payload
        .get("role")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "memory.append_turn requires payload.role".to_owned())?;
    let content = payload
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| "memory.append_turn requires payload.content".to_owned())?;

    let path = memory_db_path();
    ensure_sqlite_schema(&path)?;
    let conn = rusqlite::Connection::open(&path)
        .map_err(|error| format!("open sqlite memory db failed: {error}"))?;
    let ts = unix_ts_now();
    conn.execute(
        "INSERT INTO turns(session_id, role, content, ts) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![session_id, role, content, ts],
    )
    .map_err(|error| format!("insert memory turn failed: {error}"))?;

    Ok(MemoryCoreOutcome {
        status: "ok".to_owned(),
        payload: json!({
            "adapter": "sqlite-core",
            "operation": "append_turn",
            "session_id": session_id,
            "role": role,
            "ts": ts,
            "db_path": path.display().to_string(),
        }),
    })
}

#[cfg(not(feature = "memory-sqlite"))]
fn append_turn(_request: MemoryCoreRequest) -> Result<MemoryCoreOutcome, String> {
    Err("sqlite memory is disabled in this build (enable feature `memory-sqlite`)".to_owned())
}

#[cfg(feature = "memory-sqlite")]
fn load_window(request: MemoryCoreRequest) -> Result<MemoryCoreOutcome, String> {
    let payload = request
        .payload
        .as_object()
        .ok_or_else(|| "memory.window payload must be an object".to_owned())?;
    let session_id = payload
        .get("session_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "memory.window requires payload.session_id".to_owned())?;
    let requested_limit = payload
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or_else(default_window_size_u64)
        .max(1)
        .min(128) as usize;
    let default_window = default_window_size().max(1);
    let window_limit = requested_limit.min(default_window);

    let path = memory_db_path();
    ensure_sqlite_schema(&path)?;
    let conn = rusqlite::Connection::open(&path)
        .map_err(|error| format!("open sqlite memory db failed: {error}"))?;

    let mut stmt = conn
        .prepare(
            "SELECT role, content, ts
             FROM turns
             WHERE session_id = ?1
             ORDER BY id DESC
             LIMIT ?2",
        )
        .map_err(|error| format!("prepare memory window query failed: {error}"))?;
    let rows = stmt
        .query_map(
            rusqlite::params![session_id, window_limit as i64],
            |row| -> rusqlite::Result<ConversationTurn> {
                Ok(ConversationTurn {
                    role: row.get(0)?,
                    content: row.get(1)?,
                    ts: row.get(2)?,
                })
            },
        )
        .map_err(|error| format!("query memory window failed: {error}"))?;

    let mut turns = Vec::new();
    for item in rows {
        turns.push(item.map_err(|error| format!("decode memory window row failed: {error}"))?);
    }
    turns.reverse();

    Ok(MemoryCoreOutcome {
        status: "ok".to_owned(),
        payload: json!({
            "adapter": "sqlite-core",
            "operation": "window",
            "session_id": session_id,
            "limit": window_limit,
            "turns": turns,
            "db_path": path.display().to_string(),
        }),
    })
}

#[cfg(not(feature = "memory-sqlite"))]
fn load_window(_request: MemoryCoreRequest) -> Result<MemoryCoreOutcome, String> {
    Err("sqlite memory is disabled in this build (enable feature `memory-sqlite`)".to_owned())
}

#[cfg(feature = "memory-sqlite")]
fn clear_session(request: MemoryCoreRequest) -> Result<MemoryCoreOutcome, String> {
    let payload = request
        .payload
        .as_object()
        .ok_or_else(|| "memory.clear_session payload must be an object".to_owned())?;
    let session_id = payload
        .get("session_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "memory.clear_session requires payload.session_id".to_owned())?;

    let path = memory_db_path();
    ensure_sqlite_schema(&path)?;
    let conn = rusqlite::Connection::open(&path)
        .map_err(|error| format!("open sqlite memory db failed: {error}"))?;
    let affected = conn
        .execute(
            "DELETE FROM turns WHERE session_id = ?1",
            rusqlite::params![session_id],
        )
        .map_err(|error| format!("clear memory session failed: {error}"))?;
    Ok(MemoryCoreOutcome {
        status: "ok".to_owned(),
        payload: json!({
            "adapter": "sqlite-core",
            "operation": "clear_session",
            "session_id": session_id,
            "deleted_rows": affected,
        }),
    })
}

#[cfg(not(feature = "memory-sqlite"))]
fn clear_session(_request: MemoryCoreRequest) -> Result<MemoryCoreOutcome, String> {
    Err("sqlite memory is disabled in this build (enable feature `memory-sqlite`)".to_owned())
}

#[cfg(feature = "memory-sqlite")]
pub fn append_turn_direct(session_id: &str, role: &str, content: &str) -> Result<(), String> {
    let request = MemoryCoreRequest {
        operation: "append_turn".to_owned(),
        payload: json!({
            "session_id": session_id,
            "role": role,
            "content": content,
        }),
    };
    execute_memory_core(request)?;
    Ok(())
}

#[cfg(feature = "memory-sqlite")]
pub fn window_direct(session_id: &str, limit: usize) -> Result<Vec<ConversationTurn>, String> {
    let request = MemoryCoreRequest {
        operation: "window".to_owned(),
        payload: json!({
            "session_id": session_id,
            "limit": limit,
        }),
    };
    let outcome = execute_memory_core(request)?;
    let turns_raw = outcome.payload["turns"].clone();
    serde_json::from_value(turns_raw)
        .map_err(|error| format!("decode memory turns failed: {error}"))
}

#[cfg(feature = "memory-sqlite")]
pub fn ensure_memory_db_ready(path: Option<PathBuf>) -> Result<PathBuf, String> {
    let effective = path.unwrap_or_else(memory_db_path);
    ensure_sqlite_schema(&effective)?;
    Ok(effective)
}

#[cfg(feature = "memory-sqlite")]
fn default_window_size() -> usize {
    std::env::var("LOONGCLAW_SLIDING_WINDOW")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(12)
}

#[cfg(feature = "memory-sqlite")]
fn default_window_size_u64() -> u64 {
    default_window_size() as u64
}

#[cfg(feature = "memory-sqlite")]
fn unix_ts_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(feature = "memory-sqlite")]
fn memory_db_path() -> PathBuf {
    std::env::var("LOONGCLAW_SQLITE_PATH")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".loongclaw")
                .join("memory.sqlite3")
        })
}

#[cfg(feature = "memory-sqlite")]
fn ensure_sqlite_schema(path: &PathBuf) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("create sqlite parent directory failed: {error}"))?;
        }
    }

    let conn = rusqlite::Connection::open(path)
        .map_err(|error| format!("open sqlite memory db failed: {error}"))?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS turns(
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          session_id TEXT NOT NULL,
          role TEXT NOT NULL,
          content TEXT NOT NULL,
          ts INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_turns_session_id ON turns(session_id, id);
        ",
    )
    .map_err(|error| format!("initialize sqlite memory schema failed: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_memory_operation_stays_compatible() {
        let outcome = execute_memory_core(MemoryCoreRequest {
            operation: "noop".to_owned(),
            payload: json!({"a":1}),
        })
        .expect("fallback operation should succeed");
        assert_eq!(outcome.status, "ok");
        assert_eq!(outcome.payload["adapter"], "kv-core");
    }
}
