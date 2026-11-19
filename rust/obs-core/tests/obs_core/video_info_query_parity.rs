//! Real-header ABI and exact differential parity, including untouched storage.
use core::mem::{align_of, offset_of, size_of};
use obs_core::ffi::video_info_query::{VideoInfo, obs_get_video_info};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*mut VideoInfo) -> bool;
unsafe extern "C" {
    fn oracle_video_info_query_layout(layout: *mut usize);
    fn oracle_video_info_query_check(
        candidate: Getter,
        present: bool,
        graphics: bool,
        mixed: bool,
        null_canvas: bool,
        null_output: bool,
        null_module: bool,
        value: u8,
        word: u32,
    ) -> bool;
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 15];
    // SAFETY: the C oracle writes exactly fifteen values into this array.
    unsafe { oracle_video_info_query_layout(layout.as_mut_ptr()) };
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

fn check(flags: [bool; 6], value: u8, word: u32) {
    let [
        present,
        graphics,
        mixed,
        null_canvas,
        null_output,
        null_module,
    ] = flags;
    // Exclude exactly C's undefined inputs: live graphics with NULL canvas,
    // or writable output missing on success. Unavailable paths allow NULL output.
    if present && graphics && (null_canvas || (mixed && null_output)) {
        return;
    }
    // SAFETY: the oracle owns and snapshots all real-layout state per thread;
    // only the valid C inputs above reach either getter.
    assert!(unsafe {
        oracle_video_info_query_check(
            obs_get_video_info,
            present,
            graphics,
            mixed,
            null_canvas,
            null_output,
            null_module,
            value,
            word,
        )
    });
}

#[test]
fn characterized_edges() {
    for bits in 0..64 {
        let flags = core::array::from_fn(|bit| bits & (1 << bit) != 0);
        for value in [0, 1, 0x5a, 0xa5, u8::MAX] {
            for word in [0, 1, 7919, 0x12345678, u32::MAX] {
                check(flags, value, word);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn output_and_state_match_c(flags in any::<[bool; 6]>(), value in any::<u8>(), word in any::<u32>()) {
        check(flags, value, word);
    }
}
