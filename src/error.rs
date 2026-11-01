use crate::client::LastResponse;
use serde_json::Value;
use std::fmt;

/// The category of an SDK or API failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    Validation,
    Authentication,
    Permission,
    NotFound,
    Conflict,
    RateLimit,
    InvalidRequest,
    Api,
    Connection,
    Timeout,
    UnexpectedResponse,
    SignatureVerification,
}

/// An error with safe default formatting and explicitly accessible API details.
#[derive(Clone)]
pub struct Error {
    kind: ErrorKind,
    message: &'static str,
    response: Option<Box<LastResponse>>,
    api_error: Option<Value>,
}

impl Error {
    pub(crate) fn new(kind: ErrorKind, message: &'static str) -> Self {
        Self {
            kind,
            message,
            response: None,
            api_error: None,
        }
    }

    pub(crate) fn validation(message: &'static str) -> Self {
        Self::new(ErrorKind::Validation, message)
    }

    pub(crate) fn with_response(mut self, response: LastResponse) -> Self {
        self.response = Some(Box::new(response));
        self
    }

    pub(crate) fn with_api_error(mut self, value: Option<Value>) -> Self {
        self.api_error = value;
        self
    }

    /// The error category, independent of the API error code.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
    /// Metadata of the response, if one was received.
    pub fn last_response(&self) -> Option<&LastResponse> {
        self.response.as_deref()
    }
    /// The HTTP status, when available.
    pub fn status_code(&self) -> Option<u16> {
        self.response.as_ref().map(|r| r.status_code)
    }
    /// The server correlation identifier, when available.
    pub fn request_id(&self) -> Option<&str> {
        self.response.as_ref().and_then(|r| r.request_id.as_deref())
    }
    /// The public API error code, when present.
    pub fn code(&self) -> Option<&str> {
        self.api_error
            .as_ref()
            .and_then(|v| v.get("code"))
            .and_then(Value::as_str)
    }
    /// The API error object. It may contain sensitive application data.
    pub fn api_error(&self) -> Option<&Value> {
        self.api_error.as_ref()
    }
    /// The server retry advice, when present.
    pub fn retry_after(&self) -> Option<&str> {
        self.response
            .as_ref()
            .and_then(|r| r.retry_after.as_deref())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error")
            .field("kind", &self.kind)
            .field("status_code", &self.status_code())
            .finish_non_exhaustive()
    }
}

impl std::error::Error for Error {}
