use std::fmt::Debug;
use std::io::{self, IsTerminal, Write};

use opentelemetry::trace::{Span, Tracer, TracerProvider};
use opentelemetry::{KeyValue, global};
use opentelemetry_otlp::SpanExporter;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::util::SubscriberInitExt;

const DEFAULT_LOG_FILTER: &str = "warn";
const MAX_ERROR_CHARS: usize = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogFormat {
    Compact,
    Pretty,
    Json,
}

impl LogFormat {
    fn parse(raw: Option<&str>) -> Self {
        match raw
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("compact")
            .to_ascii_lowercase()
            .as_str()
        {
            "pretty" => Self::Pretty,
            "json" => Self::Json,
            _ => Self::Compact,
        }
    }
}

fn resolved_log_directive(loong_log: Option<&str>, rust_log: Option<&str>) -> String {
    loong_log
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| rust_log.map(str::trim).filter(|value| !value.is_empty()))
        .unwrap_or(DEFAULT_LOG_FILTER)
        .to_owned()
}

fn build_env_filter(raw: &str) -> EnvFilter {
    EnvFilter::try_new(raw).unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER))
}

pub fn summarize_error(error: &str) -> String {
    let compact = error.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= MAX_ERROR_CHARS {
        return compact;
    }

    let visible_chars = MAX_ERROR_CHARS.saturating_sub(3);
    let truncated = compact.chars().take(visible_chars).collect::<String>();
    format!("{truncated}...")
}

pub fn debug_variant_name(value: &impl Debug) -> String {
    let rendered = format!("{value:?}");
    let variant_end = rendered
        .find(|character: char| character.is_ascii_whitespace() || character == '{')
        .or_else(|| rendered.find('('))
        .unwrap_or(rendered.len());
    rendered[..variant_end].to_owned()
}

pub fn init_tracing() {
    let log_format = LogFormat::parse(std::env::var("LOONG_LOG_FORMAT").ok().as_deref());
    let directive = resolved_log_directive(
        std::env::var("LOONG_LOG").ok().as_deref(),
        std::env::var("RUST_LOG").ok().as_deref(),
    );
    let env_filter = build_env_filter(directive.as_str());
    let use_ansi = log_format != LogFormat::Json && io::stderr().is_terminal();
    let base = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(io::stderr)
        .with_target(true)
        .with_span_events(FmtSpan::CLOSE)
        .with_ansi(use_ansi);

    let init_result = match log_format {
        LogFormat::Compact => base.compact().finish().try_init(),
        LogFormat::Pretty => base.pretty().finish().try_init(),
        LogFormat::Json => base.json().flatten_event(true).finish().try_init(),
    };

    if let Err(error) = init_result {
        let mut stderr = io::stderr();
        let _ = writeln!(stderr, "loong.daemon tracing init failed: {error}");
    }
}

pub struct OtelGuard {
    provider: Option<SdkTracerProvider>,
}

impl Drop for OtelGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.provider.take()
            && let Err(e) = provider.shutdown()
        {
            let mut stderr = io::stderr();
            let _ = writeln!(stderr, "loong.daemon otel shutdown error: {e}");
        }
    }
}

pub fn init_otel() -> OtelGuard {
    let endpoint = match std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => return OtelGuard { provider: None },
    };

    let service_name = std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "loong".to_owned());

    let exporter = match SpanExporter::builder().with_http().build() {
        Ok(e) => e,
        Err(e) => {
            let mut stderr = io::stderr();
            let _ = writeln!(
                stderr,
                "loong.daemon otel exporter init failed (endpoint={endpoint}): {e}"
            );
            return OtelGuard { provider: None };
        }
    };

    let resource = Resource::builder()
        .with_attributes([KeyValue::new("service.name", service_name)])
        .build();

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build();

    global::set_tracer_provider(provider.clone());

    let tracer = provider.tracer("loong");
    let mut startup_span = tracer.span_builder("loong.init").start(&tracer);
    startup_span.set_attribute(KeyValue::new("otel.status", "initialized"));
    startup_span.end();

    OtelGuard {
        provider: Some(provider),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LogFormat, build_env_filter, debug_variant_name, resolved_log_directive, summarize_error,
    };
    use crate::Commands;

    #[test]
    fn resolved_log_directive_prefers_loong_log() {
        assert_eq!(
            resolved_log_directive(Some("loong_app=debug"), Some("warn")),
            "loong_app=debug"
        );
    }

    #[test]
    fn resolved_log_directive_falls_back_to_rust_log_then_default() {
        assert_eq!(resolved_log_directive(None, Some("info")), "info");
        assert_eq!(resolved_log_directive(None, None), "warn");
    }

    #[test]
    fn parse_log_format_accepts_known_variants() {
        assert_eq!(LogFormat::parse(Some("pretty")), LogFormat::Pretty);
        assert_eq!(LogFormat::parse(Some("json")), LogFormat::Json);
        assert_eq!(LogFormat::parse(Some("compact")), LogFormat::Compact);
        assert_eq!(LogFormat::parse(Some("unknown")), LogFormat::Compact);
    }

    #[test]
    fn build_env_filter_falls_back_on_invalid_directive() {
        let filter = build_env_filter("[broken");
        let rendered = filter.to_string();
        assert_eq!(rendered, "warn");
    }

    #[test]
    fn summarize_error_collapses_whitespace_and_truncates() {
        let repeated = "detail ".repeat(64);
        let summary = summarize_error(&format!("line one\nline two\t{repeated}"));

        assert!(!summary.contains('\n'));
        assert!(!summary.contains('\t'));
        assert!(summary.ends_with("..."));
        assert!(summary.chars().count() <= 240);
    }

    #[test]
    fn debug_variant_name_keeps_only_variant_identity() {
        let welcome = debug_variant_name(&Commands::Welcome);
        let turn_run = debug_variant_name(&Commands::Turn {
            command: crate::TurnCommands::Run {
                config: None,
                session: None,
                message: "ship".to_owned(),
                acp: false,
                acp_event_stream: false,
                acp_bootstrap_mcp_server: Vec::new(),
                acp_cwd: None,
            },
        });

        assert_eq!(welcome, "Welcome");
        assert_eq!(turn_run, "Turn");
    }
}
