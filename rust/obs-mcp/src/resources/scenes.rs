//! The `obs://scenes` resource: the scene graph as JSON (#3288).
//!
//! It reuses [`SceneBackend`] and [`SourceBackend`], so the same fakes serve the
//! tools and the resource.

use std::rc::Rc;

use serde_json::json;

use super::{Resource, ResourceRegistry};
use crate::tools::scenes::SceneBackend;
use crate::tools::sources::SourceBackend;

/// The `obs://scenes` resource bound to its backends.
struct ScenesResource {
    scenes: Rc<dyn SceneBackend>,
    sources: Rc<dyn SourceBackend>,
}

impl Resource for ScenesResource {
    fn uri(&self) -> &str {
        "obs://scenes"
    }

    fn name(&self) -> &str {
        "scenes"
    }

    fn description(&self) -> &str {
        "The scene graph: the current program scene, every scene and every source."
    }

    fn mime_type(&self) -> &str {
        "application/json"
    }

    fn read(&self) -> Result<String, String> {
        let sources: Vec<_> = self
            .sources
            .sources()
            .into_iter()
            .map(|s| json!({ "name": s.name, "kind": s.kind }))
            .collect();
        Ok(json!({
            "current": self.scenes.current(),
            "scenes": self.scenes.scenes(),
            "sources": sources,
        })
        .to_string())
    }
}

/// Registers `obs://scenes` on `registry`, backed by `scenes` and `sources`.
pub fn register(
    registry: &mut ResourceRegistry,
    scenes: &Rc<dyn SceneBackend>,
    sources: &Rc<dyn SourceBackend>,
) {
    registry.register(Box::new(ScenesResource {
        scenes: Rc::clone(scenes),
        sources: Rc::clone(sources),
    }));
}
