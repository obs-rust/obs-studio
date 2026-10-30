//! Tier 3: the Rust C ABI shim and safe core behave exactly like the
//! original C, compiled as an oracle.
//!
//! No intentional differences from C.
//!
//! Random `u32`s almost never hit the table, so besides proptest the tests
//! below enumerate every code built from the characters the table uses and
//! every single-bit change of a mapped code.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use obs_c_oracle::video_fourcc as c;
use obs_media_io::ffi::video_fourcc as rs;
use obs_media_io::video_fourcc::make_fourcc;
use obs_media_io::video_io::{VideoFormat, video_format_to_c};
use proptest::prelude::*;

/// Every fourcc `video-fourcc.c` maps.
const MAPPED: [&[u8; 4]; 16] = [
    b"UYVY", b"HDYC", b"UYNV", b"UYNY", b"uyv1", b"2vuy", b"2Vuy", b"YUY2", b"Y422", b"V422",
    b"VYUY", b"YUNV", b"yuv2", b"yuvs", b"YVYU", b"Y800",
];

/// Shim, core and oracle agree on `fourcc`.
fn check(fourcc: u32) -> Result<(), TestCaseError> {
    // SAFETY: the oracle takes a plain integer and touches no memory.
    let want = unsafe { c::oracle_video_format_from_fourcc(fourcc) };
    prop_assert_eq!(
        rs::video_format_from_fourcc(fourcc),
        want,
        "shim, fourcc {:#010x}",
        fourcc
    );
    prop_assert_eq!(
        video_format_to_c(VideoFormat::from_fourcc(fourcc)),
        want,
        "core, fourcc {:#010x}",
        fourcc
    );
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(10_000)))]

    #[test]
    fn arbitrary_fourcc_matches_c_oracle(fourcc in any::<u32>()) {
        check(fourcc)?;
    }

    #[test]
    fn mapped_code_with_one_byte_replaced_matches_c_oracle(
        code in prop::sample::select(&MAPPED[..]),
        at in 0usize..4,
        byte in any::<u8>(),
    ) {
        let mut code = *code;
        code[at] = byte;
        check(make_fourcc(code))?;
    }
}

#[test]
fn every_code_over_the_table_alphabet_matches_c_oracle() {
    let mut alphabet: Vec<u8> = MAPPED.iter().flat_map(|c| c.iter().copied()).collect();
    alphabet.sort_unstable();
    alphabet.dedup();
    for &a in &alphabet {
        for &b in &alphabet {
            for &c in &alphabet {
                for &d in &alphabet {
                    check(make_fourcc([a, b, c, d])).unwrap();
                }
            }
        }
    }
}

#[test]
fn single_bit_flips_of_mapped_codes_match_c_oracle() {
    for code in MAPPED {
        let fourcc = make_fourcc(*code);
        check(fourcc).unwrap();
        for bit in 0..32 {
            check(fourcc ^ (1 << bit)).unwrap();
        }
    }
}

/// The cases from `test/cmocka/test_video_fourcc.c`, run against the oracle
/// so the C test's expectations are checked on every platform.
#[test]
fn cmocka_cases_match_c_oracle() {
    let named: [(&[u8; 4], VideoFormat); 36] = [
        (b"UYVY", VideoFormat::Uyvy),
        (b"HDYC", VideoFormat::Uyvy),
        (b"UYNV", VideoFormat::Uyvy),
        (b"UYNY", VideoFormat::Uyvy),
        (b"uyv1", VideoFormat::Uyvy),
        (b"2vuy", VideoFormat::Uyvy),
        (b"2Vuy", VideoFormat::Uyvy),
        (b"YUY2", VideoFormat::Yuy2),
        (b"Y422", VideoFormat::Yuy2),
        (b"V422", VideoFormat::Yuy2),
        (b"VYUY", VideoFormat::Yuy2),
        (b"YUNV", VideoFormat::Yuy2),
        (b"yuv2", VideoFormat::Yuy2),
        (b"yuvs", VideoFormat::Yuy2),
        (b"YVYU", VideoFormat::Yvyu),
        (b"Y800", VideoFormat::Y800),
        (b"I420", VideoFormat::None),
        (b"NV12", VideoFormat::None),
        (b"RGBA", VideoFormat::None),
        (b"BGRA", VideoFormat::None),
        (b"I444", VideoFormat::None),
        (b"P010", VideoFormat::None),
        (b"v210", VideoFormat::None),
        (b"AYUV", VideoFormat::None),
        (b"uyvy", VideoFormat::None),
        (b"yuy2", VideoFormat::None),
        (b"yvyu", VideoFormat::None),
        (b"y800", VideoFormat::None),
        (b"2VUY", VideoFormat::None),
        (b"UYV1", VideoFormat::None),
        (b"YUV2", VideoFormat::None),
        (b"YUVS", VideoFormat::None),
        (b"UYV\0", VideoFormat::None),
        (b"\0UYV", VideoFormat::None),
        (b"\0\0\0\0", VideoFormat::None),
        (b"\xff\xff\xff\xff", VideoFormat::None),
    ];
    // test_byte_order: "UYVY" and "YUY2" read the other way round.
    for (fourcc, want) in [
        (0x5559_5659, VideoFormat::Yvyu),
        (0x5955_5932, VideoFormat::None),
    ] {
        // SAFETY: the oracle takes a plain integer and touches no memory.
        let got = unsafe { c::oracle_video_format_from_fourcc(fourcc) };
        assert_eq!(got, video_format_to_c(want), "C oracle for {fourcc:#010x}");
        check(fourcc).unwrap();
    }
    for (code, want) in named {
        let fourcc = make_fourcc(*code);
        // SAFETY: the oracle takes a plain integer and touches no memory.
        let got = unsafe { c::oracle_video_format_from_fourcc(fourcc) };
        assert_eq!(
            got,
            video_format_to_c(want),
            "C oracle for {:?}",
            String::from_utf8_lossy(code)
        );
        check(fourcc).unwrap();
    }
}

/// Every `u32`, split across threads. It is the only test here that catches
/// an extra code built from characters outside the table. The root
/// `Cargo.toml` builds this crate with `opt-level = 3` in the dev profile so
/// the sweep takes seconds in the default debug `cargo test`. Below the
/// `full` parity level (PR, queue, master push) it checks one value in each
/// window of ten; the nightly and release runs sweep every `u32` (#2888).
/// The named codes above run on every level.
#[test]
fn every_u32_matches_c_oracle() {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()) as u64;
    let span = (1u64 << 32).div_ceil(threads);
    std::thread::scope(|scope| {
        for t in 0..threads {
            scope.spawn(move || {
                let end = ((t + 1) * span).min(1 << 32);
                for fourcc in obs_c_oracle::parity::sweep(t * span..end).map(|f| f as u32) {
                    // SAFETY: the oracle takes a plain integer and touches no memory.
                    let want = unsafe { c::oracle_video_format_from_fourcc(fourcc) };
                    if rs::video_format_from_fourcc(fourcc) != want
                        || video_format_to_c(VideoFormat::from_fourcc(fourcc)) != want
                    {
                        panic!("fourcc {fourcc:#010x}: C oracle {want}");
                    }
                }
            });
        }
    });
}
