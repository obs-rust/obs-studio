//! Tier 3: real-header layout and the unchanged extracted C functions.
//! No intentional differences. NULL is defined; non-NULL dangling pointers
//! and concurrent unsynchronized field access are invalid C inputs, excluded.
//! All colors and enabled values are included. Full C objects are compared
//! after every operation, including fields outside the Rust prefix.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_core::ffi::display_state::{self as shim, DisplayStatePrefix};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_display_state_layout(index: usize) -> usize;
    fn oracle_display_state_new(enabled: bool, color: u32, seed: u32) -> *mut c_void;
    fn oracle_display_state_equal(left: *const c_void, right: *const c_void) -> bool;
    fn oracle_display_state_free(display: *mut c_void);
    fn oracle_obs_display_enabled(display: *mut c_void) -> bool;
    fn oracle_obs_display_set_enabled(display: *mut c_void, enabled: bool);
    fn oracle_obs_display_set_background_color(display: *mut c_void, color: u32);
}

struct Display(*mut c_void);
impl Drop for Display {
    fn drop(&mut self) {
        // SAFETY: this object was allocated by the matching oracle helper.
        unsafe { oracle_display_state_free(self.0) };
    }
}

#[test]
fn real_header_prefix_layout() {
    let rust = [
        size_of::<DisplayStatePrefix>(),
        align_of::<DisplayStatePrefix>(),
        offset_of!(DisplayStatePrefix, update_color_space),
        offset_of!(DisplayStatePrefix, enabled),
        offset_of!(DisplayStatePrefix, cx),
        offset_of!(DisplayStatePrefix, cy),
        offset_of!(DisplayStatePrefix, next_cx),
        offset_of!(DisplayStatePrefix, next_cy),
        offset_of!(DisplayStatePrefix, background_color),
        size_of::<bool>(),
        size_of::<u32>(),
    ];
    for (index, value) in rust.into_iter().enumerate() {
        // SAFETY: the helper only returns a real-header measurement.
        assert_eq!(
            value,
            unsafe { oracle_display_state_layout(index) },
            "layout {index}"
        );
    }
}

fn compare(initial: bool, color: u32, seed: u32, ops: &[(bool, u32)]) {
    // SAFETY: matching oracle allocations are owned until the guards drop.
    let (c, rust) = unsafe {
        (
            Display(oracle_display_state_new(initial, color, seed)),
            Display(oracle_display_state_new(initial, color, seed)),
        )
    };
    assert!(!c.0.is_null() && !rust.0.is_null());
    // SAFETY: both pointers are live complete C displays. Only this test owns
    // them, and the Rust shim accesses their measured scalar prefixes.
    unsafe {
        assert_eq!(
            oracle_obs_display_enabled(c.0),
            shim::obs_display_enabled(rust.0.cast())
        );
        for &(enable, color) in ops {
            oracle_obs_display_set_enabled(c.0, enable);
            shim::obs_display_set_enabled(rust.0.cast(), enable);
            assert_eq!(
                oracle_obs_display_enabled(c.0),
                shim::obs_display_enabled(rust.0.cast())
            );
            assert!(oracle_display_state_equal(c.0, rust.0));
            oracle_obs_display_set_background_color(c.0, color);
            shim::obs_display_set_background_color(rust.0.cast(), color);
            assert!(oracle_display_state_equal(c.0, rust.0));
        }
    }
}

#[test]
fn null_and_boundaries() {
    // SAFETY: NULL inputs are explicitly defined by these three C functions.
    unsafe {
        assert_eq!(
            oracle_obs_display_enabled(core::ptr::null_mut()),
            shim::obs_display_enabled(core::ptr::null_mut())
        );
        for enable in [false, true] {
            oracle_obs_display_set_enabled(core::ptr::null_mut(), enable);
            shim::obs_display_set_enabled(core::ptr::null_mut(), enable);
        }
        oracle_obs_display_set_background_color(core::ptr::null_mut(), u32::MAX);
        shim::obs_display_set_background_color(core::ptr::null_mut(), u32::MAX);
    }
    for initial in [false, true] {
        for color in [
            0,
            u32::MAX,
            0x8000_0000,
            0x0102_0304,
            0xff00_0000,
            0x00ff_ffff,
        ] {
            compare(
                initial,
                color,
                !color,
                &[(false, 0), (true, u32::MAX), (true, color), (false, color)],
            );
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]
    #[test]
    fn arbitrary_state_sequences(initial in any::<bool>(), color in any::<u32>(), seed in any::<u32>(), ops in prop::collection::vec((any::<bool>(), any::<u32>()), 0..65)) {
        compare(initial, color, seed, &ops);
    }
}
