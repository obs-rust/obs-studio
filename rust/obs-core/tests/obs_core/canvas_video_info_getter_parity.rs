//! Exact real-header layout and byte/state parity; invalid C inputs excluded.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_core::ffi::canvas_video_info_getter::{VideoInfo, obs_canvas_get_video_info};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void, *mut VideoInfo) -> bool;
unsafe extern "C" {
    fn oracle_canvas_video_info_layout(layout: *mut usize);
    fn oracle_canvas_video_info_check(
        candidate: Getter,
        graphics: bool,
        mixed: bool,
        null_canvas: bool,
        null_output: bool,
        value: u8,
        word: u32,
    ) -> bool;
}

#[test]
fn real_header_video_info_layout() {
    let mut layout = [0; 15];
    // SAFETY: the oracle writes exactly fifteen size/alignment/offset values.
    unsafe { oracle_canvas_video_info_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<VideoInfo>(),
            align_of::<VideoInfo>(),
            offset_of!(VideoInfo, graphics_module),
            offset_of!(VideoInfo, fps_num),
            offset_of!(VideoInfo, fps_den),
            offset_of!(VideoInfo, base_width),
            offset_of!(VideoInfo, base_height),
            offset_of!(VideoInfo, output_width),
            offset_of!(VideoInfo, output_height),
            offset_of!(VideoInfo, output_format),
            offset_of!(VideoInfo, adapter),
            offset_of!(VideoInfo, gpu_conversion),
            offset_of!(VideoInfo, colorspace),
            offset_of!(VideoInfo, range),
            offset_of!(VideoInfo, scale_type)
        ]
    );
}

fn check(graphics: bool, mixed: bool, null_canvas: bool, null_output: bool, value: u8, word: u32) {
    // Exclude exactly the original getter's invalid dereferences: NULL canvas
    // with graphics, or NULL output on success. Tier 2 probes the crash contract.
    if graphics && (null_canvas || (mixed && null_output)) {
        return;
    }
    // SAFETY: the oracle owns real-layout storage and supplies initialized OBS
    // state per thread, calling the callback only with the valid inputs above.
    assert!(unsafe {
        oracle_canvas_video_info_check(
            obs_canvas_get_video_info,
            graphics,
            mixed,
            null_canvas,
            null_output,
            value,
            word,
        )
    });
}

#[test]
fn characterized_edges() {
    for graphics in [false, true] {
        for mixed in [false, true] {
            for null_canvas in [false, true] {
                for null_output in [false, true] {
                    for value in 0..=u8::MAX {
                        check(graphics, mixed, null_canvas, null_output, value, u32::MAX);
                    }
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn output_bytes_and_state_match_c(graphics in any::<bool>(), mixed in any::<bool>(),
        null_canvas in any::<bool>(), null_output in any::<bool>(), value in any::<u8>(), word in any::<u32>()) {
        check(graphics, mixed, null_canvas, null_output, value, word);
    }
}
