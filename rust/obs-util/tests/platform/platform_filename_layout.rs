//! Tier 2: `struct obs_video_info` in the `os_generate_formatted_filename`
//! shim matches the C header.

use core::mem::{align_of, offset_of, size_of};

use obs_c_oracle::platform_filename as c;
use obs_util::ffi::platform_filename::obs_video_info;

#[test]
fn obs_video_info_layout_matches_c_header() {
    let mut offsets = [usize::MAX; 13];
    // SAFETY: the size and align functions take no arguments; the offsets
    // function writes 13 `size_t`s into `offsets`.
    let (size, align) = unsafe {
        c::oracle_obs_video_info_offsets(offsets.as_mut_ptr());
        (
            c::oracle_obs_video_info_size(),
            c::oracle_obs_video_info_align(),
        )
    };
    assert_eq!(size_of::<obs_video_info>(), size);
    assert_eq!(align_of::<obs_video_info>(), align);
    assert_eq!(
        offsets,
        [
            offset_of!(obs_video_info, graphics_module),
            offset_of!(obs_video_info, fps_num),
            offset_of!(obs_video_info, fps_den),
            offset_of!(obs_video_info, base_width),
            offset_of!(obs_video_info, base_height),
            offset_of!(obs_video_info, output_width),
            offset_of!(obs_video_info, output_height),
            offset_of!(obs_video_info, output_format),
            offset_of!(obs_video_info, adapter),
            offset_of!(obs_video_info, gpu_conversion),
            offset_of!(obs_video_info, colorspace),
            offset_of!(obs_video_info, range),
            offset_of!(obs_video_info, scale_type),
        ]
    );
}
