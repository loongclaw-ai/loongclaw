use std::collections::BTreeSet;

use aes::Aes256;
use base64::Engine;
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::CliResult;

#[derive(Debug, Clone)]
pub(super) struct FeishuInboundEvent {
    pub(super) event_id: String,
    pub(super) session_id: String,
    pub(super) message_id: String,
    pub(super) text: String,
}

#[derive(Debug)]
pub(super) enum FeishuWebhookAction {
    UrlVerification { challenge: String },
    Ignore,
    Inbound(FeishuInboundEvent),
}

pub(super) fn build_feishu_send_payload(
    receive_id: &str,
    msg_type: &str,
    content: Value,
) -> CliResult<Value> {
    let receive_id = receive_id.trim();
    if receive_id.is_empty() {
        return Err("feishu receive_id is empty".to_owned());
    }

    let msg_type = msg_type.trim();
    if msg_type.is_empty() {
        return Err("feishu msg_type is empty".to_owned());
    }

    Ok(json!({
        "receive_id": receive_id,
        "msg_type": msg_type,
        "content": encode_feishu_content(&content)?,
    }))
}

pub(super) fn build_feishu_reply_payload(msg_type: &str, content: Value) -> CliResult<Value> {
    let msg_type = msg_type.trim();
    if msg_type.is_empty() {
        return Err("feishu reply msg_type is empty".to_owned());
    }

    Ok(json!({
        "msg_type": msg_type,
        "content": encode_feishu_content(&content)?,
    }))
}

fn encode_feishu_content(content: &Value) -> CliResult<String> {
    serde_json::to_string(content).map_err(|error| format!("feishu content encode failed: {error}"))
}

pub(super) fn ensure_feishu_response_ok(action: &str, payload: &Value) -> CliResult<()> {
    let code = payload.get("code").and_then(Value::as_i64).unwrap_or(-1);
    if code != 0 {
        return Err(format!("{action} returned code {code}: {payload}"));
    }
    Ok(())
}

pub(super) fn parse_feishu_webhook_payload(
    payload: &Value,
    verification_token: Option<&str>,
    encrypt_key: Option<&str>,
    allowed_chat_ids: &BTreeSet<String>,
    ignore_bot_messages: bool,
) -> CliResult<FeishuWebhookAction> {
    let decrypted_payload = decrypt_payload_if_needed(payload, encrypt_key)?;
    let payload = decrypted_payload.as_ref().unwrap_or(payload);

    if payload.get("type").and_then(Value::as_str) == Some("url_verification") {
        verify_feishu_token(payload, verification_token)?;
        let challenge = payload
            .get("challenge")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "feishu url_verification payload missing challenge".to_owned())?;
        return Ok(FeishuWebhookAction::UrlVerification {
            challenge: challenge.to_owned(),
        });
    }

    let event_type = payload
        .get("header")
        .and_then(|header| header.get("event_type"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if event_type != "im.message.receive_v1" {
        return Ok(FeishuWebhookAction::Ignore);
    }

    verify_feishu_token(payload, verification_token)?;

    let event = payload
        .get("event")
        .and_then(Value::as_object)
        .ok_or_else(|| "feishu message event payload missing event object".to_owned())?;

    let sender_type = event
        .get("sender")
        .and_then(|sender| sender.get("sender_type"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if ignore_bot_messages && matches!(sender_type, "app" | "bot") {
        return Ok(FeishuWebhookAction::Ignore);
    }

    let message = event
        .get("message")
        .and_then(Value::as_object)
        .ok_or_else(|| "feishu message event payload missing message object".to_owned())?;

    let message_type = message
        .get("message_type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if message_type != "text" {
        return Ok(FeishuWebhookAction::Ignore);
    }

    let chat_id = message
        .get("chat_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "feishu message event missing message.chat_id".to_owned())?;
    if !allowed_chat_ids.is_empty() && !allowed_chat_ids.contains(chat_id) {
        return Ok(FeishuWebhookAction::Ignore);
    }

    let message_id = message
        .get("message_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "feishu message event missing message.message_id".to_owned())?;

    let content = message
        .get("content")
        .ok_or_else(|| "feishu message event missing message.content".to_owned())?;
    let text = parse_feishu_text_content(content)
        .ok_or_else(|| "feishu message content is not a non-empty text payload".to_owned())?;

    let event_id = payload
        .get("header")
        .and_then(|header| header.get("event_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("message:{message_id}"));

    Ok(FeishuWebhookAction::Inbound(FeishuInboundEvent {
        event_id,
        session_id: format!("feishu:{chat_id}"),
        message_id: message_id.to_owned(),
        text,
    }))
}

fn decrypt_payload_if_needed(
    payload: &Value,
    encrypt_key: Option<&str>,
) -> CliResult<Option<Value>> {
    let Some(encrypted_payload) = payload.get("encrypt").and_then(Value::as_str) else {
        return Ok(None);
    };

    let Some(encrypt_key) = encrypt_key.map(str::trim).filter(|value| !value.is_empty()) else {
        return Err(
            "unauthorized: feishu payload is encrypted but encrypt key is not configured"
                .to_owned(),
        );
    };

    decrypt_feishu_event_payload(encrypted_payload, encrypt_key).map(Some)
}

fn decrypt_feishu_event_payload(encrypted_payload: &str, encrypt_key: &str) -> CliResult<Value> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(encrypted_payload.trim())
        .map_err(|error| format!("decode encrypted feishu payload failed: {error}"))?;
    if raw.len() < 16 {
        return Err("decode encrypted feishu payload failed: payload too short".to_owned());
    }

    let iv = &raw[..16];
    let mut cipher_text = raw[16..].to_vec();
    if cipher_text.is_empty() {
        return Err("decode encrypted feishu payload failed: ciphertext is empty".to_owned());
    }

    let key = Sha256::digest(encrypt_key.as_bytes());
    let decrypted = cbc::Decryptor::<Aes256>::new_from_slices(&key, iv)
        .map_err(|error| format!("initialize feishu decryptor failed: {error}"))?
        .decrypt_padded_mut::<Pkcs7>(&mut cipher_text)
        .map_err(|error| format!("decrypt feishu payload failed: {error}"))?;

    serde_json::from_slice::<Value>(decrypted)
        .map_err(|error| format!("parse decrypted feishu payload failed: {error}"))
}

fn verify_feishu_token(payload: &Value, verification_token: Option<&str>) -> CliResult<()> {
    let Some(expected_token) = verification_token
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };

    let incoming = payload
        .get("token")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if incoming.is_empty() {
        return Err("unauthorized: feishu payload missing token".to_owned());
    }
    if incoming != expected_token {
        return Err("unauthorized: feishu verification token mismatch".to_owned());
    }
    Ok(())
}

fn parse_feishu_text_content(content: &Value) -> Option<String> {
    match content {
        Value::String(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return None;
            }
            if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                return parse_feishu_text_content(&parsed);
            }
            Some(trimmed.to_owned())
        }
        Value::Object(map) => map
            .get("text")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned),
        _ => None,
    }
}

pub(super) fn normalize_webhook_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return "/feishu/events".to_owned();
    }
    if trimmed.starts_with('/') {
        return trimmed.to_owned();
    }
    format!("/{trimmed}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feishu_url_verification_payload_parses() {
        let payload = json!({
            "type": "url_verification",
            "token": "token-123",
            "challenge": "abc"
        });
        let action =
            parse_feishu_webhook_payload(&payload, Some("token-123"), None, &BTreeSet::new(), true)
                .expect("parse feishu url verification");

        match action {
            FeishuWebhookAction::UrlVerification { challenge } => assert_eq!(challenge, "abc"),
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn feishu_message_event_parses_text_payload() {
        let payload = json!({
            "token": "token-123",
            "header": {
                "event_id": "evt_1",
                "event_type": "im.message.receive_v1"
            },
            "event": {
                "sender": {
                    "sender_type": "user"
                },
                "message": {
                    "chat_id": "oc_123",
                    "message_id": "om_123",
                    "message_type": "text",
                    "content": "{\"text\":\"hello loongclaw\"}"
                }
            }
        });

        let action =
            parse_feishu_webhook_payload(&payload, Some("token-123"), None, &BTreeSet::new(), true)
                .expect("parse feishu event");

        match action {
            FeishuWebhookAction::Inbound(event) => {
                assert_eq!(event.event_id, "evt_1");
                assert_eq!(event.session_id, "feishu:oc_123");
                assert_eq!(event.message_id, "om_123");
                assert_eq!(event.text, "hello loongclaw");
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn feishu_send_payload_serializes_content() {
        let payload = build_feishu_send_payload("oc_1", "text", json!({"text": "hi"}))
            .expect("build feishu send payload");
        assert_eq!(payload["receive_id"], "oc_1");
        assert_eq!(payload["msg_type"], "text");
        assert_eq!(payload["content"], "{\"text\":\"hi\"}");
    }

    #[test]
    fn feishu_token_mismatch_is_rejected() {
        let payload = json!({
            "type": "url_verification",
            "token": "token-x",
            "challenge": "abc"
        });
        let error =
            parse_feishu_webhook_payload(&payload, Some("token-y"), None, &BTreeSet::new(), true)
                .expect_err("token mismatch should fail");
        assert!(error.contains("unauthorized"));
    }

    #[test]
    fn feishu_non_text_message_is_ignored() {
        let payload = json!({
            "token": "token-123",
            "header": {
                "event_type": "im.message.receive_v1"
            },
            "event": {
                "sender": {"sender_type": "user"},
                "message": {
                    "chat_id": "oc_123",
                    "message_id": "om_123",
                    "message_type": "image",
                    "content": "{}"
                }
            }
        });
        let action =
            parse_feishu_webhook_payload(&payload, Some("token-123"), None, &BTreeSet::new(), true)
                .expect("non-text payload should parse");

        assert!(matches!(action, FeishuWebhookAction::Ignore));
    }

    fn encrypt_event_payload_for_test(plain_payload: &str, encrypt_key: &str) -> String {
        use cbc::cipher::{BlockEncryptMut, KeyIvInit};

        let key = Sha256::digest(encrypt_key.as_bytes());
        let iv = [7_u8; 16];

        let mut buffer = plain_payload.as_bytes().to_vec();
        let message_len = buffer.len();
        let pad_len = 16 - (message_len % 16);
        buffer.resize(message_len + pad_len, 0);

        let encrypted = cbc::Encryptor::<Aes256>::new_from_slices(&key, &iv)
            .expect("create encryptor")
            .encrypt_padded_mut::<Pkcs7>(&mut buffer, message_len)
            .expect("encrypt payload");

        let mut merged = iv.to_vec();
        merged.extend_from_slice(encrypted);
        base64::engine::general_purpose::STANDARD.encode(merged)
    }

    #[test]
    fn feishu_encrypted_payload_parses_with_encrypt_key() {
        let plain_payload = json!({
            "token": "token-123",
            "header": {
                "event_id": "evt_encrypted_1",
                "event_type": "im.message.receive_v1"
            },
            "event": {
                "sender": {"sender_type": "user"},
                "message": {
                    "chat_id": "oc_encrypt",
                    "message_id": "om_encrypt",
                    "message_type": "text",
                    "content": "{\"text\":\"encrypted hello\"}"
                }
            }
        });
        let plain_payload_str =
            serde_json::to_string(&plain_payload).expect("encode plain payload");
        let encrypted = encrypt_event_payload_for_test(&plain_payload_str, "encrypt-key");
        let wrapper = json!({ "encrypt": encrypted });

        let parsed = parse_feishu_webhook_payload(
            &wrapper,
            Some("token-123"),
            Some("encrypt-key"),
            &BTreeSet::new(),
            true,
        )
        .expect("parse encrypted payload");

        match parsed {
            FeishuWebhookAction::Inbound(event) => {
                assert_eq!(event.event_id, "evt_encrypted_1");
                assert_eq!(event.session_id, "feishu:oc_encrypt");
                assert_eq!(event.message_id, "om_encrypt");
                assert_eq!(event.text, "encrypted hello");
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn feishu_encrypted_payload_requires_encrypt_key() {
        let wrapper = json!({ "encrypt": "opaque" });
        let error =
            parse_feishu_webhook_payload(&wrapper, Some("token-123"), None, &BTreeSet::new(), true)
                .expect_err("encrypted payload without key should fail");

        assert!(error.contains("encrypt key is not configured"));
    }
}
