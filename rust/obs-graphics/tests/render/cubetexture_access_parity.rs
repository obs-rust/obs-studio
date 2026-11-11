//! Original C parity: no intentional behavior differences, no GPU or owned threads.
use obs_c_oracle as _;
use obs_graphics::ffi::cubetexture_access::{
    gs_cubetexture_destroy, gs_cubetexture_get_color_format, gs_cubetexture_get_size,
};
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_void};
unsafe extern "C" {
    fn oracle_gs_cubetexture_destroy(texture: *mut c_void);
    fn oracle_gs_cubetexture_get_size(texture: *const c_void) -> u32;
    fn oracle_gs_cubetexture_get_color_format(texture: *const c_void) -> u32;
    fn oracle_cta_reset(present: bool, size: u32, format: u32);
    fn oracle_cta_texture() -> *const c_void;
    fn oracle_cta_calls(index: u32) -> u32;
    fn oracle_cta_seen() -> *const c_void;
    fn oracle_cta_logs() -> u32;
    fn oracle_cta_level() -> i32;
    fn oracle_cta_text() -> *const c_char;
    fn oracle_cta_ref() -> i32;
    fn oracle_cta_token() -> u32;
    fn oracle_cta_enum_size() -> usize;
    fn oracle_cta_enum_align() -> usize;
    fn oracle_cta_format(index: u32) -> u32;
    fn oracle_cta_adapter_layout() -> bool;
}
#[derive(Debug, PartialEq, Eq)]
struct Observation {
    value: u32,
    calls: [u32; 3],
    seen: usize,
    logs: u32,
    level: i32,
    text: Vec<u8>,
    references: i32,
    token: u32,
}
unsafe fn observe(value: u32) -> Observation {
    // SAFETY: all fixture data and the log buffer are live and thread-local.
    unsafe {
        Observation {
            value,
            calls: std::array::from_fn(|i| oracle_cta_calls(i as u32)),
            seen: oracle_cta_seen().addr(),
            logs: oracle_cta_logs(),
            level: oracle_cta_level(),
            text: CStr::from_ptr(oracle_cta_text()).to_bytes().to_vec(),
            references: oracle_cta_ref(),
            token: oracle_cta_token(),
        }
    }
}
unsafe fn invoke(rust: bool, operation: usize, texture: *const c_void) -> u32 {
    // SAFETY: the fixture supplies a stable opaque token. Destroy mutates its byte
    // without freeing it; sequences contain at most one destroy, as their last call.
    unsafe {
        match (rust, operation) {
            (false, 0) => oracle_gs_cubetexture_get_size(texture),
            (true, 0) => gs_cubetexture_get_size(texture),
            (false, 1) => {
                oracle_gs_cubetexture_destroy(texture.cast_mut());
                0
            }
            (true, 1) => {
                gs_cubetexture_destroy(texture.cast_mut());
                0
            }
            (false, 2) => oracle_gs_cubetexture_get_color_format(texture),
            (true, 2) => gs_cubetexture_get_color_format(texture),
            _ => unreachable!(),
        }
    }
}
fn compare(present: bool, valid: bool, size: u32, format: u32, getters: &[bool]) {
    let mut sequence: Vec<_> = getters
        .iter()
        .map(|color| if *color { 2 } else { 0 })
        .collect();
    sequence.push(1);
    // SAFETY: each implementation receives a fresh copy of this thread's real-header
    // fixture and the same valid opaque token or NULL. No process globals are changed.
    unsafe {
        oracle_cta_reset(present, size, format);
        let texture = if valid {
            oracle_cta_texture()
        } else {
            std::ptr::null()
        };
        let expected: Vec<_> = sequence
            .iter()
            .map(|&i| observe(invoke(false, i, texture)))
            .collect();
        oracle_cta_reset(present, size, format);
        for (&i, expected) in sequence.iter().zip(expected) {
            let actual = observe(invoke(true, i, texture));
            assert_eq!(actual, expected);
            assert_eq!(actual.references, i32::from(present));
            if !present || !valid {
                assert_eq!(actual.value, 0);
                assert_eq!(actual.calls, [0; 3]);
                assert_eq!(actual.seen, 0);
                assert_eq!(actual.token, 0x5a);
                if present && i == 1 {
                    // A prior rejected getter can leave an old diagnostic; destroy adds none.
                    assert_eq!(actual.logs, getters.len() as u32);
                } else {
                    assert_eq!(actual.level, 400);
                    let reason = if present {
                        "Null 'cubetex' parameter"
                    } else {
                        "called while not in a graphics context"
                    };
                    let name = [
                        "gs_cubetexture_get_size",
                        "gs_cubetexture_destroy",
                        "gs_cubetexture_get_color_format",
                    ][i];
                    assert_eq!(actual.text, format!("{name}: {reason}").as_bytes());
                }
            } else {
                assert_eq!(actual.value, [size, 0, format][i]);
                assert_eq!(actual.seen, texture.addr());
                assert_eq!(actual.logs, 0);
                assert_eq!(actual.token, if i == 1 { 0xa5 } else { 0x5a });
                assert_eq!(actual.calls[1], u32::from(i == 1));
            }
        }
    }
}
#[test]
fn characterization_states_match_c() {
    for present in [false, true] {
        for valid in [false, true] {
            for (size, format) in [(0, 0), (1, 3), (1920, 2), (u32::MAX, 9)] {
                compare(present, valid, size, format, &[false, true, false, true]);
                compare(present, valid, size, format, &[]);
            }
        }
    }
}
#[test]
fn real_header_layout_and_all_formats() {
    // SAFETY: functions only inspect the real header and this thread's fixture.
    unsafe {
        oracle_cta_reset(true, 1, 3);
        assert!(oracle_cta_adapter_layout());
        assert_eq!(oracle_cta_enum_size(), std::mem::size_of::<u32>());
        assert_eq!(oracle_cta_enum_align(), std::mem::align_of::<u32>());
        for format in 0..23 {
            assert_eq!(oracle_cta_format(format), format);
            compare(true, true, u32::MAX, format, &[true, false]);
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence:None, ..ProptestConfig::default() })]
    #[test]
    fn random_dispatch_matches_c(present in any::<bool>(),valid in any::<bool>(),size in any::<u32>(),format in 0u32..23,getters in prop::collection::vec(any::<bool>(),0..48)) {
        compare(present,valid,size,format,&getters);
    }
}
