//! Tier 3: the Rust C ABI shims and safe core behave exactly like the
//! original `libobs/obs-hevc.c`, compiled as an oracle (with the oracle
//! copies of obs-nal.c and array-serializer.c under it).
//!
//! Intentional difference (not exercised here, C crashes): as for AVC, NULL
//! data with size 0 gives empty results, where the C start-code search
//! reads address 0.
//!
//! Every output buffer is compared byte for byte, including the `long`
//! reference count in front of a parsed packet, and then freed.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use core::ffi::c_long;
use core::mem::size_of;
use core::{ptr, slice};

use obs_c_oracle::hevc::{self as c, OracleEncoderPacket};
use obs_codec::ffi::hevc as rs;
use obs_codec::ffi::packet::encoder_packet;
use obs_codec::hevc;
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

/// One NAL unit: start-code length, two header bytes, payload. The first
/// header byte is `forbidden | type << 1 | layer_id high bit`.
fn nal() -> impl Strategy<Value = Vec<u8>> {
    let kind = prop_oneof![
        3 => 16u8..24,
        3 => prop_oneof![Just(1u8), Just(3), Just(5), Just(7), Just(9)],
        2 => prop_oneof![Just(0u8), Just(2), Just(4), Just(6), Just(8)],
        2 => 32u8..41,
        1 => 24u8..32,
        1 => 0u8..64,
    ];
    (
        any::<bool>(),
        kind,
        any::<bool>(),
        any::<bool>(),
        any::<u8>(),
        prop::collection::vec(
            prop_oneof![3 => any::<u8>(), 1 => Just(0u8), 1 => Just(3u8)],
            0..16,
        ),
    )
        .prop_map(|(long, kind, forbidden, layer, second, payload)| {
            let mut v = if long {
                vec![0, 0, 0, 1]
            } else {
                vec![0, 0, 1]
            };
            v.push((u8::from(forbidden) << 7) | (kind << 1) | u8::from(layer));
            v.push(second);
            v.extend(payload);
            v
        })
}

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

fn input() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        5 => stream(),
        1 => prop::collection::vec(any::<u8>(), 0..64),
    ]
}

fn packet(data: &[u8], keyframe: bool, priority: i32, extra: [i64; 2]) -> encoder_packet {
    encoder_packet {
        data: data.as_ptr().cast_mut(),
        size: data.len(),
        pts: extra[0],
        dts: extra[1],
        timebase_num: 1,
        timebase_den: 60,
        type_: 1,
        keyframe,
        dts_usec: extra[0] ^ 0x1234,
        sys_dts_usec: extra[1] ^ 0x5678,
        priority,
        drop_priority: -7,
        track_idx: 2,
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

fn take_packet(data: *mut u8, size: usize) -> Vec<u8> {
    // SAFETY: `data` points `sizeof(long)` bytes into a bmalloc block.
    let block = unsafe { data.sub(size_of::<c_long>()) };
    take(block, size_of::<c_long>() + size).expect("parsed packet has data")
}

fn check(
    input: &[u8],
    keyframe: bool,
    priority: i32,
    extra: [i64; 2],
) -> Result<(), TestCaseError> {
    // Keep even an empty input inside a real allocation (see the AVC
    // parity test for the C start-code search's NULL-range read).
    let mut backing = input.to_vec();
    backing.push(0xee);
    let data = &backing[..input.len()];
    let p = data.as_ptr();
    // SAFETY: every call reads only `data` and writes only locals; each
    // returned buffer is taken (and freed) once.
    unsafe {
        let want = c::oracle_obs_hevc_keyframe(p, data.len());
        prop_assert_eq!(rs::obs_hevc_keyframe(p, data.len()), want);
        prop_assert_eq!(hevc::keyframe(data), want);

        let src = packet(data, keyframe, priority, extra);
        let osrc = to_oracle(&src);
        prop_assert_eq!(
            rs::obs_parse_hevc_packet_priority(&src),
            c::oracle_obs_parse_hevc_packet_priority(&osrc)
        );

        let mut rd = packet(&[], false, 0, [0; 2]);
        let mut cd = to_oracle(&rd);
        rs::obs_parse_hevc_packet(&mut rd, &src);
        c::oracle_obs_parse_hevc_packet(&mut cd, &osrc);
        prop_assert_eq!(rd.size, cd.size);
        prop_assert_eq!(
            (rd.keyframe, rd.priority, rd.drop_priority),
            (cd.keyframe, cd.priority, cd.drop_priority)
        );
        prop_assert_eq!(
            (
                rd.pts,
                rd.dts,
                rd.dts_usec,
                rd.sys_dts_usec,
                rd.track_idx,
                rd.encoder
            ),
            (
                cd.pts,
                cd.dts,
                cd.dts_usec,
                cd.sys_dts_usec,
                cd.track_idx,
                cd.encoder
            )
        );
        prop_assert_eq!(take_packet(rd.data, rd.size), take_packet(cd.data, cd.size));

        let mut ri = src;
        let mut ci = osrc;
        let ri_ptr: *mut encoder_packet = &mut ri;
        let ci_ptr: *mut OracleEncoderPacket = &mut ci;
        rs::obs_parse_hevc_packet(ri_ptr, ri_ptr);
        c::oracle_obs_parse_hevc_packet(ci_ptr, ci_ptr);
        prop_assert_eq!(
            (ri.size, ri.keyframe, ri.priority),
            (ci.size, ci.keyframe, ci.priority)
        );
        prop_assert_eq!(take_packet(ri.data, ri.size), take_packet(ci.data, ci.size));

        let mut r = [ptr::null_mut::<u8>(); 3];
        let mut rn = [0usize; 3];
        let mut o = [ptr::null_mut::<u8>(); 3];
        let mut on = [0usize; 3];
        rs::obs_extract_hevc_headers(
            p,
            data.len(),
            &mut r[0],
            &mut rn[0],
            &mut r[1],
            &mut rn[1],
            &mut r[2],
            &mut rn[2],
        );
        c::oracle_obs_extract_hevc_headers(
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
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(4096)))]

    #[test]
    fn hevc_matches_c_oracle(
        data in input(),
        keyframe in any::<bool>(),
        priority in -2i32..6,
        extra in any::<[i64; 2]>(),
    ) {
        check(&data, keyframe, priority, extra)?;
    }
}

/// Every unit type 0..64 alone, then followed by an IDR, so each type's
/// keyframe, priority and bucket is compared at least once.
#[test]
fn every_unit_type_matches_c_oracle() {
    for kind in 0u8..64 {
        let alone = [0, 0, 0, 1, kind << 1, 0x01, 0xab];
        check(&alone, false, 0, [1, 2]).unwrap();
        let then_idr = [0, 0, 1, kind << 1, 0x01, 0xab, 0, 0, 1, 19 << 1, 0x01, 0xcd];
        check(&then_idr, false, 0, [1, 2]).unwrap();
    }
}
