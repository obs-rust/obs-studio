//! Scene tools: `scenes_list`, `scene_get_current`, `scene_set_current`,
//! `scene_create` and `scene_remove` (#3285).
//!
//! The tools talk to OBS through [`SceneBackend`], so they are tested with a
//! fake. The real libobs binding is a follow-up (#3283); `obs-scene` holds only
//! the pure scene logic and has no program-scene state to call yet.

use std::rc::Rc;

use serde_json::{Value, json};

use crate::server::{Tool, ToolRegistry};

/// The scene operations the tools need from OBS.
///
/// Methods take `&self`: libobs owns the state, so implementations use their
/// own interior mutability.
pub trait SceneBackend {
    /// Scene names in display order.
    fn scenes(&self) -> Vec<String>;
    /// The current program scene, if any.
    fn current(&self) -> Option<String>;
    /// Makes `name` the current program scene.
    ///
    /// # Errors
    ///
    /// Returns a message when no such scene exists.
    fn set_current(&self, name: &str) -> Result<(), String>;
    /// Creates an empty scene called `name`.
    ///
    /// # Errors
    ///
    /// Returns a message when a scene of that name already exists.
    fn create(&self, name: &str) -> Result<(), String>;
    /// Removes the scene called `name`.
    ///
    /// # Errors
    ///
    /// Returns a message when no such scene exists.
    fn remove(&self, name: &str) -> Result<(), String>;
}

/// Which scene operation a [`SceneTool`] performs.
#[derive(Clone, Copy)]
enum Op {
    List,
    GetCurrent,
    SetCurrent,
    Create,
    Remove,
}

/// One scene tool bound to a backend.
struct SceneTool {
    op: Op,
    backend: Rc<dyn SceneBackend>,
}

impl SceneTool {
    /// The `name` argument, which must be a non-empty string.
    fn name_arg(arguments: &Value) -> Result<&str, String> {
        match arguments.get("name").and_then(Value::as_str) {
            Some(name) if !name.is_empty() => Ok(name),
            _ => Err("argument `name` must be a non-empty string".into()),
        }
    }
}

impl Tool for SceneTool {
    fn name(&self) -> &str {
        match self.op {
            Op::List => "scenes_list",
            Op::GetCurrent => "scene_get_current",
            Op::SetCurrent => "scene_set_current",
            Op::Create => "scene_create",
            Op::Remove => "scene_remove",
        }
    }

    fn description(&self) -> &str {
        match self.op {
            Op::List => "List the names of all scenes, in order.",
            Op::GetCurrent => "Get the name of the current program scene, or null if none.",
            Op::SetCurrent => "Make the named scene the current program scene.",
            Op::Create => {
                "Create a new empty scene with the given name. When there is no program scene \
                 yet, the new scene becomes the current program scene."
            }
            Op::Remove => "Remove the scene with the given name.",
        }
    }

    fn input_schema(&self) -> Value {
        match self.op {
            Op::List | Op::GetCurrent => json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
            Op::SetCurrent | Op::Create | Op::Remove => json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "minLength": 1, "description": "Scene name." },
                },
                "required": ["name"],
                "additionalProperties": false,
            }),
        }
    }

    fn call(&self, arguments: &Value) -> Result<String, String> {
        let result = match self.op {
            Op::List => json!({ "scenes": self.backend.scenes() }),
            Op::GetCurrent => json!({ "name": self.backend.current() }),
            Op::SetCurrent => {
                let name = Self::name_arg(arguments)?;
                self.backend.set_current(name)?;
                json!({ "name": name })
            }
            Op::Create => {
                let name = Self::name_arg(arguments)?;
                self.backend.create(name)?;
                json!({ "name": name })
            }
            Op::Remove => {
                let name = Self::name_arg(arguments)?;
                self.backend.remove(name)?;
                json!({ "name": name })
            }
        };
        Ok(result.to_string())
    }
}

/// Registers the five scene tools on `registry`, backed by `backend`.
pub fn register(registry: &mut ToolRegistry, backend: &Rc<dyn SceneBackend>) {
    for op in [
        Op::List,
        Op::GetCurrent,
        Op::SetCurrent,
        Op::Create,
        Op::Remove,
    ] {
        registry.register(Box::new(SceneTool {
            op,
            backend: Rc::clone(backend),
        }));
    }
}
