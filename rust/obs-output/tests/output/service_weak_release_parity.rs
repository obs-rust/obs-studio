//! Exact parity on defined C inputs. NULL controls are invalid except for free.
//! Atomic add/sub wrap at C-long width, including LONG_MIN and LONG_MAX.
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::service_weak_acquire::{WeakRef, WeakService};
use obs_output::ffi::service_weak_release as rust;
use obs_util::ffi::bmem::{bmalloc, bnum_allocs};
use obs_util::platform::{CLong, CLongAtomic};
use proptest::prelude::*;
use std::sync::atomic::Ordering::SeqCst;

unsafe extern "C" {
    fn oracle_obs_weak_service_strong_release(weak: *mut WeakService) -> bool;
    fn oracle_obs_weak_service_weak_addref(weak: *mut WeakService);
    fn oracle_obs_weak_service_weak_release(weak: *mut WeakService) -> bool;
    fn oracle_obs_weak_service_free(weak: *mut WeakService);
    fn oracle_service_weak_acquire_layout(layout: *mut usize);
}

#[test]
fn real_header_layout() {
    let mut c = [0; 8];
    // SAFETY: the real-header helper writes eight size_t values.
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
            offset_of!(WeakService, service)
        ]
    );
}

fn compare(strong: CLong, weak: CLong, null_target: bool, operations: &[u8]) {
    let mut marker = 0u8;
    let service = if null_target {
        core::ptr::null_mut()
    } else {
        core::ptr::from_mut(&mut marker).cast()
    };
    let control = || WeakService {
        counts: WeakRef {
            refs: CLongAtomic::new(strong),
            weak_refs: CLongAtomic::new(weak),
        },
        service,
    };
    let (mut c, mut r) = (control(), control());
    for op in operations {
        // SAFETY: independent aligned controls use the real C layout, remain
        // live, and the target is never dereferenced.
        unsafe {
            match op % 3 {
                0 => assert_eq!(
                    rust::obs_weak_service_strong_release(&mut r),
                    oracle_obs_weak_service_strong_release(&mut c)
                ),
                1 => {
                    rust::obs_weak_service_weak_addref(&mut r);
                    oracle_obs_weak_service_weak_addref(&mut c);
                }
                _ => assert_eq!(
                    rust::obs_weak_service_weak_release(&mut r),
                    oracle_obs_weak_service_weak_release(&mut c)
                ),
            }
        }
        assert_eq!(r.counts.refs.load(SeqCst), c.counts.refs.load(SeqCst));
        assert_eq!(
            r.counts.weak_refs.load(SeqCst),
            c.counts.weak_refs.load(SeqCst)
        );
        assert_eq!(r.service, service);
        assert_eq!(c.service, service);
    }
}

#[test]
fn boundary_transitions_match_c() {
    for strong in [CLong::MIN, CLong::MIN + 1, -2, -1, 0, 1, CLong::MAX] {
        for weak in [
            CLong::MIN,
            CLong::MIN + 1,
            -2,
            -1,
            0,
            1,
            CLong::MAX - 1,
            CLong::MAX,
        ] {
            for null_target in [false, true] {
                compare(strong, weak, null_target, &[0, 1, 2, 2, 1, 0, 2, 0]);
            }
        }
    }
}

#[test]
fn free_uses_bfree_and_preserves_allocation_accounting() {
    if !super::isolated(concat!(
        module_path!(),
        "::free_uses_bfree_and_preserves_allocation_accounting"
    )) {
        return;
    }
    // SAFETY: the child has no concurrent tests, each control comes from
    // bmalloc, and ownership transfers exactly once to its matching free.
    unsafe {
        let baseline = bnum_allocs();
        rust::obs_weak_service_free(core::ptr::null_mut());
        oracle_obs_weak_service_free(core::ptr::null_mut());
        assert_eq!(bnum_allocs(), baseline);
        for _ in 0..32 {
            let c = bmalloc(size_of::<WeakService>()).cast::<WeakService>();
            let r = bmalloc(size_of::<WeakService>()).cast::<WeakService>();
            assert_eq!(bnum_allocs(), baseline + 2);
            oracle_obs_weak_service_free(c);
            assert_eq!(bnum_allocs(), baseline + 1);
            rust::obs_weak_service_free(r);
            assert_eq!(bnum_allocs(), baseline);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn operation_sequences_match_c(strong in any::<CLong>(), weak in any::<CLong>(),
        null_target in any::<bool>(), operations in prop::collection::vec(0u8..3, 1..64)) {
        compare(strong, weak, null_target, &operations);
    }
}
