//! Tier 4: `obs-studio-rust --mcp` as a black box over stdio (#3289).
//!
//! Spawns the built binary, which starts a headless libobs and serves obs-mcp
//! on stdin/stdout, and talks JSON-RPC to it like Claude Code does. Included
//! only under `bundled-native`, the feature that links libobs into the binary.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

const TIMEOUT: Duration = Duration::from_secs(60);

#[test]
fn mcp_flag_serves_obs_over_stdio_and_exits_on_eof() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_obs-studio-rust"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn obs-studio-rust --mcp");
    let mut stdin = child.stdin.take().unwrap();
    // Every stdout line must be a JSON-RPC message: libobs logging goes to stderr.
    let (lines, responses) = mpsc::channel();
    let stdout = child.stdout.take().unwrap();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let parsed = line
                .map_err(|e| e.to_string())
                .and_then(|l| serde_json::from_str::<Value>(&l).map_err(|e| format!("{e}: {l}")));
            if lines.send(parsed).is_err() {
                break;
            }
        }
    });
    let stderr = child.stderr.take().unwrap();
    let logs = thread::spawn(move || BufReader::new(stderr).lines().map_while(Result::ok).count());

    let mut call = |message: Value| -> Value {
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
        if message.get("id").is_none() {
            return Value::Null;
        }
        let reply = responses
            .recv_timeout(TIMEOUT)
            .expect("a response before the timeout")
            .expect("stdout carries only JSON lines");
        assert_eq!(reply["id"], message["id"], "{reply}");
        assert!(reply.get("error").is_none(), "{reply}");
        reply["result"].clone()
    };

    let init = call(json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "obs-mcp-tier4", "version": "0" },
        },
    }));
    assert!(init["capabilities"]["tools"].is_object(), "{init}");
    // A notification has no id and gets no reply.
    call(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));

    let tools = call(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }));
    let names: Vec<&str> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for name in ["scene_create", "scenes_list", "screenshot"] {
        assert!(names.contains(&name), "{name} missing from {names:?}");
    }

    let created = call(json!({
        "jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": { "name": "scene_create", "arguments": { "name": "Tier4" } },
    }));
    assert_ne!(created["isError"], json!(true), "{created}");

    let listed = call(json!({
        "jsonrpc": "2.0", "id": 4, "method": "tools/call",
        "params": { "name": "scenes_list", "arguments": {} },
    }));
    assert!(
        listed["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Tier4"),
        "{listed}"
    );

    let scenes = call(json!({
        "jsonrpc": "2.0", "id": 5, "method": "resources/read",
        "params": { "uri": "obs://scenes" },
    }));
    assert!(
        scenes["contents"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Tier4"),
        "{scenes}"
    );

    // Closing stdin is the clean shutdown signal.
    drop(call);
    drop(stdin);
    let (done, exited) = mpsc::channel();
    let waiter = thread::spawn(move || {
        let status = child.wait().unwrap();
        let _ = done.send(());
        status
    });
    exited
        .recv_timeout(TIMEOUT)
        .expect("exit after stdin reaches end of input");
    let status = waiter.join().unwrap();
    assert!(status.success(), "{status}");
    // Nothing but the replies above reached stdout: the channel closes empty.
    assert_eq!(
        responses.recv_timeout(TIMEOUT).unwrap_err(),
        RecvTimeoutError::Disconnected
    );
    assert!(logs.join().unwrap() > 0, "libobs logs go to stderr");
}
