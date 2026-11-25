//! The source layout stays behind the field adapters of
//! `obs-source-audio-pending-speaker-layout-fields.c.h`; the removed flag is
//! the obs-source export `obs_source_removed()`, as the C called it.
use crate::audio_pending_speaker_layout::{Pending, audio_pending, speaker_layout};
use crate::ffi::source_removed::obs_source_removed;
use core::ffi::{CStr, c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_source_audio_pending_flags_field(source: *const c_void) -> u32;
    fn obs_source_audio_pending_field(source: *const c_void) -> bool;
    fn obs_source_speaker_layout_field(source: *const c_void) -> c_int;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// Constructed only for a live, non-NULL source.
struct BorrowedSource(*const c_void);

impl Pending for BorrowedSource {
    fn removed(&self) -> bool {
        // SAFETY: a live, non-NULL source, so the export reads its flag and
        // logs nothing.
        unsafe { obs_source_removed(self.0) }
    }
    fn output_flags(&self) -> u32 {
        // SAFETY: NULL was rejected before the private C layout accessor.
        unsafe { obs_source_audio_pending_flags_field(self.0) }
    }
    fn audio_pending(&self) -> bool {
        // SAFETY: NULL was rejected before the private C layout accessor.
        unsafe { obs_source_audio_pending_field(self.0) }
    }
}

fn null_log(function: &'static CStr) {
    // SAFETY: static strings match the two %s arguments of the format.
    unsafe {
        blog(
            400,
            c"%s: Null '%s' parameter".as_ptr(),
            function.as_ptr(),
            c"source".as_ptr(),
        );
    }
}

/// # Safety
/// A non-NULL source must be live under the original libobs ownership
/// contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_audio_pending(source: *const c_void) -> bool {
    // No adapter runs until NULL has been rejected.
    let source = (!source.is_null()).then_some(BorrowedSource(source));
    audio_pending(source.as_ref(), null_log)
}

/// # Safety
/// A non-NULL source must be live under the original libobs ownership
/// contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_speaker_layout(source: *mut c_void) -> c_int {
    let speakers = (!source.is_null()).then_some(|| {
        // SAFETY: NULL was rejected before the private C layout accessor.
        unsafe { obs_source_speaker_layout_field(source) }
    });
    speaker_layout(speakers, null_log)
}
