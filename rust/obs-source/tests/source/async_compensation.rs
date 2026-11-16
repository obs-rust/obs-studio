//! Tier 1: the flag transitions used by test_source_audio_input's compensation
//! case. Resampler creation/removal remains covered by that unchanged C test.
use core::sync::atomic::{AtomicBool, Ordering};
use obs_source::async_compensation::set;
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
