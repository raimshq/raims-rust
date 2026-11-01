//! Rust client for the Raims public API.
//!
//! Reuse a [`RaimsClient`] across asynchronous tasks. The API key determines the
//! company, environment, and permissions. Monetary values use integer cents;
//! resource dates use Unix seconds UTC.
#![forbid(unsafe_code)]

mod client;
mod contract;
mod error;
mod optional;
pub mod pagination;
pub mod webhook;

#[path = "enum_values_generated.rs"]
pub mod enum_values;
#[path = "models_generated.rs"]
pub mod models;
#[path = "params_generated.rs"]
pub mod params;
#[path = "services_generated.rs"]
pub mod services;

pub use client::{
    ApiResponse, ClientBuilder, LastResponse, RaimsClient, RequestDiagnostics, RequestOptions,
    DEFAULT_BASE_URL,
};
pub use error::{Error, ErrorKind};
pub use optional::Optional;
pub use pagination::Pager;
pub use webhook::{construct_webhook_event, verify_webhook_signature, DEFAULT_WEBHOOK_TOLERANCE};

/// Installed SDK version. Independent of the HTTP API version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
