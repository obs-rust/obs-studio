//! Keep registry lookup in libobs and borrow fields through its real layout.
use crate::encoder_registry_queries;
use core::ffi::{c_char, c_int, c_void};
use obs_util::ffi::encoder_type_lookup::find_encoder;

#[repr(C)]
pub struct RegistryQueryFields {
    pub kind: *const c_int,
    pub caps: *const u32,
}

unsafe extern "C" {
    fn obs_encoder_registry_query_fields(info: *const c_void) -> RegistryQueryFields;
}

/// # Safety
/// The registry must be initialized and stable, and `id` must be a valid C
/// string whenever the registry is nonempty, exactly as for `find_encoder`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_encoder_type(id: *const c_char) -> c_int {
    // SAFETY: caller supplies the original lookup's registry/string contract;
    // the adapter borrows a live registered info and reads only its type.
    let value = unsafe {
        find_encoder(id)
            .as_ref()
            .map(|info| *obs_encoder_registry_query_fields(info).kind)
    };
    encoder_registry_queries::kind(value)
}

/// # Safety
/// The registry and ID must satisfy the same contract as `obs_get_encoder_type`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_encoder_caps(id: *const c_char) -> u32 {
    // SAFETY: caller supplies the original lookup's registry/string contract;
    // the adapter borrows a live registered info and reads only its caps.
    let value = unsafe {
        find_encoder(id)
            .as_ref()
            .map(|info| *obs_encoder_registry_query_fields(info).caps)
    };
    encoder_registry_queries::caps(value)
}
