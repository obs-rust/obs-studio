//! `libobs/graphics/graphics-state-stacks.c.h` shims against the C oracle.
//! The fixture is what a scene renders per item: push the matrix, translate,
//! rotate about z, scale, read the top (the device's view matrix), pop; a
//! thousand items, with the blend state pushed and popped around them.

use core::ffi::c_void;
use core::hint::black_box;
use core::ptr;

use criterion::Criterion;
use obs_c_oracle::graphics_state_stacks as c;
use obs_graphics::ffi::axisang::axisang;
use obs_graphics::ffi::matrix4::matrix4;
use obs_graphics::ffi::state_stacks::{
    gs_blend_state_stack_pop, gs_blend_state_stack_push, gs_matrix_stack_get, gs_matrix_stack_pop,
    gs_matrix_stack_push, gs_matrix_stack_rotaa, gs_matrix_stack_scale, gs_matrix_stack_translate,
};
use obs_graphics::ffi::vec3::vec3;
use obs_graphics::state_stacks::{BlendState, identity};
use obs_util::ffi::darray::{darray, darray_free, darray_push_back};

use crate::pair;

const ITEMS: usize = 1000;

fn matrices() -> darray {
    let mut da = darray {
        array: ptr::null_mut(),
        num: 0,
        capacity: 0,
    };
    let id: matrix4 = identity().into();
    // SAFETY: an empty darray of matrix4.
    unsafe { darray_push_back(size_of::<matrix4>(), &mut da, ptr::from_ref(&id).cast()) };
    da
}

pub fn benchmarks(criterion: &mut Criterion) {
    let pos = vec3 {
        x: 120.0,
        y: 64.0,
        z: 0.0,
        w: 0.0,
    };
    let rot = axisang {
        x: 0.0,
        y: 0.0,
        z: 1.0,
        w: 0.3,
    };
    let scale = vec3 {
        x: 0.5,
        y: 0.5,
        z: 1.0,
        w: 0.0,
    };
    let state = BlendState {
        enabled: true,
        src_c: 4,
        dest_c: 5,
        src_a: 1,
        dest_a: 5,
        op: 0,
    };
    let (mut rm, mut cm) = (matrices(), matrices());
    let (mut rb, mut cb) = (
        darray {
            array: ptr::null_mut(),
            num: 0,
            capacity: 0,
        },
        darray {
            array: ptr::null_mut(),
            num: 0,
            capacity: 0,
        },
    );
    let (mut rcur, mut ccur) = (0usize, 0usize);
    pair(
        criterion,
        "state_stacks/scene_1000_items",
        || {
            let mut out: matrix4 = identity().into();
            let mut sum = 0.0f32;
            // SAFETY: live darrays and values.
            unsafe {
                gs_blend_state_stack_push(&mut rb, &state);
                for _ in 0..ITEMS {
                    gs_matrix_stack_push(&mut rm, &mut rcur);
                    gs_matrix_stack_translate(&mut rm, rcur, &pos);
                    gs_matrix_stack_rotaa(&mut rm, rcur, &rot);
                    gs_matrix_stack_scale(&mut rm, rcur, &scale);
                    gs_matrix_stack_get(&rm, rcur, &mut out);
                    sum += out.t.x;
                    gs_matrix_stack_pop(&mut rm, &mut rcur);
                }
                let mut back = state;
                gs_blend_state_stack_pop(&mut rb, &mut back);
            }
            black_box(sum).to_bits()
        },
        || {
            let mut out: matrix4 = identity().into();
            let mut sum = 0.0f32;
            // SAFETY: as above, through the C signatures.
            unsafe {
                let (m, b) = (
                    (&raw mut cm).cast::<c_void>(),
                    (&raw mut cb).cast::<c_void>(),
                );
                c::oracle_gs_blend_state_stack_push(b, (&raw const state).cast());
                for _ in 0..ITEMS {
                    c::oracle_gs_matrix_stack_push(m, &mut ccur);
                    c::oracle_gs_matrix_stack_translate(m, ccur, (&raw const pos).cast());
                    c::oracle_gs_matrix_stack_rotaa(m, ccur, (&raw const rot).cast());
                    c::oracle_gs_matrix_stack_scale(m, ccur, (&raw const scale).cast());
                    c::oracle_gs_matrix_stack_get(m, ccur, (&raw mut out).cast());
                    sum += out.t.x;
                    c::oracle_gs_matrix_stack_pop(m, &mut ccur);
                }
                let mut back = state;
                c::oracle_gs_blend_state_stack_pop(b, (&raw mut back).cast());
            }
            black_box(sum).to_bits()
        },
    );
    // SAFETY: darrays from darray_push_back.
    unsafe {
        darray_free(&mut rm);
        darray_free(&mut cm);
        darray_free(&mut rb);
        darray_free(&mut cb);
    }
}
