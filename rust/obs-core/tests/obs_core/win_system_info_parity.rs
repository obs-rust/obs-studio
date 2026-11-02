//! Tier 3: `obs_core::win_system_info` against the original C.
//!
//! `obs-c-oracle` compiles the unchanged `libobs/obs-windows-system-info.c.h`
//! with its ten functions renamed `oracle_*` (it takes no Windows type, so
//! every host builds it). Every test runs the Rust shim, the C and the safe
//! core on the same input and compares the results exactly: the C's `bool`
//! as the byte it returns, and each label as the bytes of its C string.
//!
//! - the version over every pair of edge majors and minors (0, 1, 6, 10,
//!   255, 256, the bits that shift out of 32, `u32::MAX`, ...);
//! - the MB size over every power of two, its neighbours and the multiples
//!   of 1048576 around them, up to `u64::MAX`;
//! - the gate over the versions around 0xA00, every power of two and its
//!   neighbours, and a sweep of every 16-bit value;
//! - the Game Mode retry over every edge status;
//! - each feature's found check, value line, fallback check and fallback
//!   line, and the line they make together as `log_gaming_features()` puts
//!   it together, over every feature number from -2 to 7 and the ends of
//!   `int`, every edge status, every edge value and every build around
//!   19042: where no line is logged, neither side writes the label or text;
//! - the state and type labels over every `int` from -4096 to 4096 and the
//!   ends of `int`;
//! - proptests (4096 cases each) over any value of every input.
//!
//! The shims' null `label` or `text` (which the C never passes) is checked
//! on its own: they return false. No input is skipped on any host: the one
//! division is by a constant, and the shift and the cut are unsigned on both
//! sides. No intentional differences.

use std::ffi::{CStr, c_char};
use std::ptr;

use obs_c_oracle::win_system_info as oracle;
use obs_core::ffi::win_system_info as shim;
use obs_core::win_system_info as rules;
use proptest::prelude::*;

const U32_EDGES: &[u32] = &[
    0,
    1,
    2,
    3,
    6,
    9,
    10,
    11,
    0xFF,
    0x100,
    0x1FF,
    0x9FF,
    0xA00,
    0xA01,
    0xFFFF,
    0x00FF_FFFF,
    0x0100_0000,
    0x7FFF_FFFF,
    0x8000_0000,
    0xFFFF_FFFE,
    u32::MAX,
];

const STATUSES: &[i32] = &[0, 1, 2, 5, 234, 259, -1, i32::MIN, i32::MAX];

const BUILDS: &[u32] = &[
    0,
    7600,
    19040,
    19041,
    19042,
    19043,
    22000,
    22631,
    0x7FFF_FFFF,
    0x8000_0000,
    u32::MAX,
];

const FEATURE_NUMBERS: &[i32] = &[-2, -1, 0, 1, 2, 3, 4, 5, 6, 7, i32::MIN, i32::MAX];

fn bytes(s: *const c_char) -> Vec<u8> {
    assert!(!s.is_null());
    // SAFETY: both sides return 'static NUL-terminated strings.
    unsafe { CStr::from_ptr(s) }.to_bytes().to_vec()
}

/// One feature line from a side: the returned byte, and the label and text
/// as bytes, or the sentinel when that side did not write them.
type Answer = (u8, Option<Vec<u8>>, Option<Vec<u8>>);

/// What an out-pointer holds until a side writes it. A `static`, so it has
/// one address: a `const` C string may be a different copy at each use
/// (MSVC does not merge them), and the check below compares addresses.
static SENTINEL_BYTES: [u8; 9] = *b"sentinel\0";

fn sentinel() -> *const c_char {
    SENTINEL_BYTES.as_ptr().cast()
}

fn read(written: *const c_char) -> Option<Vec<u8>> {
    (written != sentinel()).then(|| bytes(written))
}

/// One side's four gaming functions, each answer as a byte and the label and
/// text it wrote.
struct Gaming {
    found: fn(i32) -> u8,
    value_line: fn(i32, u32) -> Answer,
    fallback: fn(i32) -> u8,
    fallback_line: fn(i32, u32) -> Answer,
}

/// Runs one out-pointer call with sentinels in place.
fn answer(call: impl FnOnce(*mut *const c_char, *mut *const c_char) -> u8) -> Answer {
    let mut label = sentinel();
    let mut text = sentinel();
    let r = call(&mut label, &mut text);
    (r, read(label), read(text))
}

// SAFETY (each): plain values, and out-pointers valid for a write.
const C: Gaming = Gaming {
    found: |s| unsafe { oracle::oracle_obs_windows_system_info_gaming_found(s) },
    value_line: |f, v| {
        answer(|l, t| unsafe {
            oracle::oracle_obs_windows_system_info_gaming_value_line(f, v, l, t)
        })
    },
    fallback: |f| unsafe { oracle::oracle_obs_windows_system_info_gaming_fallback(f) },
    fallback_line: |f, b| {
        answer(|l, t| unsafe {
            oracle::oracle_obs_windows_system_info_gaming_fallback_line(f, b, l, t)
        })
    },
};

// SAFETY (each): out-pointers valid for a write.
const SHIM: Gaming = Gaming {
    found: |s| u8::from(shim::obs_windows_system_info_gaming_found(s)),
    value_line: |f, v| {
        answer(|l, t| {
            u8::from(unsafe { shim::obs_windows_system_info_gaming_value_line(f, v, l, t) })
        })
    },
    fallback: |f| u8::from(shim::obs_windows_system_info_gaming_fallback(f)),
    fallback_line: |f, b| {
        answer(|l, t| {
            u8::from(unsafe { shim::obs_windows_system_info_gaming_fallback_line(f, b, l, t) })
        })
    },
};

fn core_answer(line: Option<rules::Line>) -> Answer {
    match line {
        Some(line) => (
            1,
            Some(line.label.to_bytes().to_vec()),
            Some(line.text.to_bytes().to_vec()),
        ),
        None => (0, None, None),
    }
}

const CORE: Gaming = Gaming {
    found: |s| u8::from(rules::gaming_found(s)),
    value_line: |f, v| core_answer(rules::Feature::from_index(f).map(|f| rules::value_line(f, v))),
    fallback: |f| u8::from(rules::Feature::from_index(f).is_some_and(rules::has_fallback)),
    fallback_line: |f, b| {
        core_answer(rules::Feature::from_index(f).and_then(|f| rules::fallback_line(f, b)))
    },
};

/// The line `log_gaming_features()` logs for one feature, from one side's
/// functions: the value only when the read found it, the build only for a
/// feature with a fallback.
fn line(side: &Gaming, feature: i32, status: i32, value: u32, build: u32) -> Answer {
    if (side.found)(status) != 0 {
        (side.value_line)(feature, value)
    } else if (side.fallback)(feature) != 0 {
        (side.fallback_line)(feature, build)
    } else {
        (0, None, None)
    }
}

fn check_line(feature: i32, status: i32, value: u32, build: u32) {
    let at = format!("{feature} {status} {value} {build}");
    for (name, side) in [("shim", &SHIM), ("core", &CORE)] {
        assert_eq!(
            (side.found)(status),
            (C.found)(status),
            "{name} found: {at}"
        );
        assert_eq!(
            (side.value_line)(feature, value),
            (C.value_line)(feature, value),
            "{name} value line: {at}"
        );
        assert_eq!(
            (side.fallback)(feature),
            (C.fallback)(feature),
            "{name} fallback: {at}"
        );
        assert_eq!(
            (side.fallback_line)(feature, build),
            (C.fallback_line)(feature, build),
            "{name} fallback line: {at}"
        );
        assert_eq!(
            line(side, feature, status, value, build),
            line(&C, feature, status, value, build),
            "{name} line: {at}"
        );
    }
}

fn check_version(major: u32, minor: u32) {
    // SAFETY: plain integer arguments.
    let c = unsafe { oracle::oracle_obs_windows_system_info_version(major, minor) };
    assert_eq!(
        shim::obs_windows_system_info_version(major, minor),
        c,
        "{major} {minor}"
    );
    assert_eq!(rules::version(major, minor), c, "{major} {minor}");
}

fn check_mb(size: u64) {
    // SAFETY: a plain integer argument.
    let c = unsafe { oracle::oracle_obs_windows_system_info_mb(size) };
    assert_eq!(shim::obs_windows_system_info_mb(size), c, "{size}");
    assert_eq!(rules::mb(size), c, "{size}");
}

fn check_gate(win_ver: u32) {
    // SAFETY: a plain integer argument.
    let c = unsafe { oracle::oracle_obs_windows_system_info_gaming_logged(win_ver) };
    assert_eq!(
        u8::from(shim::obs_windows_system_info_gaming_logged(win_ver)),
        c,
        "{win_ver}"
    );
    assert_eq!(u8::from(rules::gaming_logged(win_ver)), c, "{win_ver}");
}

fn check_retry(status: i32) {
    // SAFETY: a plain integer argument.
    let c = unsafe { oracle::oracle_obs_windows_system_info_game_mode_retry(status) };
    assert_eq!(
        u8::from(shim::obs_windows_system_info_game_mode_retry(status)),
        c,
        "{status}"
    );
    assert_eq!(u8::from(rules::game_mode_retry(status)), c, "{status}");
}

fn check_labels(value: i32) {
    // SAFETY: plain integer arguments; both return 'static strings.
    let (state, kind) = unsafe {
        (
            bytes(oracle::oracle_obs_windows_system_info_product_state(value)),
            bytes(oracle::oracle_obs_windows_system_info_product_type(value)),
        )
    };
    assert_eq!(
        bytes(shim::obs_windows_system_info_product_state(value)),
        state,
        "{value}"
    );
    assert_eq!(
        bytes(shim::obs_windows_system_info_product_type(value)),
        kind,
        "{value}"
    );
    assert_eq!(rules::product_state(value).to_bytes(), state, "{value}");
    assert_eq!(rules::product_type(value).to_bytes(), kind, "{value}");
}

#[test]
fn parity_every_version_edge() {
    for &major in U32_EDGES {
        for &minor in U32_EDGES {
            check_version(major, minor);
        }
    }
}

#[test]
fn parity_every_size_edge() {
    const MB: u64 = 1_048_576;
    for bit in 0..64 {
        let p = 1u64 << bit;
        for size in [
            p - 1,
            p,
            p + 1,
            p.wrapping_mul(MB),
            p.wrapping_mul(MB).wrapping_sub(1),
        ] {
            check_mb(size);
        }
    }
    for size in [
        0,
        MB - 1,
        MB,
        MB + 1,
        u64::from(u32::MAX) * MB,
        (1 << 52) + MB,
        u64::MAX,
    ] {
        check_mb(size);
    }
}

#[test]
fn parity_every_gate_edge() {
    for v in obs_c_oracle::parity::sweep(0..0x1_0000) {
        check_gate(u32::try_from(v).expect("16 bits"));
    }
    for bit in 0..32 {
        let p = 1u32 << bit;
        for v in [p - 1, p, p.wrapping_add(1)] {
            check_gate(v);
        }
    }
    for &v in U32_EDGES {
        check_gate(v);
    }
}

#[test]
fn parity_every_retry_edge() {
    for &status in STATUSES {
        check_retry(status);
    }
}

#[test]
fn parity_every_feature_line() {
    for &feature in FEATURE_NUMBERS {
        for &status in STATUSES {
            for &value in U32_EDGES {
                for &build in BUILDS {
                    check_line(feature, status, value, build);
                }
            }
        }
    }
}

#[test]
fn parity_every_label() {
    for value in -4096..=4096 {
        check_labels(value);
    }
    for value in [i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX] {
        check_labels(value);
    }
}

#[test]
fn shim_null_outputs() {
    let mut label = sentinel();
    let mut text = sentinel();
    for feature in 0..5 {
        // SAFETY: a null pointer, or one valid for a write.
        unsafe {
            assert!(!shim::obs_windows_system_info_gaming_value_line(
                feature,
                1,
                ptr::null_mut(),
                &mut text
            ));
            assert!(!shim::obs_windows_system_info_gaming_value_line(
                feature,
                1,
                &mut label,
                ptr::null_mut()
            ));
            assert!(!shim::obs_windows_system_info_gaming_fallback_line(
                feature,
                19042,
                ptr::null_mut(),
                &mut text
            ));
            assert!(!shim::obs_windows_system_info_gaming_fallback_line(
                feature,
                19042,
                &mut label,
                ptr::null_mut()
            ));
        }
        assert_eq!(label, sentinel());
        assert_eq!(text, sentinel());
    }
}

fn status() -> impl Strategy<Value = i32> {
    prop_oneof![Just(0), Just(2), any::<i32>()]
}

fn value() -> impl Strategy<Value = u32> {
    prop_oneof![Just(0), Just(1), any::<u32>()]
}

fn build() -> impl Strategy<Value = u32> {
    prop_oneof![19038u32..19046, any::<u32>()]
}

fn feature() -> impl Strategy<Value = i32> {
    prop_oneof![-1i32..7, any::<i32>()]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(4096)))]

    #[test]
    fn proptest_version(major in any::<u32>(), minor in any::<u32>()) {
        check_version(major, minor);
    }

    #[test]
    fn proptest_size(size in any::<u64>()) {
        check_mb(size);
    }

    #[test]
    fn proptest_gate_and_retry(win_ver in any::<u32>(), status in any::<i32>()) {
        check_gate(win_ver);
        check_retry(status);
    }

    #[test]
    fn proptest_line(feature in feature(), status in status(), value in value(), build in build()) {
        check_line(feature, status, value, build);
    }

    #[test]
    fn proptest_labels(value in any::<i32>()) {
        check_labels(value);
    }
}
