use super::pair;
use criterion::Criterion;
use obs_c_oracle::encoder_packet::OracleEncoderPacket;
use obs_c_oracle::{av1 as c1, avc as cc, hevc as ch};
use obs_codec::ffi::packet::encoder_packet;
use obs_codec::ffi::{av1 as r1, avc as rc, hevc as rh};
use obs_util::ffi::bmem::bfree;
use std::{hint::black_box, ptr, slice};

type Keyframe = unsafe extern "C" fn(*const u8, usize) -> bool;
type Extract = unsafe extern "C" fn(
    *const u8,
    usize,
    *mut *mut u8,
    *mut usize,
    *mut *mut u8,
    *mut usize,
    *mut *mut u8,
    *mut usize,
);
type ExtractAv1 =
    unsafe extern "C" fn(*const u8, usize, *mut *mut u8, *mut usize, *mut *mut u8, *mut usize);
type Header = unsafe extern "C" fn(*mut *mut u8, *const u8, usize) -> usize;

#[derive(Debug, Default)]
struct Buffers {
    pointers: [*mut u8; 3],
    lengths: [usize; 3],
}

impl Buffers {
    fn bytes(&self, index: usize) -> &[u8] {
        if self.lengths[index] == 0 {
            return &[];
        }
        // SAFETY: parser outputs own readable buffers of the returned lengths.
        unsafe { slice::from_raw_parts(self.pointers[index], self.lengths[index]) }
    }
}

impl PartialEq for Buffers {
    fn eq(&self, other: &Self) -> bool {
        (0..3).all(|index| self.bytes(index) == other.bytes(index))
    }
}

impl Drop for Buffers {
    fn drop(&mut self) {
        for pointer in self.pointers {
            // SAFETY: parser outputs use the shared libobs allocator; each
            // independent output is released once. bfree accepts NULL.
            unsafe { bfree(pointer.cast()) };
        }
    }
}

fn extract(function: Extract, input: &[u8]) -> Buffers {
    let mut output = Buffers::default();
    let [p, h, s] = &mut output.pointers;
    let [pn, hn, sn] = &mut output.lengths;
    // SAFETY: nonempty input remains readable; all distinct output slots are
    // initialized and writable. Returned allocations are owned by Buffers.
    unsafe { function(black_box(input.as_ptr()), input.len(), p, pn, h, hn, s, sn) };
    output
}

fn extract_av1(function: ExtractAv1, input: &[u8]) -> Buffers {
    let mut output = Buffers::default();
    let [p, h, _] = &mut output.pointers;
    let [pn, hn, _] = &mut output.lengths;
    // SAFETY: same input and independent output-slot contracts as extract.
    unsafe { function(black_box(input.as_ptr()), input.len(), p, pn, h, hn) };
    output
}

fn header(function: Header, input: &[u8]) -> Buffers {
    let mut output = Buffers::default();
    // SAFETY: input is readable; the initialized output pointer is writable.
    output.lengths[0] = unsafe {
        function(
            &mut output.pointers[0],
            black_box(input.as_ptr()),
            input.len(),
        )
    };
    output
}

fn annex_b(hevc: bool, payload_size: usize) -> Vec<u8> {
    let mut input = Vec::new();
    let kinds: &[u8] = if hevc {
        &[32, 33, 34, 39, 19]
    } else {
        &[7, 8, 6, 5]
    };
    for &kind in kinds {
        input.extend_from_slice(&[0, 0, 0, 1]);
        if hevc {
            input.extend_from_slice(&[kind << 1, 1]);
        } else {
            input.push(0x60 | kind);
        }
        // SPS/PPS headers have at least the bytes the AVC header parser reads.
        input.extend(std::iter::repeat_n(
            0x55,
            if kind == *kinds.last().unwrap() {
                payload_size
            } else {
                16
            },
        ));
    }
    input
}

fn obu(payload_size: usize) -> Vec<u8> {
    let mut input = vec![0x0a, 2, 0, 0, 0x2a, 2, 4, 5, 0x32];
    let mut size = payload_size;
    loop {
        let byte = (size & 0x7f) as u8;
        size >>= 7;
        input.push(byte | if size != 0 { 0x80 } else { 0 });
        if size == 0 {
            break;
        }
    }
    input.extend(std::iter::repeat_n(0, payload_size));
    input
}

#[derive(Debug, PartialEq)]
struct ParsedPacket {
    data: Buffers,
    pts: i64,
    dts: i64,
    keyframe: bool,
    priority: i32,
    drop_priority: i32,
}

fn parsed(
    data: *mut u8,
    size: usize,
    pts: i64,
    dts: i64,
    keyframe: bool,
    priority: i32,
    drop_priority: i32,
) -> ParsedPacket {
    assert!(!data.is_null());
    let prefix = std::mem::size_of::<std::ffi::c_long>();
    // SAFETY: parsed AVC/HEVC data is prefix bytes into a bmalloc block,
    // containing a C long reference count followed by size payload bytes.
    let pointer = unsafe { data.sub(prefix) };
    ParsedPacket {
        data: Buffers {
            pointers: [pointer, ptr::null_mut(), ptr::null_mut()],
            lengths: [prefix + size, 0, 0],
        },
        pts,
        dts,
        keyframe,
        priority,
        drop_priority,
    }
}

fn packets(criterion: &mut Criterion, name: &str, input: &[u8], hevc: bool) {
    let source = encoder_packet {
        data: input.as_ptr().cast_mut(),
        size: input.len(),
        pts: 123,
        dts: 100,
        timebase_num: 1,
        timebase_den: 60,
        type_: 0,
        keyframe: false,
        dts_usec: 100000,
        sys_dts_usec: 100000,
        priority: 0,
        drop_priority: 0,
        track_idx: 0,
        encoder: ptr::null_mut(),
    };
    let original = OracleEncoderPacket {
        data: source.data,
        size: source.size,
        pts: source.pts,
        dts: source.dts,
        timebase_num: source.timebase_num,
        timebase_den: source.timebase_den,
        type_: source.type_,
        keyframe: source.keyframe,
        dts_usec: source.dts_usec,
        sys_dts_usec: source.sys_dts_usec,
        priority: source.priority,
        drop_priority: source.drop_priority,
        track_idx: source.track_idx,
        encoder: source.encoder,
    };
    pair(
        criterion,
        &format!("{name}/packet/{}", input.len()),
        || {
            let mut output = source;
            // SAFETY: source data is readable for size bytes, output is distinct
            // and writable. Its allocated payload is adopted by ParsedPacket.
            unsafe {
                if hevc {
                    rh::obs_parse_hevc_packet(&mut output, black_box(&source));
                } else {
                    rc::obs_parse_avc_packet(&mut output, black_box(&source));
                }
            }
            parsed(
                output.data,
                output.size,
                output.pts,
                output.dts,
                output.keyframe,
                output.priority,
                output.drop_priority,
            )
        },
        || {
            let mut output = original;
            // SAFETY: same input and distinct output contracts for the C packet.
            unsafe {
                if hevc {
                    ch::oracle_obs_parse_hevc_packet(&mut output, black_box(&original));
                } else {
                    cc::oracle_obs_parse_avc_packet(&mut output, black_box(&original));
                }
            }
            parsed(
                output.data,
                output.size,
                output.pts,
                output.dts,
                output.keyframe,
                output.priority,
                output.drop_priority,
            )
        },
    );
    pair(
        criterion,
        &format!("{name}/priority/{}", input.len()),
        || {
            // SAFETY: the packet and all its nonempty bytes remain readable.
            unsafe {
                if hevc {
                    rh::obs_parse_hevc_packet_priority(black_box(&source))
                } else {
                    rc::obs_parse_avc_packet_priority(black_box(&source))
                }
            }
        },
        || {
            // SAFETY: same readable packet and data for the original C parser.
            unsafe {
                if hevc {
                    ch::oracle_obs_parse_hevc_packet_priority(black_box(&original))
                } else {
                    cc::oracle_obs_parse_avc_packet_priority(black_box(&original))
                }
            }
        },
    );
}

#[expect(clippy::too_many_lines, reason = "benchmark table: one row per ported unit")]
pub fn benchmarks(criterion: &mut Criterion) {
    for size in [256, 16384] {
        let avc = annex_b(false, size);
        let hevc = annex_b(true, size);
        let av1 = obu(size);
        packets(criterion, "avc", &avc, false);
        packets(criterion, "hevc", &hevc, true);
        for (name, input, rust, original) in [
            (
                "avc",
                &avc,
                rc::obs_avc_keyframe as Keyframe,
                cc::oracle_obs_avc_keyframe as Keyframe,
            ),
            (
                "hevc",
                &hevc,
                rh::obs_hevc_keyframe as Keyframe,
                ch::oracle_obs_hevc_keyframe as Keyframe,
            ),
            (
                "av1",
                &av1,
                r1::obs_av1_keyframe as Keyframe,
                c1::oracle_obs_av1_keyframe as Keyframe,
            ),
        ] {
            pair(
                criterion,
                &format!("{name}/keyframe/{size}"),
                || {
                    // SAFETY: the complete input is readable and nonempty.
                    unsafe { rust(black_box(input.as_ptr()), input.len()) }
                },
                || {
                    // SAFETY: same nonempty readable input for the C oracle.
                    unsafe { original(black_box(input.as_ptr()), input.len()) }
                },
            );
        }
        pair(
            criterion,
            &format!("avc/extract/{size}"),
            || extract(rc::obs_extract_avc_headers, &avc),
            || extract(cc::oracle_obs_extract_avc_headers, &avc),
        );
        pair(
            criterion,
            &format!("hevc/extract/{size}"),
            || extract(rh::obs_extract_hevc_headers, &hevc),
            || extract(ch::oracle_obs_extract_hevc_headers, &hevc),
        );
        pair(
            criterion,
            &format!("av1/extract/{size}"),
            || extract_av1(r1::obs_extract_av1_headers, &av1),
            || extract_av1(c1::oracle_obs_extract_av1_headers, &av1),
        );
        pair(
            criterion,
            &format!("avc/header/{size}"),
            || header(rc::obs_parse_avc_header, &avc),
            || header(cc::oracle_obs_parse_avc_header, &avc),
        );
        let mut scan = vec![0x55; size];
        scan.extend_from_slice(&[0, 0, 0, 1, 0x65, 0x55]);
        pair(
            criterion,
            &format!("avc/startcode/{size}"),
            || {
                // SAFETY: both pointers bound the same live allocation.
                unsafe {
                    rc::obs_avc_find_startcode(
                        black_box(scan.as_ptr()),
                        scan.as_ptr().add(scan.len()),
                    )
                    .offset_from(scan.as_ptr())
                }
            },
            || {
                // SAFETY: nonempty Annex B input contains a start code; result
                // and bounds belong to the same allocation.
                unsafe {
                    cc::oracle_obs_avc_find_startcode(
                        black_box(scan.as_ptr()),
                        scan.as_ptr().add(scan.len()),
                    )
                    .offset_from(scan.as_ptr())
                }
            },
        );
    }
    // Both metadata entry points allocate a complete OBU around this payload.
    let payload = vec![0x55; 256];
    pair(
        criterion,
        "av1/metadata",
        || {
            let mut output = Buffers::default();
            // SAFETY: payload is readable and output slots are writable.
            unsafe {
                r1::metadata_obu(
                    black_box(payload.as_ptr()),
                    payload.len(),
                    &mut output.pointers[0],
                    &mut output.lengths[0],
                    4,
                )
            };
            output
        },
        || {
            let mut output = Buffers::default();
            // SAFETY: same readable payload and initialized output slots.
            unsafe {
                c1::oracle_metadata_obu(
                    black_box(payload.as_ptr()),
                    payload.len(),
                    &mut output.pointers[0],
                    &mut output.lengths[0],
                    4,
                )
            };
            output
        },
    );
    pair(
        criterion,
        "av1/metadata_itu_t35",
        || {
            let mut output = Buffers::default();
            // SAFETY: payload is readable and output slots are writable.
            unsafe {
                r1::metadata_obu_itu_t35(
                    black_box(payload.as_ptr()),
                    payload.len(),
                    &mut output.pointers[0],
                    &mut output.lengths[0],
                )
            };
            output
        },
        || {
            let mut output = Buffers::default();
            // SAFETY: same readable payload and initialized output slots.
            unsafe {
                c1::oracle_metadata_obu_itu_t35(
                    black_box(payload.as_ptr()),
                    payload.len(),
                    &mut output.pointers[0],
                    &mut output.lengths[0],
                )
            };
            output
        },
    );
}
