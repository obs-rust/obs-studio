#![cfg(windows)]
//! Tier 3: the Rust shims and safe core behave exactly like the Windows
//! branch of `utf8.c` (`MultiByteToWideChar`/`WideCharToMultiByte`),
//! compiled as an oracle. Return values and the whole output buffer
//! (including what a failed conversion leaves behind) are compared.
//!
//! Excluded inputs: a null input with `insize == 0` (C calls `strlen` or
//! `wcslen` on it), and sizes above `i32::MAX`, which C casts to negative
//! `int`s (the port returns 0; documented in `obs_util::platform::windows::utf8`).
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use core::ffi::c_char;
use core::ptr;

use obs_c_oracle::utf8 as c;
use obs_util::ffi::utf8 as rs;
use obs_util::platform::windows::utf8::{utf8_to_wchar, wchar_to_utf8};
use proptest::prelude::*;

const SENTINEL_BYTE: u8 = 0xa5;
const SENTINEL_WIDE: u16 = 0x5a5a;

/// The pointer cases the API decides by itself, on both sides.
#[test]
fn null_and_empty_arguments_match_c_oracle() {
    let mut wide = [SENTINEL_WIDE; 4];
    let mut theirs = [SENTINEL_WIDE; 4];
    let mut bytes = [SENTINEL_BYTE; 4];
    let mut their_bytes = [SENTINEL_BYTE; 4];
    // SAFETY: every non-null pointer is valid for the size passed.
    unsafe {
        // null input with a size
        assert_eq!(
            rs::utf8_to_wchar(ptr::null(), 3, wide.as_mut_ptr(), 4, 0),
            c::oracle_utf8_to_wchar(ptr::null(), 3, theirs.as_mut_ptr(), 4, 0)
        );
        assert_eq!(
            rs::wchar_to_utf8(ptr::null(), 3, bytes.as_mut_ptr().cast(), 4, 0),
            c::oracle_wchar_to_utf8(ptr::null(), 3, their_bytes.as_mut_ptr().cast(), 4, 0)
        );
        // null output with a size: rejected by the API
        let r = rs::utf8_to_wchar(c"abc".as_ptr(), 3, ptr::null_mut(), 8, 0);
        assert_eq!(
            r,
            c::oracle_utf8_to_wchar(c"abc".as_ptr(), 3, ptr::null_mut(), 8, 0)
        );
        assert_eq!(r, 0);
        let input = [0x61u16, 0x62];
        let r = rs::wchar_to_utf8(input.as_ptr(), 2, ptr::null_mut(), 8, 0);
        assert_eq!(
            r,
            c::oracle_wchar_to_utf8(input.as_ptr(), 2, ptr::null_mut(), 8, 0)
        );
        assert_eq!(r, 0);
        // a buffer of size 0 asks for the size
        let r = rs::utf8_to_wchar(c"abc".as_ptr(), 3, wide.as_mut_ptr(), 0, 0);
        assert_eq!(
            r,
            c::oracle_utf8_to_wchar(c"abc".as_ptr(), 3, theirs.as_mut_ptr(), 0, 0)
        );
        assert_eq!(r, 3);
        let r = rs::wchar_to_utf8(input.as_ptr(), 2, bytes.as_mut_ptr().cast(), 0, 0);
        assert_eq!(
            r,
            c::oracle_wchar_to_utf8(input.as_ptr(), 2, their_bytes.as_mut_ptr().cast(), 0, 0)
        );
        assert_eq!(r, 2);
    }
    assert_eq!(wide, theirs);
    assert_eq!(bytes, their_bytes);
}

/// Bytes: arbitrary, biased towards UTF-8 structure and a leading BOM, or
/// valid UTF-8 text.
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
        Just(0xf4),
    ];
    let bytes = prop_oneof![
        prop::collection::vec(byte, 0..24),
        any::<String>().prop_map(String::into_bytes),
    ];
    (any::<bool>(), bytes).prop_map(|(bom, mut bytes)| {
        if bom {
            bytes.splice(0..0, [0xef, 0xbb, 0xbf]);
        }
        bytes
    })
}

/// UTF-16 units: arbitrary, biased to surrogates (paired or not) and BOMs.
fn wide_strategy() -> impl Strategy<Value = Vec<u16>> {
    let unit = prop_oneof![
        any::<u16>(),
        1u16..0x80,
        0xd800u16..0xdc00,
        0xdc00u16..0xe000,
        Just(0xfeff),
        Just(0xfffd),
        Just(0xffff),
    ];
    prop::collection::vec(unit, 0..24)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(2048)))]

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
        let core_input: &[u8] = if nul_mode {
            &data[..data.iter().position(|&b| b == 0).unwrap()]
        } else {
            &data
        };

        // Output sizes 0..=len+4.
        let out_len = out_pick.index(data.len() + 5);
        let mut ours = vec![SENTINEL_WIDE; out_len];
        let mut theirs = vec![SENTINEL_WIDE; out_len];
        let mut core_out = vec![SENTINEL_WIDE; out_len];

        let (r_shim, r_c, r_core);
        if size_only {
            // SAFETY: `data` is readable for `insize` bytes or NUL-terminated;
            // `out` is null with size 0.
            unsafe {
                r_shim = rs::utf8_to_wchar(data.as_ptr().cast::<c_char>(), insize, ptr::null_mut(), 0, flags);
                r_c = c::oracle_utf8_to_wchar(data.as_ptr().cast::<c_char>(), insize, ptr::null_mut(), 0, flags);
            }
            r_core = utf8_to_wchar(core_input, None);
        } else {
            // SAFETY: `data` is readable for `insize` bytes or NUL-terminated;
            // each output vector holds `out_len` units.
            unsafe {
                r_shim = rs::utf8_to_wchar(data.as_ptr().cast::<c_char>(), insize, ours.as_mut_ptr(), out_len, flags);
                r_c = c::oracle_utf8_to_wchar(data.as_ptr().cast::<c_char>(), insize, theirs.as_mut_ptr(), out_len, flags);
            }
            r_core = utf8_to_wchar(core_input, Some(&mut core_out));
        }

        prop_assert_eq!(r_shim, r_c);
        prop_assert_eq!(r_core, r_c);
        prop_assert_eq!(&ours, &theirs);
        prop_assert_eq!(&core_out, &theirs);
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
        let mut data = wide.clone();
        let insize = if nul_mode { data.push(0); 0 } else { data.len() };
        let core_input: &[u16] = if nul_mode {
            &data[..data.iter().position(|&u| u == 0).unwrap()]
        } else {
            &data
        };

        // Output sizes 0..=3*len+4: a UTF-16 unit takes up to 3 bytes.
        let out_len = out_pick.index(data.len() * 3 + 5);
        let mut ours = vec![SENTINEL_BYTE; out_len];
        let mut theirs = vec![SENTINEL_BYTE; out_len];
        let mut core_out = vec![SENTINEL_BYTE; out_len];

        let (r_shim, r_c, r_core);
        if size_only {
            // SAFETY: `data` is readable for `insize` units or zero-terminated;
            // `out` is null with size 0.
            unsafe {
                r_shim = rs::wchar_to_utf8(data.as_ptr(), insize, ptr::null_mut(), 0, flags);
                r_c = c::oracle_wchar_to_utf8(data.as_ptr(), insize, ptr::null_mut(), 0, flags);
            }
            r_core = wchar_to_utf8(core_input, None);
        } else {
            // SAFETY: `data` is readable for `insize` units or zero-terminated;
            // each output vector holds `out_len` bytes.
            unsafe {
                r_shim = rs::wchar_to_utf8(data.as_ptr(), insize, ours.as_mut_ptr().cast::<c_char>(), out_len, flags);
                r_c = c::oracle_wchar_to_utf8(data.as_ptr(), insize, theirs.as_mut_ptr().cast::<c_char>(), out_len, flags);
            }
            r_core = wchar_to_utf8(core_input, Some(&mut core_out));
        }

        prop_assert_eq!(r_shim, r_c);
        prop_assert_eq!(r_core, r_c);
        prop_assert_eq!(&ours, &theirs);
        prop_assert_eq!(&core_out, &theirs);
    }
}
