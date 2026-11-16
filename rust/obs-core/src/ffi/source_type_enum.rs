//! Borrow C registry fields; bounds and output order belong to the safe core.
use crate::source_type_enum::{self, Output};
use core::ffi::{c_char, c_void};
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_core_texture_activity_core() -> *const c_void;
    fn obs_source_type_enum_registry_fields(
        core: *const c_void,
        inputs: bool,
        count: *mut usize,
    ) -> *const c_void;
    fn obs_source_type_enum_id_fields(
        array: *const c_void,
        index: usize,
        unversioned: bool,
    ) -> *const *const c_char;
}

unsafe fn registry(inputs: bool) -> (*const c_void, usize) {
    // SAFETY: obtaining the existing global owner does not dereference it.
    let core = unsafe { obs_core_texture_activity_core() };
    let core = NonNull::new(core.cast_mut()).expect("source enumeration requires a live core");
    let mut count = 0;
    // SAFETY: NULL was rejected before the adapter forms any field addresses;
    // callers guarantee the core and registry are live and synchronized.
    let array = unsafe { obs_source_type_enum_registry_fields(core.as_ptr(), inputs, &mut count) };
    (array, count)
}

unsafe fn id(array: *const c_void, index: usize, unversioned: bool) -> *const c_char {
    let array = NonNull::new(array.cast_mut()).expect("a nonempty registry has storage");
    // SAFETY: the safe core calls this only for index < count. NULL storage is
    // rejected before the adapter forms the address of the live entry's field.
    unsafe { *obs_source_type_enum_id_fields(array.as_ptr(), index, unversioned) }
}

/// Return the registered borrowed source ID at `index`.
///
/// # Safety
/// The core and registry are live and synchronized. On success, `output` is a
/// writable non-NULL pointer slot. Returned storage remains owned by registration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enum_source_types(index: usize, output: *mut *const c_char) -> bool {
    // SAFETY: the caller supplies the live registry contract.
    let (array, count) = unsafe { registry(false) };
    source_type_enum::source(
        count,
        index,
        // SAFETY: the core bounds the index before invoking this closure.
        |i| unsafe { id(array, i, false) },
        // SAFETY: the caller supplies a writable output on successful lookup.
        |entry| unsafe { output.write(entry) },
    )
}

/// Return independently optional borrowed input-type IDs at `index`.
///
/// # Safety
/// The core and registry are live and synchronized. Each non-NULL output is a
/// writable pointer slot; they may alias. Registration owns the returned storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enum_input_types2(
    index: usize,
    output: *mut *const c_char,
    unversioned_output: *mut *const c_char,
) -> bool {
    // SAFETY: the caller supplies the live registry contract.
    let (array, count) = unsafe { registry(true) };
    source_type_enum::input(
        count,
        index,
        // SAFETY: the core bounds the index before invoking this closure.
        |i, slot| unsafe { id(array, i, slot == Output::Unversioned) },
        !output.is_null(),
        !unversioned_output.is_null(),
        |slot, entry| {
            let output = match slot {
                Output::Versioned => output,
                Output::Unversioned => unversioned_output,
            };
            // SAFETY: the safe core selects only present outputs. Raw writes
            // preserve aliasing and their C order without forming Rust references.
            unsafe { output.write(entry) };
        },
    )
}
