//! Source tools: `sources_list`, `source_create`, `source_set_settings`,
//! `scene_item_set_visible` and `scene_item_set_transform` (#3286).
//!
//! The tools talk to OBS through [`SourceBackend`], so they are tested with a
//! fake. Binding it to libobs is a follow-up (#3283).

use std::rc::Rc;

use serde_json::{Map, Value, json};

use crate::server::{Tool, ToolRegistry};

/// A source as listed by [`SourceBackend::sources`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceInfo {
    /// The source's unique name.
    pub name: String,
    /// The source kind id, such as `color_source`.
    pub kind: String,
}

/// A partial scene item transform: only the `Some` fields change.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Transform {
    /// Position in canvas pixels, `(x, y)`.
    pub position: Option<(f64, f64)>,
    /// Scale factors, `(x, y)`.
    pub scale: Option<(f64, f64)>,
    /// Rotation in degrees.
    pub rotation: Option<f64>,
}

/// The source operations the tools need from OBS.
///
/// Methods take `&self`: libobs owns the state, so implementations use their
/// own interior mutability.
pub trait SourceBackend {
    /// Every source, in a stable order.
    fn sources(&self) -> Vec<SourceInfo>;
    /// Creates a source of `kind` called `name` with `settings`.
    ///
    /// # Errors
    ///
    /// Returns a message when the kind is unknown or the name is taken.
    fn create(&self, kind: &str, name: &str, settings: &Value) -> Result<(), String>;
    /// Merges `settings` into the settings of the source called `name`.
    ///
    /// # Errors
    ///
    /// Returns a message when no such source exists.
    fn set_settings(&self, name: &str, settings: &Value) -> Result<(), String>;
    /// Shows or hides the item for `source` in `scene`.
    ///
    /// # Errors
    ///
    /// Returns a message when the scene or the item does not exist.
    fn set_item_visible(&self, scene: &str, source: &str, visible: bool) -> Result<(), String>;
    /// Applies `transform` to the item for `source` in `scene`.
    ///
    /// # Errors
    ///
    /// Returns a message when the scene or the item does not exist.
    fn set_item_transform(
        &self,
        scene: &str,
        source: &str,
        transform: &Transform,
    ) -> Result<(), String>;
}

/// Which source operation a [`SourceTool`] performs.
#[derive(Clone, Copy)]
enum Op {
    List,
    Create,
    SetSettings,
    ItemSetVisible,
    ItemSetTransform,
}

/// One source tool bound to a backend.
struct SourceTool {
    op: Op,
    backend: Rc<dyn SourceBackend>,
}

/// A required, non-empty string argument.
fn str_arg<'a>(arguments: &'a Value, key: &str) -> Result<&'a str, String> {
    match arguments.get(key).and_then(Value::as_str) {
        Some(s) if !s.is_empty() => Ok(s),
        _ => Err(format!("argument `{key}` must be a non-empty string")),
    }
}

/// A required object argument.
fn object_arg<'a>(arguments: &'a Value, key: &str) -> Result<&'a Value, String> {
    match arguments.get(key) {
        Some(v) if v.is_object() => Ok(v),
        _ => Err(format!("argument `{key}` must be an object")),
    }
}

/// An optional `{x, y}` pair; both members must be finite numbers.
fn pair_arg(arguments: &Value, key: &str) -> Result<Option<(f64, f64)>, String> {
    let Some(v) = arguments.get(key) else {
        return Ok(None);
    };
    let bad = || format!("argument `{key}` must be an object with numeric `x` and `y`");
    let num = |k: &str| {
        v.get(k)
            .and_then(Value::as_f64)
            .filter(|n| n.is_finite())
            .ok_or_else(bad)
    };
    if !v.is_object() {
        return Err(bad());
    }
    Ok(Some((num("x")?, num("y")?)))
}

/// Schema for an `{x, y}` pair.
fn pair_schema(description: &str) -> Value {
    json!({
        "type": "object",
        "properties": { "x": { "type": "number" }, "y": { "type": "number" } },
        "required": ["x", "y"],
        "additionalProperties": false,
        "description": description,
    })
}

impl Tool for SourceTool {
    fn name(&self) -> &str {
        match self.op {
            Op::List => "sources_list",
            Op::Create => "source_create",
            Op::SetSettings => "source_set_settings",
            Op::ItemSetVisible => "scene_item_set_visible",
            Op::ItemSetTransform => "scene_item_set_transform",
        }
    }

    fn description(&self) -> &str {
        match self.op {
            Op::List => "List all sources with their names and kind ids.",
            Op::Create => "Create a source of the given kind with a name and settings.",
            Op::SetSettings => "Merge settings into an existing source's settings.",
            Op::ItemSetVisible => "Show or hide a source's item in a scene.",
            Op::ItemSetTransform => {
                "Set the position, scale and/or rotation of a source's item in a scene."
            }
        }
    }

    fn input_schema(&self) -> Value {
        let name = json!({ "type": "string", "minLength": 1 });
        match self.op {
            Op::List => json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
            Op::Create => json!({
                "type": "object",
                "properties": {
                    "kind": { "type": "string", "minLength": 1, "description": "Source kind id." },
                    "name": { "type": "string", "minLength": 1, "description": "Source name." },
                    "settings": { "type": "object", "description": "Kind-specific settings." },
                },
                "required": ["kind", "name", "settings"],
                "additionalProperties": false,
            }),
            Op::SetSettings => json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "minLength": 1, "description": "Source name." },
                    "settings": { "type": "object", "description": "Settings to merge in." },
                },
                "required": ["name", "settings"],
                "additionalProperties": false,
            }),
            Op::ItemSetVisible => json!({
                "type": "object",
                "properties": {
                    "scene": name,
                    "source": name,
                    "visible": { "type": "boolean" },
                },
                "required": ["scene", "source", "visible"],
                "additionalProperties": false,
            }),
            Op::ItemSetTransform => json!({
                "type": "object",
                "properties": {
                    "scene": name,
                    "source": name,
                    "position": pair_schema("Position in canvas pixels."),
                    "scale": pair_schema("Scale factors."),
                    "rotation": { "type": "number", "description": "Rotation in degrees." },
                },
                "required": ["scene", "source"],
                "additionalProperties": false,
            }),
        }
    }

    fn call(&self, arguments: &Value) -> Result<String, String> {
        let result = match self.op {
            Op::List => {
                let sources: Vec<Value> = self
                    .backend
                    .sources()
                    .into_iter()
                    .map(|s| json!({ "name": s.name, "kind": s.kind }))
                    .collect();
                json!({ "sources": sources })
            }
            Op::Create => {
                let kind = str_arg(arguments, "kind")?;
                let name = str_arg(arguments, "name")?;
                let settings = object_arg(arguments, "settings")?;
                self.backend.create(kind, name, settings)?;
                json!({ "name": name, "kind": kind })
            }
            Op::SetSettings => {
                let name = str_arg(arguments, "name")?;
                let settings = object_arg(arguments, "settings")?;
                self.backend.set_settings(name, settings)?;
                json!({ "name": name })
            }
            Op::ItemSetVisible => {
                let scene = str_arg(arguments, "scene")?;
                let source = str_arg(arguments, "source")?;
                let visible = arguments
                    .get("visible")
                    .and_then(Value::as_bool)
                    .ok_or("argument `visible` must be a boolean")?;
                self.backend.set_item_visible(scene, source, visible)?;
                json!({ "scene": scene, "source": source, "visible": visible })
            }
            Op::ItemSetTransform => {
                let scene = str_arg(arguments, "scene")?;
                let source = str_arg(arguments, "source")?;
                let rotation = match arguments.get("rotation") {
                    None => None,
                    Some(v) => Some(
                        v.as_f64()
                            .filter(|n| n.is_finite())
                            .ok_or("argument `rotation` must be a number")?,
                    ),
                };
                let transform = Transform {
                    position: pair_arg(arguments, "position")?,
                    scale: pair_arg(arguments, "scale")?,
                    rotation,
                };
                if transform == Transform::default() {
                    return Err("give at least one of `position`, `scale` or `rotation`".into());
                }
                self.backend.set_item_transform(scene, source, &transform)?;
                let mut out = Map::new();
                out.insert("scene".into(), scene.into());
                out.insert("source".into(), source.into());
                json!(out)
            }
        };
        Ok(result.to_string())
    }
}

/// Registers the five source tools on `registry`, backed by `backend`.
pub fn register(registry: &mut ToolRegistry, backend: &Rc<dyn SourceBackend>) {
    for op in [
        Op::List,
        Op::Create,
        Op::SetSettings,
        Op::ItemSetVisible,
        Op::ItemSetTransform,
    ] {
        registry.register(Box::new(SourceTool {
            op,
            backend: Rc::clone(backend),
        }));
    }
}
