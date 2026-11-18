//! Tier 1 mirrors of test/cmocka/test_encoder_priming_samples.c.
use obs_output::encoder_priming_samples::get_priming_samples;
use std::cell::Cell;

#[test]
fn absent_callback_returns_zero() {
    let token = Cell::new(123u32);
    let calls = Cell::new(0);
    for data in [None, Some(&token)] {
        assert_eq!(get_priming_samples::<fn() -> u32>(None), 0);
        assert_eq!(calls.get(), 0);
        assert_eq!(data.map(Cell::get), data.map(|_| 123));
    }
    assert_eq!(token.get(), 123);
}

#[test]
fn callback_receives_exact_data_once() {
    let token = Cell::new(456u32);
    for data in [None, Some(&token)] {
        for result in [0, 1, 1024, 0x8000_0000, u32::MAX] {
            let calls = Cell::new(0);
            let sentinel = Cell::new(0);
            let seen = Cell::new(Some(&sentinel));
            assert_eq!(
                get_priming_samples(Some(|| {
                    calls.set(calls.get() + 1);
                    seen.set(data);
                    result
                })),
                result
            );
            assert_eq!(calls.get(), 1);
            assert!(match (seen.get(), data) {
                (Some(actual), Some(expected)) => std::ptr::eq(actual, expected),
                (None, None) => true,
                _ => false,
            });
            assert_eq!(data.map(Cell::get), data.map(|_| 456));
        }
    }
    assert_eq!(token.get(), 456);
}
