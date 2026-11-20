//! Tier 1: every assertion of the unchanged test_hotkey_key_translation.c
//! against the safe query and store. The table hands out handles into an
//! arena of owned strings, so identity (a handle) and content (its bytes)
//! are checked apart, as the C test checks pointers and strings. Live
//! allocations stand in for `bnum_allocs()`.
use obs_hotkey::key_translation::{KeyTranslations, OBS_KEY_NONE, store, translation};

/// Any two distinct keys other than `OBS_KEY_NONE`.
const OBS_KEY_A: i32 = 0x41;
const OBS_KEY_B: i32 = 0x42;
const KEYS: usize = 0x200;

/// A name: its arena slot. Caller-owned names live in the arena too but are
/// never counted as allocations.
type Handle = usize;

struct Table {
    slots: Vec<Option<Handle>>,
    arena: Vec<Option<Vec<u8>>>,
    /// Handles the table allocated and has not released.
    owned: Vec<Handle>,
    forbid_reads: bool,
    reads: u32,
    releases: u32,
}

impl Table {
    fn new() -> Self {
        Self {
            slots: vec![None; KEYS],
            arena: Vec::new(),
            owned: Vec::new(),
            forbid_reads: false,
            reads: 0,
            releases: 0,
        }
    }

    /// A caller-owned name (`char translation[] = "..."`).
    fn caller(&mut self, text: &str) -> Handle {
        self.arena.push(Some(text.as_bytes().to_vec()));
        self.arena.len() - 1
    }

    fn text(&self, handle: Handle) -> &[u8] {
        self.arena[handle].as_deref().expect("live name")
    }

    fn slot(&self, key: i32) -> Option<Handle> {
        self.slots[usize::try_from(key).unwrap()]
    }

    fn allocations(&self) -> usize {
        self.owned.len()
    }

    /// The C teardown: free each stored name, as obs_hotkeys_free does.
    fn teardown(mut self, before: usize) {
        for key in 0..KEYS {
            if let Some(handle) = self.slots[key].take() {
                self.owned.retain(|h| *h != handle);
            }
        }
        assert_eq!(self.allocations(), before);
    }
}

impl KeyTranslations for Table {
    type Text = Handle;
    fn get(&mut self, key: i32) -> Option<Handle> {
        assert!(!self.forbid_reads, "core read");
        self.reads += 1;
        self.slot(key)
    }
    fn set(&mut self, key: i32, value: Option<Handle>) {
        assert!(!self.forbid_reads, "core read");
        self.reads += 1;
        self.slots[usize::try_from(key).unwrap()] = value;
    }
    fn release(&mut self, value: Option<Handle>) {
        self.releases += 1;
        if let Some(handle) = value {
            let before = self.owned.len();
            self.owned.retain(|h| *h != handle);
            assert_eq!(self.owned.len() + 1, before, "release of an owned name");
            self.arena[handle] = None;
        }
    }
    fn duplicate(&mut self, value: Handle) -> Handle {
        let copy = self.text(value).to_vec();
        self.arena.push(Some(copy));
        let handle = self.arena.len() - 1;
        self.owned.push(handle);
        handle
    }
}

#[test]
fn none_without_core() {
    let mut t = Table::new();
    let before = t.allocations();
    let fallback = t.caller("default");
    t.forbid_reads = true;
    assert_eq!(translation(&mut t, OBS_KEY_NONE, Some(fallback)), None);
    assert_eq!(translation(&mut t, OBS_KEY_NONE, None), None);
    assert_eq!(t.reads, 0);
    t.forbid_reads = false;
    t.teardown(before);
}

#[test]
fn stored_pointer() {
    let mut t = Table::new();
    let before = t.allocations();
    let name = t.caller("translated A");
    let fallback = t.caller("default");
    store(&mut t, OBS_KEY_A, Some(name));
    let stored = t.slot(OBS_KEY_A).expect("stored");
    assert_ne!(stored, name);
    assert_eq!(t.text(stored), t.text(name));
    assert_eq!(translation(&mut t, OBS_KEY_A, Some(fallback)), Some(stored));
    assert_eq!(translation(&mut t, OBS_KEY_A, None), Some(stored));
    assert_eq!(t.allocations(), before + 1);
    t.teardown(before);
}

#[test]
fn unset_fallback() {
    let mut t = Table::new();
    let before = t.allocations();
    let fallback = t.caller("default");
    let empty = t.caller("");
    assert_eq!(
        translation(&mut t, OBS_KEY_B, Some(fallback)),
        Some(fallback)
    );
    assert_eq!(translation(&mut t, OBS_KEY_B, Some(empty)), Some(empty));
    assert_eq!(t.slot(OBS_KEY_B), None);
    t.teardown(before);
}

#[test]
fn null_default() {
    let mut t = Table::new();
    let before = t.allocations();
    assert_eq!(translation(&mut t, OBS_KEY_B, None), None);
    t.teardown(before);
}

#[test]
fn restore_pointer() {
    let mut t = Table::new();
    let before = t.allocations();
    let first = t.caller("first");
    let second = t.caller("a longer replacement translation");
    let empty = t.caller("");
    let fallback = t.caller("default");
    for value in [first, second, empty] {
        store(&mut t, OBS_KEY_A, Some(value));
        let stored = t.slot(OBS_KEY_A).expect("stored");
        assert_ne!(stored, value);
        assert_eq!(t.text(stored), t.text(value));
        // Compare with the CURRENT slot, as the C test does.
        assert_eq!(translation(&mut t, OBS_KEY_A, Some(fallback)), Some(stored));
        assert_eq!(t.allocations(), before + 1);
    }
    store(&mut t, OBS_KEY_A, None);
    assert_eq!(t.slot(OBS_KEY_A), None);
    assert_eq!(
        translation(&mut t, OBS_KEY_A, Some(fallback)),
        Some(fallback)
    );
    assert_eq!(translation(&mut t, OBS_KEY_A, None), None);
    assert_eq!(t.allocations(), before);
    t.teardown(before);
}

/// The original's access order: the store frees the old slot (NULL too),
/// clears it, then writes the copy; the query reads the slot again to
/// return it. No other key is touched.
#[test]
fn access_order_matches_the_original() {
    let mut t = Table::new();
    let name = t.caller("name");
    t.reads = 0;
    store(&mut t, OBS_KEY_A, Some(name));
    assert_eq!((t.reads, t.releases), (3, 1));
    t.reads = 0;
    store(&mut t, OBS_KEY_A, None);
    assert_eq!((t.reads, t.releases), (2, 2));
    t.reads = 0;
    assert_eq!(translation(&mut t, OBS_KEY_A, Some(name)), Some(name));
    assert_eq!(t.reads, 1);
    store(&mut t, OBS_KEY_A, Some(name));
    t.reads = 0;
    let stored = t.slot(OBS_KEY_A);
    assert_eq!(translation(&mut t, OBS_KEY_A, None), stored);
    assert_eq!(t.reads, 2);
    for key in 0..KEYS {
        if key != usize::try_from(OBS_KEY_A).unwrap() {
            assert_eq!(t.slots[key], None, "key {key}");
        }
    }
    t.teardown(0);
}
