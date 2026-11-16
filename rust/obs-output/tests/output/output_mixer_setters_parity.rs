//! Exact parity including logging and all untouched private bytes. No exclusions.
use core::{
    ffi::c_void,
    mem::{align_of, offset_of, size_of},
};
use obs_output::ffi::output_mixer_setters::{Fields, obs_output_set_mixer, obs_output_set_mixers};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;
type Setter = unsafe extern "C" fn(*mut c_void, usize);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_output_mixer_setters_compare(
        set: Setter,
        single: bool,
        initial: usize,
        flags: u32,
        active: bool,
        present: bool,
        value: usize,
        get: GetLog,
        put: SetLog,
    ) -> bool;
    fn oracle_output_mixer_setters_layout(layout: *mut usize);
}
#[test]
fn real_header_layout() {
    let mut layout = [0; 6];
    // SAFETY: oracle writes exactly six usize entries.
    unsafe {
        oracle_output_mixer_setters_layout(layout.as_mut_ptr());
    }
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, mask),
            offset_of!(Fields, active),
            offset_of!(Fields, name),
            offset_of!(Fields, flags)
        ]
    );
}
fn check(initial: usize, flags: u32, active: bool, present: bool, value: usize) {
    for (single, set) in [
        (true, obs_output_set_mixer as Setter),
        (false, obs_output_set_mixers as Setter),
    ] {
        // SAFETY: oracle owns real-layout live fixtures and restores logging; test runs in its own process.
        assert!(
            unsafe {
                oracle_output_mixer_setters_compare(
                    set,
                    single,
                    initial,
                    flags,
                    active,
                    present,
                    value,
                    base_get_log_handler,
                    base_set_log_handler,
                )
            },
            "single={single} initial={initial} flags={flags} active={active} present={present} value={value}"
        );
    }
}
#[test]
fn exact_c_parity() {
    if !super::isolated(concat!(module_path!(), "::exact_c_parity")) {
        return;
    }
    for initial in [0, 0x29, usize::MAX] {
        for flags in [0, 4, u32::MAX] {
            for active in [false, true] {
                for present in [false, true] {
                    for value in [0, 1, 2, 3, 4, 5, 6, 63, 64, usize::MAX] {
                        check(initial, flags, active, present, value);
                    }
                }
            }
        }
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(
                any::<usize>(),
                any::<u32>(),
                any::<bool>(),
                any::<bool>(),
                any::<usize>(),
            ),
            |(initial, flags, active, present, value)| {
                check(initial, flags, active, present, value);
                Ok(())
            },
        )
        .unwrap();
}
