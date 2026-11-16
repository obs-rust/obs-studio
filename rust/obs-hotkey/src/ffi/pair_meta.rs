//! ABI bridge to the existing registry seams and public single-hotkey setters.
use std::ffi::c_char;

use super::pair::obs_hotkey_pair;
use crate::pair_meta::{self, Field};

unsafe extern "C" {
    fn obs_hotkey_seam_find_pair(id: usize) -> *mut obs_hotkey_pair;
    fn obs_hotkey_seam_pair_id(pair: *const obs_hotkey_pair, half: usize) -> usize;
    fn obs_hotkey_set_name(id: usize, value: *const c_char);
    fn obs_hotkey_set_description(id: usize, value: *const c_char);
}

struct Registry;
impl pair_meta::Registry for Registry {
    type Pair = *mut obs_hotkey_pair;
    type Value = *const c_char;
    fn find(&mut self, id: usize) -> Option<Self::Pair> {
        // SAFETY: the caller maintains the initialized registry, as in C.
        let pair = unsafe { obs_hotkey_seam_find_pair(id) };
        (!pair.is_null()).then_some(pair)
    }
    fn hotkey_id(&self, pair: &Self::Pair, half: usize) -> usize {
        // SAFETY: only a non-null registry token reaches this accessor; half < 2.
        unsafe { obs_hotkey_seam_pair_id(*pair, half) }
    }
    fn set(&mut self, id: usize, value: Self::Value, field: Field) {
        // SAFETY: forward the original nullable string unchanged, synchronously,
        // under the same registry lifetime requirements as the C entry point.
        unsafe {
            match field {
                Field::Name => obs_hotkey_set_name(id, value),
                Field::Description => obs_hotkey_set_description(id, value),
            }
        }
    }
}

/// Set both names in the initialized registry without adding a lock.
///
/// # Safety
/// Each non-null input must be NUL-terminated, readable through its setter,
/// and must not alias a hotkey-owned string freed by that setter (as in C).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_pair_set_names(
    id: usize,
    first: *const c_char,
    second: *const c_char,
) {
    pair_meta::set(&mut Registry, id, [first, second], Field::Name);
}

/// Set both descriptions in the initialized registry without adding a lock.
///
/// # Safety
/// Same string and registry requirements as [`obs_hotkey_pair_set_names`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_pair_set_descriptions(
    id: usize,
    first: *const c_char,
    second: *const c_char,
) {
    pair_meta::set(&mut Registry, id, [first, second], Field::Description);
}
