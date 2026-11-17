//! Thin registry and allocator bridge for the public identity setters.
use crate::identity_setters::{Field, Registry, set};
use obs_util::ffi::bmem::{bfree, bmemdup};
use std::ffi::{CStr, c_char};

unsafe extern "C" {
    fn obs_hotkey_identity_slot(id: usize, description: bool) -> *mut *mut c_char;
}

struct LiveRegistry;
impl Registry for LiveRegistry {
    type Input = *const c_char;
    type Slot = *mut *mut c_char;
    type Owned = *mut c_char;

    fn find(&mut self, id: usize, field: Field) -> Option<Self::Slot> {
        // SAFETY: the caller supplies libobs's live registry, as required by C.
        let slot = unsafe { obs_hotkey_identity_slot(id, field == Field::Description) };
        (!slot.is_null()).then_some(slot)
    }
    fn release(&mut self, slot: &Self::Slot) {
        // SAFETY: lookup returned a live field containing a libobs allocation.
        unsafe { bfree((**slot).cast()) };
    }
    fn duplicate(&mut self, input: Self::Input) -> Self::Owned {
        if input.is_null() {
            return std::ptr::null_mut();
        }
        // SAFETY: the public caller supplies a NUL-terminated string independent
        // of the freed slot. Copy its terminator using the same libobs allocator.
        unsafe {
            let bytes = CStr::from_ptr(input).to_bytes_with_nul();
            bmemdup(input.cast(), bytes.len()).cast()
        }
    }
    fn store(&mut self, slot: Self::Slot, value: Self::Owned) {
        // SAFETY: lookup returned this live, writable string field.
        unsafe { *slot = value };
    }
}

/// # Safety
/// A live libobs core is required. For a known ID, `name` must be null or a
/// NUL-terminated string independent of the old name allocation, as in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_set_name(id: usize, name: *const c_char) {
    set(&mut LiveRegistry, id, Field::Name, name);
}

/// # Safety
/// A live libobs core is required. For a known ID, `description` must be null
/// or a NUL-terminated string independent of the old description allocation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_set_description(id: usize, description: *const c_char) {
    set(&mut LiveRegistry, id, Field::Description, description);
}
