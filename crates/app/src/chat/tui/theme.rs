use ratatui::style::Color;

#[derive(Debug, Clone)]
pub(super) struct Palette {
    // Brand
    pub(super) brand: Color,
    pub(super) text: Color,
    // UI chrome
    pub(super) dim: Color,
    pub(super) separator: Color,
    pub(super) user_msg: Color,
    pub(super) think_block: Color,
    // Status
    pub(super) tool_running: Color,
    pub(super) tool_done: Color,
    pub(super) tool_fail: Color,
    pub(super) success: Color,
    pub(super) warning: Color,
    pub(super) error: Color,
    pub(super) info: Color,
}

impl Palette {
    pub(super) fn dark() -> Self {
        Self {
            brand: Color::Rgb(253, 172, 172),
            text: Color::Rgb(252, 245, 226),
            dim: Color::Rgb(170, 170, 170),
            separator: Color::Rgb(120, 120, 120),
            user_msg: Color::Rgb(100, 180, 255),
            think_block: Color::Rgb(176, 176, 196),
            tool_running: Color::Rgb(236, 196, 94),
            tool_done: Color::Rgb(120, 210, 132),
            tool_fail: Color::Rgb(236, 112, 112),
            success: Color::Rgb(120, 210, 132),
            warning: Color::Rgb(236, 196, 94),
            error: Color::Rgb(236, 112, 112),
            info: Color::Rgb(124, 214, 236),
        }
    }

    pub(super) fn light() -> Self {
        Self {
            brand: Color::Rgb(200, 60, 80),
            text: Color::Rgb(40, 40, 40),
            dim: Color::Rgb(92, 92, 92),
            separator: Color::Rgb(160, 160, 160),
            user_msg: Color::Rgb(30, 100, 200),
            think_block: Color::Rgb(72, 72, 96),
            tool_running: Color::Rgb(160, 120, 0),
            tool_done: Color::Rgb(30, 120, 30),
            tool_fail: Color::Rgb(180, 30, 30),
            success: Color::Rgb(30, 120, 30),
            warning: Color::Rgb(160, 100, 0),
            error: Color::Rgb(180, 30, 30),
            info: Color::Rgb(20, 120, 140),
        }
    }

    pub(super) fn plain() -> Self {
        Self {
            brand: Color::Reset,
            text: Color::Reset,
            dim: Color::Reset,
            separator: Color::Reset,
            user_msg: Color::Reset,
            think_block: Color::Reset,
            tool_running: Color::Reset,
            tool_done: Color::Reset,
            tool_fail: Color::Reset,
            success: Color::Reset,
            warning: Color::Reset,
            error: Color::Reset,
            info: Color::Reset,
        }
    }
}

// ---------------------------------------------------------------------------
// SemanticPalette: minimal legacy palette used by terminal.rs tests
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SemanticPalette {
    pub(crate) text: Color,
    pub(crate) border: Color,
    pub(crate) accent: Color,
    pub(crate) warning: Color,
    pub(crate) error: Color,
}

impl Default for SemanticPalette {
    fn default() -> Self {
        Self {
            text: Color::White,
            border: Color::DarkGray,
            accent: Color::Cyan,
            warning: Color::Yellow,
            error: Color::Red,
        }
    }
}

impl SemanticPalette {
    pub(crate) fn plain() -> Self {
        Self {
            text: Color::Reset,
            border: Color::Reset,
            accent: Color::Reset,
            warning: Color::Reset,
            error: Color::Reset,
        }
    }
}
