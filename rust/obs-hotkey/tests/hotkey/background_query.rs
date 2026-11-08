use obs_hotkey::background_query::{NATIVE, is_background};

#[test]
fn test_native_fallback() {
    // Mirrors test_hotkey_background_query.c: keys and modifiers cannot affect
    // the native fallback because it never asks for a combination identifier.
    assert!(is_background(NATIVE, || panic!("native must not look up an identifier")));
}

#[test]
fn non_native_uses_identifier_presence() {
    for state in [i32::MIN, -1, 1, 2, 3, 4, 5, i32::MAX] {
        for present in [false, true] {
            let mut calls = 0;
            assert_eq!(is_background(state, || { calls += 1; present }), present);
            assert_eq!(calls, 1);
        }
    }
}
