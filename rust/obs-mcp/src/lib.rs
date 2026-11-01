//! In-process Model Context Protocol (MCP) server for Rust OBS (#3283).
//!
//! The server speaks JSON-RPC 2.0 over newline-delimited streams and will call
//! libobs directly; it never goes through obs-websocket.
//!
//! - [`protocol`]: JSON-RPC message types and newline-delimited framing.
//! - [`resources`]: the read-only resources the server exposes.
//! - [`server`]: the MCP lifecycle, the dispatcher and the [`server::Tool`] registry.
//! - [`tools`]: the tools the server exposes.
//! - `backend` (feature `libobs`): [`ObsBackend`](backend::ObsBackend), every backend trait over libobs.
//! - [`serve`]: runs a [`server::Server`] over a reader and a writer.

#![deny(unsafe_code)]

#[cfg(feature = "libobs")]
pub mod backend;
#[cfg(feature = "libobs")]
mod ffi;
pub mod protocol;
pub mod resources;
pub mod server;
pub mod tools;

use std::io::{self, BufRead, Write};

/// Serves MCP requests from `reader` to `writer` until `reader` reaches end of
/// input, using a server with no tools registered.
///
/// # Errors
///
/// Returns the first I/O error from reading or writing.
pub fn serve<R: BufRead, W: Write>(reader: R, writer: W) -> io::Result<()> {
    server::Server::new().serve(reader, writer)
}
