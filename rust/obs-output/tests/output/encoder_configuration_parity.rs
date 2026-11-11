//! Tier 3: exact full encoder state, diagnostics and video-call parity, no exclusions.
use core::ffi::{c_int, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::encoder_configuration::{self as rust, Fields};
use proptest::prelude::*;
unsafe extern "C" {
    fn oracle_encoder_configuration_layout(layout: *mut usize);
    fn oracle_encoder_configuration_probe(
        flags: u32,
        operation: c_int,
        a: u32,
        b: u32,
        width: u32,
        height: u32,
        size_fn: unsafe extern "C" fn(*mut c_void, u32, u32),
        gpu_fn: unsafe extern "C" fn(*mut c_void, c_int),
        divisor_fn: unsafe extern "C" fn(*mut c_void, u32) -> bool,
    ) -> bool;
}
#[test]
fn real_header_layout() {
    let mut c = [0; 15];
    // SAFETY: the oracle writes exactly 15 size_t entries.
    unsafe {
        oracle_encoder_configuration_layout(c.as_mut_ptr());
    }
    assert_eq!(
        c,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, kind),
            offset_of!(Fields, active),
            offset_of!(Fields, initialized),
            offset_of!(Fields, width),
            offset_of!(Fields, height),
            offset_of!(Fields, gpu),
            offset_of!(Fields, divisor),
            offset_of!(Fields, media),
            offset_of!(Fields, fps_override),
            offset_of!(Fields, name),
            size_of::<c_int>(),
            size_of::<c_int>(),
            size_of::<bool>()
        ]
    );
}
fn check(flags: u32, operation: i32, a: u32, b: u32, width: u32, height: u32) {
    // SAFETY: oracle uses separate, real-layout stack encoders and synchronous
    // callbacks. Its video-call stand-ins never allocate; logger is isolated.
    assert!(
        unsafe {
            oracle_encoder_configuration_probe(
                flags,
                operation,
                a,
                b,
                width,
                height,
                rust::obs_encoder_set_scaled_size,
                rust::obs_encoder_set_gpu_scale_type,
                rust::obs_encoder_set_frame_rate_divisor,
            )
        },
        "flags={flags} op={operation} a={a} b={b} output={width}x{height}"
    );
}
#[test]
fn all_validation_paths_and_lifecycle_edges() {
    if !super::isolated(concat!(
        module_path!(),
        "::all_validation_paths_and_lifecycle_edges"
    )) {
        return;
    }
    for flags in 0..64 {
        for op in 0..3 {
            for a in [0, 1, 2, 1920, 0x8000_0000, u32::MAX] {
                check(flags, op, a, 1080, 1920, 1080);
                check(flags, op, a, 0, 0, 0);
            }
        }
    }
}
#[test]
fn randomized_state_warning_and_effect_parity() {
    if !super::isolated(concat!(
        module_path!(),
        "::randomized_state_warning_and_effect_parity"
    )) {
        return;
    }
    let config = ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    };
    proptest::test_runner::TestRunner::new(config)
        .run(
            &(
                0u32..64,
                0i32..3,
                any::<u32>(),
                any::<u32>(),
                any::<u32>(),
                any::<u32>(),
            ),
            |(flags, op, a, b, w, h)| {
                check(flags, op, a, b, w, h);
                Ok(())
            },
        )
        .unwrap();
}
