// TextInputWidget: single-line text input for the onboard wizard.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

#[allow(dead_code)] // consumed by later tasks (screen / runner)
pub(crate) struct TextInputState {
    value: String,
    default: Option<String>,
    default_active: bool,
    cursor: usize,
    error: Option<String>,
}

#[allow(dead_code)] // consumed by later tasks (screen / runner)
impl TextInputState {
    pub fn new() -> Self {
        Self {
            value: String::new(),
            default: None,
            default_active: false,
            cursor: 0,
            error: None,
        }
    }

    pub fn with_value(value: impl Into<String>) -> Self {
        let v: String = value.into();
        let len = v.len();
        Self {
            value: v,
            default: None,
            default_active: false,
            cursor: len,
            error: None,
        }
    }

    pub fn with_default(default: impl Into<String>) -> Self {
        Self {
            value: String::new(),
            default: Some(default.into()),
            default_active: true,
            cursor: 0,
            error: None,
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn display_value(&self) -> &str {
        if self.default_active {
            self.default.as_deref().unwrap_or("")
        } else {
            &self.value
        }
    }

    pub fn push(&mut self, c: char) {
        if self.default_active {
            self.default_active = false;
            self.value.clear();
        }
        self.value.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        self.error = None;
    }

    pub fn backspace(&mut self) {
        if self.default_active {
            self.default_active = false;
            self.value.clear();
            return;
        }
        if self.cursor > 0 {
            let prev = self.value[..self.cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.value.remove(prev);
            self.cursor = prev;
        }
        self.error = None;
    }

    pub fn submit_value(&self) -> &str {
        if self.default_active {
            self.default.as_deref().unwrap_or("")
        } else {
            &self.value
        }
    }

    pub fn set_error(&mut self, error: Option<String>) {
        self.error = error;
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

#[allow(dead_code)] // consumed by later tasks (screen / runner)
pub(crate) struct TextInputWidget {
    label: String,
}

#[allow(dead_code)] // consumed by later tasks (screen / runner)
impl TextInputWidget {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }

    pub fn render_with_state(&self, area: Rect, buf: &mut Buffer, state: &TextInputState) {
        if area.height < 1 {
            return;
        }
        let label_span = Span::styled(&self.label, Style::default().fg(Color::Gray));
        let value_style = if state.default_active {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        };
        let value_span = Span::styled(state.display_value(), value_style);
        let cursor_span = Span::styled("\u{258f}", Style::default().fg(Color::Cyan));
        let line = Line::from(vec![label_span, Span::raw(" "), value_span, cursor_span]);
        buf.set_line(area.x, area.y, &line, area.width);

        if let Some(error) = &state.error
            && area.height >= 2
        {
            let err_line = Line::from(Span::styled(
                format!("  \u{26a0} {error}"),
                Style::default().fg(Color::Red),
            ));
            buf.set_line(area.x, area.y + 1, &err_line, area.width);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_state_appends_characters() {
        let mut state = TextInputState::new();
        state.push('h');
        state.push('i');
        assert_eq!(state.value(), "hi");
    }

    #[test]
    fn input_state_backspace_removes_last_char() {
        let mut state = TextInputState::with_value("hello");
        state.backspace();
        assert_eq!(state.value(), "hell");
    }

    #[test]
    fn input_state_backspace_on_empty_is_noop() {
        let mut state = TextInputState::new();
        state.backspace();
        assert_eq!(state.value(), "");
    }

    #[test]
    fn input_state_clears_default_on_first_keystroke() {
        let mut state = TextInputState::with_default("placeholder");
        assert_eq!(state.display_value(), "placeholder");
        state.push('x');
        assert_eq!(state.value(), "x");
    }

    #[test]
    fn input_state_sets_validation_error() {
        let mut state = TextInputState::new();
        state.set_error(Some("path not writable".into()));
        assert_eq!(state.error(), Some("path not writable"));
    }
}
