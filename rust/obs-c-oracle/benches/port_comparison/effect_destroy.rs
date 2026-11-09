//! obs-graphics: the effect destroy (`libobs/graphics/effect.c.h`). Each
//! side builds the same typical effect on bmalloc (six params, two with
//! annotations, and two techniques with three passes between them, as
//! libobs's default effects have) and destroys it with its
//! `gs_effect_destroy`: the Rust shim through libobs's allocator adapter,
//! the original C through the oracle's recorder with recording off, so it
//! only frees. Both must leave the allocation count where it was. The
//! build is the same on both sides and part of each iteration.

use super::pair;

use core::ffi::{c_char, c_long, c_void};
use core::ptr;
use criterion::Criterion;
use obs_c_oracle::effect_destroy::{oracle_destroy_set_recording, oracle_gs_effect_destroy};
use obs_graphics::ffi::effect_destroy::gs_effect_destroy;
use obs_graphics::ffi::effect_lookups::{
    gs_effect, gs_effect_param, gs_effect_pass, gs_effect_technique,
};
use obs_util::ffi::bmem::bnum_allocs;
use obs_util::ffi::darray::{bmalloc, darray};
use std::hint::black_box;

fn zeroed(size: usize) -> *mut c_void {
    // SAFETY: a plain allocation of at least one byte, then zeroed.
    unsafe {
        let block = bmalloc(size.max(1));
        ptr::write_bytes(block.cast::<u8>(), 0, size.max(1));
        block
    }
}

fn array<T>(num: usize) -> darray {
    darray {
        array: zeroed(size_of::<T>() * num),
        num,
        capacity: num,
    }
}

fn name() -> *mut c_char {
    zeroed(8).cast()
}

/// A param with a name, a 16-byte value and default, and `annotations`
/// annotations of its own.
///
/// # Safety
/// `at` is a free slot of a param array.
unsafe fn param(at: *mut gs_effect_param, annotations: usize) {
    let list = if annotations == 0 {
        darray {
            array: ptr::null_mut(),
            num: 0,
            capacity: 0,
        }
    } else {
        array::<gs_effect_param>(annotations)
    };
    let list_array = list.array.cast::<gs_effect_param>();
    let value = gs_effect_param {
        name: name(),
        section: 0,
        type_: 0,
        changed: false,
        cur_val: array::<u8>(16),
        default_val: array::<u8>(16),
        effect: ptr::null_mut(),
        next_sampler: ptr::null_mut(),
        annotations: list,
    };
    // SAFETY: the caller's slot.
    unsafe { at.write(value) };
    for i in 0..annotations {
        // SAFETY: `list` holds `annotations` slots.
        unsafe { param(list_array.add(i), 0) };
    }
}

/// The typical effect, uncached.
fn build() -> *mut gs_effect {
    const ANNOTATIONS: [usize; 6] = [0, 2, 1, 0, 0, 0];
    const PASSES: [usize; 2] = [2, 1];
    let params = array::<gs_effect_param>(ANNOTATIONS.len());
    let techniques = array::<gs_effect_technique>(PASSES.len());
    let (params_array, techniques_array) = (
        params.array.cast::<gs_effect_param>(),
        techniques.array.cast::<gs_effect_technique>(),
    );
    let effect = zeroed(size_of::<gs_effect>()).cast::<gs_effect>();
    // SAFETY: `effect` is a zeroed block of the struct's size; every
    // pointer it holds is null or one of the blocks built here.
    unsafe {
        (*effect).effect_path = ptr::null_mut();
        (*effect).effect_dir = ptr::null_mut();
        (*effect).params = params;
        (*effect).techniques = techniques;
        for (i, &annotations) in ANNOTATIONS.iter().enumerate() {
            param(params_array.add(i), annotations);
        }
        for (i, &passes) in PASSES.iter().enumerate() {
            let list = array::<gs_effect_pass>(passes);
            let list_array = list.array.cast::<gs_effect_pass>();
            techniques_array.add(i).write(gs_effect_technique {
                name: name(),
                section: 0,
                effect,
                passes: list,
            });
            for k in 0..passes {
                list_array.add(k).write(gs_effect_pass {
                    name: name(),
                    section: 0,
                    vertshader: ptr::null_mut(),
                    pixelshader: ptr::null_mut(),
                    vertshader_params: array::<[usize; 2]>(1),
                    pixelshader_params: array::<[usize; 2]>(3),
                });
            }
        }
    }
    effect
}

fn rust_destroy() -> c_long {
    // SAFETY: a plain read of the counter.
    let before = unsafe { bnum_allocs() };
    // SAFETY: a valid uncached effect, destroyed once; its shaders are
    // NULL.
    unsafe { gs_effect_destroy(black_box(build())) };
    // SAFETY: as above.
    let after = unsafe { bnum_allocs() };
    after - before
}

fn c_destroy() -> c_long {
    // SAFETY: a plain read of the counter.
    let before = unsafe { bnum_allocs() };
    // SAFETY: as for rust_destroy().
    unsafe { oracle_gs_effect_destroy(black_box(build()).cast()) };
    // SAFETY: as above.
    let after = unsafe { bnum_allocs() };
    after - before
}

pub fn benchmarks(criterion: &mut Criterion) {
    // SAFETY: the bench is the recorder's only user in this process.
    unsafe { oracle_destroy_set_recording(false) };
    pair(criterion, "effect_destroy/destroy/6_params", rust_destroy, c_destroy);
    // SAFETY: as above.
    unsafe { oracle_destroy_set_recording(true) };
}
