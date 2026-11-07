//! Tier 1 mirrors test_encoder_last_error_getter.c (#3900).
use obs_output::encoder_last_error::message;

#[test]
fn null_default_and_diagnostic() {
    let mut logs = 0;
    assert_eq!(message::<Option<&[u8]>>(None, || logs += 1), None);
    assert_eq!(logs, 1);
}

#[test]
fn borrowed_independent_messages() {
    let empty: &[u8] = b"";
    let invalid: &[u8] = &[b'a', 0xff, b'b'];
    let other: &[u8] = b"other";
    let values = [None, Some(empty), Some(invalid)];
    let before = values;
    for value in values {
        let result = message(Some(value), || panic!("valid encoder must be silent"));
        match (result, value) {
            (Some(actual), Some(expected)) => assert!(std::ptr::eq(actual, expected)),
            (None, None) => {}
            _ => panic!("stored message changed"),
        }
        let other_result: &[u8] = message(Some(other), || panic!("independent encoder must be silent"));
        assert!(std::ptr::eq(other_result, other));
        assert_eq!(values, before);
    }
}
