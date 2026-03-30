# TUI Architecture Redesign — Fix 5 Structural Issues

## Summary

The TUI module (`crates/app/src/chat/tui/`, 19 files, ~4,200 lines) on branch
`feat/issue-689-balanced-chat-tui` has five architectural issues that prevent
closing the verification loop. This spec defines a gated-parallel execution plan
to fix all five issues using multi-agent orchestration.

## Issues

| # | Issue | Root cause | Impact |
|---|-------|-----------|--------|
| 1 | No real-time screen buffer | PTY tests accumulate all bytes since launch | Cannot verify what the user actually sees |
| 2 | Streaming text dual-render | Separate `streaming_text` buffer + MessagePart | Flash/duplication during flush |
| 3 | Full re-render on every event | No dirty tracking; draw() at top of every loop | Unnecessary redraws (100+/sec during streaming) |
| 4 | No modal focus system | Ad-hoc if-chains for help/dialog input capture | Adding new modals requires two-place edits |
| 5 | No UI/render separation | Events and rendering in same select! loop | Slow render blocks event processing |

## Approach

Gated parallel batches (Approach B). Four phases with explicit file ownership
boundaries to prevent merge conflicts between parallel agents.

```
Phase 1: PTY frame capture upgrade ─────────────────────┐
                                                         │
Phase 2a: Streaming unification ─────────────────────────┤ (parallel)
Phase 2b: Modal focus system ────────────────────────────┤
                                                         │
Phase 3: Event/render decoupling ────────────────────────┘
                                                         │
Phase 4: Verification ───────────────────────────────────┘
```

## Phase 1: PTY Frame Capture Upgrade

**Fixes:** Issue 1 (no real-time screen buffer)

**Problem:** `TuiPtyFixture` accumulates all PTY output bytes into
`accumulated: Vec<u8>`, strips ANSI, and does substring matching. This shows
everything ever printed, not the current screen frame.

**Solution:** Replace byte accumulation with the `vt100` crate, which processes
escape sequences and exposes the current terminal screen buffer.

**Changes:**
- Add `vt100` as dev-dependency to `crates/daemon/Cargo.toml`
- `tui_pty.rs`: Replace `accumulated: Vec<u8>` with `parser: vt100::Parser::new(24, 80, 0)`
  (80x24 matches PTY fixture size; scrollback 0 for current-frame-only capture)
- `drain_pending()`: Feed chunks into `parser.process()` instead of vec extend
- `read_screen()`: Return `parser.screen().contents()` (current frame, not history)
- `wait_for()`: Pattern-match against current frame, polling until match or timeout
- `wait_for_any()` (lines 263-297): Same treatment as `wait_for()` — reads from
  `parser.screen().contents()` instead of `strip_ansi(&self.accumulated)`
- Keep `strip_ansi()` helper for test utility (not a production compat shim)

**Files:** `crates/daemon/tests/integration/tui_pty.rs`,
`crates/daemon/Cargo.toml` (dev-dep only)

**Gate:** All existing PTY tests must pass with the new capture method before
proceeding to Phase 2.

## Phase 2a: Streaming Text Unification

**Fixes:** Issue 2 (streaming text dual-render)

**Problem:** `Pane.streaming_text: String` accumulates tokens. Rendering shows
both `streaming_text` (as in-progress) and `messages[].parts[]` (as history).
When `flush_streaming()` fires, text briefly exists in both locations.

**Solution:** Eliminate the separate buffer. Accumulate tokens directly into the
last `MessagePart` of the current assistant message.

**Changes:**
- `state.rs`: Remove `streaming_text: String` field. Add `streaming_active: bool`.
- `append_token()`: Extend the last `MessagePart::Text` or `ThinkBlock` in-place,
  or push a new part if the `is_thinking` flag changed (preserving the
  thinking-toggle invariant that keeps `ThinkBlock` and `Text` parts separate).
- `flush_streaming()`: Remove entirely.
- `start_tool_call()`: Remove the `flush_streaming()` call — no longer needed since
  tokens are already committed to `MessagePart` in-place.
- `finalize_response()`: Remove the `flush_streaming()` call — same reason. Set
  `streaming_active = false` instead.
- `history.rs`: The `PaneView` trait (lines 16-21) defines `streaming_text()` and
  `is_thinking()` — replace both with `streaming_active()`. Update
  `render_history()` (lines 47-59) to show a cursor/highlight on the last part of
  the current assistant message when `streaming_active()` is true.
- `shell.rs` (PaneView impl only): The `PaneView` impl for `Pane` at lines 46-51
  returns `&self.streaming_text` and `self.is_thinking` — update to return
  `self.streaming_active`. This is a trait impl change, not an event loop change.

**Files owned:** `state.rs` (streaming fields), `history.rs` (PaneView trait +
render_history), `shell.rs` (PaneView impl only — not event loop)

**Contract:** Must not touch `shell.rs` event loop structure, input routing, or
`focus.rs`. The only `shell.rs` change is updating the `PaneView` trait impl to
match the new trait signature.

## Phase 2b: Modal Focus System

**Fixes:** Issue 4 (no modal focus system)

**Problem:** `focus.rs` is a 14-line stub. Input routing uses ad-hoc if-chains:
`if dialog.is_some()` / `if show_help` / else. Each modal manually swallows keys.
Adding a new modal means editing two places (input + render).

**Solution:** Introduce `FocusStack` that tracks active layers. Input routing and
render dispatch both read from the stack.

**Changes:**
- `focus.rs`: Define `FocusLayer` enum (`Composer`, `Help`, `ClarifyDialog`) and
  `FocusStack` with push/pop/top/has operations. `Composer` is always the base.
- `state.rs`: Replace `show_help: bool` on `Shell` with `FocusStack`. Remove
  the existing `focus_target: FocusTarget` field on `UiState` (lines 219-246) if
  it exists — consolidate all focus state into the single `FocusStack` on `Shell`.
  Keep `ClarifyDialog` data in `Pane` but track active state via stack.
- `shell.rs` (input routing only): Replace if-chain with `match shell.pane.focus.top()`.
- `render.rs` (ShellView trait + overlay dispatch): The `ShellView` trait (lines
  24-31) exposes `fn show_help(&self) -> bool` — replace with
  `fn focus(&self) -> &FocusStack`. Update the overlay dispatch to iterate the
  focus stack bottom-to-top; each layer above `Composer` gets `Clear` + its widget.
  Also update the `TestShell` struct in the render.rs test module (line ~409) to
  implement the new `ShellView` signature with a test `FocusStack`.

**Files owned:** `focus.rs`, `state.rs` (focus fields, UiState cleanup), `shell.rs`
(input match), `render.rs` (ShellView trait + overlay loop + TestShell in tests),
`app_shell.rs` (`build_shell_bootstrap_state` return type), `chat.rs` (two tests
at lines ~4182 and ~4216 that assert on `UiState.focus_target` / `FocusTarget::Composer`
— rewrite to assert against the new `FocusStack` API)

**Contract:** Must not touch streaming/message fields in `state.rs` or the
`select!` loop structure in `shell.rs`.

## Phase 2a/2b File Ownership Matrix

| File | Phase 2a owns | Phase 2b owns | Conflict? |
|------|:---:|:---:|:---:|
| state.rs | streaming fields | focus fields + UiState cleanup | No — disjoint fields |
| history.rs | PaneView trait + render_history | - | No |
| render.rs | - | ShellView trait + overlay loop + TestShell | No |
| shell.rs | PaneView impl only | input match only | No — disjoint sections |
| focus.rs | - | full | No |
| app_shell.rs | - | bootstrap return type | No |
| chat.rs (tests) | - | focus_target assertions | No |

## Phase 3: Event/Render Decoupling

**Fixes:** Issues 3 + 5 (full re-render, no UI/render separation)

**Problem:** The loop renders unconditionally at the top of every iteration.
Every token, tick, and keypress triggers `terminal.draw()`. Events and rendering
are interleaved in the same `select!` loop.

**Solution:** Split into two concerns: events set a `dirty` flag; rendering fires
only on tick when dirty.

**Changes:**
- `state.rs`: Add `dirty: bool` to `Shell`. Every state mutation sets it.
- `shell.rs`: Restructure the event loop into two phases:
  1. **Drain phase**: Non-blocking drain of all buffered events. Use
     `rx.try_recv()` in a while-let loop for the observer channel. For crossterm
     `EventStream`, use `poll_next_unpin()` with `cx` or `futures::poll!` to
     check without blocking. Do NOT use `tokio::select! { else => break }` — the
     `else` branch fires only when all guards are false, not when channels are
     empty. The correct pattern:
     ```rust
     // Drain observer channel
     while let Ok(event) = rx.try_recv() {
         apply_ui_event(&mut shell, event);
     }
     // Check crossterm (non-blocking)
     while let Some(event) = crossterm_events.next().now_or_never().flatten() {
         if let Ok(event) = event { apply_terminal_event(...); }
     }
     ```
  2. **Render phase**: If `dirty` or tick elapsed, call `tick_spinner()` +
     `guard.draw()` + reset dirty.
  3. **Sleep phase**: `tokio::select!` on all sources + tick to wake for next
     event or render cycle.

**Result:** Batch event processing, tick-gated rendering (max 20fps), skip render
when idle. Requires `futures::FutureExt` for `now_or_never()`.

**Files owned:** `shell.rs` (event loop restructure), `state.rs` (`dirty` flag)

**Dependency:** Must run after Phase 2a and 2b. Their changes to `state.rs` fields
and `shell.rs` input routing must be landed first.

## Phase 4: Verification

Run in the worktree after all phases complete:

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
3. `cargo test -p loongclaw-daemon --all-features` (PTY tests)
4. `cargo test -p loongclaw-app --all-features`
5. Manual: `./target/debug/loong chat --ui tui` — streaming renders without flash,
   help overlay captures focus, spinner runs smoothly

## PTY Verification Assertions (enabled by Phase 1)

After Phase 1 lands, these assertions become possible in PTY tests:

- **Issue 2 verified:** Send tokens, capture frame — text appears in exactly one
  location (history area), not duplicated in a separate streaming area.
- **Issue 4 verified:** Open help, capture frame — help overlay visible. Send
  keypress, capture frame — keypress not echoed in composer.
- **Issues 3+5 verified:** During idle (no events), consecutive frame captures
  return identical content (no unnecessary redraws observed via screen diff).

## Non-goals

- Multi-pane / split / tabs
- Mouse support
- Streaming markdown rendering (streamdown-rs)
- @mention routing between agents
- New PTY tests for every edge case (only structural verification)
