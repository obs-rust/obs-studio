//! Borrow registry fields without exposing the private C layout to Rust.
use crate::latest_input_type_id;
use core::ffi::{CStr, c_char, c_void};
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
    fn obs_latest_input_type_id_version_fields(array: *const c_void, index: usize) -> u32;
    fn obs_latest_input_type_id_assert_fields(found: bool);
}

/// Return the registry's borrowed newest ID, retaining signed version ordering.
///
/// # Safety
/// A non-NULL query is a readable NUL-terminated string and requires a live,
/// synchronized core with valid registry strings. A missing non-NULL query
/// retains C's debug assertion. Registration owns the returned ID storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_latest_input_type_id(query: *const c_char) -> *const c_char {
    if query.is_null() {
        return core::ptr::null();
    }
    // SAFETY: the caller supplies a readable NUL-terminated query.
    let query = unsafe { CStr::from_ptr(query) };
    // SAFETY: obtaining the owner token does not dereference it.
    let core = unsafe { obs_core_texture_activity_core() };
    let core = NonNull::new(core.cast_mut()).expect("latest type query requires a live core");
    let mut count = 0;
    // SAFETY: NULL is rejected before any field address is formed. The caller
    // keeps the complete source registry live and synchronized.
    let array = unsafe { obs_source_type_enum_registry_fields(core.as_ptr(), false, &mut count) };
    let selected = latest_input_type_id::latest(Some(query.to_bytes()), count, |i| {
        let array = NonNull::new(array.cast_mut()).expect("a nonempty registry has storage");
        // SAFETY: the safe core bounds i before these field accesses. Registry
        // names are live NUL-terminated strings; IDs are borrowed without copying.
        unsafe {
            let name = *obs_source_type_enum_id_fields(array.as_ptr(), i, true);
            let version = obs_latest_input_type_id_version_fields(array.as_ptr(), i);
            (CStr::from_ptr(name).to_bytes(), version, i)
        }
    });
    // SAFETY: a scalar adapter preserves the C build's exact assert/NDEBUG mode.
    unsafe { obs_latest_input_type_id_assert_fields(selected.is_some()) };
    selected.map_or(core::ptr::null(), |i| {
        // SAFETY: a selected entry is in bounds and array is non-NULL above.
        unsafe { *obs_source_type_enum_id_fields(array, i, false) }
    })
}
