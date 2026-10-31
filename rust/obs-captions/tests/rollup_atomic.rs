//! Rejected words must not poison a live session or partially emit captions.
//! These tests use the pre-fix public API so the RED commit also compiles.
use obs_captions::rollup::{Rollup, RollupError, Rows};

#[test]
fn unmappable_word_does_not_advance_the_timestamp() {
    let mut r = Rollup::new(30, Rows::Two);
    r.word(1.0, "OK").unwrap();
    let before = r.pairs().to_vec();
    assert_eq!(r.word(10.0, "A*B"), Err(RollupError::UnmappableChar('*')));
    assert_eq!(r.pairs(), before);
    r.word(2.0, "NEXT").unwrap();
}

#[test]
fn rejected_word_preserves_all_subsequent_output() {
    for fps in [30, 60] {
        for rows in [Rows::Two, Rows::Three] {
            let mut actual = Rollup::new(fps, rows);
            let mut expected = Rollup::new(fps, rows);
            actual
                .word(1.0, "ABCDEFGHIJKLMNOPQRSTUVWXYZ123456")
                .unwrap();
            expected
                .word(1.0, "ABCDEFGHIJKLMNOPQRSTUVWXYZ123456")
                .unwrap();
            assert!(actual.word(100.0, "BAD*").is_err());
            for (t, word) in [(2.0, "NEXT"), (3.0, "élÑñ"), (4.0, "END")] {
                actual.word(t, word).unwrap();
                expected.word(t, word).unwrap();
            }
            assert_eq!(actual.pairs(), expected.pairs());
        }
    }
}

#[test]
fn invalid_timestamps_leave_the_session_unchanged() {
    for timestamp in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 0.5] {
        let mut r = Rollup::new(30, Rows::Two);
        r.word(1.0, "OK").unwrap();
        let before = r.pairs().to_vec();
        assert!(r.word(timestamp, "BAD").is_err());
        assert_eq!(r.pairs(), before);
        r.word(2.0, "NEXT").unwrap();
    }
}

#[test]
fn finite_timestamp_overflow_is_an_atomic_error() {
    for fps in [30, 60] {
        for timestamp in [f64::MAX, (f64::from(u32::MAX) + 1.0) / f64::from(fps)] {
            let mut r = Rollup::new(fps, Rows::Two);
            let before = r.pairs().to_vec();
            assert!(r.word(timestamp, "AB").is_err());
            assert_eq!(r.pairs(), before);
            r.word(1.0, "OK").unwrap();
        }
    }
}

#[test]
fn final_30fps_frame_can_be_emitted_without_cursor_wrap() {
    let mut r = Rollup::new(30, Rows::Two);
    let timestamp = f64::from(u32::MAX) / 30.0;
    r.word(timestamp, "AB").unwrap();
    assert_eq!(r.pairs().last().unwrap().0, u32::MAX);
    let before = r.pairs().to_vec();
    assert!(r.word(timestamp, "CD").is_err());
    assert_eq!(r.pairs(), before);
}

#[test]
fn final_60fps_slot_preserves_alternating_picture_cadence() {
    let mut r = Rollup::new(60, Rows::Three);
    let last = u32::MAX - 1;
    let timestamp = f64::from(last) / 60.0;
    r.word(timestamp, "AB").unwrap();
    assert_eq!(r.pairs().last().unwrap().0, last);
    let before = r.pairs().to_vec();
    assert!(r.word(timestamp, "CD").is_err());
    assert_eq!(r.pairs(), before);
}

#[test]
fn rounding_to_an_unrepresentable_60fps_slot_is_atomic() {
    let mut r = Rollup::new(60, Rows::Three);
    let before = r.pairs().to_vec();
    assert!(r.word(f64::from(u32::MAX) / 60.0, "AB").is_err());
    assert_eq!(r.pairs(), before);
    r.word(1.0, "NEXT").unwrap();
}

#[test]
fn overflow_halfway_through_a_word_emits_nothing() {
    for fps in [30, 60] {
        let mut r = Rollup::new(fps, Rows::Two);
        let before = r.pairs().to_vec();
        let timestamp = f64::from(u32::MAX - 3) / f64::from(fps);
        assert!(r.word(timestamp, "ABCDEFGHIJKL").is_err());
        assert_eq!(r.pairs(), before);
        r.word(1.0, "NEXT").unwrap();
    }
}

#[test]
fn overflow_during_a_doubled_wrap_control_is_atomic() {
    let mut actual = Rollup::new(30, Rows::Two);
    let mut expected = Rollup::new(30, Rows::Two);
    let full_row = "ABCDEFGHIJKLMNOPQRSTUVWXYZ123456";
    actual.word(1.0, full_row).unwrap();
    expected.word(1.0, full_row).unwrap();
    let before = actual.pairs().to_vec();
    assert!(actual.word(f64::from(u32::MAX) / 30.0, "WRAP").is_err());
    assert_eq!(actual.pairs(), before);
    actual.word(2.0, "NEXT").unwrap();
    expected.word(2.0, "NEXT").unwrap();
    assert_eq!(actual.pairs(), expected.pairs());
}

#[test]
fn normal_words_keep_strict_frame_order_and_cadence() {
    for fps in [30, 60] {
        let mut r = Rollup::new(fps, Rows::Three);
        for i in 0..256 {
            r.word(f64::from(i) / 8.0, "CAPTIONS").unwrap();
        }
        let step = fps / 30;
        assert!(r.pairs().iter().all(|&(frame, _)| frame % step == 0));
        assert!(r.pairs().windows(2).all(|p| p[1].0 >= p[0].0 + step));
    }
}
