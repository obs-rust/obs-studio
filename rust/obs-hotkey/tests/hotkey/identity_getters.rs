//! Tier 1: mirrors identity_fields in test_hotkey_identity_getters.c (#3924).
use obs_hotkey::identity_getters as core;

#[test]
fn test_hotkey_identity_getters_identity_fields() {
    let mut bytes = [b'x', 0xff, 0];
    for id in [0, 1, 17, usize::MAX - 1, usize::MAX] {
        let state = (id, Some(bytes.as_slice()), 3, 19usize);
        let before = state;
        assert_eq!(core::id(&state.0), id);
        assert!(std::ptr::eq(core::name(&state.1).unwrap(), &bytes[..]));
        assert_eq!(state, before);
    }
    bytes[0] = b'y';
    let name = Some(bytes.as_slice());
    assert!(std::ptr::eq(core::name(&name).unwrap(), &bytes[..]));
    assert_eq!(core::name(&name).unwrap()[0], b'y');
    assert_eq!(core::name(&None::<&[u8]>), None);
    assert_eq!(core::id(&usize::MAX), usize::MAX);
}

#[test]
fn independent_fields_and_empty_name() {
    for id in [0, 1, 17, usize::MAX] {
        assert_eq!(core::id(&id), id);
        assert_eq!(core::name(&Some(&b""[..])), Some(&b""[..]));
        assert_eq!(core::name(&None::<usize>), None);
    }
}
