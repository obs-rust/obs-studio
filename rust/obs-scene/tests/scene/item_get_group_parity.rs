//! Tier 3: the original extracted C wrapper versus the shim, on one fixture,
//! through the same real-named lock, walk and group-flag boundary. Compares
//! the returned pointer, the lock/walk trace and every scene and item byte
//! (reference counts included). No intentional differences. Mutants show the
//! probe catches each broken contract.
use core::ffi::c_void;
use obs_scene::ffi::item_get_group::obs_sceneitem_get_group;
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void;
unsafe extern "C" {
    fn oracle_scene_item_get_group_probe(bits: u32, getter: Getter) -> u32;
    fn oracle_scene_item_get_group_layout() -> usize;
    fn obs_scene_atomic_full_lock(scene: *mut c_void);
    fn obs_scene_atomic_full_unlock(scene: *mut c_void);
    fn obs_sceneitem_parent_group_lookup(scene: *mut c_void, item: *mut c_void) -> *mut c_void;
    fn obs_sceneitem_group_scene_flag(item: *const c_void) -> *const bool;
}

fn probe(bits: u32, getter: Getter) -> u32 {
    // SAFETY: the oracle owns a complete fixture and passes only its own
    // scene and item, or NULL, to the getter.
    unsafe { oracle_scene_item_get_group_probe(bits, getter) }
}

fn check(bits: u32) {
    assert_eq!(probe(bits, obs_sceneitem_get_group), 0, "bits={bits:#x}");
}

#[test]
fn group_flag_is_one_byte_bool() {
    // SAFETY: a pure header measurement.
    assert_eq!(
        unsafe { oracle_scene_item_get_group_layout() },
        size_of::<bool>()
    );
}

#[test]
fn characterized_edges() {
    // Every rejection, result choice, removed and clobber flag, with and
    // without an item parent, over a few group-flag patterns.
    for low in 0..0x200 {
        for groups in [0, 0b0110, 0b1111] {
            check(low | (groups << 9));
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn get_group_matches_c(bits in 0..0x2000u32) {
        check(bits);
    }
}

unsafe fn is_group(item: *mut c_void) -> bool {
    // SAFETY: mutants read the flag only for a non-null fixture item.
    unsafe { *obs_sceneitem_group_scene_flag(item) }
}

unsafe extern "C" fn no_unlock(scene: *mut c_void, item: *mut c_void) -> *mut c_void {
    if scene.is_null() || item.is_null() || unsafe { is_group(item) } {
        return core::ptr::null_mut();
    }
    // SAFETY: fixture scene and item; the unlock is deliberately missing.
    unsafe {
        obs_scene_atomic_full_lock(scene);
        obs_sceneitem_parent_group_lookup(scene, item)
    }
}

unsafe extern "C" fn unlocked_walk(scene: *mut c_void, item: *mut c_void) -> *mut c_void {
    if scene.is_null() || item.is_null() || unsafe { is_group(item) } {
        return core::ptr::null_mut();
    }
    // SAFETY: fixture scene and item; the walk deliberately runs unlocked.
    unsafe { obs_sceneitem_parent_group_lookup(scene, item) }
}

unsafe extern "C" fn locks_group_items(scene: *mut c_void, item: *mut c_void) -> *mut c_void {
    if scene.is_null() || item.is_null() {
        return core::ptr::null_mut();
    }
    // SAFETY: fixture scene and item; the group check deliberately follows the lock.
    unsafe {
        obs_scene_atomic_full_lock(scene);
        let group = obs_sceneitem_parent_group_lookup(scene, item);
        obs_scene_atomic_full_unlock(scene);
        if is_group(item) {
            return core::ptr::null_mut();
        }
        group
    }
}

unsafe extern "C" fn rereads_flag(scene: *mut c_void, item: *mut c_void) -> *mut c_void {
    // SAFETY: as the shim, then a second flag read after the walk.
    let group = unsafe { obs_sceneitem_get_group(scene, item) };
    if group.is_null() || unsafe { is_group(item) } {
        return core::ptr::null_mut();
    }
    group
}

unsafe extern "C" fn always_null(scene: *mut c_void, item: *mut c_void) -> *mut c_void {
    // SAFETY: as the shim; the result is deliberately dropped.
    unsafe { obs_sceneitem_get_group(scene, item) };
    core::ptr::null_mut()
}

#[test]
fn mutants_are_caught() {
    // A present, non-group item whose walk finds the second group.
    let found = 2 << 5;
    // A group item: rejected before any lock.
    let group = found | 4;
    // The walk sets the item's flag, as another thread could.
    let clobbered = found | 0x10;
    assert_eq!(probe(found, no_unlock) & 2, 2);
    assert_eq!(probe(found, unlocked_walk) & 2, 2);
    assert_eq!(probe(group, locks_group_items) & 2, 2);
    assert_eq!(probe(clobbered, rereads_flag) & 1, 1);
    assert_eq!(probe(found, always_null) & 1, 1);
    for bits in [found, group, clobbered] {
        check(bits);
    }
}
