//! Opaque bridge to the private C FFmpeg adapter. No FFmpeg layouts cross it.
use crate::image_decode::{self, Decoder, Status, Warning};
use core::ffi::{c_int, c_void};

unsafe extern "C" {
    fn gs_image_decode_allocate(info: *mut c_void, alpha_mode: c_int) -> *mut c_void;
    fn gs_image_decode_read(session: *mut c_void) -> c_int;
    fn gs_image_decode_send(session: *mut c_void) -> c_int;
    fn gs_image_decode_receive(session: *mut c_void) -> c_int;
    fn gs_image_decode_rejected(session: *mut c_void) -> bool;
    fn gs_image_decode_check_size(session: *mut c_void) -> bool;
    fn gs_image_decode_update(session: *mut c_void);
    fn gs_image_decode_reformat(session: *mut c_void) -> *mut c_void;
    fn gs_image_decode_warn(info: *mut c_void, kind: c_int, error: c_int);
    fn gs_image_decode_cleanup(session: *mut c_void);
}

struct Adapter {
    info: *mut c_void,
    session: *mut c_void,
    alpha_mode: c_int,
}

fn status(code: c_int) -> Status {
    match code {
        0 => Status::Ready,
        1 => Status::Retry,
        other => Status::Error(other),
    }
}

impl Decoder for Adapter {
    type Output = *mut c_void;
    fn allocate(&mut self) -> bool {
        // SAFETY: entry point requires a live, initialized private image.
        self.session = unsafe { gs_image_decode_allocate(self.info, self.alpha_mode) };
        !self.session.is_null()
    }
    fn read(&mut self) -> Result<(), i32> {
        // SAFETY: core calls only after successful allocation, before cleanup.
        let result = unsafe { gs_image_decode_read(self.session) };
        if result < 0 { Err(result) } else { Ok(()) }
    }
    fn send(&mut self) -> Status {
        // SAFETY: the allocated session and its packet remain live.
        status(unsafe { gs_image_decode_send(self.session) })
    }
    fn receive(&mut self) -> Status {
        // SAFETY: the allocated session and its frame remain live.
        status(unsafe { gs_image_decode_receive(self.session) })
    }
    fn too_large(&mut self) -> bool {
        // SAFETY: session owns a live image reference until cleanup.
        unsafe { gs_image_decode_rejected(self.session) }
    }
    fn check_size(&mut self) -> bool {
        // SAFETY: core calls after receiving a frame in the live session.
        unsafe { gs_image_decode_check_size(self.session) }
    }
    fn update(&mut self) {
        // SAFETY: core calls after validating the received frame.
        unsafe { gs_image_decode_update(self.session) }
    }
    fn reformat(&mut self) -> Self::Output {
        // SAFETY: frame and image are live and their dimensions validated.
        unsafe { gs_image_decode_reformat(self.session) }
    }
    fn warn(&mut self, warning: Warning) {
        let (kind, error) = match warning {
            Warning::Allocate => (0, 0),
            Warning::Read(error) => (1, error),
            Warning::Decode(error) => (2, error),
        };
        // SAFETY: image remains live even when frame allocation fails.
        unsafe { gs_image_decode_warn(self.info, kind, error) }
    }
    fn cleanup(&mut self) {
        // SAFETY: core cleans a successfully allocated session exactly once.
        unsafe { gs_image_decode_cleanup(self.session) }
        self.session = core::ptr::null_mut();
    }
}

/// Decode an initialized private image, returning libobs-allocated pixels.
///
/// # Safety
/// `info` must be an exclusively borrowed private `gs_image_decode_context`
/// for the duration of this call, with an initialized image, open FFmpeg contexts,
/// and the original size-check and reformat callbacks.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_ffmpeg_image_decode(
    info: *mut c_void,
    alpha_mode: c_int,
) -> *mut c_void {
    if info.is_null() {
        return core::ptr::null_mut();
    }
    image_decode::decode(&mut Adapter {
        info,
        session: core::ptr::null_mut(),
        alpha_mode,
    })
    .unwrap_or(core::ptr::null_mut())
}
