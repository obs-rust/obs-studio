//! Tier 1: mirrors test_hotkey_binding_getters.c through the safe API.
use obs_hotkey::{binding_getters as core, nix_hotkey_state::KeyCombination};

#[test]
fn binding_fields() {
    let key = KeyCombination {
        modifiers: u32::MAX,
        key: obs_hotkey::name_map::key_from_name(c"OBS_KEY_A"),
    };
    assert_eq!(core::key_combination(&key), key);
    assert_eq!(core::hotkey_id(&usize::MAX), usize::MAX);
    let handle = Some(17usize);
    assert_eq!(core::hotkey(&handle), handle);
    let zero = KeyCombination {
        modifiers: 0,
        key: 0,
    };
    assert_eq!(core::key_combination(&zero), zero);
    assert_eq!(core::hotkey_id(&0), 0);
    assert_eq!(core::hotkey(&None::<usize>), None);
}

#[test]
fn arbitrary_stored_values() {
    for key in [i32::MIN, -1, 0, 1, i32::MAX] {
        for modifiers in [0, 1, 1 << 31, u32::MAX] {
            let value = KeyCombination { modifiers, key };
            assert_eq!(core::key_combination(&value), value);
        }
    }
    for id in [0, 1, 17, usize::MAX - 1, usize::MAX] {
        assert_eq!(core::hotkey_id(&id), id);
        assert_eq!(core::hotkey(&id), id);
    }
}
