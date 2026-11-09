//! Exact lifecycle parity with the extracted original body and real C adapter.
//! EOF/EAGAIN sequences always terminate; the C's infinite retry is preserved,
//! so a sequence of only retries is not a terminating differential input.
use core::ffi::c_void;
use obs_graphics::ffi::image_decode::gs_ffmpeg_image_decode;
use proptest::prelude::*;

type Decode = unsafe extern "C" fn(*mut c_void, i32) -> *mut c_void;
unsafe extern "C" {
    fn oracle_image_decode_run(config: *const i32, out: *mut i32, decode: Option<Decode>);
}

fn compare(config: &[i32; 25]) {
    let mut c = [0; 128];
    let mut rust = [0; 128];
    // SAFETY: the driver reads 25 config values, writes at most 128 results,
    // and gives the shim a live C context with real adapter-owned allocations.
    unsafe {
        oracle_image_decode_run(config.as_ptr(), c.as_mut_ptr(), None);
        oracle_image_decode_run(
            config.as_ptr(),
            rust.as_mut_ptr(),
            Some(gs_ffmpeg_image_decode),
        );
    }
    assert_eq!(rust, c, "config={config:?}");
}

#[test]
fn lifecycle_edges() {
    for allocate in [0, 1] {
        for read in [-22, 0, 1] {
            for too_large in [0, 1, 2] {
                for reject in [0, 1] {
                    for status in [-100, -11, -22, 0] {
                        for alpha in [0, 1, 2] {
                            let mut config = [0; 25];
                            config[..8].copy_from_slice(&[
                                allocate, read, too_large, reject, 16, 8, 26, 2,
                            ]);
                            config[8] = status;
                            config[9] = 0;
                            config[24] = alpha;
                            compare(&config);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn null_context_is_rejected_before_adapter_access() {
    // SAFETY: the shim explicitly accepts a null context and returns null.
    assert!(unsafe { gs_ffmpeg_image_decode(core::ptr::null_mut(), 0) }.is_null());
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn packet_frame_lifecycle(
        allocate in any::<bool>(), read in prop_oneof![Just(0),Just(-5),Just(1)],
        flags in 0i32..3, reject in any::<bool>(), width in any::<i32>(),
        height in any::<i32>(), format in any::<i32>(), alpha in any::<i32>(),
        statuses in prop::collection::vec(prop_oneof![Just(-100),Just(-11),Just(-22),Just(0)],0..16),
    ) {
        let mut config = [0;25];
        config[..8].copy_from_slice(&[i32::from(allocate),read,flags,i32::from(reject),width,height,format,i32::try_from(statuses.len()).unwrap()]);
        config[8..8+statuses.len()].copy_from_slice(&statuses);
        config[24] = alpha;
        compare(&config);
    }
}
