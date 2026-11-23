//! The raw-audio add wrapper reuses the existing main-audio getter and
//! media-io connect export. No core layout, locking or registration logic.
use crate::raw_audio_add::{Audio, add};
use core::ffi::c_void;
pub use obs_media_io::ffi::audio_io::audio_convert_info as Conversion;
pub use obs_media_io::ffi::audio_io::audio_output_callback_t as Callback;
use obs_media_io::ffi::audio_io::audio_output_connect;

unsafe extern "C" {
    fn obs_get_audio() -> *mut c_void;
}

/// External operations, replaceable by the original-C differential recorder.
pub struct Calls {
    pub main_audio: unsafe extern "C" fn() -> *mut c_void,
    pub connect:
        unsafe extern "C" fn(*mut c_void, usize, *const Conversion, Callback, *mut c_void) -> bool,
}

struct Seam<'a>(&'a mut Calls);

impl Audio for Seam<'_> {
    type Output = *mut c_void;
    type Conversion = *const Conversion;
    type Callback = Callback;
    type Param = *mut c_void;

    fn main_audio(&mut self) -> Self::Output {
        // SAFETY: Calls is used under add_with's initialized-core contract.
        unsafe { (self.0.main_audio)() }
    }

    fn connect(
        &mut self,
        output: Self::Output,
        mix: usize,
        conversion: *const Conversion,
        callback: Callback,
        param: *mut c_void,
    ) {
        // SAFETY: add_with's contract covers the borrowed output, the
        // conversion pointer and the tuple. The C ignores the result too.
        let _ = unsafe { (self.0.connect)(output, mix, conversion, callback, param) };
    }
}

/// Run the same ABI conversion and safe core with supplied external operations.
///
/// # Safety
/// The getter must return a live borrowed audio output or NULL, and connect
/// must accept that output, the conversion (NULL or valid) and the caller's
/// callback/parameter tuple. The core must be initialized; registration
/// lifetimes follow audio_output_connect.
pub unsafe fn add_with(
    mix: usize,
    conversion: *const Conversion,
    callback: Callback,
    param: *mut c_void,
    calls: &mut Calls,
) {
    add(&mut Seam(calls), mix, conversion, callback, param);
}

/// Add one callback/parameter registration to the main audio output.
///
/// # Safety
/// OBS must be initialized. The conversion is NULL or valid for the call; the
/// audio output, callback and parameter follow the existing
/// audio_output_connect lifetime and synchronization contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_add_raw_audio_callback(
    mix: usize,
    conversion: *const Conversion,
    callback: Callback,
    param: *mut c_void,
) {
    // SAFETY: both existing operations receive the caller's unchanged
    // arguments and the initialized core's borrowed audio output (maybe NULL).
    unsafe {
        add_with(
            mix,
            conversion,
            callback,
            param,
            &mut Calls {
                main_audio: obs_get_audio,
                connect: audio_output_connect,
            },
        );
    }
}
