//! Private callback ABI. C passes operations over its live private objects.
#![allow(
    clippy::unnecessary_cast,
    reason = "C long is i32 on Windows and i64 on 64-bit Unix"
)]
use crate::item_hotkeys::{self, Item};
use core::ffi::{c_char, c_long, c_void};
use std::ffi::{CStr, CString};

pub type Owners = unsafe extern "C" fn(*mut c_void) -> c_long;
pub type Compare = unsafe extern "C" fn(*mut c_void, *mut c_long, c_long) -> bool;
pub type Visible = unsafe extern "C" fn(*const c_void) -> bool;
pub type SetVisible = unsafe extern "C" fn(*mut c_void, bool);
pub type Release = unsafe extern "C" fn(*mut c_void);
pub type Descriptions = unsafe extern "C" fn(usize, *const c_char, *const c_char);

struct ForeignItem {
    data: *mut c_void,
    owners: Owners,
    compare: Compare,
    visible: Visible,
    set_visible: SetVisible,
    release: Release,
}
impl Item for ForeignItem {
    fn owners(&mut self) -> i64 {
        // SAFETY: C supplies operations for its live callback registration's item.
        unsafe { (self.owners)(self.data) as i64 }
    }
    fn compare_owners(&mut self, expected: i64, next: i64) -> Result<(), i64> {
        let mut expected = expected as c_long;
        // SAFETY: the adapter uses the real atomic field; expected is writable.
        // Values fit C long in the original defined domain, below LONG_MAX.
        if unsafe { (self.compare)(self.data, &mut expected, next as c_long) } {
            Ok(())
        } else {
            Err(expected as i64)
        }
    }
    fn visible(&mut self) -> bool {
        // SAFETY: the core has acquired a reference before asking for visibility.
        unsafe { (self.visible)(self.data) }
    }
    fn set_visible(&mut self, visible: bool) {
        // SAFETY: the acquired reference remains held through the setter.
        unsafe { (self.set_visible)(self.data, visible) }
    }
    fn release(&mut self, acquired: bool) {
        // SAFETY: C release accepts NULL, otherwise this is the acquired reference.
        unsafe {
            (self.release)(if acquired {
                self.data
            } else {
                core::ptr::null_mut()
            })
        }
    }
}

/// # Safety
/// C must supply the live callback item and its matching synchronous operations.
#[allow(
    clippy::too_many_arguments,
    reason = "private C ABI passes operations without introducing a shared struct layout"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_hotkey_trigger(
    data: *mut c_void,
    show: bool,
    pressed: bool,
    owners: Owners,
    compare: Compare,
    visible: Visible,
    set_visible: SetVisible,
    release: Release,
) -> bool {
    item_hotkeys::trigger(
        &mut ForeignItem {
            data,
            owners,
            compare,
            visible,
            set_visible,
            release,
        },
        show,
        pressed,
    )
}

unsafe fn bytes<'a>(value: *const c_char) -> &'a [u8] {
    if value.is_null() {
        &[]
    } else {
        // SAFETY: caller guarantees a C string readable through this synchronous call.
        unsafe { CStr::from_ptr(value).to_bytes() }
    }
}

/// # Safety
/// Strings must remain readable during this call; the setter must copy both
/// descriptions synchronously, just as obs_hotkey_pair_set_descriptions does.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_hotkey_rename(
    pair: usize,
    show_template: *const c_char,
    hide_template: *const c_char,
    name: *const c_char,
    set: Descriptions,
) {
    // SAFETY: C keeps the borrowed strings live and copies the results before drop.
    unsafe {
        let name = bytes(name);
        let show = item_hotkeys::description(bytes(show_template), name)
            .map(|value| CString::new(value).expect("description contains no NUL"));
        let hide = item_hotkeys::description(bytes(hide_template), name)
            .map(|value| CString::new(value).expect("description contains no NUL"));
        set(
            pair,
            show.as_ref().map_or(core::ptr::null(), |v| v.as_ptr()),
            hide.as_ref().map_or(core::ptr::null(), |v| v.as_ptr()),
        );
    }
}
