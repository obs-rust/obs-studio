//! Typed MCP tools and structured domain errors.
use super::{Content, Tool};
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

/// Advisory MCP annotations; clients must still enforce their own policy.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolAnnotations {
    /// Does not mutate OBS.
    pub read_only_hint: bool,
    /// Repeating the same call has the same effect.
    pub idempotent_hint: bool,
    /// May destroy data.
    pub destructive_hint: bool,
    /// Interacts with external entities.
    pub open_world_hint: bool,
}
impl ToolAnnotations {
    /// Observes local state.
    pub const READ_ONLY: Self = Self {
        read_only_hint: true,
        idempotent_hint: true,
        destructive_hint: false,
        open_world_hint: false,
    };
    /// Changes local state.
    pub const MUTATING: Self = Self {
        read_only_hint: false,
        idempotent_hint: false,
        destructive_hint: true,
        open_world_hint: true,
    };
    /// Repeating start/stop succeeds with the current state.
    pub const IDEMPOTENT: Self = Self {
        idempotent_hint: true,
        ..Self::MUTATING
    };
}
/// Domain failure category, using obs-websocket v5 status ranges.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    /// A resource is absent or not ready (ResourceNotFound/NotReady).
    NotReady,
    /// The current state prevents the operation (6xx).
    WrongState,
    /// OBS failed to process the operation (7xx).
    Processing,
}
/// A domain failure, distinct from a JSON-RPC fault.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ToolError {
    /// obs-websocket v5 RequestStatus code.
    pub code: u16,
    /// Stable machine-readable category.
    pub category: ErrorCategory,
    /// Human-readable context.
    pub message: String,
    /// Whether a later retry may succeed.
    pub retryable: bool,
}
impl ToolError {
    /// Processing failure (obs-websocket v5 RequestProcessingFailed, 702).
    pub fn processing(message: impl Into<String>) -> Self {
        Self {
            code: 702,
            category: ErrorCategory::Processing,
            message: message.into(),
            retryable: false,
        }
    }
    /// Missing resource (obs-websocket v5 ResourceNotFound, 600).
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: 600,
            category: ErrorCategory::NotReady,
            message: message.into(),
            retryable: false,
        }
    }
    /// A required subsystem is unavailable (obs-websocket v5 NotReady, 207).
    pub fn not_ready(message: impl Into<String>) -> Self {
        Self {
            code: 207,
            category: ErrorCategory::NotReady,
            message: message.into(),
            retryable: true,
        }
    }
    /// Invalid resource state (obs-websocket v5 InvalidResourceState, 604).
    pub fn wrong_state(message: impl Into<String>) -> Self {
        Self {
            code: 604,
            category: ErrorCategory::WrongState,
            message: message.into(),
            retryable: false,
        }
    }
}
impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ToolError {}

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
    /// Content accompanying structured output. Screenshots retain image content.
    /// # Errors
    /// Reports output serialization failures without panicking the server.
    fn content(output: &Self::Output) -> Result<Vec<Content>, ToolError> {
        serde_json::to_value(output)
            .map(|value| vec![Content::Text(value.to_string())])
            .map_err(|error| ToolError::processing(error.to_string()))
    }
}
impl<T: TypedTool> Tool for T {
    fn name(&self) -> &str {
        T::NAME
    }
    fn description(&self) -> &str {
        TypedTool::description(self)
    }
    fn input_schema(&self) -> Value {
        input_schema::<T::Input>()
    }
    fn output_schema(&self) -> Option<Value> {
        Some(json!(schema_for!(T::Output)))
    }
    fn annotations(&self) -> Option<Value> {
        Some(json!(T::ANNOTATIONS))
    }
    fn call(&self, arguments: &Value) -> Result<String, String> {
        let input = parse_input(arguments)?;
        let output = TypedTool::call(self, input).map_err(|e| e.to_string())?;
        serde_json::to_value(output)
            .map(|value| value.to_string())
            .map_err(|e| e.to_string())
    }
    fn call_content(&self, arguments: &Value) -> Result<Vec<Content>, String> {
        let input = parse_input(arguments)?;
        TypedTool::call(self, input)
            .and_then(|output| T::content(&output))
            .map_err(|e| e.to_string())
    }
    fn call_result(&self, arguments: &Value) -> Result<Value, String> {
        let input = parse_input(arguments)?;
        let result = TypedTool::call(self, input).and_then(|output| {
            let content = T::content(&output)?;
            let structured = serde_json::to_value(output).map_err(|error| ToolError::processing(error.to_string()))?;
            Ok(json!({"content": content.iter().map(Content::to_json).collect::<Vec<_>>(), "structuredContent": structured, "isError": false}))
        });
        Ok(result.unwrap_or_else(|error| {
            let structured = json!(error);
            json!({"content": [{"type": "text", "text": structured.to_string()}], "structuredContent": structured, "isError": true})
        }))
    }
}

/// Generated argument schema with an explicit empty properties object.
#[must_use]
pub fn input_schema<T: JsonSchema>() -> Value {
    let mut schema = json!(schema_for!(T));
    if schema["type"] == "object" && schema.get("properties").is_none() {
        schema["properties"] = json!({});
    }
    schema
}
/// The schema contract of a tool, without human-facing prose.
#[must_use]
pub fn tool_schema<T: TypedTool>() -> Value {
    json!({"name": T::NAME, "inputSchema": input_schema::<T::Input>(), "outputSchema": schema_for!(T::Output), "annotations": T::ANNOTATIONS})
}

fn parse_input<T: DeserializeOwned>(arguments: &Value) -> Result<T, String> {
    if !arguments.is_object() {
        return Err("arguments must be an object".into());
    }
    serde_path_to_error::deserialize(arguments).map_err(|error| error.to_string())
}
