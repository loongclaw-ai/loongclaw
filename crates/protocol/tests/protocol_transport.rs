use std::collections::BTreeSet;
use std::time::Duration;

use loongclaw_protocol::test_support::*;
use loongclaw_protocol::*;
use tokio::io::{AsyncWriteExt, duplex, split};
use tokio::time::{sleep, timeout};

#[test]
fn route_parser_covers_standard_methods() {
    assert_eq!(
        ProtocolRoute::from_method("tools/call"),
        ProtocolRoute::ToolsCall
    );
    assert_eq!(
        ProtocolRoute::from_method("custom/x"),
        ProtocolRoute::Custom("custom/x".to_owned())
    );
    // Previously-standard routes now map to Custom
    assert_eq!(
        ProtocolRoute::from_method("initialize"),
        ProtocolRoute::Custom("initialize".to_owned())
    );
    assert_eq!(
        ProtocolRoute::from_method("ping"),
        ProtocolRoute::Custom("ping".to_owned())
    );
}

#[test]
fn strict_router_rejects_unknown_custom_methods() {
    let router = ProtocolRouter::strict();
    let error = router
        .resolve("internal/unsafe")
        .expect_err("strict mode should block unknown methods");
    assert!(matches!(error, RouterError::UnknownMethod(method) if method == "internal/unsafe"));
}

#[test]
fn custom_route_policy_is_applied() {
    let mut router = ProtocolRouter::strict();
    router
        .register_custom_route(
            "channel/publish",
            RoutePolicy {
                allow_anonymous: false,
                required_capability: Some("channel.publish".to_owned()),
            },
        )
        .expect("custom route registration should succeed");

    let resolved = router
        .resolve("channel/publish")
        .expect("registered custom route should resolve");
    assert_eq!(
        resolved.route,
        ProtocolRoute::Custom("channel/publish".to_owned())
    );
    assert!(!resolved.policy.allow_anonymous);
    assert_eq!(
        resolved.policy.required_capability.as_deref(),
        Some("channel.publish")
    );
}

#[test]
fn resolve_rejects_invalid_method_name() {
    let router = ProtocolRouter::default();
    let error = router
        .resolve("Tools/Call")
        .expect_err("invalid characters should be rejected");
    assert!(matches!(error, RouterError::InvalidMethod(_)));
}

#[test]
fn authorize_denies_when_capability_is_missing() {
    let router = ProtocolRouter::default();
    let resolved = router
        .resolve("tools/call")
        .expect("standard route should resolve");
    let error = router
        .authorize(
            &resolved,
            &RouteAuthorizationRequest {
                capabilities: BTreeSet::from(["discover".to_owned()]),
            },
        )
        .expect_err("tools/call should require invoke");
    assert!(matches!(
        error,
        RouteAuthorizationError::MissingCapability {
            method,
            required_capability
        } if method == "tools/call" && required_capability == "invoke"
    ));
}

#[test]
fn authorize_allows_when_capability_matches() {
    let router = ProtocolRouter::default();
    let resolved = router
        .resolve("tools/call")
        .expect("standard route should resolve");
    let decision = router
        .authorize(
            &resolved,
            &RouteAuthorizationRequest {
                capabilities: BTreeSet::from([" invoke ".to_owned()]),
            },
        )
        .expect("matching capability should authorize");
    assert_eq!(decision, RouteAuthorizationDecision::Allow);
}

#[test]
fn authorize_supports_wildcard_capability() {
    let router = ProtocolRouter::default();
    let resolved = router
        .resolve("tools/call")
        .expect("standard route should resolve");
    let decision = router
        .authorize(
            &resolved,
            &RouteAuthorizationRequest {
                capabilities: BTreeSet::from(["*".to_owned()]),
            },
        )
        .expect("wildcard capability should authorize");
    assert_eq!(decision, RouteAuthorizationDecision::Allow);
}

#[tokio::test]
async fn channel_transport_roundtrip_delivers_frame() {
    let (left, right) =
        ChannelTransport::linked(8, test_transport_info("left"), test_transport_info("right"))
            .expect("linked transport should initialize");

    left.send(OutboundFrame {
        method: "tools/call".to_owned(),
        id: Some("req-1".to_owned()),
        payload: serde_json::json!({"tool":"search"}),
        version: PROTOCOL_VERSION,
    })
    .await
    .expect("send should succeed");

    let received = right
        .recv()
        .await
        .expect("recv should succeed")
        .expect("peer frame should be available");
    assert_eq!(received.method, "tools/call");
    assert_eq!(received.id.as_deref(), Some("req-1"));
    assert_eq!(received.payload["tool"], "search");
}

#[tokio::test]
async fn channel_transport_close_stops_future_sends() {
    let (left, _right) =
        ChannelTransport::linked(4, test_transport_info("left"), test_transport_info("right"))
            .expect("linked transport should initialize");

    left.close().await.expect("close should succeed");
    let error = left
        .send(OutboundFrame {
            method: "ping".to_owned(),
            id: None,
            payload: serde_json::json!({}),
            version: PROTOCOL_VERSION,
        })
        .await
        .expect_err("send after close should fail");
    assert!(matches!(error, TransportError::Closed));
}

#[tokio::test]
async fn channel_transport_peer_close_produces_recv_none() {
    let (left, right) =
        ChannelTransport::linked(4, test_transport_info("left"), test_transport_info("right"))
            .expect("linked transport should initialize");

    left.close().await.expect("close should succeed");
    let received = right.recv().await.expect("recv should succeed");
    assert!(received.is_none(), "peer close should end receiver stream");
}

#[tokio::test]
async fn channel_transport_applies_bounded_backpressure() {
    let (left, right) =
        ChannelTransport::linked(1, test_transport_info("left"), test_transport_info("right"))
            .expect("linked transport should initialize");

    left.send(OutboundFrame {
        method: "tools/call".to_owned(),
        id: Some("req-1".to_owned()),
        payload: serde_json::json!({"seq":1}),
        version: PROTOCOL_VERSION,
    })
    .await
    .expect("first send should fill queue");

    let blocked_send = tokio::spawn(async move {
        left.send(OutboundFrame {
            method: "tools/call".to_owned(),
            id: Some("req-2".to_owned()),
            payload: serde_json::json!({"seq":2}),
            version: PROTOCOL_VERSION,
        })
        .await
    });

    sleep(Duration::from_millis(25)).await;
    assert!(
        !blocked_send.is_finished(),
        "second send should remain blocked while queue is full"
    );

    let first = right
        .recv()
        .await
        .expect("recv should succeed")
        .expect("first frame should be present");
    assert_eq!(first.payload["seq"], 1);

    timeout(Duration::from_secs(1), blocked_send)
        .await
        .expect("blocked send should finish once queue drains")
        .expect("join should succeed")
        .expect("send should succeed after drain");

    let second = right
        .recv()
        .await
        .expect("recv should succeed")
        .expect("second frame should be present");
    assert_eq!(second.payload["seq"], 2);
}

#[test]
fn channel_transport_rejects_zero_capacity() {
    let error =
        ChannelTransport::linked(0, test_transport_info("left"), test_transport_info("right"))
            .expect_err("zero capacity must fail");
    assert_eq!(error, TransportBuildError::InvalidCapacity(0));
}

#[tokio::test]
async fn json_line_transport_roundtrip_is_bidirectional() {
    let (left_stream, right_stream) = duplex(4 * 1024);
    let (left_read, left_write) = split(left_stream);
    let (right_read, right_write) = split(right_stream);

    let left = JsonLineTransport::new(test_transport_info("json-left"), left_read, left_write);
    let right = JsonLineTransport::new(test_transport_info("json-right"), right_read, right_write);

    left.send(OutboundFrame {
        method: "tools/call".to_owned(),
        id: Some("left-1".to_owned()),
        payload: serde_json::json!({"side":"left"}),
        version: PROTOCOL_VERSION,
    })
    .await
    .expect("left send should succeed");
    let from_left = right
        .recv()
        .await
        .expect("right recv should succeed")
        .expect("right should receive frame");
    assert_eq!(from_left.method, "tools/call");
    assert_eq!(from_left.id.as_deref(), Some("left-1"));
    assert_eq!(from_left.payload["side"], "left");

    right
        .send(OutboundFrame {
            method: "resources/read".to_owned(),
            id: Some("right-1".to_owned()),
            payload: serde_json::json!({"side":"right"}),
            version: PROTOCOL_VERSION,
        })
        .await
        .expect("right send should succeed");
    let from_right = left
        .recv()
        .await
        .expect("left recv should succeed")
        .expect("left should receive frame");
    assert_eq!(from_right.method, "resources/read");
    assert_eq!(from_right.id.as_deref(), Some("right-1"));
    assert_eq!(from_right.payload["side"], "right");
}

#[tokio::test]
async fn json_line_transport_rejects_invalid_json_frame() {
    let (transport_stream, mut peer_stream) = duplex(1024);
    let (reader, writer) = split(transport_stream);
    let transport = JsonLineTransport::new(test_transport_info("json-parse"), reader, writer);

    peer_stream
        .write_all(b"{\"method\":123,\"id\":null,\"payload\":{}}\n")
        .await
        .expect("peer write should succeed");

    let error = transport
        .recv()
        .await
        .expect_err("invalid frame should fail decode");
    assert!(
        matches!(error, TransportError::Failure(ref message) if message.contains("failed to decode inbound frame")),
        "unexpected decode error: {error}"
    );
}

#[tokio::test]
async fn json_line_transport_skips_empty_lines() {
    let (transport_stream, mut peer_stream) = duplex(1024);
    let (reader, writer) = split(transport_stream);
    let transport = JsonLineTransport::new(test_transport_info("json-empty"), reader, writer);

    peer_stream
        .write_all(b"\n\n{\"method\":\"ping\",\"id\":null,\"payload\":{}}\n")
        .await
        .expect("peer write should succeed");

    let received = transport
        .recv()
        .await
        .expect("recv should succeed")
        .expect("frame should be returned");
    assert_eq!(received.method, "ping");
}

#[tokio::test]
async fn json_line_transport_close_blocks_future_sends() {
    let (left_stream, _right_stream) = duplex(1024);
    let (left_read, left_write) = split(left_stream);
    let left = JsonLineTransport::new(test_transport_info("json-close"), left_read, left_write);

    left.close().await.expect("close should succeed");
    let error = left
        .send(OutboundFrame {
            method: "ping".to_owned(),
            id: None,
            payload: serde_json::json!({}),
            version: PROTOCOL_VERSION,
        })
        .await
        .expect_err("send after close should fail");
    assert!(matches!(error, TransportError::Closed));
}

#[test]
fn frame_without_version_deserializes_with_default() {
    let json = r#"{"method":"ping","id":null,"payload":{}}"#;
    let frame: InboundFrame = serde_json::from_str(json).expect("should deserialize");
    assert_eq!(frame.version, PROTOCOL_VERSION);
}

#[test]
fn frame_with_explicit_version_is_preserved() {
    let json = format!(
        r#"{{"method":"ping","id":null,"payload":{{}},"version":{}}}"#,
        PROTOCOL_VERSION
    );
    let frame: InboundFrame = serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(frame.version, PROTOCOL_VERSION);
}

#[test]
fn outbound_frame_serializes_version() {
    let frame = OutboundFrame {
        method: "ping".to_owned(),
        id: None,
        payload: serde_json::json!({}),
        version: PROTOCOL_VERSION,
    };
    let serialized = serde_json::to_value(&frame).expect("should serialize");
    assert_eq!(serialized["version"], PROTOCOL_VERSION);
}

#[tokio::test]
async fn json_line_transport_rejects_unsupported_version() {
    let (transport_stream, mut peer_stream) = duplex(1024);
    let (reader, writer) = split(transport_stream);
    let transport = JsonLineTransport::new(test_transport_info("json-version"), reader, writer);

    let future_frame = format!(
        r#"{{"method":"ping","id":null,"payload":{{}},"version":{}}}"#,
        PROTOCOL_VERSION + 1
    );
    peer_stream
        .write_all(format!("{future_frame}\n").as_bytes())
        .await
        .expect("peer write should succeed");

    let error = transport
        .recv()
        .await
        .expect_err("future version should be rejected");
    assert!(
        matches!(error, TransportError::Protocol(ref msg) if msg.contains("unsupported frame version")),
        "unexpected error: {error}"
    );
}
