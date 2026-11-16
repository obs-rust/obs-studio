//! Opaque item lifetime and native atomic access; no Rust item layout.
use crate::group_resize_defer::{self, ResizeDeferral};
use core::ffi::{c_char, c_int, c_long, c_void};
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_sceneitem_group_resize_increment(item: *mut c_void);
    fn obs_sceneitem_group_resize_decrement(item: *mut c_void) -> c_long;
    fn obs_sceneitem_group_resize_schedule(item: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

struct Item(NonNull<c_void>);
impl ResizeDeferral for Item {
    fn increment(&self) {
        // SAFETY: constructed only after NULL validation; caller keeps item live.
        unsafe { obs_sceneitem_group_resize_increment(self.0.as_ptr()) };
    }
    fn decrement(&self) -> isize {
        // SAFETY: same live item; C performs one native atomic RMW.
        unsafe { obs_sceneitem_group_resize_decrement(self.0.as_ptr()) as isize }
    }
    fn schedule_resize(&self) {
        // SAFETY: same live item; C atomically exchanges only the resize flag.
        unsafe { obs_sceneitem_group_resize_schedule(self.0.as_ptr()) };
    }
}

fn validated(item: *mut c_void, function: &core::ffi::CStr) -> Option<Item> {
    let item = NonNull::new(item);
    if item.is_none() {
        // SAFETY: format and both arguments are static NUL-terminated strings.
        unsafe {
            blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                function.as_ptr(),
                c"item".as_ptr(),
            )
        };
    }
    item.map(Item)
}

/// # Safety
/// A non-null item must remain live, as required by the original C API.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_defer_group_resize_begin(item: *mut c_void) {
    if let Some(item) = validated(item, c"obs_sceneitem_defer_group_resize_begin") {
        group_resize_defer::begin(&item);
    }
}

/// # Safety
/// A non-null item must remain live, as required by the original C API.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_defer_group_resize_end(item: *mut c_void) {
    if let Some(item) = validated(item, c"obs_sceneitem_defer_group_resize_end") {
        group_resize_defer::end(&item);
    }
}
