//! Scene tools: `scenes_list`, `scene_get_current`, `scene_set_current`,
//! `scene_create` and `scene_remove` (#3285).
//!
//! The tools talk to OBS through [`SceneBackend`], so they are tested with a
//! fake. The real libobs binding is a follow-up (#3283); `obs-scene` holds only
//! the pure scene logic and has no program-scene state to call yet.

use std::rc::Rc;

use serde_json::Value;

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

impl Tool for SceneTool {
    fn name(&self) -> &str {
        todo!()
    }
    fn description(&self) -> &str {
        todo!()
    }
    fn input_schema(&self) -> Value {
        todo!()
    }
    fn call(&self, _arguments: &Value) -> Result<String, String> {
        let _ = (self.op, &self.backend);
        todo!()
    }
}

/// Registers the five scene tools on `registry`, backed by `backend`.
pub fn register(registry: &mut ToolRegistry, backend: &Rc<dyn SceneBackend>) {
    for op in [Op::List, Op::GetCurrent, Op::SetCurrent, Op::Create, Op::Remove] {
        registry.register(Box::new(SceneTool {
            op,
            backend: Rc::clone(backend),
        }));
    }
}
