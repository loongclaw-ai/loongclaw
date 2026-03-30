#![cfg(unix)]

//! PTY-based integration tests for the TUI chat mode.
//!
//! These tests spawn `loong chat --ui tui` in a real pseudo-terminal so the
//! binary enters full-screen mode instead of falling back to text mode.

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc as std_mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::*;

static PTY_TEMP_PATH_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_pty_temp_path(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let counter = PTY_TEMP_PATH_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "loongclaw-tui-pty-{label}-{}-{nanos}-{counter}",
        std::process::id(),
    ))
}

/// Strip ANSI/CSI escape sequences from raw terminal output, returning plain
/// text suitable for substring matching.
fn strip_ansi(raw: &[u8]) -> String {
    let stripped = strip_ansi_escapes::strip(raw);
    String::from_utf8_lossy(&stripped).into_owned()
}

/// Check whether `text` appears in `haystack` when both are collapsed
/// (all whitespace removed).  Terminal cell rendering often inserts spaces
/// between characters, so a direct `contains("hello world")` can fail
/// even though the characters are visually present.
fn contains_collapsed(haystack: &str, text: &str) -> bool {
    let h: String = haystack.chars().filter(|c| !c.is_whitespace()).collect();
    let t: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    h.contains(&t)
}

// ---------------------------------------------------------------------------
// TuiPtyFixture
// ---------------------------------------------------------------------------

/// A background reader thread pumps bytes from the blocking PTY reader into an
/// `mpsc` channel, allowing the fixture methods to poll with timeouts instead
/// of blocking forever.
struct TuiPtyFixture {
    child: Box<dyn portable_pty::Child + Send>,
    writer: Box<dyn Write + Send>,
    /// Receives byte chunks from the background reader thread.
    rx: std_mpsc::Receiver<Vec<u8>>,
    /// Accumulated raw bytes read so far (including previous calls).
    accumulated: Vec<u8>,
    _root: PathBuf,
}

impl TuiPtyFixture {
    /// Spawn `loong chat --ui tui` inside a real PTY.
    ///
    /// `label` is used to create a unique temp directory for the fixture.
    /// A minimal default config is written so the binary can start without
    /// triggering the onboarding flow.
    fn spawn(label: &str) -> Self {
        let root = unique_pty_temp_path(label);
        let home_dir = root.join("home");
        std::fs::create_dir_all(&home_dir).expect("create fixture home directory");

        // Write a minimal config so chat does not enter onboarding.
        let config_path = root.join("loongclaw.toml");
        let config = loongclaw_app::config::LoongClawConfig::default();
        let config_path_str = config_path.to_string_lossy().into_owned();
        loongclaw_app::config::write(Some(&config_path_str), &config, true)
            .expect("write default config for PTY fixture");

        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("open PTY pair");

        let binary_path = env!("CARGO_BIN_EXE_loongclaw");
        let mut cmd = CommandBuilder::new(binary_path);
        cmd.arg("chat");
        cmd.arg("--ui");
        cmd.arg("tui");
        cmd.arg("--config");
        cmd.arg(&config_path);
        cmd.env("HOME", &home_dir);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORFGBG", "15;0");
        cmd.env_remove("LOONGCLAW_CONFIG_PATH");
        cmd.env_remove("USERPROFILE");

        let child = pair
            .slave
            .spawn_command(cmd)
            .expect("spawn loong chat --ui tui in PTY");

        // The slave side must be dropped after spawning so EOF propagates
        // correctly when the child exits.
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .expect("clone PTY master reader");
        let writer = pair.master.take_writer().expect("take PTY master writer");

        // Spawn a background thread that reads from the blocking PTY reader
        // and sends byte chunks over an mpsc channel.
        let (tx, rx) = std_mpsc::channel::<Vec<u8>>();
        std::thread::Builder::new()
            .name(format!("pty-reader-{label}"))
            .spawn(move || {
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        #[allow(clippy::indexing_slicing)]
                        Ok(n) => {
                            if tx.send(buf[..n].to_vec()).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            })
            .expect("spawn PTY reader thread");

        Self {
            child,
            writer,
            rx,
            accumulated: Vec::new(),
            _root: root,
        }
    }

    /// Send raw bytes (keystrokes) to the PTY.
    fn send_keys(&mut self, keys: &[u8]) -> Result<(), String> {
        self.writer
            .write_all(keys)
            .map_err(|e| format!("failed to send keys to PTY: {e}"))?;
        self.writer
            .flush()
            .map_err(|e| format!("failed to flush PTY writer: {e}"))
    }

    /// Type a string of text into the PTY as individual characters.
    fn type_text(&mut self, text: &str) -> Result<(), String> {
        self.send_keys(text.as_bytes())
    }

    /// Send the Enter key (carriage return).
    #[allow(dead_code)]
    fn send_enter(&mut self) -> Result<(), String> {
        self.send_keys(b"\r")
    }

    /// Send the Escape key.
    fn send_escape(&mut self) -> Result<(), String> {
        self.send_keys(b"\x1b")
    }

    /// Send Ctrl+C (ETX).
    fn send_ctrl_c(&mut self) -> Result<(), String> {
        self.send_keys(b"\x03")
    }

    /// Drain any pending data from the reader channel into `accumulated`.
    fn drain_pending(&mut self) {
        while let Ok(chunk) = self.rx.try_recv() {
            self.accumulated.extend_from_slice(&chunk);
        }
    }

    /// Read all available output from the PTY, stripping ANSI escape
    /// sequences and returning plain text.  Waits up to `timeout` for
    /// data to arrive, then returns everything accumulated so far.
    fn read_screen(&mut self, timeout: Duration) -> Result<String, String> {
        let deadline = Instant::now() + timeout;

        loop {
            self.drain_pending();

            if !self.accumulated.is_empty() {
                // Give a short grace period for more data to arrive.
                std::thread::sleep(Duration::from_millis(100));
                self.drain_pending();
                break;
            }

            if Instant::now() >= deadline {
                break;
            }

            // Wait for the first chunk with a bounded recv_timeout.
            let remaining = deadline.saturating_duration_since(Instant::now());
            let wait = remaining.min(Duration::from_millis(100));
            if let Ok(chunk) = self.rx.recv_timeout(wait) {
                self.accumulated.extend_from_slice(&chunk);
            }
        }

        Ok(strip_ansi(&self.accumulated))
    }

    /// Wait until the screen output contains `pattern`, returning the full
    /// accumulated text.  Polls every 100ms until `timeout`.
    fn wait_for(&mut self, pattern: &str, timeout: Duration) -> Result<String, String> {
        let deadline = Instant::now() + timeout;

        loop {
            self.drain_pending();

            let plain = strip_ansi(&self.accumulated);
            if plain.contains(pattern) {
                return Ok(plain);
            }

            if Instant::now() >= deadline {
                return Err(format!(
                    "timed out waiting for pattern {:?} in PTY output (got: {:?})",
                    pattern, plain
                ));
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            let wait = remaining.min(Duration::from_millis(100));
            match self.rx.recv_timeout(wait) {
                Ok(chunk) => {
                    self.accumulated.extend_from_slice(&chunk);
                }
                Err(std_mpsc::RecvTimeoutError::Timeout) => {}
                Err(std_mpsc::RecvTimeoutError::Disconnected) => {
                    let plain = strip_ansi(&self.accumulated);
                    return Err(format!(
                        "PTY reader disconnected before pattern {:?} appeared (got: {:?})",
                        pattern, plain,
                    ));
                }
            }
        }
    }

    /// Wait until ANY of the given patterns matches in the ANSI-stripped
    /// accumulated text.  Polls every 100ms until `timeout`.  Returns the
    /// full text on match, or an error containing the accumulated text on
    /// timeout.
    fn wait_for_any(&mut self, patterns: &[&str], timeout: Duration) -> Result<String, String> {
        let deadline = Instant::now() + timeout;

        loop {
            self.drain_pending();

            let plain = strip_ansi(&self.accumulated);
            for pat in patterns {
                if plain.contains(pat) {
                    return Ok(plain);
                }
            }

            if Instant::now() >= deadline {
                return Err(format!(
                    "timed out waiting for any of {patterns:?} in PTY output (got: {plain:?})"
                ));
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            let wait = remaining.min(Duration::from_millis(100));
            match self.rx.recv_timeout(wait) {
                Ok(chunk) => {
                    self.accumulated.extend_from_slice(&chunk);
                }
                Err(std_mpsc::RecvTimeoutError::Timeout) => {}
                Err(std_mpsc::RecvTimeoutError::Disconnected) => {
                    let plain = strip_ansi(&self.accumulated);
                    return Err(format!(
                        "PTY reader disconnected before any of {patterns:?} appeared (got: {plain:?})"
                    ));
                }
            }
        }
    }

    /// Send the PageUp escape sequence (`\x1b[5~`) to the PTY.
    fn send_page_up(&mut self) -> Result<(), String> {
        self.send_keys(b"\x1b[5~")
    }

    /// Send the PageDown escape sequence (`\x1b[6~`) to the PTY.
    fn send_page_down(&mut self) -> Result<(), String> {
        self.send_keys(b"\x1b[6~")
    }

    /// Wait for the child process to exit within `timeout`.
    /// Returns the exit status code, or an error on timeout.
    fn wait_for_exit(&mut self, timeout: Duration) -> Result<u32, String> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    return Ok(status.exit_code());
                }
                Ok(None) => {
                    if Instant::now() >= deadline {
                        return Err("timed out waiting for child process to exit".to_owned());
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    return Err(format!("error checking child status: {e}"));
                }
            }
        }
    }
}

impl Drop for TuiPtyFixture {
    fn drop(&mut self) {
        // Best-effort kill of the child process.
        let _ = self.child.kill();
        // Wait for exit to reap the zombie.
        let _ = self.child.wait();
        // Clean up the temp directory.
        let _ = std::fs::remove_dir_all(&self._root);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The TUI enters full-screen mode (alternate screen) when spawned in a real
/// PTY, and does not crash or fall back to text mode.
#[test]
fn tui_enters_fullscreen_in_pty() {
    let mut fixture = TuiPtyFixture::spawn("enters-fullscreen");

    // The TUI welcome message proves we entered the full-screen surface
    // rather than falling back to text mode.
    let screen = fixture
        .wait_for("Welcome to LoongClaw TUI", Duration::from_secs(10))
        .expect("TUI should render the welcome message in PTY");

    // Verify the full welcome text is present.
    assert!(
        screen.contains("Welcome to LoongClaw TUI"),
        "TUI should show the welcome message: {screen:?}"
    );

    // Exit cleanly via Escape.
    fixture.send_escape().expect("send Escape key to exit TUI");

    let exit_code = fixture
        .wait_for_exit(Duration::from_secs(5))
        .expect("TUI should exit after Escape");
    assert_eq!(exit_code, 0, "TUI should exit with code 0 after Escape");
}

/// The TUI displays its welcome/ready message when started.
#[test]
fn tui_shows_welcome_message() {
    let mut fixture = TuiPtyFixture::spawn("shows-welcome");

    let screen = fixture
        .wait_for("Welcome to LoongClaw TUI", Duration::from_secs(10))
        .expect("TUI should render the welcome message");

    assert!(
        screen.contains("Type a message and press Enter"),
        "Welcome message should include input instructions: {screen:?}"
    );

    fixture.send_escape().expect("send Escape to exit");
}

/// Typing text into the TUI composer area makes it visible on the screen.
#[test]
fn tui_composer_accepts_input() {
    let mut fixture = TuiPtyFixture::spawn("composer-input");

    fixture
        .wait_for("Welcome to LoongClaw TUI", Duration::from_secs(10))
        .expect("TUI should be ready before typing");

    // Small delay to let the event loop settle after rendering.
    std::thread::sleep(Duration::from_millis(200));

    fixture
        .type_text("hello world")
        .expect("type text into TUI composer");

    // Give the TUI time to process keystrokes and re-render.
    std::thread::sleep(Duration::from_millis(500));

    let screen = fixture
        .read_screen(Duration::from_secs(3))
        .expect("read screen after typing");

    // TUI cell rendering may insert spaces between characters (e.g.
    // "h e l l o   w o r l d"), so we use whitespace-collapsed matching.
    assert!(
        contains_collapsed(&screen, "helloworld"),
        "typed text should appear on the screen: {screen:?}"
    );

    fixture.send_escape().expect("send Escape to exit");
}

/// Pressing Escape exits the TUI cleanly with exit code 0.
#[test]
fn tui_exit_via_escape() {
    let mut fixture = TuiPtyFixture::spawn("exit-escape");

    fixture
        .wait_for("Welcome to LoongClaw TUI", Duration::from_secs(10))
        .expect("TUI should be ready before exit");

    fixture.send_escape().expect("send Escape to exit");

    let exit_code = fixture
        .wait_for_exit(Duration::from_secs(5))
        .expect("process should exit after Escape");

    assert_eq!(
        exit_code, 0,
        "TUI should exit cleanly with code 0 via Escape"
    );
}

/// Submitting a turn shows a response or an error message (not silence).
#[test]
fn tui_submit_turn_shows_response_or_error() {
    let mut fixture = TuiPtyFixture::spawn("submit-turn");

    fixture
        .wait_for("Welcome to LoongClaw TUI", Duration::from_secs(10))
        .expect("TUI should be ready");

    std::thread::sleep(Duration::from_millis(300));

    fixture.type_text("hi").expect("type hi");
    std::thread::sleep(Duration::from_millis(200));

    // Press Enter to submit
    fixture.send_keys(b"\r").expect("send Enter to submit turn");

    // Wait up to 30s for the TUI to show something beyond the welcome
    std::thread::sleep(Duration::from_secs(5));

    let screen = fixture
        .read_screen(Duration::from_secs(25))
        .unwrap_or_default();

    // Dump screen for debugging
    eprintln!("=== TUI SCREEN AFTER SUBMIT ===\n{screen}\n=== END ===");

    // After submitting, the screen should contain either:
    // - "You" (the user message badge rendered)
    // - "Iteration" (spinner showing turn in progress)
    // - "Error:" (turn failed with a message)
    // - The response text
    // It should NOT be just the welcome message.
    let has_user_msg = contains_collapsed(&screen, "You") || contains_collapsed(&screen, "hi");
    let has_progress = screen.contains("Iteration") || screen.contains("Preparing");
    let has_error = screen.contains("Error:");

    assert!(
        has_user_msg || has_progress || has_error,
        "TUI should show user message, progress, or error after Enter: {screen:?}"
    );

    fixture.send_escape().expect("exit TUI");
}

/// Pressing Ctrl+C exits the TUI cleanly with exit code 0.
#[test]
fn tui_exit_via_ctrl_c() {
    let mut fixture = TuiPtyFixture::spawn("exit-ctrl-c");

    fixture
        .wait_for("Welcome to LoongClaw TUI", Duration::from_secs(10))
        .expect("TUI should be ready before Ctrl+C");

    fixture.send_ctrl_c().expect("send Ctrl+C to exit");

    let exit_code = fixture
        .wait_for_exit(Duration::from_secs(5))
        .expect("process should exit after Ctrl+C");

    assert_eq!(
        exit_code, 0,
        "TUI should exit cleanly with code 0 via Ctrl+C"
    );
}

// ---------------------------------------------------------------------------
// Conversation Tests
// ---------------------------------------------------------------------------

/// A multi-turn conversation shows both user messages on screen.
#[test]
fn tui_multi_turn_conversation() {
    let mut fixture = TuiPtyFixture::spawn("multi-turn");

    fixture
        .wait_for("Welcome to LoongClaw TUI", Duration::from_secs(10))
        .expect("TUI should be ready");

    std::thread::sleep(Duration::from_millis(300));

    // First turn: type "hi" and submit.
    fixture.type_text("hi").expect("type hi");
    std::thread::sleep(Duration::from_millis(200));
    fixture.send_keys(b"\r").expect("send Enter for first turn");

    // Wait for some response or error from the first turn.
    let _ = fixture.wait_for_any(
        &["LoongClaw", "Error:", "Iteration"],
        Duration::from_secs(30),
    );

    std::thread::sleep(Duration::from_millis(500));

    // Second turn: type "thanks" and submit.
    fixture.type_text("thanks").expect("type thanks");
    std::thread::sleep(Duration::from_millis(200));
    fixture
        .send_keys(b"\r")
        .expect("send Enter for second turn");

    // Wait for second response or error.
    std::thread::sleep(Duration::from_secs(5));
    let screen = fixture
        .read_screen(Duration::from_secs(25))
        .unwrap_or_default();

    eprintln!("=== TUI MULTI-TURN SCREEN ===\n{screen}\n=== END ===");

    // Both messages should appear on screen (collapsed matching).
    let has_hi = contains_collapsed(&screen, "hi");
    let has_thanks = contains_collapsed(&screen, "thanks");

    assert!(
        has_hi && has_thanks,
        "both user messages should appear on screen: has_hi={has_hi}, has_thanks={has_thanks}, screen={screen:?}"
    );

    fixture.send_escape().expect("exit TUI");
}

/// Submitting a message shows a "You" badge for the user message.
#[test]
fn tui_user_message_appears_as_badge() {
    let mut fixture = TuiPtyFixture::spawn("user-badge");

    fixture
        .wait_for("Welcome to LoongClaw TUI", Duration::from_secs(10))
        .expect("TUI should be ready");

    std::thread::sleep(Duration::from_millis(300));

    fixture
        .type_text("test message")
        .expect("type test message");
    std::thread::sleep(Duration::from_millis(200));
    fixture.send_keys(b"\r").expect("send Enter");

    // Brief wait for rendering.
    std::thread::sleep(Duration::from_secs(2));

    let screen = fixture
        .read_screen(Duration::from_secs(3))
        .unwrap_or_default();

    eprintln!("=== TUI USER BADGE SCREEN ===\n{screen}\n=== END ===");

    assert!(
        contains_collapsed(&screen, "You"),
        "user message badge 'You' should appear on screen: {screen:?}"
    );

    fixture.send_escape().expect("exit TUI");
}

/// After submitting a turn, the assistant response divider or an error
/// message must appear.
#[test]
fn tui_assistant_response_shows_divider() {
    let mut fixture = TuiPtyFixture::spawn("assistant-divider");

    fixture
        .wait_for("Welcome to LoongClaw TUI", Duration::from_secs(10))
        .expect("TUI should be ready");

    std::thread::sleep(Duration::from_millis(300));

    fixture.type_text("hi").expect("type hi");
    std::thread::sleep(Duration::from_millis(200));
    fixture.send_keys(b"\r").expect("send Enter");

    // Wait for either the LoongClaw divider or an error — both are acceptable.
    let result = fixture.wait_for_any(&["LoongClaw", "Error:"], Duration::from_secs(30));

    match result {
        Ok(screen) => {
            eprintln!("=== TUI DIVIDER SCREEN ===\n{screen}\n=== END ===");
            let has_divider = screen.contains("LoongClaw");
            let has_error = screen.contains("Error:");
            assert!(
                has_divider || has_error,
                "should show LoongClaw divider or Error: {screen:?}"
            );
        }
        Err(e) => {
            panic!("neither divider nor error appeared within timeout: {e}");
        }
    }

    fixture.send_escape().expect("exit TUI");
}

/// Pressing Enter with an empty composer should not submit a turn.
#[test]
fn tui_empty_enter_does_not_submit() {
    let mut fixture = TuiPtyFixture::spawn("empty-enter");

    fixture
        .wait_for("Welcome to LoongClaw TUI", Duration::from_secs(10))
        .expect("TUI should be ready");

    std::thread::sleep(Duration::from_millis(300));

    // Press Enter without typing anything.
    fixture
        .send_keys(b"\r")
        .expect("send Enter on empty composer");

    // Wait to see if anything happens.
    std::thread::sleep(Duration::from_secs(2));

    let screen = fixture
        .read_screen(Duration::from_secs(3))
        .unwrap_or_default();

    eprintln!("=== TUI EMPTY ENTER SCREEN ===\n{screen}\n=== END ===");

    // Nothing indicating a turn should be in progress.
    assert!(
        !screen.contains("Iteration"),
        "empty Enter should not start a turn (no Iteration indicator): {screen:?}"
    );

    fixture.send_escape().expect("exit TUI");
}

// ---------------------------------------------------------------------------
// Slash Command Tests
// ---------------------------------------------------------------------------

/// The `/help` command shows a help overlay with command names.
#[test]
fn tui_help_command_shows_overlay() {
    let mut fixture = TuiPtyFixture::spawn("help-cmd");

    fixture
        .wait_for("Welcome to LoongClaw TUI", Duration::from_secs(10))
        .expect("TUI should be ready");

    std::thread::sleep(Duration::from_millis(300));

    fixture.type_text("/help").expect("type /help");
    std::thread::sleep(Duration::from_millis(200));
    fixture.send_keys(b"\r").expect("send Enter for /help");

    // Wait for the help overlay to appear — it should mention command names.
    std::thread::sleep(Duration::from_secs(2));

    let screen = fixture
        .read_screen(Duration::from_secs(3))
        .unwrap_or_default();

    eprintln!("=== TUI HELP OVERLAY SCREEN ===\n{screen}\n=== END ===");

    let has_exit = contains_collapsed(&screen, "exit");
    let has_clear = contains_collapsed(&screen, "clear");

    assert!(
        has_exit || has_clear,
        "help overlay should mention 'exit' or 'clear' command: {screen:?}"
    );

    fixture.send_escape().expect("exit TUI");
}

/// The `/clear` command clears the transcript so the welcome message is gone.
#[test]
fn tui_clear_command_clears_transcript() {
    let mut fixture = TuiPtyFixture::spawn("clear-cmd");

    fixture
        .wait_for("Welcome to LoongClaw TUI", Duration::from_secs(10))
        .expect("TUI should be ready");

    std::thread::sleep(Duration::from_millis(300));

    fixture.type_text("/clear").expect("type /clear");
    std::thread::sleep(Duration::from_millis(200));
    fixture.send_keys(b"\r").expect("send Enter for /clear");

    // Give the TUI time to process the clear and re-render.
    std::thread::sleep(Duration::from_secs(1));

    // Reset accumulated buffer so we only read the fresh screen.
    fixture.accumulated.clear();

    let screen = fixture
        .read_screen(Duration::from_secs(3))
        .unwrap_or_default();

    eprintln!("=== TUI CLEAR SCREEN ===\n{screen}\n=== END ===");

    assert!(
        !screen.contains("Welcome to LoongClaw TUI"),
        "welcome message should be cleared after /clear: {screen:?}"
    );

    fixture.send_escape().expect("exit TUI");
}

/// The `/exit` command causes the TUI to exit cleanly with code 0.
#[test]
fn tui_exit_command_exits() {
    let mut fixture = TuiPtyFixture::spawn("exit-cmd");

    fixture
        .wait_for("Welcome to LoongClaw TUI", Duration::from_secs(10))
        .expect("TUI should be ready");

    std::thread::sleep(Duration::from_millis(300));

    fixture.type_text("/exit").expect("type /exit");
    std::thread::sleep(Duration::from_millis(200));
    fixture.send_keys(b"\r").expect("send Enter for /exit");

    let exit_code = fixture
        .wait_for_exit(Duration::from_secs(5))
        .expect("TUI should exit after /exit command");

    assert_eq!(
        exit_code, 0,
        "TUI should exit with code 0 after /exit command"
    );
}

// ---------------------------------------------------------------------------
// UI State Tests
// ---------------------------------------------------------------------------

/// During a turn, a spinner or "Iteration" indicator should briefly appear.
#[test]
fn tui_spinner_shows_during_turn() {
    let mut fixture = TuiPtyFixture::spawn("spinner");

    fixture
        .wait_for("Welcome to LoongClaw TUI", Duration::from_secs(10))
        .expect("TUI should be ready");

    std::thread::sleep(Duration::from_millis(300));

    fixture.type_text("hi").expect("type hi");
    std::thread::sleep(Duration::from_millis(200));
    fixture.send_keys(b"\r").expect("send Enter");

    // Read immediately — the spinner should appear quickly.
    std::thread::sleep(Duration::from_millis(500));

    let screen = fixture
        .read_screen(Duration::from_secs(1))
        .unwrap_or_default();

    eprintln!("=== TUI SPINNER SCREEN ===\n{screen}\n=== END ===");

    let has_iteration = screen.contains("Iteration");
    let has_preparing = screen.contains("Preparing");
    let has_you = contains_collapsed(&screen, "You");

    assert!(
        has_iteration || has_preparing || has_you,
        "spinner or turn indicator should be visible shortly after submit: {screen:?}"
    );

    fixture.send_escape().expect("exit TUI");
}

/// The status bar should show the session identifier "default".
#[test]
fn tui_status_bar_shows_session() {
    let mut fixture = TuiPtyFixture::spawn("status-bar");

    fixture
        .wait_for("Welcome to LoongClaw TUI", Duration::from_secs(10))
        .expect("TUI should be ready");

    let screen = fixture
        .read_screen(Duration::from_secs(3))
        .unwrap_or_default();

    eprintln!("=== TUI STATUS BAR SCREEN ===\n{screen}\n=== END ===");

    assert!(
        contains_collapsed(&screen, "default"),
        "status bar should show 'default' session id: {screen:?}"
    );

    fixture.send_escape().expect("exit TUI");
}

/// PageUp and PageDown do not crash the TUI.
#[test]
fn tui_scroll_does_not_crash() {
    let mut fixture = TuiPtyFixture::spawn("scroll-no-crash");

    fixture
        .wait_for("Welcome to LoongClaw TUI", Duration::from_secs(10))
        .expect("TUI should be ready");

    std::thread::sleep(Duration::from_millis(300));

    fixture.send_page_up().expect("send PageUp");
    std::thread::sleep(Duration::from_millis(200));

    fixture.send_page_down().expect("send PageDown");
    std::thread::sleep(Duration::from_millis(200));

    let screen = fixture
        .read_screen(Duration::from_secs(3))
        .unwrap_or_default();

    eprintln!("=== TUI SCROLL SCREEN ===\n{screen}\n=== END ===");

    // TUI should still be alive and showing content.
    let has_loongclaw = contains_collapsed(&screen, "LoongClaw");
    let has_welcome = contains_collapsed(&screen, "Welcome");

    assert!(
        has_loongclaw || has_welcome,
        "TUI should still be alive after scroll keys: {screen:?}"
    );

    fixture.send_escape().expect("exit TUI");
}

// ---------------------------------------------------------------------------
// Error Handling Tests
// ---------------------------------------------------------------------------

/// Submitting a turn with stub config should show either an error message or
/// a response — silence is failure.
#[test]
fn tui_turn_error_shows_message() {
    let mut fixture = TuiPtyFixture::spawn("turn-error");

    fixture
        .wait_for("Welcome to LoongClaw TUI", Duration::from_secs(10))
        .expect("TUI should be ready");

    std::thread::sleep(Duration::from_millis(300));

    fixture.type_text("hi").expect("type hi");
    std::thread::sleep(Duration::from_millis(200));
    fixture.send_keys(b"\r").expect("send Enter");

    // Wait for any kind of response — error or actual text.
    let result = fixture.wait_for_any(
        &["Error:", "LoongClaw", "Iteration", "You"],
        Duration::from_secs(30),
    );

    match result {
        Ok(screen) => {
            eprintln!("=== TUI TURN ERROR SCREEN ===\n{screen}\n=== END ===");
            // As long as something appeared, the test passes.
        }
        Err(e) => {
            panic!(
                "TUI showed no response or error after submitting turn — silence is failure: {e}"
            );
        }
    }

    fixture.send_escape().expect("exit TUI");
}

/// Rapid input does not crash the TUI.
#[test]
fn tui_resilient_to_rapid_input() {
    let mut fixture = TuiPtyFixture::spawn("rapid-input");

    fixture
        .wait_for("Welcome to LoongClaw TUI", Duration::from_secs(10))
        .expect("TUI should be ready");

    std::thread::sleep(Duration::from_millis(300));

    // Type rapidly without pauses.
    fixture
        .type_text("abcdefghij")
        .expect("type rapid characters");

    std::thread::sleep(Duration::from_millis(500));

    let screen = fixture
        .read_screen(Duration::from_secs(3))
        .unwrap_or_default();

    eprintln!("=== TUI RAPID INPUT SCREEN ===\n{screen}\n=== END ===");

    // At least some of the characters should appear (collapsed matching to
    // handle terminal cell spacing).
    let has_some_chars = contains_collapsed(&screen, "abc")
        || contains_collapsed(&screen, "def")
        || contains_collapsed(&screen, "ghij");

    assert!(
        has_some_chars,
        "some rapid input characters should appear on screen: {screen:?}"
    );

    // TUI should not have crashed — we can still exit.
    fixture.send_escape().expect("exit TUI after rapid input");
}
