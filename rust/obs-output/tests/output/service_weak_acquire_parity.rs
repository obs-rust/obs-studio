//! Exact parity; no intended behavior differences. C's owners + 1 is
//! undefined at LONG_MAX, excluded only for acquisition (target still checked).
//! NULL controls are outside the hidden C functions' contract.

use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::service_weak_acquire::{self as rust, WeakRef, WeakService};
use obs_util::platform::{CLong, CLongAtomic};
use proptest::prelude::*;
use std::sync::atomic::Ordering::SeqCst;

unsafe extern "C" {
    fn oracle_obs_weak_service_weak_get_ref(weak: *mut WeakService) -> bool;
    fn oracle_obs_weak_service_target(weak: *const WeakService) -> *mut c_void;
    fn oracle_service_weak_acquire_layout(layout: *mut usize);
}

#[test]
fn real_c_header_layout() {
    let mut c = [0; 8];
    // SAFETY: the oracle writes precisely eight size_t values.
    unsafe { oracle_service_weak_acquire_layout(c.as_mut_ptr()) };
    assert_eq!(
        c,
        [
            size_of::<WeakRef>(),
            align_of::<WeakRef>(),
            offset_of!(WeakRef, refs),
            offset_of!(WeakRef, weak_refs),
            size_of::<WeakService>(),
            align_of::<WeakService>(),
            offset_of!(WeakService, counts),
            offset_of!(WeakService, service),
        ]
    );
}

fn check(owners: CLong, weak: CLong, null_target: bool) {
    let mut marker = 0u8;
    let service = if null_target {
        core::ptr::null_mut()
    } else {
        core::ptr::from_mut(&mut marker).cast()
    };
    let control = || WeakService {
        counts: WeakRef {
            refs: CLongAtomic::new(owners),
            weak_refs: CLongAtomic::new(weak),
        },
        service,
    };
    let (mut c, mut r) = (control(), control());
    // SAFETY: both controls have the real C layout (checked above), remain
    // live and exclusive, and no target is dereferenced by either function.
    unsafe {
        assert_eq!(oracle_obs_weak_service_target(&c), service);
        assert_eq!(rust::obs_weak_service_target(&r), service);
        if owners != CLong::MAX {
            assert_eq!(
                oracle_obs_weak_service_weak_get_ref(&mut c),
                rust::obs_weak_service_weak_get_ref(&mut r)
            );
        }
    }
    assert_eq!(r.counts.refs.load(SeqCst), c.counts.refs.load(SeqCst));
    assert_eq!(r.counts.weak_refs.load(SeqCst), weak);
    assert_eq!(c.counts.weak_refs.load(SeqCst), weak);
    assert_eq!(r.service, c.service);
}

#[test]
fn boundaries_and_expired_controls() {
    for owners in [CLong::MIN, -2, -1, 0, 1, CLong::MAX - 1, CLong::MAX] {
        for weak in [CLong::MIN, -1, 0, 1, CLong::MAX] {
            for null_target in [false, true] {
                check(owners, weak, null_target);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]
    #[test]
    fn random_counts_match_c(owners in any::<CLong>(), weak in any::<CLong>(), null_target in any::<bool>()) {
        check(owners, weak, null_target);
    }
}
