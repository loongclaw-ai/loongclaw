use ratatui::style::Color;

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
