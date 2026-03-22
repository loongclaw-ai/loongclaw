//! Channel Platform API Traits
//!
//! This module defines platform-agnostic traits for channel capabilities.
//! Each platform (Feishu, Telegram, Matrix) can implement these traits
//! to provide a consistent interface for tools.

pub mod calendar;
pub mod documents;
pub mod error;
pub mod messaging;

pub use calendar::{
    Calendar, CalendarApi, CalendarEvent, CreateEventRequest, EventStatus, FreeBusyResult,
    TimeRange,
};
pub use documents::{Document, DocumentContent, DocumentsApi};
pub use error::{ApiError, ApiResult, PlatformApi};
pub use messaging::{
    MediaType, MediaUploadResult, Message, MessageContent, MessagingApi, Pagination,
};
