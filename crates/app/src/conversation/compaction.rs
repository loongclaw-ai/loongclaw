use crate::memory::WindowTurn;

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
        role: "assistant".to_owned(),
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
    turns
        .iter()
        .map(|turn| format!("{}: {}", turn.role, turn.content))
        .collect::<Vec<_>>()
        .join("\n")
}
