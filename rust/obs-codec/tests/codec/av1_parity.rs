//! Tier 3: the Rust C ABI shims and safe core behave exactly like
//! `libobs/obs-av1.c`, compiled as an oracle.
//!
//! The oracle is obs-av1.c with its OBU parsing kept inside the buffer
//! (fixed in the C file first, see test/cmocka/test_av1.c), so any bytes
//! are a valid input on both sides. No intentional differences.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use core::{ptr, slice};

use obs_c_oracle::av1 as c;
use obs_codec::av1;
use obs_codec::ffi::av1 as rs;
use obs_util::ffi::darray::bfree;
use proptest::prelude::*;

/// Takes ownership of a `bmalloc`ed buffer: its bytes, then `bfree`.
fn take(p: *mut u8, len: usize) -> Option<Vec<u8>> {
    if p.is_null() {
        return None;
    }
    // SAFETY: `p` holds `len` bytes from bmalloc and is freed once.
    unsafe {
        let v = slice::from_raw_parts(p, len).to_vec();
        bfree(p.cast());
        Some(v)
    }
}

/// leb128 encodings, including padded (over-long) ones and up to 9 bytes,
/// so values past the 8-byte read limit and the old int overflow appear.
fn leb128(value: u64, pad: usize) -> Vec<u8> {
    let mut v = Vec::new();
    let mut val = value;
    loop {
        let b = (val & 0x7f) as u8;
        val >>= 7;
        if val == 0 && pad == 0 {
            v.push(b);
            break;
        }
        v.push(b | 0x80);
        if val == 0 {
            v.extend(std::iter::repeat_n(0x80, pad - 1));
            v.push(0);
            break;
        }
    }
    v
}

/// One OBU: header bits, an optional extension byte, an optional size
/// field that may claim more or less than the payload, and the payload.
fn obu() -> impl Strategy<Value = Vec<u8>> {
    (
        prop_oneof![
            3 => Just(av1::OBU_FRAME),
            2 => Just(av1::OBU_FRAME_HEADER),
            2 => Just(av1::OBU_SEQUENCE_HEADER),
            2 => Just(av1::OBU_METADATA),
            1 => Just(av1::OBU_TEMPORAL_DELIMITER),
            1 => 0u8..16,
        ],
        any::<bool>(),
        any::<bool>(),
        any::<u8>(), // low bits: reserved / forbidden noise
        prop::collection::vec(any::<u8>(), 0..12),
        prop_oneof![
            4 => Just(None),
            2 => (0u64..200).prop_map(Some),
            1 => any::<u64>().prop_map(Some),
        ],
        0usize..4,
    )
        .prop_map(|(kind, ext, has_size, noise, payload, claim, pad)| {
            let mut v = vec![
                (noise & 0x81) | (kind << 3) | (u8::from(ext) << 2) | (u8::from(has_size) << 1),
            ];
            if ext {
                v.push(noise);
            }
            if has_size {
                let size = claim.unwrap_or(payload.len() as u64);
                v.extend(leb128(size, pad));
            }
            v.extend(payload);
            v
        })
}

fn stream() -> impl Strategy<Value = Vec<u8>> {
    (
        prop::collection::vec(obu(), 0..6),
        prop::option::of(0usize..6),
    )
        .prop_map(|(obus, cut)| {
            let mut v: Vec<u8> = obus.concat();
            if let Some(cut) = cut {
                v.truncate(v.len().saturating_sub(cut));
            }
            v
        })
}

/// Well-formed OBUs with payloads of 100..400 bytes, so their sizes take two
/// leb128 bytes and must decode exactly for the next OBU to line up.
fn big_obus() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(
        (
            prop_oneof![
                Just(av1::OBU_SEQUENCE_HEADER),
                Just(av1::OBU_METADATA),
                Just(av1::OBU_FRAME)
            ],
            100usize..400,
            any::<u8>(),
        ),
        1..4,
    )
    .prop_map(|obus| {
        let mut v = Vec::new();
        for (kind, len, fill) in obus {
            v.push((kind << 3) | (1 << 1));
            v.extend(leb128(len as u64, 0));
            v.extend(std::iter::repeat_n(fill & 0x7f, len));
        }
        v
    })
}

fn input() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        5 => stream(),
        2 => big_obus(),
        2 => prop::collection::vec(any::<u8>(), 0..64),
    ]
}

fn check(input: &[u8]) -> Result<(), TestCaseError> {
    // Keep even an empty input inside a real allocation.
    let mut backing = input.to_vec();
    backing.push(0xee);
    let data = &backing[..input.len()];
    let p = data.as_ptr();
    // SAFETY: every call reads only `data` and writes only locals; each
    // returned buffer is taken (and freed) once.
    unsafe {
        let want = c::oracle_obs_av1_keyframe(p, data.len());
        prop_assert_eq!(
            rs::obs_av1_keyframe(p, data.len()),
            want,
            "keyframe of {:02x?}",
            data
        );
        prop_assert_eq!(av1::keyframe(data), want);

        let (mut rp, mut rpn, mut rh, mut rhn) = (ptr::null_mut(), 0, ptr::null_mut(), 0);
        let (mut op, mut opn, mut oh, mut ohn) = (ptr::null_mut(), 0, ptr::null_mut(), 0);
        rs::obs_extract_av1_headers(p, data.len(), &mut rp, &mut rpn, &mut rh, &mut rhn);
        c::oracle_obs_extract_av1_headers(p, data.len(), &mut op, &mut opn, &mut oh, &mut ohn);
        prop_assert_eq!((rpn, rhn), (opn, ohn), "extract sizes of {:02x?}", data);
        prop_assert_eq!(take(rp, rpn), take(op, opn));
        prop_assert_eq!(take(rh, rhn), take(oh, ohn));
    }
    Ok(())
}

fn check_metadata(payload: &[u8], metadata_type: u8) -> Result<(), TestCaseError> {
    let mut backing = payload.to_vec();
    backing.push(0xee);
    let data = &backing[..payload.len()];
    // SAFETY: both read `data` and write locals; each output is taken once.
    unsafe {
        let (mut r, mut rn, mut o, mut on) = (ptr::null_mut(), 0, ptr::null_mut(), 0);
        rs::metadata_obu(data.as_ptr(), data.len(), &mut r, &mut rn, metadata_type);
        c::oracle_metadata_obu(data.as_ptr(), data.len(), &mut o, &mut on, metadata_type);
        prop_assert_eq!(rn, on);
        let want = take(o, on);
        prop_assert_eq!(take(r, rn), want.clone());
        prop_assert_eq!(Some(av1::metadata_obu(data, metadata_type)), want);

        let (mut r, mut rn, mut o, mut on) = (ptr::null_mut(), 0, ptr::null_mut(), 0);
        rs::metadata_obu_itu_t35(data.as_ptr(), data.len(), &mut r, &mut rn);
        c::oracle_metadata_obu_itu_t35(data.as_ptr(), data.len(), &mut o, &mut on);
        prop_assert_eq!(rn, on);
        prop_assert_eq!(take(r, rn), take(o, on));
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(4096)))]

    #[test]
    fn av1_matches_c_oracle(data in input()) {
        check(&data)?;
    }

    #[test]
    fn metadata_obu_matches_c_oracle(
        payload in prop::collection::vec(any::<u8>(), 0..300),
        metadata_type in any::<u8>(),
    ) {
        check_metadata(&payload, metadata_type)?;
    }
}

/// Payload sizes around the leb128 byte boundaries (obu_size = n + 2).
#[test]
fn metadata_obu_size_boundaries_match_c_oracle() {
    for n in [
        0usize, 1, 124, 125, 126, 127, 16_380, 16_381, 16_382, 16_383,
    ] {
        check_metadata(&vec![0x5a; n], 6).unwrap();
    }
}

/// The cases from `test/cmocka/test_av1.c`, against the oracle, so the C
/// test's expectations (and the C bounds fix) are checked on every
/// platform.
#[test]
fn cmocka_cases_match_c_oracle() {
    let obu = |kind: u8, ext: u8, size: u8| (kind << 3) | (ext << 2) | (size << 1);
    let key = [
        obu(2, 0, 1),
        0,
        obu(1, 0, 1),
        2,
        0xaa,
        0xbb,
        obu(6, 0, 1),
        2,
        0x10,
        0xcc,
    ];
    let keyframes: [(&[u8], bool); 5] = [
        (&key, true),
        (&[obu(6, 0, 1), 2, 0x30, 0xcc], false),
        (&[obu(3, 0, 1), 1, 0x80], false),
        (&[obu(3, 0, 1), 1, 0x10], true),
        (&[obu(2, 0, 1), 0, obu(15, 0, 1), 1, 0], false),
    ];
    for (data, want) in keyframes {
        // SAFETY: reads only `data`.
        let got = unsafe { c::oracle_obs_av1_keyframe(data.as_ptr(), data.len()) };
        assert_eq!(got, want, "C keyframe of {data:02x?}");
        check(data).unwrap();
    }

    // (input, packet size, header size)
    let extracts: [(&[u8], usize, usize); 5] = [
        (
            &[
                obu(2, 0, 1),
                0,
                obu(1, 0, 1),
                1,
                0xaa,
                obu(5, 0, 1),
                1,
                0xbb,
                obu(6, 0, 1),
                1,
                0x10,
            ],
            11,
            6,
        ),
        (&[obu(6, 0, 1), 16, 0x10, 0x11], 4, 0),
        (&[obu(6, 0, 1), 0x80, 0x80, 0x80, 0x80, 0x01, 0x10], 7, 0),
        (&[obu(6, 1, 0), 0x00, 0x10, 0x11], 4, 0),
        (&[obu(6, 0, 1), 1, 0x10, obu(6, 1, 1)], 4, 0),
    ];
    for (data, packet_size, header_size) in extracts {
        let (mut p, mut pn, mut h, mut hn) = (ptr::null_mut(), 0, ptr::null_mut(), 0);
        // SAFETY: reads only `data`; outputs are taken once.
        unsafe {
            c::oracle_obs_extract_av1_headers(
                data.as_ptr(),
                data.len(),
                &mut p,
                &mut pn,
                &mut h,
                &mut hn,
            );
        }
        assert_eq!(
            (pn, hn),
            (packet_size, header_size),
            "C extract of {data:02x?}"
        );
        take(p, pn);
        take(h, hn);
        check(data).unwrap();
    }

    let (mut out, mut n) = (ptr::null_mut(), 0);
    // SAFETY: reads the payload; the output is taken once.
    unsafe { c::oracle_metadata_obu([1u8, 2, 3].as_ptr(), 3, &mut out, &mut n, 4) };
    assert_eq!(take(out, n).unwrap(), [0x2a, 5, 4, 1, 2, 3, 0x80]);
}
