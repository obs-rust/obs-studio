//! `--mcp`: serve obs-mcp over stdio from an in-process libobs (#3289).

use std::io;

/// Runs the MCP server until stdin closes.
///
/// The record and stream tools control the libobs outputs the frontend names
/// `simple_file_output` and `simple_stream`.
///
/// # Errors
///
/// Returns an error when libobs fails to start or stdio fails.
#[cfg(feature = "bundled-native")]
pub fn run() -> io::Result<()> {
    obs_mcp::host::serve_stdio(Some("simple_file_output"), Some("simple_stream"))
}

/// Runs the MCP server until stdin closes.
///
/// # Errors
///
/// Always: this build does not link libobs.
#[cfg(not(feature = "bundled-native"))]
pub fn run() -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "this build has no libobs; rebuild with --features bundled-native",
    ))
}
