//! Which OBS module brought in which libraries (obs-rust/obs-studio#4525).
//!
//! When the exit watchdog fires, the main thread waits in some library. To
//! name the plugin behind it, the watchdog needs to know which module loaded
//! that library, also when the module loaded it itself (Advanced Scene
//! Switcher's sub-plugins and their libraries, #3739). libobs's module list
//! does not have that.
//!
//! The process's image list is in load order on Linux (`dl_iterate_phdr`)
//! and macOS (dyld): a `dlopen` adds the library and the dependencies it
//! pulls in at the end, and the entries before keep their place. libobs loads
//! one module after the other, so the libraries a module brings in while it
//! loads come right after it. The owner of an image is the last OBS module
//! before it in the list; images before the first module are OBS's own.
//! Images that appear later are marked by when they were first seen: during
//! post-load, or after startup (by the time the watchdog is armed).
//!
//! The table is built before the watchdog is armed and does not change, so
//! the watchdog can look an address up without allocating or locking.

use std::collections::{BTreeMap, HashSet};

/// One loaded image: its address range `[start, end)` and its path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub start: usize,
    pub end: usize,
    pub path: String,
}

/// Who brought an image in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Owner {
    /// Loaded before the first OBS module: OBS itself and its libraries.
    Obs,
    /// The OBS module, by its file name, that loaded the image or that the
    /// image is.
    Module(String),
    /// First seen after the modules' post-load.
    PostLoad,
    /// First seen when the watchdog was armed (loaded while OBS ran).
    AfterStartup,
}

/// An image of the table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub start: usize,
    pub end: usize,
    /// The file name, without the directory (the log shows no user paths).
    pub file: String,
    pub owner: Owner,
}

/// The file name of a path (`/` or `\` separated).
pub fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// The owner of each image of `images` (in load order), given the binary
/// paths of the OBS modules. A module is matched by its file name, or, for a
/// macOS bundle (`name.plugin`, whose binary is `name.plugin/Contents/MacOS/
/// name`), by a directory of the image's path.
pub fn owners(images: &[Image], modules: &[String]) -> Vec<Owner> {
    let names: HashSet<&str> = modules.iter().map(|m| file_name(m)).collect();
    let mut owner = Owner::Obs;
    images
        .iter()
        .map(|image| {
            if let Some(name) = image
                .path
                .split(['/', '\\'])
                .rev()
                .find(|part| names.contains(part))
            {
                owner = Owner::Module(name.to_string());
            }
            owner.clone()
        })
        .collect()
}

/// The images with their owners, sorted by address.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
    entries: Vec<Entry>,
}

impl Table {
    /// Builds the table from three image lists: after the modules loaded,
    /// after their post-load, and when the watchdog is armed. An image is
    /// placed by the first list it is in (by start address).
    pub fn build(
        after_load: &[Image],
        after_post_load: &[Image],
        at_arm: &[Image],
        modules: &[String],
    ) -> Table {
        let mut by_start: BTreeMap<usize, Entry> = BTreeMap::new();
        let mut add = |image: &Image, owner: Owner| {
            by_start.entry(image.start).or_insert_with(|| Entry {
                start: image.start,
                end: image.end,
                file: file_name(&image.path).to_string(),
                owner,
            });
        };
        for (image, owner) in after_load.iter().zip(owners(after_load, modules)) {
            add(image, owner);
        }
        for image in after_post_load {
            add(image, Owner::PostLoad);
        }
        for image in at_arm {
            add(image, Owner::AfterStartup);
        }
        Table {
            entries: by_start.into_values().collect(),
        }
    }

    /// A table from entries in any order (tests, and a table built by hand).
    pub fn from_entries(mut entries: Vec<Entry>) -> Table {
        entries.sort_by_key(|e| e.start);
        Table { entries }
    }

    /// The entries, sorted by start address.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The entry whose range holds `address`. No allocation.
    pub fn find(&self, address: usize) -> Option<&Entry> {
        let after = self.entries.partition_point(|e| e.start <= address);
        let entry = self.entries.get(after.checked_sub(1)?)?;
        (address < entry.end).then_some(entry)
    }
}

/// Whether this host can list its images ([`current`] is empty if not).
pub const SUPPORTED: bool = crate::platform::IMAGES_SUPPORTED;

/// The images loaded in this process now, in load order.
pub fn current() -> Vec<Image> {
    crate::platform::images()
}
