//! C ABI for the three public hotkey binding getters.
//! Only the requested field is read; the stored hotkey is never dereferenced.
use super::{nix_hotkey_state::obs_key_combination, pair::obs_hotkey_t};
use crate::{binding_getters as core, nix_hotkey_state::KeyCombination};

/// Private layout from obs-internal.h, checked against the real C header.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct obs_hotkey_binding {
    pub key: obs_key_combination,
    pub pressed: bool,
    pub modifiers_match: bool,
    pub hotkey_id: usize,
    pub hotkey: *mut obs_hotkey_t,
}

/// # Safety
/// `binding` points to a live binding with an initialized key field.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_binding_get_key_combination(
    binding: *mut obs_hotkey_binding,
) -> obs_key_combination {
    // SAFETY: the caller provides a readable key field, as required by C.
    let key = unsafe { (*binding).key };
    let key = core::key_combination(&KeyCombination {
        modifiers: key.modifiers,
        key: key.key,
    });
    obs_key_combination {
        modifiers: key.modifiers,
        key: key.key,
    }
}

/// # Safety
/// `binding` points to a live binding with an initialized hotkey_id field.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_binding_get_hotkey_id(
    binding: *mut obs_hotkey_binding,
) -> usize {
    // SAFETY: only the initialized ID field is borrowed.
    core::hotkey_id(unsafe { &(*binding).hotkey_id })
}

/// # Safety
/// `binding` points to a live binding with an initialized hotkey field.
/// The stored pointer may be null; its target is not accessed or retained.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_binding_get_hotkey(
    binding: *mut obs_hotkey_binding,
) -> *mut obs_hotkey_t {
    // SAFETY: only the initialized pointer field is borrowed.
    core::hotkey(unsafe { &(*binding).hotkey })
}
