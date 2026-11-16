//! Real-header snapshots and callback conversion for borrowed type metadata.
use super::output_type_lookup;
use crate::output_type_info::{self as type_core, Info};
use core::ffi::{c_char, c_void};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Fields {
    pub get_name: Option<unsafe extern "C" fn(*mut c_void) -> *const c_char>,
    pub type_data: *mut c_void,
    pub flags: u32,
    pub video: *const c_char,
    pub audio: *const c_char,
}
unsafe extern "C" {
    fn obs_output_type_info_fields(info: *const c_void) -> Fields;
}
// Constructed only by the unsafe queries; never expose callback invocation as a
// safe method on the publicly measurable snapshot.
struct CInfo(Fields);
impl Info for CInfo {
    type Text = *const c_char;
    fn name(&self) -> Self::Text {
        // SAFETY: a matched display-name query requires a callable get_name, just
        // as C does. The live registration owns the callback and type_data.
        unsafe { self.0.get_name.unwrap_unchecked()(self.0.type_data) }
    }
    fn flags(&self) -> u32 {
        self.0.flags
    }
    fn video_codecs(&self) -> Self::Text {
        self.0.video
    }
    fn audio_codecs(&self) -> Self::Text {
        self.0.audio
    }
}
unsafe fn find(id: *const c_char) -> Option<CInfo> {
    // SAFETY: preserve the existing lookup and its early NULL-ID return.
    unsafe { output_type_lookup::find(id) }.map(|info| {
        // SAFETY: lookup rejected NULL before this real-header field access.
        CInfo(unsafe { obs_output_type_info_fields(info.as_ptr()) })
    })
}

/// # Safety
/// NULL is valid without a core. Otherwise the core is live and synchronized,
/// and id is readable when a registered ID is compared. A match has get_name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_display_name(id: *const c_char) -> *const c_char {
    // SAFETY: forward the original lookup contract.
    type_core::display_name(unsafe { find(id) }.as_ref()).unwrap_or_default()
}
/// # Safety
/// NULL is valid without a core. Otherwise the core is live and synchronized,
/// and id is readable when a registered ID is compared.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_output_flags(id: *const c_char) -> u32 {
    // SAFETY: forward the original lookup contract.
    type_core::flags(unsafe { find(id) }.as_ref())
}
/// # Safety
/// Same live-core and borrowed-ID contract as obs_get_output_flags.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_output_supported_video_codecs(id: *const c_char) -> *const c_char {
    // SAFETY: forward the original lookup contract.
    type_core::video_codecs(unsafe { find(id) }.as_ref()).unwrap_or_default()
}
/// # Safety
/// Same live-core and borrowed-ID contract as obs_get_output_flags.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_output_supported_audio_codecs(id: *const c_char) -> *const c_char {
    // SAFETY: forward the original lookup contract.
    type_core::audio_codecs(unsafe { find(id) }.as_ref()).unwrap_or_default()
}
