//! The tools the server exposes, grouped by area.

// Each operation has a distinct input/output pair and compile-time wire name.
macro_rules! typed_tool {
    ($tool:ident, $name:literal, $backend:ident, $input:ty, $output:ty, $annotations:ident, $description:expr, |$b:ident, $i:ident| $body:block) => {
        struct $tool {
            backend: std::rc::Rc<dyn $backend>,
        }
        impl crate::server::TypedTool for $tool {
            type Input = $input;
            type Output = $output;
            const NAME: &'static str = $name;
            const ANNOTATIONS: crate::server::ToolAnnotations =
                crate::server::ToolAnnotations::$annotations;
            fn description(&self) -> &str {
                $description
            }
            fn call(&self, $i: Self::Input) -> Result<Self::Output, crate::server::ToolError> {
                let $b = &self.backend;
                $body
            }
        }
    };
}

/// Empty arguments, with unknown fields rejected.
#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EmptyInput {}

pub mod outputs;
pub mod scenes;
pub mod screenshot;
pub mod sources;

/// Deterministic generated schema snapshot for all built-in tools.
#[must_use]
pub fn schema_snapshot() -> serde_json::Value {
    let mut tools = scenes::schemas();
    tools.extend(sources::schemas());
    tools.extend(outputs::schemas());
    tools.extend(screenshot::schemas());
    tools.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    serde_json::json!({"tools": tools, "toolError": schemars::schema_for!(crate::server::ToolError)})
}

pub mod control;
