//! Tier 3: `obs_graphics::image_pq_scrgb`'s two checks of the image
//! loader's Windows host against the original C
//! (`libobs/graphics/graphics-ffmpeg-host-wic-checks.c.h`, through
//! `obs_c_oracle::image_wic_checks`), on all three sides: the oracle, the
//! C ABI shims (`gs_image_host_fallback_refuses`, `gs_image_wic_kind`) and
//! the safe core.
//!
//! - the name check over every 4-byte name over 25 bytes around `.jxr`,
//!   every one-byte change of `.jxr`, `.JXR`, `a.jxr` and `ab.jxr` to each
//!   other byte, every 5-byte name over 10 of those bytes, and a proptest
//!   of random names of 4 to 63 bytes;
//! - the pick over every `(scrgb, pq10)` pair.
//!
//! Intentional difference (the one in the core's docs): for a name shorter
//! than 4 bytes the C compares from before its start. Those names are not
//! passed to the C; the shim and the core refuse them, checked here on
//! their own.
//!
//! `astrcmpi()` calls `toupper()` on each `char`. On MSVC that is undefined
//! for a byte of 0x80 or more (a negative `char`), so there only the ASCII
//! bytes go to the C; elsewhere every byte does, and the C locale leaves
//! the non-ASCII ones unchanged, as `eq_ignore_ascii_case` does.

use core::ffi::c_char;

use obs_c_oracle::image_wic_checks::{
    oracle_gs_image_host_fallback_refuses, oracle_gs_image_wic_kind,
};
use obs_graphics::ffi::image_pq_scrgb::{gs_image_host_fallback_refuses, gs_image_wic_kind};
use obs_graphics::image_pq_scrgb::{WicKind, fallback_refuses, wic_kind};
use proptest::prelude::*;

/// Only ASCII bytes go to the C (see the module docs).
const C_TOUPPER_ASCII_ONLY: bool = cfg!(target_env = "msvc");

fn byte_ok_for_c(byte: u8) -> bool {
    byte != 0 && (!C_TOUPPER_ASCII_ONLY || byte < 0x80)
}

/// Every side's answer for `name`, which has no NUL and at least 4 bytes:
/// the oracle, the shim and the core.
fn three_sides(name: &[u8]) -> [bool; 3] {
    assert!(name.len() >= 4 && !name.contains(&0));
    let mut c_name = name.to_vec();
    c_name.push(0);
    let file = c_name.as_ptr().cast::<c_char>();
    // SAFETY: `c_name` holds `name.len()` bytes and a NUL after them, which
    // is what the C reads from `file + len - 4` to; the shim reads `len`.
    let (oracle, shim) = unsafe {
        (
            oracle_gs_image_host_fallback_refuses(file, name.len()),
            gs_image_host_fallback_refuses(file, name.len()),
        )
    };
    [oracle, shim, fallback_refuses(name)]
}

fn expect_same(name: &[u8]) {
    let [oracle, shim, core] = three_sides(name);
    assert_eq!((shim, core), (oracle, oracle), "name {name:02x?}");
}

const AROUND_JXR: [u8; 25] = [
    b'.', b'j', b'J', b'x', b'X', b'r', b'R', b'a', b'i', b'k', b'w', b'y', b'q', b's', b'-', b'/',
    0x0e, 0x4e, 0x8a, 0xaa, 0xca, 0xea, 0x80, 0xff, 0x01,
];

#[test]
fn parity_every_four_byte_name() {
    let bytes: Vec<u8> = AROUND_JXR
        .into_iter()
        .filter(|&b| byte_ok_for_c(b))
        .collect();
    for &a in &bytes {
        for &b in &bytes {
            for &c in &bytes {
                for &d in &bytes {
                    expect_same(&[a, b, c, d]);
                }
            }
        }
    }
}

#[test]
fn parity_every_one_byte_change() {
    for base in [&b".jxr"[..], b".JXR", b"a.jxr", b"ab.jxr"] {
        for at in 0..base.len() {
            for byte in (1_u8..=255).filter(|&b| byte_ok_for_c(b)) {
                let mut name = base.to_vec();
                name[at] = byte;
                expect_same(&name);
            }
        }
    }
}

#[test]
fn parity_every_five_byte_name() {
    let bytes = [b'.', b'j', b'X', b'r', b'a', b'R', b'x', b'J', b'/', 0x01];
    for a in bytes {
        for b in bytes {
            for c in bytes {
                for d in bytes {
                    for e in bytes {
                        expect_same(&[a, b, c, d, e]);
                    }
                }
            }
        }
    }
}

/// The intentional difference: names shorter than 4 bytes never go to the
/// C, and both Rust sides refuse them.
#[test]
fn parity_short_names_refused_by_rust() {
    for name in [&b""[..], b"a", b"xr", b"jxr", b".jx", b"JXR"] {
        let mut c_name = name.to_vec();
        c_name.push(0);
        // SAFETY: `name.len()` readable bytes at the pointer.
        let shim = unsafe { gs_image_host_fallback_refuses(c_name.as_ptr().cast(), name.len()) };
        assert!(shim, "{name:02x?}");
        assert!(fallback_refuses(name), "{name:02x?}");
    }
}

/// The shim refuses a null name, which the C caller never passes.
#[test]
fn parity_shim_null_refused() {
    // SAFETY: a null `file` is allowed and read from nowhere.
    assert!(unsafe { gs_image_host_fallback_refuses(core::ptr::null(), 4) });
}

#[test]
fn parity_pick() {
    for scrgb in [false, true] {
        for pq10 in [false, true] {
            // SAFETY: the oracle takes and returns plain values.
            let oracle = unsafe { oracle_gs_image_wic_kind(scrgb, pq10) };
            let shim = gs_image_wic_kind(scrgb, pq10);
            let core = wic_kind(scrgb, pq10) as i32;
            assert_eq!((shim, core), (oracle, oracle), "scrgb {scrgb} pq10 {pq10}");
        }
    }
    assert_eq!(gs_image_wic_kind(false, false), WicKind::Unsupported as i32);
}

fn c_byte() -> BoxedStrategy<u8> {
    if C_TOUPPER_ASCII_ONLY {
        (1_u8..0x80).boxed()
    } else {
        (1_u8..=255).boxed()
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(2048)))]

    #[test]
    fn parity_random_names(name in proptest::collection::vec(c_byte(), 4..64)) {
        let [oracle, shim, core] = three_sides(&name);
        prop_assert_eq!(shim, oracle);
        prop_assert_eq!(core, oracle);
    }
}
