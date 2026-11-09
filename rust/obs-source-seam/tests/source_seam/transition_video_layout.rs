//! Tier 2 layout of the `#[repr(C)]` structs the transition video render
//! shims (`obs_source_seam::ffi::source_transition_video`) hand libobs:
//! `struct vec4` (to `gs_clear()`) and `struct matrix4` (to
//! `gs_matrix_mul()` and from `obs_source_transition_seam_get_matrix()`).
//! Both are float arrays in Rust; each size, alignment and the offset of
//! every C field (each float of a `vec4`, each `vec4` row of a `matrix4`)
//! is checked against the values the oracle measures from the real C
//! headers (`oracle_vec4_*`, `oracle_matrix4_*`).

use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::graphics_math as gm;
use obs_source_seam::ffi::source_transition_video::{Matrix4, Vec4};

/// Bytes from the start of the array to float `i`.
const fn float_at(i: usize) -> usize {
    i * size_of::<f32>()
}

#[test]
fn vec4_matches_c() {
    // SAFETY: the oracle's sizeof/offsetof functions take nothing.
    unsafe {
        assert_eq!(size_of::<Vec4>(), gm::oracle_vec4_size());
        assert_eq!(align_of::<Vec4>(), gm::oracle_vec4_align());
        assert_eq!(offset_of!(Vec4, 0), 0);
        assert_eq!(float_at(0), gm::oracle_vec4_offset_x());
        assert_eq!(float_at(1), gm::oracle_vec4_offset_y());
        assert_eq!(float_at(2), gm::oracle_vec4_offset_z());
        assert_eq!(float_at(3), gm::oracle_vec4_offset_w());
    }
}

#[test]
fn matrix4_matches_c() {
    // SAFETY: as above.
    unsafe {
        assert_eq!(size_of::<Matrix4>(), gm::oracle_matrix4_size());
        assert_eq!(align_of::<Matrix4>(), gm::oracle_matrix4_align());
        assert_eq!(offset_of!(Matrix4, 0), 0);
        // the rows x, y, z and t are floats 0, 4, 8 and 12, each a vec4
        assert_eq!(float_at(0), gm::oracle_matrix4_offset_x());
        assert_eq!(float_at(4), gm::oracle_matrix4_offset_y());
        assert_eq!(float_at(8), gm::oracle_matrix4_offset_z());
        assert_eq!(float_at(12), gm::oracle_matrix4_offset_t());
        assert_eq!(float_at(4), gm::oracle_vec4_size());
    }
}
