use obs_core::core_frame_interval::interval;

#[test]
fn frame_interval_values_matches_c_characterization() {
    for _ in 0..2 {
        let value = 0;
        assert_eq!(interval(Some(&value)), 0);
        assert_eq!(interval(Some(&value)), 0);
    }
}

#[test]
fn full_width_values_are_read_without_changes() {
    for value in [0, 1, 16_666_666, 33_366_666, u64::MAX, 0] {
        assert_eq!(interval(Some(&value)), value);
        assert_eq!(interval(Some(&value)), value);
    }
    assert_eq!(interval(None), 0);
}
