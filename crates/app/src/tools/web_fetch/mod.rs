use loongclaw_contracts::{ToolCoreOutcome, ToolCoreRequest};

mod extractor;
mod sanitizer;
mod ssrf;

#[cfg(feature = "tool-webfetch")]
use reqwest::blocking::{Client, Response};
#[cfg(feature = "tool-webfetch")]
use reqwest::header::{CONTENT_TYPE, LOCATION};
#[cfg(feature = "tool-webfetch")]
use serde_json::{Value, json};

#[cfg(feature = "tool-webfetch")]
const HARD_MAX_BYTES: usize = 8 * 1_048_576;

#[cfg(feature = "tool-webfetch")]
#[derive(Debug)]
struct FetchOutput {
    final_url: String,
    status: u16,
    content_type: String,
    body: String,
    bytes: usize,
    truncated: bool,
}

#[cfg(feature = "tool-webfetch")]
struct ProcessedContent {
    content: String,
    extracted_text: String,
    title: Option<String>,
}

pub(super) fn execute_web_fetch_tool_with_config(
    request: ToolCoreRequest,
    config: &super::runtime_config::ToolRuntimeConfig,
) -> Result<ToolCoreOutcome, String> {
    #[cfg(not(feature = "tool-webfetch"))]
    {
        let _ = (request, config);
        return Err(
            "web.fetch tool is disabled in this build (enable feature `tool-webfetch`)".to_owned(),
        );
    }

    #[cfg(feature = "tool-webfetch")]
    {
        let _ = config;
        let payload = request
            .payload
            .as_object()
            .ok_or_else(|| "web.fetch payload must be an object".to_owned())?;
        let url = payload
            .get("url")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "web.fetch requires payload.url".to_owned())?;
        let validated = ssrf::validate_url(url)?;
        resolve_and_validate_host(&validated)?;

        let timeout_seconds = payload
            .get("timeout_seconds")
            .and_then(Value::as_u64)
            .unwrap_or(config.web_fetch_timeout_secs)
            .clamp(5, 120);
        let max_redirects = payload
            .get("max_redirects")
            .and_then(Value::as_u64)
            .unwrap_or(config.web_fetch_max_redirects as u64)
            .min(20) as usize;
        let max_bytes = payload
            .get("max_bytes")
            .and_then(Value::as_u64)
            .unwrap_or(config.web_fetch_max_bytes as u64)
            .clamp(1024, HARD_MAX_BYTES as u64) as usize;
        let mode = payload
            .get("mode")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("auto");
        if !matches!(mode, "auto" | "markdown" | "text" | "raw") {
            return Err(format!(
                "web.fetch payload.mode must be one of auto|markdown|text|raw, got `{mode}`"
            ));
        }

        let client = build_client(timeout_seconds)?;
        let fetched = fetch_url(
            &client,
            validated.url,
            max_redirects,
            max_bytes,
            |candidate| {
                let validated = ssrf::validate_url(candidate)?;
                resolve_and_validate_host(&validated)
            },
        )?;
        let processed = process_content(mode, &fetched.content_type, &fetched.body);

        Ok(ToolCoreOutcome {
            status: "ok".to_owned(),
            payload: json!({
                "adapter": "core-tools",
                "tool_name": request.tool_name,
                "url": url,
                "final_url": fetched.final_url,
                "status_code": fetched.status,
                "content_type": fetched.content_type,
                "content": processed.content,
                "extracted_text": processed.extracted_text,
                "title": processed.title,
                "bytes": fetched.bytes,
                "truncated": fetched.truncated,
            }),
        })
    }
}

#[cfg(feature = "tool-webfetch")]
fn process_content(mode: &str, content_type: &str, body: &str) -> ProcessedContent {
    let is_html = content_type.eq_ignore_ascii_case("text/html")
        || content_type.eq_ignore_ascii_case("application/xhtml+xml");

    if mode == "raw" {
        return ProcessedContent {
            content: body.to_owned(),
            extracted_text: String::new(),
            title: None,
        };
    }

    if is_html {
        let sanitized = sanitizer::sanitize_html(body);
        let title = extractor::extract_title(&sanitized);
        let markdown = extractor::html_to_markdown(&sanitized);
        if mode == "text" {
            let text = extractor::extract_main_content(&sanitized);
            return ProcessedContent {
                content: text.clone(),
                extracted_text: text,
                title,
            };
        }
        let text = extractor::markdown_to_text(&markdown);
        return ProcessedContent {
            content: markdown,
            extracted_text: text,
            title,
        };
    }

    if mode == "text" {
        return ProcessedContent {
            content: body.to_owned(),
            extracted_text: body.to_owned(),
            title: None,
        };
    }

    let text = extractor::markdown_to_text(body);
    ProcessedContent {
        content: body.to_owned(),
        extracted_text: text,
        title: None,
    }
}

#[cfg(feature = "tool-webfetch")]
fn build_client(timeout_seconds: u64) -> Result<Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_seconds))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| format!("web.fetch client init failed: {error}"))
}

#[cfg(feature = "tool-webfetch")]
fn fetch_url<F>(
    client: &Client,
    initial_url: reqwest::Url,
    max_redirects: usize,
    max_bytes: usize,
    mut validate_redirect: F,
) -> Result<FetchOutput, String>
where
    F: FnMut(&str) -> Result<(), String>,
{
    let mut current = initial_url;

    for redirect_count in 0..=max_redirects {
        let response = client
            .get(current.clone())
            .send()
            .map_err(classify_request_error)?;

        if response.status().is_redirection() {
            if redirect_count == max_redirects {
                return Err("too_many_redirects: reached redirect limit".to_owned());
            }
            current = resolve_redirect_target(&current, &response)?;
            validate_redirect(current.as_str())?;
            continue;
        }

        return decode_response(response, current, max_bytes);
    }

    Err("too_many_redirects: reached redirect limit".to_owned())
}

#[cfg(feature = "tool-webfetch")]
fn resolve_redirect_target(
    current: &reqwest::Url,
    response: &Response,
) -> Result<reqwest::Url, String> {
    let location = response
        .headers()
        .get(LOCATION)
        .ok_or_else(|| "invalid_redirect: missing Location header".to_owned())?
        .to_str()
        .map_err(|error| format!("invalid_redirect: bad Location header: {error}"))?;
    current
        .join(location)
        .map_err(|error| format!("invalid_redirect: {error}"))
}

#[cfg(feature = "tool-webfetch")]
fn decode_response(
    mut response: Response,
    final_url: reqwest::Url,
    max_bytes: usize,
) -> Result<FetchOutput, String> {
    use std::io::Read;

    let content_type_header = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_owned();
    let content_type = content_type_header
        .split(';')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("application/octet-stream")
        .to_owned();

    let cap = max_bytes.min(HARD_MAX_BYTES);
    let mut reader = response.by_ref().take((cap + 1) as u64);
    let mut buffer = Vec::new();
    reader
        .read_to_end(&mut buffer)
        .map_err(|error| format!("web.fetch body read failed: {error}"))?;

    let truncated = buffer.len() > cap;
    if truncated {
        buffer.truncate(cap);
    }

    let body = decode_text_body(&buffer, &content_type_header);

    Ok(FetchOutput {
        final_url: final_url.to_string(),
        status: response.status().as_u16(),
        content_type,
        bytes: buffer.len(),
        truncated,
        body,
    })
}

#[cfg(feature = "tool-webfetch")]
fn decode_text_body(bytes: &[u8], content_type_header: &str) -> String {
    let header = content_type_header.to_ascii_lowercase();

    if header.contains("charset=utf-16le") {
        return decode_utf16(bytes, true);
    }
    if header.contains("charset=utf-16be") {
        return decode_utf16(bytes, false);
    }
    if header.contains("charset=utf-8") {
        return String::from_utf8_lossy(bytes).to_string();
    }

    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let rest = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or_default();
        return String::from_utf8_lossy(rest).to_string();
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let rest = bytes.strip_prefix(&[0xFF, 0xFE]).unwrap_or_default();
        return decode_utf16(rest, true);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let rest = bytes.strip_prefix(&[0xFE, 0xFF]).unwrap_or_default();
        return decode_utf16(rest, false);
    }

    String::from_utf8_lossy(bytes).to_string()
}

#[cfg(feature = "tool-webfetch")]
fn decode_utf16(bytes: &[u8], little_endian: bool) -> String {
    let mut units = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        let [a, b] = match chunk {
            [a, b] => [*a, *b],
            _ => continue,
        };
        let value = if little_endian {
            u16::from_le_bytes([a, b])
        } else {
            u16::from_be_bytes([a, b])
        };
        units.push(value);
    }
    String::from_utf16_lossy(&units)
}

#[cfg(feature = "tool-webfetch")]
fn classify_request_error(error: reqwest::Error) -> String {
    if error.is_timeout() {
        return format!("timeout: {error}");
    }
    format!("request_failed: {error}")
}

#[cfg(feature = "tool-webfetch")]
fn resolve_and_validate_host(validated: &ssrf::ValidatedUrl) -> Result<(), String> {
    let host = validated.normalized_host.as_str();
    let port = validated.url.port_or_known_default().unwrap_or(80);
    let addrs: Vec<std::net::IpAddr> = std::net::ToSocketAddrs::to_socket_addrs(&(host, port))
        .map_err(|error| format!("dns_resolution_failed: {error}"))?
        .map(|addr| addr.ip())
        .collect();
    ssrf::validate_resolved_addresses(&addrs)
}

#[cfg(all(test, feature = "tool-webfetch"))]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn fetch_url_follows_redirects() {
        let base = spawn_server(|path, stream| {
            if path == "/redirect" {
                write_response(
                    stream,
                    "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\n\r\n",
                );
            } else {
                write_response(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: 5\r\n\r\nhello",
                );
            }
        });

        let client = build_client(5);
        assert!(client.is_ok(), "client init failed");
        let initial = reqwest::Url::parse(&format!("{base}/redirect"));
        assert!(initial.is_ok(), "url parse failed");

        let client = match client {
            Ok(value) => value,
            Err(_) => return,
        };
        let initial = match initial {
            Ok(value) => value,
            Err(_) => return,
        };

        let fetched = fetch_url(&client, initial, 5, 1024, |_candidate| Ok(()));
        assert!(fetched.is_ok(), "fetch failed");
        let fetched = match fetched {
            Ok(value) => value,
            Err(_) => return,
        };

        assert_eq!(fetched.status, 200);
        assert_eq!(fetched.body, "hello");
        assert!(fetched.final_url.ends_with("/final"));
    }

    #[test]
    fn fetch_url_applies_timeout() {
        let base = spawn_server(|_path, stream| {
            thread::sleep(Duration::from_millis(1_500));
            write_response(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nok",
            );
        });

        let client = build_client(1);
        assert!(client.is_ok(), "client init failed");
        let initial = reqwest::Url::parse(&format!("{base}/slow"));
        assert!(initial.is_ok(), "url parse failed");

        let client = match client {
            Ok(value) => value,
            Err(_) => return,
        };
        let initial = match initial {
            Ok(value) => value,
            Err(_) => return,
        };

        let err = fetch_url(&client, initial, 0, 1024, |_candidate| Ok(()))
            .err()
            .unwrap_or_else(|| "missing expected timeout".to_owned());
        assert!(
            err.contains("timeout") || err.contains("request_failed"),
            "expected timeout-like error, got `{err}`"
        );
    }

    #[test]
    fn fetch_url_limits_response_size() {
        let body = "x".repeat(3000);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let base = spawn_server(move |_path, stream| {
            write_response(stream, &response);
        });

        let client = build_client(5);
        assert!(client.is_ok(), "client init failed");
        let initial = reqwest::Url::parse(&format!("{base}/size"));
        assert!(initial.is_ok(), "url parse failed");

        let client = match client {
            Ok(value) => value,
            Err(_) => return,
        };
        let initial = match initial {
            Ok(value) => value,
            Err(_) => return,
        };

        let fetched = fetch_url(&client, initial, 0, 1024, |_candidate| Ok(()));
        assert!(fetched.is_ok(), "fetch failed");
        let fetched = match fetched {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(fetched.truncated);
        assert_eq!(fetched.bytes, 1024);
        assert_eq!(fetched.body.len(), 1024);
    }

    #[test]
    fn decode_text_body_uses_utf16_bom_when_charset_missing() {
        let bytes = [0xFF, 0xFE, b'h', 0x00, b'i', 0x00];
        let decoded = decode_text_body(&bytes, "text/plain");
        assert_eq!(decoded, "hi");
    }

    #[test]
    fn redirect_chain_revalidation_blocks_private_target() {
        let base = spawn_server(|path, stream| {
            if path == "/redirect" {
                write_response(
                    stream,
                    "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1/private\r\nContent-Length: 0\r\n\r\n",
                );
            } else {
                write_response(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nok",
                );
            }
        });

        let client = build_client(5);
        assert!(client.is_ok(), "client init failed");
        let initial = reqwest::Url::parse(&format!("{base}/redirect"));
        assert!(initial.is_ok(), "url parse failed");

        let client = match client {
            Ok(value) => value,
            Err(_) => return,
        };
        let initial = match initial {
            Ok(value) => value,
            Err(_) => return,
        };

        let err = fetch_url(&client, initial, 5, 1024, |candidate| {
            let validated = ssrf::validate_url(candidate)?;
            resolve_and_validate_host(&validated)
        })
        .err()
        .unwrap_or_else(|| "missing expected error".to_owned());

        assert!(err.contains("ssrf_blocked"));
    }

    fn spawn_server<F>(handler: F) -> String
    where
        F: Fn(&str, &mut std::net::TcpStream) + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0");
        assert!(listener.is_ok(), "bind failed");
        let listener = match listener {
            Ok(value) => value,
            Err(_) => return String::new(),
        };
        let addr = listener.local_addr();
        assert!(addr.is_ok(), "local_addr failed");
        let addr = match addr {
            Ok(value) => value,
            Err(_) => return String::new(),
        };
        let handler = std::sync::Arc::new(handler);

        thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let mut buf = [0_u8; 2048];
                let read = stream.read(&mut buf).unwrap_or(0);
                let request = String::from_utf8_lossy(buf.get(..read).unwrap_or(&[]));
                let path = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or("/");
                handler(path, &mut stream);
            }
        });

        format!("http://{}", addr)
    }

    fn write_response(stream: &mut std::net::TcpStream, response: &str) {
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    }
}
