//! Differential resource, diagnostic and callback traces against the original C.
//! FFmpeg allocation failure is outside the original's defined contract (it
//! dereferences the null allocation). All generated counts and sizes are bounded.
#![allow(unsafe_code)]
use obs_c_oracle as _;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_image_init_run(config: *const i32, out: *mut i32, rust: bool);
}
fn compare(config: [i32; 13]) {
    let mut c = [0; 128];
    let mut rust = [0; 128];
    // SAFETY: config has the thirteen bounded fixture fields and each output has 128 entries.
    unsafe {
        oracle_image_init_run(config.as_ptr(), c.as_mut_ptr(), false);
        oracle_image_init_run(config.as_ptr(), rust.as_mut_ptr(), true);
    }
    assert_eq!(rust, c, "fixture {config:?}");
}
#[test]
fn lifecycle_edges() {
    // The decoder is small; only FFmpeg's worker callback discovers the large frame.
    compare([2, 1, 0, 1, 3, 2, 0, 17, 2, 4, 0, 0, 1]);
    // A valid worker context must also be passed unchanged to the default allocator.
    compare([2, 1, 0, 1, 3, 2, 0, 4, 4, 7, -31, 0, 1]);
    for path in 0..=2 {
        for streams in 0..=2 {
            for stage in 0..=5 {
                for (width, height) in [(0, 0), (-1, 2), (3, 2), (16, 16), (17, 2), (2, 17)] {
                    compare([
                        path, streams, 0, 1, width, height, stage, width, height, 3, -31, 0, 0,
                    ]);
                }
            }
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(300), failure_persistence: None, .. ProptestConfig::default()
    })]
    #[test]
    fn lifecycle_random_traces(
        path in 0i32..3, streams in 0i32..3, video in 0i32..3, codec in 0i32..3,
        width in -2i32..20, height in -2i32..20, stage in 0i32..6,
        bw in -2i32..20, bh in -2i32..20, flags in any::<i32>(), result in -32i32..2,
        parameter_result in -3i32..1, cloned in any::<bool>(),
    ) {
        compare([path, streams, video, codec, width, height, stage, bw, bh, flags, result, parameter_result, i32::from(cloned)]);
    }
}
