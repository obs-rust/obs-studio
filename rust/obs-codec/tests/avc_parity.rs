//! Tier 3: the Rust C ABI shims and safe core behave exactly like the
//! original `libobs/obs-avc.c`, compiled as an oracle (with the oracle
//! copies of obs-nal.c, array-serializer.c and bitstream.c under it).
//!
//! Intentional difference (not exercised here, C crashes): the C start-code
//! search reads address 0 for a range starting within 3 bytes of it, such
//! as `(NULL, 0)`, so every function here crashes in C on NULL data with size
//! 0; the Rust port returns `end` for any empty range and gives empty
//! results.
//!
//! Every output buffer is compared byte
//! for byte, including the `long` reference count `obs_parse_avc_packet`
//! puts in front of the packet data, and then freed with `bfree`.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use core::ffi::c_long;
use core::mem::size_of;
use core::{ptr, slice};

use obs_c_oracle::avc::{self as c, OracleEncoderPacket};
use obs_codec::avc;
use obs_codec::ffi::avc::{self as rs, encoder_packet};
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

/// One NAL unit: start-code length, header byte, payload.
fn nal() -> impl Strategy<Value = Vec<u8>> {
    let kind = prop_oneof![
        3 => Just(1u8),
        3 => Just(5u8),
        2 => Just(6u8),
        2 => Just(7u8),
        2 => Just(8u8),
        1 => Just(9u8),
        1 => 0u8..32,
    ];
    (
        any::<bool>(),
        0u8..8,
        kind,
        prop::collection::vec(
            prop_oneof![3 => any::<u8>(), 1 => Just(0u8), 1 => Just(3u8)],
            0..24,
        ),
    )
        .prop_map(|(long, high, kind, payload)| {
            let mut v = if long {
                vec![0, 0, 0, 1]
            } else {
                vec![0, 0, 1]
            };
            v.push((high << 5) | kind);
            v.extend(payload);
            v
        })
}

/// An Annex B stream, sometimes with garbage before the first start code
/// or a truncated tail.
fn stream() -> impl Strategy<Value = Vec<u8>> {
    (
        prop::collection::vec(any::<u8>(), 0..4),
        prop::collection::vec(nal(), 0..6),
        prop::option::of(0usize..8),
    )
        .prop_map(|(prefix, nals, cut)| {
            let mut v = prefix;
            for n in nals {
                v.extend(n);
            }
            if let Some(cut) = cut {
                v.truncate(v.len().saturating_sub(cut));
            }
            v
        })
}

/// A High-profile header: SPS (profile 100/110/122/244, random or sparse
/// bits that exercise the Exp-Golomb reader and its cap) and a PPS.
fn high_header() -> impl Strategy<Value = Vec<u8>> {
    (
        prop_oneof![
            Just(100u8),
            Just(110u8),
            Just(122u8),
            Just(244u8),
            any::<u8>()
        ],
        prop::collection::vec(
            prop_oneof![2 => any::<u8>(), 2 => Just(0u8), 1 => Just(3u8)],
            0..20,
        ),
        prop::collection::vec(any::<u8>(), 0..6),
        any::<bool>(),
    )
        .prop_map(|(profile, sps_rest, pps_rest, sps_last)| {
            let mut sps = vec![0, 0, 0, 1, 0x67, profile, 0x00, 0x28];
            sps.extend(sps_rest);
            let mut pps = vec![0, 0, 0, 1, 0x68];
            pps.extend(pps_rest);
            if sps_last {
                [pps, sps].concat()
            } else {
                [sps, pps].concat()
            }
        })
}

fn input() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        4 => stream(),
        3 => high_header(),
        1 => prop::collection::vec(any::<u8>(), 0..64),
    ]
}

fn packet(data: &[u8], keyframe: bool, priority: i32, extra: [i64; 4]) -> encoder_packet {
    encoder_packet {
        data: data.as_ptr().cast_mut(),
        size: data.len(),
        pts: extra[0],
        dts: extra[1],
        timebase_num: extra[2] as i32,
        timebase_den: (extra[2] >> 32) as i32,
        type_: 1,
        keyframe,
        dts_usec: extra[3],
        sys_dts_usec: extra[3] ^ 0x5555,
        priority,
        drop_priority: -7,
        track_idx: 3,
        encoder: ptr::dangling_mut(),
    }
}

fn to_oracle(p: &encoder_packet) -> OracleEncoderPacket {
    OracleEncoderPacket {
        data: p.data,
        size: p.size,
        pts: p.pts,
        dts: p.dts,
        timebase_num: p.timebase_num,
        timebase_den: p.timebase_den,
        type_: p.type_,
        keyframe: p.keyframe,
        dts_usec: p.dts_usec,
        sys_dts_usec: p.sys_dts_usec,
        priority: p.priority,
        drop_priority: p.drop_priority,
        track_idx: p.track_idx,
        encoder: p.encoder,
    }
}

/// The bytes of a parsed packet, refcount prefix included, then freed as
/// libobs frees it (`bfree(data - sizeof(long))`).
fn take_packet(data: *mut u8, size: usize) -> Vec<u8> {
    // SAFETY: `data` points `sizeof(long)` bytes into a bmalloc block.
    let block = unsafe { data.sub(size_of::<c_long>()) };
    take(block, size_of::<c_long>() + size).expect("parsed packet has data")
}

fn check(
    input: &[u8],
    keyframe: bool,
    priority: i32,
    extra: [i64; 4],
) -> Result<(), TestCaseError> {
    // Keep even an empty input inside a real allocation. The C start-code
    // search computes `end - 3`, which wraps for a pointer within 3 bytes of
    // address 0 (an empty Vec's dangling pointer, or NULL), and then reads
    // there; the Rust port returns `end` for any empty range.
    let mut backing = input.to_vec();
    backing.push(0xee);
    let data = &backing[..input.len()];
    let p = data.as_ptr();
    // SAFETY: every call below reads only `data` and writes only locals;
    // each returned buffer is taken (and freed) once.
    unsafe {
        prop_assert_eq!(
            rs::obs_avc_keyframe(p, data.len()),
            c::oracle_obs_avc_keyframe(p, data.len())
        );
        prop_assert_eq!(
            avc::keyframe(data),
            c::oracle_obs_avc_keyframe(p, data.len())
        );
        let end = p.add(data.len());
        prop_assert_eq!(
            rs::obs_avc_find_startcode(p, end),
            c::oracle_obs_avc_find_startcode(p, end)
        );

        // obs_parse_avc_packet_priority
        let src = packet(data, keyframe, priority, extra);
        let osrc = to_oracle(&src);
        prop_assert_eq!(
            rs::obs_parse_avc_packet_priority(&src),
            c::oracle_obs_parse_avc_packet_priority(&osrc)
        );

        // obs_parse_avc_packet: every field, and the data block.
        let mut rd = packet(&[], false, 0, [0; 4]);
        let mut cd = to_oracle(&rd);
        rs::obs_parse_avc_packet(&mut rd, &src);
        c::oracle_obs_parse_avc_packet(&mut cd, &osrc);
        prop_assert_eq!(rd.size, cd.size);
        prop_assert_eq!(
            (rd.pts, rd.dts, rd.timebase_num, rd.timebase_den, rd.type_),
            (cd.pts, cd.dts, cd.timebase_num, cd.timebase_den, cd.type_)
        );
        prop_assert_eq!(
            (
                rd.keyframe,
                rd.priority,
                rd.drop_priority,
                rd.dts_usec,
                rd.sys_dts_usec
            ),
            (
                cd.keyframe,
                cd.priority,
                cd.drop_priority,
                cd.dts_usec,
                cd.sys_dts_usec
            )
        );
        prop_assert_eq!((rd.track_idx, rd.encoder), (cd.track_idx, cd.encoder));
        prop_assert_eq!(take_packet(rd.data, rd.size), take_packet(cd.data, cd.size));

        // In place: the source packet is also the destination.
        let mut ri = src;
        let mut ci = osrc;
        let ri_ptr: *mut encoder_packet = &mut ri;
        let ci_ptr: *mut OracleEncoderPacket = &mut ci;
        rs::obs_parse_avc_packet(ri_ptr, ri_ptr);
        c::oracle_obs_parse_avc_packet(ci_ptr, ci_ptr);
        prop_assert_eq!(
            (ri.size, ri.keyframe, ri.priority),
            (ci.size, ci.keyframe, ci.priority)
        );
        prop_assert_eq!(take_packet(ri.data, ri.size), take_packet(ci.data, ci.size));

        // obs_parse_avc_header: return value, whether *header was written,
        // and its bytes.
        let sentinel = ptr::dangling_mut::<u8>();
        let mut rh = sentinel;
        let mut ch = sentinel;
        let rn = rs::obs_parse_avc_header(&mut rh, p, data.len());
        let cn = c::oracle_obs_parse_avc_header(&mut ch, p, data.len());
        prop_assert_eq!(rn, cn);
        prop_assert_eq!(rh == sentinel, ch == sentinel);
        if rh != sentinel {
            prop_assert_eq!(take(rh, rn), take(ch, cn));
        }

        // obs_extract_avc_headers: three outputs, NULL when empty.
        let mut r = [ptr::null_mut::<u8>(); 3];
        let mut rn = [0usize; 3];
        let mut o = [ptr::null_mut::<u8>(); 3];
        let mut on = [0usize; 3];
        rs::obs_extract_avc_headers(
            p,
            data.len(),
            &mut r[0],
            &mut rn[0],
            &mut r[1],
            &mut rn[1],
            &mut r[2],
            &mut rn[2],
        );
        c::oracle_obs_extract_avc_headers(
            p,
            data.len(),
            &mut o[0],
            &mut on[0],
            &mut o[1],
            &mut on[1],
            &mut o[2],
            &mut on[2],
        );
        prop_assert_eq!(rn, on);
        for i in 0..3 {
            prop_assert_eq!(take(r[i], rn[i]), take(o[i], on[i]));
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn avc_matches_c_oracle(
        data in input(),
        keyframe in any::<bool>(),
        priority in -2i32..6,
        extra in any::<[i64; 4]>(),
    ) {
        check(&data, keyframe, priority, extra)?;
    }
}

/// `sizeof(long)` is the size of the reference count in front of a parsed
/// packet: 4 on Windows, 8 on Linux and macOS.
#[test]
fn parsed_packet_reference_count_is_a_c_long() {
    // SAFETY: a constant query.
    assert_eq!(
        unsafe { obs_c_oracle::encoder_packet::oracle_sizeof_long() },
        size_of::<c_long>()
    );
    let data = [0u8, 0, 1, 0x65, 0x88];
    let src = packet(&data, false, 0, [0; 4]);
    let mut out = packet(&[], false, 0, [0; 4]);
    // SAFETY: `src` reads `data`; the block is freed once.
    unsafe { rs::obs_parse_avc_packet(&mut out, &src) };
    let block = take_packet(out.data, out.size);
    assert_eq!(&block[..size_of::<c_long>()], &(1 as c_long).to_ne_bytes());
    assert_eq!(&block[size_of::<c_long>()..], &[0, 0, 0, 2, 0x65, 0x88]);
}

/// The case from `test/cmocka/test_avc.c` (#17): a High-profile SPS
/// truncated so the Exp-Golomb reader runs past the end.
#[test]
fn truncated_high_sps_matches_c_oracle() {
    let data = [0u8, 0, 0, 1, 0x67, 0x64, 0x00, 0x1f, 0, 0, 0, 1, 0x68, 0xee];
    check(&data, false, 0, [1, 2, 3, 4]).unwrap();
}

/// SPS and PPS over 65535 bytes: their sizes are written as `uint16_t`, so
/// they wrap in the header record.
#[test]
fn oversized_parameter_sets_match_c_oracle() {
    let mut data = vec![0, 0, 0, 1, 0x67, 0x42, 0x00, 0x1f];
    data.extend(std::iter::repeat_n(0x5a, 65_540));
    data.extend([0, 0, 0, 1, 0x68]);
    data.extend(std::iter::repeat_n(0xa5, 65_537));
    check(&data, false, 0, [0; 4]).unwrap();
}
