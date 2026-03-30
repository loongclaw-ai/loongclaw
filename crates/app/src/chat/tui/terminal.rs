use std::io::IsTerminal;

// ---------------------------------------------------------------------------
// Snapshot-based terminal detection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TerminalSupportSnapshot {
    pub(crate) stdin_is_terminal: bool,
    pub(crate) stdout_is_terminal: bool,
    pub(crate) stderr_is_terminal: bool,
    pub(crate) term: Option<String>,
    pub(crate) color_support: bool,
    pub(crate) colorfgbg: Option<String>,
}

impl TerminalSupportSnapshot {
    pub(crate) fn capture_current() -> Self {
        Self {
            stdin_is_terminal: std::io::stdin().is_terminal(),
            stdout_is_terminal: std::io::stdout().is_terminal(),
            stderr_is_terminal: std::io::stderr().is_terminal(),
            term: std::env::var("TERM").ok(),
            color_support: supports_color::on(supports_color::Stream::Stdout).is_some(),
            colorfgbg: std::env::var("COLORFGBG").ok(),
        }
    }
}

// ---------------------------------------------------------------------------
// Launch decision
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TerminalLaunch {
    Tui,
    FallbackToText { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaletteHint {
    Dark,
    Light,
    Plain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TerminalPolicy {
    pub(crate) launch: TerminalLaunch,
    pub(crate) palette_hint: PaletteHint,
}

/// Pure-function launch-mode resolver.
pub(crate) fn resolve_launch_mode(snapshot: &TerminalSupportSnapshot) -> TerminalLaunch {
    if !snapshot.stdin_is_terminal || !snapshot.stdout_is_terminal {
        return TerminalLaunch::FallbackToText {
            reason: "TUI requires stdin/stdout to be terminal-attached".to_owned(),
        };
    }

    if snapshot
        .term
        .as_deref()
        .is_some_and(|term| term.eq_ignore_ascii_case("dumb"))
    {
        return TerminalLaunch::FallbackToText {
            reason: "TUI requires a non-dumb terminal".to_owned(),
        };
    }

    TerminalLaunch::Tui
}

/// Detect whether the terminal background is light by inspecting `COLORFGBG`.
///
/// `COLORFGBG` is set by many terminal emulators (iTerm2, xterm, rxvt) as
/// `<fg>;<bg>` where ANSI colors 0-6 are dark and 7-15 are light.
/// A bg value >= 7 (except 8 which is dark gray) suggests a light background.
fn detect_light_background(colorfgbg: Option<&str>) -> bool {
    let Some(val) = colorfgbg else {
        return false;
    };
    let Some(bg_str) = val.rsplit(';').next() else {
        return false;
    };
    let Ok(bg) = bg_str.trim().parse::<u16>() else {
        return false;
    };
    bg >= 7 && bg != 8
}

/// Combines launch-mode resolution with palette selection.
pub(crate) fn resolve_terminal_policy(snapshot: TerminalSupportSnapshot) -> TerminalPolicy {
    let palette_hint = if !snapshot.color_support {
        PaletteHint::Plain
    } else if detect_light_background(snapshot.colorfgbg.as_deref()) {
        PaletteHint::Light
    } else {
        PaletteHint::Dark
    };
    let launch = resolve_launch_mode(&snapshot);

    TerminalPolicy {
        launch,
        palette_hint,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::tui::theme::SemanticPalette;
    use ratatui::style::Color;

    #[test]
    fn default_semantic_palette_shape_is_conservative() {
        let palette = SemanticPalette::default();

        assert_eq!(palette.text, Color::White);
        assert_eq!(palette.border, Color::DarkGray);
        assert_eq!(palette.accent, Color::Cyan);
        assert_eq!(palette.warning, Color::Yellow);
        assert_eq!(palette.error, Color::Red);
    }

    #[test]
    fn terminal_policy_chooses_text_mode_when_tty_preconditions_fail() {
        let policy = resolve_terminal_policy(TerminalSupportSnapshot {
            stdin_is_terminal: false,
            stdout_is_terminal: false,
            stderr_is_terminal: false,
            term: Some("xterm-256color".to_owned()),
            color_support: false,
            colorfgbg: None,
        });

        assert!(matches!(
            policy.launch,
            TerminalLaunch::FallbackToText { .. }
        ));
        assert_eq!(policy.palette_hint, PaletteHint::Plain);
    }

    #[test]
    fn terminal_policy_chooses_tui_when_all_conditions_met() {
        let policy = resolve_terminal_policy(TerminalSupportSnapshot {
            stdin_is_terminal: true,
            stdout_is_terminal: true,
            stderr_is_terminal: true,
            term: Some("xterm-256color".to_owned()),
            color_support: true,
            colorfgbg: None,
        });

        assert!(matches!(policy.launch, TerminalLaunch::Tui));
        assert_eq!(policy.palette_hint, PaletteHint::Dark);
    }

    #[test]
    fn dumb_terminal_falls_back() {
        let launch = resolve_launch_mode(&TerminalSupportSnapshot {
            stdin_is_terminal: true,
            stdout_is_terminal: true,
            stderr_is_terminal: true,
            term: Some("dumb".to_owned()),
            color_support: false,
            colorfgbg: None,
        });

        assert!(matches!(launch, TerminalLaunch::FallbackToText { .. }));
    }

    #[test]
    fn missing_term_env_does_not_block_launch() {
        let launch = resolve_launch_mode(&TerminalSupportSnapshot {
            stdin_is_terminal: true,
            stdout_is_terminal: true,
            stderr_is_terminal: true,
            term: None,
            color_support: true,
            colorfgbg: None,
        });

        assert!(matches!(launch, TerminalLaunch::Tui));
    }

    #[test]
    fn no_color_support_still_launches_with_plain_palette() {
        let policy = resolve_terminal_policy(TerminalSupportSnapshot {
            stdin_is_terminal: true,
            stdout_is_terminal: true,
            stderr_is_terminal: true,
            term: Some("xterm".to_owned()),
            color_support: false,
            colorfgbg: None,
        });

        assert!(matches!(policy.launch, TerminalLaunch::Tui));
        assert_eq!(policy.palette_hint, PaletteHint::Plain);
    }

    #[test]
    fn colorfgbg_detects_light_background() {
        let policy = resolve_terminal_policy(TerminalSupportSnapshot {
            stdin_is_terminal: true,
            stdout_is_terminal: true,
            stderr_is_terminal: true,
            term: Some("xterm-256color".to_owned()),
            color_support: true,
            colorfgbg: Some("0;15".to_owned()),
        });

        assert_eq!(policy.palette_hint, PaletteHint::Light);
    }

    #[test]
    fn colorfgbg_detects_dark_background() {
        let policy = resolve_terminal_policy(TerminalSupportSnapshot {
            stdin_is_terminal: true,
            stdout_is_terminal: true,
            stderr_is_terminal: true,
            term: Some("xterm-256color".to_owned()),
            color_support: true,
            colorfgbg: Some("15;0".to_owned()),
        });

        assert_eq!(policy.palette_hint, PaletteHint::Dark);
    }

    #[test]
    fn colorfgbg_dark_gray_is_dark() {
        assert!(!detect_light_background(Some("15;8")));
    }

    #[test]
    fn colorfgbg_white_is_light() {
        assert!(detect_light_background(Some("0;7")));
    }

    #[test]
    fn colorfgbg_missing_is_dark() {
        assert!(!detect_light_background(None));
    }
}
