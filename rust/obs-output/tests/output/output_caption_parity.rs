//! Tier 3 parity: the shims of `obs_output::ffi::output_caption` (and the
//! safe core under them) against the original
//! `libobs/obs-output-caption.c.h`, compiled as
//! `oracle_obs_output_caption_*` (obs-c-oracle, oracle/output_caption.c).
//!
//! Every call runs the shim, the C and the safe core, and compares the
//! results exactly, including the outputs a false return leaves alone.
//!
//! Runs: the carrier and the gate over the four codec names, near misses
//! and odd strings, with HEVC on and off, at 17 edge priorities; the HEVC
//! header over every 6-byte packet head from a 6-value alphabet that holds
//! both start codes (46,656) and over every header byte pair after each
//! start code (2 x 65,536); the triplet filter over every pair under 12
//! first bytes (each `cc_type`, `cc_valid` set and clear, other bits);
//! and proptests over any codec string, priority, packet head and
//! triplet.
//!
//! No intentional differences. The shims are never given an HEVC packet
//! shorter than what the C reads; `edge_hevc_short_packet` in the Tier 1
//! tests covers the safe core there.
//!
//! `test_output_caption_steps` runs the C test against the C; the real-libobs
//! caption tests (`test_output_caption_codec`, `test_output_caption_queue`)
//! run against the hybrid libobs, where these shims replace the C.

use std::ffi::CString;

use obs_c_oracle::output_caption::{
    oracle_obs_output_caption_begin, oracle_obs_output_caption_cc_keep,
    oracle_obs_output_caption_hevc_sei_header,
};
use obs_output::ffi::output_caption::{
    obs_output_caption_begin, obs_output_caption_cc_keep, obs_output_caption_hevc_sei_header,
};
use obs_output::output_caption::{self as core, Carrier};
use proptest::prelude::*;

/// A value neither side sets, to see what each call writes.
const UNSET_CARRIER: u32 = 0xA5A5_A5A5;
const UNSET_HEADER: [u8; 2] = [0xEE, 0xEE];
const UNSET_CC: u16 = 0xBEEF;

const PRIORITIES: [i32; 17] = [
    i32::MIN,
    i32::MIN + 1,
    -256,
    -3,
    -2,
    -1,
    0,
    1,
    2,
    3,
    4,
    5,
    100,
    255,
    256,
    i32::MAX - 1,
    i32::MAX,
];

fn check_begin(codec: &[u8], hevc: bool, priority: i32) {
    let c_codec = CString::new(codec).expect("no NUL in a test codec");
    let (mut shim_carrier, mut c_carrier) = (UNSET_CARRIER, UNSET_CARRIER);
    // SAFETY: a NUL-terminated string and a local.
    let shim = unsafe {
        obs_output_caption_begin(c_codec.as_ptr(), hevc, priority, &raw mut shim_carrier)
    };
    // SAFETY: as above.
    let c = unsafe {
        oracle_obs_output_caption_begin(c_codec.as_ptr(), hevc, priority, &raw mut c_carrier)
    };
    let safe = core::begin(codec, hevc, priority);
    let what = format!("{codec:?} hevc {hevc} priority {priority}");
    assert_eq!((shim, shim_carrier), (c, c_carrier), "shim, {what}");
    assert_eq!(
        (safe.takes, safe.carrier.to_c()),
        (c, c_carrier),
        "core, {what}"
    );
}

fn check_header(data: &[u8; 6]) {
    let (mut shim_header, mut c_header) = (UNSET_HEADER, UNSET_HEADER);
    // SAFETY: 6 readable bytes, as many as the C reads at most, and a local.
    let shim = unsafe { obs_output_caption_hevc_sei_header(data.as_ptr(), &raw mut shim_header) };
    // SAFETY: as above.
    let c = unsafe { oracle_obs_output_caption_hevc_sei_header(data.as_ptr(), &raw mut c_header) };
    assert_eq!((shim, shim_header), (c, c_header), "shim, {data:02x?}");
    assert_eq!(
        core::hevc_sei_header(data),
        c.then_some(c_header),
        "core, {data:02x?}"
    );
}

fn check_cc(triplet: [u8; 3]) {
    let (mut shim_cc, mut c_cc) = (UNSET_CC, UNSET_CC);
    // SAFETY: 3 readable bytes and a local.
    let shim = unsafe { obs_output_caption_cc_keep(&raw const triplet, &raw mut shim_cc) };
    // SAFETY: as above.
    let c = unsafe { oracle_obs_output_caption_cc_keep(&raw const triplet, &raw mut c_cc) };
    assert_eq!((shim, shim_cc), (c, c_cc), "shim, {triplet:02x?}");
    assert_eq!(
        core::cc_keep(triplet),
        c.then_some(c_cc),
        "core, {triplet:02x?}"
    );
}

#[test]
fn begin_matches_c() {
    const CODECS: [&[u8]; 22] = [
        b"h264",
        b"hevc",
        b"av1",
        b"",
        b"prores",
        b"aac",
        b"opus",
        b"H264",
        b"HEVC",
        b"AV1",
        b"h264 ",
        b" h264",
        b"h26",
        b"h2640",
        b"hevc1",
        b"hev",
        b"av",
        b"av10",
        b"avc",
        b"h265",
        b"x264",
        b"\xffh264",
    ];
    let mut carriers = [0_usize; 4];
    for codec in CODECS {
        for hevc in [false, true] {
            for priority in PRIORITIES {
                check_begin(codec, hevc, priority);
            }
            carriers[core::begin(codec, hevc, 0).carrier.to_c() as usize] += 1;
        }
    }
    // Every carrier occurs.
    assert!(carriers.iter().all(|&n| n > 0), "{carriers:?}");
}

#[test]
fn hevc_header_matches_c_on_every_small_head() {
    const ALPHABET: [u8; 6] = [0x00, 0x01, 0x02, 0x40, 0x41, 0xFF];
    let mut found = 0_usize;
    for i in 0..ALPHABET.len().pow(6) {
        let mut data = [0_u8; 6];
        let mut rest = i;
        for byte in &mut data {
            *byte = ALPHABET[rest % ALPHABET.len()];
            rest /= ALPHABET.len();
        }
        check_header(&data);
        found += usize::from(core::hevc_sei_header(&data).is_some());
    }
    assert!(found > 0);
}

#[test]
fn hevc_header_matches_c_on_every_header() {
    for pair in 0..=u16::MAX {
        let [a, b] = pair.to_be_bytes();
        check_header(&[0x00, 0x00, 0x01, a, b, 0x00]);
        check_header(&[0x00, 0x00, 0x00, 0x01, a, b]);
    }
}

#[test]
fn cc_matches_c_on_every_pair() {
    const FIRST: [u8; 12] = [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x07, 0xF8, 0xFC, 0xFD, 0xFE, 0xFF, 0x80,
    ];
    for b0 in FIRST {
        for pair in 0..=u16::MAX {
            let [b1, b2] = pair.to_be_bytes();
            check_cc([b0, b1, b2]);
        }
    }
}

#[test]
fn carrier_values_match_the_header() {
    assert_eq!(Carrier::None.to_c(), 0);
    assert_eq!(Carrier::Avc.to_c(), 1);
    assert_eq!(Carrier::Hevc.to_c(), 2);
    assert_eq!(Carrier::Av1.to_c(), 3);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(4096)))]

    #[test]
    fn begin_matches_c_on_any_string(
        codec in prop::collection::vec(1_u8..=255, 0..12),
        hevc: bool,
        priority: i32,
    ) {
        check_begin(&codec, hevc, priority);
    }

    #[test]
    fn hevc_header_matches_c_on_any_head(data: [u8; 6], form in 0_u8..3) {
        let mut data = data;
        match form {
            0 => data[..3].copy_from_slice(&[0, 0, 1]),
            1 => data[..4].copy_from_slice(&[0, 0, 0, 1]),
            _ => {}
        }
        check_header(&data);
    }

    #[test]
    fn cc_matches_c_on_any_triplet(triplet: [u8; 3]) {
        check_cc(triplet);
    }
}
