use crate::memory::WindowTurn;

const SUMMARY_MAX_RENDERED_TURNS: usize = 4;
const SUMMARY_TURN_EXCERPT_CHARS: usize = 96;
const PRIOR_COMPACTED_SUMMARY_PLACEHOLDER: &str = "[prior compacted summary]";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactPolicy {
    preserve_recent_turns: usize,
}

impl CompactPolicy {
    pub fn new(preserve_recent_turns: usize) -> Self {
        Self {
            preserve_recent_turns,
        }
    }
}

pub fn compact_window(turns: &[WindowTurn], policy: CompactPolicy) -> Option<Vec<WindowTurn>> {
    let preserve = policy.preserve_recent_turns.min(turns.len());
    if turns.len() <= preserve {
        return None;
    }

    let split_at = turns.len() - preserve;
    let (older, recent) = turns.split_at(split_at);

    let summary = WindowTurn {
        role: "user".to_owned(),
        content: format!(
            "Compacted {} earlier turns\n{}",
            older.len(),
            render_summary(older)
        ),
        ts: older.last().and_then(|turn| turn.ts),
    };

    let mut compacted = Vec::with_capacity(recent.len() + 1);
    compacted.push(summary);
    compacted.extend_from_slice(recent);
    Some(compacted)
}

fn render_summary(turns: &[WindowTurn]) -> String {
    let mut lines = turns
        .iter()
        .take(SUMMARY_MAX_RENDERED_TURNS)
        .map(render_summary_line)
        .collect::<Vec<_>>();
    let omitted_turns = turns.len().saturating_sub(lines.len());
    if omitted_turns > 0 {
        lines.push(format!("... {} earlier turns omitted", omitted_turns));
    }
    lines.join("\n")
}

fn render_summary_line(turn: &WindowTurn) -> String {
    format!("{}: {}", turn.role, summarize_turn_content(&turn.content))
}

fn summarize_turn_content(content: &str) -> String {
    if content.trim_start().starts_with("Compacted ") {
        return PRIOR_COMPACTED_SUMMARY_PLACEHOLDER.to_owned();
    }

    let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
    trim_to_chars(&normalized, SUMMARY_TURN_EXCERPT_CHARS)
}

fn trim_to_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }

    if max_chars <= 3 {
        return value.chars().take(max_chars).collect();
    }

    let mut trimmed = value.chars().take(max_chars - 3).collect::<String>();
    trimmed.push_str("...");
    trimmed
}
