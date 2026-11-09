//! Typed MCP tools and structured domain errors.
use schemars::JsonSchema;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use super::Tool;

/// Advisory MCP annotations.
#[derive(Clone, Copy)]
pub struct ToolAnnotations;
impl ToolAnnotations {
    /// Observes state without modifying it.
    pub const READ_ONLY: Self = Self;
}
/// A domain failure, distinct from a JSON-RPC fault.
pub struct ToolError;
impl ToolError {
    /// Processing failure (obs-websocket v5 code 700).
    pub fn processing(_message: impl Into<String>) -> Self { Self }
}
/// Strongly typed operation exposed over MCP.
pub trait TypedTool {
    /// Arguments.
    type Input: DeserializeOwned + JsonSchema;
    /// Successful response.
    type Output: Serialize + JsonSchema;
    /// Stable wire name.
    const NAME: &'static str;
    /// Tool annotations.
    const ANNOTATIONS: ToolAnnotations;
    /// Human-readable description.
    fn description(&self) -> &str;
    /// Execute against OBS.
    /// # Errors
    /// Reports a domain failure.
    fn call(&self, input: Self::Input) -> Result<Self::Output, ToolError>;
}
impl<T: TypedTool> Tool for T {
    fn name(&self) -> &str { T::NAME }
    fn description(&self) -> &str { TypedTool::description(self) }
    fn input_schema(&self) -> Value { todo!("typed schema") }
    fn call(&self, _arguments: &Value) -> Result<String, String> { todo!("typed execution") }
}
