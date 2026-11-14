//! Private C view adapter. Storage stays allocated with libobs bmalloc/bfree.
use crate::encoder_audio_buffers::{self as core, Host};
use obs_util::ffi::{
    darray::{bfree, bmalloc},
    deque::{deque, deque_free},
};
use std::ffi::c_void;

/// Borrowed pointers into the real private C encoder, valid for one call.
#[repr(C)]
pub struct View {
    pub input: *mut deque,
    pub output: *mut *mut u8,
    pub rate: *mut u32,
    pub planes: *mut usize,
    pub blocksize: *mut usize,
    pub frames: *mut usize,
    pub bytes: *mut usize,
    pub frame_size: Option<unsafe extern "C" fn(*mut c_void) -> usize>,
    pub context: *mut c_void,
}
struct Borrowed<'a>(&'a mut View);
impl Host for Borrowed<'_> {
    fn free_plane(&mut self, plane: usize) {
        // SAFETY: the C caller lends MAX_AV_PLANES initialized slots exclusively.
        unsafe {
            deque_free(self.0.input.add(plane));
            let output = self.0.output.add(plane);
            bfree((*output).cast());
            *output = std::ptr::null_mut();
        }
    }
    fn allocate_plane(&mut self, plane: usize, bytes: usize) {
        // SAFETY: active planes fit the borrowed array; bmalloc owns this memory.
        unsafe {
            *self.0.output.add(plane) = bmalloc(bytes).cast();
        }
    }
    fn planes(&self) -> usize {
        // SAFETY: the planes pointer borrows the live encoder field.
        unsafe { *self.0.planes }
    }
    fn bytes(&self) -> usize {
        // SAFETY: the bytes pointer borrows the live encoder field.
        unsafe { *self.0.bytes }
    }
    fn set_format(&mut self, rate: u32, planes: usize, blocksize: usize) {
        // SAFETY: these distinct fields are writable for this exclusive call.
        unsafe {
            *self.0.rate = rate;
            *self.0.planes = planes;
            *self.0.blocksize = blocksize;
        }
    }
    fn frame_size(&mut self) -> usize {
        // SAFETY: C supplies the encoder's valid callback and original context.
        unsafe { (self.0.frame_size.unwrap_unchecked())(self.0.context) }
    }
    fn blocksize(&self) -> usize {
        // SAFETY: the blocksize pointer borrows the live encoder field.
        unsafe { *self.0.blocksize }
    }
    fn set_frame_size(&mut self, frames: usize, bytes: usize) {
        // SAFETY: both pointers borrow distinct writable encoder fields.
        unsafe {
            *self.0.frames = frames;
            *self.0.bytes = bytes;
        }
    }
}
/// Release all audio slots.
/// # Safety
/// `view` and its fields must exclusively borrow a live encoder with eight slots.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_audio_buffers_free(view: *mut View) {
    // SAFETY: caller guarantees a valid exclusive view.
    core::free(&mut Borrowed(unsafe { &mut *view }));
}
/// Reset output storage for the active planes.
/// # Safety
/// Same as free; active planes must not exceed eight.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_audio_buffers_reset(view: *mut View) {
    // SAFETY: caller guarantees a valid exclusive view.
    core::reset(&mut Borrowed(unsafe { &mut *view }));
}
/// Initialize from negotiated audio information.
/// # Safety
/// Same as free; the frame-size callback and context must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_audio_buffers_initialize(
    view: *mut View,
    rate: u32,
    format: i32,
    speakers: i32,
) {
    // SAFETY: caller guarantees a valid exclusive view.
    core::initialize(&mut Borrowed(unsafe { &mut *view }), rate, format, speakers);
}
