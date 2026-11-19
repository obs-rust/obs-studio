//! Source layout stays in C; canvas acquisition uses the existing weak API.
use core::ffi::c_void;

unsafe extern "C" {
    fn obs_source_canvas_getter_weak(source: *const c_void) -> *mut c_void;
    fn obs_weak_canvas_get_canvas(weak: *mut c_void) -> *mut c_void;
}

/// # Safety
/// Source must be non-null and live, retaining its stored weak canvas control
/// throughout this call. The caller owns the returned strong canvas reference.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_canvas(source: *const c_void) -> *mut c_void {
    assert!(!source.is_null(), "obs_source_get_canvas requires a source");
    // SAFETY: reject NULL before the private C field accessor; caller retains source.
    let weak = unsafe { obs_source_canvas_getter_weak(source) };
    crate::canvas_getter::get(weak, |weak| {
        // SAFETY: source retains its weak control. The existing API accepts NULL,
        // rejects expired controls and acquires exactly one strong reference.
        unsafe { obs_weak_canvas_get_canvas(weak) }
    })
}
