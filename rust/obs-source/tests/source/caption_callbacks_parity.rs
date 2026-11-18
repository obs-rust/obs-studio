//! Tier 3: original C over real headers; no intentional differences/exclusions.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_source::ffi::source_caption_callbacks::{
    Callback, Pair, obs_source_add_caption_callback as add,
    obs_source_remove_caption_callback as remove,
};
use obs_util::ffi::{
    base::{base_get_log_handler, base_set_log_handler},
    darray::darray,
};
use proptest::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
type Operation = unsafe extern "C" fn(*mut c_void, Callback, *mut c_void);
unsafe extern "C" {
    fn oracle_caption_new() -> *mut c_void;
    fn oracle_caption_free(source: *mut c_void);
    fn oracle_caption_count(source: *mut c_void) -> usize;
    fn oracle_caption_pair(source: *mut c_void, index: usize) -> Pair;
    fn oracle_caption_unlocked(source: *mut c_void) -> bool;
    fn oracle_caption_layout(layout: *mut usize);
    fn oracle_obs_source_add_caption_callback(
        source: *mut c_void,
        callback: Callback,
        param: *mut c_void,
    );
    fn oracle_obs_source_remove_caption_callback(
        source: *mut c_void,
        callback: Callback,
        param: *mut c_void,
    );
    fn oracle_caption_logs(
        add: Operation,
        remove: Operation,
        get: unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void),
        set: unsafe extern "C" fn(*mut c_void, *mut c_void),
    ) -> bool;
}
struct Fixture(*mut c_void);
impl Fixture {
    fn new() -> Self {
        // SAFETY: C allocates and initializes a real source caption fixture.
        let fixture = Self(unsafe { oracle_caption_new() });
        assert!(!fixture.0.is_null());
        fixture
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns the source, list and initialized mutex.
        unsafe { oracle_caption_free(self.0) };
    }
}
static INVOKED: AtomicUsize = AtomicUsize::new(0);
unsafe extern "C" fn callback_a(_: *mut c_void, _: *mut c_void, _: *const c_void) {
    INVOKED.fetch_add(1, Ordering::Relaxed);
}
unsafe extern "C" fn callback_b(_: *mut c_void, _: *mut c_void, _: *const c_void) {
    INVOKED.fetch_add(2, Ordering::Relaxed);
}
fn check(ops: &[(bool, u8, u8)]) {
    let r = Fixture::new();
    let c = Fixture::new();
    let mut tokens = [0u8; 3];
    for &(insert, cb, param) in ops {
        let callback: Callback = match cb % 3 {
            0 => None,
            1 => Some(callback_a),
            _ => Some(callback_b),
        };
        let param = if param % 4 == 0 {
            core::ptr::null_mut()
        } else {
            (&mut tokens[usize::from(param % 4 - 1)] as *mut u8).cast()
        };
        // SAFETY: both real source fixtures and token addresses remain live;
        // each operation retains the mutex and C allocator contracts.
        unsafe {
            if insert {
                add(r.0, callback, param);
                oracle_obs_source_add_caption_callback(c.0, callback, param);
            } else {
                remove(r.0, callback, param);
                oracle_obs_source_remove_caption_callback(c.0, callback, param);
            }
            assert!(oracle_caption_unlocked(r.0));
            assert!(oracle_caption_unlocked(c.0));
            let count = oracle_caption_count(r.0);
            assert_eq!(count, oracle_caption_count(c.0));
            for i in 0..count {
                let rp = oracle_caption_pair(r.0, i);
                let cp = oracle_caption_pair(c.0, i);
                assert!(rp == cp, "pair {i} differs");
            }
        }
        assert_eq!(INVOKED.load(Ordering::Relaxed), 0);
    }
}
#[test]
fn real_header_layout() {
    let mut layout = [0; 9];
    // SAFETY: the C oracle writes exactly nine measured layout values.
    unsafe { oracle_caption_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Pair>(),
            align_of::<Pair>(),
            offset_of!(Pair, callback),
            offset_of!(Pair, param),
            size_of::<darray>(),
            align_of::<darray>(),
            offset_of!(darray, array),
            offset_of!(darray, num),
            offset_of!(darray, capacity)
        ]
    );
}
#[test]
fn characterized_edges() {
    check(&[
        (false, 1, 1),
        (true, 1, 1),
        (true, 2, 1),
        (true, 1, 2),
        (true, 1, 1),
        (false, 1, 1),
        (false, 2, 2),
        (false, 1, 2),
        (false, 2, 1),
        (false, 1, 1),
        (true, 0, 0),
        (false, 0, 0),
    ]);
    // Cover growth, every null/non-null pair and repeated first-match erasure.
    let mut ops = Vec::new();
    for insert in [true, true, false, false, false] {
        for cb in 0..3 {
            for param in 0..4 {
                ops.push((insert, cb, param));
            }
        }
    }
    check(&ops);
}
#[test]
fn null_diagnostics_match_c() {
    if !crate::output::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: isolated process owns the log handler; C restores it after probing.
    unsafe {
        assert!(oracle_caption_logs(
            add,
            remove,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}
proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn sequences(ops in prop::collection::vec((any::<bool>(), 0u8..3, 0u8..4), 0..128)) { check(&ops); }
}
