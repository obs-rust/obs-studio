//! Typed control API contract (#4445), in the existing core harness.
use obs_mcp::server::{ToolAnnotations, ToolError, ToolRegistry, TypedTool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Input {
    value: String,
}
#[derive(Serialize, JsonSchema)]
struct Output {
    value: String,
}
struct Echo;
impl TypedTool for Echo {
    type Input = Input;
    type Output = Output;
    const NAME: &'static str = "typed_echo";
    const ANNOTATIONS: ToolAnnotations = ToolAnnotations::READ_ONLY;
    fn description(&self) -> &str {
        "Typed echo"
    }
    fn call(&self, input: Input) -> Result<Output, ToolError> {
        if input.value == "fail" {
            return Err(ToolError::processing("deliberate failure"));
        }
        Ok(Output { value: input.value })
    }
}
#[test]
fn typed_schema_and_structured_result() {
    let mut tools = ToolRegistry::new();
    tools.register(Box::new(Echo));
    let listed = tools.list();
    assert_eq!(
        listed["tools"][0]["outputSchema"]["properties"]["value"]["type"],
        "string"
    );
    assert_eq!(listed["tools"][0]["annotations"]["readOnlyHint"], true);
    let mut server = obs_mcp::server::Server::with_tools(tools);
    let mut out = Vec::new();
    server.serve(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"typed_echo\",\"arguments\":{\"value\":\"hello\"}}}\n".as_slice(), &mut out).unwrap();
    let reply: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(
        reply["result"]["structuredContent"],
        json!({"value":"hello"})
    );
}

fn rpc(arguments: serde_json::Value) -> serde_json::Value {
    let mut tools = ToolRegistry::new();
    tools.register(Box::new(Echo));
    let mut server = obs_mcp::server::Server::with_tools(tools);
    let line = json!({"jsonrpc":"2.0", "id":7, "method":"tools/call", "params":{"name":"typed_echo", "arguments":arguments}}).to_string() + "\n";
    let mut out = Vec::new();
    server.serve(line.as_bytes(), &mut out).unwrap();
    serde_json::from_slice(&out).unwrap()
}
#[test]
fn argument_faults_are_protocol_errors() {
    for args in [
        json!({}),
        json!({"value":2}),
        json!({"value":"a","typo":true}),
        json!([]),
    ] {
        let response = rpc(args);
        assert_eq!(response["error"]["code"], -32602);
        assert!(response.get("result").is_none());
        assert_eq!(response["id"], 7);
    }
}
#[test]
fn domain_errors_have_machine_readable_status() {
    let response = rpc(json!({"value":"fail"}));
    assert!(response.get("error").is_none());
    assert_eq!(response["result"]["isError"], true);
    assert_eq!(
        response["result"]["structuredContent"],
        json!({"code":702,"category":"processing","message":"deliberate failure","retryable":false})
    );
    let text: serde_json::Value =
        serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(text, response["result"]["structuredContent"]);
}
#[test]
fn schema_snapshot_has_not_drifted() {
    let actual = obs_mcp::tools::schema_snapshot();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/rust-port/control-api.schema.json");
    if std::env::var_os("OBS_UPDATE_CONTROL_SCHEMA").is_some() {
        std::fs::write(
            &path,
            format!(
                "{{\n  \"toolError\": {},\n  \"tools\": [\n{}\n  ]\n}}\n",
                actual["toolError"],
                actual["tools"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|tool| format!("    {tool}"))
                    .collect::<Vec<_>>()
                    .join(",\n")
            ),
        )
        .unwrap();
    }
    let expected: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).expect("committed schema snapshot")).unwrap();
    assert_eq!(
        actual, expected,
        "regenerate with OBS_UPDATE_CONTROL_SCHEMA=1 and the focused schema test"
    );
}

#[test]
fn domain_status_codes_match_websocket_v5() {
    use obs_mcp::server::ErrorCategory;
    let missing = ToolError::not_found("missing");
    assert_eq!(
        (missing.code, missing.category, missing.retryable),
        (600, ErrorCategory::NotReady, false)
    );
    let not_ready = ToolError::not_ready("initializing");
    assert_eq!((not_ready.code, not_ready.retryable), (207, true));
    assert_eq!(ToolError::wrong_state("state").code, 604);
    assert_eq!(ToolError::processing("failed").code, 702);
}
