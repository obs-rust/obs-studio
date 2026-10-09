#![cfg(not(windows))]
//! Tier 3: the Rust C ABI shims and safe core behave exactly like the original
//! C `utf8.c` (non-Windows branch), compiled as an oracle. Return values and
//! the whole output buffer (including partial output left behind by failed
//! conversions) are compared.
//!
//! Skipped on Windows because there the oracle compiles the
//! `MultiByteToWideChar` branch, which is not ported.
//!
//! No intentional differences from C.
//!
//! Mutation check (done once per port): flipping the surrogate check in
//! `wchar_forbidden` in `src/utf8.rs` makes `utf8_parity` fail.
//!
//! Reminder: the mutation check is done once per port, not on every change.

use core::ffi::c_char;

use obs_c_oracle::utf8 as c;
use obs_util::ffi::utf8 as rs;
use obs_util::utf8::{utf8_to_wchar, wchar_to_utf8};
use proptest::prelude::*;

const SENTINEL_BYTE: u8 = 0xa5;
const SENTINEL_WIDE: i32 = 0x5a5a_5a5a;

/// Tier 2: shim argument checks.
#[test]
fn shims_reject_null_and_empty_out() {
    let mut wide = [0i32; 4];
    let mut bytes = [0u8; 4];
    // SAFETY: every non-null pointer is valid for the sizes passed.
    unsafe {
        assert_eq!(
            rs::utf8_to_wchar(core::ptr::null(), 0, wide.as_mut_ptr(), 4, 0),
            0
        );
        assert_eq!(
            c::oracle_utf8_to_wchar(core::ptr::null(), 0, wide.as_mut_ptr(), 4, 0),
            0
        );
        assert_eq!(
            rs::utf8_to_wchar(b"abc".as_ptr().cast(), 3, wide.as_mut_ptr(), 0, 0),
            0
        );
        assert_eq!(
            rs::wchar_to_utf8(core::ptr::null(), 0, bytes.as_mut_ptr().cast(), 4, 0),
            0
        );
        assert_eq!(
            rs::wchar_to_utf8([0x61i32].as_ptr(), 1, bytes.as_mut_ptr().cast(), 0, 0),
            0
        );
        // NUL-terminated mode and size-only mode.
        assert_eq!(
            rs::utf8_to_wchar(c"a\u{e9}".as_ptr(), 0, core::ptr::null_mut(), 0, 0),
            2
        );
        assert_eq!(
            rs::wchar_to_utf8([0x61i32, 0, 0x62].as_ptr(), 0, core::ptr::null_mut(), 0, 0),
            1
        );
    }
}

/// Bytes: arbitrary, biased towards UTF-8 structure, or valid UTF-8 text.
fn bytes_strategy() -> impl Strategy<Value = Vec<u8>> {
    let byte = prop_oneof![
        any::<u8>(),
        0u8..0x80,
        0x80u8..0xc0,
        0xc0u8..=0xff,
        Just(0xef),
        Just(0xbb),
        Just(0xbf),
        Just(0xed),
        Just(0xc0),
        Just(0xf5),
    ];
    prop_oneof![
        prop::collection::vec(byte, 0..24),
        any::<String>().prop_map(String::into_bytes),
    ]
}

/// Wide characters: arbitrary (including negative as i32), biased to edges.
fn wide_strategy() -> impl Strategy<Value = Vec<u32>> {
    let unit = prop_oneof![
        any::<u32>(),
        0u32..0x200,
        0xd7f0u32..0xe010,
        Just(0xfeff),
        0xfff0u32..0x110010,
        0x1f_fff0u32..0x20_0010,
        0x03ff_fff0u32..0x0400_0010,
        0x7fff_fff0u32..=0x8000_0010,
        0xffff_fff0u32..=u32::MAX,
    ];
    prop::collection::vec(unit, 0..24)
}

proptest! {
    #[test]
    fn utf8_to_wchar_matches_c_oracle(
        bytes in bytes_strategy(),
        nul_mode in any::<bool>(),
        out_pick in any::<prop::sample::Index>(),
        size_only in any::<bool>(),
        flags in 0i32..4,
    ) {
        let nul_mode = nul_mode || bytes.is_empty();
        let mut data = bytes.clone();
        let insize = if nul_mode { data.push(0); 0 } else { data.len() };
        // The core sees what C converts: up to insize, or up to the first NUL.
        let core_input: &[u8] = if nul_mode {
            &data[..data.iter().position(|&b| b == 0).unwrap()]
        } else {
            &data
        };

        // Output sizes 0..=len+4, where len is the input length.
        let out_len = out_pick.index(data.len() + 5);
        let mut ours = vec![SENTINEL_WIDE; out_len];
        let mut theirs = vec![SENTINEL_WIDE; out_len];
        let mut core_out: Vec<u32> = vec![SENTINEL_WIDE as u32; out_len];

        let (r_shim, r_c, r_core);
        if size_only {
            // SAFETY: `data` is readable for `insize` bytes or NUL-terminated;
            // `out` is null.
            unsafe {
                r_shim = rs::utf8_to_wchar(data.as_ptr().cast::<c_char>(), insize, core::ptr::null_mut(), out_len, flags);
                r_c = c::oracle_utf8_to_wchar(data.as_ptr().cast::<c_char>(), insize, core::ptr::null_mut(), out_len, flags);
            }
            r_core = utf8_to_wchar(core_input, None, flags);
        } else {
            // SAFETY: `data` is readable for `insize` bytes or NUL-terminated;
            // each output vector holds `out_len` wide characters.
            unsafe {
                r_shim = rs::utf8_to_wchar(data.as_ptr().cast::<c_char>(), insize, ours.as_mut_ptr(), out_len, flags);
                r_c = c::oracle_utf8_to_wchar(data.as_ptr().cast::<c_char>(), insize, theirs.as_mut_ptr(), out_len, flags);
            }
            r_core = utf8_to_wchar(core_input, Some(&mut core_out), flags);
        }

        prop_assert_eq!(r_shim, r_c);
        prop_assert_eq!(r_core, r_c);
        prop_assert_eq!(&ours, &theirs);
        let core_as_i32: Vec<i32> = core_out.iter().map(|&u| u as i32).collect();
        prop_assert_eq!(&core_as_i32, &theirs);
    }

    #[test]
    fn wchar_to_utf8_matches_c_oracle(
        wide in wide_strategy(),
        nul_mode in any::<bool>(),
        out_pick in any::<prop::sample::Index>(),
        size_only in any::<bool>(),
        flags in 0i32..4,
    ) {
        let nul_mode = nul_mode || wide.is_empty();
        let mut data: Vec<i32> = wide.iter().map(|&u| u as i32).collect();
        let insize = if nul_mode { data.push(0); 0 } else { data.len() };
        let core_input: Vec<u32> = if nul_mode {
            let end = data.iter().position(|&u| u == 0).unwrap();
            data[..end].iter().map(|&i| i as u32).collect()
        } else {
            wide.clone()
        };

        // Output sizes 0..=6*len+4: a wide character takes up to 6 bytes.
        let out_len = out_pick.index(data.len() * 6 + 5);
        let mut ours = vec![SENTINEL_BYTE; out_len];
        let mut theirs = vec![SENTINEL_BYTE; out_len];
        let mut core_out = vec![SENTINEL_BYTE; out_len];

        let (r_shim, r_c, r_core);
        if size_only {
            // SAFETY: `data` is readable for `insize` units or zero-terminated;
            // `out` is null.
            unsafe {
                r_shim = rs::wchar_to_utf8(data.as_ptr(), insize, core::ptr::null_mut(), out_len, flags);
                r_c = c::oracle_wchar_to_utf8(data.as_ptr(), insize, core::ptr::null_mut(), out_len, flags);
            }
            r_core = wchar_to_utf8(&core_input, None, flags);
        } else {
            // SAFETY: `data` is readable for `insize` units or zero-terminated;
            // each output vector holds `out_len` bytes.
            unsafe {
                r_shim = rs::wchar_to_utf8(data.as_ptr(), insize, ours.as_mut_ptr().cast::<c_char>(), out_len, flags);
                r_c = c::oracle_wchar_to_utf8(data.as_ptr(), insize, theirs.as_mut_ptr().cast::<c_char>(), out_len, flags);
            }
            r_core = wchar_to_utf8(&core_input, Some(&mut core_out), flags);
        }

        prop_assert_eq!(r_shim, r_c);
        prop_assert_eq!(r_core, r_c);
        prop_assert_eq!(&ours, &theirs);
        prop_assert_eq!(&core_out, &theirs);
    }
}
