//! Borrowed metadata ABI over private C field addresses. No private layout copy.
use std::ffi::{c_char, c_void};

type Callback = unsafe extern "C" fn() -> *const c_char;

unsafe extern "C" {
    fn obs_module_metadata_file_field(module: *mut c_void) -> *const *const c_char;
    fn obs_module_metadata_binary_path_field(module: *mut c_void) -> *const *const c_char;
    fn obs_module_metadata_data_path_field(module: *mut c_void) -> *const *const c_char;
    fn obs_module_metadata_lib_field(module: *mut c_void) -> *const *const c_void;
    fn obs_module_metadata_id_field(module: *mut c_void) -> *const *const c_char;
    fn obs_module_metadata_version_field(module: *mut c_void) -> *const *const c_char;
    fn obs_module_metadata_display_field(module: *mut c_void) -> *const *const c_char;
    fn obs_module_metadata_name_field(module: *mut c_void) -> *const Option<Callback>;
    fn obs_module_metadata_author_field(module: *mut c_void) -> *const Option<Callback>;
    fn obs_module_metadata_description_field(module: *mut c_void) -> *const Option<Callback>;
}

/// Borrow the module's file_name field, or return null.
///
/// # Safety
/// `module` is null or a live, externally synchronized `obs_module_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_module_file_name(module: *mut c_void) -> *const c_char {
    // SAFETY: C supplies the real field address for this live opaque handle.
    unsafe {
        crate::module_metadata::field(
            obs_module_metadata_file_field(module).as_ref().copied(),
            std::ptr::null(),
        )
    }
}

/// Borrow the module's binary_path field, or return null.
///
/// # Safety
/// `module` is null or a live, externally synchronized `obs_module_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_module_binary_path(module: *mut c_void) -> *const c_char {
    // SAFETY: C supplies the real field address for this live opaque handle.
    unsafe {
        crate::module_metadata::field(
            obs_module_metadata_binary_path_field(module)
                .as_ref()
                .copied(),
            std::ptr::null(),
        )
    }
}

/// Borrow the module's data_path field, or return null.
///
/// # Safety
/// `module` is null or a live, externally synchronized `obs_module_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_module_data_path(module: *mut c_void) -> *const c_char {
    // SAFETY: C supplies the real field address for this live opaque handle.
    unsafe {
        crate::module_metadata::field(
            obs_module_metadata_data_path_field(module)
                .as_ref()
                .copied(),
            std::ptr::null(),
        )
    }
}

/// Borrow the module's lib field, or return null.
///
/// # Safety
/// `module` is null or a live, externally synchronized `obs_module_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_module_lib(module: *mut c_void) -> *mut c_void {
    // SAFETY: C supplies the real field address for this live opaque handle.
    unsafe {
        crate::module_metadata::field(
            obs_module_metadata_lib_field(module).as_ref().copied(),
            std::ptr::null(),
        )
        .cast_mut()
    }
}

/// Borrow the module's id field, or return null.
///
/// # Safety
/// `module` is null or a live, externally synchronized `obs_module_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_module_id(module: *mut c_void) -> *const c_char {
    // SAFETY: C supplies the real field address for this live opaque handle.
    unsafe {
        crate::module_metadata::field(
            obs_module_metadata_id_field(module).as_ref().copied(),
            std::ptr::null(),
        )
    }
}

/// Borrow the module's version field, or return null.
///
/// # Safety
/// `module` is null or a live, externally synchronized `obs_module_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_module_version(module: *mut c_void) -> *const c_char {
    // SAFETY: C supplies the real field address for this live opaque handle.
    unsafe {
        crate::module_metadata::field(
            obs_module_metadata_version_field(module).as_ref().copied(),
            std::ptr::null(),
        )
    }
}

/// Borrow the module's name, preserving callback behavior and null returns.
///
/// # Safety
/// `module` is null or a live, externally synchronized `obs_module_t`, whose
/// callbacks obey the original C contract and return borrowed strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_module_name(module: *mut c_void) -> *const c_char {
    // SAFETY: the private seam returns addresses in the caller's live module;
    // callbacks have the same ABI and lifetime contract as the original C.
    unsafe {
        crate::module_metadata::name(
            obs_module_metadata_display_field(module)
                .as_ref()
                .copied()
                .filter(|value| !value.is_null()),
            obs_module_metadata_name_field(module)
                .as_ref()
                .copied()
                .flatten()
                .map(|callback| move || callback()),
            std::ptr::null(),
        )
    }
}

/// Borrow the module's author, preserving callback behavior and null returns.
///
/// # Safety
/// `module` is null or a live, externally synchronized `obs_module_t`, whose
/// callbacks obey the original C contract and return borrowed strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_module_author(module: *mut c_void) -> *const c_char {
    // SAFETY: the private seam returns addresses in the caller's live module;
    // callbacks have the same ABI and lifetime contract as the original C.
    unsafe {
        crate::module_metadata::name(
            None,
            obs_module_metadata_author_field(module)
                .as_ref()
                .copied()
                .flatten()
                .map(|callback| move || callback()),
            std::ptr::null(),
        )
    }
}

/// Borrow the module's description, preserving callback behavior and null returns.
///
/// # Safety
/// `module` is null or a live, externally synchronized `obs_module_t`, whose
/// callbacks obey the original C contract and return borrowed strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_module_description(module: *mut c_void) -> *const c_char {
    // SAFETY: the private seam returns addresses in the caller's live module;
    // callbacks have the same ABI and lifetime contract as the original C.
    unsafe {
        crate::module_metadata::name(
            None,
            obs_module_metadata_description_field(module)
                .as_ref()
                .copied()
                .flatten()
                .map(|callback| move || callback()),
            std::ptr::null(),
        )
    }
}
