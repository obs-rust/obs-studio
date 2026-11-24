//! Tier 1: the flag transitions used by test_source_audio_input's compensation
//! case. Resampler creation/removal remains covered by that unchanged C test.
use core::sync::atomic::{AtomicBool, Ordering};
use obs_source::async_compensation::{get, set};
use obs_source::audio_input::{Format, resampler_needed};

#[test]
fn test_source_audio_input_compensation_enable_disable() {
    let value = AtomicBool::new(false);
    let format = Format {
        format: 1,
        samples_per_sec: 48_000,
        speakers: 2,
    };
    assert!(!resampler_needed(
        format,
        format,
        value.load(Ordering::SeqCst)
    ));
    for requested in [true, true, false, false, true, false] {
        set(Some(&value), requested, || panic!("valid source logged"));
        assert_eq!(value.load(Ordering::SeqCst), requested);
        assert_eq!(
            resampler_needed(format, format, value.load(Ordering::SeqCst)),
            requested
        );
    }
}

#[test]
fn null_logs_once_for_each_value() {
    for requested in [false, true] {
        let mut logs = 0;
        set(None, requested, || logs += 1);
        assert_eq!(logs, 1);
    }
}

/// Tier 1 mirror of test_source_async_flag_pair.c's getter cases.
#[test]
fn get_null_answers_false_and_logs_once() {
    let mut logs = 0;
    assert!(!get(None, || logs += 1));
    assert_eq!(logs, 1);
}

#[test]
fn get_answers_the_flag_and_leaves_it() {
    for value in [false, true] {
        let flag = AtomicBool::new(value);
        for _ in 0..2 {
            assert_eq!(get(Some(&flag), || panic!("valid source logged")), value);
            assert_eq!(flag.load(Ordering::SeqCst), value);
        }
    }
    // A request stored by the setter is what the getter answers.
    let flag = AtomicBool::new(false);
    for requested in [true, false, true] {
        set(Some(&flag), requested, || panic!("valid source logged"));
        assert_eq!(
            get(Some(&flag), || panic!("valid source logged")),
            requested
        );
    }
}
