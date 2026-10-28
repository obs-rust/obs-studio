//! Tier 3: the shims against the original `libobs/graphics/effect-passes.c.h`.
//!
//! The oracle (`rust/obs-c-oracle/oracle/effect_passes.c`) is the C file,
//! unmodified, with its 7 exports renamed. Both sides call the same
//! recording graphics stand-in (`oracle/effect_passes_device.c`), so they
//! are compared call by call. Each side gets its own copy of the same
//! hand-built effect and runs the same sequence of calls; after every call
//! the test compares: the device calls it made, its return value, the
//! effect's loop state (looping, loop_pass, the current technique and
//! pass), `graphics->cur_effect`, every parameter's `cur_val` (num,
//! capacity, bytes), `changed` and `next_sampler`, and the `LOG_WARNING`
//! lines (count and text).
//!
//! Runs: the layout of `struct gs_shader_param_info` (the type comes first,
//! unlike `struct gs_effect_param_info`); the cmocka scenarios of
//! `test_effect_pass_loop`; 5000 random
//! sequences of up to 24 calls on random effects (0 to 3 passes, 1 to 2
//! techniques, values, defaults, changed flags and next samplers at
//! random, another effect active or not).
//!
//! Excluded, the intentional differences, tested on the shim alone below:
//! a NULL technique name and a NULL pass name (the C passes them to
//! `strcmp`).

use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr;
use std::ffi::CString;

use obs_c_oracle::effect_passes as c;
use obs_graphics::effect_params::ByteStore;
use obs_graphics::ffi::effect_lookups::{
    gs_effect, gs_effect_param, gs_effect_pass, gs_effect_technique,
};
use obs_graphics::ffi::effect_params::DarrayStore;
use obs_graphics::ffi::effect_passes as r;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use obs_util::ffi::darray::darray;
use proptest::prelude::*;

#[repr(C)]
struct Rec {
    calls: c_int,
    level: c_int,
    message: [c_char; 256],
}

unsafe extern "C" {
    fn base_test_record_ptr() -> *mut c_void;
    fn base_test_record_only_this_thread();
    fn base_test_record_any_thread();
    fn bfree(ptr: *mut c_void);
}

fn empty() -> darray {
    darray {
        array: ptr::null_mut(),
        num: 0,
        capacity: 0,
    }
}

/// A Rust-owned array as a darray the loop only reads.
fn view<T>(v: &mut Vec<T>) -> darray {
    darray {
        array: v.as_mut_ptr().cast(),
        num: v.len(),
        capacity: v.len(),
    }
}

#[derive(Debug, Clone)]
struct ParamSpec {
    value: Vec<u8>,
    default: Vec<u8>,
    changed: bool,
    sampler: usize,
}

#[derive(Debug, Clone)]
struct Spec {
    /// Passes per technique; technique 0 is "Draw", 1 "Other".
    techniques: Vec<usize>,
    params: Vec<ParamSpec>,
}

/// One side's effect. Everything the loop does not free is Rust-owned.
struct Fx {
    effect: Box<gs_effect>,
    params: Vec<gs_effect_param>,
    techniques: Vec<gs_effect_technique>,
    passes: Vec<Vec<gs_effect_pass>>,
    sparams: Vec<Vec<[r::pass_shaderparam; 2]>>,
    names: Vec<CString>,
    graphics: *mut c_void,
}

impl Fx {
    #[expect(
        clippy::too_many_lines,
        reason = "parity table: one block per ported C case"
    )]
    fn new(spec: &Spec) -> Box<Self> {
        // SAFETY: the stand-in's allocator.
        let graphics = unsafe { c::oracle_passes_graphics_new() };
        let mut fx = Box::new(Fx {
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
                graphics,
                next: ptr::null_mut(),
                loop_pass: 0,
                looping: false,
            }),
            params: Vec::new(),
            techniques: Vec::new(),
            passes: Vec::new(),
            sparams: Vec::new(),
            names: Vec::new(),
            graphics,
        });
        for p in &spec.params {
            let mut param = gs_effect_param {
                name: ptr::null_mut(),
                section: 0,
                type_: 2,
                changed: p.changed,
                cur_val: empty(),
                default_val: empty(),
                effect: &mut *fx.effect,
                next_sampler: p.sampler as *mut c_void,
                annotations: empty(),
            };
            // SAFETY: the parameter's own darrays.
            unsafe {
                let mut s = DarrayStore::new(&mut param.cur_val);
                s.resize(p.value.len());
                s.bytes_mut().copy_from_slice(&p.value);
                let mut s = DarrayStore::new(&mut param.default_val);
                s.resize(p.default.len());
                s.bytes_mut().copy_from_slice(&p.default);
            }
            fx.params.push(param);
        }
        fx.effect.params = view(&mut fx.params);
        let base = fx.params.as_mut_ptr();
        let np = fx.params.len();
        let at = |i: usize| -> *mut gs_effect_param {
            if np == 0 {
                ptr::null_mut()
            } else {
                base.wrapping_add(i % np)
            }
        };
        for (t, &n) in spec.techniques.iter().enumerate() {
            let mut sps = Vec::new();
            let mut passes = Vec::new();
            for i in 0..n {
                // Vertex: params 0, 1 on sparams 1, 2; pixel: param 2 on
                // sparam 3 and param 3 on the texture 100.
                sps.push([
                    r::pass_shaderparam {
                        eparam: at(0),
                        sparam: ptr::without_provenance_mut(1),
                    },
                    r::pass_shaderparam {
                        eparam: at(1),
                        sparam: ptr::without_provenance_mut(2),
                    },
                ]);
                sps.push([
                    r::pass_shaderparam {
                        eparam: at(2),
                        sparam: ptr::without_provenance_mut(3),
                    },
                    r::pass_shaderparam {
                        eparam: at(3),
                        sparam: ptr::without_provenance_mut(100),
                    },
                ]);
                let name = CString::new(if i == 0 { "First" } else { "Second" }).unwrap();
                passes.push((name, 10 + i + 10 * t, 20 + i + 10 * t));
            }
            fx.sparams.push(sps);
            let mut pv = Vec::new();
            for (i, (name, vs, ps)) in passes.into_iter().enumerate() {
                let count = if np == 0 { 0 } else { 2 };
                let sp = &mut fx.sparams[t];
                let (v, p) = sp.split_at_mut(2 * i + 1);
                let vparams = darray {
                    array: v[2 * i].as_mut_ptr().cast(),
                    num: count,
                    capacity: count,
                };
                let pparams = darray {
                    array: p[0].as_mut_ptr().cast(),
                    num: count,
                    capacity: count,
                };
                pv.push(gs_effect_pass {
                    name: name.as_ptr().cast_mut(),
                    section: 0,
                    vertshader: vs as *mut c_void,
                    pixelshader: ps as *mut c_void,
                    vertshader_params: vparams,
                    pixelshader_params: pparams,
                });
                fx.names.push(name);
            }
            fx.passes.push(pv);
        }
        for t in 0..spec.techniques.len() {
            let name = CString::new(if t == 0 { "Draw" } else { "Other" }).unwrap();
            let passes = view(&mut fx.passes[t]);
            fx.techniques.push(gs_effect_technique {
                name: name.as_ptr().cast_mut(),
                section: 0,
                effect: &mut *fx.effect,
                passes,
            });
            fx.names.push(name);
        }
        fx.effect.techniques = view(&mut fx.techniques);
        fx
    }

    fn tech(&mut self, t: usize) -> *mut gs_effect_technique {
        self.techniques.get_mut(t).map_or(ptr::null_mut(), |t| t)
    }

    /// The state compared after each call.
    fn state(&self) -> String {
        let e = &*self.effect;
        let tech = self
            .techniques
            .iter()
            .position(|t| ptr::eq(t, e.cur_technique));
        let pass = self.passes.iter().enumerate().find_map(|(t, ps)| {
            ps.iter()
                .position(|p| ptr::eq(p, e.cur_pass))
                .map(|i| (t, i))
        });
        // SAFETY: this side's graphics subsystem.
        let cur = unsafe { c::oracle_passes_cur_effect(self.graphics) };
        let cur = if cur.is_null() {
            "none"
        } else if ptr::eq(cur, &*self.effect as *const gs_effect as *const c_void) {
            "self"
        } else {
            "other"
        };
        let mut s = format!(
            "looping={} loop_pass={} tech={tech:?} pass={pass:?} cur={cur}",
            e.looping, e.loop_pass
        );
        for p in &self.params {
            let bytes = if p.cur_val.num == 0 || p.cur_val.array.is_null() {
                Vec::new()
            } else {
                // SAFETY: `num` bytes at `array`.
                unsafe { core::slice::from_raw_parts(p.cur_val.array.cast::<u8>(), p.cur_val.num) }
                    .to_vec()
            };
            s += &format!(
                " [{}/{} {:02x?} {} {}]",
                p.cur_val.num, p.cur_val.capacity, bytes, p.changed, p.next_sampler as usize
            );
        }
        s
    }
}

impl Drop for Fx {
    fn drop(&mut self) {
        // SAFETY: the bmem value buffers and the stand-in's subsystem.
        unsafe {
            for p in &self.params {
                bfree(p.cur_val.array);
                bfree(p.default_val.array);
            }
            c::oracle_passes_graphics_free(self.graphics);
        }
    }
}

#[derive(Debug, Clone)]
enum Op {
    Loop(u8),
    Update,
    Begin(usize),
    End(usize),
    BeginPass(usize, usize),
    BeginPassByName(usize, bool),
    EndPass(usize),
    Change(usize),
    OtherActive(bool),
}

fn events() -> Vec<String> {
    // SAFETY: the stand-in's records.
    unsafe {
        (0..c::oracle_passes_count())
            .map(|i| {
                CStr::from_ptr(c::oracle_passes_event(i))
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }
}

/// Runs `ops` on one side and returns one line per call.
#[expect(
    clippy::too_many_lines,
    reason = "parity table: one block per ported C case"
)]
fn run(spec: &Spec, ops: &[Op], rust: bool) -> Vec<String> {
    let mut fx = Fx::new(spec);
    let other = Box::new(0u8);
    let draw = CString::new("Draw").unwrap();
    let nope = CString::new("Nope").unwrap();
    let first = CString::new("First").unwrap();
    let second = CString::new("Second").unwrap();
    // SAFETY: this side's subsystem for gs_get_effect.
    unsafe { c::oracle_passes_set_graphics(fx.graphics) };
    let mut out = Vec::new();
    for op in ops {
        let mut rec = Rec {
            calls: 0,
            level: 0,
            message: [0; 256],
        };
        let (mut h, mut hp) = (ptr::null_mut(), ptr::null_mut());
        let effect: *mut gs_effect = &mut *fx.effect;
        let ve = effect.cast::<c_void>();
        // SAFETY: valid effects and techniques of this side, C strings;
        // the record outlives the handler, restored before it goes.
        let ret = unsafe {
            c::oracle_passes_reset();
            base_get_log_handler(&mut h, &mut hp);
            base_test_record_only_this_thread();
            base_set_log_handler(base_test_record_ptr(), (&mut rec as *mut Rec).cast());
            let ret = match op {
                Op::Loop(which) => {
                    let name = if which % 2 == 0 {
                        draw.as_ptr()
                    } else {
                        nope.as_ptr()
                    };
                    format!(
                        "{}",
                        if rust {
                            r::gs_effect_loop(effect, name)
                        } else {
                            c::oracle_gs_effect_loop(ve, name)
                        }
                    )
                }
                Op::Update => {
                    if rust {
                        r::gs_effect_update_params(effect)
                    } else {
                        c::oracle_gs_effect_update_params(ve)
                    }
                    String::new()
                }
                Op::Begin(t) => {
                    let t = fx.tech(*t);
                    format!(
                        "{}",
                        if rust {
                            r::gs_technique_begin(t)
                        } else {
                            c::oracle_gs_technique_begin(t.cast())
                        }
                    )
                }
                Op::End(t) => {
                    let t = fx.tech(*t);
                    if rust {
                        r::gs_technique_end(t)
                    } else {
                        c::oracle_gs_technique_end(t.cast())
                    }
                    String::new()
                }
                Op::BeginPass(t, i) => {
                    let t = fx.tech(*t);
                    format!(
                        "{}",
                        if rust {
                            r::gs_technique_begin_pass(t, *i)
                        } else {
                            c::oracle_gs_technique_begin_pass(t.cast(), *i)
                        }
                    )
                }
                Op::BeginPassByName(t, f) => {
                    let t = fx.tech(*t);
                    let n = if *f { first.as_ptr() } else { second.as_ptr() };
                    format!(
                        "{}",
                        if rust {
                            r::gs_technique_begin_pass_by_name(t, n)
                        } else {
                            c::oracle_gs_technique_begin_pass_by_name(t.cast(), n)
                        }
                    )
                }
                Op::EndPass(t) => {
                    let t = fx.tech(*t);
                    if rust {
                        r::gs_technique_end_pass(t)
                    } else {
                        c::oracle_gs_technique_end_pass(t.cast())
                    }
                    String::new()
                }
                Op::Change(i) => {
                    let n = fx.params.len();
                    if n > 0 {
                        fx.params[i % n].changed = true;
                    }
                    String::new()
                }
                Op::OtherActive(on) => {
                    let cur = if *on {
                        (&*other as *const u8).cast_mut().cast()
                    } else {
                        ptr::null_mut()
                    };
                    c::oracle_passes_set_cur_effect(fx.graphics, cur);
                    String::new()
                }
            };
            base_set_log_handler(h, hp);
            base_test_record_any_thread();
            ret
        };
        let log = if rec.calls > 0 {
            let bytes: Vec<u8> = rec.message.iter().map(|b| *b as u8).collect();
            let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
            format!("{} {}", rec.calls, String::from_utf8_lossy(&bytes[..end]))
        } else {
            String::new()
        };
        out.push(format!(
            "{op:?} -> {ret} | {:?} | {} | log {log}",
            events(),
            fx.state()
        ));
    }
    // SAFETY: no subsystem for gs_get_effect after the run.
    unsafe { c::oracle_passes_set_graphics(ptr::null_mut()) };
    out
}

fn compare(spec: &Spec, ops: &[Op]) {
    let _guard = super::LOG_HANDLER.lock().unwrap_or_else(|e| e.into_inner());
    let rs = run(spec, ops, true);
    let cs = run(spec, ops, false);
    for (i, (a, b)) in rs.iter().zip(&cs).enumerate() {
        assert_eq!(a, b, "call {i} of {ops:?} on {spec:?}");
    }
}

fn f32b(v: f32) -> Vec<u8> {
    v.to_ne_bytes().to_vec()
}

fn cmocka_spec(passes: usize) -> Spec {
    let p = |value: Vec<u8>| ParamSpec {
        value,
        default: Vec::new(),
        changed: false,
        sampler: 0,
    };
    Spec {
        techniques: vec![passes],
        params: vec![p(f32b(1.0)), p(Vec::new()), p(f32b(2.0)), p(Vec::new())],
    }
}

#[test]
fn shader_param_info_layout() {
    use core::mem::{align_of, offset_of, size_of};
    let values = [
        size_of::<r::gs_shader_param_info>(),
        align_of::<r::gs_shader_param_info>(),
        offset_of!(r::gs_shader_param_info, type_),
        offset_of!(r::gs_shader_param_info, name),
    ];
    for (which, value) in values.into_iter().enumerate() {
        // SAFETY: a pure C function.
        assert_eq!(
            unsafe { c::oracle_passes_shader_param_info_layout(which) },
            value,
            "{which}"
        );
    }
}

#[test]
fn cmocka_scenarios() {
    use Op::*;
    // one_pass_loop, two_pass_loop
    compare(&cmocka_spec(1), &[Change(0), Loop(0), Loop(0)]);
    compare(&cmocka_spec(2), &[Loop(0), Loop(0), Loop(0)]);
    // missing_technique_and_active_effect
    compare(
        &cmocka_spec(1),
        &[
            Loop(1),
            OtherActive(true),
            Loop(0),
            OtherActive(false),
            Loop(0),
        ],
    );
    // default_upload_and_next_sampler
    let mut s = cmocka_spec(1);
    s.params[1].default = f32b(3.0);
    s.params[3] = ParamSpec {
        value: vec![7; 16],
        default: Vec::new(),
        changed: false,
        sampler: 55,
    };
    compare(&s, &[Loop(0), Loop(0)]);
    // update_params_sends_only_changed
    let mut s = cmocka_spec(1);
    s.params[3].sampler = 56;
    compare(
        &s,
        &[
            Loop(0),
            Update,
            Change(2),
            Update,
            EndPass(0),
            Change(2),
            Update,
            End(0),
        ],
    );
    // begin_pass_by_name_and_bounds
    compare(
        &cmocka_spec(2),
        &[
            Begin(0),
            BeginPassByName(0, false),
            BeginPassByName(0, true),
            BeginPass(0, 2),
            EndPass(0),
            EndPass(0),
            End(0),
            Begin(9),
            BeginPass(9, 0),
            End(9),
            EndPass(9),
        ],
    );
}

/// The shim alone: NULL names (the C passes them to strcmp).
#[test]
fn shim_null_names() {
    let _guard = super::LOG_HANDLER.lock().unwrap_or_else(|e| e.into_inner());
    let mut fx = Fx::new(&cmocka_spec(1));
    // SAFETY: a valid effect; NULL names are the case under test.
    unsafe {
        c::oracle_passes_set_graphics(fx.graphics);
        assert!(!r::gs_effect_loop(&mut *fx.effect, ptr::null()));
        assert!(!fx.effect.looping);
        let t = fx.tech(0);
        assert!(!r::gs_technique_begin_pass_by_name(t, ptr::null()));
        assert!(!r::gs_effect_loop(ptr::null_mut(), ptr::null()));
        c::oracle_passes_set_graphics(ptr::null_mut());
    }
}

fn param_strategy() -> impl Strategy<Value = ParamSpec> {
    (
        prop_oneof![Just(Vec::new()), prop::collection::vec(any::<u8>(), 1..24)],
        prop_oneof![Just(Vec::new()), prop::collection::vec(any::<u8>(), 1..24)],
        any::<bool>(),
        prop_oneof![Just(0usize), 1usize..90],
    )
        .prop_map(|(value, default, changed, sampler)| ParamSpec {
            value,
            default,
            changed,
            sampler,
        })
}

fn spec_strategy() -> impl Strategy<Value = Spec> {
    (
        prop::collection::vec(0usize..3, 1..3),
        prop::collection::vec(param_strategy(), 0..5),
    )
        .prop_map(|(techniques, params)| Spec { techniques, params })
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => any::<u8>().prop_map(Op::Loop),
        1 => Just(Op::Update),
        1 => (0usize..3).prop_map(Op::Begin),
        1 => (0usize..3).prop_map(Op::End),
        1 => (0usize..3, 0usize..4).prop_map(|(t, i)| Op::BeginPass(t, i)),
        1 => (0usize..3, any::<bool>()).prop_map(|(t, f)| Op::BeginPassByName(t, f)),
        1 => (0usize..3).prop_map(Op::EndPass),
        1 => (0usize..5).prop_map(Op::Change),
        1 => any::<bool>().prop_map(Op::OtherActive),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5000))]

    #[test]
    fn random_sequences_match(spec in spec_strategy(), ops in prop::collection::vec(op_strategy(), 0..24)) {
        compare(&spec, &ops);
    }
}
