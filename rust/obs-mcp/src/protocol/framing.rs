//! Newline-delimited framing: one JSON message per line.

use std::io::{self, BufRead, Write};

use serde_json::Value;

use super::{Id, Request, Response, error_code};

/// Reads the next message. Blank lines are skipped and `Ok(None)` means end of
/// input. A line that is not a valid request yields `Ok(Some(Err(response)))`
/// holding the error response to send (-32700 or -32600).
///
/// # Errors
///
/// Returns I/O errors from `reader`.
pub fn read_message<R: BufRead>(reader: &mut R) -> io::Result<Option<Result<Request, Response>>> {
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        if !line.trim().is_empty() {
            return Ok(Some(parse_request(line.trim())));
        }
    }
}

/// Writes `response` as one line and flushes.
///
/// # Errors
///
/// Returns I/O errors from `writer`.
pub fn write_message<W: Write>(writer: &mut W, response: &Response) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, response)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

/// Validates one line as a JSON-RPC 2.0 request or notification.
fn parse_request(line: &str) -> Result<Request, Response> {
    let value: Value = serde_json::from_str(line)
        .map_err(|e| Response::err(None, error_code::PARSE_ERROR, format!("parse error: {e}")))?;
    let id = value
        .get("id")
        .and_then(|v| serde_json::from_value::<Id>(v.clone()).ok());
    let invalid = |why: &str| Response::err(id.clone(), error_code::INVALID_REQUEST, why);
    let Value::Object(mut object) = value else {
        return Err(invalid("request must be an object"));
    };
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(invalid("jsonrpc must be \"2.0\""));
    }
    if object
        .get("id")
        .is_some_and(|v| id.is_none() && !v.is_null())
    {
        return Err(invalid("id must be a number or a string"));
    }
    let Some(Value::String(method)) = object.remove("method") else {
        return Err(invalid("method must be a string"));
    };
    let params = object.remove("params").unwrap_or(Value::Null);
    if !matches!(params, Value::Null | Value::Object(_) | Value::Array(_)) {
        return Err(invalid("params must be an object or an array"));
    }
    Ok(Request { id, method, params })
}
