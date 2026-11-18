//! Thin C boundary around an opaque, C-owned FFmpeg image session.
use crate::texture_file_data::{Backend, Metadata, create_texture_file_data};
use crate::texture_render::ColorFormat;
use core::ffi::{c_char, c_void};

unsafe extern "C" {
    fn obs_texture_file_data_open(file: *const c_char) -> *mut c_void;
    fn obs_texture_file_data_decode(session: *mut c_void) -> *mut u8;
    fn obs_texture_file_data_format(session: *mut c_void) -> i32;
    fn obs_texture_file_data_width(session: *mut c_void) -> u32;
    fn obs_texture_file_data_height(session: *mut c_void) -> u32;
    fn obs_texture_file_data_close(session: *mut c_void);
}
struct Session {
    file: *const c_char,
    image: *mut c_void,
    format: *mut ColorFormat,
    width: *mut u32,
    height: *mut u32,
}
impl Backend for Session {
    type Pixels = *mut u8;
    fn init(&mut self) -> bool {
        // SAFETY: file retains the public entry point's C path contract.
        self.image = unsafe { obs_texture_file_data_open(self.file) };
        !self.image.is_null()
    }
    fn decode_straight(&mut self) -> Option<Self::Pixels> {
        if self.image.is_null() {
            return None;
        }
        // SAFETY: checked live session; C supplies the original callback pair.
        let data = unsafe { obs_texture_file_data_decode(self.image) };
        (!data.is_null()).then_some(data)
    }
    fn metadata(&mut self) -> Metadata {
        assert!(!self.image.is_null());
        // SAFETY: the non-null session remains alive until free, after publication.
        unsafe {
            Metadata {
                format: obs_texture_file_data_format(self.image),
                width: obs_texture_file_data_width(self.image),
                height: obs_texture_file_data_height(self.image),
            }
        }
    }
    fn publish(&mut self, metadata: Metadata) {
        // SAFETY: successful loads require writable public output slots.
        unsafe {
            *self.format = ColorFormat(metadata.format as u32);
            *self.width = metadata.width;
            *self.height = metadata.height;
        }
    }
    fn free(&mut self) {
        if !self.image.is_null() {
            // SAFETY: close consumes this initialized session exactly once.
            unsafe { obs_texture_file_data_close(self.image) };
            self.image = core::ptr::null_mut();
        }
    }
}
/// Decode legacy straight-alpha image pixels.
///
/// # Safety
/// `file` follows ffmpeg_image_init's path contract. On success all outputs
/// must be writable. The caller frees the returned bmalloc allocation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_create_texture_file_data(
    file: *const c_char,
    format: *mut ColorFormat,
    width: *mut u32,
    height: *mut u32,
) -> *mut u8 {
    create_texture_file_data(&mut Session {
        file,
        image: core::ptr::null_mut(),
        format,
        width,
        height,
    })
    .unwrap_or(core::ptr::null_mut())
}
