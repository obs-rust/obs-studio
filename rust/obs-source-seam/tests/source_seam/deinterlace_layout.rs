//! Tier 2 layout of the `#[repr(C)]` types the deinterlace shims
//! (`obs_source_seam::ffi::source_deinterlace`) share with libobs:
//! `struct vec2` (to `gs_effect_set_vec2()`) and `struct
//! obs_source_deinterlace_fields` (from `obs_source_deinterlace_fields()`),
//! with the widths of the two fields it points at. Each size, alignment and
//! field offset is checked against the values the oracle measures from the
//! real C headers (`oracle_deint_*`).

use std::ffi::c_int;
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::source_deinterlace as c;
use obs_source_seam::ffi::source_deinterlace::{DeinterlaceFields, Vec2};

#[test]
fn vec2_matches_c() {
    // SAFETY: the oracle's sizeof/offsetof functions take nothing.
    unsafe {
        assert_eq!(size_of::<Vec2>(), c::oracle_deint_vec2_size());
        assert_eq!(align_of::<Vec2>(), c::oracle_deint_vec2_align());
        assert_eq!(offset_of!(Vec2, 0), c::oracle_deint_vec2_offset_x());
        assert_eq!(size_of::<f32>(), c::oracle_deint_vec2_offset_y());
    }
}

#[test]
fn deinterlace_fields_match_c() {
    // SAFETY: as above.
    unsafe {
        assert_eq!(
            size_of::<DeinterlaceFields>(),
            c::oracle_deint_fields_size()
        );
        assert_eq!(
            align_of::<DeinterlaceFields>(),
            c::oracle_deint_fields_align()
        );
        assert_eq!(
            offset_of!(DeinterlaceFields, mode),
            c::oracle_deint_fields_offset_mode()
        );
        assert_eq!(
            offset_of!(DeinterlaceFields, top_first),
            c::oracle_deint_fields_offset_top_first()
        );
        // what each pointer reads: the mode as an int, the flag as a bool
        assert_eq!(size_of::<c_int>(), c::oracle_deint_mode_size());
        assert_eq!(size_of::<bool>(), c::oracle_deint_top_first_size());
    }
}
