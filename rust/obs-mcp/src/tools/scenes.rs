//! Scene tools: `scenes_list`, `scene_get_current`, `scene_set_current`,
//! `scene_create` and `scene_remove` (#3285).
//!
//! The tools talk to OBS through [`SceneBackend`], so they are tested with a
//! fake. The real libobs binding is a follow-up (#3283); `obs-scene` holds only
//! the pure scene logic and has no program-scene state to call yet.

use std::rc::Rc;

use super::EmptyInput;
use crate::server::ToolError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::server::ToolRegistry;

/// The scene operations the tools need from OBS.
///
/// Methods take `&self`: libobs owns the state, so implementations use their
/// own interior mutability.
pub trait SceneBackend {
    /// Typed `ListScenes` backend operation.
    /// # Errors
    /// Reports invalid state, a missing resource or processing failure.
    fn list_scenes(&self, _input: EmptyInput) -> Result<ScenesOutput, ToolError> {
        Ok(ScenesOutput {
            scenes: self.scenes(),
        })
    }
    /// Typed `CurrentScene` backend operation.
    /// # Errors
    /// Reports invalid state, a missing resource or processing failure.
    fn current_scene(&self, _input: EmptyInput) -> Result<SceneOutput, ToolError> {
        Ok(SceneOutput {
            name: self.current(),
        })
    }
    /// Typed `SetScene` backend operation.
    /// # Errors
    /// Reports invalid state, a missing resource or processing failure.
    fn set_scene(&self, input: NameInput) -> Result<SceneOutput, ToolError> {
        check_name(&input.name)?;
        self.set_current(&input.name)
            .map_err(ToolError::not_found)?;
        Ok(SceneOutput {
            name: Some(input.name),
        })
    }
    /// Typed `CreateScene` backend operation.
    /// # Errors
    /// Reports invalid state, a missing resource or processing failure.
    fn create_scene(&self, input: NameInput) -> Result<SceneOutput, ToolError> {
        check_name(&input.name)?;
        self.create(&input.name).map_err(ToolError::wrong_state)?;
        Ok(SceneOutput {
            name: Some(input.name),
        })
    }
    /// Typed `RemoveScene` backend operation.
    /// # Errors
    /// Reports invalid state, a missing resource or processing failure.
    fn remove_scene(&self, input: NameInput) -> Result<SceneOutput, ToolError> {
        check_name(&input.name)?;
        self.remove(&input.name).map_err(ToolError::not_found)?;
        Ok(SceneOutput {
            name: Some(input.name),
        })
    }

    // Legacy backend hooks retained for existing implementers. New operations
    // use the typed request/response methods above.

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

/// A scene name.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NameInput {
    /// Scene name.
    #[schemars(length(min = 1))]
    pub name: String,
}
/// Scene listing.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ScenesOutput {
    /// Scene names in display order.
    pub scenes: Vec<String>,
}
/// Current or changed scene.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SceneOutput {
    /// Null when there is no program scene.
    pub name: Option<String>,
}
fn check_name(name: &str) -> Result<(), ToolError> {
    if name.is_empty() {
        Err(ToolError::processing(
            "argument `name` must be a non-empty string",
        ))
    } else {
        Ok(())
    }
}
typed_tool!(
    ListScenes,
    "scenes_list",
    SceneBackend,
    EmptyInput,
    ScenesOutput,
    READ_ONLY,
    "List the names of all scenes, in order.",
    |backend, input| { backend.list_scenes(input) }
);
typed_tool!(
    CurrentScene,
    "scene_get_current",
    SceneBackend,
    EmptyInput,
    SceneOutput,
    READ_ONLY,
    "Get the name of the current program scene, or null if none.",
    |backend, input| { backend.current_scene(input) }
);
typed_tool!(
    SetScene,
    "scene_set_current",
    SceneBackend,
    NameInput,
    SceneOutput,
    IDEMPOTENT,
    "Make the named scene the current program scene.",
    |backend, input| { backend.set_scene(input) }
);
typed_tool!(
    CreateScene,
    "scene_create",
    SceneBackend,
    NameInput,
    SceneOutput,
    MUTATING,
    "Create a new empty scene with the given name. When there is no program scene yet, the new scene becomes the current program scene.",
    |backend, input| { backend.create_scene(input) }
);
typed_tool!(
    RemoveScene,
    "scene_remove",
    SceneBackend,
    NameInput,
    SceneOutput,
    MUTATING,
    "Remove the scene with the given name.",
    |backend, input| { backend.remove_scene(input) }
);
/// Registers the five typed scene tools.
pub fn register(registry: &mut ToolRegistry, backend: &Rc<dyn SceneBackend>) {
    registry.register(Box::new(ListScenes {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(CurrentScene {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(SetScene {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(CreateScene {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(RemoveScene {
        backend: Rc::clone(backend),
    }));
}

/// Generated tool schema contracts.
#[must_use]
pub fn schemas() -> Vec<serde_json::Value> {
    vec![
        crate::server::tool_schema::<ListScenes>(),
        crate::server::tool_schema::<CurrentScene>(),
        crate::server::tool_schema::<SetScene>(),
        crate::server::tool_schema::<CreateScene>(),
        crate::server::tool_schema::<RemoveScene>(),
    ]
}
