# LoongClaw Session and Delegate Tool Surface Design

**Date:** 2026-03-12

**Status:** Approved design for `alpha-test`

**Scope:** Build the first thick app-layer session and delegation tool surface for `loongclaw-ai/loongclaw` on branch `alpha-test`, with emphasis on `delegate`, `sessions_list`, `sessions_history`, and `session_status`.

---

## 1. Product Goal

`loongclaw` already has a working MVP loop for:

- provider turns
- tool execution through the kernel
- SQLite-backed conversation history
- CLI, Telegram, and Feishu channels

What it does not have is a product-level session surface or a safe, inspectable delegation model. The current tool surface is too thin for multi-step agent workflows because everything is treated as a stateless core tool.

This design adds a thick app-layer capability surface that lets the agent:

- inspect sessions
- inspect session history
- inspect session status
- delegate a focused subtask into a child session and consume the result synchronously

The first milestone is intentionally narrow. It optimizes for depth, safety, and testability instead of broad tool count.

---

## 2. Current State

### 2.1 What Exists Today

The current app already has reliable primitives we can build on:

- session ids are already stable across channels and CLI entrypoints
- turns are persisted into SQLite through the conversation runtime
- the turn loop already supports tool-result follow-up rounds
- tool execution already routes through the kernel for policy and audit

Key files:

- `crates/app/src/chat.rs`
- `crates/app/src/channel/mod.rs`
- `crates/app/src/conversation/runtime.rs`
- `crates/app/src/conversation/turn_loop.rs`
- `crates/app/src/conversation/turn_engine.rs`
- `crates/app/src/memory/sqlite.rs`
- `crates/app/src/tools/mod.rs`

### 2.2 What Is Missing

The current tool architecture is insufficient for session-aware orchestration because:

- only `shell.exec`, `file.read`, and `file.write` are real agent-visible tools
- tool registration is global and static
- provider tool schemas are global and static
- capability snapshots are global and static
- there is no explicit session registry beyond transcript rows
- there is no app-layer tool dispatcher for stateful orchestration tools

This means the app cannot safely expose a child session with a reduced tool view, and it cannot model delegation as a first-class workflow.

---

## 3. Comparison Findings

The external comparison repos point in the same direction but with different levels of ambition:

- `openclaw/openclaw` has the richest session routing model and background session orchestration, but it depends on a much larger session-key, delivery, and visibility system.
- `zeroclaw-labs/zeroclaw` provides the most directly relevant reference for synchronous delegation: named sub-agents, filtered tool allowlists, and recursion guards.
- `qwibitai/nanoclaw` shows the value of strong isolation, but its container, IPC, and scheduler architecture is substantially heavier than `loongclaw` needs for this phase.
- `HKUDS/NanoBot` demonstrates the usefulness of `message`, `spawn`, and `cron`, but its session and subagent model is closer to bus-driven async workflows than to `loongclaw`'s current app loop.

The correct conclusion for `loongclaw` is not to copy the broadest design. It is to implement the strongest design that matches current app reality:

- synchronous nested delegation
- explicit child sessions
- dynamic per-session tool views
- lightweight but real session registry

---

## 4. Goals

### 4.1 In Scope

- introduce app-layer tools that are not just stateless core tool adapters
- add a session registry that can represent session metadata and parent-child relationships
- add dynamic tool views so different sessions can see different tool sets
- implement `sessions_list`
- implement `sessions_history`
- implement `session_status`
- implement synchronous `delegate`
- ensure child delegate sessions run with a smaller tool surface than the parent
- add focused tests around visibility, session state, and delegate behavior

### 4.2 Out of Scope

- background subagents
- `wait`, `cancel`, `resume`, or task queue management
- cross-channel `sessions_send`
- a general outbound `message` tool
- kernel-native task orchestration
- ACP-style or harness-native subagent execution
- broad cross-agent session visibility models

---

## 5. Design Principles

### 5.1 Thick App Layer First

This phase should land in `crates/app`, not in `crates/spec` stubs and not in kernel task abstractions. The app already has working session ids, transcript persistence, and turn orchestration. That is the correct place to make the feature real.

### 5.2 Dynamic Tool Exposure

Tool visibility must be a runtime decision, not a global static list. Child delegate sessions must not inherit the full root tool surface.

### 5.3 Transcript Cleanliness

Conversation transcript rows should contain user and assistant turns that are useful to the model. Session lifecycle and delegate control events should live outside transcript history.

### 5.4 Explicit Session State

A session should not only exist because it has turns. It should have explicit metadata:

- kind
- parent
- label
- state
- updated timestamp
- last error

### 5.5 Strong Defaults Over Broad Flexibility

This milestone should default to:

- one level of delegation
- parent plus direct children visibility
- no child `delegate`
- no child `sessions_*`
- no child outbound messaging

---

## 6. Chosen Architecture

### 6.1 Split Tool Surface Into Core and App Tools

Tools are split into two execution kinds:

- `Core`
  - stateless execution tools that can continue to route through the kernel core tool adapter
  - examples: `file.read`, `file.write`, `shell.exec`
- `App`
  - orchestration tools that need access to config, runtime, session metadata, or nested turn execution
  - examples: `sessions_list`, `sessions_history`, `session_status`, `delegate`

This avoids forcing stateful workflows into the existing stateless `ToolCoreRequest -> ToolCoreOutcome` path.

### 6.2 Add `ToolCatalog` and `ToolView`

The tool system becomes runtime-aware through two new concepts:

- `ToolCatalog`
  - global registry of known tools and their execution kind
- `ToolView`
  - the visible subset of tools for the current session

`ToolView` becomes the single source of truth for:

- `is_known_tool_name` decisions in the current session
- provider tool schema generation
- capability snapshot generation
- tool execution allow/deny at the turn engine boundary

### 6.3 Add an App Tool Dispatcher

`TurnEngine` should dispatch tools based on execution kind:

- `Core` tools continue to use the current kernel execution path
- `App` tools are executed by a new app-layer dispatcher with access to:
  - `LoongClawConfig`
  - conversation runtime
  - current session context
  - optional kernel context

This dispatcher is the orchestration boundary for session and delegate tools.

---

## 7. Session Model

### 7.1 Session Identity

Existing session ids remain unchanged for compatibility:

- CLI can continue using `default` or a caller-provided hint
- Telegram remains `telegram:<chat_id>`
- Feishu remains `feishu:<chat_id>`

New delegate child sessions use:

- `delegate:<uuid>`

No existing session ids are rewritten.

### 7.2 Session Metadata

Add a session metadata model with these fields:

- `session_id`
- `kind`
- `parent_session_id`
- `label`
- `state`
- `created_at`
- `updated_at`
- `last_error`

Recommended state values:

- `ready`
- `running`
- `completed`
- `failed`
- `timed_out`

### 7.3 SQLite Tables

Reuse the existing SQLite file, but add two lightweight tables:

`sessions`

- `session_id TEXT PRIMARY KEY`
- `kind TEXT NOT NULL`
- `parent_session_id TEXT NULL`
- `label TEXT NULL`
- `state TEXT NOT NULL`
- `created_at INTEGER NOT NULL`
- `updated_at INTEGER NOT NULL`
- `last_error TEXT NULL`

`session_events`

- `id INTEGER PRIMARY KEY AUTOINCREMENT`
- `session_id TEXT NOT NULL`
- `event_kind TEXT NOT NULL`
- `actor_session_id TEXT NULL`
- `payload_json TEXT NOT NULL`
- `ts INTEGER NOT NULL`

The existing `turns` table remains the transcript source.

### 7.4 Why Separate Registry and Transcript

The session registry solves problems that transcript rows should not solve:

- session listing
- status reporting
- parent-child visibility
- failure and timeout tracking
- delegate lifecycle auditing

This keeps provider message windows cleaner and avoids injecting control events into future prompts.

---

## 8. Delegate Design

### 8.1 Delegate Semantics

`delegate` is a synchronous nested conversation tool.

The parent session:

- creates a child session
- runs a focused child turn loop
- waits for completion
- receives a structured result as a tool outcome

The child session is not backgrounded and is not announced through channels.

### 8.2 Delegate Request Shape

First-phase delegate arguments:

- `task: string` required
- `label: string` optional
- `timeout_seconds: integer` optional

The tool is intentionally small. It does not accept arbitrary provider overrides, agent ids, or channel routing targets in this phase.

### 8.3 Delegate Execution Flow

1. Parent session invokes `delegate`
2. App tool dispatcher creates child session metadata
3. Session state is set to `running`
4. `session_events` records `delegate_started`
5. Child `ToolView` is constructed
6. Nested `ConversationTurnLoop` runs for the child session
7. Child session is marked `completed`, `failed`, or `timed_out`
8. `session_events` records completion or failure
9. Parent session receives structured tool output
10. Existing follow-up logic turns that output into a natural-language answer

### 8.4 Child Tool Surface

The first-phase child tool view should default to:

- `file.read`
- `file.write`
- `file.edit`
- `glob_search`
- `content_search`

The child tool view should explicitly exclude:

- `delegate`
- `sessions_list`
- `sessions_history`
- `session_status`
- `message`
- `schedule`

`shell.exec` should be disabled for children by default and controlled by config.

### 8.5 Delegate Result Shape

Delegate outcomes should be structured rather than stringly typed.

Success example:

```json
{
  "status": "ok",
  "payload": {
    "child_session_id": "delegate:8f3a1b2c",
    "label": "research-subtask",
    "final_output": "Summary text",
    "turn_count": 4,
    "duration_ms": 1822
  }
}
```

Timeout example:

```json
{
  "status": "timeout",
  "payload": {
    "child_session_id": "delegate:8f3a1b2c",
    "label": "research-subtask",
    "duration_ms": 60000,
    "error": "delegate_timeout"
  }
}
```

This gives the parent follow-up phase a stable, machine-readable contract.

---

## 9. Session Tool Design

### 9.1 `sessions_list`

Purpose:

- show visible sessions for the current session tree

Behavior:

- default visibility is current session plus direct children
- returns metadata rows, not full transcript history
- merges explicit `sessions` rows with best-effort legacy rows inferred from `turns`

First-phase row shape:

- `session_id`
- `kind`
- `parent_session_id`
- `label`
- `state`
- `created_at`
- `updated_at`
- `turn_count`
- `last_turn_at`
- `last_error`

### 9.2 `sessions_history`

Purpose:

- load transcript history for one visible session

Behavior:

- reads from `turns`
- returns only transcript rows, not lifecycle events
- enforces visibility based on the current session tree

### 9.3 `session_status`

Purpose:

- show structured status for one visible session

Behavior:

- reads from `sessions`
- may include recent event summaries from `session_events`
- should not require transcript scanning for normal cases

---

## 10. Visibility and Guardrails

### 10.1 Visibility Default

The default visibility model for this milestone is:

- current session
- direct child sessions created by the current session

This is strong enough for delegate inspection and weak enough to avoid accidental cross-session sprawl.

### 10.2 Delegate Guardrails

The first phase should hard-code or strongly default the following:

- `max_depth = 1`
- child sessions cannot call `delegate`
- child sessions cannot call `sessions_*`
- child sessions cannot send outbound messages
- child sessions time out explicitly
- parent sessions can inspect their direct child sessions

### 10.3 Failure Handling

Delegate child failures should not be persisted as synthetic assistant transcript turns.

Instead:

- update `sessions.state`
- write `last_error`
- append a `session_events` row
- return a structured tool outcome to the parent

This avoids poisoning future child prompts with operator-level error text.

---

## 11. Configuration

Extend `ToolConfig` so tool surface policy is explicit and centralized.

Recommended first-phase shape:

```toml
[tools]
shell_allowlist = ["echo", "cat", "ls", "pwd"]
file_root = "."

[tools.sessions]
enabled = true
visibility = "children"
list_limit = 100
history_limit = 200

[tools.delegate]
enabled = true
max_depth = 1
timeout_seconds = 60
child_tool_allowlist = ["file.read", "file.write", "file.edit", "glob_search", "content_search"]
allow_shell_in_child = false
```

The first phase should keep visibility values intentionally narrow:

- `self`
- `children`

No broader session-visibility matrix is needed yet.

---

## 12. Migration Strategy

### 12.1 No Destructive Migration

Do not rewrite or rename existing session ids.

Do not rewrite transcript history.

Do not require a separate migration command.

### 12.2 Incremental Schema Upgrade

On SQLite initialization:

- keep creating the existing `turns` table if missing
- add `sessions` if missing
- add `session_events` if missing

### 12.3 Legacy Session Fallback

For users with historical transcripts but no `sessions` rows:

- infer legacy session rows from `turns`
- derive `kind` from known prefixes when possible
- expose them through `sessions_list`

This preserves continuity after upgrade without rewriting history.

---

## 13. Testing Strategy

### 13.1 Tool Catalog and Tool View

Add tests that verify:

- root sessions see the full intended tool view
- delegate children do not see `delegate`
- delegate children do not see `sessions_*`
- provider tool schemas change with `ToolView`
- capability snapshots change with `ToolView`

### 13.2 Session Repository

Add tests that verify:

- session rows are created and updated correctly
- parent-child relationships are persisted
- state transitions are persisted
- events are stored outside transcript history

### 13.3 Delegate Integration

Add tests that verify:

- a parent tool call creates a child session
- child tool results return to the parent follow-up flow
- timeout produces a structured timeout result
- failed child execution updates session state and error metadata

### 13.4 Negative Controls

Add tests that verify:

- visibility restrictions block unrelated session inspection
- delegate depth limits are enforced
- child sessions cannot invoke forbidden tools
- transcript history excludes session control events

---

## 14. File Ownership

Primary files to modify:

- `crates/app/src/tools/mod.rs`
- `crates/app/src/conversation/turn_engine.rs`
- `crates/app/src/conversation/turn_loop.rs`
- `crates/app/src/conversation/runtime.rs`
- `crates/app/src/provider/mod.rs`
- `crates/app/src/config/tools_memory.rs`
- `crates/app/src/memory/sqlite.rs`

Recommended new files:

- `crates/app/src/tools/catalog.rs`
- `crates/app/src/tools/session.rs`
- `crates/app/src/tools/delegate.rs`
- `crates/app/src/session/repository.rs`

`session/repository.rs` is preferred over continuing to accumulate unrelated responsibilities in `memory/sqlite.rs`.

---

## 15. Rejected Alternatives

### 15.1 Background Spawn First

Rejected for this phase because it requires:

- task ownership
- wait/cancel/resume semantics
- announce delivery semantics
- persistent async orchestration

`loongclaw` does not yet have that product surface.

### 15.2 Kernel Task Supervisor First

Rejected for this phase because the kernel task and harness abstractions are still low-level infrastructure, not an app-facing subagent UX.

### 15.3 Global Static Tool Registry

Rejected because it cannot support child-session tool reduction and will expose tools to the model that the child should never see.

---

## 16. Delivery Sequence

Implementation should proceed in this order:

1. dynamic tool catalog and tool view
2. app tool dispatcher
3. session repository and schema
4. `sessions_list`, `sessions_history`, `session_status`
5. synchronous `delegate`
6. docs and config template updates

This ordering minimizes rework and keeps the new abstractions honest.
