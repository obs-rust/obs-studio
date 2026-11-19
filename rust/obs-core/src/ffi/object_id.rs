//! Borrow private C fields only after NULL and kind validation.
use crate::object_id::{self, Kind};
use core::ffi::{c_char, c_int, c_void};
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_object_id_kind(obj: *const c_void) -> c_int;
    fn obs_object_id_source_id(obj: *const c_void) -> *const *const c_char;
    fn obs_object_id_output_id(obj: *const c_void) -> *const *const c_char;
    fn obs_object_id_encoder_id(obj: *const c_void) -> *const *const c_char;
    fn obs_object_id_service_id(obj: *const c_void) -> *const *const c_char;
}

/// Return the concrete info ID, borrowed from the object, or NULL.
///
/// # Safety
/// A non-NULL object must start with a readable live obs_context_data. For a
/// supported kind it must have that kind's concrete layout, with the same
/// lifetime and synchronization as the original C query. The object owns the ID.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_obj_get_id(obj: *mut c_void) -> *const c_char {
    let kind = NonNull::new(obj).map(|obj| {
        // SAFETY: NULL is rejected before the real-header context projection.
        unsafe { obs_object_id_kind(obj.as_ptr()) }
    });
    object_id::query(kind, |kind| {
        // SAFETY: the safe core calls this only for a non-NULL object with a
        // supported kind. Only that concrete layout's field is projected.
        unsafe {
            let field = match kind {
                Kind::Source => obs_object_id_source_id(obj),
                Kind::Output => obs_object_id_output_id(obj),
                Kind::Encoder => obs_object_id_encoder_id(obj),
                Kind::Service => obs_object_id_service_id(obj),
            };
            *field
        }
    })
    .unwrap_or(core::ptr::null())
}
