//! Middleware stack for the REST transport layer.
//!
//! Applied in order: CORS, auth, logging, error handling (Requirement 11).

pub mod auth;
pub mod cors;
pub mod error_handler;
pub mod logging;
