//! `obs_c_oracle::parity::scale`: the CI level scales parity case counts (#2593).

use obs_c_oracle::parity::{FLOOR, scale};

#[test]
fn full_keeps_the_written_count() {
    assert_eq!(scale(20_000, None), 20_000);
    assert_eq!(scale(20_000, Some("full")), 20_000);
    assert_eq!(scale(20_000, Some("")), 20_000);
}

#[test]
fn reduced_keeps_ten_percent() {
    assert_eq!(scale(20_000, Some("reduced")), 2_000);
    assert_eq!(scale(10_000, Some(" reduced ")), 1_000);
}

#[test]
fn a_percent_scales_by_that_percent() {
    assert_eq!(scale(20_000, Some("25")), 5_000);
    assert_eq!(scale(20_000, Some("100")), 20_000);
}

#[test]
fn scaled_counts_keep_the_floor() {
    assert_eq!(scale(512, Some("reduced")), FLOOR);
    assert_eq!(scale(3_000, Some("1")), FLOOR);
    // A module written below the floor keeps its own count.
    assert_eq!(scale(16, Some("reduced")), 16);
}

#[test]
#[should_panic(expected = "OBS_PARITY_CASES")]
fn an_unknown_level_panics() {
    scale(1000, Some("fast"));
}

#[test]
#[should_panic(expected = "OBS_PARITY_CASES")]
fn a_percent_above_100_panics() {
    scale(1000, Some("150"));
}
