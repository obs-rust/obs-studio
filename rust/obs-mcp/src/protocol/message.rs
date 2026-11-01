//! JSON-RPC 2.0 message types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Standard JSON-RPC 2.0 error codes.
pub mod error_code {
    /// Invalid JSON was received.
    pub const PARSE_ERROR: i64 = -32700;
    /// The JSON is not a valid request object.
    pub const INVALID_REQUEST: i64 = -32600;
    /// The method does not exist.
    pub const METHOD_NOT_FOUND: i64 = -32601;
    /// Invalid method parameters.
    pub const INVALID_PARAMS: i64 = -32602;
    /// Internal error while handling the request.
    pub const INTERNAL_ERROR: i64 = -32603;
    /// MCP: `resources/read` named a URI that is not registered.
    pub const RESOURCE_NOT_FOUND: i64 = -32002;
}

/// A request identifier: a number or a string.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Id {
    /// Numeric identifier.
    Number(i64),
    /// String identifier.
    String(String),
}

/// A validated request (`id` present) or notification (`id` absent).
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    /// `None` for a notification, which gets no response.
    pub id: Option<Id>,
    /// The method name.
    pub method: String,
    /// The `params` member, `Value::Null` when absent.
    pub params: Value,
}

/// The `error` member of a failed response.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ErrorObject {
    /// One of [`error_code`], or an application code.
    pub code: i64,
    /// Short description.
    pub message: String,
}

/// A response: exactly one of `result` and `error` is set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Response {
    /// Always `"2.0"`.
    pub jsonrpc: String,
    /// The request id; `null` when it could not be determined.
    pub id: Option<Id>,
    /// The success value.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub result: Option<Value>,
    /// The failure.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<ErrorObject>,
}

impl Response {
    /// A success response.
    #[must_use]
    pub fn ok(id: Option<Id>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// An error response.
    #[must_use]
    pub fn err(id: Option<Id>, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(ErrorObject {
                code,
                message: message.into(),
            }),
        }
    }
}
