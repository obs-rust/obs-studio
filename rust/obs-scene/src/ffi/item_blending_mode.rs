//! Opaque scene-item field lookup and the original C atomic operation.
use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_sceneitem_blending_mode_field(item: *mut c_void) -> *mut u32;
    fn obs_sceneitem_blending_mode_dirty(item: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-null item must be live and exclusively writable under the original
/// setter's synchronization contract for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_set_blending_mode(item: *mut c_void, value: u32) {
    let mode = if item.is_null() {
        None
    } else {
        // SAFETY: null was checked before the C field accessor; the caller owns
        // a live writable item. The C enum's size/alignment are tested.
        Some(unsafe { &mut *obs_sceneitem_blending_mode_field(item) })
    };
    crate::item_blending_mode::set(
        mode,
        value,
        || {
            // SAFETY: invoked only for a live item, after its blend field was stored.
            unsafe { obs_sceneitem_blending_mode_dirty(item) };
        },
        || {
            // SAFETY: all variadic arguments and the format are static C strings.
            unsafe {
                blog(
                    400,
                    c"%s: Null '%s' parameter".as_ptr(),
                    c"obs_sceneitem_set_blending_mode".as_ptr(),
                    c"item".as_ptr(),
                )
            };
        },
    );
}
