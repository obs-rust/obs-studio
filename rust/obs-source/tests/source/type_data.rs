//! Mirrors source type-data readback in test_register_partial_callbacks.c.
use obs_source::type_data::get;

#[test]
fn mirrors_registered_type_data() {
    assert_eq!(
        get(Some(0x1234usize), |_| panic!("live source must not log")),
        0x1234
    );
}

#[test]
fn mirrors_partially_registered_null_type_data() {
    assert_eq!(get(Some(0usize), |_| panic!("stored NULL must not log")), 0);
}

#[test]
fn null_source_logs_once_and_returns_default() {
    let mut calls = 0;
    assert_eq!(
        get::<usize>(None, |name| {
            assert_eq!(name, c"obs_source_get_type_data");
            calls += 1;
        }),
        0
    );
    assert_eq!(calls, 1);
}
