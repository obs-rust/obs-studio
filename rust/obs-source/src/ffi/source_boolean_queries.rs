//! C ABI over borrowed booleans without mirroring the private source layout:
//! the two getters and the centered-texture setter (#5599).

use crate::boolean_queries;
use core::ffi::c_void;

#[repr(C)]
pub struct BooleanQueryFields {
    pub texcoords_centered: *const bool,
    pub hidden: *const bool,
}

unsafe extern "C" {
    fn obs_source_boolean_query_fields(source: *const c_void) -> BooleanQueryFields;
}

/// # Safety
/// The source must be live and non-null, with the same synchronization as the
/// original C getter. The NULL guard prevents an invalid field-address call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_texcoords_centered(source: *mut c_void) -> bool {
    if source.is_null() {
        return false;
    }
    // SAFETY: the checked live source has a readable centered-texture field.
    let fields = unsafe { obs_source_boolean_query_fields(source) };
    // SAFETY: the adapter returns the address of the requested live bool.
    boolean_queries::texcoords_centered(unsafe { &*fields.texcoords_centered })
}

/// # Safety
/// The source must be live and non-null, with the same synchronization as the
/// original C getter. The NULL guard prevents an invalid field-address call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_is_hidden(source: *mut c_void) -> bool {
    if source.is_null() {
        return false;
    }
    // SAFETY: the checked live source has a readable hidden-state field.
    let fields = unsafe { obs_source_boolean_query_fields(source) };
    // SAFETY: the adapter returns the address of the requested live bool.
    boolean_queries::is_hidden(unsafe { &*fields.hidden })
}

/// # Safety
/// A non-null source must be live, with its centered-texture flag writable
/// under the same synchronization as the original C setter (a plain store, no
/// lock). NULL is outside the C contract; the guard only prevents an invalid
/// field-address call and stores nothing.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_set_texcoords_centered(source: *mut c_void, centered: bool) {
    if source.is_null() {
        return;
    }
    // SAFETY: NULL was checked; the adapter returns the live source's own
    // field address. The source is not const, so the store through that
    // address is the original C store.
    let fields = unsafe { obs_source_boolean_query_fields(source) };
    // SAFETY: the address is the source's aligned, initialized bool field,
    // written by no other party during this call under the C contract.
    boolean_queries::set_texcoords_centered(
        unsafe { &mut *fields.texcoords_centered.cast_mut() },
        centered,
    );
}
