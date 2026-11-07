//! obs-graphics: the effect parser's compile step
//! (`libobs/graphics/effect-parser-compile.c.h`). Each side compiles a
//! typical effect (a `ViewProj` matrix, a color with a default and an
//! annotation, and one technique with a pass whose vertex and pixel calls
//! use them) over the recording graphics stand-in, and folds the result,
//! the effect's params, passes and shader params into a checksum both must
//! match. Every iteration frees the effect it filled; the fixtures own the
//! parser.

use super::pair;

use core::ffi::c_char;
use core::ptr;
use criterion::Criterion;
use obs_c_oracle::effect_parser_compile as c;
use obs_graphics::ffi::effect_lookups::{gs_effect, gs_effect_param, gs_effect_technique};
use obs_graphics::ffi::effect_parser::{self as rs, ep_param, ep_pass, ep_technique};
use obs_graphics::ffi::effect_parser_lifecycle::effect_parser;
use obs_util::ffi::cf_tokenizer::cf_token;
use obs_util::ffi::darray::darray;
use obs_util::ffi::lexer::strref;
use std::ffi::CStr;
use std::hint::black_box;

const COLOR: [u8; 16] = [0, 0, 0x80, 0x3f, 0, 0, 0, 0x3f, 0, 0, 0x80, 0x3e, 0, 0, 0x80, 0x3f];

fn s(c: &'static CStr) -> *mut c_char {
    c.as_ptr().cast_mut()
}

fn over<T>(v: &mut [T]) -> darray {
    darray {
        array: v.as_mut_ptr().cast(),
        num: v.len(),
        capacity: v.len(),
    }
}

const fn empty() -> darray {
    darray {
        array: ptr::null_mut(),
        num: 0,
        capacity: 0,
    }
}

fn param(ty: &'static CStr, name: &'static CStr, uniform: bool) -> ep_param {
    ep_param {
        r#type: s(ty),
        name: s(name),
        default_val: empty(),
        properties: empty(),
        param: ptr::null_mut(),
        is_const: false,
        is_property: false,
        is_uniform: uniform,
        is_texture: false,
        written: false,
        writeorder: 0,
        array_count: 0,
        annotations: empty(),
    }
}

fn token(text: &'static str, r#type: i32) -> cf_token {
    let str = strref {
        array: text.as_ptr().cast(),
        len: text.len(),
    };
    cf_token {
        lex: ptr::null(),
        str,
        unmerged_str: str,
        r#type,
    }
}

fn call(name: &'static str, arg: &'static str) -> Vec<cf_token> {
    let mut tokens = vec![
        token(name, 1),
        token("(", 6),
        token(arg, 1),
        token(")", 6),
    ];
    // SAFETY: cf_token_clear(): all zero.
    tokens.push(unsafe { core::mem::zeroed() });
    tokens
}

/// The parser, over memory the fixture owns, and the effect it fills.
struct Fixture {
    ep: Box<effect_parser>,
    effect: Box<gs_effect>,
    _params: Vec<ep_param>,
    _annotations: Vec<ep_param>,
    _techniques: Vec<ep_technique>,
    _passes: Vec<ep_pass>,
    _calls: Vec<Vec<cf_token>>,
    defaults: Vec<[u8; 16]>,
}

impl Fixture {
    fn new() -> Self {
        // SAFETY: every field of both permits zero; the oracle initializes
        // them.
        let mut ep: Box<effect_parser> = Box::new(unsafe { core::mem::zeroed() });
        let mut effect: Box<gs_effect> = Box::new(unsafe { core::mem::zeroed() });
        // SAFETY: boxed structs that stay put, and a C string the lexer
        // copies.
        let ok = unsafe {
            c::oracle_effect_parser_compile_prepare(
                (&raw mut *ep).cast(),
                (&raw mut *effect).cast(),
                c"bench.effect".as_ptr(),
            )
        };
        assert!(ok);
        let mut annotations = vec![param(c"string", c"name", false)];
        let mut params = vec![
            param(c"float4x4", c"ViewProj", true),
            param(c"float4", c"color", true),
        ];
        params[1].annotations = over(&mut annotations);
        let mut calls = vec![call("VSDraw", "ViewProj"), call("PSDraw", "color")];
        let mut passes = vec![ep_pass {
            name: ptr::null_mut(),
            vertex_program: over(&mut calls[0]),
            fragment_program: over(&mut calls[1]),
            pass: ptr::null_mut(),
        }];
        let mut techniques = vec![ep_technique {
            name: s(c"Draw"),
            passes: over(&mut passes),
        }];
        ep.params = over(&mut params);
        ep.techniques = over(&mut techniques);
        Self {
            ep,
            effect,
            _params: params,
            _annotations: annotations,
            _techniques: techniques,
            _passes: passes,
            _calls: calls,
            defaults: vec![COLOR],
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // The parser's arrays are the fixture's: free only the cf parser
        // and the (empty) effect.
        self.ep.params = empty();
        self.ep.techniques = empty();
        // SAFETY: a prepared parser whose arrays are empty now.
        unsafe {
            c::oracle_effect_parser_compile_free(
                (&raw mut *self.ep).cast(),
                (&raw mut *self.effect).cast(),
            );
        }
    }
}

type Compile = unsafe fn(*mut effect_parser) -> bool;

/// One effect, folded into a checksum, then freed.
fn run(f: &mut Fixture, compile: Compile) -> u64 {
    // The compile step moves the color's default out of the parser: give
    // it back, over the fixture's buffer, before each run.
    let default = f.defaults[0].as_mut_ptr();
    // SAFETY: param 1 of the fixture's parser.
    unsafe {
        (*f.ep.params.array.cast::<ep_param>().add(1)).default_val = darray {
            array: default.cast(),
            num: 16,
            capacity: 16,
        };
    }
    let missing = c"none";
    // SAFETY: the stand-in records into its own buffer.
    unsafe { c::oracle_compile_device_reset(0, 0, 0, 0, missing.as_ptr()) };
    // SAFETY: the fixture's prepared parser and effect.
    let ok = unsafe { compile(black_box(&raw mut *f.ep)) };
    let effect = &mut *f.effect;
    let mut acc = u64::from(ok);
    // SAFETY: the arrays as the compile step sized them.
    unsafe {
        for i in 0..effect.params.num {
            let p = &mut *effect.params.array.cast::<gs_effect_param>().add(i);
            acc = acc * 31 + p.type_ as u64 + p.annotations.num as u64;
            // The default came from the fixture's buffer: take it back
            // before effect_free() sees it.
            if p.default_val.array == default.cast() {
                acc = acc * 31 + p.default_val.num as u64;
                p.default_val = empty();
            }
        }
        for t in 0..effect.techniques.num {
            let tech = &*effect.techniques.array.cast::<gs_effect_technique>().add(t);
            acc = acc * 31 + tech.passes.num as u64;
        }
        c::oracle_compile_device_stop();
        c::oracle_effect_parser_compile_effect_reset((&raw mut *f.effect).cast());
    }
    acc
}

unsafe fn oracle_compile(ep: *mut effect_parser) -> bool {
    // SAFETY: the caller's.
    unsafe { c::oracle_ep_compile(ep.cast()) }
}

unsafe fn rust_compile(ep: *mut effect_parser) -> bool {
    // SAFETY: the caller's.
    unsafe { rs::ep_compile(ep) }
}

pub fn benchmarks(criterion: &mut Criterion) {
    let mut rust = Fixture::new();
    let mut oracle = Fixture::new();
    pair(
        criterion,
        "effect_parser/compile/effect",
        || run(&mut rust, rust_compile),
        || run(&mut oracle, oracle_compile),
    );
}
