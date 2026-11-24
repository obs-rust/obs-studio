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

#[cfg(test)]
mod tests {
    use super::*;

    fn class(id: &str, path: &str, name: &str, plugin_name: &str, effect: bool) -> Class {
        Class {
            id: id.into(),
            path: path.into(),
            name: name.into(),
            plugin_name: plugin_name.into(),
            effect,
        }
    }

    #[test]
    fn unavailable_classes_are_removed_and_instruments_still_count_toward_module_size() {
        let catalog = Catalog::new(
            vec![
                class("z", "music", "Tone", "Music", false),
                class("a", "music", "Amp", "Music", true),
                class("b", "standalone", "Bass", "Bass", true),
                class("c", "removed", "Compressor", "Removed", true),
            ],
            |path| path != "removed",
        );
        assert_eq!(
            catalog.choices(),
            vec![("Amp (Music)".to_owned(), "a"), ("Bass".to_owned(), "b")]
        );
        assert_eq!(catalog.location("c"), None);
        assert_eq!(
            catalog.location("z"),
            Some(Location {
                path: "music",
                name: "Tone"
            })
        );
    }

    #[test]
    fn duplicate_id_resolution_uses_the_first_case_insensitively_sorted_class() {
        let catalog = Catalog::new(
            vec![
                class("same", "z", "AMP", "zebra", true),
                class("same", "a", "amp", "Alpine", true),
            ],
            |_| true,
        );
        assert_eq!(
            catalog.location("same"),
            Some(Location {
                path: "a",
                name: "amp"
            })
        );
    }
}
