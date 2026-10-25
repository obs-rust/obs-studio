//! #2409: `libobs/graphics/effect-params.c.h` shims against the C oracle.
//! The fixtures are the setters every filter and scene item runs on every
//! frame: a float, a vec4, a matrix4 and a texture, each with the value
//! already stored (the common case: nothing changed) and with a new value
//! (stored, changed set). `changed` is cleared after each call on both
//! sides.

use std::ffi::c_void;
use std::hint::black_box;
use std::ptr;

use criterion::Criterion;
use obs_c_oracle::effect_params as c;
use obs_graphics::ffi::effect_lookups::gs_effect_param;
use obs_graphics::ffi::effect_params as r;
use obs_util::ffi::darray::darray;

use crate::measure_pair;

/// A parameter whose value buffer is freed when it goes.
struct Param(Box<gs_effect_param>);

impl Drop for Param {
    fn drop(&mut self) {
        // SAFETY: the bmem buffer of this parameter's value (or null).
        unsafe { obs_util::ffi::bmem::bfree(self.0.cur_val.array) };
    }
}

fn param() -> Param {
    let empty = || darray {
        array: ptr::null_mut(),
        num: 0,
        capacity: 0,
    };
    Param(Box::new(gs_effect_param {
        name: ptr::null_mut(),
        section: 0,
        type_: 7,
        changed: false,
        cur_val: empty(),
        default_val: empty(),
        effect: ptr::null_mut(),
        next_sampler: ptr::null_mut(),
        annotations: empty(),
    }))
}

/// Times `set` on a parameter of each side. With `alternate`, the value
/// switches between two on each call, so every call stores and marks a
/// change.
fn setter(
    criterion: &mut Criterion,
    name: &str,
    rust: unsafe fn(*mut gs_effect_param, bool),
    original: unsafe fn(*mut c_void, bool),
    alternate: bool,
) {
    let mut rp = param();
    let mut cp = param();
    let mut flip = false;
    let mut flip_c = false;
    measure_pair(
        criterion,
        name,
        || {
            flip ^= alternate;
            // SAFETY: a valid parameter.
            unsafe { rust(&mut *rp.0, flip) };
            rp.0.changed = false;
            black_box(rp.0.cur_val.num)
        },
        || {
            flip_c ^= alternate;
            // SAFETY: a valid parameter, through the C signature.
            unsafe { original(ptr::from_mut(&mut *cp.0).cast(), flip_c) };
            cp.0.changed = false;
            black_box(cp.0.cur_val.num)
        },
    );
}

const M4: [f32; 16] = [1.0; 16];
const M4B: [f32; 16] = [2.0; 16];
const V4: [f32; 4] = [1.0, 2.0, 3.0, 4.0];
const V4B: [f32; 4] = [5.0, 6.0, 7.0, 8.0];

unsafe fn r_float(p: *mut gs_effect_param, b: bool) {
    unsafe { r::gs_effect_set_float(p, if b { 2.0 } else { 1.0 }) }
}
unsafe fn c_float(p: *mut c_void, b: bool) {
    unsafe { c::oracle_gs_effect_set_float(p, if b { 2.0 } else { 1.0 }) }
}
unsafe fn r_vec4(p: *mut gs_effect_param, b: bool) {
    unsafe { r::gs_effect_set_vec4(p, (if b { &V4B } else { &V4 }).as_ptr().cast()) }
}
unsafe fn c_vec4(p: *mut c_void, b: bool) {
    unsafe { c::oracle_gs_effect_set_vec4(p, (if b { &V4B } else { &V4 }).as_ptr().cast()) }
}
unsafe fn r_matrix4(p: *mut gs_effect_param, b: bool) {
    unsafe { r::gs_effect_set_matrix4(p, (if b { &M4B } else { &M4 }).as_ptr().cast()) }
}
unsafe fn c_matrix4(p: *mut c_void, b: bool) {
    unsafe { c::oracle_gs_effect_set_matrix4(p, (if b { &M4B } else { &M4 }).as_ptr().cast()) }
}
unsafe fn r_texture(p: *mut gs_effect_param, b: bool) {
    unsafe { r::gs_effect_set_texture(p, (if b { 0x2000 } else { 0x1000 }) as *mut c_void) }
}
unsafe fn c_texture(p: *mut c_void, b: bool) {
    unsafe { c::oracle_gs_effect_set_texture(p, (if b { 0x2000 } else { 0x1000 }) as *mut c_void) }
}

pub fn benchmarks(criterion: &mut Criterion) {
    setter(
        criterion,
        "effect_params/set_float_same",
        r_float,
        c_float,
        false,
    );
    setter(
        criterion,
        "effect_params/set_float_new",
        r_float,
        c_float,
        true,
    );
    setter(
        criterion,
        "effect_params/set_vec4_same",
        r_vec4,
        c_vec4,
        false,
    );
    setter(
        criterion,
        "effect_params/set_vec4_new",
        r_vec4,
        c_vec4,
        true,
    );
    setter(
        criterion,
        "effect_params/set_matrix4_same",
        r_matrix4,
        c_matrix4,
        false,
    );
    setter(
        criterion,
        "effect_params/set_matrix4_new",
        r_matrix4,
        c_matrix4,
        true,
    );
    setter(
        criterion,
        "effect_params/set_texture_same",
        r_texture,
        c_texture,
        false,
    );
    setter(
        criterion,
        "effect_params/set_texture_new",
        r_texture,
        c_texture,
        true,
    );
}
