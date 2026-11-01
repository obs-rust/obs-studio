//! Insertion-ordered table looked up by name (#3214).
//!
//! The proc and signal handlers scan their entries by name on every call, as
//! the C does. Small tables keep that scan, which beats hashing for a few
//! entries. Once a table grows past [`INDEX_AT`] entries, a hash index maps
//! each name to its position. Entries are never removed, so positions stay
//! valid. Insertion order is unchanged either way.

use std::collections::HashMap;

use foldhash::fast::RandomState;

/// The entry count above which lookups go through the hash index.
pub(crate) const INDEX_AT: usize = 16;

pub(crate) struct NameTable<T> {
    items: Vec<(Box<[u8]>, T)>,
    /// [`tag`] of each item's name, in item order: the small-table scan
    /// reads this contiguous array instead of each name's heap copy (#3350).
    tags: Vec<u32>,
    index: Option<HashMap<Box<[u8]>, usize, RandomState>>,
}

impl<T> Default for NameTable<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            tags: Vec::new(),
            index: None,
        }
    }
}

impl<T> NameTable<T> {
    /// The entry stored under `name`, if any.
    pub(crate) fn get(&self, name: &[u8]) -> Option<&T> {
        let position = match &self.index {
            Some(index) => *index.get(name)?,
            // Length and last byte first, from the contiguous tags: names in
            // one table tend to share a prefix ("s00".."s15", "source_*"),
            // and a libc memcmp call per entry was the largest cost of a
            // lookup (#3350).
            None => {
                let want = tag(name);
                self.tags
                    .iter()
                    .zip(&self.items)
                    .position(|(&t, (key, _))| t == want && &**key == name)?
            }
        };
        Some(&self.items[position].1)
    }

    /// Stores `item` under `name` unless the name is taken; the first entry
    /// wins. True when stored.
    pub(crate) fn insert(&mut self, name: &[u8], item: T) -> bool {
        if self.get(name).is_some() {
            return false;
        }
        let position = self.items.len();
        self.items.push((name.into(), item));
        self.tags.push(tag(name));
        match &mut self.index {
            Some(index) => {
                index.insert(name.into(), position);
            }
            None if self.items.len() > INDEX_AT => {
                self.index = Some(
                    self.items
                        .iter()
                        .enumerate()
                        .map(|(i, (key, _))| (key.clone(), i))
                        .collect(),
                );
            }
            None => {}
        }
        true
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// A name's length (low 24 bits) and last byte: equal names have equal tags.
fn tag(name: &[u8]) -> u32 {
    ((name.len() as u32) << 8) | u32::from(name.last().copied().unwrap_or(0))
}
