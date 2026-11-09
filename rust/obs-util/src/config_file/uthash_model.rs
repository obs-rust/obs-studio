//! A model of the uthash tables `libobs/util/config-file.c` keeps its
//! sections and items in (obs-rust/obs-studio#681).
//!
//! C stores duplicates: `HASH_ADD_STR` never replaces an existing key, and
//! `HASH_FIND_STR` returns the first match on a bucket chain. Which of
//! several same-key entries that is depends on uthash internals: entries are
//! prepended to their bucket's chain, and when a chain reaches
//! `(expand_mult + 1) * 10` entries the bucket array doubles and every chain
//! is relinked, which reverses the order of same-key entries. This module
//! reproduces that bookkeeping (uthash 2.3.0, `HASH_FUNCTION` overridden to
//! `HASH_SFH` by `libobs/util/uthash.h`) so lookups, `config_set_*`,
//! `config_remove_value` and `config_has_user_value` pick the entry C picks.
//!
//! [`UtList`] keeps the entries in insertion order, which is the iteration
//! order of C's `HASH_ITER` (the `hh.next` list), and an id-based model of
//! the bucket chains used only for lookups.

const INITIAL_BUCKETS: usize = 32;
const INITIAL_LOG2: u32 = 5;
const BKT_CAPACITY_THRESH: u32 = 10;

/// `HASH_SFH` (Paul Hsieh's SuperFastHash) with uthash's `get16bits`, a
/// little-endian 16-bit load.
fn sfh(key: &[u8]) -> u32 {
    let get16 = |i: usize| u32::from(key[i]) + (u32::from(key[i + 1]) << 8);
    let len = key.len();
    let rem = len & 3;
    let mut h: u32 = 0xcafe_babe;
    let mut i = 0;
    for _ in 0..(len >> 2) {
        h = h.wrapping_add(get16(i));
        let tmp = (get16(i + 2) << 11) ^ h;
        h = (h << 16) ^ tmp;
        i += 4;
        h = h.wrapping_add(h >> 11);
    }
    match rem {
        3 => {
            h = h.wrapping_add(get16(i));
            h ^= h << 16;
            h ^= u32::from(key[i + 2]) << 18;
            h = h.wrapping_add(h >> 11);
        }
        2 => {
            h = h.wrapping_add(get16(i));
            h ^= h << 11;
            h = h.wrapping_add(h >> 17);
        }
        1 => {
            h = h.wrapping_add(u32::from(key[i]));
            h ^= h << 10;
            h = h.wrapping_add(h >> 1);
        }
        _ => {}
    }
    h ^= h << 3;
    h = h.wrapping_add(h >> 5);
    h ^= h << 4;
    h = h.wrapping_add(h >> 17);
    h ^= h << 25;
    h = h.wrapping_add(h >> 6);
    h
}

#[derive(Debug)]
struct Node {
    id: u64,
    hashv: u32,
    key: Vec<u8>,
}

/// `UT_hash_bucket`. The chain is stored tail first: the last element is
/// uthash's chain head, so prepending (`HASH_ADD_TO_BKT`) is an O(1) push
/// and a long chain of duplicates loads in linear time, as in C.
#[derive(Debug, Default)]
struct Bucket {
    chain: Vec<Node>,
    expand_mult: u32,
}

/// `UT_hash_table`.
#[derive(Debug)]
struct Model {
    buckets: Vec<Bucket>,
    log2: u32,
    num_items: u32,
    ineff_expands: u32,
    noexpand: bool,
}

impl Model {
    fn new() -> Self {
        Self {
            buckets: (0..INITIAL_BUCKETS).map(|_| Bucket::default()).collect(),
            log2: INITIAL_LOG2,
            num_items: 0,
            ineff_expands: 0,
            noexpand: false,
        }
    }

    fn bucket_of(&self, hashv: u32) -> usize {
        hashv as usize & (self.buckets.len() - 1)
    }

    /// `HASH_ADD_TO_TABLE` / `HASH_ADD_TO_BKT`.
    fn add(&mut self, node: Node) {
        self.num_items += 1;
        let b = self.bucket_of(node.hashv);
        let bucket = &mut self.buckets[b];
        bucket.chain.push(node);
        let count = bucket.chain.len() as u32;
        if count >= (bucket.expand_mult + 1) * BKT_CAPACITY_THRESH && !self.noexpand {
            self.expand();
        }
    }

    /// `HASH_EXPAND_BUCKETS`.
    fn expand(&mut self) {
        let old_len = self.buckets.len();
        let new_len = old_len * 2;
        let ideal = (self.num_items >> (self.log2 + 1))
            + u32::from(self.num_items & (new_len as u32 - 1) != 0);
        let mut nonideal = 0u32;
        let mut fresh: Vec<Bucket> = (0..new_len).map(|_| Bucket::default()).collect();
        for old in std::mem::take(&mut self.buckets) {
            // Walk each old chain from its head, as uthash does.
            for node in old.chain.into_iter().rev() {
                let nb = &mut fresh[node.hashv as usize & (new_len - 1)];
                let count = nb.chain.len() as u32 + 1;
                if count > ideal {
                    nonideal += 1;
                    if count > nb.expand_mult * ideal {
                        nb.expand_mult += 1;
                    }
                }
                nb.chain.push(node);
            }
        }
        self.buckets = fresh;
        self.log2 += 1;
        self.ineff_expands = if nonideal > (self.num_items >> 1) {
            self.ineff_expands + 1
        } else {
            0
        };
        if self.ineff_expands > 1 {
            self.noexpand = true;
        }
    }

    /// `HASH_FIND`: the first chain entry with the same hash and key.
    fn find(&self, key: &[u8]) -> Option<u64> {
        let hashv = sfh(key);
        self.buckets[self.bucket_of(hashv)]
            .chain
            .iter()
            .rev()
            .find(|n| n.hashv == hashv && n.key == key)
            .map(|n| n.id)
    }

    /// `HASH_DELETE_HH` (bucket part).
    fn remove(&mut self, id: u64, hashv: u32) {
        let b = self.bucket_of(hashv);
        self.buckets[b].chain.retain(|n| n.id != id);
        self.num_items -= 1;
    }
}

/// An entry with a C-string key.
pub(super) trait Keyed {
    fn key(&self) -> &[u8];
}

/// One `HASH_ADD_STR` table: entries in insertion order plus the model used
/// to pick which duplicate a lookup finds.
#[derive(Debug)]
pub(super) struct UtList<T> {
    /// `(id, entry)`; ids increase along the vector.
    entries: Vec<(u64, T)>,
    model: Model,
    next_id: u64,
}

impl<T> Default for UtList<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            model: Model::new(),
            next_id: 0,
        }
    }
}

impl<T: Keyed> UtList<T> {
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Entries in insertion order (`HASH_ITER`).
    pub(super) fn iter(&self) -> impl Iterator<Item = &T> {
        self.entries.iter().map(|(_, e)| e)
    }

    pub(super) fn get(&self, idx: usize) -> Option<&T> {
        self.entries.get(idx).map(|(_, e)| e)
    }

    /// `HASH_ADD_STR`: appends, keeping any entry with the same key.
    pub(super) fn push(&mut self, entry: T) {
        let id = self.next_id;
        self.next_id += 1;
        let key = entry.key().to_vec();
        self.model.add(Node {
            id,
            hashv: sfh(&key),
            key,
        });
        self.entries.push((id, entry));
    }

    fn position(&self, id: u64) -> usize {
        self.entries
            .binary_search_by_key(&id, |(i, _)| *i)
            .expect("model ids refer to live entries")
    }

    /// `HASH_FIND_STR`.
    pub(super) fn find(&self, key: &[u8]) -> Option<&T> {
        let id = self.model.find(key)?;
        Some(&self.entries[self.position(id)].1)
    }

    pub(super) fn find_mut(&mut self, key: &[u8]) -> Option<&mut T> {
        let id = self.model.find(key)?;
        let pos = self.position(id);
        Some(&mut self.entries[pos].1)
    }

    /// `HASH_FIND_STR` then `HASH_DELETE`; whether an entry was removed.
    pub(super) fn remove(&mut self, key: &[u8]) -> bool {
        let Some(id) = self.model.find(key) else {
            return false;
        };
        let pos = self.position(id);
        let (_, entry) = self.entries.remove(pos);
        let hashv = sfh(entry.key());
        if self.entries.is_empty() {
            // Deleting the only entry frees the whole uthash table.
            self.model = Model::new();
        } else {
            self.model.remove(id, hashv);
        }
        true
    }
}
