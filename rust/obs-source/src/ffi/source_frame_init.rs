//! Public source-frame ABI. Never read unrelated fields or rewrite padding.
use crate::frame_init::{Destination, Initialized, Planes, init};
use core::ffi::{CStr, c_char, c_int, c_long};
use core::mem::MaybeUninit;
use core::ptr::{NonNull, addr_of_mut};
use obs_media_io::ffi::video_frame::{video_frame, video_frame_init};

/// The layout in `obs.h`. Boolean storage is represented by bytes because
/// initialization accepts writable storage with unrelated fields uninitialized.
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct obs_source_frame {
    pub data: [*mut u8; 8],
    pub linesize: [u32; 8],
    pub width: u32,
    pub height: u32,
    pub timestamp: u64,
    pub format: c_int,
    pub color_matrix: [f32; 16],
    pub full_range: u8,
    pub max_luminance: u16,
    pub color_range_min: [f32; 3],
    pub color_range_max: [f32; 3],
    pub flip: u8,
    pub flags: u8,
    pub trc: u8,
    pub refs: c_long,
    pub prev_frame: u8,
}

struct Frame(NonNull<obs_source_frame>);

impl Destination for Frame {
    type Plane = *mut u8;
    fn store(&mut self, value: Initialized<Self::Plane>) {
        let frame = self.0.as_ptr();
        // SAFETY: the caller provides writable source-frame storage. Raw field
        // writes do not read any other field or overwrite its padding bytes.
        unsafe {
            addr_of_mut!((*frame).format).write(value.format);
            addr_of_mut!((*frame).width).write(value.width);
            addr_of_mut!((*frame).height).write(value.height);
            addr_of_mut!((*frame).data).write(value.planes.data);
            addr_of_mut!((*frame).linesize).write(value.planes.linesize);
        }
    }
}

unsafe extern "C" {
    fn blog(level: c_int, format: *const c_char, ...);
}

fn log(level: i32, message: &'static CStr) {
    // SAFETY: static terminated strings; the only variadic argument is a string.
    unsafe { blog(level, c"%s".as_ptr(), message.as_ptr()) }
}

/// Allocate video planes and initialize only the original C function's fields.
/// The caller frees the allocation with `obs_source_frame_free`/`bfree`.
///
/// # Safety
/// `frame` must be NULL or aligned, writable storage for `struct obs_source_frame`.
/// As in C, unsupported formats or empty allocations hit `bmalloc(0)`'s fatal
/// boundary. Existing plane allocations are not freed by initialization.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_frame_init(
    frame: *mut obs_source_frame,
    format: c_int,
    width: u32,
    height: u32,
) {
    let mut frame = NonNull::new(frame).map(Frame);
    init(
        frame.as_mut(),
        format,
        width,
        height,
        |format, width, height| {
            let mut video = MaybeUninit::<video_frame>::uninit();
            // SAFETY: video_frame_init fully initializes its writable destination.
            // Invalid/empty allocations retain the existing allocator failure.
            let video = unsafe {
                video_frame_init(video.as_mut_ptr(), format, width, height);
                video.assume_init()
            };
            Planes {
                data: video.data,
                linesize: video.linesize,
            }
        },
        log,
    );
}
