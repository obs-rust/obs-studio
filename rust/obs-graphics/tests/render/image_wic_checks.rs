//! Tier 1: `obs_graphics::image_pq_scrgb`'s two checks of the image
//! loader's Windows host, the file-name check of
//! `gs_image_host_fallback_load()` ([`fallback_refuses`]) and the
//! pixel-format pick of `wic_image_init_internal()` ([`wic_kind`]).
//!
//! The `steps_*` cases port `test/cmocka/test_graphics_ffmpeg_wic_checks.c`
//! 1:1: its 4 cases under the same names, in the same order, with the same
//! values and expectations, the characterized-not-endorsed longer names
//! included.
//!
//! The `edge_*` cases are new: the names shorter than 4 bytes, which the
//! C compares from before their start and the port refuses (the intentional
//! difference); every 4-byte name over the bytes around `.jxr` in either
//! case, against the rule spelled out byte by byte; and proptests that a
//! longer name is never refused and a 4-byte one only by that rule.

use obs_graphics::image_pq_scrgb::{JXR_SUFFIX, WicKind, fallback_refuses, wic_kind};
use proptest::prelude::*;

fn refuses(name: &str) -> bool {
    fallback_refuses(name.as_bytes())
}

/// The 4-byte rule, byte by byte: `.`, then `j`, `x` and `r` in either case.
fn is_jxr(name: &[u8]) -> bool {
    matches!(name, [b'.', b'j' | b'J', b'x' | b'X', b'r' | b'R'])
}

// ---------------------------------------------------------------------------
// test_graphics_ffmpeg_wic_checks.c, 1:1

#[test]
fn steps_four_bytes() {
    assert!(!refuses(".jxr"));
    assert!(!refuses(".JXR"));
    assert!(!refuses(".JxR"));
    assert!(!refuses(".jXr"));

    assert!(refuses("a.jx"));
    assert!(refuses("abcd"));
    assert!(refuses("jxr."));
    assert!(refuses("jxr "));
    assert!(refuses(",jxr"));
    assert!(refuses(".jxs"));
    assert!(refuses(".png"));
}

/// Characterized, not endorsed: a name longer than 4 bytes is never
/// refused, whatever it ends with.
#[test]
fn steps_longer_names() {
    assert!(!refuses("a.jxr"));
    assert!(!refuses("a.JXR"));
    assert!(!refuses("a.png"));
    assert!(!refuses("a.jxra"));
    assert!(!refuses("abcde"));
    assert!(!refuses("C:\\images\\hdr.jxr"));
    assert!(!refuses("C:\\images\\shot.png"));
    assert!(!refuses("C:\\images\\shot.wdp"));
    assert!(!refuses("/images/no-extension"));
}

#[test]
fn steps_pick() {
    assert_eq!(wic_kind(false, false), WicKind::Unsupported);
    assert_eq!(wic_kind(true, false), WicKind::Copy);
    assert_eq!(wic_kind(false, true), WicKind::Pq10);
    assert_eq!(wic_kind(true, true), WicKind::Pq10);
}

#[test]
fn steps_kind_values() {
    assert_eq!(WicKind::Unsupported as i32, 0);
    assert_eq!(WicKind::Copy as i32, 1);
    assert_eq!(WicKind::Pq10 as i32, 2);
}

// ---------------------------------------------------------------------------
// edge cases

/// The intentional difference: the C compares a name shorter than 4 bytes
/// from before its start; none of them can end in `.jxr`, and the port
/// refuses them all.
#[test]
fn edge_short_names_refused() {
    for name in ["", "a", ".", "r", "xr", "jxr", ".jx", "JXR", "\u{80}"] {
        assert!(refuses(name), "{name:?}");
    }
    for len in 0..JXR_SUFFIX.len() {
        assert!(fallback_refuses(&JXR_SUFFIX[JXR_SUFFIX.len() - len..]));
        assert!(fallback_refuses(&JXR_SUFFIX[..len]));
    }
}

/// Every 4-byte name over the bytes of `.jxr` in both cases, their
/// neighbours, the bytes 0x20 away (an ASCII case flip of a non-letter)
/// and some non-ASCII ones.
#[test]
fn edge_every_four_byte_name() {
    let bytes = [
        b'.', b'j', b'J', b'x', b'X', b'r', b'R', b'a', b'i', b'k', b'w', b'y', b'q', b's', b'-',
        b'/', 0x0e, 0x4e, 0x8a, 0xaa, 0xca, 0xea, 0x80, 0xff, 0x01,
    ];
    let mut accepted = 0_u32;
    for a in bytes {
        for b in bytes {
            for c in bytes {
                for d in bytes {
                    let name = [a, b, c, d];
                    let refused = fallback_refuses(&name);
                    assert_eq!(refused, !is_jxr(&name), "{name:02x?}");
                    accepted += u32::from(!refused);
                }
            }
        }
    }
    assert_eq!(accepted, 8);
}

/// Only a name of exactly four bytes is compared with `.jxr`: where a
/// longer name has it, or doesn't, changes nothing.
#[test]
fn edge_suffix_position() {
    assert!(!refuses(".jxra"));
    assert!(!refuses("a.jxr"));
    assert!(!refuses(".jxr.jxr"));
    assert!(!refuses("x.jxs"));
    assert!(refuses("x.jx"));
}

#[test]
fn edge_pick_is_pq10_first() {
    for scrgb in [false, true] {
        for pq10 in [false, true] {
            let want = if pq10 {
                WicKind::Pq10
            } else if scrgb {
                WicKind::Copy
            } else {
                WicKind::Unsupported
            };
            assert_eq!(wic_kind(scrgb, pq10), want, "scrgb {scrgb} pq10 {pq10}");
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    /// A name longer than 4 bytes is never refused (characterized, not
    /// endorsed).
    #[test]
    fn edge_longer_names_accepted(name in proptest::collection::vec(1_u8..=255, 5..64)) {
        prop_assert!(!fallback_refuses(&name));
    }

    /// A 4-byte name is refused unless it is `.jxr` in some case.
    #[test]
    fn edge_four_byte_names(name in proptest::collection::vec(any::<u8>(), 4)) {
        prop_assert_eq!(fallback_refuses(&name), !is_jxr(&name));
    }

    /// A shorter one is always refused (the intentional difference).
    #[test]
    fn edge_short_names(name in proptest::collection::vec(any::<u8>(), 0..4)) {
        prop_assert!(fallback_refuses(&name));
    }
}
