//! Original C and Rust exports run through the same observed native primitives.
//! No intentional differences. Counts/repeats are capped to bound all allocations.
#![allow(unsafe_code)]
use super::encoder_group_lifecycle::Model;
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use lifecycle::Backend;
use obs_output::encoder_group_lifecycle as lifecycle;
use obs_output::ffi::encoder_group_lifecycle::{
    GroupFields, obs_encoder_group_actually_destroy, obs_encoder_group_create,
    obs_encoder_group_destroy,
};
use proptest::prelude::*;

type Create = unsafe extern "C" fn() -> *mut c_void;
type Destroy = unsafe extern "C" fn(*mut c_void);
unsafe extern "C" {
    fn oracle_obs_encoder_group_create() -> *mut c_void;
    fn oracle_obs_encoder_group_destroy(group: *mut c_void);
    fn oracle_obs_encoder_group_actually_destroy(group: *mut c_void);
    fn oracle_encoder_group_lifecycle_run(
        create: Create,
        destroy: Destroy,
        actually: Destroy,
        count: usize,
        started: u32,
        repeats: usize,
        direct: bool,
        fail_init: bool,
        events: *mut u32,
    ) -> usize;
    fn oracle_encoder_group_lifecycle_layout(layout: *mut usize);
}

fn check(count: usize, started: u32, repeats: usize, direct: bool, fail_init: bool) {
    let mut model = Model {
        fail_init,
        ..Model::default()
    };
    if lifecycle::create(&mut model).is_some() {
        model.encoders = vec![true; count];
        model.started = started;
        if started != 0 {
            for _ in 0..repeats {
                lifecycle::destroy(&mut model, Some(()));
            }
        }
        model.started = 0;
        if direct {
            model.lock(());
            lifecycle::actually_destroy(&mut model, ());
        } else {
            lifecycle::destroy(&mut model, Some(()));
        }
    }
    lifecycle::destroy(&mut model, None);
    for (create, destroy, actually) in [
        (
            oracle_obs_encoder_group_create as Create,
            oracle_obs_encoder_group_destroy as Destroy,
            oracle_obs_encoder_group_actually_destroy as Destroy,
        ),
        (
            obs_encoder_group_create as Create,
            obs_encoder_group_destroy as Destroy,
            obs_encoder_group_actually_destroy as Destroy,
        ),
    ] {
        let mut events = [0; 512];
        // SAFETY: the runner bounds all inputs and owns complete real-header objects;
        // callbacks follow the original live-group / locked-handoff contracts.
        let length = unsafe {
            oracle_encoder_group_lifecycle_run(
                create,
                destroy,
                actually,
                count,
                started,
                repeats,
                direct,
                fail_init,
                events.as_mut_ptr(),
            )
        };
        assert!(length <= events.len(), "native lifecycle invariants failed");
        assert_eq!(&events[..length], model.trace);
    }
}

#[test]
fn layout_matches_real_header() {
    let mut layout = [usize::MAX; 4];
    // SAFETY: the helper writes exactly four size_t values.
    unsafe { oracle_encoder_group_lifecycle_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<GroupFields>(),
            align_of::<GroupFields>(),
            offset_of!(GroupFields, started),
            offset_of!(GroupFields, destroy_on_stop)
        ]
    );
}

#[test]
fn characterized_creation_failure_deferred_order_and_null() {
    for count in 0..=3 {
        for started in [0, 1, 2, u32::MAX] {
            for direct in [false, true] {
                for fail_init in [false, true] {
                    check(count, started, 2, direct, fail_init);
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn randomized_lifecycle_matches_c(count in 0usize..=16, started in prop_oneof![Just(0u32), Just(1), Just(2), any::<u32>()],
        repeats in 0usize..=16, direct in any::<bool>(), fail_init in any::<bool>()) {
        check(count, started, repeats, direct, fail_init);
    }
}
