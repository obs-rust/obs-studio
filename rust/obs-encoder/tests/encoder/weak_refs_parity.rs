//! Tier 3 over the original C and real private encoder/control layouts.
//! Dangling input handles and signed native-long overflow are outside C's domain.
use core::ffi::{c_long, c_void};
use core::mem::{align_of, size_of};
use obs_encoder::ffi::weak_refs::*;
use obs_util::ffi::bmem::bnum_allocs;
use obs_util::platform::CLongAtomic;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_encoder_weak_new(
        refs: c_long,
        weak: c_long,
        control: bool,
        owner: bool,
        noise: u8,
    ) -> *mut c_void;
    fn oracle_encoder_weak_handle(fixture: *mut c_void) -> *mut c_void;
    fn oracle_encoder_weak_encoder(fixture: *mut c_void) -> *mut c_void;
    fn oracle_encoder_weak_count(fixture: *mut c_void) -> c_long;
    fn oracle_encoder_weak_unchanged(fixture: *mut c_void) -> bool;
    fn oracle_encoder_weak_free(fixture: *mut c_void, released: bool);
    fn oracle_encoder_weak_layout(values: *mut usize);
    fn oracle_obs_weak_encoder_addref(weak: *mut c_void);
    fn oracle_obs_weak_encoder_release(weak: *mut c_void);
    fn oracle_obs_encoder_get_weak_encoder(encoder: *mut c_void) -> *mut c_void;
}

struct Fixture {
    handle: *mut c_void,
    released: bool,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns this fixture; tracks whether its weak allocation was released.
        unsafe { oracle_encoder_weak_free(self.handle, self.released) };
    }
}

fn check(refs: c_long, weak: c_long, control: bool, owner: bool, noise: u8, ops: &[u8]) {
    // SAFETY: bounded counts and live, independently owned real-header fixtures.
    unsafe {
        let mut rust = Fixture {
            handle: oracle_encoder_weak_new(refs, weak, control, owner, noise),
            released: false,
        };
        let mut c = Fixture {
            handle: oracle_encoder_weak_new(refs, weak, control, owner, noise),
            released: false,
        };
        assert!(!rust.handle.is_null() && !c.handle.is_null());
        let rw = oracle_encoder_weak_handle(rust.handle);
        let cw = oracle_encoder_weak_handle(c.handle);
        let re = oracle_encoder_weak_encoder(rust.handle);
        let ce = oracle_encoder_weak_encoder(c.handle);
        for op in ops {
            let before = oracle_encoder_weak_count(rust.handle);
            match op % 5 {
                0 => {
                    obs_weak_encoder_addref(rw);
                    oracle_obs_weak_encoder_addref(cw);
                }
                1 => {
                    assert_eq!(
                        obs_encoder_get_weak_encoder(re),
                        if control { rw } else { core::ptr::null_mut() }
                    );
                    assert_eq!(
                        oracle_obs_encoder_get_weak_encoder(ce),
                        if control { cw } else { core::ptr::null_mut() }
                    );
                }
                2 => {
                    let allocations = bnum_allocs();
                    obs_weak_encoder_release(rw);
                    oracle_obs_weak_encoder_release(cw);
                    if before == 0 {
                        rust.released = true;
                        c.released = true;
                        assert_eq!(bnum_allocs(), allocations - 2);
                        break;
                    }
                    assert_eq!(bnum_allocs(), allocations);
                }
                3 => {
                    obs_weak_encoder_addref(core::ptr::null_mut());
                    oracle_obs_weak_encoder_addref(core::ptr::null_mut());
                    obs_weak_encoder_release(core::ptr::null_mut());
                    oracle_obs_weak_encoder_release(core::ptr::null_mut());
                }
                _ => {
                    assert!(obs_encoder_get_weak_encoder(core::ptr::null_mut()).is_null());
                    assert!(oracle_obs_encoder_get_weak_encoder(core::ptr::null_mut()).is_null());
                }
            }
            assert_eq!(
                oracle_encoder_weak_count(rust.handle),
                oracle_encoder_weak_count(c.handle)
            );
            assert!(oracle_encoder_weak_unchanged(rust.handle));
            assert!(oracle_encoder_weak_unchanged(c.handle));
        }
    }
}

#[test]
fn native_long_layout() {
    let mut layout = [0; 2];
    // SAFETY: writes exactly two entries from the real header's native long.
    unsafe { oracle_encoder_weak_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [size_of::<CLongAtomic>(), align_of::<CLongAtomic>()]
    );
}

#[test]
fn characterized_edges() {
    if !super::isolated(concat!(module_path!(), "::characterized_edges")) {
        return;
    }
    for refs in [c_long::MIN, -2, -1, 0, 1, 7, c_long::MAX] {
        for weak in [c_long::MIN + 128, -2, 0, 1, 3, c_long::MAX - 128] {
            for control in [false, true] {
                for owner in [false, true] {
                    check(
                        refs,
                        weak,
                        control,
                        owner,
                        0xa5,
                        &[3, 4, 0, 1, 1, 1, 2, 2, 2, 2, 2],
                    );
                    check(refs, weak, control, owner, 0x5a, &[2, 2, 2, 2]);
                }
            }
        }
    }
}

#[test]
fn generated_sequences() {
    if !super::isolated(concat!(module_path!(), "::generated_sequences")) {
        return;
    }
    let config = proptest::test_runner::Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Default::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(config);
    runner
        .run(
            &(
                -2 as c_long..32,
                -2 as c_long..32,
                any::<bool>(),
                any::<bool>(),
                any::<u8>(),
                prop::collection::vec(0u8..5, 0..64),
            ),
            |(refs, weak, control, owner, noise, ops)| {
                check(refs, weak, control, owner, noise, &ops);
                Ok(())
            },
        )
        .unwrap();
}
