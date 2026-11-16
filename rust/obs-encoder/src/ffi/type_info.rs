//! Real-header snapshot conversion and borrowed encoder metadata ABI.
use crate::type_info::{self, Info};
use core::ffi::{c_char, c_void};
use obs_util::ffi::encoder_type_lookup::find_encoder;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Fields {
    pub get_name: Option<unsafe extern "C" fn(*mut c_void) -> *const c_char>,
    pub type_data: *mut c_void,
    pub codec: *const c_char,
}
unsafe extern "C" {
    fn obs_encoder_type_info_fields(info: *const c_void) -> Fields;
}
// Only an unsafe query can construct this wrapper. Public layout snapshots
// must never expose callback invocation through a safe trait implementation.
struct CInfo(Fields);
impl Info for CInfo {
    type Text = *const c_char;
    fn name(&self) -> Self::Text {
        // SAFETY: a matched name query requires the original callable callback
        // and live type_data. C likewise invokes get_name without a NULL guard.
        unsafe { self.0.get_name.unwrap_unchecked()(self.0.type_data) }
    }
    fn codec(&self) -> Self::Text {
        self.0.codec
    }
}
unsafe fn find(id: *const c_char) -> Option<CInfo> {
    // SAFETY: caller guarantees the original live-core/string contract.
    let info = unsafe { find_encoder(id) };
    if info.is_null() {
        None
    } else {
        // SAFETY: only a live, non-NULL registration reaches this field adapter.
        Some(CInfo(unsafe { obs_encoder_type_info_fields(info) }))
    }
}

/// # Safety
/// The core is live and synchronized. The ID is readable whenever its registry
/// is nonempty. A matching registration has a callable get_name and live data.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_display_name(id: *const c_char) -> *const c_char {
    // SAFETY: forward the original lookup and plugin lifetime contract.
    type_info::display_name(unsafe { find(id) }.as_ref()).unwrap_or_default()
}
/// # Safety
/// The core is live and synchronized; id is a C string whenever the registry
/// is nonempty. Returned storage remains borrowed from its registration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_encoder_codec(id: *const c_char) -> *const c_char {
    // SAFETY: forward the original lookup contract; no callback is invoked.
    type_info::codec(unsafe { find(id) }.as_ref()).unwrap_or_default()
}
