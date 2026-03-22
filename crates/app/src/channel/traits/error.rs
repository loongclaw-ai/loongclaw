use std::fmt;
use std::time::Duration;

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, Clone)]
pub enum ApiError {
    Network(String),
    Http {
        status: u16,
        body: String,
    },
    Auth {
        message: String,
        retry_after: Option<chrono::DateTime<chrono::Utc>>,
    },
    RateLimited {
        retry_after_secs: Option<u64>,
    },
    NotFound {
        resource: String,
        id: Option<String>,
    },
    PermissionDenied {
        action: String,
        resource: String,
    },
    InvalidRequest {
        message: String,
        field: Option<String>,
    },
    NotSupported {
        operation: String,
        platform: String,
    },
    Platform {
        platform: String,
        code: String,
        message: String,
        raw: Option<serde_json::Value>,
    },
    Serialization(String),
    Internal(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::Network(msg) => write!(f, "network error: {}", msg),
            ApiError::Http { status, body } => {
                write!(f, "HTTP error: status={}, body={}", status, body)
            }
            ApiError::Auth { message, .. } => write!(f, "authentication failed: {}", message),
            ApiError::RateLimited { retry_after_secs } => {
                write!(
                    f,
                    "rate limited, retry after {:?} seconds",
                    retry_after_secs
                )
            }
            ApiError::NotFound { resource, id } => {
                write!(f, "not found: {} (id={:?})", resource, id)
            }
            ApiError::PermissionDenied { action, resource } => {
                write!(f, "permission denied: {} on {}", action, resource)
            }
            ApiError::InvalidRequest { message, field } => {
                write!(f, "invalid request: {} (field={:?})", message, field)
            }
            ApiError::NotSupported {
                operation,
                platform,
            } => {
                write!(f, "operation not supported: {} on {}", operation, platform)
            }
            ApiError::Platform {
                platform,
                code,
                message,
                ..
            } => {
                write!(
                    f,
                    "platform error: platform={}, code={}, message={}",
                    platform, code, message
                )
            }
            ApiError::Serialization(e) => write!(f, "serialization error: {}", e),
            ApiError::Internal(msg) => write!(f, "internal error: {}", msg),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        ApiError::Serialization(err.to_string())
    }
}

impl ApiError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ApiError::Network(_) | ApiError::RateLimited { .. } | ApiError::Auth { .. }
        )
    }

    pub fn retry_delay(&self) -> Option<Duration> {
        match self {
            ApiError::RateLimited {
                retry_after_secs: Some(secs),
            } => Some(Duration::from_secs(*secs)),
            ApiError::Network(_)
            | ApiError::Http { .. }
            | ApiError::Auth { .. }
            | ApiError::RateLimited {
                retry_after_secs: None,
            }
            | ApiError::NotFound { .. }
            | ApiError::PermissionDenied { .. }
            | ApiError::InvalidRequest { .. }
            | ApiError::NotSupported { .. }
            | ApiError::Platform { .. }
            | ApiError::Serialization(_)
            | ApiError::Internal(_) => None,
        }
    }
}

pub trait PlatformApi: Send + Sync {
    fn as_any(&self) -> &dyn std::any::Any;
}
