#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExplicitSkillActivationInput {
    pub(super) skill_id: String,
    pub(super) followup_request: String,
}

pub(super) fn parse_explicit_skill_activation_input(
    input: &str,
) -> Option<ExplicitSkillActivationInput> {
    let trimmed = input.trim_start();
    let raw_skill_token = trimmed.strip_prefix('$')?;
    let skill_token_len = raw_skill_token
        .char_indices()
        .take_while(|(_idx, ch)| explicit_skill_token_char(*ch))
        .last()
        .map_or(0, |(idx, ch)| idx + ch.len_utf8());
    if skill_token_len == 0 {
        return None;
    }

    let raw_skill_id = &raw_skill_token[..skill_token_len];
    let trailing = &raw_skill_token[skill_token_len..];
    if trailing
        .chars()
        .next()
        .is_some_and(|ch| !ch.is_whitespace())
    {
        return None;
    }

    let skill_id = normalize_explicit_skill_activation_id(raw_skill_id)?;
    let remaining_request = trailing.trim();
    let followup_request = if remaining_request.is_empty() {
        format!(
            "The user explicitly activated skill `{skill_id}` without an additional task. Confirm activation briefly and ask what to do next."
        )
    } else {
        remaining_request.to_owned()
    };

    Some(ExplicitSkillActivationInput {
        skill_id,
        followup_request,
    })
}

pub(super) fn parse_named_skill_activation_input(
    input: &str,
    visible_skill_ids: &[String],
) -> Option<ExplicitSkillActivationInput> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized_input = normalize_skill_activation_phrase(trimmed);
    let mut matched_skill_id = None;
    let mut matched_len = 0usize;

    for skill_id in visible_skill_ids {
        let normalized_skill_id = normalize_explicit_skill_activation_id(skill_id)?;
        let spaced_alias = normalized_skill_id.replace('-', " ");
        let mentions_skill = normalized_input.contains(format!("{spaced_alias} skill").as_str())
            || normalized_input.contains(format!("skill {spaced_alias}").as_str())
            || normalized_input.contains(normalized_skill_id.as_str());
        if !mentions_skill {
            continue;
        }
        let has_activation_verb = [
            "use ",
            "load ",
            "invoke ",
            "call ",
            "run ",
            "apply ",
            "activate ",
            "try ",
            "inspect ",
            "check ",
            "调用",
            "使用",
            "用",
            "加载",
            "启用",
            "看看",
            "查看",
        ]
        .iter()
        .any(|token| normalized_input.contains(token));
        if !has_activation_verb {
            continue;
        }
        if normalized_skill_id.len() > matched_len {
            matched_len = normalized_skill_id.len();
            matched_skill_id = Some(normalized_skill_id);
        }
    }

    let skill_id = matched_skill_id?;
    Some(ExplicitSkillActivationInput {
        skill_id,
        followup_request: trimmed.to_owned(),
    })
}

pub(super) fn explicit_skill_activation_tool_call_id(skill_id: &str) -> String {
    let normalized = normalize_explicit_skill_activation_id(skill_id)
        .unwrap_or_else(|| "external-skill".to_owned());
    format!("call-explicit-skill-activation-{normalized}")
}

fn explicit_skill_token_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.')
}

fn normalize_explicit_skill_activation_id(raw: &str) -> Option<String> {
    let mut normalized = String::new();
    let mut last_dash = false;
    for ch in raw.trim().chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            Some(ch.to_ascii_lowercase())
        } else if matches!(ch, '-' | '_' | ' ' | '.') {
            Some('-')
        } else {
            None
        };
        if let Some(value) = mapped {
            if value == '-' {
                if !last_dash {
                    normalized.push(value);
                }
                last_dash = true;
            } else {
                normalized.push(value);
                last_dash = false;
            }
        }
    }
    let normalized = normalized.trim_matches('-').to_owned();
    (!normalized.is_empty()).then_some(normalized)
}

fn normalize_skill_activation_phrase(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else if matches!(ch, '-' | '_' | '.' | '/' | '\\') {
                ' '
            } else {
                ch
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
