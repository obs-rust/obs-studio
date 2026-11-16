//! Tier 3: original extracted C wrappers versus the exported Rust ABI.
//! No intentional differences. Fixtures and diagnostics are thread-local.
use std::ffi::{CStr, c_char, c_void};
use std::ptr;

use obs_c_oracle as _;
use obs_graphics::ffi::vertexbuffer_access::{gs_vertexbuffer_flush, gs_vertexbuffer_get_data};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_gs_vertexbuffer_flush(buffer: *mut c_void);
    fn oracle_gs_vertexbuffer_get_data(buffer: *const c_void) -> *mut c_void;
    fn oracle_vertexbuffer_access_reset(present: bool, result_present: bool);
    fn oracle_vertexbuffer_access_leave();
    fn oracle_vertexbuffer_access_buffer() -> *mut c_void;
    fn oracle_vertexbuffer_access_result() -> *mut c_void;
    fn oracle_vertexbuffer_access_calls() -> u32;
    fn oracle_vertexbuffer_access_seen() -> *const c_void;
    fn oracle_vertexbuffer_access_logs() -> u32;
    fn oracle_vertexbuffer_access_level() -> i32;
    fn oracle_vertexbuffer_access_log() -> *const c_char;
    fn oracle_vertexbuffer_access_pointer_size() -> usize;
    fn oracle_vertexbuffer_access_pointer_align() -> usize;
}

#[derive(Debug, PartialEq, Eq)]
struct Trace {
    result: *mut c_void,
    seen: *const c_void,
    calls: u32,
    logs: u32,
    level: i32,
    message: String,
}

fn run(
    rust: bool,
    getter: bool,
    present: bool,
    buffer_present: bool,
    result_present: bool,
) -> Trace {
    // SAFETY: fixtures are thread-local, all opaque pointers refer to live C
    // storage, and callbacks inspect only pointer identity without dereferencing.
    unsafe {
        oracle_vertexbuffer_access_reset(present, result_present);
        let buffer = if buffer_present {
            oracle_vertexbuffer_access_buffer()
        } else {
            ptr::null_mut()
        };
        let result = match (rust, getter) {
            (true, true) => gs_vertexbuffer_get_data(buffer),
            (false, true) => oracle_gs_vertexbuffer_get_data(buffer),
            (true, false) => {
                gs_vertexbuffer_flush(buffer);
                ptr::null_mut()
            }
            (false, false) => {
                oracle_gs_vertexbuffer_flush(buffer);
                ptr::null_mut()
            }
        };
        let trace = Trace {
            result,
            seen: oracle_vertexbuffer_access_seen(),
            calls: oracle_vertexbuffer_access_calls(),
            logs: oracle_vertexbuffer_access_logs(),
            level: oracle_vertexbuffer_access_level(),
            message: CStr::from_ptr(oracle_vertexbuffer_access_log())
                .to_string_lossy()
                .into_owned(),
        };
        let valid = present && buffer_present;
        assert_eq!(trace.calls, u32::from(valid));
        assert_eq!(
            trace.seen,
            if valid {
                buffer.cast_const()
            } else {
                ptr::null()
            }
        );
        assert_eq!(
            result,
            if getter && valid {
                oracle_vertexbuffer_access_result()
            } else {
                ptr::null_mut()
            }
        );
        let name = if getter {
            "gs_vertexbuffer_get_data"
        } else {
            "gs_vertexbuffer_flush"
        };
        let expected = if !present {
            format!("{name}: called while not in a graphics context")
        } else if !buffer_present {
            format!("{name}: Null 'vertbuffer' parameter")
        } else {
            String::new()
        };
        assert_eq!(trace.message, expected);
        assert_eq!(trace.logs, u32::from(!valid));
        assert_eq!(trace.level, if valid { 0 } else { 400 }); // LOG_DEBUG
        trace
    }
}

#[test]
fn exact_pointer_callback_and_diagnostic_traces() {
    for getter in [false, true] {
        for present in [false, true] {
            for buffer in [false, true] {
                for result in [false, true] {
                    assert_eq!(
                        run(true, getter, present, buffer, result),
                        run(false, getter, present, buffer, result)
                    );
                }
            }
        }
    }
}

#[test]
fn context_is_not_retained_and_opaque_pointer_layout_matches_c() {
    // SAFETY: the thread-local fixture owns both buffers and context throughout.
    unsafe {
        assert_eq!(
            size_of::<*mut c_void>(),
            oracle_vertexbuffer_access_pointer_size()
        );
        assert_eq!(
            align_of::<*mut c_void>(),
            oracle_vertexbuffer_access_pointer_align()
        );
        for rust in [false, true] {
            oracle_vertexbuffer_access_reset(true, true);
            let buffer = oracle_vertexbuffer_access_buffer();
            let getter = if rust {
                gs_vertexbuffer_get_data
            } else {
                oracle_gs_vertexbuffer_get_data
            };
            assert_eq!(getter(buffer), oracle_vertexbuffer_access_result());
            oracle_vertexbuffer_access_leave();
            assert!(getter(buffer).is_null());
            assert_eq!(oracle_vertexbuffer_access_calls(), 1);
            assert_eq!(oracle_vertexbuffer_access_logs(), 1);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(64)))]
    #[test]
    fn callback_sequences_match_c(sequence in prop::collection::vec((any::<bool>(), any::<bool>(), any::<bool>(), any::<bool>()), 0..32)) {
        for (getter, context, buffer, result) in sequence {
            prop_assert_eq!(run(true, getter, context, buffer, result), run(false, getter, context, buffer, result));
        }
    }
}
