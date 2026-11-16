//! Thin bridge to existing libobs source/data APIs. No private layout is mirrored.
use super::item_transition_getters::{
    obs_sceneitem_get_transition, obs_sceneitem_get_transition_duration,
};
use crate::item_transition_serialization::{self as engine, Host};
use core::ffi::{c_char, c_void};
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_data_create() -> *mut c_void;
    fn obs_data_get_string(data: *mut c_void, key: *const c_char) -> *const c_char;
    fn obs_data_get_obj(data: *mut c_void, key: *const c_char) -> *mut c_void;
    fn obs_data_get_int(data: *mut c_void, key: *const c_char) -> i64;
    fn obs_data_release(data: *mut c_void);
    fn obs_data_set_string(data: *mut c_void, key: *const c_char, value: *const c_char);
    fn obs_data_set_obj(data: *mut c_void, key: *const c_char, value: *mut c_void);
    fn obs_data_set_int(data: *mut c_void, key: *const c_char, value: i64);
    fn obs_source_create_private(
        id: *const c_char,
        name: *const c_char,
        settings: *mut c_void,
    ) -> *mut c_void;
    fn obs_source_release(source: *mut c_void);
    fn obs_source_get_unversioned_id(source: *mut c_void) -> *const c_char;
    fn obs_source_get_id(source: *mut c_void) -> *const c_char;
    fn obs_source_get_name(source: *mut c_void) -> *const c_char;
    fn obs_source_get_settings(source: *mut c_void) -> *mut c_void;
    fn obs_sceneitem_set_transition(item: *mut c_void, show: bool, source: *mut c_void);
    fn obs_sceneitem_set_transition_duration(item: *mut c_void, show: bool, duration: u32);
}
fn key(value: &'static str) -> *const c_char {
    match value {
        "id" => c"id".as_ptr(),
        "versioned_id" => c"versioned_id".as_ptr(),
        "name" => c"name".as_ptr(),
        "transition" => c"transition".as_ptr(),
        "duration" => c"duration".as_ptr(),
        _ => unreachable!("serialization key"),
    }
}
// Only constructed while the exported functions' live-handle contract holds.
struct C;
impl Host for C {
    type Item = *mut c_void;
    type Data = *mut c_void;
    type Source = *mut c_void;
    type Text = *const c_char;
    fn create_data(&mut self) -> Self::Data {
        // SAFETY: no preconditions; ownership is returned to the caller.
        unsafe { obs_data_create() }
    }
    fn text(&mut self, data: Self::Data, name: &'static str) -> Self::Text {
        // SAFETY: the live data owns the returned NUL-terminated string.
        unsafe { obs_data_get_string(data, key(name)) }
    }
    fn nonempty(&self, text: Self::Text) -> bool {
        // SAFETY: non-null text comes from the live data's string getter.
        !text.is_null() && unsafe { *text != 0 }
    }
    fn object(&mut self, data: Self::Data, name: &'static str) -> Self::Data {
        // SAFETY: live data and static key; returns a retained (possibly null) object.
        unsafe { obs_data_get_obj(data, key(name)) }
    }
    fn integer(&mut self, data: Self::Data, name: &'static str) -> i64 {
        // SAFETY: live data and static NUL-terminated key.
        unsafe { obs_data_get_int(data, key(name)) }
    }
    fn create_source(
        &mut self,
        id: Self::Text,
        name: Self::Text,
        settings: Self::Data,
    ) -> Self::Source {
        // SAFETY: strings are borrowed from live input data, settings is retained.
        unsafe { obs_source_create_private(id, name, settings) }
    }
    fn release_source(&mut self, source: Self::Source) {
        // SAFETY: release the creation reference, including the API's null case.
        unsafe { obs_source_release(source) }
    }
    fn release_data(&mut self, data: Self::Data) {
        // SAFETY: release exactly the reference returned by get_obj/get_settings.
        unsafe { obs_data_release(data) }
    }
    fn set_transition(&mut self, item: Self::Item, show: bool, source: Option<Self::Source>) {
        // SAFETY: live item; setter retains source before the creation reference is released.
        unsafe { obs_sceneitem_set_transition(item, show, source.unwrap_or_default()) }
    }
    fn set_duration(&mut self, item: Self::Item, show: bool, duration: u32) {
        // SAFETY: live item, caller owns the same synchronization as the C API.
        unsafe { obs_sceneitem_set_transition_duration(item, show, duration) }
    }
    fn transition(&mut self, item: Self::Item, show: bool) -> Option<Self::Source> {
        // SAFETY: live item remains valid throughout serialization.
        NonNull::new(unsafe { obs_sceneitem_get_transition(item, show) }).map(NonNull::as_ptr)
    }
    fn duration(&mut self, item: Self::Item, show: bool) -> u32 {
        // SAFETY: live item remains valid throughout serialization.
        unsafe { obs_sceneitem_get_transition_duration(item, show) }
    }
    fn source_text(&mut self, source: Self::Source, name: &'static str) -> Self::Text {
        // SAFETY: the selected slot holds a live source reference.
        unsafe {
            match name {
                "id" => obs_source_get_unversioned_id(source),
                "versioned_id" => obs_source_get_id(source),
                "name" => obs_source_get_name(source),
                _ => unreachable!("source identity key"),
            }
        }
    }
    fn settings(&mut self, source: Self::Source) -> Self::Data {
        // SAFETY: live source; getter retains settings for the core to release.
        unsafe { obs_source_get_settings(source) }
    }
    fn set_text(&mut self, data: Self::Data, name: &'static str, text: Self::Text) {
        // SAFETY: live output and source string; setter copies the string.
        unsafe { obs_data_set_string(data, key(name), text) }
    }
    fn set_object(&mut self, data: Self::Data, name: &'static str, object: Self::Data) {
        // SAFETY: live output and retained settings; setter retains its own reference.
        unsafe { obs_data_set_obj(data, key(name), object) }
    }
    fn set_integer(&mut self, data: Self::Data, name: &'static str, integer: i64) {
        // SAFETY: live output and static key.
        unsafe { obs_data_set_int(data, key(name), integer) }
    }
}
/// # Safety
/// Non-null handles must be live, with the original C API's synchronization.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_transition_load(
    item: *mut c_void,
    data: *mut c_void,
    show: bool,
) {
    engine::load(
        &mut C,
        NonNull::new(item).map(NonNull::as_ptr),
        NonNull::new(data).map(NonNull::as_ptr),
        show,
    );
}
/// # Safety
/// Item must be live and non-null; its transitions must remain live throughout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_transition_save(
    item: *mut c_void,
    show: bool,
) -> *mut c_void {
    engine::save(&mut C, item, show)
}
