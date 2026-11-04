//! Tier 2 layout of the `#[repr(C)]` structs the transition sizing shims
//! (`obs_source_seam::ffi::source_transition_size`) hand libobs:
//! `struct vec2` (to `add_alignment()`), `struct vec3` (to `matrix4_scale()`
//! and `matrix4_translate3v()`) and `struct matrix4` (to those two and to
//! `obs_source_transition_seam_set_matrix()`). Each size, alignment and field
//! offset is checked against the values the oracle measures from the real C
//! headers (`oracle_vec2_*`, `oracle_vec3_*`, `oracle_matrix4_*`).

use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::graphics_math as gm;
use obs_c_oracle::vec2 as v2;
use obs_source_seam::ffi::source_transition_size::{Matrix4, Vec2, Vec3};

#[test]
fn vec2_matches_c() {
    // SAFETY: the oracle's sizeof/offsetof functions take nothing.
    unsafe {
        assert_eq!(size_of::<Vec2>(), v2::oracle_vec2_size());
        assert_eq!(align_of::<Vec2>(), v2::oracle_vec2_align());
        assert_eq!(offset_of!(Vec2, x), v2::oracle_vec2_offset_x());
        assert_eq!(offset_of!(Vec2, y), v2::oracle_vec2_offset_y());
    }
}

#[test]
fn vec3_matches_c() {
    // SAFETY: as above.
    unsafe {
        assert_eq!(size_of::<Vec3>(), gm::oracle_vec3_size());
        assert_eq!(align_of::<Vec3>(), gm::oracle_vec3_align());
        assert_eq!(offset_of!(Vec3, x), gm::oracle_vec3_offset_x());
        assert_eq!(offset_of!(Vec3, y), gm::oracle_vec3_offset_y());
        assert_eq!(offset_of!(Vec3, z), gm::oracle_vec3_offset_z());
        assert_eq!(offset_of!(Vec3, w), gm::oracle_vec3_offset_w());
    }
}

#[test]
fn matrix4_matches_c() {
    // SAFETY: as above.
    unsafe {
        assert_eq!(size_of::<Matrix4>(), gm::oracle_matrix4_size());
        assert_eq!(align_of::<Matrix4>(), gm::oracle_matrix4_align());
        assert_eq!(offset_of!(Matrix4, x), gm::oracle_matrix4_offset_x());
        assert_eq!(offset_of!(Matrix4, y), gm::oracle_matrix4_offset_y());
        assert_eq!(offset_of!(Matrix4, z), gm::oracle_matrix4_offset_z());
        assert_eq!(offset_of!(Matrix4, t), gm::oracle_matrix4_offset_t());
        // each row is a `struct vec4` of four floats
        assert_eq!(size_of::<[f32; 4]>(), gm::oracle_vec4_size());
    }
}

/// The test-side `struct matrix4` the parity tests read the seam's matrices
/// into (`obs_c_oracle::source_transition_size::Matrix4`).
#[test]
fn oracle_matrix4_matches_c() {
    use obs_c_oracle::source_transition_size::Matrix4 as Read;
    // SAFETY: as above.
    unsafe {
        assert_eq!(size_of::<Read>(), gm::oracle_matrix4_size());
        assert_eq!(align_of::<Read>(), gm::oracle_matrix4_align());
    }
}
