//! Tier 1: mirrors object_fields in test_hotkey_object_getters.c.
use obs_hotkey::object_getters as core;

#[test]
fn test_hotkey_object_getters_object_fields() {
    let mut bytes = [b'x', 0xff, 0];
    let token = Some(17usize);
    let id = 19usize;
    let pressed = 3;
    for kind in 0..=4 {
        let description = Some(bytes.as_slice());
        assert_eq!(core::description(&description), description);
        assert_eq!(core::registerer_type(&kind), kind);
        assert_eq!(core::registerer(&token), token);
        assert_eq!(core::pair_partner_id(&usize::MAX), usize::MAX);
        assert_eq!(id, 19);
        assert_eq!(pressed, 3);
    }
    bytes[0] = b'y';
    let description = Some(bytes.as_slice());
    assert_eq!(core::description(&description), description);
    assert_eq!(core::description(&description).unwrap()[0], b'y');
    assert_eq!(core::description(&None::<&[u8]>), None);
    assert_eq!(core::registerer(&None::<usize>), None);
    assert_eq!(core::pair_partner_id(&0), 0);
    assert_eq!(id, 19);
    assert_eq!(pressed, 3);
}

#[test]
fn independent_field_values() {
    for id in [0, 1, 17, usize::MAX - 1, usize::MAX] {
        assert_eq!(core::pair_partner_id(&id), id);
        assert_eq!(core::registerer(&id), id);
    }
    for kind in [0, 1, 2, 3, 4, u32::MAX] {
        assert_eq!(core::registerer_type(&kind), kind);
    }
    assert_eq!(core::description(&Some(&b""[..])), Some(&b""[..]));
}
