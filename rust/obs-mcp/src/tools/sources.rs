//! Source tools: `sources_list`, `source_create`, `source_set_settings`,
//! `scene_item_add`, `scene_items_list`, `scene_item_set_visible` and
//! `scene_item_set_transform` (#3286, #3338).
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

/// A scene item as listed by [`SourceBackend::items`].
#[derive(Clone, Debug, PartialEq)]
pub struct ItemInfo {
    /// The item id, unique within its scene.
    pub id: i64,
    /// Name of the source the item shows.
    pub source: String,
    /// Whether the item is visible.
    pub visible: bool,
    /// Position in canvas pixels, `(x, y)`.
    pub position: (f64, f64),
    /// Scale factors, `(x, y)`.
    pub scale: (f64, f64),
    /// Rotation in degrees.
    pub rotation: f64,
}

/// Names a scene item: by the id `scene_items_list` returns, or by the name
/// of its source (the first item of that source).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemRef {
    /// The item id.
    Id(i64),
    /// The name of the item's source.
    Source(String),
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
    /// Adds an item for the existing `source` to `scene`, at `position` when
    /// given, and returns it.
    ///
    /// # Errors
    ///
    /// Returns a message when the scene or the source does not exist.
    fn add_item(
        &self,
        scene: &str,
        source: &str,
        position: Option<(f64, f64)>,
    ) -> Result<ItemInfo, String>;
    /// The items of `scene`, bottom to top.
    ///
    /// # Errors
    ///
    /// Returns a message when the scene does not exist.
    fn items(&self, scene: &str) -> Result<Vec<ItemInfo>, String>;
    /// Shows or hides `item` in `scene`.
    ///
    /// # Errors
    ///
    /// Returns a message when the scene or the item does not exist.
    fn set_item_visible(&self, scene: &str, item: &ItemRef, visible: bool) -> Result<(), String>;
    /// Applies `transform` to `item` in `scene`.
    ///
    /// # Errors
    ///
    /// Returns a message when the scene or the item does not exist.
    fn set_item_transform(
        &self,
        scene: &str,
        item: &ItemRef,
        transform: &Transform,
    ) -> Result<(), String>;
}

/// Which source operation a [`SourceTool`] performs.
#[derive(Clone, Copy)]
enum Op {
    List,
    Create,
    SetSettings,
    ItemAdd,
    ItemsList,
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

/// The item a tool names: `item` (an id) or `source`, exactly one of them.
fn item_arg(arguments: &Value) -> Result<ItemRef, String> {
    match (arguments.get("item"), arguments.get("source")) {
        (Some(_), Some(_)) => Err("give `item` or `source`, not both".into()),
        (Some(id), None) => id
            .as_i64()
            .map(ItemRef::Id)
            .ok_or_else(|| "argument `item` must be an integer item id".into()),
        (None, Some(_)) => str_arg(arguments, "source").map(|s| ItemRef::Source(s.to_owned())),
        (None, None) => Err("give `item` (an id from scene_items_list) or `source`".into()),
    }
}

/// The reply of an item tool: the scene, the item as it was named, and `extra`.
fn item_reply(scene: &str, item: &ItemRef, extra: Value) -> Value {
    let mut out = Map::new();
    out.insert("scene".into(), scene.into());
    match item {
        ItemRef::Id(id) => out.insert("item".into(), (*id).into()),
        ItemRef::Source(source) => out.insert("source".into(), source.as_str().into()),
    };
    if let Value::Object(extra) = extra {
        out.extend(extra);
    }
    Value::Object(out)
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
            Op::ItemAdd => "scene_item_add",
            Op::ItemsList => "scene_items_list",
            Op::ItemSetVisible => "scene_item_set_visible",
            Op::ItemSetTransform => "scene_item_set_transform",
        }
    }

    fn description(&self) -> &str {
        match self.op {
            Op::List => "List all sources with their names and kind ids.",
            Op::Create => {
                "Create a source of the given kind with a name and settings. Give `scene` \
                 (and optionally `position`) to also add it to that scene as an item."
            }
            Op::SetSettings => "Merge settings into an existing source's settings.",
            Op::ItemAdd => {
                "Add an existing source to a scene as a scene item, optionally at a position. \
                 A source only shows in the program output once it is an item of the program scene."
            }
            Op::ItemsList => {
                "List the items of a scene (bottom to top) with their id, source name, \
                 visibility, position, scale and rotation."
            }
            Op::ItemSetVisible => {
                "Show or hide a scene item, named by `item` (id from scene_items_list) or `source`."
            }
            Op::ItemSetTransform => {
                "Set the position, scale and/or rotation of a scene item, named by `item` \
                 (id from scene_items_list) or `source`."
            }
        }
    }

    fn input_schema(&self) -> Value {
        let name = json!({ "type": "string", "minLength": 1 });
        let item_id = json!({
            "type": "integer",
            "description": "Item id from scene_items_list; give this or `source`.",
        });
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
                    "scene": { "type": "string", "minLength": 1, "description": "Scene to add the new source to as an item." },
                    "position": pair_schema("Item position in canvas pixels; needs `scene`."),
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
            Op::ItemAdd => json!({
                "type": "object",
                "properties": {
                    "scene": name,
                    "source": name,
                    "position": pair_schema("Item position in canvas pixels."),
                },
                "required": ["scene", "source"],
                "additionalProperties": false,
            }),
            Op::ItemsList => json!({
                "type": "object",
                "properties": { "scene": name },
                "required": ["scene"],
                "additionalProperties": false,
            }),
            Op::ItemSetVisible => json!({
                "type": "object",
                "properties": {
                    "scene": name,
                    "item": item_id,
                    "source": name,
                    "visible": { "type": "boolean" },
                },
                "required": ["scene", "visible"],
                "additionalProperties": false,
            }),
            Op::ItemSetTransform => json!({
                "type": "object",
                "properties": {
                    "scene": name,
                    "item": item_id,
                    "source": name,
                    "position": pair_schema("Position in canvas pixels."),
                    "scale": pair_schema("Scale factors."),
                    "rotation": { "type": "number", "description": "Rotation in degrees." },
                },
                "required": ["scene"],
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
                todo!("source_create with scene (#3338)")
            }
            Op::SetSettings => {
                let name = str_arg(arguments, "name")?;
                let settings = object_arg(arguments, "settings")?;
                self.backend.set_settings(name, settings)?;
                json!({ "name": name })
            }
            Op::ItemAdd | Op::ItemsList => todo!("scene_item_add, scene_items_list (#3338)"),
            Op::ItemSetVisible => {
                let scene = str_arg(arguments, "scene")?;
                let item = item_arg(arguments)?;
                let visible = arguments
                    .get("visible")
                    .and_then(Value::as_bool)
                    .ok_or("argument `visible` must be a boolean")?;
                self.backend.set_item_visible(scene, &item, visible)?;
                item_reply(scene, &item, json!({ "visible": visible }))
            }
            Op::ItemSetTransform => {
                let scene = str_arg(arguments, "scene")?;
                let item = item_arg(arguments)?;
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
                self.backend.set_item_transform(scene, &item, &transform)?;
                item_reply(scene, &item, json!({}))
            }
        };
        Ok(result.to_string())
    }
}

/// Registers the seven source tools on `registry`, backed by `backend`.
pub fn register(registry: &mut ToolRegistry, backend: &Rc<dyn SourceBackend>) {
    for op in [
        Op::List,
        Op::Create,
        Op::SetSettings,
        Op::ItemAdd,
        Op::ItemsList,
        Op::ItemSetVisible,
        Op::ItemSetTransform,
    ] {
        registry.register(Box::new(SourceTool {
            op,
            backend: Rc::clone(backend),
        }));
    }
}
