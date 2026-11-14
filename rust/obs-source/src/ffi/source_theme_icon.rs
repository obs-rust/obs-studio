//! Borrow registry metadata through the existing real-header icon adapter.
use super::source_type_lookup::get_source_info;
use crate::theme_icon::{Registry, Theme};
use core::{
    ffi::{c_char, c_void},
    ptr,
};

type Callback = unsafe extern "C" fn(*mut c_void) -> *const c_char;
unsafe extern "C" {
    fn obs_source_info_icon_data(info: *const c_void) -> *mut c_void;
    fn obs_source_info_dark_icon(info: *const c_void) -> Option<Callback>;
    fn obs_source_info_light_icon(info: *const c_void) -> Option<Callback>;
}

struct Query(*const c_char);
impl Registry for Query {
    type Info = *const c_void;
    type Icon = *const c_char;
    fn icon_type(&self) -> u32 {
        // SAFETY: the exported caller guarantees the original registry/ID contract.
        unsafe { super::source_icon_type::obs_source_get_icon_type(self.0) }
    }
    fn source_info(&self) -> Option<Self::Info> {
        // SAFETY: make a distinct lookup, borrowing the same live registry and ID.
        let info = unsafe { get_source_info(self.0) };
        (!info.is_null()).then_some(info)
    }
    fn call(&self, info: Self::Info, theme: Theme) -> Option<Self::Icon> {
        // SAFETY: source_info rejected NULL; the registry entry remains live.
        // Copy the optional callback before calling it with the exact type_data.
        unsafe {
            let callback = match theme {
                Theme::Dark => obs_source_info_dark_icon(info),
                Theme::Light => obs_source_info_light_icon(info),
            };
            callback.map(|callback| callback(obs_source_info_icon_data(info)))
        }
    }
}

/// # Safety
/// OBS must be initialized. With a populated registry `id` must point to a
/// readable NUL-terminated string. Entries and callback data must stay live
/// through the synchronous callback; its result retains its original lifetime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_dark_icon(id: *const c_char) -> *const c_char {
    crate::theme_icon::get(&Query(id), Theme::Dark).unwrap_or(ptr::null())
}

/// # Safety
/// Same registry, ID, callback and borrowed result requirements as the dark getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_light_icon(id: *const c_char) -> *const c_char {
    crate::theme_icon::get(&Query(id), Theme::Light).unwrap_or(ptr::null())
}
