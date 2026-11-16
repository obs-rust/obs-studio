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
pub fn run(record_dir: Option<&str>) -> io::Result<()> {
    let options = obs_mcp::host::StartOptions::from_env(record_dir.map(Into::into));
    obs_mcp::host::serve_stdio(&options)
}

/// Runs the MCP server until stdin closes.
///
/// # Errors
///
/// Always: this build does not link libobs.
#[cfg(not(feature = "bundled-native"))]
pub fn run(_record_dir: Option<&str>) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "this build has no libobs; rebuild with --features bundled-native",
    ))
}

/// Serve a headless socket until stdin closes.
/// # Errors
/// Fails if libobs or the socket cannot start.
#[cfg(feature = "bundled-native")]
pub fn listen(record_dir: Option<&str>, socket: Option<&str>) -> io::Result<()> {
    obs_mcp::host::serve_socket(
        &obs_mcp::host::StartOptions::from_env(record_dir.map(Into::into)),
        &obs_mcp::platform::EndpointOptions {
            socket: socket.map(Into::into),
        },
    )
}
#[cfg(not(feature = "bundled-native"))]
pub fn listen(record_dir: Option<&str>, _: Option<&str>) -> io::Result<()> {
    run(record_dir)
}
