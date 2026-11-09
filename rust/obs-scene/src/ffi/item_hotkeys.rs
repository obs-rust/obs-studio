//! Private callback ABI. C passes operations over its live private objects.
#![allow(
    clippy::unnecessary_cast,
    reason = "C long is i32 on Windows and i64 on 64-bit Unix"
)]
use crate::item_hotkeys::{self, Item, Rename};
use core::ffi::{c_char, c_long, c_void};
use std::ffi::{CStr, CString};
use std::ptr;

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

pub type NewName = unsafe extern "C" fn(*mut c_void) -> *const c_char;
pub type Template = unsafe extern "C" fn(bool) -> *const c_char;
pub type Pair = unsafe extern "C" fn(*const c_void) -> usize;

/// # Safety
/// `value` is NULL or a C string readable for this synchronous call.
unsafe fn owned(value: *const c_char) -> Option<Vec<u8>> {
    if value.is_null() {
        None
    } else {
        // SAFETY: the caller guarantees a readable C string.
        Some(unsafe { CStr::from_ptr(value) }.to_bytes().to_vec())
    }
}

struct ForeignRename {
    item: *mut c_void,
    data: *mut c_void,
    new_name: NewName,
    template: Template,
    pair: Pair,
    set: Descriptions,
}

impl Rename for ForeignRename {
    fn new_name(&mut self) -> Option<Vec<u8>> {
        // SAFETY: C passes the signal's live calldata; the string is copied
        // before the call returns.
        unsafe { owned((self.new_name)(self.data)) }
    }
    fn template(&mut self, show: bool) -> Option<Vec<u8>> {
        // SAFETY: the adapter returns the current `obs->hotkeys` template,
        // copied before any other call.
        unsafe { owned((self.template)(show)) }
    }
    fn pair(&mut self) -> usize {
        // SAFETY: the item is the live one the handler was connected with.
        unsafe { (self.pair)(self.item) }
    }
    fn set_descriptions(&mut self, pair: usize, show: Option<&[u8]>, hide: Option<&[u8]>) {
        // Descriptions hold no NUL: `description` stops each input at its
        // first NUL, as C strings do.
        let show = show.map(|v| CString::new(v).expect("description contains no NUL"));
        let hide = hide.map(|v| CString::new(v).expect("description contains no NUL"));
        // SAFETY: the setter copies both strings before returning, as
        // obs_hotkey_pair_set_descriptions does.
        unsafe {
            (self.set)(
                pair,
                show.as_ref().map_or(ptr::null(), |v| v.as_ptr()),
                hide.as_ref().map_or(ptr::null(), |v| v.as_ptr()),
            );
        }
    }
}

/// `sceneitem_renamed()`, each read made through C at its original point.
///
/// # Safety
/// `item` is the live scene item the handler was connected with, `data`
/// the signal's calldata, and the operations are the private adapter's,
/// all valid for this synchronous call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_hotkey_rename(
    item: *mut c_void,
    data: *mut c_void,
    new_name: NewName,
    template: Template,
    pair: Pair,
    set: Descriptions,
) {
    item_hotkeys::rename(&mut ForeignRename {
        item,
        data,
        new_name,
        template,
        pair,
        set,
    });
}
