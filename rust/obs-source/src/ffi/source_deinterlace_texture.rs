//! Borrow private C fields; graphics allocation and destination stores are callbacks.
use crate::deinterlace_texture::{self, Allocator, PLANES, Plane};
use core::ffi::c_void;

pub type Create = unsafe extern "C" fn(u32, u32, i32, u32, *const *const u8, u32) -> *mut c_void;
pub type Texrender = unsafe extern "C" fn(i32, i32) -> *mut c_void;

/// `struct obs_source_deinterlace_texture_fields`: the borrowed addresses of
/// the source's fields. The arrays hold `PLANES` (`MAX_AV_PLANES`) entries.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Fields {
    pub format: *const i32,
    pub trc: *const u8,
    pub gpu: *const bool,
    pub channels: *const i32,
    pub width: *const u32,
    pub height: *const u32,
    pub widths: *const u32,
    pub heights: *const u32,
    pub formats: *const i32,
    pub textures: *mut *mut c_void,
    pub render: *mut *mut c_void,
}

/// Each read goes to the live field when the core asks for it.
struct Source(Fields);
impl deinterlace_texture::Fields for Source {
    fn format(&self) -> i32 {
        // SAFETY: the adapter's address of the live source's field.
        unsafe { self.0.format.read() }
    }
    fn trc(&self) -> u8 {
        // SAFETY: as above.
        unsafe { self.0.trc.read() }
    }
    fn gpu(&self) -> bool {
        // SAFETY: as above.
        unsafe { self.0.gpu.read() }
    }
    fn channels(&self) -> i32 {
        // SAFETY: as above.
        unsafe { self.0.channels.read() }
    }
    fn plane(&self, index: usize) -> Plane {
        assert!(index < PLANES, "plane {index} past MAX_AV_PLANES");
        // SAFETY: the three arrays hold MAX_AV_PLANES entries, checked above.
        unsafe {
            Plane {
                width: self.0.widths.add(index).read(),
                height: self.0.heights.add(index).read(),
                format: self.0.formats.add(index).read(),
            }
        }
    }
    fn width(&self) -> u32 {
        // SAFETY: as above.
        unsafe { self.0.width.read() }
    }
    fn height(&self) -> u32 {
        // SAFETY: as above.
        unsafe { self.0.height.read() }
    }
}
unsafe extern "C" {
    fn obs_source_deinterlace_texture_fields(source: *mut c_void) -> Fields;
    fn gs_texture_create(
        width: u32,
        height: u32,
        format: i32,
        levels: u32,
        data: *const *const u8,
        flags: u32,
    ) -> *mut c_void;
    fn gs_texrender_create(format: i32, zs: i32) -> *mut c_void;
}

struct Graphics(Fields, Create, Texrender);
impl Allocator for Graphics {
    fn render(&mut self, format: i32, zs: i32) {
        // SAFETY: the live source supplies the destination; the bridge supplies the callback.
        unsafe {
            self.0.render.write((self.2)(format, zs));
        }
    }
    fn texture(&mut self, index: usize, plane: Plane, levels: u32, flags: u32) {
        // SAFETY: the core emits at most eight indices into the live source array.
        // Initial data is NULL, exactly as in the original C allocation call.
        unsafe {
            self.0.textures.add(index).write((self.1)(
                plane.width,
                plane.height,
                plane.format,
                levels,
                core::ptr::null(),
                flags,
            ));
        }
    }
}

/// # Safety
/// A non-null source must be live and exclusively accessible under the original
/// graphics/source synchronization; graphics allocation must be permitted.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn set_deinterlace_texture_size(source: *mut c_void) {
    // SAFETY: same live-source and graphics preconditions; callbacks are libobs's.
    unsafe {
        allocate_with(source, gs_texture_create, gs_texrender_create);
    }
}

/// The production bridge with injected graphics allocation entry points.
/// # Safety
/// Same requirements as `set_deinterlace_texture_size`; callbacks must implement
/// the graphics ABI and return pointers valid for storage in this source.
pub unsafe extern "C" fn allocate_with(source: *mut c_void, create: Create, render: Texrender) {
    if source.is_null() {
        return;
    }
    // SAFETY: NULL was checked before even forming addresses in the adapter.
    let fields = unsafe { obs_source_deinterlace_texture_fields(source) };
    deinterlace_texture::allocate(Source(fields), &mut Graphics(fields, create, render));
}
