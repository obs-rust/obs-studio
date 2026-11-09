use obs_mcp::{
    client::{Client, Error},
    platform::Connection,
    tools::control::StartInput,
};
use serde_json::{Value, json};
use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);
impl Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn connection(reply: Value) -> (Connection, Captured) {
    let output = Captured::default();
    let data = format!("{}\n{reply}\n", json!({"jsonrpc":"2.0","id":1,"result":{}}));
    (
        Connection {
            reader: Box::new(io::Cursor::new(data.into_bytes())),
            writer: Box::new(output.clone()),
        },
        output,
    )
}
#[test]
fn client_reuses_typed_inputs_and_structured_outputs() {
    let (connection, written) = connection(
        json!({"jsonrpc":"2.0","id":2,"result":{"structuredContent":{"configured":true,"active":true,"vmic_active":true}}}),
    );
    let client = Client::from_connection(connection).unwrap();
    assert!(
        client
            .vcam()
            .start(StartInput {
                wait: true,
                ..Default::default()
            })
            .unwrap()
            .active
    );
    let bytes = written.0.lock().unwrap();
    let requests: Vec<Value> = std::str::from_utf8(&bytes)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(requests[2]["params"]["name"], "vcam_start");
    assert_eq!(requests[2]["params"]["arguments"]["wait"], true);
}
#[test]
fn client_preserves_domain_error_and_rejects_wrong_id() {
    let (stream, _) = connection(
        json!({"jsonrpc":"2.0","id":2,"result":{"isError":true,"structuredContent":{"code":207,"category":"not_ready","message":"video unavailable","retryable":true}}}),
    );
    let client = Client::from_connection(stream).unwrap();
    assert!(
        matches!(client.vcam().start(StartInput::default()),Err(Error::Domain(error)) if error.code==207 && error.retryable)
    );
    let (stream, _) = connection(json!({"jsonrpc":"2.0","id":99,"result":{}}));
    let client = Client::from_connection(stream).unwrap();
    assert!(matches!(
        client.vcam().start(StartInput::default()),
        Err(Error::Protocol(_))
    ));
}
