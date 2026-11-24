//! Scanner-derived choices shown by the OBS filter properties.

use crate::filter::settings::Location;

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
}

impl Catalog {
    pub fn new(_classes: Vec<Class>, _is_available: impl FnMut(&str) -> bool) -> Self {
        todo!()
    }

    pub fn location(&self, _id: &str) -> Option<Location<'_>> {
        todo!()
    }

    pub fn choices(&self) -> Vec<(String, &str)> {
        todo!()
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
        let catalog = Catalog::new(vec![
            class("z", "music", "Tone", "Music", false),
            class("a", "music", "Amp", "Music", true),
            class("b", "standalone", "Bass", "Bass", true),
            class("c", "removed", "Compressor", "Removed", true),
        ], |path| path != "removed");
        assert_eq!(catalog.choices(), vec![("Amp (Music)".to_owned(), "a"), ("Bass".to_owned(), "b")]);
        assert_eq!(catalog.location("c"), None);
        assert_eq!(catalog.location("z"), Some(Location { path: "music", name: "Tone" }));
    }

    #[test]
    fn duplicate_id_resolution_uses_the_first_case_insensitively_sorted_class() {
        let catalog = Catalog::new(vec![
            class("same", "z", "AMP", "zebra", true),
            class("same", "a", "amp", "Alpine", true),
        ], |_| true);
        assert_eq!(catalog.location("same"), Some(Location { path: "a", name: "amp" }));
    }
}
