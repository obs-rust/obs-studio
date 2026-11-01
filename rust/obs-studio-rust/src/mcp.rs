//! `--mcp`: serve obs-mcp over stdio from an in-process libobs (#3289).

use std::io;

/// The libobs output the frontend names for local recording.
pub const RECORD_OUTPUT: &str = "simple_file_output";
/// The libobs output the frontend names for streaming.
pub const STREAM_OUTPUT: &str = "simple_stream";

/// Runs the MCP server until stdin closes.
///
/// # Errors
///
/// Returns an error when libobs fails to start or stdio fails.
#[cfg(feature = "bundled-native")]
pub fn run() -> io::Result<()> {
    obs_mcp::host::serve_stdio(Some(RECORD_OUTPUT), Some(STREAM_OUTPUT))
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
