use ratatui::text::Line;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TranscriptRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TranscriptEntry {
    pub(crate) role: TranscriptRole,
    pub(crate) text: String,
    pub(crate) streaming: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TranscriptState {
    entries: Vec<TranscriptEntry>,
}

impl TranscriptState {
    #[cfg(test)]
    pub(crate) fn entries(&self) -> &[TranscriptEntry] {
        &self.entries
    }

    pub(crate) fn push_message(&mut self, role: TranscriptRole, text: impl Into<String>) {
        self.entries.push(TranscriptEntry {
            role,
            text: text.into(),
            streaming: false,
        });
    }

    pub(crate) fn update_assistant_stream(&mut self, text: impl Into<String>) {
        let text = text.into();

        match self.entries.last_mut() {
            Some(entry) if entry.role == TranscriptRole::Assistant && entry.streaming => {
                entry.text = text;
            }
            _ => self.entries.push(TranscriptEntry {
                role: TranscriptRole::Assistant,
                text,
                streaming: true,
            }),
        }
    }

    pub(crate) fn finalize_assistant_message(&mut self, text: impl Into<String>) {
        let text = text.into();

        match self.entries.last_mut() {
            Some(entry) if entry.role == TranscriptRole::Assistant && entry.streaming => {
                entry.text = text;
                entry.streaming = false;
            }
            _ => self.entries.push(TranscriptEntry {
                role: TranscriptRole::Assistant,
                text,
                streaming: false,
            }),
        }
    }
}

pub(crate) fn render_transcript_lines(state: &TranscriptState) -> Vec<Line<'static>> {
    if state.entries.is_empty() {
        return vec![Line::from("assistant> TUI shell bootstrap ready.")];
    }

    state
        .entries
        .iter()
        .map(|entry| {
            let role = match entry.role {
                TranscriptRole::User => "you",
                TranscriptRole::Assistant => "assistant",
            };
            let streaming_suffix = if entry.streaming { " (streaming)" } else { "" };
            Line::from(format!("{role}{streaming_suffix}> {}", entry.text))
        })
        .collect()
}
