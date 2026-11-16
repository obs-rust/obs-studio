//! Exact C trace parity. No private struct layout crosses the Rust boundary.
//! A non-null buffer is opaque and is never dereferenced by the recording device.
use obs_c_oracle as _;
use obs_graphics::ffi::vertexbuffer_destroy::gs_vertexbuffer_destroy;
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_void};

unsafe extern "C" {
    fn oracle_gs_vertexbuffer_destroy(buffer: *mut c_void);
    fn oracle_vertexbuffer_reset(present: bool, second: bool);
    fn oracle_vertexbuffer_calls() -> u32;
    fn oracle_vertexbuffer_pointer() -> usize;
    fn oracle_vertexbuffer_device() -> u32;
    fn oracle_vertexbuffer_logs() -> u32;
    fn oracle_vertexbuffer_level() -> i32;
    fn oracle_vertexbuffer_log() -> *const c_char;
}

fn compare(present: bool, second: bool, nonnull: bool, byte: u8) {
    let mut storage = byte;
    let buffer = if nonnull {
        std::ptr::from_mut(&mut storage).cast()
    } else {
        std::ptr::null_mut()
    };
    // SAFETY: the oracle owns this thread's real-header context. Its recording
    // callbacks never dereference or free buffer; the local storage remains live.
    unsafe {
        oracle_vertexbuffer_reset(present, second);
        oracle_gs_vertexbuffer_destroy(buffer);
        let expected = (
            oracle_vertexbuffer_calls(),
            oracle_vertexbuffer_pointer(),
            oracle_vertexbuffer_device(),
            oracle_vertexbuffer_logs(),
            oracle_vertexbuffer_level(),
            CStr::from_ptr(oracle_vertexbuffer_log())
                .to_bytes()
                .to_vec(),
        );
        oracle_vertexbuffer_reset(present, second);
        gs_vertexbuffer_destroy(buffer);
        let actual = (
            oracle_vertexbuffer_calls(),
            oracle_vertexbuffer_pointer(),
            oracle_vertexbuffer_device(),
            oracle_vertexbuffer_logs(),
            oracle_vertexbuffer_level(),
            CStr::from_ptr(oracle_vertexbuffer_log())
                .to_bytes()
                .to_vec(),
        );
        assert_eq!(actual, expected);
        assert_eq!(expected.0, u32::from(present && nonnull));
        assert_eq!(
            expected.1,
            if present && nonnull { buffer.addr() } else { 0 }
        );
        assert_eq!(
            expected.2,
            if present && nonnull {
                if second { 2 } else { 1 }
            } else {
                0
            }
        );
        assert_eq!(expected.3, u32::from(!present));
        assert_eq!(expected.4, if present { 0 } else { 400 });
        assert_eq!(
            expected.5,
            if present {
                b"".as_slice()
            } else {
                b"gs_vertexbuffer_destroy: called while not in a graphics context".as_slice()
            }
        );
    }
    assert_eq!(storage, byte);
}

#[test]
fn all_context_buffer_and_device_states_match_c() {
    for present in [false, true] {
        for second in [false, true] {
            for nonnull in [false, true] {
                compare(present, second, nonnull, 0);
                compare(present, second, nonnull, 255);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn device_and_buffer_sequences_match_c(states in prop::collection::vec((any::<bool>(), any::<bool>(), any::<bool>(), any::<u8>()), 0..64)) {
        for (present, second, nonnull, byte) in states { compare(present, second, nonnull, byte); }
    }
}
