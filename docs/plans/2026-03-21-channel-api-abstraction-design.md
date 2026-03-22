# Channel API Abstraction Design

## Status

Draft - Pending Implementation

## Summary

Refactor the channel integration architecture to establish a platform-agnostic API abstraction layer. This enables:
- Consistent directory structure across all channel integrations (Telegram, Matrix, Feishu)
- Shared tool infrastructure via common traits
- Dependency inversion: tools depend on traits, not concrete implementations
- Extensibility for future integrations (Slack, Discord, etc.)

## Problem Statement

### Current Issues

| Issue | Description |
|-------|-------------|
| **Inconsistent Structure** | Feishu has two directories (`feishu/` and `channel/feishu/`) while Telegram/Matrix are single files |
| **Bidirectional Dependencies** | Both `tools/feishu.rs` and `channel/feishu/` depend on top-level `feishu/` module |
| **No API Reuse** | Each channel's tools are implemented independently without shared abstractions |
| **Tight Coupling** | Tools are bound to specific client implementations rather than interfaces |
| **Trait Overlap** | Existing `ChannelAdapter` trait handles transport, but platform API capabilities are not abstracted |

### Current Architecture Analysis

**Existing `ChannelAdapter` Trait** (`channel/mod.rs:548`):
```rust
pub trait ChannelAdapter {
    fn name(&self) -> &str;
    async fn receive_batch(&mut self) -> CliResult<Vec<ChannelInboundMessage>>;
    async fn send_message(&self, target: &ChannelOutboundTarget, message: &ChannelOutboundMessage>) -> CliResult<()>;
    async fn ack_inbound(&mut self, _message: &ChannelInboundMessage) -> CliResult<()>;
    async fn complete_batch(&mut self) -> CliResult<()>;
}
```

**Current Dependency Flow**:
```
tools/feishu.rs ────────┐
                        ▼
feishu/ (client) ←──── channel/feishu/adapter.rs
        │                      │
        └── resources/ ←───────┘
```

The `ChannelAdapter` trait focuses on **transport layer** concerns (receiving batches, acknowledging messages). Platform-specific capabilities (documents, calendar, advanced messaging) are not abstracted and are directly accessed by tools.

### Root Cause

The current architecture lacks a clear separation between:
1. **Channel transport layer** - `ChannelAdapter` trait (webhook/websocket/polling)
2. **Platform API layer** - No abstraction exists (messaging/docs/calendar)
3. **Tool registration layer** - Direct dependency on concrete implementations

## Design Goals

1. **Structural Consistency**: All channels follow the same directory pattern
2. **API Abstraction**: Common capabilities expressed as traits
3. **Dependency Inversion**: Higher-level modules depend on abstractions
4. **Extensibility**: New channels implement existing traits without modifying tools layer
5. **Backward Compatibility**: Existing tool names, configs, and behaviors unchanged

## Architecture

### Dependency Flow

```text
┌─────────────────────────────────────────────────────────────────┐
│                           tools/                                 │
│              Generic tool handlers (messaging, docs, calendar)    │
│                         ▲                                       │
│                         │ uses dyn PlatformApi                  │
│                         │                                       │
│                         ▼                                       │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                    channel/traits/                          │ │
│  │  ┌─────────────┐  ┌──────────────┐  ┌─────────────┐        │ │
│  │  │ MessagingApi │  │ DocumentsApi │  │ CalendarApi │        │ │
│  │  │  (extends)   │  │  (optional)  │  │  (optional) │        │ │
│  │  └──────┬───────┘  └──────────────┘  └─────────────┘        │ │
│  │         │ extends                                           │ │
│  │  ┌──────┴───────┐                                           │ │
│  │  │ ChannelAdapter│ ← existing trait (transport layer)       │ │
│  │  └───────────────┘                                           │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                         ▲                                       │
│                         │ implemented by                        │
│                         │                                       │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │               channel/{feishu,telegram,matrix}/api/         │ │
│  │         Platform API trait implementations                   │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                         ▲                                       │
│                         │ uses                                  │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │            channel/{feishu,telegram,matrix}/                │ │
│  │     ChannelAdapter impl (transport + platform glue)         │ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

**Layer Responsibilities**:
- **ChannelAdapter** (existing): Transport layer - message receiving, batching, acknowledgment
- **PlatformApi traits** (new): Platform capability layer - documents, calendar, messaging operations
- **Implementation glue**: Each adapter implements both ChannelAdapter and PlatformApi traits

### Directory Structure

```text
crates/app/src/
├── channel/
│   ├── mod.rs              ← re-exports channel types
│   ├── traits/
│   │   ├── mod.rs          ← re-exports all traits
│   │   ├── messaging.rs    ← MessagingApi trait
│   │   ├── documents.rs    ← DocumentsApi trait
│   │   ├── calendar.rs     ← CalendarApi trait
│   │   └── error.rs        ← ApiError and ApiResult types
│   ├── registry.rs         ← Channel registry
│   ├── runtime_state.rs    ← Runtime state tracking
│   ├── telegram/
│   │   ├── mod.rs          ← TelegramAdapter (implements ChannelAdapter + MessagingApi)
│   │   └── api/
│   │       └── mod.rs      ← Telegram client and API implementations
│   ├── matrix/
│   │   ├── mod.rs          ← MatrixAdapter (implements ChannelAdapter + MessagingApi)
│   │   └── api/
│   │       └── mod.rs      ← Matrix client and API implementations
│   └── feishu/
│       ├── mod.rs          ← FeishuAdapter (implements ChannelAdapter + all Platform APIs)
│       ├── api/            ← All Feishu client code (moved from feishu/)
│       │   ├── mod.rs      ← Module exports
│       │   ├── client.rs   ← FeishuClient (HTTP client)
│       │   ├── auth.rs     ← Authentication logic
│       │   ├── token_store.rs ← Token management
│       │   ├── principal.rs   ← User principal handling
│       │   ├── runtime.rs     ← Runtime operations
│       │   ├── resources/     ← API resource modules
│       │   │   ├── mod.rs
│       │   │   ├── messages.rs
│       │   │   ├── docs.rs
│       │   │   ├── calendar.rs
│       │   │   ├── cards.rs
│       │   │   └── media.rs
│       │   └── error.rs       ← API errors
│       ├── adapter.rs         ← FeishuAdapter (ChannelAdapter impl)
│       ├── webhook.rs         ← Webhook handling
│       ├── websocket.rs       ← WebSocket handling
│       └── payload/           ← Payload types
│           ├── mod.rs
│           ├── types.rs
│           ├── inbound.rs
│           ├── outbound.rs
│           └── crypto.rs
│
└── tools/
    ├── mod.rs
    ├── channel/
    │   ├── mod.rs          ← Channel tools module
    │   ├── generic.rs      ← Generic tool handlers using dyn PlatformApi
    │   ├── telegram.rs     ← Telegram-specific tool wrappers
    │   ├── matrix.rs       ← Matrix-specific tool wrappers
    │   └── feishu.rs       ← Feishu-specific tool wrappers (moved from tools/feishu.rs)
    └── ...                 ← Other tools (shell, file, browser, etc.)

# DEPRECATED (to be removed after migration):
feishu/                     ← All contents moved to channel/feishu/api/
tools/feishu.rs             ← Contents moved to tools/channel/feishu.rs
```

### Key Principles

1. **Two-Layer Trait Architecture**:
   - **`ChannelAdapter`** (existing): Transport layer - handles message receiving, batching, acknowledgment
   - **`PlatformApi` traits** (new): Capability layer - messaging, documents, calendar operations
   - Adapters implement both traits; tools depend only on PlatformApi traits

2. **Mandatory vs Optional Traits**:
   - `MessagingApi` - All channels must implement (extends ChannelAdapter)
   - `DocumentsApi`, `CalendarApi` - Implement only if supported (optional)

3. **Single Responsibility**:
   - Each channel directory contains only channel-specific code
   - `api/` subdirectory contains platform client implementations
   - Shared abstractions live in `traits/`
   - Generic tools use dynamic dispatch (`dyn PlatformApi`)

4. **Feishu Consolidation**:
   - Move all code from top-level `feishu/` to `channel/feishu/api/`
   - Eliminate bidirectional dependencies
   - Maintain backward compatibility through re-exports during transition

5. **Feature Gating**:
   - Optional traits gated by corresponding feature flags
   - Tool registration checks trait availability at runtime via `Any` downcasting

## Traits Design

### Trait Architecture

The traits are designed with minimal associated types to reduce complexity while maintaining flexibility:

```rust
// channel/traits/mod.rs
pub use messaging::{MessagingApi, MessageContent, Message, MediaType, MediaUploadResult};
pub use documents::{DocumentsApi, Document, DocumentContent};
pub use calendar::{CalendarApi, Calendar, CalendarList, FreeBusyResult, TimeRange};
pub use error::{ApiError, ApiResult};

/// Marker trait for all platform API capabilities
pub trait PlatformApi: Send + Sync {}
```

### MessagingApi (Mandatory)

All channel adapters MUST implement this trait. It extends `PlatformApi` and provides messaging operations.

**Design Rationale**: Reduced from 4 associated types to 0. Most platforms use similar types:
- Receipt: String (message ID)
- Message: Concrete struct with common fields
- Pagination: Generic struct with platform-specific serialization

```rust
// channel/traits/messaging.rs
use async_trait::async_trait;

#[async_trait]
pub trait MessagingApi: PlatformApi {
    /// Send a message to a target (chat, channel, room)
    /// Returns the platform-specific message ID
    async fn send_message(
        &self,
        target: &str,
        content: &MessageContent,
    ) -> ApiResult<String>;

    /// Reply to an existing message
    async fn reply(
        &self,
        parent_id: &str,
        content: &MessageContent,
    ) -> ApiResult<String>;

    /// Get a message by ID
    async fn get_message(&self, message_id: &str) -> ApiResult<Message>;

    /// List messages in a chat with pagination
    async fn list_messages(
        &self,
        chat_id: &str,
        pagination: &Pagination,
    ) -> ApiResult<Vec<Message>>;

    /// Upload media (image, file, etc.)
    async fn upload_media(
        &self,
        file_path: &std::path::Path,
        media_type: MediaType,
    ) -> ApiResult<MediaUploadResult>;
}

#[derive(Clone, Debug, Default)]
pub struct MessageContent {
    pub text: Option<String>,
    pub html: Option<String>,
    pub markdown: Option<String>,
    pub image_key: Option<String>,
    pub file_key: Option<String>,
    pub file_type: Option<String>,
    /// Platform-specific card/interactive message content (JSON)
    pub card: Option<serde_json::Value>,
}

#[derive(Clone, Debug)]
pub struct Message {
    pub id: String,
    pub chat_id: String,
    pub sender_id: String,
    pub content: MessageContent,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub reply_to: Option<String>,
    /// Platform-specific metadata (JSON)
    pub platform_metadata: serde_json::Value,
}

#[derive(Clone, Debug, Default)]
pub struct Pagination {
    pub page_size: u32,
    pub cursor: Option<String>,
    /// Some platforms use page numbers instead of cursors
    pub page: Option<u32>,
}

#[derive(Clone, Copy, Debug)]
pub enum MediaType {
    Image,
    File,
    Audio,
    Video,
    Sticker,
}

#[derive(Clone, Debug)]
pub struct MediaUploadResult {
    pub file_key: String,
    pub file_name: Option<String>,
    pub file_size: Option<u64>,
    pub mime_type: Option<String>,
}
```

### DocumentsApi (Optional)

Implement this trait if the platform supports document operations.

**Design Rationale**: Removed associated types, use concrete structs that can hold platform-specific data in `platform_metadata`.

```rust
// channel/traits/documents.rs
use async_trait::async_trait;

#[async_trait]
pub trait DocumentsApi: PlatformApi {
    /// Create a new document
    async fn create_document(
        &self,
        title: &str,
        content: Option<&str>,
        folder_id: Option<&str>,
    ) -> ApiResult<Document>;

    /// Read document content
    async fn read_document(&self, doc_id: &str) -> ApiResult<DocumentContent>;

    /// Append content to document
    async fn append_to_document(
        &self,
        doc_id: &str,
        content: &str,
    ) -> ApiResult<()>;

    /// Update document (replace content)
    async fn update_document(
        &self,
        doc_id: &str,
        content: &str,
    ) -> ApiResult<()>;

    /// Delete a document
    async fn delete_document(&self, doc_id: &str) -> ApiResult<()>;

    /// Search documents
    async fn search_documents(
        &self,
        query: &str,
        pagination: &Pagination,
    ) -> ApiResult<Vec<Document>>;
}

#[derive(Clone, Debug)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub url: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub owner_id: Option<String>,
    /// Platform-specific metadata (e.g., Feishu document type, permissions)
    pub platform_metadata: serde_json::Value,
}

#[derive(Clone, Debug)]
pub struct DocumentContent {
    pub doc_id: String,
    pub title: String,
    /// Plain text content
    pub text: Option<String>,
    /// Markdown content (if available)
    pub markdown: Option<String>,
    /// HTML content (if available)
    pub html: Option<String>,
    /// Platform-specific content blocks (JSON)
    pub blocks: Option<serde_json::Value>,
}
```

### CalendarApi (Optional)

Implement this trait if the platform supports calendar operations.

```rust
// channel/traits/calendar.rs
use async_trait::async_trait;

#[async_trait]
pub trait CalendarApi: PlatformApi {
    /// List all accessible calendars
    async fn list_calendars(&self) -> ApiResult<Vec<Calendar>>;

    /// Get the user's primary calendar
    async fn get_primary_calendar(&self) -> ApiResult<Calendar>;

    /// Get a specific calendar by ID
    async fn get_calendar(&self, calendar_id: &str) -> ApiResult<Calendar>;

    /// Query free/busy status for participants
    async fn query_freebusy(
        &self,
        time_range: &TimeRange,
        participants: &[String],
    ) -> ApiResult<Vec<FreeBusyResult>>;

    /// Create a calendar event
    async fn create_event(
        &self,
        calendar_id: &str,
        event: &CreateEventRequest,
    ) -> ApiResult<CalendarEvent>;

    /// Get events in a time range
    async fn list_events(
        &self,
        calendar_id: &str,
        time_range: &TimeRange,
    ) -> ApiResult<Vec<CalendarEvent>>;
}

#[derive(Clone, Debug)]
pub struct Calendar {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_primary: bool,
    pub timezone: Option<String>,
    pub platform_metadata: serde_json::Value,
}

#[derive(Clone, Debug)]
pub struct CalendarEvent {
    pub id: String,
    pub calendar_id: String,
    pub title: String,
    pub description: Option<String>,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub end_time: chrono::DateTime<chrono::Utc>,
    pub location: Option<String>,
    pub organizer_id: String,
    pub attendee_ids: Vec<String>,
    pub status: EventStatus,
    pub platform_metadata: serde_json::Value,
}

#[derive(Clone, Copy, Debug)]
pub enum EventStatus {
    Confirmed,
    Tentative,
    Cancelled,
}

#[derive(Clone, Debug)]
pub struct CreateEventRequest {
    pub title: String,
    pub description: Option<String>,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub end_time: chrono::DateTime<chrono::Utc>,
    pub location: Option<String>,
    pub attendee_ids: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct TimeRange {
    pub start: chrono::DateTime<chrono::Utc>,
    pub end: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug)]
pub struct FreeBusyResult {
    pub user_id: String,
    pub freebusy: Vec<TimeRange>,
}
```

### CalendarApiAble (Optional)

Implement this trait if the platform supports calendar operations.

```rust
pub trait CalendarApiAble: Send + Sync {
    type Calendar: Send + Sync;
    type CalendarList: Send + Sync;
    type FreeBusyResult: Send + Sync;

    async fn list_calendars(&self) -> ApiResult<Self::CalendarList>;

    async fn get_primary_calendar(&self) -> ApiResult<Self::Calendar>;

    async fn query_freebusy(
        &self,
        time_range: &TimeRange,
        participants: &[String],
    ) -> ApiResult<Self::FreeBusyResult>;
}

#[derive(Clone, Debug)]
pub struct TimeRange {
    pub start_timestamp: i64,
    pub end_timestamp: i64,
}
```

### Error Handling

Unified error type for all platform APIs with platform-specific error details preserved.

```rust
// channel/traits/error.rs
pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("network error: {0}")]
    Network(#[source] std::io::Error),

    #[error("HTTP error: status={status}, body={body}")]
    Http {
        status: u16,
        body: String,
    },

    #[error("authentication failed: {message}")]
    Auth {
        message: String,
        /// Suggested retry timestamp (if token refresh needed)
        retry_after: Option<chrono::DateTime<chrono::Utc>>,
    },

    #[error("rate limited, retry after {retry_after:?}")]
    RateLimited {
        retry_after: Option<chrono::DateTime<chrono::Utc>>,
        retry_after_secs: Option<u64>,
    },

    #[error("not found: {resource} (id={id:?})")]
    NotFound {
        resource: String,
        id: Option<String>,
    },

    #[error("permission denied: {action} on {resource}")]
    PermissionDenied {
        action: String,
        resource: String,
    },

    #[error("invalid request: {message} (field={field:?})")]
    InvalidRequest {
        message: String,
        field: Option<String>,
    },

    #[error("operation not supported: {operation}")]
    NotSupported {
        operation: String,
        platform: String,
    },

    #[error("platform error: platform={platform}, code={code}, message={message}")]
    Platform {
        platform: String,
        code: String,
        message: String,
        /// Raw platform error response (JSON)
        raw: Option<serde_json::Value>,
    },

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("internal error: {0}")]
    Internal(String),
}

impl ApiError {
    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ApiError::Network(_) | ApiError::RateLimited { .. } | ApiError::Auth { .. }
        )
    }

    /// Get retry delay if applicable
    pub fn retry_delay(&self) -> Option<std::time::Duration> {
        match self {
            ApiError::RateLimited { retry_after_secs: Some(secs), .. } => {
                Some(std::time::Duration::from_secs(*secs))
            }
            _ => None,
        }
    }
}
```

## Generic Tools Layer

### Design Approach

The tools layer uses **dynamic dispatch** (`dyn Trait`) instead of generics to:
1. Avoid monomorphization overhead in the tool catalog
2. Allow runtime polymorphism for different platforms
3. Simplify registration patterns

### Tool Handlers

```rust
// tools/channel/generic.rs

use std::sync::Arc;
use crate::channel::traits::*;

/// Type-erased platform API reference
pub type PlatformApiRef = Arc<dyn PlatformApi>;

/// Check if platform supports MessagingApi
pub fn has_messaging_api(api: &PlatformApiRef) -> bool {
    api.as_any().downcast_ref::<dyn MessagingApi>().is_some()
}

/// Get MessagingApi if supported
pub fn as_messaging_api(api: &PlatformApiRef) -> Option<&dyn MessagingApi> {
    api.as_any().downcast_ref::<dyn MessagingApi>()
}

/// Check if platform supports DocumentsApi
pub fn has_documents_api(api: &PlatformApiRef) -> bool {
    api.as_any().downcast_ref::<dyn DocumentsApi>().is_some()
}

/// Get DocumentsApi if supported
pub fn as_documents_api(api: &PlatformApiRef) -> Option<&dyn DocumentsApi> {
    api.as_any().downcast_ref::<dyn DocumentsApi>()
}

/// Check if platform supports CalendarApi
pub fn has_calendar_api(api: &PlatformApiRef) -> bool {
    api.as_any().downcast_ref::<dyn CalendarApi>().is_some()
}

/// Get CalendarApi if supported
pub fn as_calendar_api(api: &PlatformApiRef) -> Option<&dyn CalendarApi> {
    api.as_any().downcast_ref::<dyn CalendarApi>()
}

/// Send a message using dynamic dispatch
pub async fn send_message(
    api: &dyn MessagingApi,
    target: &str,
    content: &MessageContent,
) -> ApiResult<String> {
    api.send_message(target, content).await
}

/// Reply to a message using dynamic dispatch
pub async fn reply_message(
    api: &dyn MessagingApi,
    parent_id: &str,
    content: &MessageContent,
) -> ApiResult<String> {
    api.reply(parent_id, content).await
}

/// Get message using dynamic dispatch
pub async fn get_message(
    api: &dyn MessagingApi,
    message_id: &str,
) -> ApiResult<Message> {
    api.get_message(message_id).await
}

/// List messages using dynamic dispatch
pub async fn list_messages(
    api: &dyn MessagingApi,
    chat_id: &str,
    pagination: &Pagination,
) -> ApiResult<Vec<Message>> {
    api.list_messages(chat_id, pagination).await
}

/// Create document using dynamic dispatch
pub async fn create_document(
    api: &dyn DocumentsApi,
    title: &str,
    content: Option<&str>,
) -> ApiResult<Document> {
    api.create_document(title, content, None).await
}

/// Read document using dynamic dispatch
pub async fn read_document(
    api: &dyn DocumentsApi,
    doc_id: &str,
) -> ApiResult<DocumentContent> {
    api.read_document(doc_id).await
}

/// List calendars using dynamic dispatch
pub async fn list_calendars(
    api: &dyn CalendarApi,
) -> ApiResult<Vec<Calendar>> {
    api.list_calendars().await
}

/// Query freebusy using dynamic dispatch
pub async fn query_freebusy(
    api: &dyn CalendarApi,
    range: &TimeRange,
    participants: &[String],
) -> ApiResult<Vec<FreeBusyResult>> {
    api.query_freebusy(range, participants).await
}
```

### Registration Pattern

Tools are registered using dynamic dispatch. The registration functions check which traits are implemented and register only the supported tools.

```rust
// tools/channel/registry.rs

use std::sync::Arc;
use crate::channel::traits::*;
use crate::tools::ToolCatalog;

/// Register all messaging tools for a platform
pub fn register_messaging_tools(
    catalog: &mut ToolCatalog,
    platform: &str,
    api: Arc<dyn MessagingApi>,
) {
    let send_name = format!("{}.messages.send", platform);
    let reply_name = format!("{}.messages.reply", platform);
    let get_name = format!("{}.messages.get", platform);
    let list_name = format!("{}.messages.history", platform);

    // Register send tool
    catalog.register_tool(
        send_name,
        move |params: serde_json::Value| {
            let api = api.clone();
            async move {
                let target = params["target"].as_str()
                    .ok_or("Missing target")?;
                let content = parse_message_content(&params)?;
                let message_id = api.send_message(target, &content).await?;
                Ok(serde_json::json!({ "message_id": message_id }))
            }
        }
    );

    // Register reply tool
    catalog.register_tool(
        reply_name,
        move |params: serde_json::Value| {
            let api = api.clone();
            async move {
                let parent_id = params["parent_id"].as_str()
                    .ok_or("Missing parent_id")?;
                let content = parse_message_content(&params)?;
                let message_id = api.reply(parent_id, &content).await?;
                Ok(serde_json::json!({ "message_id": message_id }))
            }
        }
    );

    // ... register get, list tools
}

/// Register document tools if platform supports DocumentsApi
pub fn register_document_tools(
    catalog: &mut ToolCatalog,
    platform: &str,
    api: Option<Arc<dyn DocumentsApi>>,
) {
    let Some(api) = api else {
        // Platform doesn't support documents, skip registration
        return;
    };

    let create_name = format!("{}.doc.create", platform);
    let read_name = format!("{}.doc.read", platform);
    let append_name = format!("{}.doc.append", platform);

    // Register create tool
    catalog.register_tool(
        create_name,
        move |params: serde_json::Value| {
            let api = api.clone();
            async move {
                let title = params["title"].as_str()
                    .ok_or("Missing title")?;
                let content = params["content"].as_str();
                let doc = api.create_document(title, content, None).await?;
                Ok(serde_json::to_value(&doc)?)
            }
        }
    );

    // ... register read, append tools
}

/// Register calendar tools if platform supports CalendarApi
pub fn register_calendar_tools(
    catalog: &mut ToolCatalog,
    platform: &str,
    api: Option<Arc<dyn CalendarApi>>,
) {
    let Some(api) = api else {
        // Platform doesn't support calendar, skip registration
        return;
    };

    let list_name = format!("{}.calendar.list", platform);
    let freebusy_name = format!("{}.calendar.freebusy", platform);

    // Register list tool
    catalog.register_tool(
        list_name,
        move |_params: serde_json::Value| {
            let api = api.clone();
            async move {
                let calendars = api.list_calendars().await?;
                Ok(serde_json::to_value(&calendars)?)
            }
        }
    );

    // ... register freebusy tool
}

/// Convenience function to register all tools for a platform
pub fn register_all_platform_tools(
    catalog: &mut ToolCatalog,
    platform: &str,
    api: Arc<dyn PlatformApi>,
) {
    // Register messaging tools if supported
    if let Some(messaging_api) = api.as_any().downcast_ref::<Arc<dyn MessagingApi>>() {
        register_messaging_tools(catalog, platform, messaging_api.clone());
    }

    // Register document tools if supported
    if let Some(doc_api) = api.as_any().downcast_ref::<Arc<dyn DocumentsApi>>() {
        register_document_tools(catalog, platform, Some(doc_api.clone()));
    } else {
        register_document_tools(catalog, platform, None);
    }

    // Register calendar tools if supported
    if let Some(cal_api) = api.as_any().downcast_ref::<Arc<dyn CalendarApi>>() {
        register_calendar_tools(catalog, platform, Some(cal_api.clone()));
    } else {
        register_calendar_tools(catalog, platform, None);
    }
}

/// Helper to parse MessageContent from JSON params
fn parse_message_content(params: &serde_json::Value) -> Result<MessageContent, String> {
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

    if content.text.is_none() && content.html.is_none() && content.markdown.is_none() 
        && content.image_key.is_none() && content.card.is_none() {
        return Err("Message must have text, html, markdown, image_key, or card".to_string());
    }

    Ok(content)
}
```

## Migration Plan

### Pre-Phase: Architecture Validation

**Goal**: Verify traits design works in isolation before touching production code

1. **Spike Implementation** (1-2 days):
   - Create traits in a temporary module (`channel/traits_experimental/`)
   - Implement for one simple channel (Telegram) only
   - Test tool registration with mock platform API
   - Verify dynamic dispatch works as expected
   - **Decision Point**: Proceed only if spike proves the approach

2. **Existing Code Audit**:
   - Document all current usages of `feishu::` imports in `tools/feishu.rs`
   - Document all current usages of `feishu::` imports in `channel/feishu/adapter.rs`
   - Identify cyclic dependencies to break
   - **Output**: Import dependency graph

### Phase 1: Create Traits Module

**Risk Level**: Low (pure addition, no changes to existing code)
**Duration**: 2-3 days

1. Create `channel/traits/` directory structure:
   ```
   channel/traits/
   ├── mod.rs       # Re-exports
   ├── error.rs     # ApiError, ApiResult
   ├── messaging.rs # MessagingApi trait
   ├── documents.rs # DocumentsApi trait
   └── calendar.rs  # CalendarApi trait
   ```

2. Implement traits with minimal associated types (as designed above)

3. Add `as_any()` method to `PlatformApi` trait for downcasting:
   ```rust
   pub trait PlatformApi: Send + Sync {
       fn as_any(&self) -> &dyn std::any::Any;
   }
   ```

4. **Verification**:
   ```bash
   cargo check --all-features  # Should compile, traits unused
   cargo test --features channel-traits  # If feature flag added
   ```

5. **Commit Point**: Traits module complete and compiling

### Phase 2: Telegram Restructure (Pilot)

**Risk Level**: Medium (validates the pattern)
**Duration**: 3-4 days

1. Create `channel/telegram/api/` directory:
   ```
   channel/telegram/
   ├── mod.rs          # TelegramAdapter (ChannelAdapter + MessagingApi impl)
   └── api/
       └── mod.rs      # Telegram client and API methods
   ```

2. Move HTTP client logic from `telegram.rs` to `telegram/api/mod.rs`

3. Implement `MessagingApi` for `TelegramClient` in `telegram/api/mod.rs`

4. Update `TelegramAdapter` in `telegram/mod.rs`:
   - Keep `ChannelAdapter` implementation (transport layer)
   - Delegate `MessagingApi` methods to inner `TelegramClient`
   - Or have `TelegramAdapter` implement both traits directly

5. **Verification**:
   ```bash
   cargo test --features channel-telegram
   cargo clippy --features channel-telegram -- -D warnings
   ```

6. **Commit Point**: Telegram restructure complete, tests passing

### Phase 3: Matrix Restructure

**Risk Level**: Medium (applies validated pattern)
**Duration**: 2-3 days

1. Follow same steps as Phase 2 for Matrix

2. **Leverage learnings**: Apply any refinements discovered during Telegram implementation

3. **Verification**:
   ```bash
   cargo test --features channel-matrix
   ```

4. **Commit Point**: Matrix restructure complete

### Phase 4: Feishu Consolidation (Complex)

**Risk Level**: High (largest change, bidirectional dependencies)
**Duration**: 5-7 days

**Goal**: Eliminate top-level `feishu/` module and `tools/feishu.rs`

#### Phase 4a: Move Feishu Code (Preparation)

1. **Preserve existing imports** by creating re-exports:
   - Update top-level `feishu/mod.rs` to re-export from new location:
     ```rust
     // feishu/mod.rs (temporary compatibility layer)
     pub use crate::channel::feishu::api::*;
     ```

2. **Move code** from `feishu/` to `channel/feishu/api/`:
   ```
   channel/feishu/
   ├── mod.rs              # FeishuAdapter (ChannelAdapter)
   ├── adapter.rs          # (moved from channel/feishu/adapter.rs)
   ├── api/                # (moved from feishu/)
   │   ├── mod.rs
   │   ├── client.rs
   │   ├── auth.rs
   │   ├── token_store.rs
   │   ├── principal.rs
   │   ├── runtime.rs
   │   ├── resources/
   │   └── error.rs
   ├── webhook.rs
   ├── websocket.rs
   └── payload/
   ```

3. **Update imports** within moved files to use new paths

4. **Verification**:
   ```bash
   cargo check --features feishu-integration,channel-feishu
   ```

#### Phase 4b: Implement PlatformApi Traits

1. Implement `MessagingApi` for `FeishuClient`:
   ```rust
   impl MessagingApi for FeishuClient {
       // Delegate to existing methods in resources/messages.rs
   }
   ```

2. Implement `DocumentsApi` for `FeishuClient`

3. Implement `CalendarApi` for `FeishuClient`

4. Have `FeishuAdapter` implement `ChannelAdapter` + `PlatformApi` marker

#### Phase 4c: Migrate Tools

1. Create `tools/channel/feishu.rs` with Feishu-specific tool implementations

2. Move generic functionality to `tools/channel/generic.rs`

3. **Gradual migration**:
   - Keep `tools/feishu.rs` temporarily
   - Have it delegate to new implementations
   - Update tests to use new paths

4. **Verification**:
   ```bash
   cargo test --features feishu-integration,channel-feishu,tool-feishu
   ```

#### Phase 4d: Cleanup

1. Remove top-level `feishu/` directory
2. Remove `tools/feishu.rs`
3. Update `lib.rs` to remove `pub mod feishu`
4. **Verification**:
   ```bash
   cargo test --all-features
   ```

5. **Commit Point**: Feishu consolidation complete

### Phase 5: Generic Tool Registration

**Risk Level**: Medium (new registration pattern)
**Duration**: 3-4 days

1. Create `tools/channel/mod.rs` with:
   - `generic.rs` - Dynamic dispatch tool handlers
   - `registry.rs` - Registration functions
   - `telegram.rs` - Telegram-specific wrappers (if any)
   - `matrix.rs` - Matrix-specific wrappers (if any)
   - `feishu.rs` - Feishu-specific wrappers

2. Implement `register_all_platform_tools()` for each channel

3. Update main tool catalog initialization:
   ```rust
   // In tool catalog setup
   if let Some(telegram) = registry.get_telegram_adapter() {
       tools::channel::register_all_platform_tools(
           &mut catalog,
           "telegram",
           telegram as Arc<dyn PlatformApi>
       );
   }
   ```

4. **Backward compatibility**: Ensure tool names unchanged:
   - `telegram.messages.send` → `telegram.messages.send`
   - `feishu.messages.send` → `feishu.messages.send`
   - `feishu.doc.create` → `feishu.doc.create`

5. **Verification**:
   ```bash
   cargo test --all-features
   # Integration tests: verify tool responses match pre-migration
   ```

6. **Commit Point**: Generic tool registration working

### Phase 6: Integration & Validation

**Risk Level**: Low (verification only)
**Duration**: 2-3 days

1. **Full test suite**:
   ```bash
   cargo test --workspace --all-features
   ```

2. **Linting**:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   ```

3. **Feature flag combinations**:
   ```bash
   cargo check --features channel-telegram
   cargo check --features channel-matrix
   cargo check --features channel-feishu
   cargo check --features channel-telegram,channel-feishu
   cargo check --all-features
   ```

4. **Integration tests**:
   - End-to-end tests with mocked platform APIs
   - Tool name verification
   - Response format verification

5. **Documentation**:
   - Update ARCHITECTURE.md with new structure
   - Update API docs for new traits

6. **Final Commit Point**: Migration complete

## Backward Compatibility

### Public Interface Compatibility

| Aspect | Current | New | Strategy |
|--------|---------|-----|----------|
| **Tool Names** | `feishu.messages.send` | Same | Registration maps to same names |
| **Tool Names** | `feishu.doc.create` | Same | Registration maps to same names |
| **Tool Names** | `telegram.messages.send` | Same | Registration maps to same names |
| **API Response Fields** | Platform-specific JSON | Same fields | Concrete structs with `platform_metadata` for extras |
| **Configuration** | TOML files | Same | No changes to config parsing |
| **Feature Flags** | `channel-telegram`, `feishu-integration` | Same | Flags control compilation |
| **Library API** | `feishu::FeishuClient` | `channel::feishu::api::FeishuClient` | Re-export during transition |

### Module Path Compatibility

**During Migration** (temporary re-exports):
```rust
// feishu/mod.rs (compatibility layer)
#![deprecated(since = "0.x.0", note = "Use channel::feishu::api instead")]
pub use crate::channel::feishu::api::*;
```

**After Migration**:
- Remove top-level `feishu` module
- All code imports from `channel::feishu::api`

### Breaking Changes (Documented)

| Change | Mitigation | Timeline |
|--------|-----------|----------|
| Module path `feishu::` → `channel::feishu::api::` | Deprecation warning in v0.x | Remove in v0.(x+1) |
| Internal trait bounds | N/A (internal API) | Immediate |

### Migration Path for Users

**No action required** for:
- Configuration files
- Tool invocations (CLI or programmatic)
- Feature flags

**Action required** for direct library users:
```rust
// Before
use loongclaw::feishu::FeishuClient;

// After
use loongclaw::channel::feishu::api::FeishuClient;
// Or during transition:
use loongclaw::feishu::FeishuClient; // With deprecation warning
```

## Open Questions

### 1. Platform-Specific Features

**Problem**: Each platform has unique capabilities:
- **Feishu**: Card updates, custom emoji reactions, approval workflows
- **Telegram**: Inline keyboards, callback queries, bot commands
- **Matrix**: Room encryption, mentions, threading
- **Slack** (future): Blocks, shortcuts, workflows

**Options**:

**Option A**: Extension Traits with Default Impls
```rust
#[async_trait]
pub trait CardSupport: MessagingApi {
    async fn update_card(&self, card_id: &str, card: &Value) -> ApiResult<()> {
        Err(ApiError::NotSupported { operation: "update_card".to_string(), platform: "generic".to_string() })
    }
}

// Feishu implements with actual functionality
#[async_trait]
impl CardSupport for FeishuClient {
    async fn update_card(&self, card_id: &str, card: &Value) -> ApiResult<()> {
        // Actual implementation
    }
}
```
**Pros**: Type-safe, discoverable, can check support at compile time
**Cons**: More traits to implement, proliferation of trait bounds

**Option B**: Platform-Specific Tools
```rust
// tools/channel/feishu.rs
pub async fn update_card(feishu: &FeishuClient, card_id: &str, card: &Value) -> ApiResult<()> {
    feishu.update_card(card_id, card).await
}

// Register only for Feishu
fn register_feishu_specific_tools(catalog: &mut ToolCatalog, feishu: Arc<FeishuClient>) {
    catalog.register("feishu.card.update", move |params| {
        let feishu = feishu.clone();
        async move { update_card(&feishu, ...).await }
    });
}
```
**Pros**: Simple, no trait complexity, clear separation
**Cons**: Less generic, platform-specific code scattered

**Option C**: Generic Tools with Platform Detection
```rust
pub async fn update_card(api: &dyn PlatformApi, card_id: &str, card: &Value) -> ApiResult<()> {
    if let Some(feishu) = api.as_any().downcast_ref::<FeishuClient>() {
        feishu.update_card(card_id, card).await
    } else {
        Err(ApiError::NotSupported { ... })
    }
}
```
**Pros**: Unified interface, runtime detection
**Cons**: Loses type safety, downcasting overhead, less discoverable

**Recommendation**: **Option A (Extension Traits)** for features shared by multiple platforms (e.g., Cards supported by Feishu and Slack), **Option B (Platform-Specific)** for truly unique features.

### 2. Authentication Abstraction

**Current State**: Auth is platform-specific:
- Feishu: OAuth 2.0, user tokens, tenant tokens, token refresh
- Telegram: Simple bot token
- Matrix: Access token + homeserver URL

**Question**: Should we abstract authentication into traits?

**Analysis**:
- Auth is primarily an implementation detail of the client
- Token refresh is handled internally by FeishuClient
- Users don't interact with auth directly (configured in TOML)

**Recommendation**: Keep auth platform-specific. If cross-platform auth patterns emerge (e.g., OAuth 2.0 for multiple platforms), introduce `AuthProvider` trait later.

### 3. Rate Limiting and Retry Strategy

**Current State**: Each client implements its own retry logic (Feishu has `FeishuRetryPolicy`)

**Options**:

**Option A**: Trait-Level Default
```rust
#[async_trait]
pub trait PlatformApi: Send + Sync {
    fn retry_policy(&self) -> Box<dyn RetryPolicy> {
        Box::new(DefaultRetryPolicy)
    }
}
```

**Option B**: Middleware Layer
```rust
pub struct RateLimitedClient<C> {
    inner: C,
    rate_limiter: RateLimiter,
}

#[async_trait]
impl<C: MessagingApi> MessagingApi for RateLimitedClient<C> {
    // Wrap all calls with rate limiting
}
```

**Option C**: HTTP Client Configuration
Keep retry logic in the HTTP client layer (reqwest middleware)

**Recommendation**: **Option C** - Use reqwest middleware (e.g., `reqwest-retry`) for consistent retry behavior across all platforms. Platform-specific retry policies (like Feishu's exponential backoff) can be configured per-client.

### 4. Event/Callback System

**Problem**: Incoming events (webhooks, WebSocket messages) need to be routed to appropriate handlers.

**Current State**: Adapters poll for messages (`receive_batch()`)

**Question**: Should we add an event-driven trait?

```rust
#[async_trait]
pub trait EventSource: Send + Sync {
    type Event;
    
    async fn next_event(&mut self) -> ApiResult<Self::Event>;
    async fn ack_event(&mut self, event: &Self::Event) -> ApiResult<()>;
}
```

**Recommendation**: Out of scope for this refactoring. Current polling model (`receive_batch()`) is sufficient. Event-driven architecture can be explored separately if needed for push-based platforms (e.g., WebSocket-heavy platforms).

### 5. Testing Strategy for Platform APIs

**Question**: How to test tools that depend on PlatformApi traits?

**Option A**: Mock Implementations
```rust
pub struct MockMessagingApi {
    sent_messages: Arc<Mutex<Vec<MessageContent>>>,
}

#[async_trait]
impl MessagingApi for MockMessagingApi {
    async fn send_message(&self, target: &str, content: &MessageContent) -> ApiResult<String> {
        self.sent_messages.lock().unwrap().push(content.clone());
        Ok("mock-message-id".to_string())
    }
    // ...
}
```

**Option B**: Record/Replay (VCR-style)
Use `mockito` or similar to record real API responses and replay in tests.

**Option C**: Integration Tests Only
Test against real sandbox environments.

**Recommendation**: **Option A (Mock)** for unit tests, **Option B (Record/Replay)** for integration tests. Real sandbox tests only for release validation.

## Risk Assessment

### Phase Risk Summary

| Phase | Risk Level | Primary Risk | Mitigation |
|-------|------------|--------------|------------|
| Pre-Phase | Low | Design flaws discovered late | Spike implementation validates approach |
| Phase 1 | Low | Trait design doesn't fit use cases | Review with stakeholders before proceeding |
| Phase 2 | Medium | Telegram-specific edge cases | Extensive testing, isolated changes |
| Phase 3 | Medium | Matrix-specific edge cases | Apply Phase 2 learnings |
| Phase 4a | High | Import cycle breakage | Gradual migration with re-exports |
| Phase 4b | Medium | Trait implementation gaps | Comprehensive test coverage |
| Phase 4c | High | Tool functionality regression | Side-by-side testing |
| Phase 4d | Medium | Cleanup breaks builds | CI verification at each step |
| Phase 5 | Medium | Registration pattern issues | Feature flag fallback |
| Phase 6 | Low | Undetected integration issues | Full test matrix |

### Major Risks and Mitigations

#### Risk 1: Dynamic Dispatch Overhead
**Concern**: Using `dyn Trait` instead of generics may impact performance

**Mitigation**:
- Trait objects are invoked at tool call boundaries (infrequent relative to message processing)
- Measure before optimizing: benchmark tool registration and invocation
- Can migrate to static dispatch later if needed (generics or enum dispatch)

#### Risk 2: Feishu Import Cycles
**Concern**: Moving `feishu/` to `channel/feishu/api/` creates circular dependencies

**Current Dependencies**:
```
tools/feishu.rs → feishu::client
channel/feishu/adapter.rs → feishu::client
feishu/ → (no reverse dependencies)
```

**Mitigation**:
- Move files atomically with `git mv` to preserve history
- Use re-exports to maintain backward compatibility during transition
- Update imports incrementally: `feishu::` → `channel::feishu::api::`

#### Risk 3: Tool Behavior Changes
**Concern**: Refactored tools produce different outputs

**Mitigation**:
- Snapshot testing: capture tool outputs before and after
- Integration tests verify response format compatibility
- Keep old implementation side-by-side during transition

#### Risk 4: Feature Flag Complexity
**Concern**: New feature flags interact unpredictably

**Mitigation**:
- CI tests all flag combinations
- Document flag dependencies in `Cargo.toml`
- Avoid nested feature flags

#### Risk 5: Compile Time Increase
**Concern**: Additional traits and dynamic dispatch increase compile times

**Mitigation**:
- Monitor compile times in CI
- Consider `dyn-clone` or similar only if needed
- Review trait object usage for unnecessary allocations

## Testing Strategy

### Testing Layers

```
┌─────────────────────────────────────────────────────────┐
│  Layer 4: Integration Tests                              │
│  - Mock platform APIs (mockito/httpmock)                 │
│  - End-to-end tool workflows                             │
│  - Response format verification                          │
├─────────────────────────────────────────────────────────┤
│  Layer 3: Adapter Tests                                  │
│  - ChannelAdapter trait implementations                  │
│  - PlatformApi trait implementations                     │
│  - Transport layer (webhook/websocket/polling)           │
├─────────────────────────────────────────────────────────┤
│  Layer 2: Trait Tests                                    │
│  - Mock trait implementations                            │
│  - Tool handler logic with mock APIs                     │
│  - Registration pattern verification                     │
├─────────────────────────────────────────────────────────┤
│  Layer 1: Unit Tests                                     │
│  - Serialization/deserialization                         │
│  - Error handling                                        │
│  - Pagination logic                                      │
└─────────────────────────────────────────────────────────┘
```

### Test Implementation

#### Layer 1: Unit Tests
```rust
// channel/traits/messaging.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_content_defaults() {
        let content = MessageContent::default();
        assert!(content.text.is_none());
        assert!(content.html.is_none());
    }

    #[test]
    fn pagination_cursor_serialization() {
        let pagination = Pagination {
            page_size: 50,
            cursor: Some("abc123".to_string()),
            page: None,
        };
        let json = serde_json::to_string(&pagination).unwrap();
        assert!(json.contains("abc123"));
    }
}
```

#### Layer 2: Trait Tests with Mocks
```rust
// tools/channel/tests/messaging_tests.rs
use crate::channel::traits::*;
use crate::tools::channel::generic::*;
use std::sync::{Arc, Mutex};

struct MockMessagingApi {
    sent_messages: Arc<Mutex<Vec<(String, MessageContent)>>>,
}

#[async_trait]
impl MessagingApi for MockMessagingApi {
    async fn send_message(
        &self,
        target: &str,
        content: &MessageContent,
    ) -> ApiResult<String> {
        self.sent_messages.lock().unwrap().push((
            target.to_string(),
            content.clone(),
        ));
        Ok("mock-id".to_string())
    }

    // ... other methods
}

#[tokio::test]
async fn test_send_message_handler() {
    let mock = Arc::new(MockMessagingApi {
        sent_messages: Arc::new(Mutex::new(Vec::new())),
    });

    let content = MessageContent {
        text: Some("Hello".to_string()),
        ..Default::default()
    };

    let result = send_message(mock.as_ref(), "chat123", &content).await;
    assert!(result.is_ok());

    let messages = mock.sent_messages.lock().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].0, "chat123");
}
```

#### Layer 3: Adapter Tests
```rust
// channel/telegram/tests/adapter_tests.rs
use crate::channel::telegram::TelegramAdapter;

#[tokio::test]
async fn test_telegram_adapter_implements_messaging_api() {
    let adapter = create_test_adapter().await;

    // Test ChannelAdapter trait
    let batch = adapter.receive_batch().await.unwrap();
    assert!(batch.is_empty() || !batch.is_empty()); // Basic smoke test

    // Test MessagingApi trait via dynamic dispatch
    let api: &dyn MessagingApi = &adapter;
    let content = MessageContent {
        text: Some("Test".to_string()),
        ..Default::default()
    };

    // This would need mocking the HTTP layer
    // let result = api.send_message("12345", &content).await;
}
```

#### Layer 4: Integration Tests
```rust
// tests/channel_integration_tests.rs
use loongclaw::tools::ToolCatalog;
use loongclaw::channel::registry::ChannelRegistry;

#[tokio::test]
async fn test_telegram_tool_end_to_end() {
    // Setup mock server
    let mut server = mockito::Server::new_async().await;
    let mock = server.mock("POST", "/botTOKEN/sendMessage")
        .with_status(200)
        .with_body(r#"{"ok":true,"result":{"message_id":123}}"#)
        .create();

    // Create adapter pointing to mock server
    let config = create_test_config(server.url());
    let adapter = TelegramAdapter::new(&config, "TOKEN".to_string());

    // Register tools
    let mut catalog = ToolCatalog::new();
    register_telegram_tools(&mut catalog, Arc::new(adapter));

    // Invoke tool
    let result = catalog.invoke("telegram.messages.send", json!({
        "target": "12345",
        "text": "Hello World"
    })).await;

    assert!(result.is_ok());
    mock.assert();
}
```

### Migration Testing

#### Before/After Comparison
```bash
# Capture current tool outputs
./scripts/capture_tool_outputs.sh > before_snapshot.json

# After migration
./scripts/capture_tool_outputs.sh > after_snapshot.json

# Compare
diff before_snapshot.json after_snapshot.json
```

#### Continuous Integration Matrix
```yaml
# .github/workflows/test.yml
strategy:
  matrix:
    features:
      - channel-telegram
      - channel-matrix
      - channel-feishu
      - channel-telegram,channel-feishu
      - all-features
```

### Test Coverage Goals

| Component | Target Coverage | Priority |
|-----------|----------------|----------|
| channel/traits | 90% | High |
| channel/telegram/api | 80% | High |
| channel/matrix/api | 80% | High |
| channel/feishu/api | 80% | High |
| tools/channel/generic | 85% | High |
| tools/channel/registry | 85% | High |
| Adapter implementations | 70% | Medium |
| Integration tests | Key paths | Medium |

## References

| Component | Location |
|-----------|----------|
| ChannelAdapter trait | `crates/app/src/channel/mod.rs` |
| Current FeishuClient | `crates/app/src/feishu/client.rs` (to be moved) |
| Current Feishu tools | `crates/app/src/tools/feishu.rs` (to be deleted) |
| Current Telegram adapter | `crates/app/src/channel/telegram.rs` (to be restructured) |
| Current Matrix adapter | `crates/app/src/channel/matrix.rs` (to be restructured) |

## References

| Component | Location |
|-----------|----------|
| `ChannelAdapter` trait | `crates/app/src/channel/mod.rs:548` |
| Current Feishu client | `crates/app/src/feishu/client.rs` |
| Current Feishu tools | `crates/app/src/tools/feishu.rs` |
| Current Telegram adapter | `crates/app/src/channel/telegram.rs` |
| Current Matrix adapter | `crates/app/src/channel/matrix.rs` |
| Architecture Contract | `ARCHITECTURE.md` |
| Core Beliefs | `docs/design-docs/core-beliefs.md` |
| Layered Kernel Design | `docs/design-docs/layered-kernel-design.md` |

## Appendix: Implementation Checklist

### Pre-Implementation
- [ ] Review this design with team
- [ ] Complete spike implementation (Pre-Phase)
- [ ] Set up feature branch: `feat/channel-api-abstraction`
- [ ] Create tracking issue for migration

### Phase Completion
- [ ] Phase 0: Architecture validation complete
- [ ] Phase 1: Traits module merged to main
- [ ] Phase 2: Telegram restructure merged
- [ ] Phase 3: Matrix restructure merged
- [ ] Phase 4: Feishu consolidation merged
- [ ] Phase 5: Generic tool registration merged
- [ ] Phase 6: Full integration testing passed

### Post-Implementation
- [ ] Update ARCHITECTURE.md
- [ ] Update API documentation
- [ ] Migration guide for library users
- [ ] Performance benchmarks (before/after)
- [ ] Remove deprecated re-exports (v0.x.1)

## Changelog

| Date | Version | Changes |
|------|---------|---------|
| 2026-03-21 | 0.1.0 | Initial draft |
| 2026-03-21 | 0.2.0 | Incorporated review feedback: simplified traits (removed associated types), clarified ChannelAdapter/PlatformApi separation, changed `impl/` to `api/`, detailed Feishu migration plan, added dynamic dispatch pattern for tools, expanded testing strategy, updated risk assessment |
