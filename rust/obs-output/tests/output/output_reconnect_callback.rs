//! Tier 1 mirrors every assertion in test_output_reconnect_callback.c.
use obs_output::output_reconnect_callback::{accepts_output, select};

fn first(data: usize, output: usize, code: i32) -> bool {
    data == output && code == -5
}
fn second(data: usize, output: usize, code: i32) -> bool {
    data == 0 && output != 0 && code == 0
}
type Callback = fn(usize, usize, i32) -> bool;
fn check(
    callback: Option<Callback>,
    data: usize,
    expected: Option<Callback>,
    expected_data: usize,
) {
    assert_eq!(callback.map(|f| f as usize), expected.map(|f| f as usize));
    assert_eq!(data, expected_data);
}

#[test]
fn installs_and_replaces_both_fields() {
    let (callback, data) = select(Some(first as Callback), 17);
    check(callback, data, Some(first), 17);
    assert!(callback.unwrap()(data, 17, -5));
    let (callback, data) = select(Some(second as Callback), 7);
    check(callback, data, Some(second), 7);
    let (callback, data) = select(Some(second as Callback), 0);
    check(callback, data, Some(second), 0);
    assert!(callback.unwrap()(data, 17, 0));
}

#[test]
fn null_callback_discards_the_supplied_data() {
    let (callback, data) = select(Some(first as Callback), 17);
    check(callback, data, Some(first), 17);
    let (callback, data) = select(None::<Callback>, 7);
    check(callback, data, None, 0);
    let (callback, data) = select(None::<Callback>, 0);
    check(callback, data, None, 0);
    let (callback, data) = select(Some(first as Callback), 7);
    check(callback, data, Some(first), 7);
}

#[test]
fn clearing_an_empty_callback_keeps_both_fields_empty() {
    let (callback, data) = select(None::<Callback>, 7);
    check(callback, data, None, 0);
}

#[test]
fn null_output_reconnect_setter_is_refused() {
    assert!(!accepts_output(false));
    assert!(accepts_output(true));
}
