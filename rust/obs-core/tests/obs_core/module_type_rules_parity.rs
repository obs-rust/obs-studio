//! Tier 3: the output, encoder and service registration rule shims against
//! the original C.
//!
//! The oracle is `libobs/obs-module-type-rules.c.h`, unmodified, with its
//! entry points renamed to `oracle_*` (`obs-c-oracle/oracle/
//! module_type_rules.c`); its `strcmp()` is the C library's. The same file
//! gives the constants of `obs-module-type-rules.h`, `obs-output.h` and
//! `obs-encoder.h`, and the field ends and structure sizes as the C sees
//! them. Compared, the C enum codes as they are returned:
//!
//! - the constants, against the Rust ones and the shims' codes;
//! - `obs_module_type_rules_output_check()` exhaustively: every subset of the
//!   five flags the rules read, with flag bits they do not read, × every set
//!   of the ten output callbacks, with and without the other callback bits;
//! - `_encoder_check()` exhaustively: audio, video and other types × every
//!   subset of the seven capabilities, and all bits, × every set of the
//!   seven encoder callbacks, with and without the other bits;
//! - `_service_check()` over every callback mask;
//! - `_protocol_listed()` over fixed lists (empty, NULL, duplicates, case,
//!   spaces, bytes outside ASCII) and random ones;
//! - the whole decision of the three registrations, each side's functions in
//!   the C's order (the callbacks `HAS_VAL()` finds inside the size from the
//!   C's own field ends, the rules, then slice 4's size limit), at every size
//!   from 0 to past the structure, for every flag set and present mask the
//!   rules tell apart;
//! - proptests over any flags, capabilities, type, callback bits, size and
//!   present mask, and any protocols.
//!
//! Intentional differences, where the C's `strcmp()` would read a NULL
//! pointer and `obs-module.c` passes none: a NULL protocol is listed
//! nowhere, a NULL list is empty and a NULL entry matches nothing. They are
//! not compared with the C; `protocol_listed_null_pointers` pins them.

use core::ffi::c_char;
use core::ptr;
use std::ffi::CString;

use obs_c_oracle::module_source_rules::oracle_obs_module_source_rules_size_supported;
use obs_c_oracle::module_type_rules::{
    CONSTANT_NAMES, oracle_module_type_rules_constant, oracle_module_type_rules_constant_count,
    oracle_module_type_rules_field_end, oracle_module_type_rules_other_end,
    oracle_module_type_rules_struct_size, oracle_obs_module_type_rules_encoder_check,
    oracle_obs_module_type_rules_output_check, oracle_obs_module_type_rules_protocol_listed,
    oracle_obs_module_type_rules_service_check,
};
use obs_core::ffi::module_source_rules::obs_module_source_rules_size_supported;
use obs_core::ffi::module_type_rules::{
    NO_CREATE, NO_DESTROY, NO_ENCODE, NO_ENCODE_TEXTURE, NO_ENCODED_PACKET, NO_GET_FRAME_SIZE,
    NO_GET_NAME, NO_GET_PROTOCOL, NO_PROTOCOLS, NO_RAW_AUDIO, NO_RAW_AUDIO2, NO_RAW_VIDEO,
    NO_START, NO_STOP, OK, TEXTURE_SCALING, obs_module_type_rules_encoder_check,
    obs_module_type_rules_output_check, obs_module_type_rules_protocol_listed,
    obs_module_type_rules_service_check,
};
use obs_core::module_type_rules::{
    CAP_PASS_TEXTURE, CAP_SCALING, ENCODER_AUDIO, ENCODER_VIDEO, HAS_CREATE, HAS_DESTROY,
    HAS_ENCODE, HAS_ENCODE_TEXTURE, HAS_ENCODE_TEXTURE2, HAS_ENCODED_PACKET, HAS_GET_FRAME_SIZE,
    HAS_GET_NAME, HAS_GET_PROTOCOL, HAS_PROTOCOLS, HAS_RAW_AUDIO, HAS_RAW_AUDIO2, HAS_RAW_VIDEO,
    HAS_START, HAS_STOP, OUTPUT_AUDIO, OUTPUT_ENCODED, OUTPUT_MULTI_TRACK, OUTPUT_SERVICE,
    OUTPUT_VIDEO,
};
use proptest::prelude::*;

const OUTPUT: u32 = 0;
const ENCODER: u32 = 1;
const SERVICE: u32 = 2;

/// The callback bits each kind reads, in bit order.
const OUTPUT_BITS: [u32; 10] = [
    HAS_GET_NAME,
    HAS_CREATE,
    HAS_DESTROY,
    HAS_START,
    HAS_STOP,
    HAS_PROTOCOLS,
    HAS_ENCODED_PACKET,
    HAS_RAW_VIDEO,
    HAS_RAW_AUDIO,
    HAS_RAW_AUDIO2,
];
const ENCODER_BITS: [u32; 7] = [
    HAS_GET_NAME,
    HAS_CREATE,
    HAS_DESTROY,
    HAS_ENCODE,
    HAS_ENCODE_TEXTURE,
    HAS_ENCODE_TEXTURE2,
    HAS_GET_FRAME_SIZE,
];
const SERVICE_BITS: [u32; 4] = [HAS_GET_NAME, HAS_CREATE, HAS_DESTROY, HAS_GET_PROTOCOL];

const ALL_CBS: u32 = (1 << 15) - 1;

/// The output flags the rules read, and bits they do not.
const OUTPUT_FLAGS: [u32; 5] = [
    OUTPUT_VIDEO,
    OUTPUT_AUDIO,
    OUTPUT_ENCODED,
    OUTPUT_SERVICE,
    OUTPUT_MULTI_TRACK,
];
const OTHER_OUTPUT_FLAGS: [u32; 6] = [0, 1 << 5, 1 << 6, 1 << 7, 1 << 31, !0x1f];

const ENCODER_TYPES: [u32; 6] = [
    ENCODER_AUDIO,
    ENCODER_VIDEO,
    2,
    0x100,
    0x8000_0000,
    u32::MAX,
];

/// Every subset of `bits`, as masks.
fn subsets(bits: &[u32]) -> Vec<u32> {
    (0u32..1 << bits.len())
        .map(|n| {
            bits.iter()
                .enumerate()
                .filter(|(i, _)| n & (1 << i) != 0)
                .fold(0, |a, (_, b)| a | b)
        })
        .collect()
}

fn output_both(flags: u32, cbs: u32) -> (u32, u32) {
    // SAFETY: plain values.
    let c = unsafe { oracle_obs_module_type_rules_output_check(flags, cbs) };
    (obs_module_type_rules_output_check(flags, cbs), c)
}

fn encoder_both(t: u32, caps: u32, cbs: u32) -> (u32, u32) {
    // SAFETY: plain values.
    let c = unsafe { oracle_obs_module_type_rules_encoder_check(t, caps, cbs) };
    (obs_module_type_rules_encoder_check(t, caps, cbs), c)
}

fn service_both(cbs: u32) -> (u32, u32) {
    // SAFETY: plain values.
    let c = unsafe { oracle_obs_module_type_rules_service_check(cbs) };
    (obs_module_type_rules_service_check(cbs), c)
}

/// Both sides over a protocol and a list of C strings.
fn listed_both(protocol: &[u8], list: &[Vec<u8>]) -> (bool, bool) {
    let protocol = CString::new(protocol).unwrap();
    let owned: Vec<CString> = list
        .iter()
        .map(|p| CString::new(p.clone()).unwrap())
        .collect();
    let ptrs: Vec<*mut c_char> = owned.iter().map(|p| p.as_ptr().cast_mut()).collect();
    let array = if ptrs.is_empty() {
        ptr::null()
    } else {
        ptrs.as_ptr()
    };
    // SAFETY: NUL-terminated strings, `ptrs.len()` entries (or NULL with 0).
    unsafe {
        (
            obs_module_type_rules_protocol_listed(protocol.as_ptr(), array, ptrs.len()),
            oracle_obs_module_type_rules_protocol_listed(protocol.as_ptr(), array, ptrs.len()),
        )
    }
}

#[test]
fn constants_match_c() {
    // SAFETY: plain values.
    let count = unsafe { oracle_module_type_rules_constant_count() };
    assert_eq!(count, CONSTANT_NAMES.len());
    let rust: [u32; 49] = [
        OUTPUT_VIDEO,
        OUTPUT_AUDIO,
        OUTPUT_ENCODED,
        OUTPUT_SERVICE,
        OUTPUT_MULTI_TRACK,
        ENCODER_AUDIO,
        CAP_PASS_TEXTURE,
        CAP_SCALING,
        OUTPUT_VIDEO,
        OUTPUT_AUDIO,
        OUTPUT_ENCODED,
        OUTPUT_SERVICE,
        OUTPUT_MULTI_TRACK,
        ENCODER_AUDIO,
        ENCODER_VIDEO,
        CAP_PASS_TEXTURE,
        CAP_SCALING,
        HAS_GET_NAME,
        HAS_CREATE,
        HAS_DESTROY,
        HAS_START,
        HAS_STOP,
        HAS_PROTOCOLS,
        HAS_ENCODED_PACKET,
        HAS_RAW_VIDEO,
        HAS_RAW_AUDIO,
        HAS_RAW_AUDIO2,
        HAS_ENCODE,
        HAS_ENCODE_TEXTURE,
        HAS_ENCODE_TEXTURE2,
        HAS_GET_FRAME_SIZE,
        HAS_GET_PROTOCOL,
        OK,
        TEXTURE_SCALING,
        NO_GET_NAME,
        NO_CREATE,
        NO_DESTROY,
        NO_START,
        NO_STOP,
        NO_PROTOCOLS,
        NO_ENCODED_PACKET,
        NO_RAW_VIDEO,
        NO_RAW_AUDIO,
        NO_RAW_AUDIO2,
        NO_ENCODE_TEXTURE,
        NO_ENCODE,
        NO_GET_FRAME_SIZE,
        NO_GET_PROTOCOL,
        4,
    ];
    for (i, (name, value)) in CONSTANT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: plain values.
        assert_eq!(
            unsafe { oracle_module_type_rules_constant(i) },
            value,
            "{name}"
        );
    }
}

#[test]
fn output_check_matches_c_exhaustively() {
    let foreign = !(OUTPUT_BITS.iter().fold(0, |a, b| a | b));
    for flags in subsets(&OUTPUT_FLAGS) {
        for other in OTHER_OUTPUT_FLAGS {
            for cbs in subsets(&OUTPUT_BITS) {
                for extra in [0, foreign] {
                    let (rust, c) = output_both(flags | other, cbs | extra);
                    assert_eq!(rust, c, "flags {:#x} cbs {:#x}", flags | other, cbs | extra);
                }
            }
        }
    }
}

#[test]
fn encoder_check_matches_c_exhaustively() {
    let foreign = !(ENCODER_BITS.iter().fold(0, |a, b| a | b));
    let mut caps_sets = subsets(&[1 << 0, 1 << 1, 1 << 2, 1 << 3, 1 << 4, 1 << 5, 1 << 6]);
    caps_sets.extend([u32::MAX, 1 << 7, 0xFFFF_FF00 | CAP_PASS_TEXTURE]);
    for t in ENCODER_TYPES {
        for &caps in &caps_sets {
            for cbs in subsets(&ENCODER_BITS) {
                for extra in [0, foreign] {
                    let (rust, c) = encoder_both(t, caps, cbs | extra);
                    assert_eq!(rust, c, "type {t} caps {caps:#x} cbs {:#x}", cbs | extra);
                }
            }
        }
    }
}

#[test]
fn service_check_matches_c_over_every_mask() {
    for cbs in 0..=ALL_CBS {
        let (rust, c) = service_both(cbs);
        assert_eq!(rust, c, "cbs {cbs:#x}");
        let (rust, c) = service_both(cbs | 0xFFFF_8000);
        assert_eq!(rust, c, "cbs {cbs:#x} high");
    }
}

#[test]
fn protocol_listed_matches_c() {
    let list: Vec<Vec<u8>> = [
        "RTMP",
        "RTMPS",
        "SRT",
        "",
        "rtmp",
        " RIST",
        "RTMP",
        "WHIP;RIST",
        "\u{e9}t\u{e9}",
    ]
    .iter()
    .map(|s| s.as_bytes().to_vec())
    .collect();
    let probes = [
        "RTMP",
        "RTMPS",
        "SRT",
        "",
        "rtmp",
        " RIST",
        "RIST",
        "RTM",
        "RTMPSX",
        "Rtmp",
        "SRT ",
        "WHIP",
        "WHIP;RIST",
        "\u{e9}t\u{e9}",
        "\u{c9}T\u{c9}",
    ];
    for probe in probes {
        for n in 0..=list.len() {
            let (rust, c) = listed_both(probe.as_bytes(), &list[..n]);
            assert_eq!(rust, c, "{probe:?} in {n}");
            let (rust, c) = listed_both(probe.as_bytes(), &list[list.len() - n..]);
            assert_eq!(rust, c, "{probe:?} in last {n}");
        }
    }
}

/// The callbacks `HAS_VAL()` finds: present, and whole inside the size by
/// the C's own field ends.
fn has_val(kind: u32, present: u32, size: usize) -> u32 {
    let mut bits = 0;
    for bit in 0..15u32 {
        // SAFETY: plain values.
        let end = unsafe { oracle_module_type_rules_field_end(kind, bit) };
        if end != 0 && present & (1 << bit) != 0 && end <= size {
            bits |= 1 << bit;
        }
    }
    bits
}

/// What a registration decides after the duplicate lookup: the rule
/// broken, then (when none is) whether the size is supported.
#[derive(Debug, PartialEq, Eq)]
struct Decision {
    code: u32,
    size_supported: Option<bool>,
}

struct Fns {
    output: fn(u32, u32) -> u32,
    encoder: fn(u32, u32, u32) -> u32,
    service: fn(u32) -> u32,
    size_supported: fn(usize, usize) -> bool,
}

fn rust_fns() -> Fns {
    Fns {
        output: |f, cb| obs_module_type_rules_output_check(f, cb),
        encoder: |t, caps, cb| obs_module_type_rules_encoder_check(t, caps, cb),
        service: |cb| obs_module_type_rules_service_check(cb),
        size_supported: |size, full| obs_module_source_rules_size_supported(size, full),
    }
}

fn c_fns() -> Fns {
    // SAFETY (each): plain values.
    Fns {
        output: |f, cb| unsafe { oracle_obs_module_type_rules_output_check(f, cb) },
        encoder: |t, caps, cb| unsafe { oracle_obs_module_type_rules_encoder_check(t, caps, cb) },
        service: |cb| unsafe { oracle_obs_module_type_rules_service_check(cb) },
        size_supported: |size, full| unsafe {
            oracle_obs_module_source_rules_size_supported(size, full)
        },
    }
}

/// The decision of `obs_register_<kind>_s()` for a structure with the
/// `present` callbacks, cut at `size`. The flags, capabilities and type are
/// the caller's, as the C reads them whatever the size.
fn decide(
    side: &Fns,
    kind: u32,
    flags: u32,
    encoder_type: u32,
    present: u32,
    size: usize,
) -> Decision {
    let cbs = has_val(kind, present, size);
    let code = match kind {
        OUTPUT => (side.output)(flags, cbs),
        ENCODER => (side.encoder)(encoder_type, flags, cbs),
        _ => (side.service)(cbs),
    };
    let size_supported = (code == OK).then(|| {
        // SAFETY: plain values.
        let full = unsafe { oracle_module_type_rules_struct_size(kind) };
        (side.size_supported)(size, full)
    });
    Decision {
        code,
        size_supported,
    }
}

fn decide_both(kind: u32, flags: u32, t: u32, present: u32, size: usize) {
    let rust = decide(&rust_fns(), kind, flags, t, present, size);
    let c = decide(&c_fns(), kind, flags, t, present, size);
    assert_eq!(
        rust, c,
        "kind {kind} flags {flags:#x} type {t} present {present:#x} size {size}"
    );
}

/// The present masks the rules tell apart: all, none, each one missing.
fn masks(bits: &[u32]) -> Vec<u32> {
    let all = bits.iter().fold(0, |a, b| a | b);
    let mut out = vec![all, 0];
    out.extend(bits.iter().map(|b| all & !b));
    out
}

#[test]
fn registration_matches_c_at_every_size() {
    for kind in [OUTPUT, ENCODER, SERVICE] {
        // SAFETY: plain values.
        let full = unsafe { oracle_module_type_rules_struct_size(kind) };
        for size in 0..=full + 16 {
            match kind {
                OUTPUT => {
                    for flags in subsets(&OUTPUT_FLAGS) {
                        for present in masks(&OUTPUT_BITS) {
                            decide_both(kind, flags, 0, present, size);
                        }
                    }
                }
                ENCODER => {
                    for t in [ENCODER_AUDIO, ENCODER_VIDEO, 2] {
                        for caps in subsets(&[CAP_PASS_TEXTURE, CAP_SCALING, 1 << 2]) {
                            for present in subsets(&ENCODER_BITS) {
                                decide_both(kind, caps, t, present, size);
                            }
                        }
                    }
                }
                _ => {
                    for present in subsets(&SERVICE_BITS) {
                        decide_both(kind, 0, 0, present, size);
                    }
                }
            }
        }
    }
}

#[test]
fn field_ends_are_inside_each_structure() {
    for (kind, bits) in [
        (OUTPUT, &OUTPUT_BITS[..]),
        (ENCODER, &ENCODER_BITS[..]),
        (SERVICE, &SERVICE_BITS[..]),
    ] {
        // SAFETY: plain values.
        let full = unsafe { oracle_module_type_rules_struct_size(kind) };
        for bit in 0..15u32 {
            // SAFETY: plain values.
            let end = unsafe { oracle_module_type_rules_field_end(kind, bit) };
            assert_eq!(
                end != 0,
                bits.contains(&(1 << bit)),
                "kind {kind} bit {bit}"
            );
            assert!(end <= full);
        }
    }
    for which in 0..9 {
        // SAFETY: plain values.
        assert_ne!(unsafe { oracle_module_type_rules_other_end(which) }, 0);
    }
}

/// Every field end the registration reads, with the sizes around it.
fn interesting_sizes() -> Vec<usize> {
    let mut sizes = vec![0, 1];
    for kind in [OUTPUT, ENCODER, SERVICE] {
        for bit in 0..15u32 {
            // SAFETY: plain values.
            let end = unsafe { oracle_module_type_rules_field_end(kind, bit) };
            if end != 0 {
                sizes.extend([end - 1, end, end + 1]);
            }
        }
        // SAFETY: plain values.
        let full = unsafe { oracle_module_type_rules_struct_size(kind) };
        sizes.extend([full - 1, full, full + 1]);
    }
    for which in 0..9 {
        // SAFETY: plain values.
        let end = unsafe { oracle_module_type_rules_other_end(which) };
        sizes.extend([end - 1, end, end + 1]);
    }
    sizes.sort_unstable();
    sizes.dedup();
    sizes
}

fn any_size() -> impl Strategy<Value = usize> {
    prop_oneof![
        proptest::sample::select(interesting_sizes()),
        0usize..300,
        any::<usize>(),
    ]
}

fn any_flags() -> impl Strategy<Value = u32> {
    prop_oneof![0u32..0x80, any::<u32>()]
}

fn any_bytes() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        proptest::collection::vec(proptest::sample::select(b"RTMPSrtmps; ".to_vec()), 0..5),
        proptest::collection::vec(1u8.., 0..8),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn output_check_matches_c_for_any_input(flags in any::<u32>(), cbs in any::<u32>()) {
        let (rust, c) = output_both(flags, cbs);
        prop_assert_eq!(rust, c);
    }

    #[test]
    fn encoder_check_matches_c_for_any_input(t in any::<u32>(), caps in any::<u32>(), cbs in any::<u32>()) {
        let (rust, c) = encoder_both(t, caps, cbs);
        prop_assert_eq!(rust, c);
    }

    #[test]
    fn service_check_matches_c_for_any_input(cbs in any::<u32>()) {
        let (rust, c) = service_both(cbs);
        prop_assert_eq!(rust, c);
    }

    #[test]
    fn protocol_listed_matches_c_for_any_list(
        protocol in any_bytes(),
        list in proptest::collection::vec(any_bytes(), 0..8),
    ) {
        let (rust, c) = listed_both(&protocol, &list);
        prop_assert_eq!(rust, c);
    }

    #[test]
    fn registration_matches_c_for_any_size_and_mask(
        kind in 0u32..3,
        flags in any_flags(),
        t in prop_oneof![0u32..3, any::<u32>()],
        present in any::<u32>(),
        size in any_size(),
    ) {
        let rust = decide(&rust_fns(), kind, flags, t, present, size);
        let c = decide(&c_fns(), kind, flags, t, present, size);
        prop_assert_eq!(rust, c);
    }
}

/// The intentional differences, on the shim alone (the C's `strcmp()`
/// would read the NULL pointer): a NULL protocol is listed nowhere, a NULL
/// list is empty, and a NULL entry matches nothing but does not end the
/// list.
#[test]
fn protocol_listed_null_pointers() {
    let rtmp = c"RTMP";
    let list = [ptr::null_mut(), rtmp.as_ptr().cast_mut()];
    // SAFETY: NULL or NUL-terminated strings; `list.len()` entries.
    unsafe {
        assert!(!obs_module_type_rules_protocol_listed(
            ptr::null(),
            list.as_ptr(),
            2
        ));
        assert!(!obs_module_type_rules_protocol_listed(
            rtmp.as_ptr(),
            ptr::null(),
            0
        ));
        assert!(!obs_module_type_rules_protocol_listed(
            rtmp.as_ptr(),
            ptr::null(),
            5
        ));
        assert!(obs_module_type_rules_protocol_listed(
            rtmp.as_ptr(),
            list.as_ptr(),
            2
        ));
        assert!(!obs_module_type_rules_protocol_listed(
            rtmp.as_ptr(),
            list.as_ptr(),
            1
        ));
        assert!(!obs_module_type_rules_protocol_listed(
            c"".as_ptr(),
            list.as_ptr(),
            2
        ));
    }
}
