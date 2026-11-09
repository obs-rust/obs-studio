//! Manifest field selection, independent of the filesystem and C ownership.

/// The existing OBS data reader supplies typed defaults for missing or invalid values.
pub trait ManifestSource {
    fn open(&mut self) -> bool;
    fn string(&self, key: &str) -> Vec<u8>;
    fn url(&self, key: &str) -> Vec<u8>;
    fn boolean(&self, key: &str) -> bool;
}

#[derive(Debug, PartialEq, Eq)]
pub struct Metadata {
    /// display_name, id, version, os_arch, name, description, long_description,
    /// repository, website, support (in the original C read order).
    pub strings: [Vec<u8>; 10],
    pub has_banner: bool,
    pub has_icon: bool,
}

pub fn load(source: &mut impl ManifestSource) -> Option<Metadata> {
    if !source.open() {
        return None;
    }
    Some(Metadata {
        strings: [
            source.string("display_name"),
            source.string("id"),
            source.string("version"),
            source.string("os_arch"),
            source.string("name"),
            source.string("description"),
            source.string("long_description"),
            source.url("repository"),
            source.url("website"),
            source.url("support"),
        ],
        has_banner: source.boolean("has_banner"),
        has_icon: source.boolean("has_icon"),
    })
}
