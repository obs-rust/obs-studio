//! The tool trait and registry.

use serde_json::{Value, json};

/// One item of a tool result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Content {
    /// Plain text.
    Text(String),
    /// An image: base64 `data` of the given MIME type.
    Image {
        /// Base64-encoded image bytes.
        data: String,
        /// MIME type, such as `image/png`.
        mime_type: String,
    },
}

impl Content {
    /// The MCP JSON form of this item.
    #[must_use]
    pub fn to_json(&self) -> Value {
        match self {
            Self::Text(text) => json!({ "type": "text", "text": text }),
            Self::Image { data, mime_type } => {
                json!({ "type": "image", "data": data, "mimeType": mime_type })
            }
        }
    }
}

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
    /// Runs the tool and returns structured content. The default wraps
    /// [`Tool::call`] as one text item; tools that return images override it.
    ///
    /// # Errors
    ///
    /// Returns a message describing why the call failed.
    fn call_content(&self, arguments: &Value) -> Result<Vec<Content>, String> {
        self.call(arguments).map(|text| vec![Content::Text(text)])
    }
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
