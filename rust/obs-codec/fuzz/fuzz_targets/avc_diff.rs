//! Tier 3 fuzz target: cargo-fuzz differential of the Rust obs-avc port
//! (C ABI shims and safe core) against the original `libobs/obs-avc.c`
//! compiled as an oracle. Same comparison as `tests/avc_parity.rs`.
//!
//! Input layout: byte 0 = keyframe flag (bit 0) and priority (bits 1..3,
//! minus 2); then up to four little-endian i64 (pts, dts, timebase,
//! dts_usec); the rest is the bitstream.
//!
//! Run: `soldr cargo +nightly fuzz run avc_diff` from `rust/obs-codec`.
//!
//! Exclusions (C UB / crash, not a mismatch): NULL or near-NULL data with
//! size 0 (the C start-code search wraps `end - 3`). The harness always
//! passes a pointer inside a real allocation, so this is never generated.
//! The `get_ue_golomb` UB is fixed in C (#11, #17); no other input is
//! excluded.

#![no_main]

use core::ffi::c_long;
use core::mem::size_of;
use core::{ptr, slice};

use libfuzzer_sys::fuzz_target;
use obs_c_oracle::avc::{self as c, OracleEncoderPacket};
use obs_codec::avc;
use obs_codec::ffi::avc as rs;
use obs_codec::ffi::packet::encoder_packet;
use obs_util::ffi::darray::bfree;

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

fn check(input: &[u8], keyframe: bool, priority: i32, extra: [i64; 4]) {
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
        assert_eq!(
            rs::obs_avc_keyframe(p, data.len()),
            c::oracle_obs_avc_keyframe(p, data.len())
        );
        assert_eq!(
            avc::keyframe(data),
            c::oracle_obs_avc_keyframe(p, data.len())
        );
        let end = p.add(data.len());
        assert_eq!(
            rs::obs_avc_find_startcode(p, end),
            c::oracle_obs_avc_find_startcode(p, end)
        );

        // obs_parse_avc_packet_priority
        let src = packet(data, keyframe, priority, extra);
        let osrc = to_oracle(&src);
        assert_eq!(
            rs::obs_parse_avc_packet_priority(&src),
            c::oracle_obs_parse_avc_packet_priority(&osrc)
        );

        // obs_parse_avc_packet: every field, and the data block.
        let mut rd = packet(&[], false, 0, [0; 4]);
        let mut cd = to_oracle(&rd);
        rs::obs_parse_avc_packet(&mut rd, &src);
        c::oracle_obs_parse_avc_packet(&mut cd, &osrc);
        assert_eq!(rd.size, cd.size);
        assert_eq!(
            (rd.pts, rd.dts, rd.timebase_num, rd.timebase_den, rd.type_),
            (cd.pts, cd.dts, cd.timebase_num, cd.timebase_den, cd.type_)
        );
        assert_eq!(
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
        assert_eq!((rd.track_idx, rd.encoder), (cd.track_idx, cd.encoder));
        assert_eq!(take_packet(rd.data, rd.size), take_packet(cd.data, cd.size));

        // In place: the source packet is also the destination.
        let mut ri = src;
        let mut ci = osrc;
        let ri_ptr: *mut encoder_packet = &mut ri;
        let ci_ptr: *mut OracleEncoderPacket = &mut ci;
        rs::obs_parse_avc_packet(ri_ptr, ri_ptr);
        c::oracle_obs_parse_avc_packet(ci_ptr, ci_ptr);
        assert_eq!(
            (ri.size, ri.keyframe, ri.priority),
            (ci.size, ci.keyframe, ci.priority)
        );
        assert_eq!(take_packet(ri.data, ri.size), take_packet(ci.data, ci.size));

        // obs_parse_avc_header: return value, whether *header was written,
        // and its bytes.
        let sentinel = ptr::dangling_mut::<u8>();
        let mut rh = sentinel;
        let mut ch = sentinel;
        let rn = rs::obs_parse_avc_header(&mut rh, p, data.len());
        let cn = c::oracle_obs_parse_avc_header(&mut ch, p, data.len());
        assert_eq!(rn, cn);
        assert_eq!(rh == sentinel, ch == sentinel);
        if rh != sentinel {
            assert_eq!(take(rh, rn), take(ch, cn));
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
        assert_eq!(rn, on);
        for i in 0..3 {
            assert_eq!(take(r[i], rn[i]), take(o[i], on[i]));
        }
    }
}

fuzz_target!(|input: &[u8]| {
    let (flags, mut rest) = match input.split_first() {
        Some((f, r)) => (*f, r),
        None => (0, input),
    };
    let mut extra = [0i64; 4];
    for e in &mut extra {
        if rest.len() >= 8 {
            *e = i64::from_le_bytes(rest[..8].try_into().unwrap());
            rest = &rest[8..];
        }
    }
    let keyframe = flags & 1 != 0;
    let priority = i32::from((flags >> 1) & 7) - 2;
    check(rest, keyframe, priority, extra);
});
