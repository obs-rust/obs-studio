//! The output tools against a fake [`OutputBackend`].

use std::cell::RefCell;
use std::rc::Rc;

use obs_mcp::server::{Server, ToolRegistry};
use obs_mcp::tools::outputs::{OutputBackend, OutputKind, OutputStatus, register};
use serde_json::{Value, json};

#[derive(Default)]
struct Fake {
    record: RefCell<Option<OutputStatus>>,
    stream: RefCell<Option<OutputStatus>>,
}

impl Fake {
    fn slot(&self, kind: OutputKind) -> &RefCell<Option<OutputStatus>> {
        match kind {
            OutputKind::Record => &self.record,
            OutputKind::Stream => &self.stream,
        }
    }
}

impl OutputBackend for Fake {
    fn start(&self, kind: OutputKind) -> Result<(), String> {
        let mut slot = self.slot(kind).borrow_mut();
        if slot.is_some() {
            return Err(format!("{kind:?} is already active"));
        }
        *slot = Some(OutputStatus {
            active: true,
            duration_ms: Some(1500),
            // A stream reports no byte count in this fake.
            bytes: (kind == OutputKind::Record).then_some(4096),
        });
        Ok(())
    }
    fn stop(&self, kind: OutputKind) -> Result<(), String> {
        match self.slot(kind).borrow_mut().take() {
            Some(_) => Ok(()),
            None => Err(format!("{kind:?} is not active")),
        }
    }
    fn status(&self, kind: OutputKind) -> OutputStatus {
        self.slot(kind).borrow().unwrap_or_default()
    }
}

fn server() -> Server {
    let backend: Rc<dyn OutputBackend> = Rc::new(Fake::default());
    let mut reg = ToolRegistry::new();
    register(&mut reg, &backend);
    Server::with_tools(reg)
}

fn rpc(s: &mut Server, method: &str, params: &Value) -> Value {
    let line = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
    let mut out = Vec::new();
    s.serve(format!("{line}\n").as_bytes(), &mut out).unwrap();
    serde_json::from_slice(&out).unwrap()
}

/// Calls a tool; returns (isError, parsed JSON text or the error text).
fn call(s: &mut Server, name: &str, args: &Value) -> (bool, Value) {
    let r = rpc(s, "tools/call", &json!({"name": name, "arguments": args}));
    let res = &r["result"];
    let text = res["content"][0]["text"].as_str().expect("text content");
    let is_err = res["isError"] == true;
    let value = if is_err {
        Value::String(text.into())
    } else {
        serde_json::from_str(text).expect("tool text is JSON")
    };
    (is_err, value)
}

#[test]
fn lists_six_tools_with_empty_object_schemas() {
    let mut s = server();
    let r = rpc(&mut s, "tools/list", &json!({}));
    let tools = r["result"]["tools"].as_array().unwrap();
    let mut names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "record_start",
            "record_status",
            "record_stop",
            "stream_start",
            "stream_status",
            "stream_stop"
        ]
    );
    for t in tools {
        assert_eq!(t["inputSchema"]["type"], "object");
        assert_eq!(t["inputSchema"]["properties"], json!({}));
        assert_eq!(t["inputSchema"]["additionalProperties"], false);
        assert!(!t["description"].as_str().unwrap().is_empty());
    }
}

#[test]
fn record_lifecycle() {
    let mut s = server();
    assert_eq!(
        call(&mut s, "record_status", &json!({})),
        (false, json!({"active": false}))
    );
    assert_eq!(
        call(&mut s, "record_start", &json!({})),
        (false, json!({"active": true}))
    );
    assert_eq!(
        call(&mut s, "record_status", &json!({})),
        (
            false,
            json!({"active": true, "duration_ms": 1500, "bytes": 4096})
        )
    );
    assert_eq!(
        call(&mut s, "record_stop", &json!({})),
        (false, json!({"active": false}))
    );
    assert_eq!(
        call(&mut s, "record_status", &json!({})),
        (false, json!({"active": false}))
    );
}

#[test]
fn stream_status_omits_unknown_bytes() {
    let mut s = server();
    assert!(!call(&mut s, "stream_start", &json!({})).0);
    assert_eq!(
        call(&mut s, "stream_status", &json!({})),
        (false, json!({"active": true, "duration_ms": 1500}))
    );
    assert_eq!(
        call(&mut s, "stream_stop", &json!({})),
        (false, json!({"active": false}))
    );
}

#[test]
fn record_and_stream_are_independent() {
    let mut s = server();
    assert!(!call(&mut s, "stream_start", &json!({})).0);
    assert_eq!(
        call(&mut s, "record_status", &json!({})),
        (false, json!({"active": false}))
    );
    assert!(call(&mut s, "record_stop", &json!({})).0);
    assert_eq!(call(&mut s, "stream_status", &json!({})).1["active"], true);
}

#[test]
fn backend_errors_become_tool_errors() {
    let mut s = server();
    let (is_err, msg) = call(&mut s, "record_stop", &json!({}));
    assert!(is_err);
    assert!(msg.as_str().unwrap().contains("not active"));
    assert!(!call(&mut s, "stream_start", &json!({})).0);
    let (is_err, msg) = call(&mut s, "stream_start", &json!({}));
    assert!(is_err);
    assert!(msg.as_str().unwrap().contains("already active"));
}
