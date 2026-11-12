//! Real-header snapshot and borrowed source-type metadata ABI.
use super::source_type_lookup::get_source_info;
use crate::type_info::{self, Info};
use core::ffi::{c_char, c_void};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Fields {
    pub get_name: Option<unsafe extern "C" fn(*mut c_void) -> *const c_char>,
    pub type_data: *mut c_void,
    pub output_flags: u32,
    pub has_properties: bool,
    pub has_properties2: bool,
}
unsafe extern "C" {
    fn obs_source_type_info_fields(info: *const c_void) -> Fields;
}
// Only the unsafe query constructs this wrapper. Layout snapshots must not
// expose unchecked callback invocation through a public safe implementation.
struct CInfo(Fields);
impl Info for CInfo {
    type Text = *const c_char;
    fn name(&self) -> Self::Text {
        // SAFETY: a matched name query requires a callable get_name and live
        // type_data. Original C invokes it without a missing-callback guard.
        unsafe { self.0.get_name.unwrap_unchecked()(self.0.type_data) }
    }
    fn flags(&self) -> u32 {
        self.0.output_flags
    }
    fn has_properties(&self) -> bool {
        self.0.has_properties
    }
    fn has_properties2(&self) -> bool {
        self.0.has_properties2
    }
}
unsafe fn find(id: *const c_char) -> Option<CInfo> {
    // SAFETY: caller supplies the original core/registry/ID contract.
    let info = unsafe { get_source_info(id) };
    if info.is_null() {
        None
    } else {
        // SAFETY: only a non-NULL live registration reaches the field adapter.
        Some(CInfo(unsafe { obs_source_type_info_fields(info) }))
    }
}

/// # Safety
/// Core/registry are live and synchronized; ID satisfies the existing lookup
/// contract. A matching get_name is callable with its live type_data. Returned
/// storage is borrowed from the plugin and may be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_display_name(id: *const c_char) -> *const c_char {
    // SAFETY: forward the original lookup and plugin lifetime contract.
    type_info::display_name(unsafe { find(id) }.as_ref()).unwrap_or_default()
}
/// # Safety
/// Core/registry are live and synchronized; ID satisfies the lookup contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_source_output_flags(id: *const c_char) -> u32 {
    // SAFETY: forward the original lookup contract; no callback is invoked.
    type_info::output_flags(unsafe { find(id) }.as_ref())
}
/// # Safety
/// Core/registry are live and synchronized; ID satisfies the lookup contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_is_source_configurable(id: *const c_char) -> bool {
    // SAFETY: forward the original lookup contract; no callback is invoked.
    type_info::configurable(unsafe { find(id) }.as_ref())
}
