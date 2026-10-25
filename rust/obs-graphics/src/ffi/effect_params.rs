//! C ABI for `libobs/graphics/effect-params.c.h`: the 17 exported
//! parameter value functions of `graphics/graphics.h`.
//!
//! `struct gs_effect_param` is the mirror in [`super::effect_lookups`]. The
//! value is the C `DARRAY(uint8_t)` `cur_val`, kept in place by
//! [`DarrayStore`], which grows as the C's `darray_resize` does (capacity
//! doubling, new bytes zeroed). The getters' copies and the darray buffers
//! are bmem memory, and the error lines go to `blog` with the C's literal
//! formats (`obs_graphics_effect_param_error`), so a log handler sees the
//! same format string; both through the C adapters in
//! `libobs/graphics/obs-graphics-adapter.c.h`, as in `image_file`, so this
//! module names no obs-util symbol.
//!
//! The functions do not check the parameter's type, as in C: the setters
//! store whatever bytes they are given.
//!
//! Intentional difference: `gs_effect_set_texture` and
//! `gs_effect_set_texture_srgb` store `struct gs_shader_texture` with its
//! padding bytes zero. The C builds it on the stack and copies and compares
//! the padding as it finds it, so setting the same texture again can count
//! as a change; here it never does.

use core::ffi::{c_int, c_void};
use core::{ptr, slice};

use obs_util::ffi::darray::darray;

use super::effect_lookups::gs_effect_param;
use crate::effect_params::{ByteStore, bgra_to_vec4, copy_out, set_val, takes_next_sampler};

/// The error lines, by the number `obs_graphics_effect_param_error` takes.
#[derive(Clone, Copy)]
enum ParamError {
    /// "effect_setval_inline: invalid param"
    SetInvalidParam = 0,
    /// "effect_setval_inline: invalid data"
    SetInvalidData = 1,
    /// "gs_effect_get_val: invalid param"
    GetValInvalidParam = 2,
    /// "gs_effect_get_default_val: invalid param"
    GetDefaultInvalidParam = 3,
    /// "gs_effect_set_next_sampler: invalid param"
    NextSamplerInvalidParam = 4,
}

unsafe extern "C" {
    /// `blog(LOG_ERROR, ...)` with the C's literal format.
    fn obs_graphics_effect_param_error(which: c_int);
    #[link_name = "obs_graphics_bmalloc"]
    fn bmalloc(size: usize) -> *mut c_void;
    #[link_name = "obs_graphics_bfree"]
    fn bfree(ptr: *mut c_void);
}

fn log_error(error: ParamError) {
    // SAFETY: a plain value.
    unsafe { obs_graphics_effect_param_error(error as c_int) }
}

/// `struct gs_shader_texture` (`graphics/graphics.h`).
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct gs_shader_texture {
    pub tex: *mut c_void,
    pub srgb: bool,
}

/// A parameter's `DARRAY(uint8_t)`, kept in place.
pub struct DarrayStore {
    da: *mut darray,
}

impl DarrayStore {
    /// # Safety
    ///
    /// `da` is a valid `DARRAY(uint8_t)` with bmem memory, not accessed
    /// elsewhere while the store lives.
    pub unsafe fn new(da: *mut darray) -> Self {
        Self { da }
    }

    /// `darray_ensure_capacity` for 1-byte items.
    fn ensure_capacity(&mut self, new_size: usize) {
        // SAFETY: a valid darray (the constructor's contract).
        let da = unsafe { &mut *self.da };
        if new_size <= da.capacity {
            return;
        }
        let mut new_cap = if da.capacity == 0 {
            new_size
        } else {
            da.capacity.wrapping_mul(2)
        };
        if new_size > new_cap {
            new_cap = new_size;
        }
        // SAFETY: the libobs allocator; the old buffer holds `capacity`
        // bytes and the new one `new_cap` >= `capacity`.
        unsafe {
            let p = bmalloc(new_cap).cast::<u8>();
            if !da.array.is_null() {
                if da.capacity != 0 {
                    ptr::copy_nonoverlapping(da.array.cast::<u8>(), p, da.capacity);
                }
                bfree(da.array);
            }
            da.array = p.cast();
        }
        da.capacity = new_cap;
    }
}

impl ByteStore for DarrayStore {
    fn bytes(&self) -> &[u8] {
        // SAFETY: a valid darray: `num` bytes at `array` (or none).
        unsafe {
            let da = &*self.da;
            if da.num == 0 || da.array.is_null() {
                &[]
            } else {
                slice::from_raw_parts(da.array.cast::<u8>(), da.num)
            }
        }
    }

    fn bytes_mut(&mut self) -> &mut [u8] {
        // SAFETY: as bytes(), with the store's exclusive access.
        unsafe {
            let da = &mut *self.da;
            if da.num == 0 || da.array.is_null() {
                &mut []
            } else {
                slice::from_raw_parts_mut(da.array.cast::<u8>(), da.num)
            }
        }
    }

    /// `darray_resize` for 1-byte items.
    fn resize(&mut self, size: usize) {
        // SAFETY: a valid darray.
        let old_num = unsafe { (*self.da).num };
        if size == old_num {
            return;
        }
        if size == 0 {
            // SAFETY: as above.
            unsafe { (*self.da).num = 0 };
            return;
        }
        self.ensure_capacity(size);
        // SAFETY: capacity now covers `size` bytes.
        unsafe {
            (*self.da).num = size;
            if size > old_num {
                ptr::write_bytes(
                    (*self.da).array.cast::<u8>().add(old_num),
                    0,
                    size - old_num,
                );
            }
        }
    }
}

/// `effect_setval_inline`. Inlined into each setter, as the C's static
/// inline is, so the comparison of a constant-size value is inlined too.
///
/// # Safety
///
/// `param` is null or a valid `struct gs_effect_param`; `data` is null or
/// `size` readable bytes that do not overlap its `cur_val`.
#[inline(always)]
unsafe fn setval(param: *mut gs_effect_param, data: *const c_void, size: usize) {
    if param.is_null() {
        log_error(ParamError::SetInvalidParam);
        return;
    }
    if data.is_null() {
        log_error(ParamError::SetInvalidData);
        return;
    }
    // SAFETY: the contract: a valid parameter and `size` bytes of data.
    unsafe {
        let param = &mut *param;
        let data = slice::from_raw_parts(data.cast::<u8>(), size);
        let mut cur = DarrayStore::new(&mut param.cur_val);
        set_val(&mut cur, &mut param.changed, data);
    }
}

/// # Safety
///
/// `param` is null or a valid `struct gs_effect_param`.
#[inline(always)]
unsafe fn set_bytes(param: *mut gs_effect_param, bytes: &[u8]) {
    // SAFETY: the caller's contract; `bytes` is a local buffer.
    unsafe { setval(param, bytes.as_ptr().cast(), bytes.len()) }
}

/// `gs_effect_set_bool`: an `int`.
///
/// # Safety
///
/// `param` is null or a valid `struct gs_effect_param`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_set_bool(param: *mut gs_effect_param, val: bool) {
    // SAFETY: the caller's contract.
    unsafe { set_bytes(param, &c_int::from(val).to_ne_bytes()) }
}

/// `gs_effect_set_float`.
///
/// # Safety
///
/// As [`gs_effect_set_bool`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_set_float(param: *mut gs_effect_param, val: f32) {
    // SAFETY: the caller's contract.
    unsafe { set_bytes(param, &val.to_ne_bytes()) }
}

/// `gs_effect_set_int`.
///
/// # Safety
///
/// As [`gs_effect_set_bool`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_set_int(param: *mut gs_effect_param, val: c_int) {
    // SAFETY: the caller's contract.
    unsafe { set_bytes(param, &val.to_ne_bytes()) }
}

/// The C's `struct vec2`, `vec3`, `vec4` and `matrix4` sizes.
const VEC2_SIZE: usize = 8;
const VEC3_STORED: usize = 12;
const VEC4_SIZE: usize = 16;
const MATRIX4_SIZE: usize = 64;

/// `gs_effect_set_matrix4`: the whole `struct matrix4`.
///
/// # Safety
///
/// As [`gs_effect_set_bool`], and `val` is null or a valid `struct matrix4`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_set_matrix4(param: *mut gs_effect_param, val: *const c_void) {
    // SAFETY: the caller's contract.
    unsafe { setval(param, val, MATRIX4_SIZE) }
}

/// `gs_effect_set_vec2`: the whole `struct vec2`.
///
/// # Safety
///
/// As [`gs_effect_set_bool`], and `val` is null or a valid `struct vec2`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_set_vec2(param: *mut gs_effect_param, val: *const c_void) {
    // SAFETY: the caller's contract.
    unsafe { setval(param, val, VEC2_SIZE) }
}

/// `gs_effect_set_vec3`: 12 bytes (`sizeof(float) * 3`), not the 16 of
/// `struct vec3`.
///
/// # Safety
///
/// As [`gs_effect_set_bool`], and `val` is null or a valid `struct vec3`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_set_vec3(param: *mut gs_effect_param, val: *const c_void) {
    // SAFETY: the caller's contract.
    unsafe { setval(param, val, VEC3_STORED) }
}

/// `gs_effect_set_vec4`: the whole `struct vec4`.
///
/// # Safety
///
/// As [`gs_effect_set_bool`], and `val` is null or a valid `struct vec4`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_set_vec4(param: *mut gs_effect_param, val: *const c_void) {
    // SAFETY: the caller's contract.
    unsafe { setval(param, val, VEC4_SIZE) }
}

/// `gs_effect_set_color`: `vec4_from_bgra`.
///
/// # Safety
///
/// As [`gs_effect_set_bool`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_set_color(param: *mut gs_effect_param, argb: u32) {
    let mut bytes = [0u8; VEC4_SIZE];
    for (chunk, f) in bytes
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(bgra_to_vec4(argb))
    {
        chunk.copy_from_slice(&f.to_ne_bytes());
    }
    // SAFETY: the caller's contract.
    unsafe { set_bytes(param, &bytes) }
}

/// `struct gs_shader_texture` as bytes, its padding zero.
fn shader_texture(tex: *mut c_void, srgb: bool) -> [u8; size_of::<gs_shader_texture>()] {
    let mut bytes = [0u8; size_of::<gs_shader_texture>()];
    let st = gs_shader_texture { tex, srgb };
    // SAFETY: two plain fields written into a zeroed buffer of the struct's
    // size at their offsets; the padding stays zero.
    unsafe {
        let base = bytes.as_mut_ptr();
        ptr::write_unaligned(
            base.add(core::mem::offset_of!(gs_shader_texture, tex))
                .cast::<*mut c_void>(),
            st.tex,
        );
        ptr::write_unaligned(
            base.add(core::mem::offset_of!(gs_shader_texture, srgb))
                .cast::<bool>(),
            st.srgb,
        );
    }
    bytes
}

/// `gs_effect_set_texture`.
///
/// # Safety
///
/// As [`gs_effect_set_bool`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_set_texture(param: *mut gs_effect_param, val: *mut c_void) {
    // SAFETY: the caller's contract.
    unsafe { set_bytes(param, &shader_texture(val, false)) }
}

/// `gs_effect_set_texture_srgb`.
///
/// # Safety
///
/// As [`gs_effect_set_bool`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_set_texture_srgb(param: *mut gs_effect_param, val: *mut c_void) {
    // SAFETY: the caller's contract.
    unsafe { set_bytes(param, &shader_texture(val, true)) }
}

/// `gs_effect_set_val`.
///
/// # Safety
///
/// As [`gs_effect_set_bool`], and `val` is null or `size` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_set_val(
    param: *mut gs_effect_param,
    val: *const c_void,
    size: usize,
) {
    // SAFETY: the caller's contract.
    unsafe { setval(param, val, size) }
}

/// A bmem copy of `da`'s bytes, or null for none.
///
/// # Safety
///
/// `da` is a valid `DARRAY(uint8_t)`.
unsafe fn copy_of(da: *mut darray) -> *mut c_void {
    // SAFETY: the caller's contract.
    let store = unsafe { DarrayStore::new(da) };
    let src = store.bytes();
    let size = src.len();
    if size == 0 {
        return ptr::null_mut();
    }
    // SAFETY: the libobs allocator; bzalloc is bmalloc plus zeroing, and
    // every byte is written below.
    unsafe {
        let data = bmalloc(size);
        ptr::write_bytes(data.cast::<u8>(), 0, size);
        copy_out(src, slice::from_raw_parts_mut(data.cast::<u8>(), size));
        data
    }
}

/// `gs_effect_get_val`: a bmem copy of the value, freed with `bfree`.
///
/// # Safety
///
/// As [`gs_effect_set_bool`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_get_val(param: *mut gs_effect_param) -> *mut c_void {
    if param.is_null() {
        log_error(ParamError::GetValInvalidParam);
        return ptr::null_mut();
    }
    // SAFETY: a valid parameter.
    unsafe { copy_of(&mut (*param).cur_val) }
}

/// `gs_effect_get_val_size`.
///
/// # Safety
///
/// As [`gs_effect_set_bool`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_get_val_size(param: *mut gs_effect_param) -> usize {
    // SAFETY: null or a valid parameter.
    unsafe { param.as_ref() }.map_or(0, |p| p.cur_val.num)
}

/// `gs_effect_get_default_val`: a bmem copy of the default, freed with
/// `bfree`.
///
/// # Safety
///
/// As [`gs_effect_set_bool`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_get_default_val(param: *mut gs_effect_param) -> *mut c_void {
    if param.is_null() {
        log_error(ParamError::GetDefaultInvalidParam);
        return ptr::null_mut();
    }
    // SAFETY: a valid parameter.
    unsafe { copy_of(&mut (*param).default_val) }
}

/// `gs_effect_get_default_val_size`.
///
/// # Safety
///
/// As [`gs_effect_set_bool`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_get_default_val_size(param: *mut gs_effect_param) -> usize {
    // SAFETY: null or a valid parameter.
    unsafe { param.as_ref() }.map_or(0, |p| p.default_val.num)
}

/// `gs_effect_set_default`: the default bytes back. Without a default the
/// NULL array goes on, as in C, so it logs "invalid data".
///
/// # Safety
///
/// As [`gs_effect_set_bool`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_set_default(param: *mut gs_effect_param) {
    if param.is_null() {
        // The error the other setters log through effect_setval_inline().
        log_error(ParamError::SetInvalidParam);
        return;
    }
    // SAFETY: a valid parameter; its default and current value are two
    // separate darrays.
    unsafe {
        let (array, num) = ((*param).default_val.array, (*param).default_val.num);
        setval(param, array, num);
    }
}

/// `gs_effect_set_next_sampler`: only on a texture parameter.
///
/// # Safety
///
/// As [`gs_effect_set_bool`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_set_next_sampler(
    param: *mut gs_effect_param,
    sampler: *mut c_void,
) {
    // SAFETY: null or a valid parameter.
    let Some(param) = (unsafe { param.as_mut() }) else {
        log_error(ParamError::NextSamplerInvalidParam);
        return;
    };
    if takes_next_sampler(param.type_) {
        param.next_sampler = sampler;
    }
}
