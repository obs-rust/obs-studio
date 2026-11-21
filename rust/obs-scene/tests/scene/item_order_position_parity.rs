//! Tier 3: the original extracted C wrapper versus the shim, on one fixture,
//! through the same real-named full lock and the real parent, head and link
//! adapters. Compares the returned position, the lock trace and every scene
//! and item byte (links and reference counts included). The lock stand-in
//! clears the head or the first link until the unlock, so a head read under
//! the lock or a walk outside it gives another position. No intentional
//! differences on attached items. NULL and detached items are undefined on the
//! C reference (its legacy predicate always accepts, #2899), so they are
//! excluded from differential execution and checked against the shim alone.
//! Mutants show the probe catches each broken contract.
use core::ffi::{c_int, c_void};
use obs_scene::ffi::item_order_position::obs_sceneitem_get_order_position;
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*mut c_void) -> c_int;
unsafe extern "C" {
    fn oracle_scene_item_order_position_probe(bits: u32, getter: Getter) -> u32;
    fn oracle_scene_item_order_position_expected(bits: u32) -> c_int;
    fn oracle_scene_item_order_position_hybrid(which: u32, bits: u32, getter: Getter) -> u32;
    fn oracle_scene_item_order_position_int_size() -> usize;
    fn obs_scene_atomic_full_lock(scene: *mut c_void);
    fn obs_scene_atomic_full_unlock(scene: *mut c_void);
    fn obs_sceneitem_parent_fields(item: *const c_void) -> *const *mut c_void;
    fn obs_scene_item_order_position_first(scene: *const c_void) -> *mut c_void;
    fn obs_scene_lookup_next(item: *mut c_void) -> *mut c_void;
}

/// Bits of the oracle fixture: linked count, target, clobber.
const fn bits(count: u32, target: u32, clobber: u32) -> u32 {
    count | (target << 3) | (clobber << 7)
}
const HEAD: u32 = 1;
const LINK: u32 = 2;

fn probe(bits: u32, getter: Getter) -> u32 {
    // SAFETY: the oracle owns a complete fixture and passes only its own
    // attached item to the getter.
    unsafe { oracle_scene_item_order_position_probe(bits, getter) }
}

fn check(bits: u32) {
    assert_eq!(
        probe(bits, obs_sceneitem_get_order_position),
        0,
        "bits={bits:#x}"
    );
}

#[test]
fn position_is_c_int() {
    // SAFETY: a pure header measurement.
    assert_eq!(
        unsafe { oracle_scene_item_order_position_int_size() },
        size_of::<c_int>()
    );
}

#[test]
fn characterized_edges() {
    // Every count, target (absent included), order, clobber and field noise.
    for bits in 0..0x400 {
        check(bits);
    }
    // The original's own values, as test_sceneitem_order_position.c pins them.
    // SAFETY: the oracle runs the original on its own attached fixture.
    let expected = |bits| unsafe { oracle_scene_item_order_position_expected(bits) };
    for count in 0..=6 {
        for target in 0..8 {
            let position = if target < count { target } else { count };
            assert_eq!(expected(bits(count, target, 0)) as u32, position);
            // Reversed links: position counts from the other end.
            let reversed = if target < count {
                count - 1 - target
            } else {
                count
            };
            assert_eq!(expected(bits(count, target, 0) | 0x40) as u32, reversed);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn order_position_matches_c(bits in 0..0x400u32) {
        check(bits);
    }
}

#[test]
fn hybrid_null_and_detached_return_zero_without_lock() {
    for fixture in [bits(0, 0, 0), bits(6, 2, 0), bits(6, 2, HEAD | LINK)] {
        for which in [0, 1] {
            // SAFETY: the oracle passes NULL or its own detached item.
            let result =
                unsafe { oracle_scene_item_order_position_hybrid(which, fixture, obs_sceneitem_get_order_position) };
            assert_eq!(result, 0, "which={which} bits={fixture:#x}");
        }
    }
}

/// The real adapter reads a mutant uses on the oracle's attached item.
unsafe fn parent(item: *mut c_void) -> *mut c_void {
    // SAFETY: a non-null fixture item.
    unsafe { *obs_sceneitem_parent_fields(item) }
}
unsafe fn head(scene: *mut c_void) -> *mut c_void {
    // SAFETY: the fixture scene.
    unsafe { obs_scene_item_order_position_first(scene) }
}
unsafe fn walk(mut next: *mut c_void, item: *mut c_void) -> c_int {
    let mut index = 0;
    while !next.is_null() && next != item {
        // SAFETY: a fixture item linked into the fixture scene.
        next = unsafe { obs_scene_lookup_next(next) };
        index += 1;
    }
    index
}

unsafe extern "C" fn no_unlock(item: *mut c_void) -> c_int {
    // SAFETY: fixture item and scene; the unlock is deliberately missing.
    unsafe {
        let scene = parent(item);
        let first = head(scene);
        obs_scene_atomic_full_lock(scene);
        walk(first, item)
    }
}

unsafe extern "C" fn walk_after_unlock(item: *mut c_void) -> c_int {
    // SAFETY: fixture item and scene; the walk deliberately follows the unlock.
    unsafe {
        let scene = parent(item);
        let first = head(scene);
        obs_scene_atomic_full_lock(scene);
        obs_scene_atomic_full_unlock(scene);
        walk(first, item)
    }
}

unsafe extern "C" fn walk_before_lock(item: *mut c_void) -> c_int {
    // SAFETY: fixture item and scene; the walk deliberately precedes the lock.
    unsafe {
        let scene = parent(item);
        let index = walk(head(scene), item);
        obs_scene_atomic_full_lock(scene);
        obs_scene_atomic_full_unlock(scene);
        index
    }
}

unsafe extern "C" fn head_in_lock(item: *mut c_void) -> c_int {
    // SAFETY: fixture item and scene; the head is deliberately read locked.
    unsafe {
        let scene = parent(item);
        obs_scene_atomic_full_lock(scene);
        let index = walk(head(scene), item);
        obs_scene_atomic_full_unlock(scene);
        index
    }
}

unsafe extern "C" fn counts_target(item: *mut c_void) -> c_int {
    // SAFETY: as the shim, then one too many.
    unsafe { obs_sceneitem_get_order_position(item) + 1 }
}

unsafe extern "C" fn stops_one_early(item: *mut c_void) -> c_int {
    // SAFETY: as the shim, one too few for a counted item.
    let index = unsafe { obs_sceneitem_get_order_position(item) };
    (index - 1).max(0)
}

#[test]
fn mutants_are_caught() {
    // The fourth of six linked items, at position 3.
    let plain = bits(6, 3, 0);
    // The lock clears the head: a locked head read sees an empty list.
    let head_cleared = bits(6, 3, HEAD);
    // The lock clears the first link: an unlocked walk sees the whole list.
    let link_cleared = bits(6, 3, LINK);
    // The absent item counts the whole list.
    let absent = bits(4, 7, 0);
    assert_eq!(probe(plain, no_unlock) & 2, 2);
    assert_eq!(probe(link_cleared, walk_after_unlock) & 1, 1);
    assert_eq!(probe(link_cleared, walk_before_lock) & 1, 1);
    assert_eq!(probe(head_cleared, head_in_lock) & 1, 1);
    assert_eq!(probe(plain, counts_target) & 1, 1);
    assert_eq!(probe(absent, counts_target) & 1, 1);
    assert_eq!(probe(plain, stops_one_early) & 1, 1);
    assert_eq!(probe(absent, stops_one_early) & 1, 1);
    for bits in [plain, head_cleared, link_cleared, absent] {
        check(bits);
    }
}
