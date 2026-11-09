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
    let _ = source;
    todo!()
}
