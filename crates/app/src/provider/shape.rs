use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::conversation::turn_engine::{ProviderTurn, ToolIntent};
use crate::tools;

pub fn extract_provider_turn(body: &Value) -> Option<ProviderTurn> {
    let message = body
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))?;

    let mut assistant_text = message_content(message).unwrap_or_default();
    let mut raw_meta = message.clone();

    let mut tool_intents: Vec<ToolIntent> = message
        .get("tool_calls")
        .and_then(Value::as_array)
        .map(|calls| {
            calls
                .iter()
                .filter_map(|call| {
                    let function = call.get("function")?;
                    let raw_tool_name = function.get("name").and_then(Value::as_str)?;
                    let tool_name = tools::canonical_tool_name(raw_tool_name).to_owned();
                    let args_str = function
                        .get("arguments")
                        .and_then(Value::as_str)
                        .unwrap_or("{}");
                    let args_json = match serde_json::from_str::<Value>(args_str) {
                        Ok(value) => value,
                        Err(e) => json!({
                            "_parse_error": format!("{e}"),
                            "_raw_arguments": args_str
                        }),
                    };
                    let tool_call_id = call
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned();
                    Some(ToolIntent {
                        tool_name,
                        args_json,
                        source: "provider_tool_call".to_owned(),
                        session_id: String::new(),
                        turn_id: String::new(),
                        tool_call_id,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    if tool_intents.is_empty() {
        match extract_inline_function_call_turn(assistant_text.as_str()) {
            InlineFunctionParseResult::Parsed {
                cleaned_text,
                tool_intents: inline_tool_intents,
                telemetry,
            } => {
                assistant_text = cleaned_text;
                tool_intents = inline_tool_intents;
                attach_inline_function_parse_telemetry(&mut raw_meta, telemetry);
            }
            InlineFunctionParseResult::Malformed { telemetry } => {
                attach_inline_function_parse_telemetry(&mut raw_meta, telemetry);
            }
            InlineFunctionParseResult::Absent => {}
        }
    }

    Some(ProviderTurn {
        assistant_text,
        tool_intents,
        raw_meta,
    })
}

pub(super) fn extract_message_content(body: &Value) -> Option<String> {
    let content = body
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(message_content_value)?;

    extract_content_text(content)
}

fn message_content(message: &Value) -> Option<String> {
    let content = message_content_value(message)?;
    extract_content_text(content)
}

fn message_content_value(message: &Value) -> Option<&Value> {
    message.get("content")
}

fn extract_content_text(content: &Value) -> Option<String> {
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct InlineFunctionParseTelemetry {
    status: &'static str,
    tool_count: usize,
    error_code: Option<&'static str>,
}

impl InlineFunctionParseTelemetry {
    fn parsed(tool_count: usize) -> Self {
        Self {
            status: "parsed",
            tool_count,
            error_code: None,
        }
    }

    fn malformed(tool_count: usize, error_code: InlineFunctionParseError) -> Self {
        Self {
            status: "malformed",
            tool_count,
            error_code: Some(error_code.as_str()),
        }
    }
}

#[derive(Debug, Clone)]
enum InlineFunctionParseResult {
    Parsed {
        cleaned_text: String,
        tool_intents: Vec<ToolIntent>,
        telemetry: InlineFunctionParseTelemetry,
    },
    Malformed {
        telemetry: InlineFunctionParseTelemetry,
    },
    Absent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlineFunctionParseError {
    MissingFunctionHeaderClose,
    EmptyFunctionName,
    MissingFunctionClose,
    MissingParameterOpen,
    MissingParameterHeaderClose,
    EmptyParameterName,
    MissingParameterClose,
}

impl InlineFunctionParseError {
    fn as_str(self) -> &'static str {
        match self {
            Self::MissingFunctionHeaderClose => "missing_function_header_close",
            Self::EmptyFunctionName => "empty_function_name",
            Self::MissingFunctionClose => "missing_function_close",
            Self::MissingParameterOpen => "missing_parameter_open",
            Self::MissingParameterHeaderClose => "missing_parameter_header_close",
            Self::EmptyParameterName => "empty_parameter_name",
            Self::MissingParameterClose => "missing_parameter_close",
        }
    }
}

fn attach_inline_function_parse_telemetry(
    raw_meta: &mut Value,
    telemetry: InlineFunctionParseTelemetry,
) {
    let Some(message) = raw_meta.as_object_mut() else {
        return;
    };

    let mut inline_function = serde_json::Map::new();
    inline_function.insert(
        "status".to_owned(),
        Value::String(telemetry.status.to_owned()),
    );
    inline_function.insert(
        "tool_count".to_owned(),
        Value::from(telemetry.tool_count as u64),
    );
    if let Some(error_code) = telemetry.error_code {
        inline_function.insert(
            "error_code".to_owned(),
            Value::String(error_code.to_owned()),
        );
    }

    let mut provider_parse = serde_json::Map::new();
    provider_parse.insert("inline_function".to_owned(), Value::Object(inline_function));
    message.insert(
        "loongclaw_provider_parse".to_owned(),
        Value::Object(provider_parse),
    );
}

fn extract_inline_function_call_turn(text: &str) -> InlineFunctionParseResult {
    const FUNCTION_OPEN: &str = "<function=";
    const FUNCTION_CLOSE: &str = "</function>";

    let mut cursor = 0usize;
    let mut cleaned = String::new();
    let mut tool_intents = Vec::new();
    let mut found_inline_function = false;

    while let Some(relative_start) = text[cursor..].find(FUNCTION_OPEN) {
        found_inline_function = true;
        let start = cursor + relative_start;
        cleaned.push_str(&text[cursor..start]);

        let name_start = start + FUNCTION_OPEN.len();
        let header_remainder = &text[name_start..];
        let Some(header_end) = header_remainder.find('>') else {
            return InlineFunctionParseResult::Malformed {
                telemetry: InlineFunctionParseTelemetry::malformed(
                    tool_intents.len(),
                    InlineFunctionParseError::MissingFunctionHeaderClose,
                ),
            };
        };
        let raw_tool_name = header_remainder[..header_end].trim();
        if raw_tool_name.is_empty() {
            return InlineFunctionParseResult::Malformed {
                telemetry: InlineFunctionParseTelemetry::malformed(
                    tool_intents.len(),
                    InlineFunctionParseError::EmptyFunctionName,
                ),
            };
        }

        let body_start = name_start + header_end + 1;
        let body_remainder = &text[body_start..];
        let Some(body_end) = body_remainder.find(FUNCTION_CLOSE) else {
            return InlineFunctionParseResult::Malformed {
                telemetry: InlineFunctionParseTelemetry::malformed(
                    tool_intents.len(),
                    InlineFunctionParseError::MissingFunctionClose,
                ),
            };
        };
        let function_body = &body_remainder[..body_end];
        let args_json = match parse_inline_function_parameters(function_body) {
            Ok(args_json) => args_json,
            Err(error_code) => {
                return InlineFunctionParseResult::Malformed {
                    telemetry: InlineFunctionParseTelemetry::malformed(
                        tool_intents.len(),
                        error_code,
                    ),
                };
            }
        };

        tool_intents.push(ToolIntent {
            tool_name: tools::canonical_tool_name(raw_tool_name).to_owned(),
            args_json,
            source: "provider_inline_function_call".to_owned(),
            session_id: String::new(),
            turn_id: String::new(),
            tool_call_id: format!("inline-call-{}", tool_intents.len()),
        });

        cursor = body_start + body_end + FUNCTION_CLOSE.len();
    }

    if !found_inline_function {
        return InlineFunctionParseResult::Absent;
    }

    cleaned.push_str(&text[cursor..]);
    let telemetry = InlineFunctionParseTelemetry::parsed(tool_intents.len());
    InlineFunctionParseResult::Parsed {
        cleaned_text: normalize_text(cleaned.as_str()).unwrap_or_default(),
        tool_intents,
        telemetry,
    }
}

fn parse_inline_function_parameters(body: &str) -> Result<Value, InlineFunctionParseError> {
    const PARAMETER_OPEN: &str = "<parameter=";
    const PARAMETER_CLOSE: &str = "</parameter>";

    let mut cursor = 0usize;
    let mut payload = serde_json::Map::new();

    while cursor < body.len() {
        let remainder = &body[cursor..];
        let trimmed_len = remainder.len().saturating_sub(remainder.trim_start().len());
        cursor += trimmed_len;
        if cursor >= body.len() {
            break;
        }

        let remainder = &body[cursor..];
        if !remainder.starts_with(PARAMETER_OPEN) {
            return Err(InlineFunctionParseError::MissingParameterOpen);
        }

        let name_start = cursor + PARAMETER_OPEN.len();
        let name_remainder = &body[name_start..];
        let Some(name_end) = name_remainder.find('>') else {
            return Err(InlineFunctionParseError::MissingParameterHeaderClose);
        };
        let parameter_name = name_remainder[..name_end].trim();
        if parameter_name.is_empty() {
            return Err(InlineFunctionParseError::EmptyParameterName);
        }

        let value_start = name_start + name_end + 1;
        let value_remainder = &body[value_start..];
        let Some(value_end) = value_remainder.find(PARAMETER_CLOSE) else {
            return Err(InlineFunctionParseError::MissingParameterClose);
        };
        let raw_value = &value_remainder[..value_end];
        payload.insert(
            parameter_name.to_owned(),
            parse_inline_parameter_value(raw_value),
        );

        cursor = value_start + value_end + PARAMETER_CLOSE.len();
    }

    Ok(Value::Object(payload))
}

fn parse_inline_parameter_value(raw_value: &str) -> Value {
    let decoded = decode_inline_xml_text(raw_value);
    let trimmed = decoded.trim();
    if trimmed.is_empty() {
        return Value::String(String::new());
    }
    serde_json::from_str::<Value>(trimmed).unwrap_or_else(|_| Value::String(trimmed.to_owned()))
}

fn decode_inline_xml_text(raw: &str) -> String {
    raw.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModelCandidate {
    id: String,
    created: Option<i64>,
}

pub(super) fn extract_model_ids(body: &Value) -> Vec<String> {
    let mut candidates = collect_model_candidates(body);
    if candidates.is_empty() {
        return Vec::new();
    }

    candidates.sort_by(|left, right| {
        right
            .created
            .cmp(&left.created)
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut seen = BTreeSet::new();
    let mut ids = Vec::new();
    for candidate in candidates {
        if seen.insert(candidate.id.clone()) {
            ids.push(candidate.id);
        }
    }
    ids
}

fn collect_model_candidates(body: &Value) -> Vec<ModelCandidate> {
    let mut out = Vec::new();
    let Some(items) = model_items(body) else {
        return out;
    };

    for item in items {
        if let Some(id) = model_id_from_value(item) {
            out.push(ModelCandidate {
                id,
                created: model_created_from_value(item),
            });
        }
    }
    out
}

fn model_items(body: &Value) -> Option<&[Value]> {
    if let Some(data) = body.get("data").and_then(Value::as_array) {
        return Some(data);
    }
    if let Some(models) = body.get("models").and_then(Value::as_array) {
        return Some(models);
    }
    if let Some(models) = body
        .get("result")
        .and_then(|value| value.get("models"))
        .and_then(Value::as_array)
    {
        return Some(models);
    }
    body.as_array().map(Vec::as_slice)
}

fn model_id_from_value(value: &Value) -> Option<String> {
    if let Some(id) = value.as_str() {
        return normalize_text(id);
    }
    if let Some(id) = value.get("id").and_then(Value::as_str) {
        return normalize_text(id);
    }
    if let Some(id) = value.get("model").and_then(Value::as_str) {
        return normalize_text(id);
    }
    if let Some(id) = value.get("name").and_then(Value::as_str) {
        return normalize_text(id);
    }
    None
}

fn model_created_from_value(value: &Value) -> Option<i64> {
    if let Some(created) = value.get("created").and_then(Value::as_i64) {
        return Some(created);
    }
    if let Some(created) = value.get("created").and_then(Value::as_u64) {
        return i64::try_from(created).ok();
    }
    if let Some(created) = value.get("created_at").and_then(Value::as_i64) {
        return Some(created);
    }
    if let Some(created) = value.get("created_at").and_then(Value::as_u64) {
        return i64::try_from(created).ok();
    }
    None
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn extract_provider_turn_parses_tool_calls() {
        let body = serde_json::json!({
            "choices": [{
                "message": {
                    "content": "checking",
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "file.read",
                            "arguments": "{\"path\":\"README.md\"}"
                        }
                    }]
                }
            }]
        });
        let turn = extract_provider_turn(&body).expect("turn");
        assert_eq!(turn.assistant_text, "checking");
        assert_eq!(turn.tool_intents.len(), 1);
        assert_eq!(turn.tool_intents[0].tool_name, "file.read");
        assert_eq!(turn.tool_intents[0].tool_call_id, "call_1");
    }

    #[test]
    fn extract_provider_turn_surfaces_malformed_json_args() {
        let body = serde_json::json!({
            "choices": [{
                "message": {
                    "content": "calling",
                    "tool_calls": [{
                        "id": "call_bad",
                        "type": "function",
                        "function": {
                            "name": "file.read",
                            "arguments": "{{not valid json"
                        }
                    }]
                }
            }]
        });
        let turn = extract_provider_turn(&body).expect("turn");
        assert_eq!(turn.tool_intents.len(), 1);
        let args = &turn.tool_intents[0].args_json;
        assert!(
            args.get("_parse_error").is_some(),
            "malformed args should surface parse error, got: {args}"
        );
        assert_eq!(
            args.get("_raw_arguments").and_then(|v| v.as_str()),
            Some("{{not valid json")
        );
    }

    #[test]
    fn extract_provider_turn_normalizes_underscore_tool_aliases() {
        let body = serde_json::json!({
            "choices": [{
                "message": {
                    "content": "calling",
                    "tool_calls": [{
                        "id": "call_underscore",
                        "type": "function",
                        "function": {
                            "name": "file_read",
                            "arguments": "{\"path\":\"README.md\"}"
                        }
                    }]
                }
            }]
        });
        let turn = extract_provider_turn(&body).expect("turn");
        assert_eq!(turn.tool_intents.len(), 1);
        assert_eq!(turn.tool_intents[0].tool_name, "file.read");
    }

    #[test]
    fn extract_provider_turn_handles_text_only() {
        let body = serde_json::json!({
            "choices": [{
                "message": {
                    "content": "hello world"
                }
            }]
        });
        let turn = extract_provider_turn(&body).expect("turn");
        assert_eq!(turn.assistant_text, "hello world");
        assert!(turn.tool_intents.is_empty());
    }

    #[test]
    fn extract_provider_turn_parses_inline_shell_function_block() {
        let body = serde_json::json!({
            "choices": [{
                "message": {
                    "content": "抱歉，刚才的命令执行失败了。让我用更简单的方式重试:\n<function=shell.exec><parameter=command>ls /root</parameter></function>"
                }
            }]
        });

        let turn = extract_provider_turn(&body).expect("turn");
        assert_eq!(
            turn.assistant_text,
            "抱歉，刚才的命令执行失败了。让我用更简单的方式重试:"
        );
        assert_eq!(turn.tool_intents.len(), 1);
        assert_eq!(turn.tool_intents[0].tool_name, "shell.exec");
        assert_eq!(
            turn.tool_intents[0].args_json,
            json!({"command":"ls /root"})
        );
        assert_eq!(
            turn.raw_meta["loongclaw_provider_parse"]["inline_function"]["status"],
            "parsed"
        );
        assert_eq!(
            turn.raw_meta["loongclaw_provider_parse"]["inline_function"]["tool_count"],
            1
        );
    }

    #[test]
    fn extract_provider_turn_parses_inline_external_skill_function_block() {
        let body = serde_json::json!({
            "choices": [{
                "message": {
                    "content": "我看到已经安装了 Home Assistant 技能。让我调用它来获取所有实体状态。\n<function=external_skills.invoke><parameter=skill_id>home-assistant-1-0-0</parameter><parameter=action>get_states</parameter></function>"
                }
            }]
        });

        let turn = extract_provider_turn(&body).expect("turn");
        assert_eq!(
            turn.assistant_text,
            "我看到已经安装了 Home Assistant 技能。让我调用它来获取所有实体状态。"
        );
        assert_eq!(turn.tool_intents.len(), 1);
        assert_eq!(turn.tool_intents[0].tool_name, "external_skills.invoke");
        assert_eq!(
            turn.tool_intents[0].args_json,
            json!({"skill_id":"home-assistant-1-0-0","action":"get_states"})
        );
    }

    #[test]
    fn extract_provider_turn_recovers_inline_parameter_json_types() {
        let body = serde_json::json!({
            "choices": [{
                "message": {
                    "content": "让我按结构化参数重试。\n<function=shell.exec><parameter=command>\"echo\"</parameter><parameter=args>[\"hello\",\"world\"]</parameter><parameter=timeout_ms>3000</parameter><parameter=login>false</parameter></function>"
                }
            }]
        });

        let turn = extract_provider_turn(&body).expect("turn");
        assert_eq!(turn.tool_intents.len(), 1);
        assert_eq!(
            turn.tool_intents[0].args_json,
            json!({
                "command": "echo",
                "args": ["hello", "world"],
                "timeout_ms": 3000,
                "login": false
            })
        );
    }

    #[test]
    fn extract_provider_turn_records_malformed_inline_function_telemetry() {
        let body = serde_json::json!({
            "choices": [{
                "message": {
                    "content": "让我重试。\n<function=shell.exec><parameter=command>ls /root</parameter>"
                }
            }]
        });

        let turn = extract_provider_turn(&body).expect("turn");
        assert_eq!(
            turn.assistant_text,
            "让我重试。\n<function=shell.exec><parameter=command>ls /root</parameter>"
        );
        assert!(turn.tool_intents.is_empty());
        assert_eq!(
            turn.raw_meta["loongclaw_provider_parse"]["inline_function"]["status"],
            "malformed"
        );
        assert_eq!(
            turn.raw_meta["loongclaw_provider_parse"]["inline_function"]["error_code"],
            "missing_function_close"
        );
    }

    #[test]
    fn extract_provider_turn_supports_array_content_shape() {
        let body = serde_json::json!({
            "choices": [{
                "message": {
                    "content": [
                        {"type": "text", "text": "line1"},
                        {"type": "text", "text": {"value": "line2"}}
                    ]
                }
            }]
        });
        let turn = extract_provider_turn(&body).expect("turn");
        assert_eq!(turn.assistant_text, "line1\nline2");
        assert!(turn.tool_intents.is_empty());
    }

    #[test]
    fn extract_provider_turn_preserves_reasoning_content_in_raw_meta() {
        let body = serde_json::json!({
            "choices": [{
                "message": {
                    "content": "done",
                    "reasoning_content": "thinking"
                }
            }]
        });
        let turn = extract_provider_turn(&body).expect("turn");
        assert_eq!(turn.assistant_text, "done");
        assert_eq!(turn.raw_meta["reasoning_content"], "thinking");
    }

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

    #[test]
    fn extract_model_ids_prefers_newer_timestamp_when_available() {
        let body = json!({
            "data": [
                {"id": "model-v1", "created": 100},
                {"id": "model-v2", "created": 200}
            ]
        });
        let ids = extract_model_ids(&body);
        assert_eq!(ids, vec!["model-v2", "model-v1"]);
    }

    #[test]
    fn extract_model_ids_supports_models_array_and_strings() {
        let body = json!({
            "models": [
                "model-c",
                {"name": "model-b"},
                {"model": "model-a"}
            ]
        });
        let ids = extract_model_ids(&body);
        assert_eq!(ids, vec!["model-a", "model-b", "model-c"]);
    }

    #[test]
    fn extract_model_ids_deduplicates_results() {
        let body = json!({
            "data": [
                {"id": "model-a", "created": 200},
                {"id": "model-a", "created": 100},
                {"id": "model-b", "created": 150}
            ]
        });
        let ids = extract_model_ids(&body);
        assert_eq!(ids, vec!["model-a", "model-b"]);
    }
}
