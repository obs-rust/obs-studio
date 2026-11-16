use obs_core::weak_object_expired::expired;

#[test]
fn mirrors_c_null_is_expired() {
    assert!(expired(None));
}

#[test]
fn mirrors_c_nonnegative_owners_are_alive() {
    for owners in [0, 1, i32::MAX as i64, i64::MAX] {
        let before = owners;
        assert!(!expired(Some(owners)));
        assert_eq!(owners, before);
    }
}

#[test]
fn mirrors_c_negative_owners_are_expired() {
    for owners in [i64::MIN, i32::MIN as i64, -2, -1] {
        let before = owners;
        assert!(expired(Some(owners)));
        assert_eq!(owners, before);
    }
}
