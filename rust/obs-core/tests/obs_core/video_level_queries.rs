//! Safe mirrors of test_video_level_queries.c (#5168, #5187). Every level is
//! compared by its exact binary32 bits, never approximately.
use obs_core::video_level_queries::{
    DEFAULT_HDR_NOMINAL_PEAK_LEVEL, DEFAULT_SDR_WHITE_LEVEL, VideoLevels, hdr_nominal_peak_level,
    sdr_white_level,
};

const WHITE_DEFAULT: u32 = 0x4396_0000;
const PEAK_DEFAULT: u32 = 0x447a_0000;

fn stored(graphics: bool, white: u32, peak: u32) -> VideoLevels {
    VideoLevels {
        graphics,
        sdr_white_level: f32::from_bits(white),
        hdr_nominal_peak_level: f32::from_bits(peak),
    }
}

fn expect_levels(levels: Option<VideoLevels>, white: u32, peak: u32) {
    assert_eq!(sdr_white_level(levels).to_bits(), white);
    assert_eq!(hdr_nominal_peak_level(levels).to_bits(), peak);
}

#[test]
fn defaults_are_the_characterized_bits() {
    assert_eq!(DEFAULT_SDR_WHITE_LEVEL.to_bits(), WHITE_DEFAULT);
    assert_eq!(DEFAULT_HDR_NOMINAL_PEAK_LEVEL.to_bits(), PEAK_DEFAULT);
}

#[test]
fn no_core_before_and_after() {
    expect_levels(None, WHITE_DEFAULT, PEAK_DEFAULT);
    expect_levels(
        Some(stored(true, 0x4323_4567, 0x45ab_cdef)),
        0x4323_4567,
        0x45ab_cdef,
    );
    expect_levels(None, WHITE_DEFAULT, PEAK_DEFAULT);
}

#[test]
fn no_graphics_ignores_stored_fields() {
    expect_levels(
        Some(stored(false, 0x4323_4567, 0x45ab_cdef)),
        WHITE_DEFAULT,
        PEAK_DEFAULT,
    );
    expect_levels(
        Some(stored(false, 0x7fc1_2345, 0x8000_0000)),
        WHITE_DEFAULT,
        PEAK_DEFAULT,
    );
}

#[test]
fn graphics_returns_exact_field_bits() {
    // Characterized, not endorsed: no validation, clamping or arithmetic;
    // signed zeros and quiet NaN payloads come back verbatim.
    const PAIRS: [(u32, u32); 9] = [
        (0x4323_4567, 0x45ab_cdef),
        (0x45ab_cdef, 0x4323_4567),
        (0x0000_0000, 0x8000_0000),
        (0x8000_0000, 0x0000_0000),
        (0xbf80_0000, 0xc020_0000),
        (0x7f80_0000, 0xff80_0000),
        (0x7fc1_2345, 0xffc5_4321),
        (0xffc5_4321, 0x7fc1_2345),
        (0x0080_0000, 0x7f7f_ffff),
    ];
    for (white, peak) in PAIRS {
        expect_levels(Some(stored(true, white, peak)), white, peak);
    }
    // Removing graphics selects defaults even with stored exceptional bits.
    let (white, peak) = PAIRS[PAIRS.len() - 1];
    expect_levels(
        Some(stored(false, white, peak)),
        WHITE_DEFAULT,
        PEAK_DEFAULT,
    );
}
