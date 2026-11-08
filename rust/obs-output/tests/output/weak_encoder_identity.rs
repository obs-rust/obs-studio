use obs_output::weak_encoder_identity::references;

#[test]
fn mirrors_c_identity_without_mutation() {
    assert!(!references(None, 0));
    assert!(!references(None, 1));
    for stored in 0..3 {
        let before = stored;
        assert!(!references(Some(stored), 0));
        for target in 1..3 {
            assert_eq!(references(Some(stored), target), stored == target);
            assert_eq!(stored, before);
        }
    }
    assert!(references(Some(usize::MAX), usize::MAX));
}
