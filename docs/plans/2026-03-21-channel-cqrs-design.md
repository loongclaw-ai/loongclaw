# Channel Architecture Redesign: Event Sourcing + CQRS

## Status

Design - Ready for Implementation

## Summary

Complete architectural overhaul of the channel integration layer using **Event Sourcing** and **CQRS** patterns. This design transforms the 8,000+ lines of tightly-coupled platform-specific code into a decoupled, command-driven system where:

- **Tools** issue commands without knowing platform details
- **Command Bus** routes commands to appropriate handlers  
- **Event Store** records all operations for audit and replay
- **Projections** provide read-optimized views for tools

This architecture eliminates the 3,341-line `tools/feishu.rs` monolith and enables true cross-platform capability composition.

## Problem Statement

### Current Pain Points

| Metric | Current State | Impact |
|--------|--------------|--------|
| `tools/feishu.rs` | 3,341 lines | Unmaintainable, tests difficult |
| `channel/mod.rs` | 3,430 lines | God module, too many responsibilities |
| `feishu/client.rs` | 1,276 lines | Mixed concerns (auth, transport, business logic) |
| Platform coupling | Direct | Adding Slack requires touching tools layer |
| Test complexity | High | Mocking entire FeishuClient for one test |
| Cross-platform ops | Impossible | Cannot send Telegram + update Feishu doc atomically |

### Root Cause Analysis

**Current Architecture (Tight Coupling)**:
```
Tool → FeishuClient → HTTP → Feishu API
         ↓
    TokenStore, Auth, Error Handling, Retry Logic
         ↓
    All mixed in 3,341 lines
```

The fundamental issue: **Business logic (tools) depends on implementation details (HTTP clients, auth flows)**.

## Design Goals

1. **Complete Decoupling**: Tools depend only on commands, not platforms
2. **Single Source of Truth**: Event store records every operation
3. **Auditability**: Full history of all channel operations
4. **Testability**: Test tools with in-memory command bus
5. **Composability**: Cross-platform operations as command chains
6. **Scalability**: Event handlers can be distributed

## Architecture Overview

### Core Concepts

```
┌─────────────────────────────────────────────────────────────────────┐
│                           Command Layer                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │
│  │  SendMsg    │  │ CreateDoc   │  │ QueryCal    │  ← Domain Cmds   │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘                  │
└─────────┼────────────────┼────────────────┼─────────────────────────┘
          │                │                │
          └────────────────┴────────────────┘
                           │
          ┌────────────────┴────────────────┐
          ▼                                 ▼
┌──────────────────────┐      ┌──────────────────────┐
│   Command Bus        │      │   Event Store        │
│  ┌────────────────┐  │      │  ┌────────────────┐  │
│  │ Router         │  │      │  │ MessageSent    │  │
│  │ Handler Reg.   │──┼──────┼─▶│ DocCreated     │  │
│  │ Validation     │  │      │  │ CalendarQueried│  │
│  └────────────────┘  │      │  └────────────────┘  │
└──────────┬───────────┘      └──────────────────────┘
           │
           ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        Handler Layer                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │
│  │TelegramHdlr │  │ MatrixHdlr  │  │ FeishuHdlr  │  ← Impl Details  │
│  │ - HTTP      │  │ - HTTP      │  │ - HTTP      │                  │
│  │ - Auth      │  │ - Auth      │  │ - Auth      │                  │
│  │ - Retry     │  │ - Retry     │  │ - Retry     │                  │
│  └─────────────┘  └─────────────┘  └─────────────┘                  │
└─────────────────────────────────────────────────────────────────────┘
           │
           ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        Projection Layer                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │
│  │MessageView  │  │DocumentView │  │CalendarView │  ← Read Models   │
│  │(for tools)  │  │(for tools)  │  │(for tools)  │                  │
│  └─────────────┘  └─────────────┘  └─────────────┘                  │
└─────────────────────────────────────────────────────────────────────┘
```

### Key Components

#### 1. Command Layer
Commands represent **intent** - what the user wants to do, not how to do it.

```rust
// channel/commands/mod.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChannelCommand {
    // Messaging
    SendMessage(SendMessageCommand),
    ReplyToMessage(ReplyToMessageCommand),
    GetMessage(GetMessageCommand),
    ListMessages(ListMessagesCommand),
    
    // Documents
    CreateDocument(CreateDocumentCommand),
    ReadDocument(ReadDocumentCommand),
    AppendToDocument(AppendToDocumentCommand),
    
    // Calendar
    ListCalendars(ListCalendarsCommand),
    QueryFreebusy(QueryFreebusyCommand),
    CreateEvent(CreateEventCommand),
}

#[derive(Debug, Clone)]
pub struct SendMessageCommand {
    pub platform: PlatformId,
    pub target: TargetId,
    pub content: MessageContent,
    pub reply_to: Option<MessageId>,
    pub idempotency_key: Option<String>,
}
```

#### 2. Event Layer  
Events represent **facts** - what actually happened.

```rust
// channel/events/mod.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChannelEvent {
    // Messaging events
    MessageSent(MessageSentEvent),
    MessageReceived(MessageReceivedEvent),
    MessageRead(MessageReadEvent),
    
    // Document events
    DocumentCreated(DocumentCreatedEvent),
    DocumentUpdated(DocumentUpdatedEvent),
    
    // Calendar events
    CalendarListed(CalendarListedEvent),
    FreebusyQueried(FreebusyQueriedEvent),
    
    // Error events
    CommandFailed(CommandFailedEvent),
}

#[derive(Debug, Clone)]
pub struct MessageSentEvent {
    pub platform: PlatformId,
    pub command_id: CommandId,
    pub message_id: MessageId,
    pub target: TargetId,
    pub timestamp: DateTime<Utc>,
    pub platform_metadata: serde_json::Value,
}
```

#### 3. Command Bus
The central router that validates and dispatches commands.

```rust
// channel/command_bus.rs
pub struct CommandBus {
    handlers: HashMap<CapabilityType, Box<dyn CommandHandler>>,
    event_store: Arc<dyn EventStore>,
    validator: CommandValidator,
}

impl CommandBus {
    pub async fn dispatch(&self, cmd: ChannelCommand) -> Result<CommandResult, CommandError> {
        // 1. Validate command
        self.validator.validate(&cmd).await?;
        
        // 2. Generate command ID for idempotency
        let cmd_id = CommandId::new();
        
        // 3. Route to handler
        let handler = self.handlers.get(&cmd.capability_type())
            .ok_or(CommandError::NoHandler)?;
        
        // 4. Execute
        let events = handler.handle(cmd_id.clone(), cmd).await?;
        
        // 5. Store events
        for event in &events {
            self.event_store.append(event).await?;
        }
        
        // 6. Return result
        Ok(CommandResult { command_id: cmd_id, events })
    }
}
```

#### 4. Command Handlers
Platform-specific implementations that translate commands to API calls.

```rust
// channel/handlers/feishu.rs
pub struct FeishuCommandHandler {
    client: FeishuClient,
    config: FeishuHandlerConfig,
}

#[async_trait]
impl CommandHandler for FeishuCommandHandler {
    async fn handle(
        &self,
        cmd_id: CommandId,
        cmd: ChannelCommand,
    ) -> Result<Vec<ChannelEvent>, HandlerError> {
        match cmd {
            ChannelCommand::SendMessage(send_cmd) => {
                self.handle_send_message(cmd_id, send_cmd).await
            }
            ChannelCommand::CreateDocument(doc_cmd) => {
                self.handle_create_document(cmd_id, doc_cmd).await
            }
            // ... other commands
            _ => Err(HandlerError::UnsupportedCommand),
        }
    }
}

impl FeishuCommandHandler {
    async fn handle_send_message(
        &self,
        cmd_id: CommandId,
        cmd: SendMessageCommand,
    ) -> Result<Vec<ChannelEvent>, HandlerError> {
        // 1. Get tenant token
        let token = self.client.get_tenant_access_token().await?;
        
        // 2. Call Feishu API
        let response = self.client
            .send_message(&token, &cmd.target, &cmd.content)
            .await?;
        
        // 3. Return event
        Ok(vec![ChannelEvent::MessageSent(MessageSentEvent {
            platform: PlatformId::feishu(),
            command_id: cmd_id,
            message_id: response.message_id.into(),
            target: cmd.target,
            timestamp: Utc::now(),
            platform_metadata: serde_json::to_value(response)?,
        })])
    }
}
```

#### 5. Event Store
Persists all events for audit and replay capabilities.

```rust
// channel/event_store/mod.rs
#[async_trait]
pub trait EventStore: Send + Sync {
    async fn append(&self, event: &ChannelEvent) -> Result<EventId, EventStoreError>;
    async fn get_events(
        &self,
        filter: EventFilter,
    ) -> Result<Vec<ChannelEvent>, EventStoreError>;
    async fn get_stream(
        &self,
        stream_id: StreamId,
    ) -> Result<Vec<ChannelEvent>, EventStoreError>;
}

// SQLite implementation for local development
pub struct SqliteEventStore {
    pool: SqlitePool,
}

// In-memory implementation for testing
pub struct InMemoryEventStore {
    events: Mutex<Vec<ChannelEvent>>,
}
```

#### 6. Projections (Read Models)
Optimized views for tools to query state.

```rust
// channel/projections/messages.rs
pub struct MessageProjection {
    event_store: Arc<dyn EventStore>,
    cache: DashMap<MessageId, MessageView>,
}

impl MessageProjection {
    pub async fn get_message(&self, id: &MessageId) -> Option<MessageView> {
        // Check cache first
        if let Some(msg) = self.cache.get(id) {
            return Some(msg.clone());
        }
        
        // Rebuild from events
        let events = self.event_store
            .get_events(EventFilter::for_message(id))
            .await
            .ok()?;
        
        let view = self.rebuild_message_view(&events)?;
        self.cache.insert(id.clone(), view.clone());
        Some(view)
    }
    
    pub async fn list_messages(&self, filter: MessageFilter) -> Vec<MessageView> {
        // Query events and project
        let events = self.event_store
            .get_events(filter.into())
            .await
            .unwrap_or_default();
        
        self.project_messages(&events)
    }
}

#[derive(Debug, Clone)]
pub struct MessageView {
    pub id: MessageId,
    pub platform: PlatformId,
    pub content: MessageContent,
    pub status: MessageStatus,
    pub created_at: DateTime<Utc>,
}
```

### Tools Layer (Completely Rewritten)

Tools now interact only with the command bus and projections:

```rust
// tools/channel/messaging.rs
pub struct MessagingTool {
    command_bus: Arc<CommandBus>,
    projection: Arc<MessageProjection>,
}

impl MessagingTool {
    pub async fn send(
        &self,
        platform: &str,
        target: &str,
        content: MessageContent,
    ) -> Result<ToolOutcome, ToolError> {
        // Build command
        let cmd = ChannelCommand::SendMessage(SendMessageCommand {
            platform: PlatformId::from(platform)?,
            target: TargetId::from(target)?,
            content,
            reply_to: None,
            idempotency_key: Some(generate_idempotency_key()),
        });
        
        // Dispatch
        let result = self.command_bus.dispatch(cmd).await
            .map_err(|e| ToolError::CommandFailed(e.to_string()))?;
        
        // Extract result from events
        let message_id = result.events.iter()
            .find_map(|e| match e {
                ChannelEvent::MessageSent(evt) => Some(evt.message_id.clone()),
                _ => None,
            })
            .ok_or(ToolError::NoResultEvent)?;
        
        Ok(ToolOutcome::success(json!({
            "message_id": message_id,
            "platform": platform,
        })))
    }
    
    pub async fn get(&self, message_id: &str) -> Result<ToolOutcome, ToolError> {
        let view = self.projection
            .get_message(&MessageId::from(message_id)?)
            .await
            .ok_or(ToolError::NotFound)?;
        
        Ok(ToolOutcome::success(json!({
            "message": view,
        })))
    }
}
```

## Directory Structure

```
crates/app/src/channel/
├── mod.rs                      # Re-exports, backward compatibility
├── lib.rs                      # Public API
│
├── commands/                   # Command definitions
│   ├── mod.rs                  # ChannelCommand enum
│   ├── messaging.rs            # SendMessage, ReplyToMessage, etc.
│   ├── documents.rs            # CreateDocument, ReadDocument, etc.
│   ├── calendar.rs             # ListCalendars, QueryFreebusy, etc.
│   └── validation.rs           # Command validation rules
│
├── events/                     # Event definitions
│   ├── mod.rs                  # ChannelEvent enum
│   ├── messaging.rs            # MessageSent, MessageReceived, etc.
│   ├── documents.rs            # DocumentCreated, etc.
│   ├── calendar.rs             # CalendarListed, etc.
│   └── errors.rs               # CommandFailed event
│
├── command_bus.rs              # Central command dispatcher
├── command_handler.rs          # CommandHandler trait
│
├── handlers/                   # Platform-specific handlers
│   ├── mod.rs                  # Handler registration
│   ├── traits.rs               # Handler capability traits
│   ├── telegram.rs             # Telegram command handler
│   ├── matrix.rs               # Matrix command handler
│   └── feishu.rs               # Feishu command handler
│
├── event_store/                # Event persistence
│   ├── mod.rs                  # EventStore trait
│   ├── memory.rs               # In-memory implementation (testing)
│   ├── sqlite.rs               # SQLite implementation
│   └── migration.rs            # Schema migrations
│
├── projections/                # Read model projections
│   ├── mod.rs                  # Projection traits
│   ├── messages.rs             # Message projection
│   ├── documents.rs            # Document projection
│   └── calendar.rs             # Calendar projection
│
├── types.rs                    # Shared types (CommandId, EventId, etc.)
└── error.rs                    # Error types

crates/app/src/tools/channel/   # NEW: Tools use command bus
├── mod.rs                      # Module exports
├── messaging.rs                # 200 lines (was 3341)
├── documents.rs                # Document tools
├── calendar.rs                 # Calendar tools
├── registry.rs                 # Tool registration
└── tests/                      # Tool tests (in-memory bus)
    ├── messaging_tests.rs
    └── mod.rs

# DEPRECATED (migrate content then delete):
crates/app/src/tools/feishu.rs          # → split into handlers/feishu.rs + tools/channel/*.rs
crates/app/src/feishu/                  # → handlers/feishu.rs
```

## Benefits Over Original Design

### 1. Testability Improvement

**Before**: Testing `feishu.messages.send` required mocking entire FeishuClient:
```rust
// Complex mock setup
let mut mock_client = MockFeishuClient::new();
mock_client.expect_send_message()
    .with(...)
    .returning(...);
// 50 lines of setup for 1 test
```

**After**: Test with in-memory command bus:
```rust
let bus = CommandBus::in_memory();
bus.register_handler("feishu", MockHandler::new());

let tool = MessagingTool::new(bus);
let result = tool.send("feishu", "target", content).await;

assert_eq!(bus.events().len(), 1);
```

### 2. Cross-Platform Operations

**Before**: Impossible to compose operations across platforms atomically.

**After**: Command chains with compensation:
```rust
// Send message to Telegram, create doc in Feishu atomically
let cmd = CompoundCommand::new()
    .add(ChannelCommand::SendMessage(telegram_msg))
    .add(ChannelCommand::CreateDocument(feishu_doc))
    .on_failure(|evt| {
        // Rollback: delete doc if message failed
    });

bus.dispatch_compound(cmd).await?;
```

### 3. Audit Trail

Every operation is recorded as events:
```sql
SELECT * FROM events 
WHERE platform = 'feishu' 
  AND timestamp > '2026-03-21'
ORDER BY timestamp;

-- Returns:
-- MessageSent { user: "alice", target: "group123", ... }
-- DocumentCreated { user: "alice", title: "Notes", ... }
-- etc.
```

### 4. Incremental Migration

Can migrate one capability at a time:
1. Phase 1: Messaging (commands + handler)
2. Phase 2: Documents
3. Phase 3: Calendar
4. Phase 4: Remove old code

Each phase adds value without breaking existing functionality.

## Migration Strategy

### Phase 1: Foundation (Week 1)

**Goal**: Command bus + in-memory event store + 1 test handler

1. Create `channel/commands/` and `channel/events/` modules
2. Implement `CommandBus` with in-memory event store
3. Create `CommandHandler` trait
4. Write tests demonstrating command → event flow

**Validation**:
```bash
cargo test -p loongclaw-app channel::commands
cargo test -p loongclaw-app channel::events
```

### Phase 2: Telegram Handler (Week 2)

**Goal**: First real platform handler

1. Implement `TelegramCommandHandler`
2. Handle `SendMessage` command
3. Tests with mock Telegram API
4. Create `MessagingTool` facade

**Validation**:
```bash
cargo test -p loongclaw-app --features channel-telegram
```

### Phase 3: Feishu Handler (Week 3-4)

**Goal**: Migrate Feishu functionality incrementally

1. Create `FeishuCommandHandler`
2. Migrate messaging commands first
3. Migrate document commands
4. Migrate calendar commands
5. Each migration: old code delegates to new handler

**Validation**:
```bash
cargo test -p loongclaw-app --features feishu-integration
```

### Phase 4: Tools Migration (Week 5)

**Goal**: Rewrite tools layer

1. Create `tools/channel/messaging.rs` (200 lines)
2. Migrate `feishu.messages.*` → `channel.send`
3. Migrate `telegram.messages.*` → `channel.send`
4. Feature flag to switch between old/new

**Validation**:
```bash
cargo test -p loongclaw-app --features channel-new-tools
# Compare outputs with old tools
```

### Phase 5: Cleanup (Week 6)

**Goal**: Remove old code

1. Delete `tools/feishu.rs`
2. Delete `feishu/` module
3. Remove feature flags
4. Update documentation

**Validation**:
```bash
cargo test --workspace --all-features
```

## Error Handling Strategy

```rust
// Three-layer error handling

// Layer 1: Handler errors (platform-specific)
pub enum HandlerError {
    Network(reqwest::Error),
    Auth(String),
    RateLimited(Duration),
    Platform { code: String, message: String },
}

// Layer 2: Command errors (generic)
pub enum CommandError {
    ValidationFailed(String),
    HandlerFailed(HandlerError),
    NoHandler,
    EventStoreFailed(String),
}

// Layer 3: Tool errors (user-facing)
pub enum ToolError {
    CommandFailed(String),
    NotFound,
    PlatformNotSupported(String),
    InvalidInput(String),
}

// Conversion
impl From<CommandError> for ToolError {
    fn from(e: CommandError) -> Self {
        match e {
            CommandError::HandlerFailed(he) => ToolError::CommandFailed(he.to_string()),
            CommandError::ValidationFailed(msg) => ToolError::InvalidInput(msg),
            _ => ToolError::CommandFailed(e.to_string()),
        }
    }
}
```

## Performance Considerations

| Operation | Before | After | Notes |
|-----------|--------|-------|-------|
| Simple message send | 1 HTTP call | 1 HTTP call | Same |
| Get message | 1 HTTP call | Cache lookup | Faster after first fetch |
| List messages | 1 HTTP call | Event projection | Eventually consistent |
| Cross-platform op | N HTTP calls (sequential) | N HTTP calls (can parallelize) | Better composability |
| Memory usage | Low | Medium (event cache) | Configurable retention |

**Optimizations**:
- Event store retention: 30 days default
- Projection cache: LRU with 1000 entries
- Snapshotting: Every 100 events for fast replay

## Open Questions

### 1. Event Store Backend

**Option A**: SQLite (default, local)
- Pros: Zero config, works offline, SQL queries
- Cons: Single node only

**Option B**: PostgreSQL (optional, multi-node)
- Pros: Distributed, powerful queries
- Cons: Additional dependency

**Option C**: File-based (simplest)
- Pros: No DB needed
- Cons: Hard to query

**Recommendation**: SQLite default, PostgreSQL optional feature flag.

### 2. Event Schema Evolution

**Problem**: Events are persisted. Schema changes break replay.

**Options**:
1. Versioned events: `MessageSentV1`, `MessageSentV2`
2. Migration scripts: Transform old events on read
3. Tolerant readers: New fields optional

**Recommendation**: Tolerant readers + explicit versioning for breaking changes.

### 3. Snapshots vs Full Replay

**Problem**: Rebuilding state from 10,000 events is slow.

**Solution**: Periodic snapshots
```rust
pub struct Snapshot {
    pub stream_id: StreamId,
    pub version: u64,
    pub state: serde_json::Value,
}

// Every 100 events, save snapshot
// Replay: snapshot + events since snapshot
```

## Appendix: Command/Event Reference

### Messaging Commands

| Command | Events | Handlers |
|---------|--------|----------|
| SendMessage | MessageSent, CommandFailed | Telegram, Matrix, Feishu |
| ReplyToMessage | MessageReplied, CommandFailed | Telegram, Matrix, Feishu |
| GetMessage | (queries projection) | - |
| ListMessages | (queries projection) | - |

### Document Commands

| Command | Events | Handlers |
|---------|--------|----------|
| CreateDocument | DocumentCreated, CommandFailed | Feishu |
| ReadDocument | (queries projection) | - |
| AppendToDocument | DocumentUpdated, CommandFailed | Feishu |

### Calendar Commands

| Command | Events | Handlers |
|---------|--------|----------|
| ListCalendars | CalendarListed, CommandFailed | Feishu |
| QueryFreebusy | FreebusyQueried, CommandFailed | Feishu |
| CreateEvent | EventCreated, CommandFailed | Feishu |

## Changelog

| Date | Version | Changes |
|------|---------|---------|
| 2026-03-21 | 1.0.0 | Initial design: Event Sourcing + CQRS architecture |

---

**Related Documents**:
- Original design: `docs/plans/2026-03-21-channel-api-abstraction-design.md`
- Implementation plan: `docs/plans/2026-03-21-channel-cqrs-implementation.md` (to be created)
