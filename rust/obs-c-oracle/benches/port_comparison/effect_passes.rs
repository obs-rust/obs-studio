//! `libobs/graphics/effect-passes.c.h` shims against the C oracle. The
//! fixture is one draw: `while (gs_effect_loop(effect, "Draw"))` over a
//! technique with one or two passes and 4 float parameters (two per shader
//! stage), each with a value. The graphics functions are the stand-in of
//! oracle/effect_passes_device.c with recording off, so they cost a call
//! and nothing else, on both sides.

use std::ffi::{CString, c_void};
use std::ptr;

use criterion::Criterion;
use obs_c_oracle::effect_passes as c;
use obs_graphics::effect_params::ByteStore;
use obs_graphics::ffi::effect_lookups::{
    gs_effect, gs_effect_param, gs_effect_pass, gs_effect_technique,
};
use obs_graphics::ffi::effect_params::DarrayStore;
use obs_graphics::ffi::effect_passes::{gs_effect_loop, pass_shaderparam};
use obs_util::ffi::darray::darray;

use crate::measure_pair;

fn empty() -> darray {
    darray {
        array: ptr::null_mut(),
        num: 0,
        capacity: 0,
    }
}

fn view<T>(v: &mut Vec<T>, n: usize) -> darray {
    darray {
        array: v.as_mut_ptr().cast(),
        num: n,
        capacity: n,
    }
}

/// One effect with its arrays; the value buffers are bmem, freed on drop.
struct Fixture {
    effect: Box<gs_effect>,
    params: Vec<gs_effect_param>,
    sparams: Vec<Vec<pass_shaderparam>>,
    passes: Vec<gs_effect_pass>,
    technique: Vec<gs_effect_technique>,
    names: Vec<CString>,
}

impl Fixture {
    fn new(passes: usize) -> Box<Self> {
        let mut f = Box::new(Fixture {
            effect: Box::new(gs_effect {
                processing: false,
                cached: false,
                effect_path: ptr::null_mut(),
                effect_dir: ptr::null_mut(),
                params: empty(),
                techniques: empty(),
                cur_technique: ptr::null_mut(),
                cur_pass: ptr::null_mut(),
                view_proj: ptr::null_mut(),
                world: ptr::null_mut(),
                scale: ptr::null_mut(),
                // SAFETY: the stand-in's allocator.
                graphics: unsafe { c::oracle_passes_graphics_new() },
                next: ptr::null_mut(),
                loop_pass: 0,
                looping: false,
            }),
            params: Vec::new(),
            sparams: Vec::new(),
            passes: Vec::new(),
            technique: Vec::new(),
            names: Vec::new(),
        });
        for i in 0..4 {
            let mut p = gs_effect_param {
                name: ptr::null_mut(),
                section: 0,
                type_: 2,
                changed: false,
                cur_val: empty(),
                default_val: empty(),
                effect: &mut *f.effect,
                next_sampler: ptr::null_mut(),
                annotations: empty(),
            };
            // SAFETY: the parameter's own darray.
            unsafe {
                let mut s = DarrayStore::new(&mut p.cur_val);
                s.resize(4);
                s.bytes_mut().copy_from_slice(&(i as f32).to_ne_bytes());
            }
            f.params.push(p);
        }
        f.effect.params = view(&mut f.params, 4);
        let base = f.params.as_mut_ptr();
        for _ in 0..passes {
            for (a, b) in [(0usize, 1usize), (2, 3)] {
                f.sparams.push(vec![
                    pass_shaderparam {
                        eparam: base.wrapping_add(a),
                        sparam: (a + 1) as *mut c_void,
                    },
                    pass_shaderparam {
                        eparam: base.wrapping_add(b),
                        sparam: (b + 1) as *mut c_void,
                    },
                ]);
            }
        }
        for i in 0..passes {
            let name = CString::new(format!("P{i}")).unwrap();
            let vparams = view(&mut f.sparams[2 * i], 2);
            let pparams = view(&mut f.sparams[2 * i + 1], 2);
            f.passes.push(gs_effect_pass {
                name: name.as_ptr().cast_mut(),
                section: 0,
                vertshader: (10 + i) as *mut c_void,
                pixelshader: (20 + i) as *mut c_void,
                vertshader_params: vparams,
                pixelshader_params: pparams,
            });
            f.names.push(name);
        }
        let draw = CString::new("Draw").unwrap();
        let passes_view = view(&mut f.passes, passes);
        f.technique.push(gs_effect_technique {
            name: draw.as_ptr().cast_mut(),
            section: 0,
            effect: &mut *f.effect,
            passes: passes_view,
        });
        f.names.push(draw);
        f.effect.techniques = view(&mut f.technique, 1);
        f
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: the bmem value buffers and the stand-in's subsystem.
        unsafe {
            for p in &self.params {
                obs_util::ffi::bmem::bfree(p.cur_val.array);
            }
            c::oracle_passes_graphics_free(self.effect.graphics);
        }
    }
}

/// One draw. The values are emptied at the end of the technique, so they
/// are put back as the next frame's setters would.
fn draw(f: &mut Fixture, rust: bool, name: &CString) -> usize {
    let mut passes = 0;
    let effect: *mut gs_effect = &mut *f.effect;
    // SAFETY: a valid effect and name; the stand-in records nothing.
    unsafe {
        c::oracle_passes_set_graphics(f.effect.graphics);
        while if rust {
            gs_effect_loop(effect, name.as_ptr())
        } else {
            c::oracle_gs_effect_loop(effect.cast(), name.as_ptr())
        } {
            passes += 1;
        }
        for (i, p) in f.params.iter_mut().enumerate() {
            let mut s = DarrayStore::new(&mut p.cur_val);
            s.resize(4);
            s.bytes_mut().copy_from_slice(&(i as f32).to_ne_bytes());
        }
    }
    passes
}

fn bench(criterion: &mut Criterion, name: &str, passes: usize) {
    let draw_name = CString::new("Draw").unwrap();
    let mut rf = Fixture::new(passes);
    let mut cf = Fixture::new(passes);
    // SAFETY: plain switch of the stand-in.
    unsafe { c::oracle_passes_set_recording(false) };
    assert_eq!(
        draw(&mut rf, true, &draw_name),
        draw(&mut cf, false, &draw_name)
    );
    measure_pair(
        criterion,
        name,
        || draw(&mut rf, true, &draw_name),
        || draw(&mut cf, false, &draw_name),
    );
    // SAFETY: as above.
    unsafe {
        c::oracle_passes_set_recording(true);
        c::oracle_passes_set_graphics(ptr::null_mut());
    }
}

pub fn benchmarks(criterion: &mut Criterion) {
    bench(criterion, "effect_passes/draw_one_pass", 1);
    bench(criterion, "effect_passes/draw_two_passes", 2);
}
