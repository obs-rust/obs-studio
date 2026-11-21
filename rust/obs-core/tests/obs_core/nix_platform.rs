use obs_core::platform::nix_platform::PlatformState;

#[test]
fn defaults_and_independent_updates() {
    let mut state = PlatformState::new();
    assert_eq!(state.platform(), 1);
    assert!(state.display().is_null());
    let mut first = 7_u8;
    let mut second = 9_u8;
    let first_ptr = (&raw mut first).cast();
    let second_ptr = (&raw mut second).cast();
    state.set_display(first_ptr);
    assert_eq!(state.display(), first_ptr);
    assert_eq!(state.platform(), 1);
    for platform in [2, 1, 2, 17, u32::MAX, 1] {
        state.set_platform(platform);
        assert_eq!(state.platform(), platform);
        assert_eq!(state.display(), first_ptr);
    }
    state.set_display(second_ptr);
    assert_eq!(state.display(), second_ptr);
    assert_eq!(state.platform(), 1);
    state.set_display(core::ptr::null_mut());
    assert!(state.display().is_null());
    assert_eq!((first, second), (7, 9));
}

#[test]
fn invalid_platform_asserts_only_in_debug() {
    let mut state = PlatformState::default();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| state.set_platform(0)));
    assert_eq!(result.is_err(), cfg!(debug_assertions));
    if !cfg!(debug_assertions) {
        assert_eq!(state.platform(), 0);
    }
}
