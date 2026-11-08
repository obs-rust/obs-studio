use obs_core::core_initialized::initialized;

#[test]
fn initialized_lifecycle_matches_c_characterization() {
    let core = 7_u8;
    assert!(!initialized::<u8>(std::ptr::null()));
    assert!(!initialized::<u8>(std::ptr::null()));
    assert!(initialized(&core));
    assert!(initialized(&core));
    assert!(!initialized::<u8>(std::ptr::null()));
    assert!(!initialized::<u8>(std::ptr::null()));
    assert_eq!(core, 7);
}

#[test]
fn only_pointer_presence_matters() {
    for address in [0, 1, 2, usize::MAX / 2, usize::MAX] {
        let core = std::ptr::without_provenance::<u8>(address);
        assert_eq!(initialized(core), address != 0);
    }
}
