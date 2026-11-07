//! Tier 3: exact parity, including pointers and unchanged latch state.
//! No intentional differences. A null binding is excluded: C dereferences it.
//! Stored hotkeys may be null and are never dereferenced by either side.
use obs_c_oracle as _;
use obs_hotkey::ffi::{
    binding_getters as rs, nix_hotkey_state::obs_key_combination, pair::obs_hotkey_t,
};
use proptest::prelude::*;
use std::mem::{align_of, offset_of, size_of};

unsafe extern "C" {
    fn oracle_hotkey_binding_getters_layout(which: usize) -> usize;
    fn oracle_obs_hotkey_binding_get_key_combination(
        binding: *mut rs::obs_hotkey_binding,
    ) -> obs_key_combination;
    fn oracle_obs_hotkey_binding_get_hotkey_id(binding: *mut rs::obs_hotkey_binding) -> usize;
    fn oracle_obs_hotkey_binding_get_hotkey(
        binding: *mut rs::obs_hotkey_binding,
    ) -> *mut obs_hotkey_t;
}

#[test]
fn real_header_layout() {
    type Binding = rs::obs_hotkey_binding;
    let rust = [
        size_of::<Binding>(),
        align_of::<Binding>(),
        offset_of!(Binding, key),
        offset_of!(Binding, pressed),
        offset_of!(Binding, modifiers_match),
        offset_of!(Binding, hotkey_id),
        offset_of!(Binding, hotkey),
        size_of::<obs_key_combination>(),
        align_of::<obs_key_combination>(),
        offset_of!(obs_key_combination, modifiers),
        offset_of!(obs_key_combination, key),
    ];
    for (which, value) in rust.into_iter().enumerate() {
        // SAFETY: the helper reads a constant table measured from the real headers.
        assert_eq!(
            value,
            unsafe { oracle_hotkey_binding_getters_layout(which) },
            "layout index {which}"
        );
    }
}

fn check(modifiers: u32, key: i32, id: usize, pressed: bool, matched: bool, has_hotkey: bool) {
    // A live token whose address is copied only; no hotkey fields are accessed.
    let mut token = 0u8;
    let hotkey = if has_hotkey {
        (&raw mut token).cast::<obs_hotkey_t>()
    } else {
        std::ptr::null_mut()
    };
    let mut binding = rs::obs_hotkey_binding {
        key: obs_key_combination { modifiers, key },
        pressed,
        modifiers_match: matched,
        hotkey_id: id,
        hotkey,
    };
    // SAFETY: layout is checked against C, all fields are initialized, and the
    // getters copy the opaque pointer without accessing its target.
    unsafe {
        assert_eq!(
            rs::obs_hotkey_binding_get_key_combination(&raw mut binding),
            oracle_obs_hotkey_binding_get_key_combination(&raw mut binding)
        );
        assert_eq!(
            rs::obs_hotkey_binding_get_hotkey_id(&raw mut binding),
            oracle_obs_hotkey_binding_get_hotkey_id(&raw mut binding)
        );
        assert_eq!(
            rs::obs_hotkey_binding_get_hotkey(&raw mut binding),
            oracle_obs_hotkey_binding_get_hotkey(&raw mut binding)
        );
    }
    assert_eq!(binding.key, obs_key_combination { modifiers, key });
    assert_eq!(binding.hotkey_id, id);
    assert_eq!(binding.hotkey, hotkey);
    assert_eq!(binding.pressed, pressed);
    assert_eq!(binding.modifiers_match, matched);
}

#[test]
fn characterized_edges() {
    for modifiers in [0, 1, 1 << 31, u32::MAX] {
        for id in [0, 17, usize::MAX] {
            for has_hotkey in [false, true] {
                check(modifiers, 0, id, true, true, has_hotkey);
                check(modifiers, 1, id, false, false, has_hotkey);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]
    #[test]
    fn values_match_c(modifiers in any::<u32>(), key in 0i32..256, id in any::<usize>(), pressed in any::<bool>(), matched in any::<bool>(), has_hotkey in any::<bool>()) {
        check(modifiers, key, id, pressed, matched, has_hotkey);
    }
}
