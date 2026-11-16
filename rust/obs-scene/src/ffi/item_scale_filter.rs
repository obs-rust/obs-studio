//! Scale-filter setter over the already layout-checked render prefix.
use super::item_render_getters::SceneItemRenderPrefix;
use core::ffi::{c_char, c_int};
use core::sync::atomic::AtomicBool;

unsafe extern "C" {
    fn blog(level: c_int, format: *const c_char, ...);
}

/// Store the filter, then publish transform invalidation with the C ordering.
///
/// # Safety
/// A non-null item must be live, and scale_filter exclusively writable under
/// the original setter's synchronization contract. Concurrent accesses to
/// update_transform must be atomic; no reference to the whole item is formed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_set_scale_filter(
    item: *mut SceneItemRenderPrefix,
    filter: u32,
) {
    let fields = if item.is_null() {
        None
    } else {
        // SAFETY: caller supplies a live item with exclusive filter access.
        // AtomicBool has bool's size/alignment; from_ptr borrows only that field.
        Some(unsafe {
            (
                &mut *core::ptr::addr_of_mut!((*item).scale_filter),
                AtomicBool::from_ptr(core::ptr::addr_of_mut!((*item).update_transform)),
            )
        })
    };
    crate::item_scale_filter::set(fields, filter, || {
        // SAFETY: format and arguments are static, NUL-terminated C strings.
        unsafe {
            blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                c"obs_sceneitem_set_scale_filter".as_ptr(),
                c"item".as_ptr(),
            )
        };
    });
}
