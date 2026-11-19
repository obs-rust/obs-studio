//! C ABI over private registry field adapters. No registry layout crosses into Rust.
use crate::type_catalog_enum;
use core::ffi::{c_char, c_void};
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_core_texture_activity_core() -> *const c_void;
    fn obs_type_catalog_enum_registry_fields(
        core: *const c_void,
        catalog: u32,
        count: *mut usize,
    ) -> *const c_void;
    fn obs_type_catalog_enum_id_fields(
        array: *const c_void,
        catalog: u32,
        index: usize,
    ) -> *const *const c_char;
}

unsafe fn enumerate(catalog: u32, index: usize, output: *mut *const c_char) -> bool {
    // SAFETY: reading the owner token does not dereference it.
    let core = unsafe { obs_core_texture_activity_core() };
    let core = NonNull::new(core.cast_mut()).expect("catalog enumeration requires a live core");
    let mut count = 0;
    // SAFETY: NULL is rejected before forming any field addresses. The caller
    // guarantees the core and registry remain live and synchronized.
    let array =
        unsafe { obs_type_catalog_enum_registry_fields(core.as_ptr(), catalog, &mut count) };
    type_catalog_enum::enumerate(
        count,
        index,
        |i| {
            let array = NonNull::new(array.cast_mut()).expect("a nonempty catalog has storage");
            // SAFETY: the safe core checks the index before this field access.
            unsafe { *obs_type_catalog_enum_id_fields(array.as_ptr(), catalog, i) }
        },
        |id| {
            // SAFETY: success requires a writable output slot. Failed lookups do
            // not call this closure, so their output may be NULL.
            unsafe { output.write(id) };
        },
    )
}

/// Return the borrowed registered input ID at `index`.
///
/// # Safety
/// A live synchronized core is required. On success `output` must be writable
/// and non-NULL. Registration owns the returned ID storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enum_input_types(index: usize, output: *mut *const c_char) -> bool {
    // SAFETY: the caller supplies the live registry and output contract.
    unsafe { enumerate(0, index, output) }
}

/// Return the borrowed registered filter ID at `index`.
///
/// # Safety
/// A live synchronized core is required. On success `output` must be writable
/// and non-NULL. Registration owns the returned ID storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enum_filter_types(index: usize, output: *mut *const c_char) -> bool {
    // SAFETY: the caller supplies the live registry and output contract.
    unsafe { enumerate(1, index, output) }
}

/// Return the borrowed registered transition ID at `index`.
///
/// # Safety
/// A live synchronized core is required. On success `output` must be writable
/// and non-NULL. Registration owns the returned ID storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enum_transition_types(
    index: usize,
    output: *mut *const c_char,
) -> bool {
    // SAFETY: the caller supplies the live registry and output contract.
    unsafe { enumerate(2, index, output) }
}

/// Return the borrowed registered output ID at `index`.
///
/// # Safety
/// A live synchronized core is required. On success `output` must be writable
/// and non-NULL. Registration owns the returned ID storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enum_output_types(index: usize, output: *mut *const c_char) -> bool {
    // SAFETY: the caller supplies the live registry and output contract.
    unsafe { enumerate(3, index, output) }
}

/// Return the borrowed registered encoder ID at `index`.
///
/// # Safety
/// A live synchronized core is required. On success `output` must be writable
/// and non-NULL. Registration owns the returned ID storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enum_encoder_types(index: usize, output: *mut *const c_char) -> bool {
    // SAFETY: the caller supplies the live registry and output contract.
    unsafe { enumerate(4, index, output) }
}

/// Return the borrowed registered service ID at `index`.
///
/// # Safety
/// A live synchronized core is required. On success `output` must be writable
/// and non-NULL. Registration owns the returned ID storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enum_service_types(index: usize, output: *mut *const c_char) -> bool {
    // SAFETY: the caller supplies the live registry and output contract.
    unsafe { enumerate(5, index, output) }
}
