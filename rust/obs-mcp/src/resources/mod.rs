//! MCP resources: read-only documents the client can list and read (#3288).
//!
//! A [`Resource`] mirrors a [`crate::server::Tool`] and a [`ResourceRegistry`]
//! mirrors [`crate::server::ToolRegistry`]. The server answers `resources/list`
//! and `resources/read` from the registry.

pub mod scenes;
pub mod stats;

use serde_json::{Value, json};

/// A readable document with a stable URI.
pub trait Resource {
    /// Unique URI, such as `obs://stats`.
    fn uri(&self) -> &str;
    /// Short human-readable name.
    fn name(&self) -> &str;
    /// Human-readable description.
    fn description(&self) -> &str;
    /// MIME type of the text [`Resource::read`] returns.
    fn mime_type(&self) -> &str;
    /// The current contents.
    ///
    /// # Errors
    ///
    /// Returns a message describing why the resource cannot be read.
    fn read(&self) -> Result<String, String>;
}

/// The set of registered resources.
#[derive(Default)]
pub struct ResourceRegistry {
    resources: Vec<Box<dyn Resource>>,
}

impl ResourceRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `resource`, replacing any resource with the same URI.
    pub fn register(&mut self, resource: Box<dyn Resource>) {
        self.resources.retain(|r| r.uri() != resource.uri());
        self.resources.push(resource);
    }

    /// The `resources/list` result.
    #[must_use]
    pub fn list(&self) -> Value {
        let resources: Vec<Value> = self
            .resources
            .iter()
            .map(|r| {
                json!({
                    "uri": r.uri(),
                    "name": r.name(),
                    "description": r.description(),
                    "mimeType": r.mime_type(),
                })
            })
            .collect();
        json!({ "resources": resources })
    }

    /// Looks a resource up by URI.
    #[must_use]
    pub fn get(&self, uri: &str) -> Option<&dyn Resource> {
        self.resources.iter().find(|r| r.uri() == uri).map(|r| &**r)
    }

    /// The `resources/read` result for `uri`, or `None` for an unknown URI.
    ///
    /// # Errors
    ///
    /// Returns the resource's message when it cannot be read.
    pub fn read(&self, uri: &str) -> Option<Result<Value, String>> {
        let resource = self.get(uri)?;
        Some(resource.read().map(|text| {
            json!({
                "contents": [{
                    "uri": resource.uri(),
                    "mimeType": resource.mime_type(),
                    "text": text,
                }],
            })
        }))
    }
}
