//! Lock setter C ABI; the existing, real-header-tested item prefix is reused.
use super::item_state_getters::SceneItemPrefix;
use core::ffi::c_void;

unsafe extern "C" {
    fn obs_sceneitem_locked_signal(item: *mut SceneItemPrefix, parent: *mut c_void, locked: bool);
}

/// Update the stored lock flag before synchronously notifying the parent.
///
/// # Safety
/// A non-null item must be live and writable under the original C setter's
/// synchronization contract. Its parent and signal handler must remain live
/// until notification completes. Callbacks may access or remove the item.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_set_locked(item: *mut SceneItemPrefix, lock: bool) -> bool {
    let (current, parent) = if item.is_null() {
        (None, core::ptr::null_mut())
    } else {
        // SAFETY: caller supplies a complete live item; read only these fields.
        unsafe {
            (
                Some(core::ptr::addr_of!((*item).locked).read()),
                core::ptr::addr_of!((*item).parent).read(),
            )
        }
    };
    let Some(value) = crate::item_locked_setter::set_locked(current, !parent.is_null(), lock)
    else {
        return false;
    };
    // SAFETY: the decision requires a live item and parent. Write only locked;
    // the adapter emits the original calldata payload. Do not access item after
    // callbacks, which may remove it. No Rust reference crosses the callback.
    unsafe {
        core::ptr::addr_of_mut!((*item).locked).write(value);
        obs_sceneitem_locked_signal(item, parent, value);
    }
    true
}
