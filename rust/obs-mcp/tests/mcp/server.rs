//! MCP lifecycle, ping, tools and error codes through [`obs_mcp::serve`].

use obs_mcp::server::{Server, Tool, ToolRegistry};
use serde_json::{Value, json};

use super::roundtrip;

#[test]
fn initialize_handshake() {
    let out = roundtrip(concat!(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        "\n",
    ));
    assert_eq!(out.len(), 1, "a notification gets no response");
    let r = &out[0]["result"];
    assert_eq!(out[0]["id"], 1);
    assert_eq!(r["protocolVersion"], "2025-06-18");
    assert_eq!(r["serverInfo"]["name"], "obs-mcp");
    assert_eq!(r["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(r["capabilities"], json!({"tools": {}, "resources": {}}));
}

#[test]
fn initialized_notification_sets_state() {
    let mut s = Server::new();
    assert!(!s.is_initialized());
    let note = obs_mcp::protocol::Request {
        id: None,
        method: "notifications/initialized".into(),
        params: Value::Null,
    };
    assert!(s.handle(&note).is_none());
    assert!(s.is_initialized());
}

#[test]
fn ping_returns_empty_result() {
    let out = roundtrip("{\"jsonrpc\":\"2.0\",\"id\":\"p\",\"method\":\"ping\"}\n");
    assert_eq!(out, vec![json!({"jsonrpc": "2.0", "id": "p", "result": {}})]);
}

#[test]
fn tools_list_is_empty_without_tools() {
    let out = roundtrip("{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n");
    assert_eq!(out[0]["result"], json!({"tools": []}));
}

struct Echo;
impl Tool for Echo {
    fn name(&self) -> &str {
        "echo"
    }
    fn description(&self) -> &str {
        "Echoes text"
    }
    fn input_schema(&self) -> Value {
        json!({"type": "object"})
    }
    fn call(&self, arguments: &Value) -> Result<String, String> {
        arguments["text"].as_str().map(str::to_owned).ok_or_else(|| "missing text".into())
    }
}

fn echo_server() -> Server {
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(Echo));
    Server::with_tools(reg)
}

fn call(server: &mut Server, line: &str) -> Value {
    let mut out = Vec::new();
    server.serve(line.as_bytes(), &mut out).unwrap();
    serde_json::from_slice(&out).unwrap()
}

#[test]
fn registered_tool_is_listed_and_callable() {
    let mut s = echo_server();
    let list = call(&mut s, "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n");
    assert_eq!(
        list["result"]["tools"],
        json!([{"name": "echo", "description": "Echoes text", "inputSchema": {"type": "object"}}])
    );
    let ok = call(
        &mut s,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"echo","arguments":{"text":"hi"}}}
"#,
    );
    assert_eq!(ok["result"], json!({"content": [{"type": "text", "text": "hi"}], "isError": false}));
    let bad = call(
        &mut s,
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"echo","arguments":{}}}
"#,
    );
    assert_eq!(bad["result"]["isError"], true);
    let unknown = call(
        &mut s,
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"nope"}}
"#,
    );
    assert_eq!(unknown["error"]["code"], -32602);
}

#[test]
fn error_codes() {
    let out = roundtrip(concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"nope\"}\n",
        "{broken\n",
        "{\"jsonrpc\":\"2.0\",\"id\":9}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\n",
    ));
    assert_eq!(out.len(), 4, "the server keeps serving after errors");
    assert_eq!(out[0]["error"]["code"], -32601);
    assert_eq!(out[0]["id"], 1);
    assert_eq!(out[1]["error"]["code"], -32700);
    assert_eq!(out[1]["id"], Value::Null);
    assert_eq!(out[2]["error"]["code"], -32600);
    assert_eq!(out[2]["id"], 9);
    assert_eq!(out[3]["result"], json!({}));
}

#[test]
fn unknown_notification_is_ignored() {
    assert!(roundtrip("{\"jsonrpc\":\"2.0\",\"method\":\"whatever\"}\n").is_empty());
}
