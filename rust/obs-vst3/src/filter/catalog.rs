//! Scanner-derived choices shown by the OBS filter properties.

use crate::filter::settings::Location;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Class {
    pub id: String,
    pub path: String,
    pub name: String,
    pub plugin_name: String,
    pub effect: bool,
}

pub struct Catalog {
    classes: Vec<Class>,
    class_counts: HashMap<String, usize>,
}

impl Catalog {
    pub fn new(mut classes: Vec<Class>, mut is_available: impl FnMut(&str) -> bool) -> Self {
        classes.retain(|class| !class.path.is_empty() && is_available(&class.path));
        classes.sort_by_cached_key(|class| {
            (
                class.name.to_ascii_lowercase(),
                class.plugin_name.to_ascii_lowercase(),
                class.path.clone(),
                class.id.clone(),
            )
        });
        let mut class_counts = HashMap::new();
        for class in &classes {
            *class_counts.entry(class.path.clone()).or_insert(0) += 1;
        }
        Self {
            classes,
            class_counts,
        }
    }

    pub fn location(&self, id: &str) -> Option<Location<'_>> {
        self.classes
            .iter()
            .find(|class| class.id == id)
            .map(|class| Location {
                path: &class.path,
                name: &class.name,
            })
    }

    pub fn choices(&self) -> Vec<(String, &str)> {
        self.classes
            .iter()
            .filter(|class| class.effect)
            .map(|class| {
                let display = if self.class_counts[&class.path] > 1 {
                    format!("{} ({})", class.name, class.plugin_name)
                } else {
                    class.name.clone()
                };
                (display, class.id.as_str())
            })
            .collect()
    }
}
