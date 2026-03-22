//! Channel tools module
//!
//! Provides generic tool handlers using dynamic dispatch over PlatformApi traits.

pub mod dispatch;
pub mod feishu;
pub mod generic;
pub mod registry;

#[allow(unused_imports)]
pub use generic::{
    create_document, get_message, list_calendars, list_messages, query_freebusy, read_document,
    reply_message, send_message, upload_media,
};
#[allow(unused_imports)]
pub use registry::{parse_message_content, parse_pagination, parse_time_range};
