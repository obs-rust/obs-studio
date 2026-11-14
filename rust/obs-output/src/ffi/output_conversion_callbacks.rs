//! ABI conversion and private C layout/mutex access.
use crate::output_conversion_callbacks::{self as engine, Rejection};
use core::ffi::{CStr, c_char, c_int, c_void};
use obs_util::ffi::darray::{darray, darray_erase, darray_insert};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Video {
    pub format: c_int,
    pub width: u32,
    pub height: u32,
    pub range: c_int,
    pub colorspace: c_int,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Audio {
    pub samples_per_sec: u32,
    pub format: c_int,
    pub speakers: c_int,
    pub allow_clipping: bool,
}
pub type Callback =
    Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void)>;
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PacketCallback {
    pub callback: Callback,
    pub param: *mut c_void,
}
impl PartialEq for PacketCallback {
    fn eq(&self, other: &Self) -> bool {
        self.callback.map(|f| f as usize) == other.callback.map(|f| f as usize)
            && self.param == other.param
    }
}
unsafe extern "C" {
    fn obs_output_conversion_flags(output: *const c_void) -> u32;
    fn obs_output_conversion_name(output: *const c_void) -> *const c_char;
    fn obs_output_conversion_video(output: *mut c_void) -> *mut Video;
    fn obs_output_conversion_audio(output: *mut c_void) -> *mut Audio;
    fn obs_output_conversion_set(output: *mut c_void, video: bool) -> *mut bool;
    fn obs_output_packet_callbacks_lock(output: *mut c_void) -> *mut darray;
    fn obs_output_packet_callbacks_unlock(output: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}
unsafe fn set<T: Copy>(
    output: *mut c_void,
    conversion: *const T,
    media: u32,
    name: &CStr,
    target: unsafe extern "C" fn(*mut c_void) -> *mut T,
) {
    if let Err(reason) = engine::store_reading(
        !output.is_null(),
        !conversion.is_null(),
        // SAFETY: store_reading calls this only after both pointer checks,
        // once per flag check, as the C flag helpers read `info.flags`.
        || unsafe { obs_output_conversion_flags(output) },
        media,
        // SAFETY: store invokes these only after both guards and flag checks.
        || unsafe { *conversion },
        // SAFETY: same contract; copy precedes mutation, including aliased input.
        |value| unsafe {
            *target(output) = value;
            *obs_output_conversion_set(output, media == 1) = true;
        },
    ) {
        // SAFETY: static printf strings match arguments; output name is
        // accessed only for the non-null, non-conversion rejections.
        unsafe {
            match reason {
                Rejection::Output => {
                    blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr())
                }
                Rejection::Conversion => blog(
                    400,
                    c"%s: Null 'conversion' parameter".as_ptr(),
                    name.as_ptr(),
                ),
                Rejection::Encoded | Rejection::Media => {
                    let kind = if reason == Rejection::Encoded {
                        c"an encoded"
                    } else if media == 1 {
                        c"a non-video"
                    } else {
                        c"a non-audio"
                    };
                    blog(
                        200,
                        c"Output '%s': Tried to use %s on %s output".as_ptr(),
                        obs_output_conversion_name(output),
                        name.as_ptr(),
                        kind.as_ptr(),
                    );
                }
            }
        }
    }
}
/// # Safety
/// Non-null arguments obey the original output setter lifetime/access contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_set_video_conversion(
    output: *mut c_void,
    conversion: *const Video,
) {
    // SAFETY: forwarded original C contract; set checks null first.
    unsafe {
        set(
            output,
            conversion,
            1,
            c"obs_output_set_video_conversion",
            obs_output_conversion_video,
        )
    }
}
/// # Safety
/// Non-null arguments obey the original output setter lifetime/access contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_set_audio_conversion(
    output: *mut c_void,
    conversion: *const Audio,
) {
    // SAFETY: forwarded original C contract; set checks null first.
    unsafe {
        set(
            output,
            conversion,
            2,
            c"obs_output_set_audio_conversion",
            obs_output_conversion_audio,
        )
    }
}
unsafe fn packet(
    output: *mut c_void,
    callback: Callback,
    param: *mut c_void,
    add: bool,
    name: &CStr,
) {
    if engine::packet_guard(!output.is_null()).is_err() {
        // SAFETY: static printf string matching its one string argument.
        unsafe { blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr()) };
        return;
    }
    let entry = PacketCallback { callback, param };
    // SAFETY: the output is non-NULL and, as C requires, live; the lock keeps
    // its darray stable. darray helpers use libobs bmem, as the C owner requires.
    unsafe {
        let array = obs_output_packet_callbacks_lock(output);
        if add {
            darray_insert(
                size_of::<PacketCallback>(),
                array,
                engine::insertion_index(),
                (&entry as *const PacketCallback).cast(),
            );
        } else {
            let entries = if (*array).num == 0 {
                &[]
            } else {
                ::core::slice::from_raw_parts((*array).array.cast::<PacketCallback>(), (*array).num)
            };
            if let Some(index) = engine::removal_index(entries, &entry) {
                darray_erase(size_of::<PacketCallback>(), array, index);
            }
        }
        obs_output_packet_callbacks_unlock(output);
    }
}
/// # Safety
/// A non-NULL output is live with an initialized callback mutex/list, as
/// required by C. A NULL output logs and returns (#4754).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_add_packet_callback(
    output: *mut c_void,
    callback: Callback,
    param: *mut c_void,
) {
    // SAFETY: caller supplies the original registration contract.
    unsafe {
        packet(
            output,
            callback,
            param,
            true,
            c"obs_output_add_packet_callback",
        )
    }
}
/// # Safety
/// A non-NULL output is live with an initialized callback mutex/list, as
/// required by C. A NULL output logs and returns (#4754).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_remove_packet_callback(
    output: *mut c_void,
    callback: Callback,
    param: *mut c_void,
) {
    // SAFETY: caller supplies the original registration contract.
    unsafe {
        packet(
            output,
            callback,
            param,
            false,
            c"obs_output_remove_packet_callback",
        )
    }
}
