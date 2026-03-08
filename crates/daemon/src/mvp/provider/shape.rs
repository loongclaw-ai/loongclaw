use serde_json::Value;

pub(super) fn extract_message_content(body: &Value) -> Option<String> {
    let content = body
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))?;

    if let Some(text) = content.as_str() {
        return normalize_text(text);
    }

    let parts = content.as_array()?;
    let mut merged = Vec::new();
    for part in parts {
        if let Some(text) = extract_content_part_text(part) {
            merged.push(text);
        }
    }
    if merged.is_empty() {
        return None;
    }
    normalize_text(&merged.join("\n"))
}

fn extract_content_part_text(part: &Value) -> Option<String> {
    if let Some(text) = part.get("text").and_then(Value::as_str) {
        return normalize_text(text);
    }
    if let Some(text) = part
        .get("text")
        .and_then(|value| value.get("value"))
        .and_then(Value::as_str)
    {
        return normalize_text(text);
    }
    None
}

fn normalize_text(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn extract_message_content_supports_part_array_shape() {
        let body = json!({
            "choices": [{
                "message": {
                    "content": [
                        {"type": "text", "text": "line1"},
                        {"type": "text", "text": {"value": "line2"}}
                    ]
                }
            }]
        });
        let content = extract_message_content(&body).expect("content");
        assert_eq!(content, "line1\nline2");
    }

    #[test]
    fn extract_message_content_keeps_plain_string_shape() {
        let body = json!({
            "choices": [{
                "message": {
                    "content": "  hello world  "
                }
            }]
        });
        let content = extract_message_content(&body).expect("content");
        assert_eq!(content, "hello world");
    }

    #[test]
    fn extract_message_content_ignores_empty_parts() {
        let body = json!({
            "choices": [{
                "message": {
                    "content": [
                        {"type": "text", "text": "   "},
                        {"type": "text", "text": {"value": ""}}
                    ]
                }
            }]
        });
        assert!(extract_message_content(&body).is_none());
    }
}
