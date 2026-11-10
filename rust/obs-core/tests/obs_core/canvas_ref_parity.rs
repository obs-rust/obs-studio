//! Real-header layout and parity of all five helpers. Strong LONG_MAX upgrades
//! are excluded because owners + 1 overflows signed C long. Stack controls keep
//! a weak owner; final release has its own allocator-backed probe.
use core::ffi::{c_long, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_core::ffi::canvas_ref::{
    WeakCanvas, WeakRef, obs_canvas_get_ref, obs_canvas_get_weak_canvas, obs_weak_canvas_addref,
    obs_weak_canvas_get_canvas, obs_weak_canvas_release,
};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_canvas_ref_layout(layout: *mut usize);
    fn oracle_canvas_ref_probe(
        operation: u32,
        strong: c_long,
        weak_count: c_long,
        null_handle: bool,
        addref: unsafe extern "C" fn(*mut WeakCanvas),
        release: unsafe extern "C" fn(*mut WeakCanvas),
        get_ref: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
        get_weak: unsafe extern "C" fn(*mut c_void) -> *mut WeakCanvas,
        upgrade: unsafe extern "C" fn(*mut WeakCanvas) -> *mut c_void,
    ) -> bool;
    fn oracle_canvas_ref_final_release(release: unsafe extern "C" fn(*mut WeakCanvas));
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 8];
    // SAFETY: the real C header probe writes eight size/alignment/offset values.
    unsafe { oracle_canvas_ref_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<WeakRef>(),
            align_of::<WeakRef>(),
            offset_of!(WeakRef, refs),
            offset_of!(WeakRef, weak_refs),
            size_of::<WeakCanvas>(),
            align_of::<WeakCanvas>(),
            offset_of!(WeakCanvas, counts),
            offset_of!(WeakCanvas, canvas),
        ]
    );
}

fn check(operation: u32, strong: c_long, weak_count: c_long, null_handle: bool) {
    // SAFETY: C owns the real stack objects, retains their controls, and only
    // passes valid handles to callbacks. Counts avoid signed overflow/freeing stack.
    assert!(unsafe {
        oracle_canvas_ref_probe(
            operation,
            strong,
            weak_count,
            null_handle,
            obs_weak_canvas_addref,
            obs_weak_canvas_release,
            obs_canvas_get_ref,
            obs_canvas_get_weak_canvas,
            obs_weak_canvas_get_canvas,
        )
    });
}

#[test]
fn characterized_edges_and_final_free() {
    for operation in 0..5 {
        for strong in [c_long::MIN, -2, -1, 0, 1, c_long::MAX - 1] {
            for weak_count in [1, 2, c_long::MAX - 1] {
                for null_handle in [false, true] {
                    check(operation, strong, weak_count, null_handle);
                }
            }
        }
    }
    // SAFETY: the C fixture transfers two bzalloc allocations to their final releases.
    unsafe { oracle_canvas_ref_final_release(obs_weak_canvas_release) };
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn helpers_match_c(operation in 0u32..5, strong in -2 as c_long..1000, weak in 1 as c_long..1000, null in any::<bool>()) {
        check(operation, strong, weak, null);
    }
}
