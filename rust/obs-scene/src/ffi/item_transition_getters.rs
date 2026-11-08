//! Read the private C fields and delegate all selection to the safe core.
use core::ffi::c_void;
use core::ptr::NonNull;

/// Borrowed addresses supplied by the real C layout.
#[repr(C)]
pub struct Fields {
    pub show_transition: *const *mut c_void,
    pub hide_transition: *const *mut c_void,
    pub show_duration: *const u32,
    pub hide_duration: *const u32,
}

unsafe extern "C" {
    fn obs_scene_item_transition_fields(item: *const c_void) -> Fields;
}

/// # Safety
/// A non-null item must be live and its transition fields readable under
/// the same lifetime and synchronization contract as the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_transition(
    item: *mut c_void,
    show: bool,
) -> *mut c_void {
    let values = NonNull::new(item).map(|item| {
        // SAFETY: null is excluded before the C adapter forms field addresses.
        let fields = unsafe { obs_scene_item_transition_fields(item.as_ptr()) };
        // SAFETY: the caller guarantees live, readable transition fields.
        (
            move || {
                // SAFETY: the caller guarantees this selected field is readable.
                unsafe { *fields.show_transition }
            },
            move || {
                // SAFETY: the caller guarantees this selected field is readable.
                unsafe { *fields.hide_transition }
            },
        )
    });
    crate::item_transition_getters::read_transition(values, show)
}

/// # Safety
/// A non-null item must be live and its duration fields readable under
/// the same lifetime and synchronization contract as the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_get_transition_duration(
    item: *mut c_void,
    show: bool,
) -> u32 {
    let values = NonNull::new(item).map(|item| {
        // SAFETY: null is excluded before the C adapter forms field addresses.
        let fields = unsafe { obs_scene_item_transition_fields(item.as_ptr()) };
        // SAFETY: the caller guarantees live, readable duration fields.
        (
            move || {
                // SAFETY: the caller guarantees this selected field is readable.
                unsafe { *fields.show_duration }
            },
            move || {
                // SAFETY: the caller guarantees this selected field is readable.
                unsafe { *fields.hide_duration }
            },
        )
    });
    crate::item_transition_getters::read_transition(values, show)
}
