//! Borrow scalar fields without mirroring the private C source layout.
use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_source_audio_mixers_owned_field(source: *const c_void) -> bool;
    fn obs_source_audio_mixers_flags_field(source: *const c_void) -> u32;
    fn obs_source_audio_mixers_mask_field(source: *const c_void) -> u32;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// Constructed only for a live, non-NULL borrowed source.
struct BorrowedSource(*const c_void);

impl crate::audio_mixers_getter::Source for BorrowedSource {
    fn owns_info_id(&self) -> bool {
        // SAFETY: the shim checks NULL before constructing this borrow; the
        // caller retains a live source under the original C access contract.
        unsafe { obs_source_audio_mixers_owned_field(self.0) }
    }
    fn output_flags(&self) -> u32 {
        // SAFETY: the borrowed source is live and non-NULL; only flags are read.
        unsafe { obs_source_audio_mixers_flags_field(self.0) }
    }
    fn audio_mixers(&self) -> u32 {
        // SAFETY: the borrowed source is live and non-NULL; only mixers are read.
        unsafe { obs_source_audio_mixers_mask_field(self.0) }
    }
}

/// # Safety
/// A non-NULL source must remain live with its fields readable under the same
/// ownership and synchronization rules as the original C getter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_audio_mixers(source: *const c_void) -> u32 {
    // No field adapter may run until NULL has been rejected.
    let source = (!source.is_null()).then_some(BorrowedSource(source));
    crate::audio_mixers_getter::get(source.as_ref(), |level, message| {
        // SAFETY: the literal format and message are NUL-terminated C strings.
        unsafe { blog(level, c"%s".as_ptr(), message.as_ptr()) }
    })
}
