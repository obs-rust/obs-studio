//! C ABI over borrowed source fields and the original audio mutex.
use crate::push_queries::{self, PushFields};
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct PushQueryFields {
    pub audio_mutex: *mut c_void,
    pub mute_enabled: *const bool,
    pub talk_enabled: *const bool,
    pub mute_delay: *const u64,
    pub talk_delay: *const u64,
}
unsafe extern "C" {
    fn obs_source_push_query_fields(source: *mut c_void) -> PushQueryFields;
    fn pthread_mutex_lock(mutex: *mut c_void) -> c_int;
    fn pthread_mutex_unlock(mutex: *mut c_void) -> c_int;
    fn blog(level: c_int, format: *const c_char, ...);
}
struct Source(PushQueryFields);
impl Source {
    unsafe fn borrow(source: *mut c_void) -> Option<Self> {
        if source.is_null() {
            None
        } else {
            // SAFETY: the caller provides a live source; NULL was checked before any field address is formed.
            Some(Self(unsafe { obs_source_push_query_fields(source) }))
        }
    }
}
impl PushFields for Source {
    fn lock(&mut self) {
        // SAFETY: the view borrows the initialized audio mutex of a live source.
        unsafe { pthread_mutex_lock(self.0.audio_mutex) };
    }
    fn unlock(&mut self) {
        // SAFETY: the core releases the audio mutex it just locked on this thread.
        unsafe { pthread_mutex_unlock(self.0.audio_mutex) };
    }
    fn mute_enabled(&self) -> bool {
        // SAFETY: the core reads only while holding the live source's audio mutex.
        unsafe { *self.0.mute_enabled }
    }
    fn talk_enabled(&self) -> bool {
        // SAFETY: the core reads only while holding the live source's audio mutex.
        unsafe { *self.0.talk_enabled }
    }
    fn mute_delay(&self) -> u64 {
        // SAFETY: the core reads only while holding the live source's audio mutex.
        unsafe { *self.0.mute_delay }
    }
    fn talk_delay(&self) -> u64 {
        // SAFETY: the core reads only while holding the live source's audio mutex.
        unsafe { *self.0.talk_delay }
    }
}
fn null_log(function: &'static CStr, parameter: &'static CStr) {
    // SAFETY: format and arguments are static NUL-terminated strings.
    unsafe {
        blog(
            400,
            c"%s: Null '%s' parameter".as_ptr(),
            function.as_ptr(),
            parameter.as_ptr(),
        )
    };
}

/// # Safety
/// A non-NULL source must remain live and have an initialized audio mutex.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_push_to_mute_enabled(source: *mut c_void) -> bool {
    // SAFETY: the public contract supplies a live source or NULL.
    let mut source = unsafe { Source::borrow(source) };
    push_queries::mute_enabled(source.as_mut(), null_log)
}

/// # Safety
/// A non-NULL source must remain live and have an initialized audio mutex.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_push_to_talk_enabled(source: *mut c_void) -> bool {
    // SAFETY: the public contract supplies a live source or NULL.
    let mut source = unsafe { Source::borrow(source) };
    push_queries::talk_enabled(source.as_mut(), null_log)
}

/// # Safety
/// A non-NULL source must remain live and have an initialized audio mutex.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_push_to_mute_delay(source: *mut c_void) -> u64 {
    // SAFETY: the public contract supplies a live source or NULL.
    let mut source = unsafe { Source::borrow(source) };
    push_queries::mute_delay(source.as_mut(), null_log)
}

/// # Safety
/// A non-NULL source must remain live and have an initialized audio mutex.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_push_to_talk_delay(source: *mut c_void) -> u64 {
    // SAFETY: the public contract supplies a live source or NULL.
    let mut source = unsafe { Source::borrow(source) };
    push_queries::talk_delay(source.as_mut(), null_log)
}
