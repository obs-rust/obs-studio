//! Newline-delimited framing: one JSON message per line.

use std::io::{self, BufRead, Write};

use super::{Request, Response};

/// Reads the next message. Blank lines are skipped and `Ok(None)` means end of
/// input. A line that is not a valid request yields `Ok(Some(Err(response)))`
/// holding the error response to send (-32700 or -32600).
///
/// # Errors
///
/// Returns I/O errors from `reader`.
pub fn read_message<R: BufRead>(reader: &mut R) -> io::Result<Option<Result<Request, Response>>> {
    let _ = reader;
    todo!("read_message")
}

/// Writes `response` as one line and flushes.
///
/// # Errors
///
/// Returns I/O errors from `writer`.
pub fn write_message<W: Write>(writer: &mut W, response: &Response) -> io::Result<()> {
    let _ = (writer, response);
    todo!("write_message")
}
