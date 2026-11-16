//! Original C creators, real layout adapter and Rust ABI over identical fake devices.
use obs_c_oracle as _;
use obs_graphics::ffi::texture_creators as r;
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_void};

unsafe extern "C" {
    fn oracle_texture_creators_reset(present: bool, fail: bool);
    fn oracle_texture_creators_finish();
    fn oracle_texture_creators_log() -> *const c_char;
    fn oracle_gs_texture_create(
        w: u32,
        h: u32,
        f: i32,
        l: u32,
        d: *const *const u8,
        flags: u32,
    ) -> *mut c_void;
    fn oracle_gs_cubetexture_create(
        s: u32,
        f: i32,
        l: u32,
        d: *const *const u8,
        flags: u32,
    ) -> *mut c_void;
}

#[allow(clippy::too_many_arguments)]
fn compare(cube: bool, w: u32, h: u32, l: u32, flags: u32, data: bool, present: bool, fail: bool) {
    let byte = 7_u8;
    let p = &byte as *const u8;
    let d = if data {
        &p as *const *const u8
    } else {
        std::ptr::null()
    };
    // SAFETY: thread-local fake context; data is live and the fake never reads it.
    unsafe {
        oracle_texture_creators_reset(present, fail);
        let want = if cube {
            oracle_gs_cubetexture_create(w, 3, l, d, flags)
        } else {
            oracle_gs_texture_create(w, h, 3, l, d, flags)
        };
        let log = CStr::from_ptr(oracle_texture_creators_log()).to_owned();
        oracle_texture_creators_reset(present, fail);
        let got = if cube {
            r::gs_cubetexture_create(w, 3, l, d, flags)
        } else {
            r::gs_texture_create(w, h, 3, l, d, flags)
        };
        let actual = CStr::from_ptr(oracle_texture_creators_log()).to_owned();
        oracle_texture_creators_finish();
        assert_eq!(got, want);
        assert_eq!(actual, log);
    }
}

#[test]
fn creator_edges_and_no_context() {
    for cube in [false, true] {
        for w in [0, 1, 2, 64, 100, 1 << 31, u32::MAX] {
            for h in [0, 1, 32, 100, u32::MAX] {
                for l in [0, 1, 7, u32::MAX] {
                    for flags in [0, 1, 2, 4, 5, 6, 7, u32::MAX] {
                        for data in [false, true] {
                            compare(cube, w, h, l, flags, data, true, false);
                            compare(cube, w, h, l, flags, data, false, false);
                            compare(cube, w, h, l, flags, data, true, true);
                        }
                    }
                }
            }
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig { cases:obs_c_oracle::parity::cases(256), failure_persistence:None, ..ProptestConfig::default() })]
    #[test]
    fn creator_random(cube in any::<bool>(),w in any::<u32>(),h in any::<u32>(),l in any::<u32>(),f in any::<u32>(),d in any::<bool>(),p in any::<bool>(),fail in any::<bool>()) {
        compare(cube,w,h,l,f,d,p,fail);
    }
}
