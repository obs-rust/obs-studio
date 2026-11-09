//! Exact Tier 3 sequences and real-header layout. No behavior exclusions.
//! NULL output is excluded: the original setter requires a live output.
use core::ffi::{c_int, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::output_reconnect_callback::{
    Callback, ReconnectCallback, obs_output_set_reconnect_callback,
};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_output_reconnect_callback_layout(layout: *mut usize);
    fn oracle_output_reconnect_callback_new() -> *mut c_void;
    fn oracle_output_reconnect_callback_free(output: *mut c_void);
    fn oracle_obs_output_set_reconnect_callback(
        output: *mut c_void,
        callback: Option<Callback>,
        param: *mut c_void,
    );
    fn oracle_output_reconnect_callback_matches(
        output: *mut c_void,
        callback: Option<Callback>,
        param: *mut c_void,
    ) -> bool;
    fn oracle_output_reconnect_callback_invoke(
        output: *mut c_void,
        argument: *mut c_void,
        code: c_int,
    ) -> bool;
}

unsafe extern "C" fn first(data: *mut c_void, output: *mut c_void, code: c_int) -> bool {
    data == output && code == -5
}
unsafe extern "C" fn second(data: *mut c_void, output: *mut c_void, code: c_int) -> bool {
    data.is_null() && !output.is_null() && code == 0
}
struct Output(*mut c_void);
impl Output {
    fn new() -> Self {
        // SAFETY: resource-free real-header fixture, owned by this wrapper.
        let value = Self(unsafe { oracle_output_reconnect_callback_new() });
        assert!(!value.0.is_null());
        value
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        // SAFETY: fixture allocated by the matching C allocator, exactly once.
        unsafe { oracle_output_reconnect_callback_free(self.0) };
    }
}

#[test]
fn real_header_callback_layout() {
    let mut layout = [0; 4];
    // SAFETY: writes four size_t values into the array.
    unsafe { oracle_output_reconnect_callback_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<ReconnectCallback>(),
            align_of::<ReconnectCallback>(),
            offset_of!(ReconnectCallback, reconnect_cb),
            offset_of!(ReconnectCallback, param)
        ]
    );
}

fn sequence(steps: &[(u8, u8)]) {
    let rust = Output::new();
    let c = Output::new();
    let mut tokens = [7_u8, 19_u8];
    let pointers = [
        core::ptr::null_mut(),
        (&raw mut tokens[0]).cast(),
        (&raw mut tokens[1]).cast(),
    ];
    for &(callback, data) in steps {
        let callback: Option<Callback> = match callback {
            0 => None,
            1 => Some(first),
            _ => Some(second),
        };
        let data = pointers[usize::from(data)];
        let expected_data = if callback.is_some() {
            data
        } else {
            core::ptr::null_mut()
        };
        // SAFETY: independent exclusively owned live C outputs; callbacks never
        // dereference arguments and all data tokens remain live for the sequence.
        unsafe {
            obs_output_set_reconnect_callback(rust.0, callback, data);
            oracle_obs_output_set_reconnect_callback(c.0, callback, data);
            assert!(oracle_output_reconnect_callback_matches(
                rust.0,
                callback,
                expected_data
            ));
            assert!(oracle_output_reconnect_callback_matches(
                c.0,
                callback,
                expected_data
            ));
            if let Some(callback) = callback {
                for argument in pointers {
                    for code in [i32::MIN, -5, 0, i32::MAX] {
                        let expected = callback(expected_data, argument, code);
                        assert_eq!(
                            oracle_output_reconnect_callback_invoke(rust.0, argument, code),
                            expected
                        );
                        assert_eq!(
                            oracle_output_reconnect_callback_invoke(c.0, argument, code),
                            expected
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn characterization_sequence_and_every_transition() {
    sequence(&[(1, 1), (2, 2), (2, 0), (0, 2), (0, 0), (1, 2)]);
    sequence(&[(0, 2)]);
    for first in 0..3 {
        for second in 0..3 {
            for data in 0..3 {
                sequence(&[(first, data), (second, data)]);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn arbitrary_replacement_sequences(steps in prop::collection::vec((0_u8..3, 0_u8..3), 1..65)) {
        sequence(&steps);
    }
}
