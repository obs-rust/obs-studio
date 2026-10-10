//! Tier 1: the safe AVC helpers on hand-made Annex B packets.

use obs_c_oracle as _;
use obs_codec::avc::{
    AvcHeader, SpsHighParams, extract_headers, keyframe, packet_priority, parse_header,
    sps_high_params, to_avcc,
};
use obs_codec::nal::nal_units;
use proptest as _;

/// A 1080p High-profile SPS as x264 writes it (4:2:0, 8-bit), with
/// emulation-prevention bytes.
const SPS_HIGH: [u8; 27] = [
    0x67, 0x64, 0x00, 0x28, 0xac, 0xd9, 0x40, 0x78, 0x02, 0x27, 0xe5, 0xc0, 0x44, 0x00, 0x00, 0x03,
    0x00, 0x04, 0x00, 0x00, 0x03, 0x00, 0xc8, 0x3c, 0x60, 0xc9, 0x20,
];
const PPS: [u8; 4] = [0x68, 0xeb, 0xe3, 0xcb];

fn annex_b(units: &[&[u8]]) -> Vec<u8> {
    let mut v = Vec::new();
    for u in units {
        v.extend([0, 0, 0, 1]);
        v.extend(*u);
    }
    v
}

#[test]
fn nal_units_walks_header_bytes() {
    let data = annex_b(&[&[0x67, 1, 2], &[0x65, 9]]);
    let units: Vec<_> = nal_units(&data)
        .map(|n| (n.code_start, n.start, n.end))
        .collect();
    assert_eq!(units, [(0, 4, 7), (7, 11, 13)]);
}

#[test]
fn keyframe_is_decided_by_the_first_slice() {
    assert!(keyframe(&annex_b(&[&[0x67, 1], &[0x68, 2], &[0x65, 3]])));
    assert!(!keyframe(&annex_b(&[&[0x41, 1], &[0x65, 3]])));
    assert!(!keyframe(&annex_b(&[&[0x67, 1], &[0x06, 2]])));
    assert!(!keyframe(&[]));
}

#[test]
fn priority_is_the_highest_header_top_bits() {
    // 0x65 >> 5 = 3, 0x41 >> 5 = 2, 0x06 >> 5 = 0
    assert_eq!(packet_priority(&annex_b(&[&[0x06, 0], &[0x41, 0]]), 0), 2);
    assert_eq!(packet_priority(&annex_b(&[&[0x65, 0]]), 1), 3);
    assert_eq!(packet_priority(&annex_b(&[&[0x06, 0]]), 5), 5);
}

#[test]
fn to_avcc_length_prefixes_units() {
    let data = annex_b(&[&[0x06, 0xaa], &[0x65, 0x88, 0x84]]);
    let out = to_avcc(&data, false, 0);
    assert_eq!(
        out.payload,
        [0, 0, 0, 2, 0x06, 0xaa, 0, 0, 0, 3, 0x65, 0x88, 0x84]
    );
    assert!(out.keyframe);
    assert_eq!(out.priority, 3);

    // a non-IDR packet keeps the source keyframe flag
    assert!(to_avcc(&annex_b(&[&[0x41, 0]]), true, 0).keyframe);
}

#[test]
fn high_profile_sps_params() {
    assert_eq!(
        sps_high_params(&SPS_HIGH[1..]),
        SpsHighParams {
            chroma_format_idc: 1,
            bit_depth_luma: 0,
            bit_depth_chroma: 0,
        }
    );
}

#[test]
fn header_record_for_high_profile() {
    let AvcHeader::Record(rec) = parse_header(&annex_b(&[&SPS_HIGH, &PPS])) else {
        panic!("expected a record");
    };
    let mut want = vec![0x01, 0x64, 0x00, 0x28, 0xff, 0xe1, 0x00, 27];
    want.extend(SPS_HIGH);
    want.extend([0x01, 0x00, 4]);
    want.extend(PPS);
    want.extend([0xfd, 0xf8, 0xf8, 0x00]);
    assert_eq!(rec, want);
}

#[test]
fn header_without_start_code_or_parameter_sets() {
    assert_eq!(parse_header(&[1, 2, 3, 4, 5, 6]), AvcHeader::None);
    assert_eq!(parse_header(&[1, 2, 3, 4, 5, 6, 7]), AvcHeader::Copy);
    assert_eq!(parse_header(&annex_b(&[&SPS_HIGH])), AvcHeader::None);
    assert_eq!(
        parse_header(&annex_b(&[&[0x67, 0x42, 0], &PPS])),
        AvcHeader::None
    );
}

#[test]
fn extract_headers_splits_by_type() {
    let sps = annex_b(&[&SPS_HIGH]);
    let pps = annex_b(&[&PPS]);
    let sei = annex_b(&[&[0x06, 1, 2]]);
    let idr = annex_b(&[&[0x65, 7, 7]]);
    let out = extract_headers(&[sps.clone(), sei.clone(), pps.clone(), idr.clone()].concat());
    assert_eq!(out.header, [sps, pps].concat());
    assert_eq!(out.sei, sei);
    assert_eq!(out.packet, idr);
}
