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
    index: Option<HashMap<Box<[u8]>, usize, RandomState>>,
}

impl<T> Default for NameTable<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            index: None,
        }
    }
}

impl<T> NameTable<T> {
    /// The entry stored under `name`, if any.
    pub(crate) fn get(&self, name: &[u8]) -> Option<&T> {
        let position = match &self.index {
            Some(index) => *index.get(name)?,
            // Length and last byte first, inline: names in one table tend to
            // share a prefix ("s00".."s15", "source_*"), and a libc memcmp
            // call per entry was the largest cost of a lookup (#3350).
            None => self.items.iter().position(|(key, _)| {
                key.len() == name.len() && key.last() == name.last() && &**key == name
            })?,
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
