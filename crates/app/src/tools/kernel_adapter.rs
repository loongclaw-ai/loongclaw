use async_trait::async_trait;
use loongclaw_contracts::ToolPlaneError;
use loongclaw_kernel::{CoreToolAdapter, ToolCoreOutcome, ToolCoreRequest};

use super::runtime_config::ToolRuntimeConfig;

pub struct MvpToolAdapter {
    config: Option<ToolRuntimeConfig>,
}

impl Default for MvpToolAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl MvpToolAdapter {
    pub fn new() -> Self {
        Self { config: None }
    }

    pub fn with_config(config: ToolRuntimeConfig) -> Self {
        Self {
            config: Some(config),
        }
    }
}

#[async_trait]
impl CoreToolAdapter for MvpToolAdapter {
    fn name(&self) -> &str {
        "mvp-tools"
    }

    async fn execute_core_tool(
        &self,
        request: ToolCoreRequest,
    ) -> Result<ToolCoreOutcome, ToolPlaneError> {
        let config = self.config.clone();
        let trusted_internal_payload = super::trusted_internal_tool_payload_enabled();
        tokio::task::spawn_blocking(move || {
            let execute = move || match config {
                Some(config) => super::execute_tool_core_with_config(request, &config),
                None => super::execute_tool_core(request),
            };

            if trusted_internal_payload {
                super::with_trusted_internal_tool_payload(execute)
            } else {
                execute()
            }
        })
        .await
        .map_err(|error| ToolPlaneError::Execution(format!("core tool task join failed: {error}")))?
        .map_err(ToolPlaneError::Execution)
    }
}

#[cfg(all(test, feature = "tool-webfetch"))]
#[allow(clippy::disallowed_methods, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::{Duration, Instant};

    fn spawn_slow_http_server(delay: Duration, body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let address = listener.local_addr().expect("local addr");

        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = [0_u8; 4096];
            let _ = stream.read(&mut buffer).expect("read request");
            thread::sleep(delay);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
            stream.flush().ok();
        });

        format!("http://{}", address)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn execute_core_tool_runs_blocking_fetch_without_pinching_runtime_progress() {
        let url = spawn_slow_http_server(Duration::from_millis(350), "ok");
        let mut config = ToolRuntimeConfig::default();
        config.web_fetch.allow_private_hosts = true;
        let adapter = MvpToolAdapter::with_config(config);

        let started = Instant::now();
        let timer = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Instant::now()
        });
        let fetch = tokio::spawn(async move {
            adapter
                .execute_core_tool(ToolCoreRequest {
                    tool_name: "web.fetch".to_owned(),
                    payload: json!({ "url": url }),
                })
                .await
        });

        let timer_fired_at = timer.await.expect("timer task should complete");
        assert!(
            timer_fired_at.duration_since(started) < Duration::from_millis(200),
            "tool execution blocked the runtime thread instead of yielding to the timer"
        );

        let outcome = fetch
            .await
            .expect("fetch task should join")
            .expect("fetch should succeed");
        assert_eq!(outcome.status, "ok");
    }
}
