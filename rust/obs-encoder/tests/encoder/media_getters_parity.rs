//! Tier 3: real private-layout fixtures, untouched original C and the Rust ABI.
//! No intentional behavior differences. Media identities are never dereferenced.
use core::ffi::{c_char, c_int, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_encoder::ffi::media_getters::{Fields, obs_encoder_audio, obs_encoder_parent_video, obs_encoder_video};
use proptest::prelude::*;
use std::ffi::CString;

type Getter = unsafe extern "C" fn(*const c_void) -> *mut c_void;
unsafe extern "C" {
    fn oracle_encoder_media_new(kind: c_int, media: *mut c_void, fps_override: *mut c_void, active: bool, initialized: bool, name: *const c_char) -> *mut c_void;
    fn oracle_encoder_media_free(encoder: *mut c_void);
    fn oracle_encoder_media_unchanged(encoder: *const c_void) -> bool;
    fn oracle_encoder_media_layout(layout: *mut usize);
    fn oracle_encoder_media_probe(encoder: *const c_void, video: Getter, parent: Getter, audio: Getter) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: this guard owns the C allocation, which owns no media tokens.
        unsafe { oracle_encoder_media_free(self.0) }
    }
}

fn check(kind: i32, has_parent: bool, override_kind: u8, active: bool, initialized: bool, name: &[u8]) {
    let mut tokens = [0_u64; 2];
    let parent = if has_parent { (&raw mut tokens[0]).cast() } else { core::ptr::null_mut() };
    let fps_override = match override_kind {
        0 => core::ptr::null_mut(),
        1 => parent,
        _ => (&raw mut tokens[1]).cast(),
    };
    let name = CString::new(name).unwrap();
    // SAFETY: fixture copies the bounded name and borrows live opaque tokens.
    // The isolated process owns the global log callback for both implementations.
    unsafe {
        let fixture = Fixture(oracle_encoder_media_new(kind, parent, fps_override, active, initialized, name.as_ptr()));
        assert!(!fixture.0.is_null());
        assert!(oracle_encoder_media_probe(fixture.0, obs_encoder_video, obs_encoder_parent_video, obs_encoder_audio));
        assert!(oracle_encoder_media_unchanged(fixture.0));
    }
}

#[test]
fn header_derived_layout() {
    let mut c = [0; 10];
    // SAFETY: C writes exactly ten size_t values into this array.
    unsafe { oracle_encoder_media_layout(c.as_mut_ptr()) };
    assert_eq!(c, [size_of::<Fields>(), align_of::<Fields>(), offset_of!(Fields, encoder_type), offset_of!(Fields, media), offset_of!(Fields, fps_override), offset_of!(Fields, name), size_of::<c_int>(), align_of::<c_int>(), 0, 1]);
}

#[test]
fn test_encoder_media_getters_original_c_edges() {
    if !super::isolated(concat!(module_path!(), "::test_encoder_media_getters_original_c_edges")) { return; }
    // SAFETY: NULL must be diagnosed before any private adapter read.
    unsafe { assert!(oracle_encoder_media_probe(core::ptr::null(), obs_encoder_video, obs_encoder_parent_video, obs_encoder_audio)); }
    for kind in [0, 1, 2, i32::MAX] {
        for has_parent in [false, true] {
            for override_kind in 0..3 {
                for active in [false, true] {
                    for initialized in [false, true] {
                        check(kind, has_parent, override_kind, active, initialized, b"media-%s");
                    }
                }
            }
        }
    }
}

#[test]
fn randomized_original_c_parity() {
    if !super::isolated(concat!(module_path!(), "::randomized_original_c_parity")) { return; }
    use proptest::test_runner::{Config, TestRunner};
    let mut runner = TestRunner::new(Config { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..Config::default() });
    let strategy = (prop_oneof![Just(0), Just(1), 2_i32..=i32::MAX], any::<bool>(), 0_u8..3, any::<bool>(), any::<bool>(), proptest::collection::vec(1_u8..=255, 0..64));
    runner.run(&strategy, |(kind, parent, fps_override, active, initialized, name)| {
        check(kind, parent, fps_override, active, initialized, &name);
        Ok(())
    }).unwrap();
}
