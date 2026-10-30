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

// Sweeps and fixed grids (#2888): `full` visits everything, `reduced` a fixed
// spread-out tenth.
mod sweeps {
    use obs_c_oracle::parity::{keep_at, percent_at, sweep_at};

    #[test]
    fn percent_follows_the_level() {
        assert_eq!(percent_at(None), 100);
        assert_eq!(percent_at(Some("full")), 100);
        assert_eq!(percent_at(Some("reduced")), 10);
        assert_eq!(percent_at(Some("25")), 25);
    }

    #[test]
    fn full_keeps_every_grid_item() {
        assert!((0..10_000).all(|i| keep_at(i, None)));
    }

    #[test]
    fn reduced_keeps_about_a_tenth_and_is_fixed() {
        let kept: Vec<u64> = (0..100_000)
            .filter(|&i| keep_at(i, Some("reduced")))
            .collect();
        assert!((9_000..=11_000).contains(&kept.len()), "{}", kept.len());
        let again: Vec<u64> = (0..100_000)
            .filter(|&i| keep_at(i, Some("reduced")))
            .collect();
        assert_eq!(kept, again);
        // Spread out: some item kept in every block of 100.
        assert!((0..1000).all(|b| kept.iter().any(|&i| i / 100 == b)));
    }

    #[test]
    fn full_sweeps_every_value() {
        assert!(sweep_at(5..1005, None).eq(5..1005));
    }

    #[test]
    fn reduced_sweeps_one_value_per_window() {
        let got: Vec<u64> = sweep_at(0..1_000_000, Some("reduced")).collect();
        assert_eq!(got.len(), 100_000);
        for (w, v) in got.iter().enumerate() {
            assert_eq!(v / 10, w as u64, "{v} outside window {w}");
        }
        // Offsets vary, so no residue class is favoured.
        assert!((0..10).all(|r| got.iter().any(|v| v % 10 == r)));
        // A short tail window still yields a value inside the range.
        let tail: Vec<u64> = sweep_at(0..25, Some("reduced")).collect();
        assert_eq!(tail.len(), 3);
        assert!(tail.iter().all(|&v| v < 25));
    }

    #[test]
    #[should_panic(expected = "OBS_PARITY_CASES")]
    fn an_unknown_sweep_level_panics() {
        percent_at(Some("fast"));
    }
}
