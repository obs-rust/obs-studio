//! obs-graphics: the effect parser's shader writer
//! (`libobs/graphics/effect-parser-writer.c.h`). Each side writes a typical
//! pixel shader (a uniform, a texture and a sampler, a mapped struct, and a
//! function that calls a helper) for the call `PSDraw(v)`, and folds the
//! shader string and used params into a checksum both must match. Every
//! iteration frees what it wrote; the fixtures own the rest.

use super::pair;

use core::ffi::c_char;
use core::ptr;
use criterion::Criterion;
use obs_c_oracle::effect_parser_writer as c;
use obs_graphics::ffi::effect_parser::{
    self as rs, ep_func, ep_param, ep_sampler, ep_struct, ep_var,
};
use obs_graphics::ffi::effect_parser_lifecycle::effect_parser;
use obs_util::ffi::cf_tokenizer::cf_token;
use obs_util::ffi::darray::{bfree, darray};
use obs_util::ffi::dstr::dstr;
use obs_util::ffi::lexer::strref;
use std::ffi::CStr;
use std::hint::black_box;

const SAMPLE: &[u8] = b"{\n\treturn image.Sample(def_sampler, uv) * tint;\n}\0";
const DRAW: &[u8] = b"{\n\treturn sample_at(v.uv);\n}\0";
const CALL: &[u8] = b"PSDraw(v);\0";

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

fn body(b: &'static [u8]) -> dstr {
    dstr {
        array: b.as_ptr().cast_mut().cast(),
        len: b.len() - 1,
        capacity: b.len(),
    }
}

fn param(ty: &'static CStr, name: &'static CStr) -> ep_param {
    ep_param {
        r#type: s(ty),
        name: s(name),
        default_val: empty(),
        properties: empty(),
        param: ptr::null_mut(),
        is_const: false,
        is_property: false,
        is_uniform: true,
        is_texture: false,
        written: false,
        writeorder: 0,
        array_count: 0,
        annotations: empty(),
    }
}

/// The parser and the call, over memory the fixture owns.
struct Fixture {
    ep: Box<effect_parser>,
    call: Box<darray>,
    _params: Vec<ep_param>,
    _structs: Vec<ep_struct>,
    _vars: Vec<Vec<ep_var>>,
    _funcs: Vec<ep_func>,
    _samplers: Vec<ep_sampler>,
    _lists: Vec<Vec<*mut c_char>>,
    _tokens: Vec<cf_token>,
}

impl Fixture {
    fn new() -> Self {
        let var = |ty, name, mapping: Option<&'static CStr>| ep_var {
            r#type: s(ty),
            name: s(name),
            mapping: mapping.map_or(ptr::null_mut(), s),
            var_type: 0,
        };
        let mut vars = vec![
            vec![var(c"float4", c"pos", Some(c"POSITION")), var(c"float2", c"uv", Some(c"TEXCOORD0"))],
            vec![var(c"float2", c"uv", None)],
            vec![var(c"VertInOut", c"v", None)],
        ];
        let mut lists = vec![
            vec![s(c"Filter"), s(c"AddressU")],
            vec![s(c"Linear"), s(c"Clamp")],
            vec![s(c"tint"), s(c"image")],
            vec![s(c"def_sampler")],
            vec![s(c"VertInOut")],
            vec![s(c"sample_at")],
        ];
        let mut params = vec![param(c"float4", c"tint"), param(c"texture2d", c"image")];
        let mut structs = vec![ep_struct {
            name: s(c"VertInOut"),
            vars: over(&mut vars[0]),
            written: false,
        }];
        let mut samplers = vec![ep_sampler {
            name: s(c"def_sampler"),
            states: over(&mut lists[0]),
            values: over(&mut lists[1]),
            written: false,
        }];
        let mut funcs = vec![
            ep_func {
                name: s(c"sample_at"),
                ret_type: s(c"float4"),
                mapping: ptr::null_mut(),
                contents: body(SAMPLE),
                param_vars: over(&mut vars[1]),
                func_deps: empty(),
                struct_deps: empty(),
                param_deps: over(&mut lists[2]),
                sampler_deps: over(&mut lists[3]),
                written: false,
            },
            ep_func {
                name: s(c"PSDraw"),
                ret_type: s(c"float4"),
                mapping: s(c"TARGET"),
                contents: body(DRAW),
                param_vars: over(&mut vars[2]),
                func_deps: over(&mut lists[5]),
                struct_deps: over(&mut lists[4]),
                param_deps: empty(),
                sampler_deps: empty(),
                written: false,
            },
        ];
        let base = CALL.as_ptr().cast::<c_char>();
        let token = |off: usize, len: usize| {
            // SAFETY: off + len is inside CALL.
            let str = strref {
                array: unsafe { base.add(off) },
                len,
            };
            cf_token {
                lex: ptr::null(),
                str,
                unmerged_str: str,
                r#type: 1,
            }
        };
        let mut tokens = vec![token(0, 6), token(6, 1), token(7, 1), token(8, 1), token(9, 1)];
        // SAFETY: cf_token_clear(): all zero.
        tokens.push(unsafe { core::mem::zeroed() });

        // SAFETY: every field of effect_parser permits zero; the writer reads
        // only the four arrays set here.
        let mut ep: Box<effect_parser> = Box::new(unsafe { core::mem::zeroed() });
        ep.params = over(&mut params);
        ep.structs = over(&mut structs);
        ep.funcs = over(&mut funcs);
        ep.samplers = over(&mut samplers);
        let call = Box::new(over(&mut tokens));
        Self {
            ep,
            call,
            _params: params,
            _structs: structs,
            _vars: vars,
            _funcs: funcs,
            _samplers: samplers,
            _lists: lists,
            _tokens: tokens,
        }
    }
}

type Writer = unsafe fn(*mut effect_parser, *mut dstr, *mut darray, *mut darray);

/// One shader, folded into a checksum, then freed.
fn run(f: &mut Fixture, write: Writer) -> u64 {
    let mut shader = dstr {
        array: ptr::null_mut(),
        len: 0,
        capacity: 0,
    };
    let mut used = empty();
    // SAFETY: the fixture's parser and call, an empty dstr and darray.
    unsafe {
        write(
            black_box(&raw mut *f.ep),
            &raw mut shader,
            &raw mut *f.call,
            &raw mut used,
        );
    }
    let mut acc = shader.len as u64;
    // SAFETY: used holds num dstrs on bmalloc; shader's buffer is bmalloc'd.
    unsafe {
        for i in 0..used.num {
            let d = &*used.array.cast::<dstr>().add(i);
            acc = acc * 31 + d.len as u64;
            bfree(d.array.cast());
        }
        bfree(used.array);
        bfree(shader.array.cast());
    }
    acc
}

unsafe fn oracle_write(ep: *mut effect_parser, shader: *mut dstr, call: *mut darray, used: *mut darray) {
    // SAFETY: the caller's.
    unsafe { c::oracle_ep_makeshaderstring(ep.cast(), shader.cast(), call.cast(), used.cast()) };
}

unsafe fn rust_write(ep: *mut effect_parser, shader: *mut dstr, call: *mut darray, used: *mut darray) {
    // SAFETY: the caller's.
    unsafe { rs::ep_makeshaderstring(ep, shader, call, used) };
}

pub fn benchmarks(criterion: &mut Criterion) {
    let mut rust = Fixture::new();
    let mut oracle = Fixture::new();
    pair(
        criterion,
        "effect_parser/writer/shader",
        || run(&mut rust, rust_write),
        || run(&mut oracle, oracle_write),
    );
}
