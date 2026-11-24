//! The source layout stays behind the address adapters of
//! `obs-source-audio-buffer-getters-fields.c.h`; the shim takes and releases
//! the source's `audio_buf_mutex` itself around the `audio_ts` read, as the
//! push queries take `audio_mutex`.
use crate::audio_buffer_getters::{
    MAX_AUDIO_CHANNELS, MAX_AUDIO_MIXES, Mix, OutputBuffers, Timestamp, mix, timestamp,
};
use core::ffi::{CStr, c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_source_audio_buf_mutex_field(source: *const c_void) -> *mut c_void;
    fn obs_source_audio_ts_field(source: *const c_void) -> *const u64;
    fn obs_source_audio_output_buf_field(source: *const c_void) -> *const *mut f32;
    fn pthread_mutex_lock(mutex: *mut c_void) -> c_int;
    fn pthread_mutex_unlock(mutex: *mut c_void) -> c_int;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// Constructed only for a live, non-NULL source.
struct BorrowedSource(*const c_void);

impl Timestamp for BorrowedSource {
    fn lock(&mut self) {
        // SAFETY: a live source's initialized audio_buf_mutex; the adapter
        // casts away const exactly as the original C did.
        unsafe { pthread_mutex_lock(obs_source_audio_buf_mutex_field(self.0)) };
    }
    fn audio_ts(&self) -> u64 {
        // SAFETY: the core reads only while holding the live source's
        // audio_buf_mutex, which every writer of audio_ts holds.
        unsafe { obs_source_audio_ts_field(self.0).read() }
    }
    fn unlock(&mut self) {
        // SAFETY: the core releases the mutex it just took on this thread.
        unsafe { pthread_mutex_unlock(obs_source_audio_buf_mutex_field(self.0)) };
    }
}

impl OutputBuffers for BorrowedSource {
    type Buffer = *mut f32;
    fn output_buf(&self, mix: usize, channel: usize) -> *mut f32 {
        debug_assert!(mix < MAX_AUDIO_MIXES && channel < MAX_AUDIO_CHANNELS);
        // SAFETY: the adapter returns audio_output_buf[0][0] of a live
        // source, a contiguous float *[MAX_AUDIO_MIXES][MAX_AUDIO_CHANNELS];
        // the core asks only for slots inside it. Only the pointer is read.
        unsafe {
            obs_source_audio_output_buf_field(self.0)
                .add(mix * MAX_AUDIO_CHANNELS + channel)
                .read()
        }
    }
}

fn null_log(function: &'static CStr, parameter: &'static CStr) {
    // SAFETY: static strings match the two %s arguments of the format.
    unsafe {
        blog(
            400,
            c"%s: Null '%s' parameter".as_ptr(),
            function.as_ptr(),
            parameter.as_ptr(),
        );
    }
}

/// # Safety
/// A non-NULL source must be live with an initialized `audio_buf_mutex`,
/// under the original libobs ownership contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_audio_timestamp(source: *const c_void) -> u64 {
    // No adapter runs until NULL has been rejected.
    let mut source = (!source.is_null()).then_some(BorrowedSource(source));
    timestamp(source.as_mut(), null_log)
}

/// # Safety
/// A non-NULL source must be live under the original libobs ownership
/// contract; a non-NULL `audio` must point to a writable
/// `struct obs_source_audio_mix`, which is `float *[MAX_AUDIO_MIXES]
/// [MAX_AUDIO_CHANNELS]` in layout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_audio_mix(source: *const c_void, audio: *mut c_void) {
    let source = (!source.is_null()).then_some(BorrowedSource(source));
    // SAFETY: the caller's audio mix, NULL or writable; it is formed into a
    // reference only after the NULL check, and nothing else borrows it.
    let audio = unsafe { audio.cast::<Mix<*mut f32>>().as_mut() };
    mix(source.as_ref(), audio, null_log);
}
