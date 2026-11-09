//! Source tools: `sources_list`, `source_create`, `source_set_settings`,
//! `scene_item_add`, `scene_items_list`, `scene_item_set_visible` and
//! `scene_item_set_transform` (#3286, #3338).
//!
//! The tools talk to OBS through [`SourceBackend`], so they are tested with a
//! fake. Binding it to libobs is a follow-up (#3283).

use std::rc::Rc;

use super::EmptyInput;
use crate::server::ToolError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::server::ToolRegistry;

/// A source as listed by [`SourceBackend::sources`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
    /// Typed `ListSources` backend operation.
    /// # Errors
    /// Reports invalid state, a missing resource or processing failure.
    fn list_sources(&self, _input: EmptyInput) -> Result<SourcesOutput, ToolError> {
        Ok(SourcesOutput {
            sources: self.sources(),
        })
    }
    /// Typed `CreateSource` backend operation.
    /// # Errors
    /// Reports invalid state, a missing resource or processing failure.
    fn create_source(&self, input: CreateInput) -> Result<CreateOutput, ToolError> {
        nonempty(&input.name)?;
        nonempty(&input.kind)?;
        if input.position.is_some() && input.scene.is_none() {
            return Err(ToolError::processing(
                "`position` needs `scene`, the scene to add the source to",
            ));
        }
        if let Some(scene) = &input.scene {
            nonempty(scene)?;
        }
        self.create(&input.kind, &input.name, &Value::Object(input.settings))
            .map_err(ToolError::processing)?;
        let item = if let Some(scene) = &input.scene {
            Some(
                self.add_item(scene, &input.name, input.position.map(Pair::tuple))
                    .map_err(|error| {
                        ToolError::processing(format!(
                            "created source '{}' but could not add it to scene '{scene}': {error}",
                            input.name
                        ))
                    })?
                    .id,
            )
        } else {
            None
        };
        Ok(CreateOutput {
            name: input.name,
            kind: input.kind,
            scene: input.scene,
            item,
        })
    }
    /// Typed `SetSettings` backend operation.
    /// # Errors
    /// Reports invalid state, a missing resource or processing failure.
    fn update_settings(&self, input: SettingsInput) -> Result<NameOutput, ToolError> {
        nonempty(&input.name)?;
        self.set_settings(&input.name, &Value::Object(input.settings))
            .map_err(ToolError::not_found)?;
        Ok(NameOutput { name: input.name })
    }
    /// Typed `AddItem` backend operation.
    /// # Errors
    /// Reports invalid state, a missing resource or processing failure.
    fn add_scene_item(&self, input: AddItemInput) -> Result<ItemOutput, ToolError> {
        nonempty(&input.scene)?;
        nonempty(&input.source)?;
        let mut output = ItemOutput::from(
            self.add_item(&input.scene, &input.source, input.position.map(Pair::tuple))
                .map_err(ToolError::not_found)?,
        );
        output.scene = Some(input.scene);
        Ok(output)
    }
    /// Typed `ListItems` backend operation.
    /// # Errors
    /// Reports invalid state, a missing resource or processing failure.
    fn list_scene_items(&self, input: ItemsInput) -> Result<ItemsOutput, ToolError> {
        nonempty(&input.scene)?;
        Ok(ItemsOutput {
            items: self
                .items(&input.scene)
                .map_err(ToolError::not_found)?
                .into_iter()
                .map(ItemOutput::from)
                .collect(),
            scene: input.scene,
        })
    }
    /// Typed `SetVisible` backend operation.
    /// # Errors
    /// Reports invalid state, a missing resource or processing failure.
    fn update_visibility(&self, input: VisibleInput) -> Result<ItemReply, ToolError> {
        nonempty(&input.scene)?;
        let item = selector(input.item, input.source.as_deref())?;
        self.set_item_visible(&input.scene, &item, input.visible)
            .map_err(ToolError::not_found)?;
        Ok(ItemReply {
            scene: input.scene,
            item: input.item,
            source: input.source,
            visible: Some(input.visible),
        })
    }
    /// Typed `SetTransform` backend operation.
    /// # Errors
    /// Reports invalid state, a missing resource or processing failure.
    fn update_transform(&self, input: TransformInput) -> Result<ItemReply, ToolError> {
        nonempty(&input.scene)?;
        let item = selector(input.item, input.source.as_deref())?;
        let transform = Transform {
            position: input.position.map(Pair::tuple),
            scale: input.scale.map(Pair::tuple),
            rotation: input.rotation,
        };
        if transform == Transform::default() {
            return Err(ToolError::processing(
                "give at least one of `position`, `scale` or `rotation`",
            ));
        }
        self.set_item_transform(&input.scene, &item, &transform)
            .map_err(ToolError::not_found)?;
        Ok(ItemReply {
            scene: input.scene,
            item: input.item,
            source: input.source,
            visible: None,
        })
    }

    // Legacy backend hooks retained for existing implementers. New operations
    // use the typed request/response methods above.

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

/// Pixel position or scale factors.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Pair {
    /// Horizontal component.
    pub x: f64,
    /// Vertical component.
    pub y: f64,
}
impl Pair {
    fn tuple(self) -> (f64, f64) {
        (self.x, self.y)
    }
}
/// Source creation, optionally placing it in a scene.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateInput {
    /// Source kind id.
    pub kind: String,
    /// Unique name.
    pub name: String,
    /// Kind-specific settings.
    pub settings: Map<String, Value>,
    /// Scene receiving the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene: Option<String>,
    /// Initial position (requires scene).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Pair>,
}
/// Created source.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreateOutput {
    /// Source name.
    pub name: String,
    /// Source kind.
    pub kind: String,
    /// Scene receiving the new item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene: Option<String>,
    /// Created item id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<i64>,
}
/// Settings update.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SettingsInput {
    /// Source name.
    pub name: String,
    /// Kind-specific settings to merge.
    pub settings: Map<String, Value>,
}
/// Updated source name.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct NameOutput {
    /// Source name.
    pub name: String,
}
/// Source listing.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SourcesOutput {
    /// Sources in stable order.
    pub sources: Vec<SourceInfo>,
}
/// Place an existing source in a scene.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddItemInput {
    /// Scene name.
    pub scene: String,
    /// Source name.
    pub source: String,
    /// Initial position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Pair>,
}
/// Scene selection.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ItemsInput {
    /// Scene name.
    pub scene: String,
}
/// Listed scene item, preserving the existing wire fields.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ItemOutput {
    /// Item id.
    pub item: i64,
    /// Source name.
    pub source: String,
    /// Visibility.
    pub visible: bool,
    /// Canvas position.
    pub position: Pair,
    /// Scale factors.
    pub scale: Pair,
    /// Rotation in degrees.
    pub rotation: f64,
    /// Present when an item was added.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene: Option<String>,
}
impl From<ItemInfo> for ItemOutput {
    fn from(item: ItemInfo) -> Self {
        Self {
            item: item.id,
            source: item.source,
            visible: item.visible,
            position: Pair {
                x: item.position.0,
                y: item.position.1,
            },
            scale: Pair {
                x: item.scale.0,
                y: item.scale.1,
            },
            rotation: item.rotation,
            scene: None,
        }
    }
}
/// Items of a scene.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ItemsOutput {
    /// Scene name.
    pub scene: String,
    /// Items bottom to top.
    pub items: Vec<ItemOutput>,
}
/// Visibility update; exactly one of item/source must be specified.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleInput {
    /// Scene name.
    pub scene: String,
    /// Item id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<i64>,
    /// Source name instead of item id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// New visibility.
    pub visible: bool,
}
/// Transform update; at least one transform field is required.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TransformInput {
    /// Scene name.
    pub scene: String,
    /// Item id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<i64>,
    /// Source name instead of item id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// New position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Pair>,
    /// New scale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<Pair>,
    /// New rotation in degrees.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
}
/// Item mutation response, retaining the caller's selector.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ItemReply {
    /// Scene name.
    pub scene: String,
    /// Item id when selected by id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<i64>,
    /// Source name when selected by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Present for visibility changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
}
fn nonempty(name: &str) -> Result<(), ToolError> {
    if name.is_empty() {
        Err(ToolError::processing("name must be a non-empty string"))
    } else {
        Ok(())
    }
}
fn selector(item: Option<i64>, source: Option<&str>) -> Result<ItemRef, ToolError> {
    match (item, source) {
        (Some(id), None) => Ok(ItemRef::Id(id)),
        (None, Some(name)) => {
            nonempty(name)?;
            Ok(ItemRef::Source(name.to_owned()))
        }
        _ => Err(ToolError::processing(
            "give exactly one of `item` or `source`",
        )),
    }
}
typed_tool!(
    ListSources,
    "sources_list",
    SourceBackend,
    EmptyInput,
    SourcesOutput,
    READ_ONLY,
    "List all sources with their names and kind ids.",
    |backend, input| { backend.list_sources(input) }
);
typed_tool!(
    CreateSource,
    "source_create",
    SourceBackend,
    CreateInput,
    CreateOutput,
    MUTATING,
    "Create a source of the given kind with a name and settings. Give `scene` (and optionally `position`) to also add it to that scene as an item.",
    |backend, input| { backend.create_source(input) }
);
typed_tool!(
    SetSettings,
    "source_set_settings",
    SourceBackend,
    SettingsInput,
    NameOutput,
    MUTATING,
    "Merge settings into an existing source's settings.",
    |backend, input| { backend.update_settings(input) }
);
typed_tool!(
    AddItem,
    "scene_item_add",
    SourceBackend,
    AddItemInput,
    ItemOutput,
    MUTATING,
    "Add an existing source to a scene as a scene item, optionally at a position. A source only shows in the program output once it is an item of the program scene.",
    |backend, input| { backend.add_scene_item(input) }
);
typed_tool!(
    ListItems,
    "scene_items_list",
    SourceBackend,
    ItemsInput,
    ItemsOutput,
    READ_ONLY,
    "List the items of a scene (bottom to top) with their id, source name, visibility, position, scale and rotation.",
    |backend, input| { backend.list_scene_items(input) }
);
typed_tool!(
    SetVisible,
    "scene_item_set_visible",
    SourceBackend,
    VisibleInput,
    ItemReply,
    IDEMPOTENT,
    "Show or hide a scene item, named by `item` (id from scene_items_list) or `source`.",
    |backend, input| { backend.update_visibility(input) }
);
typed_tool!(
    SetTransform,
    "scene_item_set_transform",
    SourceBackend,
    TransformInput,
    ItemReply,
    MUTATING,
    "Set the position, scale and/or rotation of a scene item, named by `item` (id from scene_items_list) or `source`.",
    |backend, input| { backend.update_transform(input) }
);
/// Registers seven typed source tools.
pub fn register(registry: &mut ToolRegistry, backend: &Rc<dyn SourceBackend>) {
    registry.register(Box::new(ListSources {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(CreateSource {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(SetSettings {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(AddItem {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(ListItems {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(SetVisible {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(SetTransform {
        backend: Rc::clone(backend),
    }));
}

/// Generated tool schema contracts.
#[must_use]
pub fn schemas() -> Vec<serde_json::Value> {
    vec![
        crate::server::tool_schema::<ListSources>(),
        crate::server::tool_schema::<CreateSource>(),
        crate::server::tool_schema::<SetSettings>(),
        crate::server::tool_schema::<AddItem>(),
        crate::server::tool_schema::<ListItems>(),
        crate::server::tool_schema::<SetVisible>(),
        crate::server::tool_schema::<SetTransform>(),
    ]
}
