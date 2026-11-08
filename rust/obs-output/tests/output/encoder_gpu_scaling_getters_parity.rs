//! Exact public getter parity, real-header layout, logs and whole-encoder immutability.
use core::ffi::{c_int, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::encoder_gpu_scaling_getters::{
    GpuScalingFields, obs_encoder_get_scale_type, obs_encoder_gpu_scaling_enabled,
};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_encoder_gpu_scaling_layout(layout: *mut usize);
    fn oracle_encoder_gpu_scaling_new(
        kind: c_int,
        scale: c_int,
        width: u32,
        height: u32,
    ) -> *mut c_void;
    fn oracle_encoder_gpu_scaling_unchanged(encoder: *mut c_void) -> bool;
    fn oracle_encoder_gpu_scaling_free(encoder: *mut c_void);
    fn oracle_encoder_gpu_scaling_probe(
        encoder: *mut c_void,
        enabled: unsafe extern "C" fn(*mut c_void) -> bool,
        scale_type: unsafe extern "C" fn(*mut c_void) -> c_int,
    ) -> bool;
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 13];
    // SAFETY: the oracle writes exactly thirteen size_t values.
    unsafe { oracle_encoder_gpu_scaling_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<GpuScalingFields>(),
            align_of::<GpuScalingFields>(),
            offset_of!(GpuScalingFields, kind),
            offset_of!(GpuScalingFields, scale),
            offset_of!(GpuScalingFields, name),
            size_of::<c_int>(),
            align_of::<c_int>(),
            size_of::<c_int>(),
            align_of::<c_int>(),
            0,
            1,
            0,
            5
        ]
    );
}

struct Encoder(*mut c_void);
impl Drop for Encoder {
    fn drop(&mut self) {
        // SAFETY: this wrapper owns the fixture, released exactly once.
        unsafe { oracle_encoder_gpu_scaling_free(self.0) };
    }
}
fn check(kind: i32, scale: i32, width: u32, height: u32) {
    // SAFETY: the fixture constructor initializes the real private C layout.
    let encoder = Encoder(unsafe { oracle_encoder_gpu_scaling_new(kind, scale, width, height) });
    assert!(!encoder.0.is_null());
    // SAFETY: fixture is exclusive and live, the probe restores logging in this isolated process.
    unsafe {
        assert!(oracle_encoder_gpu_scaling_probe(
            encoder.0,
            obs_encoder_gpu_scaling_enabled,
            obs_encoder_get_scale_type
        ));
        assert!(oracle_encoder_gpu_scaling_unchanged(encoder.0));
    }
}

#[test]
fn characterized_edges_and_null_logs() {
    if !super::isolated(concat!(
        module_path!(),
        "::characterized_edges_and_null_logs"
    )) {
        return;
    }
    // SAFETY: both callbacks accept NULL and the probe restores logging.
    unsafe {
        assert!(oracle_encoder_gpu_scaling_probe(
            core::ptr::null_mut(),
            obs_encoder_gpu_scaling_enabled,
            obs_encoder_get_scale_type
        ));
    }
    for kind in [i32::MIN, -1, 0, 1, 2, i32::MAX] {
        for scale in [i32::MIN, -1, 0, 1, 2, 3, 4, 5, 6, i32::MAX] {
            for (width, height) in [(0, 0), (1, 0), (0, 1), (u32::MAX, u32::MAX)] {
                check(kind, scale, width, height);
            }
        }
    }
}

#[test]
fn random_fields_match_c() {
    if !super::isolated(concat!(module_path!(), "::random_fields_match_c")) {
        return;
    }
    let config = ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(config);
    runner
        .run(
            &(
                prop_oneof![Just(1i32), any::<i32>()],
                any::<i32>(),
                any::<u32>(),
                any::<u32>(),
            ),
            |(kind, scale, width, height)| {
                check(kind, scale, width, height);
                Ok(())
            },
        )
        .unwrap();
}
