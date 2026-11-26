//! Tier 3: the original extracted `obs_sceneitem_set_order()` versus the
//! shim, on one real-header fixture (oracle/scene_item_set_order.c). Both
//! end in the same recorded `detach_sceneitem()`, `attach_sceneitem()`,
//! `signal_reorder()`, full lock and scene reference: the original
//! directly, the shim through the real adapters and the Rust
//! `obs_scene_get_ref()`. Compared: the trace (each call with its operands
//! and the lock depth), every scene, source and item byte (links, parents
//! and noise), the reference count and the lock depth. Every movement of
//! the enum and outside it, on every item of lists of 0 to 6 items in both
//! link orders, NULL, detached and dying-scene items. No intentional
//! differences: the original runs on the anchor hybrid libobs ships
//! (#2895). Mutants show the probe catches each broken contract.
use core::ffi::{c_int, c_void};
use obs_scene::ffi::item_set_order::obs_sceneitem_set_order;
use obs_scene::item_order::{
    OBS_ORDER_MOVE_BOTTOM, OBS_ORDER_MOVE_DOWN, OBS_ORDER_MOVE_TOP, OBS_ORDER_MOVE_UP,
};
use obs_scene::item_set_order::{SetOrderScene, set_order};
use proptest::prelude::*;

type SetOrder = unsafe extern "C" fn(*mut c_void, c_int);
unsafe extern "C" {
    fn oracle_scene_item_set_order_probe(bits: u32, movement: c_int, rust: SetOrder) -> u32;
    fn oracle_scene_item_set_order_list(
        bits: u32,
        movement: c_int,
        order: *mut u8,
        signals: *mut u32,
        events: *mut u32,
    ) -> usize;
    fn oracle_scene_item_set_order_movement_size() -> usize;
    fn obs_sceneitem_parent_fields(item: *const c_void) -> *const *mut c_void;
    fn obs_scene_get_ref(scene: *mut c_void) -> *mut c_void;
    fn obs_scene_release(scene: *mut c_void);
    fn obs_scene_atomic_full_lock(scene: *mut c_void);
    fn obs_scene_atomic_full_unlock(scene: *mut c_void);
    fn obs_scene_lookup_next(item: *mut c_void) -> *mut c_void;
    fn obs_sceneitem_set_order_prev(item: *const c_void) -> *mut c_void;
    fn obs_sceneitem_set_order_detach(item: *mut c_void);
    fn obs_sceneitem_set_order_attach(scene: *mut c_void, item: *mut c_void, prev: *mut c_void);
    fn obs_sceneitem_set_order_signal(item: *mut c_void);
}

/// Bits of the oracle fixture: linked count, target (6 and 7 NULL; at or
/// past the count detached), reversed links, dying scene, noise byte.
const fn bits(count: u32, target: u32, reverse: bool, dying: bool, noise: u32) -> u32 {
    count | (target << 3) | ((reverse as u32) << 6) | ((dying as u32) << 7) | (noise << 8)
}

const MOVEMENTS: [c_int; 11] = [
    OBS_ORDER_MOVE_UP,
    OBS_ORDER_MOVE_DOWN,
    OBS_ORDER_MOVE_TOP,
    OBS_ORDER_MOVE_BOTTOM,
    -1,
    4,
    5,
    0x100,
    -2,
    c_int::MIN,
    c_int::MAX,
];

fn probe(bits: u32, movement: c_int, rust: SetOrder) -> u32 {
    // SAFETY: the oracle owns a complete fixture and passes only its own
    // items (or NULL) to the function under test.
    unsafe { oracle_scene_item_set_order_probe(bits, movement, rust) }
}

fn check(bits: u32, movement: c_int) {
    assert_eq!(
        probe(bits, movement, obs_sceneitem_set_order),
        0,
        "bits={bits:#x} movement={movement}"
    );
}

#[test]
fn movement_is_c_int() {
    // SAFETY: a pure header measurement.
    assert_eq!(
        unsafe { oracle_scene_item_set_order_movement_size() },
        size_of::<c_int>()
    );
}

#[test]
fn every_case_matches_c() {
    for count in 0..=6 {
        for target in 0..8 {
            for reverse in [false, true] {
                for dying in [false, true] {
                    for noise in [0, 0xa5] {
                        for movement in MOVEMENTS {
                            check(bits(count, target, reverse, dying, noise), movement);
                        }
                    }
                }
            }
        }
    }
}

/// The original's list from the bottom, as item indexes, its signals and
/// its trace length.
fn original(bits: u32, movement: c_int) -> (Vec<u8>, u32, u32) {
    let mut order = [0u8; 7];
    let (mut signals, mut events) = (0, 0);
    // SAFETY: the oracle writes at most 7 entries and the two counts.
    let length = unsafe {
        oracle_scene_item_set_order_list(
            bits,
            movement,
            order.as_mut_ptr(),
            &mut signals,
            &mut events,
        )
    };
    (order[..length].to_vec(), signals, events)
}

#[test]
fn original_moves_as_characterized() {
    for count in 1..=6u8 {
        for index in 0..count {
            for movement in MOVEMENTS {
                let mut want: Vec<u8> = (0..count).collect();
                let in_enum = (OBS_ORDER_MOVE_UP..=OBS_ORDER_MOVE_BOTTOM).contains(&movement);
                match movement {
                    OBS_ORDER_MOVE_UP if index + 1 < count => {
                        want.swap(usize::from(index), usize::from(index) + 1)
                    }
                    OBS_ORDER_MOVE_DOWN if index > 0 => {
                        want.swap(usize::from(index), usize::from(index) - 1)
                    }
                    OBS_ORDER_MOVE_TOP => {
                        let item = want.remove(usize::from(index));
                        want.push(item);
                    }
                    OBS_ORDER_MOVE_BOTTOM => {
                        let item = want.remove(usize::from(index));
                        want.insert(0, item);
                    }
                    _ => {}
                }
                let fixture = bits(u32::from(count), u32::from(index), false, false, 0x3c);
                let (order, signals, events) = original(fixture, movement);
                let case = format!("count {count} index {index} movement {movement}");
                assert_eq!(order, want, "{case}");
                assert_eq!(signals, u32::from(in_enum), "{case}");
                // ref, lock, [detach, attach,] unlock, [signal,] release
                assert_eq!(events, if in_enum { 7 } else { 4 }, "{case}");
            }
        }
    }
    // NULL and detached items, and a dying scene: no lock, no move.
    for fixture in [
        bits(3, 6, false, false, 0),
        bits(3, 4, false, false, 0),
        bits(3, 1, false, true, 0),
    ] {
        let (order, signals, events) = original(fixture, OBS_ORDER_MOVE_TOP);
        assert_eq!((order, signals), (vec![0, 1, 2], 0));
        assert!(events <= 1, "only the reference attempt: {events}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn set_order_matches_c(bits in 0..0x10000u32, movement in any::<c_int>()) {
        check(bits, movement);
    }
}

/// The ways a mutant breaks the shim's contract.
const SIGNAL_UNDER_LOCK: u8 = 1;
const NO_RELEASE: u8 = 2;
const ALWAYS_BOTTOM: u8 = 3;
const NO_LOCK: u8 = 4;
const PREV_FOR_PREV_PREV: u8 = 5;
const STAY_SIGNALS: u8 = 6;

/// The shim's seams, broken one way.
struct Mutant<const M: u8> {
    item: *mut c_void,
    stayed: bool,
}

impl<const M: u8> SetOrderScene for Mutant<M> {
    type Item = *mut c_void;
    type Scene = *mut c_void;
    fn parent_ref(&mut self, item: *mut c_void) -> Option<*mut c_void> {
        // SAFETY: a fixture item; its parent is the fixture scene or NULL.
        let scene = unsafe { obs_scene_get_ref(*obs_sceneitem_parent_fields(item)) };
        (!scene.is_null()).then_some(scene)
    }
    fn lock(&mut self, scene: *mut c_void) {
        self.stayed = true;
        if M != NO_LOCK {
            // SAFETY: the fixture scene, referenced.
            unsafe { obs_scene_atomic_full_lock(scene) };
        }
    }
    fn next(&mut self, item: *mut c_void) -> Option<*mut c_void> {
        // SAFETY: a fixture item.
        let next = unsafe { obs_scene_lookup_next(item) };
        (!next.is_null()).then_some(next)
    }
    fn prev(&mut self, item: *mut c_void) -> Option<*mut c_void> {
        // SAFETY: a fixture item.
        let prev = unsafe { obs_sceneitem_set_order_prev(item) };
        if M == PREV_FOR_PREV_PREV && item != self.item {
            // Takes prev for prev->prev: down puts the item back in place.
            return Some(item);
        }
        (!prev.is_null()).then_some(prev)
    }
    fn detach(&mut self, item: *mut c_void) {
        self.stayed = false;
        // SAFETY: a linked fixture item.
        unsafe { obs_sceneitem_set_order_detach(item) };
    }
    fn attach(&mut self, scene: *mut c_void, item: *mut c_void, after: Option<*mut c_void>) {
        let after = if M == ALWAYS_BOTTOM {
            core::ptr::null_mut()
        } else {
            after.unwrap_or(core::ptr::null_mut())
        };
        // SAFETY: the fixture scene and its just detached item.
        unsafe { obs_sceneitem_set_order_attach(scene, item, after) };
    }
    fn unlock(&mut self, scene: *mut c_void) {
        if M == SIGNAL_UNDER_LOCK && !self.stayed {
            // SAFETY: the attached fixture item.
            unsafe { obs_sceneitem_set_order_signal(self.item) };
        }
        if M != NO_LOCK {
            // SAFETY: paired with the lock.
            unsafe { obs_scene_atomic_full_unlock(scene) };
        }
        if M == STAY_SIGNALS && self.stayed {
            // SAFETY: the fixture item, still attached.
            unsafe { obs_sceneitem_set_order_signal(self.item) };
        }
    }
    fn signal_reorder(&mut self, item: *mut c_void) {
        if M != SIGNAL_UNDER_LOCK {
            // SAFETY: the attached fixture item.
            unsafe { obs_sceneitem_set_order_signal(item) };
        }
    }
    fn release(&mut self, scene: *mut c_void) {
        if M != NO_RELEASE {
            // SAFETY: the reference parent_ref took.
            unsafe { obs_scene_release(scene) };
        }
    }
}

unsafe extern "C" fn mutant<const M: u8>(item: *mut c_void, movement: c_int) {
    let mut seams = Mutant::<M> {
        item,
        stayed: false,
    };
    set_order(&mut seams, (!item.is_null()).then_some(item), movement);
}

#[test]
fn mutants_are_caught() {
    let middle = bits(5, 2, false, false, 0);
    assert_eq!(
        probe(middle, OBS_ORDER_MOVE_UP, mutant::<SIGNAL_UNDER_LOCK>) & 1,
        1
    );
    assert_eq!(
        probe(middle, OBS_ORDER_MOVE_UP, mutant::<NO_RELEASE>) & 5,
        5
    );
    assert_eq!(
        probe(middle, OBS_ORDER_MOVE_TOP, mutant::<ALWAYS_BOTTOM>) & 3,
        3
    );
    assert_eq!(probe(middle, OBS_ORDER_MOVE_DOWN, mutant::<NO_LOCK>) & 1, 1);
    assert_eq!(
        probe(middle, OBS_ORDER_MOVE_DOWN, mutant::<PREV_FOR_PREV_PREV>) & 1,
        1
    );
    assert_eq!(probe(middle, 7, mutant::<STAY_SIGNALS>) & 1, 1);
    // Each mutant still agrees where its break does not show.
    assert_eq!(
        probe(
            bits(5, 6, false, false, 0),
            OBS_ORDER_MOVE_UP,
            mutant::<NO_LOCK>
        ),
        0
    );
    assert_eq!(
        probe(middle, OBS_ORDER_MOVE_BOTTOM, mutant::<ALWAYS_BOTTOM>),
        0
    );
    for movement in MOVEMENTS {
        check(middle, movement);
    }
}
