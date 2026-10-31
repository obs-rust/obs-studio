//! MCP lifecycle and method dispatch.

use std::io::{self, BufRead, Write};

use serde_json::{Value, json};

use crate::protocol::{Request, Response, error_code, read_message, write_message};

use super::ToolRegistry;

/// The MCP revision this server implements.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// The `initialize` result: version, identity and capabilities.
fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "serverInfo": { "name": "obs-mcp", "version": env!("CARGO_PKG_VERSION") },
        "capabilities": { "tools": {}, "resources": {} },
    })
}

/// An MCP server session.
#[derive(Default)]
pub struct Server {
    tools: ToolRegistry,
    initialized: bool,
}

impl Server {
    /// A server with no tools.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A server exposing `tools`.
    #[must_use]
    pub fn with_tools(tools: ToolRegistry) -> Self {
        Self {
            tools,
            initialized: false,
        }
    }

    /// True once the client sent `notifications/initialized`.
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Handles one request or notification; notifications return `None`.
    pub fn handle(&mut self, request: &Request) -> Option<Response> {
        let Some(id) = request.id.clone() else {
            if request.method == "notifications/initialized" {
                self.initialized = true;
            }
            return None;
        };
        let reply = match request.method.as_str() {
            "initialize" => Ok(initialize_result()),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(self.tools.list()),
            "tools/call" => self.call_tool(&request.params),
            other => Err((
                error_code::METHOD_NOT_FOUND,
                format!("method not found: {other}"),
            )),
        };
        Some(match reply {
            Ok(result) => Response::ok(Some(id), result),
            Err((code, message)) => Response::err(Some(id), code, message),
        })
    }

    /// Serves until `reader` reaches end of input.
    ///
    /// # Errors
    ///
    /// Returns the first I/O error from reading or writing.
    pub fn serve<R: BufRead, W: Write>(&mut self, mut reader: R, mut writer: W) -> io::Result<()> {
        while let Some(message) = read_message(&mut reader)? {
            let response = match message {
                Ok(request) => self.handle(&request),
                Err(error) => Some(error),
            };
            if let Some(response) = response {
                write_message(&mut writer, &response)?;
            }
        }
        Ok(())
    }

    /// `tools/call`: unknown tools are protocol errors, failing tools are
    /// results with `isError` set so the model can see the message.
    fn call_tool(&self, params: &Value) -> Result<Value, (i64, String)> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or((error_code::INVALID_PARAMS, "missing tool name".to_owned()))?;
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| (error_code::INVALID_PARAMS, format!("unknown tool: {name}")))?;
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let (text, is_error) = match tool.call(&arguments) {
            Ok(text) => (text, false),
            Err(text) => (text, true),
        };
        Ok(json!({ "content": [{ "type": "text", "text": text }], "isError": is_error }))
    }
}
