//! No intentional differences from the extracted C texture getters.
use obs_c_oracle as _;
use obs_graphics::ffi::voltexture_metadata::{
    gs_voltexture_get_color_format, gs_voltexture_get_depth, gs_voltexture_get_height,
    gs_voltexture_get_width,
};
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_void};
use std::mem::{align_of, size_of};
unsafe extern "C" {
    fn oracle_gs_voltexture_get_width(texture: *const c_void) -> u32;
    fn oracle_gs_voltexture_get_depth(texture: *const c_void) -> u32;
    fn oracle_gs_voltexture_get_height(texture: *const c_void) -> u32;
    fn oracle_gs_voltexture_get_color_format(texture: *const c_void) -> u32;
    fn oracle_vtm_reset(present: bool, width: u32, height: u32, depth: u32, format: u32);
    fn oracle_vtm_texture() -> *const c_void;
    fn oracle_vtm_calls(index: u32) -> u32;
    fn oracle_vtm_seen() -> *const c_void;
    fn oracle_vtm_logs() -> u32;
    fn oracle_vtm_level() -> i32;
    fn oracle_vtm_text() -> *const c_char;
    fn oracle_vtm_ref() -> i32;
    fn oracle_vtm_enum_size() -> usize;
    fn oracle_vtm_enum_align() -> usize;
    fn oracle_vtm_format(index: u32) -> u32;
    fn oracle_vtm_adapter_layout() -> bool;
    fn obs_voltexture_metadata_context() -> *mut c_void;
}
#[test]
fn real_header_adapter_and_enum_layout() {
    // SAFETY: the oracle owns this thread's initialized fixture; no GPU is used.
    unsafe {
        oracle_vtm_reset(true, 1, 2, 4, 3);
        assert_eq!(oracle_vtm_enum_size(), size_of::<u32>());
        assert_eq!(oracle_vtm_enum_align(), align_of::<u32>());
        assert_eq!(
            (0..23).map(|i| oracle_vtm_format(i)).collect::<Vec<_>>(),
            (0..23).collect::<Vec<_>>()
        );
        assert!(oracle_vtm_adapter_layout());
        assert_eq!(
            obs_voltexture_metadata_context().cast_const(),
            oracle_vtm_texture()
        );
        assert_eq!(oracle_vtm_ref(), 1);
    }
}
#[derive(Debug, PartialEq, Eq)]
struct Observation {
    value: u32,
    calls: [u32; 4],
    seen: usize,
    logs: u32,
    level: i32,
    text: Vec<u8>,
    context: usize,
    references: i32,
}
unsafe fn observe(value: u32) -> Observation {
    // SAFETY: all pointers are the live thread-local fixture or its log buffer.
    unsafe {
        Observation {
            value,
            calls: [
                oracle_vtm_calls(0),
                oracle_vtm_calls(1),
                oracle_vtm_calls(2),
                oracle_vtm_calls(3),
            ],
            seen: oracle_vtm_seen().addr(),
            logs: oracle_vtm_logs(),
            level: oracle_vtm_level(),
            text: CStr::from_ptr(oracle_vtm_text()).to_bytes().to_vec(),
            context: obs_voltexture_metadata_context().addr(),
            references: oracle_vtm_ref(),
        }
    }
}
fn compare(
    present: bool,
    valid: bool,
    width: u32,
    height: u32,
    depth: u32,
    format: u32,
    sequence: &[usize],
) {
    let original = [
        oracle_gs_voltexture_get_width,
        oracle_gs_voltexture_get_height,
        oracle_gs_voltexture_get_depth,
        oracle_gs_voltexture_get_color_format,
    ];
    let rust = [
        gs_voltexture_get_width,
        gs_voltexture_get_height,
        gs_voltexture_get_depth,
        gs_voltexture_get_color_format,
    ];
    // SAFETY: both implementations receive the same initialized, thread-local
    // real-header fixture and an opaque token they never dereference.
    unsafe {
        oracle_vtm_reset(present, width, height, depth, format);
        let texture = if valid {
            oracle_vtm_texture()
        } else {
            std::ptr::null()
        };
        let expected: Vec<_> = sequence
            .iter()
            .map(|&i| observe(original[i](texture)))
            .collect();
        oracle_vtm_reset(present, width, height, depth, format);
        for (&i, expected) in sequence.iter().zip(expected) {
            let actual = observe(rust[i](texture));
            assert_eq!(actual, expected);
            if !present || !valid {
                assert_eq!(actual.value, 0);
                assert_eq!(actual.calls, [0; 4]);
                assert_eq!(actual.seen, 0);
                assert_eq!(actual.level, 400);
                let reason = if present {
                    "Null 'voltex' parameter"
                } else {
                    "called while not in a graphics context"
                };
                let names = [
                    "gs_voltexture_get_width",
                    "gs_voltexture_get_height",
                    "gs_voltexture_get_depth",
                    "gs_voltexture_get_color_format",
                ];
                assert_eq!(actual.text, format!("{}: {reason}", names[i]).as_bytes());
            } else {
                assert_eq!(actual.value, [width, height, depth, format][i]);
                assert_eq!(actual.seen, texture.addr());
                assert_eq!(actual.logs, 0);
            }
        }
    }
}
#[test]
fn characterization_states_match_c() {
    for present in [false, true] {
        for valid in [false, true] {
            for (i, width) in [0, 1, 1920, u32::MAX].into_iter().enumerate() {
                compare(
                    present,
                    valid,
                    width,
                    [u32::MAX, 1920, 1, 0][i],
                    [1, 1920, u32::MAX, 0][i],
                    [0, 3, 9, 2][i],
                    &[0, 1, 2, 3, 0, 1, 2, 3],
                );
            }
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn random_dispatch_matches_c(present in any::<bool>(), valid in any::<bool>(), width in any::<u32>(), height in any::<u32>(), depth in any::<u32>(), format in 0u32..23, sequence in prop::collection::vec(0usize..4, 0..48)) {
        compare(present, valid, width, height, depth, format, &sequence);
    }
}

#[test]
fn all_formats_match_c() {
    for format in 0..23 {
        compare(true, true, 0, u32::MAX, 1, format, &[3, 3, 0, 1, 2]);
    }
}
