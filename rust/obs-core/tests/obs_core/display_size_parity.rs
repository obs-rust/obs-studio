//! Real-header layout, original C differential and concurrent writer coverage.
//! No intended differences. Outputs must be valid pointers. NULL output pointers
//! and dangling displays are undefined C inputs and are excluded.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_core::ffi::display_size::{SizeFields, obs_display_size};
use proptest::prelude::*;
type Getter = unsafe extern "C" fn(*mut c_void, *mut u32, *mut u32);
unsafe extern "C" {
    fn oracle_obs_display_size(display: *mut c_void, width: *mut u32, height: *mut u32);
    fn obs_display_size_fields(display: *mut c_void) -> SizeFields;
    fn oracle_display_size_new(width: u32, height: u32) -> *mut c_void;
    fn oracle_display_size_free(display: *mut c_void);
    fn oracle_display_size_unchanged(display: *mut c_void) -> bool;
    fn oracle_display_size_layout(index: usize) -> usize;
    fn oracle_display_size_concurrent(getter: Getter) -> bool;
}
struct Display(*mut c_void);
impl Drop for Display {
    fn drop(&mut self) {
        // SAFETY: owns the initialized fixture from the matching C allocator.
        unsafe { oracle_display_size_free(self.0) };
    }
}
fn compare(width: u32, height: u32, mode: u8) {
    // SAFETY: both allocations are complete C displays with initialized mutexes.
    let (c, rust) = unsafe {
        (
            Display(oracle_display_size_new(width, height)),
            Display(oracle_display_size_new(width, height)),
        )
    };
    assert!(!c.0.is_null() && !rust.0.is_null());
    let run = |d: &Display, getter: Getter| {
        let (mut w, mut h) = (123, 456);
        // SAFETY: valid output storage may alias; display is exclusively owned.
        unsafe {
            let fields = obs_display_size_fields(d.0);
            match mode {
                0 => getter(d.0, &mut w, &mut h),
                1 => getter(d.0, &mut w, &mut w),
                _ => getter(d.0, fields.cy, &mut h),
            }
            assert!(oracle_display_size_unchanged(d.0));
            (w, h, fields.cx.read(), fields.cy.read())
        }
    };
    assert_eq!(
        run(&c, oracle_obs_display_size),
        run(&rust, obs_display_size)
    );
}
#[test]
fn real_header_layout() {
    for (index, value) in [
        size_of::<SizeFields>(),
        align_of::<SizeFields>(),
        offset_of!(SizeFields, cx),
        offset_of!(SizeFields, cy),
    ]
    .into_iter()
    .enumerate()
    {
        // SAFETY: helper returns constant real-header measurements only.
        assert_eq!(value, unsafe { oracle_display_size_layout(index) });
    }
}
#[test]
fn null_and_boundaries() {
    for getter in [
        oracle_obs_display_size as Getter,
        obs_display_size as Getter,
    ] {
        let (mut w, mut h) = (123, 456);
        // SAFETY: NULL display is defined, with valid output pointers.
        unsafe { getter(core::ptr::null_mut(), &mut w, &mut h) };
        assert_eq!((w, h), (0, 0));
    }
    for width in [0, 1, u32::MAX, 0x8000_0000, 1920] {
        for mode in 0..3 {
            compare(width, !width, mode);
        }
    }
}
#[test]
fn concurrent_writer() {
    // SAFETY: helper owns, joins and frees its thread and C fixture; both
    // getters obey the same pointer and mutex lifetime contracts.
    unsafe {
        assert!(oracle_display_size_concurrent(oracle_obs_display_size));
        assert!(oracle_display_size_concurrent(obs_display_size));
    }
}
proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn dimensions_match(width in any::<u32>(), height in any::<u32>(), mode in 0_u8..3) {
        compare(width, height, mode);
    }
}
