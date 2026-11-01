//! Host entry point: libobs lifecycle plus the stdio server (#3289).

use std::io;

/// Starts a headless libobs and serves obs-mcp on stdin/stdout until stdin
/// reaches end of input, then shuts libobs down.
///
/// # Errors
///
/// Returns an error when libobs fails to start or stdio fails.
pub fn serve_stdio(record: Option<&str>, stream: Option<&str>) -> io::Result<()> {
    let _ = (record, stream);
    todo!("start libobs and serve obs-mcp on stdio")
}
