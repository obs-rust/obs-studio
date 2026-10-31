//! The MCP server: lifecycle, dispatcher and tool registry.

mod dispatch;
mod tool;

pub use dispatch::{PROTOCOL_VERSION, Server};
pub use tool::{Tool, ToolRegistry};
