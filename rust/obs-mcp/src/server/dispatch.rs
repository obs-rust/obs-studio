//! MCP lifecycle and method dispatch.

use std::io::{self, BufRead, Write};

use crate::protocol::{Request, Response};

use super::ToolRegistry;

/// The MCP revision this server implements.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

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
        Self { tools, initialized: false }
    }

    /// True once the client sent `notifications/initialized`.
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Handles one request or notification; notifications return `None`.
    pub fn handle(&mut self, request: &Request) -> Option<Response> {
        let _ = (request, &self.tools);
        todo!("handle")
    }

    /// Serves until `reader` reaches end of input.
    ///
    /// # Errors
    ///
    /// Returns the first I/O error from reading or writing.
    pub fn serve<R: BufRead, W: Write>(&mut self, reader: R, writer: W) -> io::Result<()> {
        let _ = (reader, writer);
        todo!("serve")
    }
}
