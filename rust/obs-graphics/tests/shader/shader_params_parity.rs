//! Tier 3: original C setters and real private C backend adapters, no GPU.
//! No intentional differences; pointers remain opaque, so no new ABI structs.
use obs_c_oracle as _;
use proptest::prelude::*;
use std::ffi::{CStr, c_char};

unsafe extern "C" {
    fn oracle_shader_params_reset(present: bool);
    fn oracle_shader_params_invoke(rust: bool, kind: u32, token: u32, value: bool, bits: u32);
    fn oracle_shader_params_calls() -> u32;
    fn oracle_shader_params_kind() -> u32;
    fn oracle_shader_params_logs() -> u32;
    fn oracle_shader_params_level() -> i32;
    fn oracle_shader_params_bits() -> u32;
    fn oracle_shader_params_parameter() -> usize;
    fn oracle_shader_params_value() -> usize;
    fn oracle_shader_params_text() -> *const c_char;
    fn oracle_shader_params_input_unchanged() -> bool;
    fn oracle_shader_params_expected_parameter(token: u32) -> usize;
    fn oracle_shader_params_expected_value() -> usize;
}

#[derive(Debug, PartialEq, Eq)]
struct Record {
    calls: u32,
    kind: u32,
    bits: u32,
    parameter: usize,
    value: usize,
    logs: u32,
    level: i32,
    text: Vec<u8>,
}

fn record() -> Record {
    // SAFETY: all getters copy this thread's recording state; text is a live
    // thread-local NUL-terminated array and is copied before the next call.
    unsafe {
        assert!(oracle_shader_params_input_unchanged());
        Record {
            calls: oracle_shader_params_calls(),
            kind: oracle_shader_params_kind(),
            bits: oracle_shader_params_bits(),
            parameter: oracle_shader_params_parameter(),
            value: oracle_shader_params_value(),
            logs: oracle_shader_params_logs(),
            level: oracle_shader_params_level(),
            text: CStr::from_ptr(oracle_shader_params_text())
                .to_bytes()
                .to_vec(),
        }
    }
}

const NAMES: [&str; 8] = [
    "bool", "float", "int", "matrix3", "matrix4", "vec2", "vec3", "vec4",
];

fn compare(context: bool, kind: u32, token: u32, value: bool, bits: u32) {
    // SAFETY: bounded kind/token select live thread-local C tokens and aligned
    // C matrix/vector storage. Both paths share the exact same input addresses.
    unsafe {
        oracle_shader_params_reset(context);
        oracle_shader_params_invoke(false, kind, token, value, bits);
    }
    let expected = record();
    // SAFETY: same live storage and bounded selectors as the original C call.
    unsafe {
        oracle_shader_params_reset(context);
        oracle_shader_params_invoke(true, kind, token, value, bits);
    }
    let actual = record();
    assert_eq!(actual, expected);
    let reason = if !context {
        Some("called while not in a graphics context")
    } else if token == 0 {
        Some("Null 'param' parameter")
    } else if kind >= 3 && !value {
        Some("Null 'val' parameter")
    } else {
        None
    };
    if let Some(reason) = reason {
        assert_eq!(actual.calls, 0);
        assert_eq!(actual.logs, 1);
        assert_eq!(actual.level, 400);
        assert_eq!(
            actual.text,
            format!("gs_shader_set_{}: {reason}", NAMES[kind as usize]).into_bytes()
        );
    } else {
        assert_eq!(actual.calls, 1);
        assert_eq!(actual.logs, 0);
        assert_eq!(actual.kind, kind);
        // SAFETY: token is 1 or 2, and getters only expose live C storage addresses.
        unsafe {
            assert_eq!(
                actual.parameter,
                oracle_shader_params_expected_parameter(token)
            );
            if kind >= 3 {
                assert_eq!(actual.value, oracle_shader_params_expected_value());
            }
        }
        if kind < 3 {
            assert_eq!(actual.bits, if kind == 0 { bits & 1 } else { bits });
        }
    }
}

#[test]
fn validation_and_writes_match_c_characterization() {
    for kind in 0..8 {
        for context in [false, true, false] {
            for token in 0..=2 {
                for value in [false, true] {
                    for bits in [
                        0,
                        0x8000_0000,
                        1,
                        u32::MAX,
                        i32::MAX as u32,
                        1_234_567,
                        1.25_f32.to_bits(),
                        (-42.5_f32).to_bits(),
                        f32::MAX.to_bits(),
                        f32::MIN_POSITIVE.to_bits(),
                        f32::INFINITY.to_bits(),
                        f32::NEG_INFINITY.to_bits(),
                        f32::NAN.to_bits(),
                        0x7fa1_2345,
                    ] {
                        compare(context, kind, token, value, bits);
                    }
                }
            }
        }
    }
}

#[test]
fn repeated_writes_dispatch_once_each() {
    for rust in [false, true] {
        // SAFETY: reset creates the live TLS context; all following selectors are bounded.
        unsafe {
            oracle_shader_params_reset(true);
        }
        for i in 0..32 {
            // SAFETY: valid kind, parameter token, value storage and current context.
            unsafe {
                oracle_shader_params_invoke(rust, i % 8, i % 2 + 1, true, i);
            }
            assert_eq!(record().calls, i + 1);
            assert_eq!(record().logs, 0);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn random_writes_match_c(context in any::<bool>(), kind in 0_u32..8,
        token in 0_u32..3, value in any::<bool>(), bits in any::<u32>()) {
        compare(context, kind, token, value, bits);
    }
}
