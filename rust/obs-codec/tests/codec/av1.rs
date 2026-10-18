//! Tier 1: the safe AV1 helpers, mirroring `test/cmocka/test_av1.c`.

use obs_c_oracle as _;
use obs_codec::av1::{
    OBU_FRAME, OBU_FRAME_HEADER, OBU_METADATA, OBU_PADDING, OBU_SEQUENCE_HEADER,
    OBU_TEMPORAL_DELIMITER, extract_headers, keyframe, metadata_obu, obus,
};
use proptest as _;

fn obu(kind: u8, ext: bool, has_size: bool) -> u8 {
    (kind << 3) | (u8::from(ext) << 2) | (u8::from(has_size) << 1)
}

#[test]
fn keyframe_from_the_first_frame_obu() {
    let td = [obu(OBU_TEMPORAL_DELIMITER, false, true), 0];
    let seq = [obu(OBU_SEQUENCE_HEADER, false, true), 2, 0xaa, 0xbb];
    let key = [obu(OBU_FRAME, false, true), 2, 0x10, 0xcc];
    assert!(keyframe(&[&td[..], &seq, &key].concat()));
    assert!(!keyframe(&[obu(OBU_FRAME, false, true), 2, 0x30, 0xcc]));
    assert!(!keyframe(&[obu(OBU_FRAME_HEADER, false, true), 1, 0x80]));
    assert!(keyframe(&[obu(OBU_FRAME_HEADER, false, true), 1, 0x10]));
    assert!(!keyframe(&[
        obu(OBU_TEMPORAL_DELIMITER, false, true),
        0,
        obu(OBU_PADDING, false, true),
        1,
        0
    ]));
    assert!(!keyframe(&[]));
}

#[test]
fn extract_copies_headers_and_keeps_everything() {
    let data = [
        obu(OBU_TEMPORAL_DELIMITER, false, true),
        0,
        obu(OBU_SEQUENCE_HEADER, false, true),
        1,
        0xaa,
        obu(OBU_METADATA, false, true),
        1,
        0xbb,
        obu(OBU_FRAME, false, true),
        1,
        0x10,
    ];
    let out = extract_headers(&data);
    assert_eq!(out.header, &data[2..8]);
    assert_eq!(out.packet, data);
}

#[test]
fn metadata_obu_layout() {
    assert_eq!(
        metadata_obu(&[1, 2, 3], 4),
        [obu(OBU_METADATA, false, true), 5, 4, 1, 2, 3, 0x80]
    );
    let big = metadata_obu(&[0x5a; 200], 6);
    assert_eq!(big.len(), 205);
    assert_eq!(&big[1..4], &[0xca, 0x01, 6]);
    assert_eq!(big[204], 0x80);
}

#[test]
fn oversized_obu_is_cut_at_the_end() {
    let data = [obu(OBU_FRAME, false, true), 16, 0x10, 0x11];
    let all: Vec<_> = obus(&data)
        .map(|(at, o)| (at, o.header_len, o.size))
        .collect();
    assert_eq!(all, [(0, 2, 2)]);
    assert_eq!(extract_headers(&data).packet, data);
    assert!(keyframe(&data));
}

#[test]
fn long_leb128_is_read_in_64_bits_then_cut() {
    let data = [
        obu(OBU_FRAME, false, true),
        0x80,
        0x80,
        0x80,
        0x80,
        0x01,
        0x10,
    ];
    let all: Vec<_> = obus(&data)
        .map(|(at, o)| (at, o.header_len, o.size))
        .collect();
    assert_eq!(all, [(0, 6, 1)]);
}

#[test]
fn extension_without_size_field() {
    let data = [obu(OBU_FRAME, true, false), 0x00, 0x10, 0x11];
    let all: Vec<_> = obus(&data)
        .map(|(at, o)| (at, o.header_len, o.size))
        .collect();
    assert_eq!(all, [(0, 2, 2)]);
    assert!(keyframe(&data));
}

#[test]
fn lone_header_with_missing_extension_byte() {
    let data = [
        obu(OBU_FRAME, false, true),
        1,
        0x10,
        obu(OBU_FRAME, true, true),
    ];
    let all: Vec<_> = obus(&data)
        .map(|(at, o)| (at, o.header_len, o.size))
        .collect();
    assert_eq!(all, [(0, 2, 1), (3, 1, 0)]);
    assert_eq!(extract_headers(&data).packet, data);
    assert!(!keyframe(&[obu(OBU_FRAME, true, true)]));
}

#[test]
fn size_field_after_the_extension_byte() {
    // Header, extension byte 0xff, two-byte size field 2: the payload
    // starts at 4. Read from byte 1, 0xff would continue the size field.
    let key = [obu(OBU_FRAME, true, true), 0xff, 0x82, 0x00, 0x10, 0x11];
    let all: Vec<_> = obus(&key)
        .map(|(at, o)| (at, o.header_len, o.size))
        .collect();
    assert_eq!(all, [(0, 4, 2)]);
    assert!(keyframe(&key));
    let inter = [obu(OBU_FRAME, true, true), 0x00, 0x81, 0x00, 0x30];
    assert!(!keyframe(&inter));
}

#[test]
fn walk_alternates_obus_with_and_without_extension() {
    let data = [
        obu(OBU_TEMPORAL_DELIMITER, true, true),
        0x05,
        0,
        obu(OBU_SEQUENCE_HEADER, false, true),
        1,
        0xaa,
        obu(OBU_FRAME, true, true),
        0x08,
        1,
        0x10,
    ];
    let all: Vec<_> = obus(&data)
        .map(|(at, o)| (at, o.header_len, o.size))
        .collect();
    assert_eq!(all, [(0, 3, 0), (3, 2, 1), (6, 3, 1)]);
    assert!(keyframe(&data));
}
