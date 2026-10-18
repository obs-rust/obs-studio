//! Tier 3: the Rust C ABI shim and safe core behave exactly like the
//! original `libobs/obs-nal.c`, compiled as an oracle.
//!
//! Intentional difference (not run against C, which crashes): with fewer than
//! three bytes the C search computes `end - 3`, before the start of the
//! range. For a pointer within 3 bytes of address 0, such as `(NULL, NULL)`,
//! that wraps around and the C code reads address 0. The port returns `end`
//! for any empty or short range.
//!
//! The C search reads a word at a time once the pointer is 4-byte aligned,
//! so every case runs the same bytes at all 8 start offsets in an aligned
//! buffer.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use obs_c_oracle::nal as c;
use obs_codec::ffi::nal as rs;
use obs_codec::nal::{find_startcode, nal_headers, nal_units};
use proptest::prelude::*;

/// An 8-byte-aligned scratch buffer.
#[repr(C, align(8))]
struct Aligned([u8; 512]);

/// Runs `data` through the shim, the core and the C oracle, copied to
/// every start offset 0..8, and checks they agree.
fn check(data: &[u8]) -> Result<(), TestCaseError> {
    assert!(data.len() + 8 <= 512);
    let mut buf = Aligned([0xa5; 512]);
    for shift in 0..8 {
        buf.0[shift..shift + data.len()].copy_from_slice(data);
        let p = buf.0[shift..].as_ptr();
        // SAFETY: `[p, p + len)` lies inside `buf`.
        let end = unsafe { p.add(data.len()) };
        // SAFETY: both read only `[p, end)`.
        let (r, o) = unsafe {
            (
                rs::obs_nal_find_startcode(p, end),
                c::oracle_obs_nal_find_startcode(p, end),
            )
        };
        // SAFETY: both results lie in `[p, end]`.
        let (r, o) = unsafe { (r.offset_from(p), o.offset_from(p)) };
        prop_assert_eq!(r, o, "shim vs C at shift {} for {:?}", shift, data);
        prop_assert_eq!(
            find_startcode(data) as isize,
            o,
            "core vs C at shift {} for {:?}",
            shift,
            data
        );
    }
    Ok(())
}

/// Bytes biased toward start codes: mostly 0, 1 and a few others, so
/// `00 00 01`, `00 00 00 01` and near misses are common.
fn nal_bytes() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(
        prop_oneof![
            4 => Just(0u8),
            2 => Just(1u8),
            1 => Just(3u8),
            1 => any::<u8>(),
        ],
        0..80,
    )
}

/// Real-looking Annex B streams: start codes of both lengths around payloads.
fn annex_b() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(
        (any::<bool>(), prop::collection::vec(any::<u8>(), 0..12)),
        0..6,
    )
    .prop_map(|nals| {
        let mut v = Vec::new();
        for (long, payload) in nals {
            if long {
                v.push(0);
            }
            v.extend([0, 0, 1]);
            v.extend(payload);
        }
        v
    })
}

/// Long buffers of nonzero bytes with a few start codes and near misses
/// dropped in, so a search runs through many steps before it finds one.
fn sparse_codes() -> impl Strategy<Value = Vec<u8>> {
    let marks = prop::collection::vec((any::<prop::sample::Index>(), 0..4usize), 0..4);
    (prop::collection::vec(1u8..=255, 64..500), marks).prop_map(|(mut data, marks)| {
        for (at, kind) in marks {
            let pattern: &[u8] = match kind {
                0 => &[0, 0, 1],
                1 => &[0, 0, 0, 1],
                2 => &[0, 0, 2],
                _ => &[0, 1],
            };
            let at = at.index(data.len());
            for (k, &b) in pattern.iter().enumerate() {
                if at + k < data.len() {
                    data[at + k] = b;
                }
            }
        }
        data
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn find_startcode_matches_c_on_sparse_codes(data in sparse_codes()) {
        check(&data)?;
    }

    /// The lazy header walk yields the header of every unit of the full walk.
    #[test]
    fn headers_match_units(data in prop_oneof![nal_bytes(), annex_b(), sparse_codes()]) {
        let headers: Vec<u8> = nal_headers(&data).collect();
        let units: Vec<u8> = nal_units(&data).map(|nal| nal.header(&data)).collect();
        prop_assert_eq!(headers, units);
    }

    #[test]
    fn find_startcode_matches_c_on_biased_bytes(data in nal_bytes()) {
        check(&data)?;
    }

    #[test]
    fn find_startcode_matches_c_on_annex_b(data in annex_b()) {
        check(&data)?;
    }

    #[test]
    fn find_startcode_matches_c_on_any_bytes(data in prop::collection::vec(any::<u8>(), 0..200)) {
        check(&data)?;
    }
}

/// Every placement of a start code in a short buffer, so each position is
/// tried against the prologue, the word loop and the tail loop.
#[test]
fn every_position_matches_c() {
    for len in 0..40 {
        for pos in 0..len {
            for code in [&[0u8, 0, 1][..], &[0, 0, 0, 1]] {
                let mut data = vec![0x11u8; len];
                for (k, &b) in code.iter().enumerate() {
                    if pos + k < len {
                        data[pos + k] = b;
                    }
                }
                check(&data).unwrap();
            }
        }
    }
}

/// Every placement of a start code in buffers around one, two and three
/// 64-position steps, so each step of the search sees a code at each of its
/// offsets and at each distance from the end.
#[test]
fn every_position_in_long_buffers_matches_c() {
    for len in (60..=76).chain(124..=140).chain(188..=204) {
        for pos in 0..len {
            for code in [&[0u8, 0, 1][..], &[0, 0, 0, 1]] {
                let mut data = vec![0x11u8; len];
                for (k, &b) in code.iter().enumerate() {
                    if pos + k < len {
                        data[pos + k] = b;
                    }
                }
                check(&data).unwrap();
            }
        }
    }
}

/// Zeros that start no code (`00 02`, `00 00 02`, `00 00 00 02`) at every
/// position before a real code, in short buffers and around the first
/// 64-position step. A word that holds a zero but no code must not stop
/// the search, and the header walk must still find every unit (#1301).
#[test]
fn zeros_without_a_code_at_every_position_match_c() {
    for len in (1usize..=48).chain(60..=76) {
        for pos in 0..len {
            for decoy in [&[0u8, 2][..], &[0, 0, 2], &[0, 0, 0, 2]] {
                for code_at in [None, Some(len.saturating_sub(6))] {
                    let mut data = vec![0x11u8; len];
                    if let Some(at) = code_at {
                        data[at..]
                            .iter_mut()
                            .zip([0u8, 0, 1, 0x65])
                            .for_each(|(b, c)| *b = c);
                    }
                    for (k, &b) in decoy.iter().enumerate() {
                        if pos + k < len {
                            data[pos + k] = b;
                        }
                    }
                    check(&data).unwrap();
                    let headers: Vec<u8> = nal_headers(&data).collect();
                    let units: Vec<u8> = nal_units(&data).map(|nal| nal.header(&data)).collect();
                    assert_eq!(headers, units, "{data:02x?}");
                }
            }
        }
    }
}

/// An empty or reversed range returns `end` on both sides.
#[test]
fn empty_and_reversed_ranges_match_c() {
    let buf = [0u8, 0, 1, 5];
    let p = buf.as_ptr();
    // SAFETY: both pointers are inside `buf`; neither side reads when
    // `end <= p`.
    unsafe {
        let mid = p.add(2);
        assert_eq!(rs::obs_nal_find_startcode(p, p), p);
        assert_eq!(c::oracle_obs_nal_find_startcode(p, p), p);
        assert_eq!(rs::obs_nal_find_startcode(mid, p), p);
        assert_eq!(c::oracle_obs_nal_find_startcode(mid, p), p);
    }
}

/// The cases from `test/cmocka/test_nal.c`, against the oracle.
#[test]
fn cmocka_cases_match_c_oracle() {
    let cases: [(&[u8], usize); 8] = [
        (&[0, 0, 1, 0x65, 0xaa], 0),
        (&[0, 0, 0, 1, 0x67, 0xaa], 0),
        (&[0x11, 0x22, 0x33, 0, 0, 1, 0x41, 0x42], 3),
        (&[0x11, 0x22, 0, 0, 0, 1, 0x41, 0x42], 2),
        (&[0x11, 0, 0, 0, 0, 1, 0x41, 0x42], 2),
        (&[0x11, 0, 0x22, 0, 0, 0x33, 1, 0], 8),
        (&[0x11, 0x22, 0, 0, 1, 0x65], 2),
        (&[0x11, 0x22, 0, 0, 1], 5),
    ];
    for (data, want) in cases {
        let p = data.as_ptr();
        // SAFETY: reads only `data`.
        let got = unsafe {
            c::oracle_obs_nal_find_startcode(p, p.add(data.len())).offset_from(p) as usize
        };
        assert_eq!(got, want, "C oracle for {data:?}");
        check(data).unwrap();
    }
}

/// The port's side of the intentional difference: a NULL range returns
/// `end` (here NULL) without reading. C would read address 0.
#[test]
fn null_range_returns_end() {
    // SAFETY: `end <= p`, so nothing is read.
    let got = unsafe { rs::obs_nal_find_startcode(core::ptr::null(), core::ptr::null()) };
    assert!(got.is_null());
}
