#![allow(dead_code)]

use crate::channel::traits::{MessageContent, Pagination, TimeRange};

/// Parse MessageContent from JSON parameters
pub fn parse_message_content(params: &serde_json::Value) -> Result<MessageContent, String> {
    let mut content = MessageContent::default();

    if let Some(text) = params["text"].as_str() {
        content.text = Some(text.to_string());
    }

    if let Some(html) = params["html"].as_str() {
        content.html = Some(html.to_string());
    }

    if let Some(markdown) = params["markdown"].as_str() {
        content.markdown = Some(markdown.to_string());
    }

    if let Some(image_key) = params["image_key"].as_str() {
        content.image_key = Some(image_key.to_string());
    }

    if let Some(card) = params["card"].as_object() {
        content.card = Some(serde_json::Value::Object(card.clone()));
    }

    if content.text.is_none()
        && content.html.is_none()
        && content.markdown.is_none()
        && content.image_key.is_none()
        && content.card.is_none()
    {
        return Err("Message must have text, html, markdown, image_key, or card".to_string());
    }

    Ok(content)
}

/// Parse pagination from JSON parameters
pub fn parse_pagination(params: &serde_json::Value) -> Pagination {
    Pagination {
        page_size: params["limit"].as_u64().map(|v| v as u32).unwrap_or(50),
        cursor: params["cursor"].as_str().map(|s| s.to_string()),
        page: params["page"].as_u64().map(|v| v as u32),
    }
}

/// Parse TimeRange from JSON parameters
pub fn parse_time_range(params: &serde_json::Value) -> Result<TimeRange, String> {
    let start_str = params["start_time"].as_str().ok_or("Missing start_time")?;
    let end_str = params["end_time"].as_str().ok_or("Missing end_time")?;

    let start = chrono::DateTime::parse_from_rfc3339(start_str)
        .map_err(|e| format!("Invalid start_time: {}", e))?
        .with_timezone(&chrono::Utc);
    let end = chrono::DateTime::parse_from_rfc3339(end_str)
        .map_err(|e| format!("Invalid end_time: {}", e))?
        .with_timezone(&chrono::Utc);

    Ok(TimeRange { start, end })
}
