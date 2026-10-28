//! Layout of the `#[repr(C)]` values the obs-data shim passes through.
//! `obs_data_t`, `obs_data_item_t`, and `obs_data_array_t` are opaque.

use core::ffi::c_int;
use core::mem::{align_of, offset_of, size_of};

use obs_c_oracle as _;
use obs_c_oracle::frame_rate::oracle_media_frames_per_second_align;
use obs_c_oracle::frame_rate::oracle_media_frames_per_second_offset_denominator;
use obs_c_oracle::frame_rate::oracle_media_frames_per_second_offset_numerator;
use obs_c_oracle::frame_rate::oracle_media_frames_per_second_size;
use obs_c_oracle::graphics_math::oracle_quat_align;
use obs_c_oracle::graphics_math::oracle_quat_offset_w;
use obs_c_oracle::graphics_math::oracle_quat_offset_x;
use obs_c_oracle::graphics_math::oracle_quat_offset_y;
use obs_c_oracle::graphics_math::oracle_quat_offset_z;
use obs_c_oracle::graphics_math::oracle_quat_size;
use obs_c_oracle::graphics_math::oracle_vec3_align;
use obs_c_oracle::graphics_math::oracle_vec3_offset_w;
use obs_c_oracle::graphics_math::oracle_vec3_offset_x;
use obs_c_oracle::graphics_math::oracle_vec3_offset_y;
use obs_c_oracle::graphics_math::oracle_vec3_offset_z;
use obs_c_oracle::graphics_math::oracle_vec3_size;
use obs_c_oracle::graphics_math::oracle_vec4_align;
use obs_c_oracle::graphics_math::oracle_vec4_offset_w;
use obs_c_oracle::graphics_math::oracle_vec4_offset_x;
use obs_c_oracle::graphics_math::oracle_vec4_offset_y;
use obs_c_oracle::graphics_math::oracle_vec4_offset_z;
use obs_c_oracle::graphics_math::oracle_vec4_size;
use obs_c_oracle::vec2::oracle_vec2_align;
use obs_c_oracle::vec2::oracle_vec2_offset_x;
use obs_c_oracle::vec2::oracle_vec2_offset_y;
use obs_c_oracle::vec2::oracle_vec2_size;
use obs_data::ffi::obs_data::{media_frames_per_second, quat, vec2, vec3, vec4};
use obs_util as _;

unsafe extern "C" {
    fn oracle_obs_data_type_size() -> usize;
    fn oracle_obs_data_number_type_size() -> usize;
}

#[test]
#[expect(
    clippy::cognitive_complexity,
    reason = "scenario table: one block per ported step"
)]
fn vec_and_frames_layouts_match_c() {
    // SAFETY: the oracle functions return compile-time sizes.
    unsafe {
        assert_eq!(size_of::<vec2>(), oracle_vec2_size());
        assert_eq!(align_of::<vec2>(), oracle_vec2_align());
        assert_eq!(offset_of!(vec2, x), oracle_vec2_offset_x());
        assert_eq!(offset_of!(vec2, y), oracle_vec2_offset_y());

        assert_eq!(size_of::<vec3>(), oracle_vec3_size());
        assert_eq!(align_of::<vec3>(), oracle_vec3_align());
        assert_eq!(offset_of!(vec3, x), oracle_vec3_offset_x());
        assert_eq!(offset_of!(vec3, y), oracle_vec3_offset_y());
        assert_eq!(offset_of!(vec3, z), oracle_vec3_offset_z());
        assert_eq!(offset_of!(vec3, w), oracle_vec3_offset_w());

        assert_eq!(size_of::<vec4>(), oracle_vec4_size());
        assert_eq!(align_of::<vec4>(), oracle_vec4_align());
        assert_eq!(offset_of!(vec4, x), oracle_vec4_offset_x());
        assert_eq!(offset_of!(vec4, y), oracle_vec4_offset_y());
        assert_eq!(offset_of!(vec4, z), oracle_vec4_offset_z());
        assert_eq!(offset_of!(vec4, w), oracle_vec4_offset_w());

        assert_eq!(size_of::<quat>(), oracle_quat_size());
        assert_eq!(align_of::<quat>(), oracle_quat_align());
        assert_eq!(offset_of!(quat, x), oracle_quat_offset_x());
        assert_eq!(offset_of!(quat, y), oracle_quat_offset_y());
        assert_eq!(offset_of!(quat, z), oracle_quat_offset_z());
        assert_eq!(offset_of!(quat, w), oracle_quat_offset_w());

        assert_eq!(
            size_of::<media_frames_per_second>(),
            oracle_media_frames_per_second_size()
        );
        assert_eq!(
            align_of::<media_frames_per_second>(),
            oracle_media_frames_per_second_align()
        );
        assert_eq!(
            offset_of!(media_frames_per_second, numerator),
            oracle_media_frames_per_second_offset_numerator()
        );
        assert_eq!(
            offset_of!(media_frames_per_second, denominator),
            oracle_media_frames_per_second_offset_denominator()
        );

        assert_eq!(oracle_obs_data_type_size(), size_of::<c_int>());
        assert_eq!(oracle_obs_data_number_type_size(), size_of::<c_int>());
    }
}
