//! The MCP server: lifecycle, dispatcher and tool registry.

mod dispatch;
mod tool;

pub use dispatch::{PROTOCOL_VERSION, Server};
pub use tool::{Content, Tool, ToolRegistry};

mod typed;
pub use typed::{ErrorCategory, ToolAnnotations, ToolError, TypedTool, tool_schema};
