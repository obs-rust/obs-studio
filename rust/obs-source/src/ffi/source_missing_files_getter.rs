//! The source layout and callback ABI stay at the C boundary.
use crate::missing_files_getter::{Source, get};
use core::ffi::{CStr, c_char, c_int, c_void};

pub type Callback = unsafe extern "C" fn(*mut c_void) -> *mut c_void;

#[repr(C)]
pub struct Fields {
    pub data: *mut c_void,
    pub callback: Option<Callback>,
}

unsafe extern "C" {
    fn obs_source_missing_files_getter_fields(source: *const c_void) -> Fields;
    fn obs_missing_files_create() -> *mut c_void;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-null source and its callback data must remain live for the call,
/// as required by the original C API. The caller owns the returned list.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_missing_files(source: *const c_void) -> *mut c_void {
    let source = if source.is_null() {
        None
    } else {
        // SAFETY: NULL was rejected before the private C layout accessor.
        let fields = unsafe { obs_source_missing_files_getter_fields(source) };
        Some(Source {
            data: (!fields.data.is_null()).then_some(fields.data),
            callback: fields.callback.map(|callback| {
                move |data| {
                    // SAFETY: the live source supplies this callback and context data.
                    unsafe { callback(data) }
                }
            }),
        })
    };
    get(
        source,
        || {
            // SAFETY: the collection API creates a fresh list owned by the caller.
            unsafe { obs_missing_files_create() }
        },
        |name: &'static CStr| {
            // SAFETY: static strings and the pointer argument match the %s format.
            unsafe { blog(400, c"%s: Null 'source' parameter".as_ptr(), name.as_ptr()) }
        },
    )
}
