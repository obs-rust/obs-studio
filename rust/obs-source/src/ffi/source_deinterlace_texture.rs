//! Borrow private C fields; graphics allocation and destination stores are callbacks.
use crate::deinterlace_texture::{self, Allocator, Input, Plane};
use core::ffi::c_void;

pub type Create = unsafe extern "C" fn(u32, u32, i32, u32, *const *const u8, u32) -> *mut c_void;
pub type Texrender = unsafe extern "C" fn(i32, i32) -> *mut c_void;

#[repr(C)]
pub struct Fields {
    pub format: i32,
    pub trc: u8,
    pub gpu: bool,
    pub channels: i32,
    pub width: u32,
    pub height: u32,
    pub widths: [u32; 8],
    pub heights: [u32; 8],
    pub formats: [i32; 8],
    pub textures: *mut *mut c_void,
    pub render: *mut *mut c_void,
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
    let input = Input {
        format: fields.format,
        trc: fields.trc,
        gpu: fields.gpu,
        channels: fields.channels,
        width: fields.width,
        height: fields.height,
        planes: core::array::from_fn(|c| Plane {
            width: fields.widths[c],
            height: fields.heights[c],
            format: fields.formats[c],
        }),
    };
    deinterlace_texture::allocate(input, &mut Graphics(fields, create, render));
}
