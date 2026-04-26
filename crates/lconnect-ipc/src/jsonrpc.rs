//! JSON-RPC 2.0 envelope types.
//!
//! These types are pure (de)serialisation: they describe the wire shape of
//! requests, responses, and notifications. Line-delimited framing
//! (one JSON object per newline) is enforced at the transport layer
//! (server.rs / client.rs), not here.
//!
//! `Response` uses `Option<result>` / `Option<error>` rather than
//! `#[serde(untagged)]` over an enum, because the JSON-RPC 2.0 spec
//! requires *exactly one* of the two members to be present. The
//! [`Response::ok`] and [`Response::err`] constructors are the supported
//! way to build a response and prevent callers from accidentally
//! producing a both-set or neither-set envelope.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A JSON-RPC 2.0 request envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    pub params: Value,
}

impl Request {
    /// Construct a new request with a numeric id.
    #[must_use]
    pub fn new(id: u64, method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(id),
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC 2.0 response envelope.
///
/// Per the spec, exactly one of `result` or `error` must be present.
/// Use [`Response::ok`] or [`Response::err`] to construct valid envelopes;
/// `skip_serializing_if = "Option::is_none"` keeps the unused field off
/// the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorObj>,
}

impl Response {
    /// Build a success response carrying `result`.
    #[must_use]
    pub fn ok(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id: Some(id),
            result: Some(result),
            error: None,
        }
    }

    /// Build an error response carrying `error`.
    #[must_use]
    pub fn err(id: Option<Value>, error: ErrorObj) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorObj {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Server-pushed notification on the same connection (no `id` field).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub jsonrpc: String,
    pub method: String,
    pub params: Value,
}

impl Notification {
    #[must_use]
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            method: method.into(),
            params,
        }
    }
}
