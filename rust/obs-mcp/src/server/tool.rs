//! The tool trait and registry.

use serde_json::{Value, json};

/// A tool the model can call. Implementations will wrap libobs operations.
pub trait Tool {
    /// Unique tool name.
    fn name(&self) -> &str;
    /// Human-readable description.
    fn description(&self) -> &str;
    /// JSON Schema of the arguments object.
    fn input_schema(&self) -> Value;
    /// Runs the tool; `Err` is reported to the model as a tool error.
    ///
    /// # Errors
    ///
    /// Returns a message describing why the call failed.
    fn call(&self, arguments: &Value) -> Result<String, String>;
}

/// The set of registered tools.
#[derive(Default)]
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `tool`, replacing any tool with the same name.
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.retain(|t| t.name() != tool.name());
        self.tools.push(tool);
    }

    /// The `tools/list` result.
    #[must_use]
    pub fn list(&self) -> Value {
        let tools: Vec<Value> = self
            .tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name(),
                    "description": t.description(),
                    "inputSchema": t.input_schema(),
                })
            })
            .collect();
        json!({ "tools": tools })
    }

    /// Looks a tool up by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.iter().find(|t| t.name() == name).map(|t| &**t)
    }
}
