//! Tier 3: the Rust wrapper against the original extracted
//! obs-output-preferred-size.c.h over the real private output layout. Both
//! sides delegate to the same C recorder in place of the indexed setter, so
//! the comparison covers the log line (count, level, text), each delegated
//! call (output, width, height, slot) and every byte of the output.
use core::ffi::c_void;
use obs_output::ffi::output_preferred_size::{Calls, set_preferred_size};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Setter = unsafe extern "C" fn(*mut c_void, u32, u32);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_preferred_size_record(output: *mut c_void, width: u32, height: u32, idx: usize);
    fn oracle_output_preferred_size_compare(
        set: Setter,
        present: bool,
        flags: u32,
        active: bool,
        width: u32,
        height: u32,
        get: GetLog,
        put: SetLog,
    ) -> bool;
}

struct Recorder;
impl Calls for Recorder {
    unsafe fn set_preferred_size2(&self, output: *mut c_void, width: u32, height: u32, idx: usize) {
        // SAFETY: the recorder only stores its arguments.
        unsafe { oracle_preferred_size_record(output, width, height, idx) }
    }
}

unsafe extern "C" fn rust_wrapper(output: *mut c_void, width: u32, height: u32) {
    // SAFETY: the oracle passes NULL or its live fixture.
    unsafe { set_preferred_size(&Recorder, output, width, height) }
}

fn check(present: bool, flags: u32, active: bool, width: u32, height: u32) {
    // SAFETY: the oracle owns live real-layout fixtures and restores logging;
    // the caller runs in its own process.
    assert!(
        unsafe {
            oracle_output_preferred_size_compare(
                rust_wrapper,
                present,
                flags,
                active,
                width,
                height,
                base_get_log_handler,
                base_set_log_handler,
            )
        },
        "present={present} flags={flags:#x} active={active} width={width} height={height}"
    );
}

#[test]
fn exact_c_parity() {
    if !super::isolated(concat!(module_path!(), "::exact_c_parity")) {
        return;
    }
    let sizes = [
        (0, 0),
        (1, 3),
        (1919, 1079),
        (0, 701),
        (905, 0),
        (u32::MAX, u32::MAX - 1),
    ];
    for present in [false, true] {
        for flags in [0, 1, 2, 3, 4, 5, 8, 0xe, u32::MAX] {
            for active in [false, true] {
                for (width, height) in sizes {
                    check(present, flags, active, width, height);
                }
            }
        }
    }
}

#[test]
fn random_c_parity() {
    if !super::isolated(concat!(module_path!(), "::random_c_parity")) {
        return;
    }
    proptest!(ProptestConfig::with_cases(256), |(
        present in any::<bool>(),
        flags in any::<u32>(),
        active in any::<bool>(),
        width in any::<u32>(),
        height in any::<u32>(),
    )| {
        check(present, flags, active, width, height);
    });
}
