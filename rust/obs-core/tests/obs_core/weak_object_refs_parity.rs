//! Original-C parity for bounded, defined reference-count operations.
//! Signed overflow at LONG_MAX is outside the C-defined domain.
use core::ffi::{c_long, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_core::ffi::weak_object_refs::*;
use obs_util::platform::CLongAtomic;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_weak_object_refs_new(refs: c_long, weak: c_long) -> *mut c_void;
    fn oracle_weak_object_refs_weak(handle: *mut c_void) -> *mut c_void;
    fn oracle_weak_object_refs_object(handle: *mut c_void) -> *mut c_void;
    fn oracle_weak_object_refs_counts(handle: *mut c_void, counts: *mut c_long);
    fn oracle_weak_object_refs_free(handle: *mut c_void, released: bool);
    fn oracle_weak_object_refs_layout(layout: *mut usize);
    fn oracle_weak_object_refs_addresses(handle: *mut c_void) -> bool;
    fn oracle_obs_weak_object_addref(weak: *mut c_void);
    fn oracle_obs_weak_object_release(weak: *mut c_void);
    fn oracle_obs_object_get_weak_object(object: *mut c_void) -> *mut c_void;
    fn oracle_obs_weak_object_get_object(weak: *mut c_void) -> *mut c_void;
}

struct Fixture {
    handle: *mut c_void,
    released: bool,
}
impl Fixture {
    fn new(refs: c_long, weak: c_long) -> Self {
        // SAFETY: bounded counts initialize an owned real-header fixture.
        let handle = unsafe { oracle_weak_object_refs_new(refs, weak) };
        assert!(!handle.is_null());
        Self {
            handle,
            released: false,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns the fixture, with explicit tracking of final weak release.
        unsafe { oracle_weak_object_refs_free(self.handle, self.released) };
    }
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 7];
    // SAFETY: oracle writes exactly seven entries.
    unsafe { oracle_weak_object_refs_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, refs),
            offset_of!(Fields, weak_refs),
            offset_of!(Fields, object),
            size_of::<CLongAtomic>(),
            align_of::<CLongAtomic>()
        ]
    );
}

fn check(refs: c_long, weak_count: c_long, ops: &[u8]) {
    let mut rust = Fixture::new(refs, weak_count);
    let mut c = Fixture::new(refs, weak_count);
    // SAFETY: both fixtures stay alive; operations stop before any freed handle is reused.
    unsafe {
        assert!(oracle_weak_object_refs_addresses(rust.handle));
        let rw = oracle_weak_object_refs_weak(rust.handle);
        let cw = oracle_weak_object_refs_weak(c.handle);
        let ro = oracle_weak_object_refs_object(rust.handle);
        let co = oracle_weak_object_refs_object(c.handle);
        for op in ops {
            let mut before = [0; 2];
            oracle_weak_object_refs_counts(rust.handle, before.as_mut_ptr());
            match op % 4 {
                0 => {
                    obs_weak_object_addref(rw);
                    oracle_obs_weak_object_addref(cw);
                }
                1 => {
                    assert_eq!(obs_object_get_weak_object(ro), rw);
                    assert_eq!(oracle_obs_object_get_weak_object(co), cw);
                }
                2 => {
                    let rr = obs_weak_object_get_object(rw);
                    let cr = oracle_obs_weak_object_get_object(cw);
                    assert_eq!(rr.is_null(), cr.is_null());
                    assert_eq!(
                        rr,
                        if before[0] >= 0 {
                            ro
                        } else {
                            core::ptr::null_mut()
                        }
                    );
                    assert_eq!(
                        cr,
                        if before[0] >= 0 {
                            co
                        } else {
                            core::ptr::null_mut()
                        }
                    );
                }
                _ => {
                    rust.released = before[1] == 0;
                    c.released = rust.released;
                    obs_weak_object_release(rw);
                    oracle_obs_weak_object_release(cw);
                    if rust.released {
                        break;
                    }
                }
            }
            let mut rc = [0; 2];
            let mut cc = [0; 2];
            oracle_weak_object_refs_counts(rust.handle, rc.as_mut_ptr());
            oracle_weak_object_refs_counts(c.handle, cc.as_mut_ptr());
            assert_eq!(rc, cc);
            assert!(oracle_weak_object_refs_addresses(rust.handle));
        }
    }
}

#[test]
fn characterized_edges() {
    // SAFETY: NULL is explicitly supported by all four original functions.
    unsafe {
        obs_weak_object_addref(core::ptr::null_mut());
        oracle_obs_weak_object_addref(core::ptr::null_mut());
        obs_weak_object_release(core::ptr::null_mut());
        oracle_obs_weak_object_release(core::ptr::null_mut());
        assert!(obs_object_get_weak_object(core::ptr::null_mut()).is_null());
        assert!(oracle_obs_object_get_weak_object(core::ptr::null_mut()).is_null());
        assert!(obs_weak_object_get_object(core::ptr::null_mut()).is_null());
        assert!(oracle_obs_weak_object_get_object(core::ptr::null_mut()).is_null());
    }
    for refs in [-2, -1, 0, 1, 4, 7] {
        for weak in [0, 1, 3] {
            check(refs, weak, &[0, 1, 2, 3, 3, 3, 3]);
            check(refs, weak, &[3, 3, 3, 3]);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn operation_sequences_match_c(refs in -2i32..32, weak in 0i32..16,
        ops in prop::collection::vec(0u8..4, 0..32)) {
        check(refs.into(), weak.into(), &ops);
    }
}
