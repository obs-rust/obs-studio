//! Tier 2 layout of the display shim's `#[repr(C)]` types against the real
//! headers, measured by the oracle (oracle/display.c): `struct
//! draw_callback` (`obs-internal.h`) and `struct vec4` (`graphics/vec4.h`),
//! the seam's `sizeof(struct obs_display)`, and the oracle's own config.

use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::display::{self as c, Config};
use obs_core::ffi::display::{draw_callback, vec4};

unsafe extern "C" {
    fn obs_display_seam_size() -> usize;
}

fn layout(index: usize) -> usize {
    // SAFETY: reads a table of constants.
    unsafe { c::oracle_display_layout(index) }
}

#[test]
fn draw_callback_matches_the_header() {
    assert_eq!(size_of::<draw_callback>(), layout(0));
    assert_eq!(align_of::<draw_callback>(), layout(1));
    assert_eq!(offset_of!(draw_callback, draw), layout(2));
    assert_eq!(offset_of!(draw_callback, param), layout(3));
}

#[test]
fn vec4_matches_the_header() {
    assert_eq!(size_of::<vec4>(), layout(4));
    assert_eq!(align_of::<vec4>(), layout(5));
    assert_eq!(offset_of!(vec4, x), layout(6));
    assert_eq!(offset_of!(vec4, y), layout(7));
    assert_eq!(offset_of!(vec4, z), layout(8));
    assert_eq!(offset_of!(vec4, w), layout(9));
}

#[test]
fn the_seam_allocates_a_whole_display() {
    // SAFETY: returns a constant.
    assert_eq!(unsafe { obs_display_seam_size() }, layout(10));
}

#[test]
fn the_oracle_config_matches() {
    assert_eq!(size_of::<Config>(), layout(11));
    assert_eq!(offset_of!(Config, space), layout(12));
    assert_eq!(offset_of!(Config, loops), layout(13));
}
