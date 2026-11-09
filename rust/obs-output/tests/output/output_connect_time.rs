//! Mirrors test_encoder_output_getters_characterization.c; callback edge cases.
use obs_output::output_connect_time::get;
use std::cell::Cell;

#[test]
fn mirrors_missing_callback_minus_one() {
    assert_eq!(get(Some(()), |_| None, || panic!("valid output")), -1);
}

#[test]
fn mirrors_null_minus_one_with_one_diagnostic() {
    let logs = Cell::new(0);
    assert_eq!(
        get(
            None::<()>,
            |_| panic!("NULL callback"),
            || logs.set(logs.get() + 1)
        ),
        -1
    );
    assert_eq!(logs.get(), 1);
}

#[test]
fn callback_once_with_exact_signed_result_and_data() {
    for result in [i32::MIN, -97, -1, 0, 29, i32::MAX] {
        let calls = Cell::new(0);
        assert_eq!(
            get(
                Some(71),
                |data| {
                    assert_eq!(data, 71);
                    calls.set(calls.get() + 1);
                    Some(result)
                },
                || panic!("valid output")
            ),
            result
        );
        assert_eq!(calls.get(), 1);
    }
}
