//! Tier 1: `obs_output::output_caption`, the caption plan of
//! `add_caption()` in `obs-output.c`.
//!
//! The `steps_*` cases port `test/cmocka/test_output_caption_steps.c` 1:1:
//! its 10 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones
//! included. The C calls the hidden `obs_output_caption_*` functions; here
//! the safe core is called directly.
//!
//! The `edge_*` cases are new: a packet too short for its start code and
//! header (`None` from the slice; the C reads past it), and proptests of
//! every decision against a model of the C and of any input without a
//! panic.

#![allow(clippy::as_conversions, clippy::cast_possible_truncation)]

use obs_output::output_caption::{Begin, Carrier, begin, cc_keep, hevc_sei_header};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// obs_output_caption_begin()

fn carrier_of(codec: &str, hevc: bool) -> Carrier {
    let b = begin(codec.as_bytes(), hevc, 0);
    assert!(b.takes, "{codec:?} at priority 0");
    b.carrier
}

#[test]
fn steps_carriers() {
    assert_eq!(carrier_of("h264", false), Carrier::Avc);
    assert_eq!(carrier_of("h264", true), Carrier::Avc);
    assert_eq!(carrier_of("av1", false), Carrier::Av1);
    assert_eq!(carrier_of("av1", true), Carrier::Av1);
    assert_eq!(carrier_of("hevc", true), Carrier::Hevc);
}

#[test]
fn steps_no_carrier() {
    const CODECS: [&str; 15] = [
        "", "prores", "aac", "H264", "h264 ", "h26", "h2640", "avc", "AV1", "av", "av10", "HEVC",
        "hevc ", "h265", "x264",
    ];

    assert_eq!(carrier_of("hevc", false), Carrier::None);
    for codec in CODECS {
        assert_eq!(carrier_of(codec, false), Carrier::None, "{codec:?}");
        assert_eq!(carrier_of(codec, true), Carrier::None, "{codec:?}");
    }
}

#[test]
fn steps_priority_gate() {
    const CASES: [(i32, bool); 7] = [
        (i32::MIN, true),
        (-1, true),
        (0, true),
        (1, true),
        (2, false),
        (3, false),
        (i32::MAX, false),
    ];

    for (priority, takes) in CASES {
        assert_eq!(
            begin(b"h264", true, priority),
            Begin {
                takes,
                carrier: Carrier::Avc
            },
            "h264 at {priority}"
        );
        assert_eq!(
            begin(b"prores", true, priority),
            Begin {
                takes,
                carrier: Carrier::None
            },
            "prores at {priority}"
        );
    }
}

// ---------------------------------------------------------------------------
// obs_output_caption_hevc_sei_header()

fn check_header(data: &[u8], first: u8, second: u8) {
    assert_eq!(hevc_sei_header(data), Some([first, second]), "{data:02x?}");
}

#[test]
fn steps_hevc_header() {
    check_header(&[0x00, 0x00, 0x01, 0x26, 0x01, 0xAF], 0x50, 0x01);
    check_header(&[0x00, 0x00, 0x00, 0x01, 0x40, 0x01], 0x50, 0x01);
    check_header(&[0x00, 0x00, 0x01, 0x03, 0x2B], 0x51, 0x2B);
    check_header(&[0x00, 0x00, 0x00, 0x01, 0xFF, 0xFF], 0x51, 0xFF);
    check_header(&[0x00, 0x00, 0x01, 0x80, 0x07], 0x50, 0x07);
}

#[test]
fn steps_hevc_three_byte_first() {
    check_header(&[0x00, 0x00, 0x01, 0x00, 0x01, 0x42], 0x50, 0x01);
}

#[test]
fn steps_hevc_no_start_code() {
    const CASES: [[u8; 6]; 6] = [
        [0x00, 0x01, 0x40, 0x01, 0x0C, 0x01],
        [0x00, 0x00, 0x02, 0x40, 0x01, 0x0C],
        [0x00, 0x00, 0x00, 0x00, 0x01, 0x40],
        [0x01, 0x00, 0x00, 0x01, 0x40, 0x01],
        [0x00, 0x00, 0x00, 0x02, 0x40, 0x01],
        [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    ];

    for data in CASES {
        assert_eq!(hevc_sei_header(&data), None, "{data:02x?}");
    }
}

// ---------------------------------------------------------------------------
// obs_output_caption_cc_keep()

fn check_cc(b0: u8, b1: u8, b2: u8, keep: bool) {
    let want = keep.then_some(u16::from_be_bytes([b1, b2]));
    assert_eq!(cc_keep([b0, b1, b2]), want, "{b0:02x} {b1:02x} {b2:02x}");
}

#[test]
fn steps_cc_type() {
    check_cc(0xFC, 0x94, 0x2C, true);
    check_cc(0xFD, 0x94, 0x2C, false);
    check_cc(0xFE, 0x94, 0x2C, false);
    check_cc(0xFF, 0x94, 0x2C, false);
    check_cc(0x00, 0x94, 0x2C, true);
    check_cc(0xF8, 0x94, 0x2C, true);
    check_cc(0x03, 0x94, 0x2C, false);
}

#[test]
fn steps_cc_padding_and_zero() {
    check_cc(0xFC, 0x80, 0x80, false);
    check_cc(0xFC, 0x00, 0x00, false);
    check_cc(0xFC, 0x80, 0x01, true);
    check_cc(0xFC, 0x01, 0x80, true);
}

#[test]
fn steps_cc_parity() {
    check_cc(0xFC, 0x14, 0x2C, false);
    check_cc(0xFC, 0x94, 0x2D, false);
    check_cc(0xFC, 0x14, 0x2D, false);
    check_cc(0xFC, 0xC8, 0xE9, true);
    check_cc(0xFC, 0x48, 0xE9, false);
    check_cc(0xFC, 0xC8, 0x69, false);
    check_cc(0xFC, 0x00, 0x01, false);
    check_cc(0xFC, 0x01, 0x00, false);
    check_cc(0xFC, 0x01, 0x01, true);
}

/// The C's rule, written out: `cc_type` 0, not `0x8080`, not 0, odd parity
/// in both bytes.
fn c_cc_keep([b0, b1, b2]: [u8; 3]) -> Option<u16> {
    let word = u16::from_be_bytes([b1, b2]);
    let odd = b1.count_ones() % 2 == 1 && b2.count_ones() % 2 == 1;
    (b0 & 0x3 == 0 && word != 0x8080 && word != 0 && odd).then_some(word)
}

#[test]
fn steps_cc_every_triplet() {
    let mut kept = 0_usize;
    for t in 0..1_u32 << 24 {
        let [_, b0, b1, b2] = t.to_be_bytes();
        let got = cc_keep([b0, b1, b2]);
        assert_eq!(got, c_cc_keep([b0, b1, b2]), "{b0:02x} {b1:02x} {b2:02x}");
        kept += usize::from(got.is_some());
    }
    assert_eq!(kept, 64 * (128 * 128 - 1));
}

// ---------------------------------------------------------------------------
// edge cases

/// A packet that ends before its start code and 2 header bytes: the slice
/// gives `None` where the C reads past the packet.
#[test]
fn edge_hevc_short_packet() {
    for data in [
        &[][..],
        &[0x00],
        &[0x00, 0x00],
        &[0x00, 0x00, 0x01],
        &[0x00, 0x00, 0x01, 0x26],
        &[0x00, 0x00, 0x00],
        &[0x00, 0x00, 0x00, 0x01],
        &[0x00, 0x00, 0x00, 0x01, 0x40],
    ] {
        assert_eq!(hevc_sei_header(data), None, "{data:02x?}");
    }
    // Exactly long enough.
    check_header(&[0x00, 0x00, 0x01, 0x26, 0x01], 0x50, 0x01);
    check_header(&[0x00, 0x00, 0x00, 0x01, 0x40, 0x01], 0x50, 0x01);
}

/// The C's `obs_output_caption_begin()`, written out.
fn c_begin(codec: &[u8], hevc: bool, priority: i32) -> Begin {
    let carrier = match codec {
        b"h264" => Carrier::Avc,
        b"av1" => Carrier::Av1,
        b"hevc" if hevc => Carrier::Hevc,
        _ => Carrier::None,
    };
    Begin {
        takes: priority <= 1,
        carrier,
    }
}

/// The C's `obs_output_caption_hevc_sei_header()`, written out, on a
/// packet long enough for what it reads.
fn c_hevc_sei_header(data: &[u8; 6]) -> Option<[u8; 2]> {
    let start = if data[..3] == [0, 0, 1] {
        3
    } else if data[..4] == [0, 0, 0, 1] {
        4
    } else {
        return None;
    };
    Some([(40 << 1) | (data[start] & 1), data[start + 1]])
}

fn codec() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        prop::sample::select(vec![
            b"h264".to_vec(),
            b"hevc".to_vec(),
            b"av1".to_vec(),
            b"prores".to_vec(),
            Vec::new(),
        ]),
        prop::collection::vec(prop::sample::select(b"h264evcav1 HEVC".to_vec()), 0..6),
        prop::collection::vec(1_u8..=255, 0..12),
    ]
}

fn packet_head() -> impl Strategy<Value = [u8; 6]> {
    (any::<[u8; 6]>(), 0_u8..4).prop_map(|(mut d, form)| {
        match form {
            0 => d[..3].copy_from_slice(&[0, 0, 1]),
            1 => d[..4].copy_from_slice(&[0, 0, 0, 1]),
            2 => d[..2].copy_from_slice(&[0, 0]),
            _ => {}
        }
        d
    })
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(4096),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn edge_begin_matches_the_c(codec in codec(), hevc: bool, priority: i32) {
        prop_assert_eq!(begin(&codec, hevc, priority), c_begin(&codec, hevc, priority));
    }

    #[test]
    fn edge_hevc_header_matches_the_c(data in packet_head()) {
        prop_assert_eq!(hevc_sei_header(&data), c_hevc_sei_header(&data));
    }

    #[test]
    fn edge_hevc_header_is_a_suffix_sei(data in packet_head()) {
        if let Some([first, _]) = hevc_sei_header(&data) {
            prop_assert_eq!(first >> 1, 40);
        }
    }

    #[test]
    fn edge_cc_matches_the_c(triplet: [u8; 3]) {
        prop_assert_eq!(cc_keep(triplet), c_cc_keep(triplet));
    }

    #[test]
    fn edge_no_panic(
        codec in prop::collection::vec(any::<u8>(), 0..16),
        hevc: bool,
        priority: i32,
        data in prop::collection::vec(any::<u8>(), 0..8),
        triplet: [u8; 3],
    ) {
        let _ = begin(&codec, hevc, priority);
        let _ = hevc_sei_header(&data);
        let _ = cc_keep(triplet);
    }
}
