//! The raw-audio removal wrapper reuses the existing main-audio getter and
//! media-io disconnect export. No core layout, locking or registration logic.
use crate::raw_audio_remove::{Audio, remove};
use core::ffi::c_void;
pub use obs_media_io::ffi::audio_io::audio_output_callback_t as Callback;
use obs_media_io::ffi::audio_io::audio_output_disconnect;

unsafe extern "C" {
    fn obs_get_audio() -> *mut c_void;
}

/// External operations, replaceable by the original-C differential recorder.
pub struct Calls {
    pub main_audio: unsafe extern "C" fn() -> *mut c_void,
    pub disconnect: unsafe extern "C" fn(*mut c_void, usize, Callback, *mut c_void),
}

struct Seam<'a>(&'a mut Calls);

impl Audio for Seam<'_> {
    type Output = *mut c_void;
    type Callback = Callback;
    type Param = *mut c_void;

    fn main_audio(&mut self) -> Self::Output {
        // SAFETY: Calls is used under remove_with's initialized-core contract.
        unsafe { (self.0.main_audio)() }
    }

    fn disconnect(
        &mut self,
        output: Self::Output,
        mix: usize,
        callback: Callback,
        param: *mut c_void,
    ) {
        // SAFETY: remove_with's contract covers the borrowed output and tuple.
        unsafe { (self.0.disconnect)(output, mix, callback, param) };
    }
}

/// Run the same ABI conversion and safe core with supplied external operations.
///
/// # Safety
/// The getter must return a live borrowed audio output or NULL, and disconnect
/// must accept that output and the caller's callback/parameter tuple. The core
/// must be initialized; registration lifetimes follow audio_output_disconnect.
pub unsafe fn remove_with(mix: usize, callback: Callback, param: *mut c_void, calls: &mut Calls) {
    remove(&mut Seam(calls), mix, callback, param);
}

/// Remove one callback/parameter registration from the main audio output.
///
/// # Safety
/// OBS must be initialized. The audio output, callback and parameter follow the
/// existing audio_output_disconnect lifetime and synchronization contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_remove_raw_audio_callback(
    mix: usize,
    callback: Callback,
    param: *mut c_void,
) {
    // SAFETY: both existing operations receive the caller's unchanged tuple and
    // the initialized core's borrowed audio output (which may be NULL).
    unsafe {
        remove_with(
            mix,
            callback,
            param,
            &mut Calls {
                main_audio: obs_get_audio,
                disconnect: audio_output_disconnect,
            },
        );
    }
}
