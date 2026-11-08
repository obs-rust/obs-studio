//! Tier 1 mirror of test_object_validity_query.c plus dangling-data coverage.
use obs_core::object_validity_query::is_invalid;

#[test]
fn query_matches_c_characterization() {
    assert!(is_invalid::<i32>(None));
    let mut payloads = [0, 42];
    let values = [
        core::ptr::null_mut(),
        &raw mut payloads[0],
        &raw mut payloads[1],
    ];
    for kind in 0..=5 {
        for data in values {
            let context = (kind, true, "object data sentinel", data);
            let before = context;
            assert_eq!(is_invalid(Some(context.3)), data.is_null());
            assert_eq!(is_invalid(Some(context.3)), data.is_null());
            assert_eq!(context, before);
        }
    }
    assert_eq!(payloads, [0, 42]);
}

#[test]
fn data_is_never_dereferenced() {
    let data = core::ptr::dangling_mut::<u8>();
    assert!(!is_invalid(Some(data)));
}
