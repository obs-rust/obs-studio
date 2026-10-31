//! Newline-delimited framing and JSON-RPC types.

use obs_mcp::protocol::{Id, Response, error_code, read_message, write_message};
use serde_json::json;

#[test]
fn write_is_one_line_ending_in_newline() {
    let mut out = Vec::new();
    write_message(
        &mut out,
        &Response::ok(Some(Id::Number(7)), json!({"a": 1})),
    )
    .unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.ends_with('\n'));
    assert_eq!(text.matches('\n').count(), 1);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v, json!({"jsonrpc": "2.0", "id": 7, "result": {"a": 1}}));
}

#[test]
fn read_roundtrips_request_and_skips_blank_lines() {
    let mut input = "\n{\"jsonrpc\":\"2.0\",\"id\":\"x\",\"method\":\"ping\"}\n\n".as_bytes();
    let req = read_message(&mut input).unwrap().unwrap().unwrap();
    assert_eq!(req.id, Some(Id::String("x".into())));
    assert_eq!(req.method, "ping");
    assert!(read_message(&mut input).unwrap().is_none());
}

#[test]
fn read_notification_has_no_id() {
    let mut input = "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n".as_bytes();
    let req = read_message(&mut input).unwrap().unwrap().unwrap();
    assert_eq!(req.id, None);
}

#[test]
fn read_bad_json_is_parse_error() {
    let mut input = "{not json\n".as_bytes();
    let resp = read_message(&mut input).unwrap().unwrap().unwrap_err();
    assert_eq!(resp.error.unwrap().code, error_code::PARSE_ERROR);
    assert_eq!(resp.id, None);
}

#[test]
fn read_invalid_request_keeps_id() {
    for line in [
        "[1,2]\n",
        "{\"jsonrpc\":\"1.0\",\"id\":3,\"method\":\"ping\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":3}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":5}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"ping\",\"params\":4}\n",
    ] {
        let resp = read_message(&mut line.as_bytes())
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert_eq!(
            resp.error.unwrap().code,
            error_code::INVALID_REQUEST,
            "{line}"
        );
    }
    let resp = read_message(&mut "{\"jsonrpc\":\"2.0\",\"id\":3}\n".as_bytes())
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert_eq!(resp.id, Some(Id::Number(3)));
}
