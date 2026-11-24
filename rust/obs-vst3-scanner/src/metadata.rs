use crate::cache::Class;
use crate::discovery;
use crate::platform;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
struct Module {
    #[serde(rename = "Name")]
    _name: String,
    #[serde(rename = "Version")]
    _version: String,
    #[serde(rename = "Factory Info")]
    factory: Factory,
    #[serde(rename = "Classes")]
    classes: Vec<Info>,
}

#[derive(Deserialize)]
struct Factory {
    #[serde(rename = "Vendor")]
    _vendor: String,
    #[serde(rename = "URL")]
    _url: String,
    #[serde(rename = "E-Mail")]
    _email: String,
    #[serde(rename = "Flags")]
    flags: Flags,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Flags {
    #[serde(rename = "Classes Discardable", default)]
    discardable: bool,
    #[serde(rename = "Component Non Discardable", default)]
    _component_non_discardable: bool,
    #[serde(rename = "Unicode", default)]
    _unicode: bool,
}

#[derive(Deserialize)]
struct Info {
    #[serde(rename = "CID")]
    id: String,
    #[serde(rename = "Category")]
    category: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Vendor")]
    _vendor: String,
    #[serde(rename = "Version")]
    _version: String,
    #[serde(rename = "SDKVersion")]
    _sdk_version: String,
    #[serde(rename = "Sub Categories", default)]
    sub_categories: Vec<String>,
    #[serde(rename = "Class Flags")]
    _class_flags: u32,
    #[serde(rename = "Cardinality")]
    _cardinality: u32,
}

pub(super) fn classes(path: &Path) -> Vec<Class> {
    let Some(root) = platform::metadata_root(path) else {
        return Vec::new();
    };
    let Ok(data) = fs::read_to_string(root.join("Contents/Resources/moduleinfo.json")) else {
        return Vec::new();
    };
    let data = data.strip_prefix('\u{feff}').unwrap_or(&data);
    let Ok(module) = json5::from_str::<Module>(data) else {
        return Vec::new();
    };
    if module.factory.flags.discardable {
        return Vec::new();
    }
    module
        .classes
        .into_iter()
        .filter(|info| info.category == discovery::AUDIO_MODULE_CLASS)
        .map(|info| {
            discovery::class(
                root,
                info.name,
                info.id,
                false,
                info.sub_categories.into_iter(),
            )
        })
        .collect()
}
