# TUI Architecture Redesign Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 5 architectural issues in the TUI module to close the verification loop.

**Architecture:** Gated parallel batches. Phase 1 (PTY upgrade) unblocks verification. Phases 2a (streaming) and 2b (modal focus) run in parallel with disjoint file ownership. Phase 3 (event/render decoupling) restructures the event loop after 2a+2b land.

**Parallel agent safety note:** Tasks 2 and 3 both edit `state.rs` and `shell.rs`
but at disjoint struct/line ranges. If running as parallel subagents, each agent
must use content-based (not line-number-based) matching for edits to avoid
misalignment when the other agent's changes shift line numbers. Alternatively,
run Task 2 first, commit, then run Task 3 on the updated code.

**Tech Stack:** Rust, ratatui, crossterm, vt100, tokio, futures-util, tui-textarea

**Spec:** `docs/superpowers/specs/2026-03-29-tui-architecture-redesign.md`

**Worktree:** `/Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui/`

**Branch:** `feat/issue-689-balanced-chat-tui`

---

## Chunk 1: Phase 1 — PTY Frame Capture Upgrade

### Task 1: Replace byte accumulation with vt100 virtual terminal

**Files:**
- Modify: `crates/daemon/Cargo.toml:87-93` (add dev-dep)
- Modify: `crates/daemon/tests/integration/tui_pty.rs:55-297` (fixture rewrite)

- [ ] **Step 1: Add vt100 dev-dependency**

In `crates/daemon/Cargo.toml`, add `vt100` to `[dev-dependencies]`:

```toml
[dev-dependencies]
axum.workspace = true
loongclaw-spec = { path = "../spec", features = ["test-hooks"] }
portable-pty = "0.9"
sha2.workspace = true
strip-ansi-escapes = "0.2"
vt100 = "0.15"
wat = "1"
```

Run: `cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui && cargo check -p loongclaw-daemon`
Expected: compiles with new dep

- [ ] **Step 2: Replace `accumulated: Vec<u8>` with `parser: vt100::Parser`**

In `tui_pty.rs`, change the struct definition (lines 55-63):

```rust
struct TuiPtyFixture {
    child: Box<dyn portable_pty::Child + Send>,
    writer: Box<dyn Write + Send>,
    /// Receives byte chunks from the background reader thread.
    rx: std_mpsc::Receiver<Vec<u8>>,
    /// Virtual terminal that processes escape sequences and tracks the
    /// current screen buffer (80x24, no scrollback).
    parser: vt100::Parser,
    _root: PathBuf,
}
```

Update the constructor at line 143-149:

```rust
Self {
    child,
    writer,
    rx,
    parser: vt100::Parser::new(24, 80, 0),
    _root: root,
}
```

- [ ] **Step 3: Rewrite `drain_pending()` to feed parser**

Replace lines 184-188:

```rust
fn drain_pending(&mut self) {
    while let Ok(chunk) = self.rx.try_recv() {
        self.parser.process(&chunk);
    }
}
```

- [ ] **Step 4: Rewrite `read_screen()` to return current frame**

Replace lines 193-218:

```rust
/// Returns the current visible screen contents (not historical output).
/// Waits up to `timeout` for data to arrive, then returns whatever is
/// currently on screen.
fn read_screen(&mut self, timeout: Duration) -> Result<String, String> {
    let deadline = Instant::now() + timeout;

    loop {
        self.drain_pending();

        let contents = self.parser.screen().contents();
        if !contents.trim().is_empty() {
            // Give a short grace period for more data to arrive.
            std::thread::sleep(Duration::from_millis(100));
            self.drain_pending();
            return Ok(self.parser.screen().contents());
        }

        if Instant::now() >= deadline {
            return Ok(self.parser.screen().contents());
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        let wait = remaining.min(Duration::from_millis(100));
        if let Ok(chunk) = self.rx.recv_timeout(wait) {
            self.parser.process(&chunk);
        }
    }
}
```

- [ ] **Step 5: Rewrite `wait_for()` to match against current frame**

Replace lines 223-257:

```rust
/// Wait until the current screen frame contains `pattern`.
/// Polls every 100ms until `timeout`.
fn wait_for(&mut self, pattern: &str, timeout: Duration) -> Result<String, String> {
    let deadline = Instant::now() + timeout;

    loop {
        self.drain_pending();

        let screen = self.parser.screen().contents();
        if screen.contains(pattern) {
            return Ok(screen);
        }

        if Instant::now() >= deadline {
            return Err(format!(
                "timed out waiting for pattern {:?} in PTY screen (got: {:?})",
                pattern, screen
            ));
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        let wait = remaining.min(Duration::from_millis(100));
        match self.rx.recv_timeout(wait) {
            Ok(chunk) => {
                self.parser.process(&chunk);
            }
            Err(std_mpsc::RecvTimeoutError::Timeout) => {}
            Err(std_mpsc::RecvTimeoutError::Disconnected) => {
                let screen = self.parser.screen().contents();
                return Err(format!(
                    "PTY reader disconnected before pattern {:?} appeared (got: {:?})",
                    pattern, screen,
                ));
            }
        }
    }
}
```

- [ ] **Step 6: Rewrite `wait_for_any()` to match against current frame**

Replace lines 263-297:

```rust
/// Wait until ANY of the given patterns matches in the current screen
/// frame. Polls every 100ms until `timeout`.
fn wait_for_any(&mut self, patterns: &[&str], timeout: Duration) -> Result<String, String> {
    let deadline = Instant::now() + timeout;

    loop {
        self.drain_pending();

        let screen = self.parser.screen().contents();
        for pat in patterns {
            if screen.contains(pat) {
                return Ok(screen);
            }
        }

        if Instant::now() >= deadline {
            return Err(format!(
                "timed out waiting for any of {patterns:?} in PTY screen (got: {screen:?})"
            ));
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        let wait = remaining.min(Duration::from_millis(100));
        match self.rx.recv_timeout(wait) {
            Ok(chunk) => {
                self.parser.process(&chunk);
            }
            Err(std_mpsc::RecvTimeoutError::Timeout) => {}
            Err(std_mpsc::RecvTimeoutError::Disconnected) => {
                let screen = self.parser.screen().contents();
                return Err(format!(
                    "PTY reader disconnected before any of {patterns:?} appeared (got: {screen:?})"
                ));
            }
        }
    }
}
```

- [ ] **Step 7: Update any remaining references to `accumulated`**

Search for `self.accumulated` in `tui_pty.rs`. If any callers reference
it directly (outside drain/read/wait functions), update them to use
`self.parser.screen().contents()`.

Also check `contains_collapsed()` helper (line 42) — it operates on
strings not byte slices, so it still works unchanged.

- [ ] **Step 8: Run all PTY tests and adapt any failures**

Run: `cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui && cargo test -p loongclaw-daemon --all-features -- tui_pty`

**Expected:** Most tests pass, but some may fail because the new frame-only
semantics differ from the old accumulated-history semantics. Specifically:
- Tests that check for strings that appeared earlier but have since scrolled
  off the 24-line viewport will now fail.
- Tests that check `read_screen()` immediately after startup may get an
  empty/partial frame if the TUI hasn't fully rendered yet.

**For failing tests:** Increase the wait timeout, use `wait_for()` instead
of `read_screen()` to poll for specific content, or update the expected
pattern to match what's currently visible (not historical). The `wait_for()`
polling loop naturally retries until the pattern appears on screen.

- [ ] **Step 9: Run full workspace checks**

Run: `cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui && cargo fmt --all -- --check && cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: clean

- [ ] **Step 10: Commit**

```bash
cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui
git add crates/daemon/Cargo.toml crates/daemon/tests/integration/tui_pty.rs
git commit -m "fix(tui): replace PTY byte accumulation with vt100 frame capture

Switches TuiPtyFixture from accumulating all raw bytes to using a
vt100::Parser that exposes the current screen buffer. Tests now
verify what the user actually sees, not historical output."
```

---

## Chunk 2: Phase 2a — Streaming Text Unification

### Task 2: Eliminate streaming_text buffer and dual-render

**Files:**
- Modify: `crates/app/src/chat/tui/state.rs:24-25,58-83,87-100,133-138` (remove buffer, rewrite append)
- Modify: `crates/app/src/chat/tui/history.rs:16-21,46-59` (PaneView trait, render_history)
- Modify: `crates/app/src/chat/tui/shell.rs:39-51` (PaneView impl)

- [ ] **Step 1: Write failing test — in-place token accumulation**

Add test to `crates/app/src/chat/tui/state.rs` in the `#[cfg(test)] mod tests`:

```rust
#[test]
fn append_token_accumulates_in_message_part() {
    let mut pane = Pane::new("sess-1");
    pane.append_token("hello ", false);
    pane.append_token("world", false);
    // Tokens should be in the last MessagePart directly, no separate buffer
    assert_eq!(pane.messages.len(), 1);
    let parts = &pane.messages[0].parts;
    assert_eq!(parts.len(), 1);
    match &parts[0] {
        MessagePart::Text(text) => assert_eq!(text, "hello world"),
        other => panic!("expected Text, got {:?}", other),
    }
}

#[test]
fn thinking_toggle_creates_separate_parts() {
    let mut pane = Pane::new("sess-1");
    pane.append_token("thought", true);
    pane.append_token("visible", false);
    assert_eq!(pane.messages.len(), 1);
    let parts = &pane.messages[0].parts;
    assert_eq!(parts.len(), 2);
    match &parts[0] {
        MessagePart::ThinkBlock(text) => assert_eq!(text, "thought"),
        other => panic!("expected ThinkBlock, got {:?}", other),
    }
    match &parts[1] {
        MessagePart::Text(text) => assert_eq!(text, "visible"),
        other => panic!("expected Text, got {:?}", other),
    }
}
```

Run: `cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui && cargo test -p loongclaw-app --all-features -- state::tests::append_token_accumulates`
Expected: FAIL (current impl uses streaming_text buffer, not message parts)

- [ ] **Step 2: Rewrite state.rs — remove buffer, add streaming_active**

In `crates/app/src/chat/tui/state.rs`, make these changes:

Replace the struct fields (lines 24-25):
```rust
    // OLD:
    // pub(super) streaming_text: String,
    // pub(super) is_thinking: bool,
    // NEW:
    pub(super) streaming_active: bool,
```

Update `Pane::new()` initializer (lines 46-47):
```rust
    // OLD:
    // streaming_text: String::new(),
    // is_thinking: false,
    // NEW:
    streaming_active: false,
```

Replace `append_token()` (lines 58-64):
```rust
/// Accumulates tokens directly into the last MessagePart.
/// When `is_thinking` changes, pushes a new part to keep
/// ThinkBlock and Text separate.
pub(super) fn append_token(&mut self, content: &str, is_thinking: bool) {
    self.streaming_active = true;
    self.ensure_assistant_message();
    let msg = match self.messages.last_mut() {
        Some(m) => m,
        None => return,
    };

    // Extend existing part if same type, otherwise push new
    let extend_existing = match msg.parts.last() {
        Some(MessagePart::ThinkBlock(_)) if is_thinking => true,
        Some(MessagePart::Text(_)) if !is_thinking => true,
        _ => false,
    };

    if extend_existing {
        match msg.parts.last_mut() {
            Some(MessagePart::ThinkBlock(ref mut text))
            | Some(MessagePart::Text(ref mut text)) => {
                text.push_str(content);
            }
            _ => {}
        }
    } else {
        let part = if is_thinking {
            MessagePart::ThinkBlock(content.to_string())
        } else {
            MessagePart::Text(content.to_string())
        };
        msg.parts.push(part);
    }
}
```

Remove `flush_streaming()` entirely (lines 69-83).

Update `start_tool_call()` — remove the `flush_streaming()` call (line 88):
```rust
pub(super) fn start_tool_call(&mut self, tool_id: &str, tool_name: &str, args_preview: &str) {
    // flush_streaming() removed — tokens already in MessagePart
    self.ensure_assistant_message();
    // ... rest unchanged
```

Update `finalize_response()` (lines 133-138):
```rust
pub(super) fn finalize_response(&mut self, input_tokens: u32, output_tokens: u32) {
    self.streaming_active = false;
    self.input_tokens = self.input_tokens.saturating_add(input_tokens);
    self.output_tokens = self.output_tokens.saturating_add(output_tokens);
    self.agent_running = false;
}
```

- [ ] **Step 3: Update PaneView trait in history.rs**

Replace the trait definition (lines 16-21):
```rust
pub(super) trait PaneView {
    fn messages(&self) -> &[Message];
    fn scroll_offset(&self) -> u16;
    fn streaming_active(&self) -> bool;
}
```

Replace the streaming section in `render_history()` (lines 46-59):
```rust
    // Show cursor indicator on the last part of the current assistant message
    // when streaming is active.
    if pane.streaming_active() {
        if let Some(last_msg) = pane.messages().last() {
            if last_msg.role == Role::Assistant {
                // Append a blinking cursor to the last line
                if let Some(last_line) = lines.last_mut() {
                    last_line.spans.push(Span::styled(
                        "\u{2588}",
                        Style::default()
                            .fg(palette.accent)
                            .add_modifier(Modifier::SLOW_BLINK),
                    ));
                }
            }
        }
    }
```

- [ ] **Step 4: Update PaneView impl in shell.rs**

Replace the impl (lines 39-52):
```rust
impl PaneView for state::Pane {
    fn messages(&self) -> &[Message] {
        &self.messages
    }
    fn scroll_offset(&self) -> u16 {
        self.scroll_offset
    }
    fn streaming_active(&self) -> bool {
        self.streaming_active
    }
}
```

- [ ] **Step 4b: Update TestPane in history.rs test module**

The `history.rs` test module (lines ~356-387) has a `TestPane` struct that
implements `PaneView` with `streaming_text: String` and `is_thinking: bool`
fields. Update it:

Remove `streaming_text` and `is_thinking` fields from `TestPane`. Add
`streaming_active: bool`. Update the `PaneView` impl:
```rust
impl PaneView for TestPane {
    fn messages(&self) -> &[Message] {
        &self.messages
    }
    fn scroll_offset(&self) -> u16 {
        self.scroll_offset
    }
    fn streaming_active(&self) -> bool {
        self.streaming_active
    }
}
```

Update any `TestPane` constructors to initialize `streaming_active: false`
instead of `streaming_text: String::new()` and `is_thinking: false`.

- [ ] **Step 5: Update existing tests in state.rs**

Replace `append_and_flush_streaming` test (lines 298-309):
```rust
#[test]
fn append_and_flush_streaming() {
    let mut pane = Pane::new("sess-1");
    pane.append_token("hello ", false);
    pane.append_token("world", false);
    assert!(pane.streaming_active);
    assert_eq!(pane.messages.len(), 1);
    assert_eq!(pane.messages[0].parts.len(), 1);
    match &pane.messages[0].parts[0] {
        MessagePart::Text(text) => assert_eq!(text, "hello world"),
        other => panic!("expected Text, got {:?}", other),
    }
}
```

Replace `thinking_toggle_flushes` test (lines 311-320):
```rust
#[test]
fn thinking_toggle_creates_separate_parts() {
    let mut pane = Pane::new("sess-1");
    pane.append_token("thought", true);
    pane.append_token("visible", false);
    assert_eq!(pane.messages.len(), 1);
    let parts = &pane.messages[0].parts;
    assert_eq!(parts.len(), 2);
    assert!(matches!(&parts[0], MessagePart::ThinkBlock(t) if t == "thought"));
    assert!(matches!(&parts[1], MessagePart::Text(t) if t == "visible"));
}
```

- [ ] **Step 6: Run tests**

Run: `cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui && cargo test -p loongclaw-app --all-features`
Expected: all tests pass

- [ ] **Step 7: Run workspace checks**

Run: `cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui && cargo fmt --all -- --check && cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: clean

- [ ] **Step 8: Commit**

```bash
cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui
git add crates/app/src/chat/tui/state.rs crates/app/src/chat/tui/history.rs crates/app/src/chat/tui/shell.rs
git commit -m "fix(tui): unify streaming into message parts, eliminate dual-render

Tokens accumulate directly into the last MessagePart instead of a
separate streaming_text buffer. Removes flush_streaming() and the
one-frame duplication window that caused visible flash during flushes."
```

---

## Chunk 3: Phase 2b — Modal Focus System

### Task 3: Replace ad-hoc modal if-chains with FocusStack

**Files:**
- Rewrite: `crates/app/src/chat/tui/focus.rs` (14 → ~60 lines)
- Modify: `crates/app/src/chat/tui/state.rs:194-209,219-246` (Shell + UiState)
- Modify: `crates/app/src/chat/tui/render.rs:24-31,72-78,406-439` (ShellView trait + overlays + TestShell)
- Modify: `crates/app/src/chat/tui/shell.rs:99-114,301-334` (ShellView impl + input routing)
- Modify: `crates/app/src/chat/tui/app_shell.rs` (bootstrap return type)
- Modify: `crates/app/src/chat.rs:~4178-4217` (two focus_target test assertions)

- [ ] **Step 1: Write failing test — FocusStack behavior**

Add to a new test block at the bottom of `crates/app/src/chat/tui/focus.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_defaults_to_composer() {
        let stack = FocusStack::new();
        assert_eq!(stack.top(), FocusLayer::Composer);
    }

    #[test]
    fn push_and_pop() {
        let mut stack = FocusStack::new();
        stack.push(FocusLayer::Help);
        assert_eq!(stack.top(), FocusLayer::Help);
        assert!(stack.has(FocusLayer::Help));

        stack.pop();
        assert_eq!(stack.top(), FocusLayer::Composer);
        assert!(!stack.has(FocusLayer::Help));
    }

    #[test]
    fn pop_never_removes_composer() {
        let mut stack = FocusStack::new();
        stack.pop();
        assert_eq!(stack.top(), FocusLayer::Composer);
    }

    #[test]
    fn stacking_order() {
        let mut stack = FocusStack::new();
        stack.push(FocusLayer::Help);
        stack.push(FocusLayer::ClarifyDialog);
        assert_eq!(stack.top(), FocusLayer::ClarifyDialog);
        stack.pop();
        assert_eq!(stack.top(), FocusLayer::Help);
    }
}
```

Run: `cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui && cargo test -p loongclaw-app --all-features -- focus::tests`
Expected: FAIL (FocusStack doesn't exist yet)

- [ ] **Step 2: Implement FocusStack in focus.rs**

Replace the entire `crates/app/src/chat/tui/focus.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FocusLayer {
    Composer,
    Help,
    ClarifyDialog,
}

/// A stack of UI focus layers. `Composer` is always the base and cannot
/// be popped. Push a layer to capture input; pop to return to the
/// previous layer.
#[derive(Debug, Clone)]
pub(super) struct FocusStack {
    layers: Vec<FocusLayer>,
}

impl FocusStack {
    pub(super) fn new() -> Self {
        Self {
            layers: vec![FocusLayer::Composer],
        }
    }

    pub(super) fn top(&self) -> FocusLayer {
        self.layers.last().copied().unwrap_or(FocusLayer::Composer)
    }

    pub(super) fn push(&mut self, layer: FocusLayer) {
        self.layers.push(layer);
    }

    pub(super) fn pop(&mut self) {
        // Never pop below Composer
        if self.layers.len() > 1 {
            self.layers.pop();
        }
    }

    pub(super) fn has(&self, layer: FocusLayer) -> bool {
        self.layers.contains(&layer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_defaults_to_composer() {
        let stack = FocusStack::new();
        assert_eq!(stack.top(), FocusLayer::Composer);
    }

    #[test]
    fn push_and_pop() {
        let mut stack = FocusStack::new();
        stack.push(FocusLayer::Help);
        assert_eq!(stack.top(), FocusLayer::Help);
        assert!(stack.has(FocusLayer::Help));
        stack.pop();
        assert_eq!(stack.top(), FocusLayer::Composer);
        assert!(!stack.has(FocusLayer::Help));
    }

    #[test]
    fn pop_never_removes_composer() {
        let mut stack = FocusStack::new();
        stack.pop();
        assert_eq!(stack.top(), FocusLayer::Composer);
    }

    #[test]
    fn stacking_order() {
        let mut stack = FocusStack::new();
        stack.push(FocusLayer::Help);
        stack.push(FocusLayer::ClarifyDialog);
        assert_eq!(stack.top(), FocusLayer::ClarifyDialog);
        stack.pop();
        assert_eq!(stack.top(), FocusLayer::Help);
    }
}
```

- [ ] **Step 3: Run FocusStack tests**

Run: `cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui && cargo test -p loongclaw-app --all-features -- focus::tests`
Expected: PASS

- [ ] **Step 4: Update Shell struct in state.rs**

Replace Shell struct (lines 194-209):
```rust
#[derive(Debug, Clone)]
pub(super) struct Shell {
    pub(super) pane: Pane,
    pub(super) running: bool,
    pub(super) show_thinking: bool,
    pub(super) focus: FocusStack,
}

impl Shell {
    pub(super) fn new(session_id: &str) -> Self {
        Self {
            pane: Pane::new(session_id),
            running: true,
            show_thinking: true,
            focus: FocusStack::new(),
        }
    }
}
```

Add import at top of state.rs: `use super::focus::FocusStack;`

Remove the `FocusTarget` import: `use super::focus::FocusTarget;` → delete.

- [ ] **Step 5: Update UiState in state.rs**

Replace `UiState` struct and impls (lines 219-246):
```rust
/// Top-level TUI state combining pane state with focus.
/// Used as the single source of truth for the render loop.
#[derive(Debug, Clone)]
pub(crate) struct UiState {
    pub(crate) session_id: String,
    pub(super) pane: Pane,
    pub(crate) focus: FocusStack,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            pane: Pane::new("default"),
            focus: FocusStack::new(),
        }
    }
}

impl UiState {
    pub(crate) fn with_session_id(session_id: impl Into<String>) -> Self {
        let id: String = session_id.into();
        Self {
            session_id: id.clone(),
            pane: Pane::new(&id),
            ..Self::default()
        }
    }
}
```

Make `FocusStack` pub(crate) in focus.rs (change `pub(super)` to `pub(crate)` on struct and `FocusLayer`).

- [ ] **Step 6: Update ShellView trait in render.rs**

Replace the ShellView trait (lines 24-31):
```rust
pub(super) trait ShellView {
    type Pane: PaneView + SpinnerView + StatusBarView + InputView;

    fn pane(&self) -> &Self::Pane;
    fn show_thinking(&self) -> bool;
    fn focus(&self) -> &FocusStack;
    fn clarify_dialog(&self) -> Option<&ClarifyDialog>;
}
```

Add import at top of render.rs: `use super::focus::{FocusLayer, FocusStack};`

Update overlay dispatch in `draw()` (lines 71-78):
```rust
    // 7. Overlays — render each focus layer above Composer
    if let Some(dialog) = state.clarify_dialog() {
        if state.focus().has(FocusLayer::ClarifyDialog) {
            render_clarify_dialog(dialog, frame, area, palette);
        }
    }

    if state.focus().has(FocusLayer::Help) {
        render_help_overlay(frame, area, palette);
    }
```

- [ ] **Step 7: Update ShellView impl in shell.rs**

Replace the impl (lines 99-114):
```rust
impl ShellView for state::Shell {
    type Pane = state::Pane;

    fn pane(&self) -> &state::Pane {
        &self.pane
    }
    fn show_thinking(&self) -> bool {
        self.show_thinking
    }
    fn focus(&self) -> &FocusStack {
        &self.focus
    }
    fn clarify_dialog(&self) -> Option<&ClarifyDialog> {
        self.pane.clarify_dialog.as_ref()
    }
}
```

Add import: `use super::focus::{FocusLayer, FocusStack};`

- [ ] **Step 8: Replace input routing if-chain in shell.rs**

Replace the dialog/help/composer if-chain (lines 301-334):
```rust
    // --- Focus-based input routing -----------------------------------
    match shell.focus.top() {
        FocusLayer::ClarifyDialog => {
            if let Some(ref mut dialog) = shell.pane.clarify_dialog {
                #[allow(clippy::wildcard_enum_match_arm)]
                match key.code {
                    KeyCode::Enter => {
                        let response = dialog.response();
                        shell.pane.clarify_dialog = None;
                        shell.focus.pop();
                        let _ = tx.send(UiEvent::Token {
                            content: format!("\n[user chose: {response}]\n"),
                            is_thinking: false,
                        });
                    }
                    KeyCode::Esc => {
                        shell.pane.clarify_dialog = None;
                        shell.focus.pop();
                    }
                    KeyCode::Up => dialog.select_up(),
                    KeyCode::Down => dialog.select_down(),
                    KeyCode::Left => dialog.move_cursor_left(),
                    KeyCode::Right => dialog.move_cursor_right(),
                    KeyCode::Backspace => dialog.delete_back(),
                    KeyCode::Char(ch) => dialog.insert_char(ch),
                    _ => {}
                }
            }
            return;
        }
        FocusLayer::Help => {
            if key.code == KeyCode::Esc || key.code == KeyCode::Char('q') {
                shell.focus.pop();
            }
            // Swallow all other keys while help is open.
            return;
        }
        FocusLayer::Composer => {
            // Fall through to global shortcuts + textarea below
        }
    }
```

- [ ] **Step 9: Update /help command and clarify_dialog set points**

Find where `shell.show_help = !shell.show_help` is set (toggle in the slash
command handler, line ~417). The current code is a boolean toggle. Replace with
a stack-aware toggle:
```rust
if shell.focus.has(FocusLayer::Help) {
    shell.focus.pop();
} else {
    shell.focus.push(FocusLayer::Help);
}
```

Find where `shell.pane.clarify_dialog = Some(...)` is set (in `apply_ui_event`)
and add `shell.focus.push(FocusLayer::ClarifyDialog)` after it.

Search `shell.rs` for any remaining `shell.show_help` references — they must
all be replaced with `shell.focus.has(FocusLayer::Help)` or the push/pop
pattern above. Compile errors will surface any missed references.

- [ ] **Step 10: Update TestShell in render.rs tests**

Replace TestShell struct and impl (lines 406-439):
```rust
    struct TestShell {
        pane: TestPane,
        show_thinking: bool,
        focus: FocusStack,
        clarify_dialog: Option<ClarifyDialog>,
    }

    impl TestShell {
        fn idle() -> Self {
            Self {
                pane: TestPane::default_idle(),
                show_thinking: false,
                focus: FocusStack::new(),
                clarify_dialog: None,
            }
        }
    }

    impl ShellView for TestShell {
        type Pane = TestPane;

        fn pane(&self) -> &TestPane {
            &self.pane
        }
        fn show_thinking(&self) -> bool {
            self.show_thinking
        }
        fn focus(&self) -> &FocusStack {
            &self.focus
        }
        fn clarify_dialog(&self) -> Option<&ClarifyDialog> {
            self.clarify_dialog.as_ref()
        }
    }
```

- [ ] **Step 11: Update chat.rs tests**

Find the two tests in `crates/app/src/chat.rs` that assert on `focus_target`:

Test at ~line 4182: replace `assert_eq!(bootstrap.focus_target, tui::focus::FocusTarget::Composer)` with:
```rust
assert_eq!(bootstrap.focus.top(), tui::focus::FocusLayer::Composer);
```

Test at ~line 4183-4184: remove `assert!(bootstrap.drawer.is_none())` — the
`drawer` field was removed from `UiState`.

Test at ~line 4213: remove `assert!(state.drawer.is_none())` — same reason.

Test at ~line 4216: replace `assert_eq!(state.focus_target, tui::focus::FocusTarget::Composer)` with:
```rust
assert_eq!(state.focus.top(), tui::focus::FocusLayer::Composer);
```

Update imports in chat.rs: replace `tui::focus::FocusTarget` with
`tui::focus::FocusLayer`.

- [ ] **Step 12: Update shell_defaults test in state.rs**

Replace the `shell_defaults` test (lines 363-368):
```rust
#[test]
fn shell_defaults() {
    let shell = Shell::new("s1");
    assert!(shell.running);
    assert!(shell.show_thinking);
    assert_eq!(shell.focus.top(), super::super::focus::FocusLayer::Composer);
    assert_eq!(shell.pane.session_id, "s1");
}
```

- [ ] **Step 13: Run all tests**

Run: `cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui && cargo test -p loongclaw-app --all-features`
Expected: all pass

- [ ] **Step 14: Run workspace checks**

Run: `cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui && cargo fmt --all -- --check && cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: clean

- [ ] **Step 15: Commit**

```bash
cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui
git add crates/app/src/chat/tui/focus.rs crates/app/src/chat/tui/state.rs \
        crates/app/src/chat/tui/render.rs crates/app/src/chat/tui/shell.rs \
        crates/app/src/chat/tui/app_shell.rs crates/app/src/chat.rs
git commit -m "fix(tui): add FocusStack for proper modal input isolation

Replaces ad-hoc if-chains with a FocusStack that tracks active UI
layers. Input routing and overlay rendering both read from the stack.
Adding new modals now requires one enum variant, not scattered if-blocks."
```

---

## Chunk 4: Phase 3 — Event/Render Decoupling

### Task 4: Restructure event loop with dirty flag and tick-gated rendering

**Files:**
- Modify: `crates/app/src/chat/tui/state.rs` (add dirty flag)
- Modify: `crates/app/src/chat/tui/shell.rs:480-521` (event loop restructure)
- Note: `futures-util` is already a workspace dep (provides `FutureExt::now_or_never`)

**Dependency:** Must run AFTER Tasks 2 and 3 are merged.

- [ ] **Step 1: Verify futures-util is available**

Run: `cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui && grep 'futures-util' crates/app/Cargo.toml`

Expected: `futures-util.workspace = true` already present. We need
`futures_util::FutureExt` for `now_or_never()` and `futures_util::StreamExt`
for `now_or_never()` on streams. Both are in `futures-util` — do NOT add
a separate `futures` crate.

- [ ] **Step 2: Add dirty flag to Shell**

In `crates/app/src/chat/tui/state.rs`, add to Shell struct:
```rust
pub(super) dirty: bool,
```

Initialize as `true` in `Shell::new()`:
```rust
dirty: true,
```

Add a helper method to Pane that marks the parent dirty. Since Pane doesn't
own the dirty flag (Shell does), the caller in shell.rs must set
`shell.dirty = true` after every `apply_ui_event` and `apply_terminal_event`.

- [ ] **Step 3: Write failing test — dirty flag semantics**

Add test to state.rs:
```rust
#[test]
fn shell_dirty_on_creation() {
    let shell = Shell::new("s1");
    assert!(shell.dirty, "shell should start dirty for first render");
}
```

Run: `cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui && cargo test -p loongclaw-app --all-features -- state::tests::shell_dirty`
Expected: PASS (since we just added the field)

- [ ] **Step 4: Restructure the event loop in shell.rs**

Replace the event loop (lines 480-521) with the drain/render/sleep pattern:

```rust
    loop {
        // ── Phase 1: Drain all pending events (non-blocking) ──────────
        // Observer channel
        while let Ok(event) = rx.try_recv() {
            apply_ui_event(&mut shell, event);
            shell.dirty = true;
        }

        // Crossterm terminal events
        {
            use futures_util::StreamExt as _;
            while let Some(maybe_event) = crossterm_events.next().now_or_never().flatten() {
                if let Ok(event) = maybe_event {
                    let mut submit_text: Option<String> = None;
                    apply_terminal_event(
                        &mut shell,
                        &mut textarea,
                        event,
                        &tx,
                        &mut submit_text,
                    );
                    shell.dirty = true;

                    // Submit turn if requested
                    if let Some(text) = submit_text.take() {
                        // ... turn submission logic (same as current)
                    }
                }
            }
        }

        // Check turn completion (non-blocking)
        if turn_active {
            use futures_util::FutureExt as _;
            if turn_future.now_or_never().is_some() {
                turn_active = false;
                turn_future = Box::pin(std::future::pending());
                shell.pane.agent_running = false;
                shell.dirty = true;
            }
        }

        // ── Phase 2: Render (only when dirty) ──────────────────────
        // The tick in the sleep phase sets dirty=true, so we don't need
        // a separate tick-elapsed check here. Just render when dirty.
        if shell.dirty {
            shell.pane.tick_spinner();
            guard.draw(&shell, &textarea, &palette)?;
            shell.dirty = false;
        }

        if !shell.running {
            break;
        }

        // ── Phase 3: Sleep until next event or tick ───────────────────
        let mut submit_text: Option<String> = None;

        tokio::select! {
            biased;

            Some(event) = rx.recv() => {
                apply_ui_event(&mut shell, event);
                shell.dirty = true;
            }

            maybe_event = crossterm_events.next() => {
                if let Some(Ok(event)) = maybe_event {
                    apply_terminal_event(
                        &mut shell,
                        &mut textarea,
                        event,
                        &tx,
                        &mut submit_text,
                    );
                    shell.dirty = true;
                }
            }

            _ = &mut turn_future, if turn_active => {
                turn_active = false;
                turn_future = Box::pin(std::future::pending());
                shell.pane.agent_running = false;
                shell.dirty = true;
            }

            _ = tick.tick() => {
                shell.dirty = true; // tick always triggers render
            }
        }

        // Submit turn after select! releases borrows
        if let Some(text) = submit_text.take() {
            // ... existing turn submission logic
        }
    }
```

Note: The turn submission logic block (currently lines 524-572) must be
preserved intact. Copy it into both the drain-phase submit and the
sleep-phase submit locations, or extract it into a helper function to
avoid duplication.

- [ ] **Step 5: Run tests**

Run: `cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui && cargo test -p loongclaw-app --all-features`
Expected: all pass

- [ ] **Step 6: Run full workspace checks**

Run: `cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui && cargo fmt --all -- --check && cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: clean

- [ ] **Step 7: Commit**

```bash
cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui
git add crates/app/src/chat/tui/state.rs crates/app/src/chat/tui/shell.rs
git commit -m "fix(tui): decouple event processing from rendering with dirty flag

Events are drained non-blocking, then rendering fires only when dirty
or on tick. Eliminates unconditional 100+fps redraws during streaming
and separates event processing from the render path."
```

---

## Chunk 5: Phase 4 — Final Verification

### Task 5: Full verification gate

- [ ] **Step 1: Format check**

Run: `cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui && cargo fmt --all -- --check`
Expected: clean

- [ ] **Step 2: Clippy**

Run: `cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui && cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: clean

- [ ] **Step 3: App tests**

Run: `cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui && cargo test -p loongclaw-app --all-features`
Expected: all pass

- [ ] **Step 4: Daemon tests (including PTY)**

Run: `cd /Users/xj/github/loongclaw/loongclaw/.worktrees/issue-689-balanced-chat-tui && cargo test -p loongclaw-daemon --all-features`
Expected: all pass (PTY tests now use vt100 frame capture)

- [ ] **Step 5: Verify issue resolution checklist**

For each issue, confirm the fix is structurally present:

| Issue | Verification |
|-------|-------------|
| 1. PTY screen buffer | `tui_pty.rs` uses `vt100::Parser`, no `accumulated: Vec<u8>` |
| 2. Dual-render | No `streaming_text: String` in state.rs, no `flush_streaming()` |
| 3. Full re-render | `shell.dirty` flag controls render, drain loop processes events without draw |
| 4. Modal focus | `FocusStack` in focus.rs, match-based input routing in shell.rs |
| 5. UI/render separation | Three-phase loop: drain → render → sleep |
