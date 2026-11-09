//! Typed control API contract (#4445), in the existing core harness.
use obs_mcp::server::{ToolRegistry, TypedTool, ToolAnnotations, ToolError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Input { value: String }
#[derive(Serialize, JsonSchema)]
struct Output { value: String }
struct Echo;
impl TypedTool for Echo {
    type Input = Input;
    type Output = Output;
    const NAME: &'static str = "typed_echo";
    const ANNOTATIONS: ToolAnnotations = ToolAnnotations::READ_ONLY;
    fn description(&self) -> &str { "Typed echo" }
    fn call(&self, input: Input) -> Result<Output, ToolError> {
        if input.value == "fail" { return Err(ToolError::processing("deliberate failure")); }
        Ok(Output { value: input.value })
    }
}
#[test]
fn typed_schema_and_structured_result() {
    let mut tools = ToolRegistry::new();
    tools.register(Box::new(Echo));
    let listed = tools.list();
    assert_eq!(listed["tools"][0]["outputSchema"]["properties"]["value"]["type"], "string");
    assert_eq!(listed["tools"][0]["annotations"]["readOnlyHint"], true);
    let mut server = obs_mcp::server::Server::with_tools(tools);
    let mut out = Vec::new();
    server.serve(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"typed_echo\",\"arguments\":{\"value\":\"hello\"}}}\n".as_slice(), &mut out).unwrap();
    let reply: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(reply["result"]["structuredContent"], json!({"value":"hello"}));
}
