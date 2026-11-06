//! Tier 2 layouts and Tier 3 original-C parity for the effect parser's
//! shader writer (`libobs/graphics/effect-parser-writer.c.h`,
//! `obs_graphics::ffi::effect_parser`).
//!
//! Both sides get the same parser, built twice from one random model: its
//! params, structs, samplers and functions (every string, each function's
//! body as a `struct dstr` that may hold NUL bytes, its param vars with
//! every `enum ep_var_type` value and one past it, and its dependency
//! lists), with random `written` flags, and the same pass call: no token
//! array, or tokens with whitespace, `NULL`, names of the parser's items,
//! punctuation, empty tokens whose next byte is or is not whitespace, and
//! tokens holding a NUL, before the end token. The shader string may start
//! with text already in it. The oracle is the original C with the lookups
//! of `effect-parser-funcs.c.h` and the C `strref_cmp` and dstr; the shim
//! is `ep_makeshaderstring`. The shader string (bytes, length, capacity),
//! every pushed `used_params` entry (bytes, length, capacity) with the
//! list's count and capacity, and every `written` flag afterwards must
//! match.
//!
//! The parser only records dependency names it found, and the C crashes on
//! one it did not, so every dependency names an item of its kind here (the
//! port skips a missing one; Tier 1 covers that). Names are never `NULL`
//! (`strcmp` would crash). A call's token array always ends in its end
//! token, as the pass parser leaves it (the C reads on past its last token
//! until it finds one).
//!
//! No intentional differences.

use core::ffi::{c_char, c_int};
use core::mem::{align_of, offset_of, size_of};
use core::ptr;
use std::ffi::CString;

use obs_c_oracle::effect_parser_writer as c;
use obs_graphics::ffi::effect_parser::{
    self as rs, ep_func, ep_param, ep_sampler, ep_struct, ep_var,
};
use obs_graphics::ffi::effect_parser_lifecycle::effect_parser;
use obs_util::ffi::cf_tokenizer::cf_token;
use obs_util::ffi::darray::{bfree, bmalloc, darray};
use obs_util::ffi::dstr::dstr;
use obs_util::ffi::lexer::strref;
use proptest::prelude::*;

#[test]
fn layouts() {
    let rust = [
        size_of::<ep_func>(),
        align_of::<ep_func>(),
        offset_of!(ep_func, name),
        offset_of!(ep_func, ret_type),
        offset_of!(ep_func, mapping),
        offset_of!(ep_func, contents),
        offset_of!(ep_func, param_vars),
        offset_of!(ep_func, func_deps),
        offset_of!(ep_func, struct_deps),
        offset_of!(ep_func, param_deps),
        offset_of!(ep_func, sampler_deps),
        offset_of!(ep_func, written),
        size_of::<ep_param>(),
        align_of::<ep_param>(),
        offset_of!(ep_param, r#type),
        offset_of!(ep_param, name),
        offset_of!(ep_param, is_const),
        offset_of!(ep_param, is_uniform),
        offset_of!(ep_param, written),
        offset_of!(ep_param, array_count),
        size_of::<ep_var>(),
        align_of::<ep_var>(),
        offset_of!(ep_var, r#type),
        offset_of!(ep_var, name),
        offset_of!(ep_var, mapping),
        offset_of!(ep_var, var_type),
        size_of::<c_int>(),
        size_of::<ep_struct>(),
        align_of::<ep_struct>(),
        offset_of!(ep_struct, name),
        offset_of!(ep_struct, vars),
        offset_of!(ep_struct, written),
        size_of::<ep_sampler>(),
        align_of::<ep_sampler>(),
        offset_of!(ep_sampler, name),
        offset_of!(ep_sampler, states),
        offset_of!(ep_sampler, values),
        offset_of!(ep_sampler, written),
        size_of::<dstr>(),
        align_of::<dstr>(),
        offset_of!(dstr, array),
        offset_of!(dstr, len),
        offset_of!(dstr, capacity),
    ];
    let mut oracle = [0usize; 64];
    // SAFETY: the oracle writes fewer than 64 values.
    let n = unsafe { c::oracle_effect_parser_writer_layout(oracle.as_mut_ptr()) };
    assert_eq!(&rust[..], &oracle[..n]);
}

#[derive(Clone, Debug)]
struct MVar {
    ty: Vec<u8>,
    name: Vec<u8>,
    mapping: Option<Vec<u8>>,
    var_type: c_int,
}

#[derive(Clone, Debug)]
struct MParam {
    ty: Vec<u8>,
    name: Vec<u8>,
    is_const: bool,
    is_uniform: bool,
    count: c_int,
    written: bool,
}

#[derive(Clone, Debug)]
struct MStruct {
    name: Vec<u8>,
    vars: Vec<MVar>,
    written: bool,
}

#[derive(Clone, Debug)]
struct MSampler {
    name: Vec<u8>,
    items: Vec<(Vec<u8>, Vec<u8>)>,
    written: bool,
}

#[derive(Clone, Debug)]
struct MFunc {
    name: Vec<u8>,
    ret: Vec<u8>,
    mapping: Option<Vec<u8>>,
    contents: Vec<u8>,
    vars: Vec<MVar>,
    /// Indices into the parser's lists, taken modulo their lengths.
    funcs: Vec<usize>,
    structs: Vec<usize>,
    params: Vec<usize>,
    samplers: Vec<usize>,
    written: bool,
}

#[derive(Clone, Debug)]
struct Model {
    params: Vec<MParam>,
    structs: Vec<MStruct>,
    funcs: Vec<MFunc>,
    samplers: Vec<MSampler>,
    /// `None` for no token array; each token's text, and for an empty one
    /// the byte after it.
    call: Option<Vec<(Vec<u8>, u8)>>,
    /// Text already in the shader string, and spare capacity past it.
    prefix: Option<(Vec<u8>, usize)>,
}

/// Names shared by every kind of item, so calls, struct types and
/// dependencies meet them.
const NAMES: &[&str] = &[
    "a",
    "b",
    "f",
    "g",
    "S",
    "T",
    "smp",
    "tint",
    "main",
    "NULL",
    "",
    "VertInOut",
    "float4",
];

fn name() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        8 => prop::sample::select(NAMES).prop_map(|s| s.as_bytes().to_vec()),
        1 => "[a-z]{1,4}".prop_map(String::into_bytes),
    ]
}

fn mapping() -> impl Strategy<Value = Option<Vec<u8>>> {
    prop::option::weighted(
        0.5,
        prop::sample::select(&["POSITION", "TEXCOORD0", "TARGET", "COLOR", ""][..])
            .prop_map(|s| s.as_bytes().to_vec()),
    )
}

fn var() -> impl Strategy<Value = MVar> {
    (name(), name(), mapping(), 0..5).prop_map(|(ty, name, mapping, var_type)| MVar {
        ty,
        name,
        mapping,
        var_type,
    })
}

fn indices() -> impl Strategy<Value = Vec<usize>> {
    prop::collection::vec(0usize..16, 0..4)
}

fn model() -> impl Strategy<Value = Model> {
    let param = (
        name(),
        name(),
        any::<bool>(),
        any::<bool>(),
        prop_oneof![4 => Just(0), 2 => 1..5, 1 => any::<c_int>()],
        any::<bool>(),
    )
        .prop_map(|(ty, name, is_const, is_uniform, count, written)| MParam {
            ty,
            name,
            is_const,
            is_uniform,
            count,
            written,
        });
    let strukt = (name(), prop::collection::vec(var(), 0..4), any::<bool>()).prop_map(
        |(name, vars, written)| MStruct {
            name,
            vars,
            written,
        },
    );
    let sampler = (
        name(),
        prop::collection::vec((name(), name()), 0..3),
        any::<bool>(),
    )
        .prop_map(|(name, items, written)| MSampler {
            name,
            items,
            written,
        });
    let func = (
        (name(), name(), mapping()),
        prop_oneof![
            4 => "\\{[ -~\\n\\t]{0,12}\\}".prop_map(String::into_bytes),
            1 => prop::collection::vec(any::<u8>(), 0..6),
        ],
        prop::collection::vec(var(), 0..4),
        (indices(), indices(), indices(), indices()),
        any::<bool>(),
    )
        .prop_map(
            |(
                (name, ret, mapping),
                contents,
                vars,
                (funcs, structs, params, samplers),
                written,
            )| {
                MFunc {
                    name,
                    ret,
                    mapping,
                    contents,
                    vars,
                    funcs,
                    structs,
                    params,
                    samplers,
                    written,
                }
            },
        );
    let token = prop_oneof![
        6 => name().prop_map(|t| (t, 0u8)),
        3 => prop::sample::select(&["(", ")", ",", ";", " ", "\t", "\n", "\r", "NULL"][..])
            .prop_map(|t| (t.as_bytes().to_vec(), 0u8)),
        1 => prop::sample::select(&[b' ', b'x', b'\n', 0u8][..]).prop_map(|b| (Vec::new(), b)),
        1 => prop::sample::select(&["NULL\0x", "a\0", "\0a"][..])
            .prop_map(|t| (t.as_bytes().to_vec(), 0u8)),
    ];
    (
        prop::collection::vec(param, 0..5),
        prop::collection::vec(strukt, 0..4),
        prop::collection::vec(func, 0..5),
        prop::collection::vec(sampler, 0..3),
        prop::option::weighted(0.95, prop::collection::vec(token, 0..10)),
        prop::option::weighted(0.2, (prop::collection::vec(any::<u8>(), 1..6), 0usize..8)),
    )
        .prop_map(|(params, structs, funcs, samplers, call, prefix)| Model {
            params,
            structs,
            funcs,
            samplers,
            call,
            prefix,
        })
}

/// The parser and the call, as C structs over memory this owns.
struct CParser {
    ep: Box<effect_parser>,
    call: Box<darray>,
    strings: Vec<CString>,
    bodies: Vec<Vec<u8>>,
    vars: Vec<Vec<ep_var>>,
    lists: Vec<Vec<*mut c_char>>,
    params: Vec<ep_param>,
    structs: Vec<ep_struct>,
    funcs: Vec<ep_func>,
    samplers: Vec<ep_sampler>,
    text: Vec<u8>,
    tokens: Vec<cf_token>,
}

const fn empty() -> darray {
    darray {
        array: ptr::null_mut(),
        num: 0,
        capacity: 0,
    }
}

fn over<T>(v: &mut Vec<T>) -> darray {
    darray {
        array: if v.is_empty() {
            ptr::null_mut()
        } else {
            v.as_mut_ptr().cast()
        },
        num: v.len(),
        capacity: v.len(),
    }
}

impl CParser {
    fn cstr(&mut self, s: &[u8]) -> *mut c_char {
        let c = CString::new(s).expect("no NUL in names");
        let p = c.as_ptr().cast_mut();
        self.strings.push(c);
        p
    }

    fn opt(&mut self, s: Option<&Vec<u8>>) -> *mut c_char {
        s.map_or(ptr::null_mut(), |s| self.cstr(s))
    }

    fn vars(&mut self, vars: &[MVar]) -> darray {
        let mut out = Vec::new();
        for v in vars {
            out.push(ep_var {
                r#type: self.cstr(&v.ty),
                name: self.cstr(&v.name),
                mapping: self.opt(v.mapping.as_ref()),
                var_type: v.var_type,
            });
        }
        let da = over(&mut out);
        self.vars.push(out);
        da
    }

    fn list(&mut self, names: Vec<*mut c_char>) -> darray {
        let mut names = names;
        let da = over(&mut names);
        self.lists.push(names);
        da
    }

    fn new(m: &Model) -> Self {
        let mut p = Self {
            // SAFETY: every field of effect_parser permits zero; the writer
            // reads only the four arrays set below.
            ep: Box::new(unsafe { core::mem::zeroed() }),
            call: Box::new(empty()),
            strings: Vec::new(),
            bodies: Vec::new(),
            vars: Vec::new(),
            lists: Vec::new(),
            params: Vec::new(),
            structs: Vec::new(),
            funcs: Vec::new(),
            samplers: Vec::new(),
            text: Vec::new(),
            tokens: Vec::new(),
        };
        for mp in &m.params {
            let param = ep_param {
                r#type: p.cstr(&mp.ty),
                name: p.cstr(&mp.name),
                default_val: empty(),
                properties: empty(),
                param: ptr::null_mut(),
                is_const: mp.is_const,
                is_property: false,
                is_uniform: mp.is_uniform,
                is_texture: false,
                written: mp.written,
                writeorder: 0,
                array_count: mp.count,
                annotations: empty(),
            };
            p.params.push(param);
        }
        for ms in &m.structs {
            let st = ep_struct {
                name: p.cstr(&ms.name),
                vars: p.vars(&ms.vars),
                written: ms.written,
            };
            p.structs.push(st);
        }
        for ms in &m.samplers {
            let states: Vec<_> = ms.items.iter().map(|(s, _)| p.cstr(s)).collect();
            let values: Vec<_> = ms.items.iter().map(|(_, v)| p.cstr(v)).collect();
            let sa = ep_sampler {
                name: p.cstr(&ms.name),
                states: p.list(states),
                values: p.list(values),
                written: ms.written,
            };
            p.samplers.push(sa);
        }
        p.funcs_from(m);
        p.ep.params = over(&mut p.params);
        p.ep.structs = over(&mut p.structs);
        p.ep.funcs = over(&mut p.funcs);
        p.ep.samplers = over(&mut p.samplers);

        if let Some(call) = &m.call {
            p.call_from(call);
        }
        p
    }

    /// The functions, with their bodies, vars and dependency lists.
    fn funcs_from(&mut self, m: &Model) {
        // Dependencies name items of their kind: the C crashes on others.
        let pick = |names: &[Vec<u8>], idx: &[usize]| -> Vec<Vec<u8>> {
            if names.is_empty() {
                return Vec::new();
            }
            idx.iter().map(|i| names[i % names.len()].clone()).collect()
        };
        let func_names: Vec<_> = m.funcs.iter().map(|f| f.name.clone()).collect();
        let struct_names: Vec<_> = m.structs.iter().map(|s| s.name.clone()).collect();
        let param_names: Vec<_> = m.params.iter().map(|s| s.name.clone()).collect();
        let sampler_names: Vec<_> = m.samplers.iter().map(|s| s.name.clone()).collect();
        for mf in &m.funcs {
            let mut body = mf.contents.clone();
            let len = body.len();
            // dstr_cat_dstr copies the NUL after the bytes too.
            body.push(0);
            let contents = dstr {
                array: if len == 0 {
                    ptr::null_mut()
                } else {
                    body.as_mut_ptr().cast()
                },
                len,
                capacity: if len == 0 { 0 } else { len + 1 },
            };
            self.bodies.push(body);
            let mut deps = Vec::new();
            for (names, idx) in [
                (&func_names, &mf.funcs),
                (&struct_names, &mf.structs),
                (&param_names, &mf.params),
                (&sampler_names, &mf.samplers),
            ] {
                let list: Vec<_> = pick(names, idx).iter().map(|n| self.cstr(n)).collect();
                deps.push(self.list(list));
            }
            let mut deps = deps.into_iter();
            let mut next = || deps.next().expect("four lists");
            let f = ep_func {
                name: self.cstr(&mf.name),
                ret_type: self.cstr(&mf.ret),
                mapping: self.opt(mf.mapping.as_ref()),
                contents,
                param_vars: self.vars(&mf.vars),
                func_deps: next(),
                struct_deps: next(),
                param_deps: next(),
                sampler_deps: next(),
                written: mf.written,
            };
            self.funcs.push(f);
        }
    }

    /// The call's tokens over one text, then the end token.
    fn call_from(&mut self, call: &[(Vec<u8>, u8)]) {
        let mut offsets = Vec::new();
        for (t, lead) in call {
            offsets.push((self.text.len(), t.len()));
            if t.is_empty() {
                self.text.push(*lead);
            } else {
                self.text.extend_from_slice(t);
            }
        }
        self.text.push(0);
        let base = self.text.as_ptr().cast::<c_char>();
        for (off, len) in offsets {
            // SAFETY: off is inside text.
            let s = strref {
                array: unsafe { base.add(off) },
                len,
            };
            self.tokens.push(cf_token {
                lex: ptr::null(),
                str: s,
                unmerged_str: s,
                r#type: 1,
            });
        }
        // SAFETY: cf_token_clear(): all zero, CFTOKEN_NONE.
        self.tokens.push(unsafe { core::mem::zeroed() });
        *self.call = over(&mut self.tokens);
    }

    fn flags(&self) -> Vec<bool> {
        let mut out: Vec<bool> = self.params.iter().map(|x| x.written).collect();
        out.extend(self.structs.iter().map(|x| x.written));
        out.extend(self.funcs.iter().map(|x| x.written));
        out.extend(self.samplers.iter().map(|x| x.written));
        out
    }
}

/// A dstr's bytes (with its NUL, `None` for no buffer), length and
/// capacity.
type DstrShape = (Option<Vec<u8>>, usize, usize);

/// A dstr's shape, then freed.
fn take(d: &mut dstr) -> DstrShape {
    let bytes = (!d.array.is_null()).then(|| {
        // SAFETY: a dstr buffer holds len bytes and its NUL.
        unsafe { core::slice::from_raw_parts(d.array.cast::<u8>(), d.len + 1) }.to_vec()
    });
    // SAFETY: the buffer came from bmalloc (or is null).
    unsafe { bfree(d.array.cast()) };
    (bytes, d.len, d.capacity)
}

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    shader: DstrShape,
    /// The list's count and capacity, and each entry.
    used: (usize, usize, Vec<DstrShape>),
    flags: Vec<bool>,
}

fn run(m: &Model, oracle: bool) -> Outcome {
    let mut p = CParser::new(m);
    let mut shader = dstr {
        array: ptr::null_mut(),
        len: 0,
        capacity: 0,
    };
    if let Some((bytes, spare)) = &m.prefix {
        let cap = bytes.len() + 1 + spare;
        // SAFETY: a fresh buffer of cap bytes, holding the prefix and a NUL.
        unsafe {
            let buf = bmalloc(cap).cast::<u8>();
            ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len());
            *buf.add(bytes.len()) = 0;
            shader.array = buf.cast();
        }
        shader.len = bytes.len();
        shader.capacity = cap;
    }
    let mut used = empty();
    let ep = &raw mut *p.ep;
    let call = &raw mut *p.call;
    // SAFETY: a parser whose arrays and strings outlive the call, a dstr and
    // an empty darray of dstr on bmalloc, and a call ending in its end token
    // (or with no array).
    unsafe {
        if oracle {
            c::oracle_ep_makeshaderstring(
                ep.cast(),
                (&raw mut shader).cast(),
                call.cast(),
                (&raw mut used).cast(),
            );
        } else {
            rs::ep_makeshaderstring(ep, &raw mut shader, call, &raw mut used);
        }
    }
    let mut entries = Vec::new();
    for i in 0..used.num {
        // SAFETY: used holds num dstrs.
        entries.push(take(unsafe { &mut *used.array.cast::<dstr>().add(i) }));
    }
    let used_shape = (used.num, used.capacity, entries);
    // SAFETY: used's buffer came from bmalloc (or is null).
    unsafe { bfree(used.array) };
    Outcome {
        shader: take(&mut shader),
        used: used_shape,
        flags: p.flags(),
    }
}

fn check(m: &Model) -> Result<(), TestCaseError> {
    prop_assert_eq!(run(m, false), run(m, true), "{:?}", m);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(1024)))]

    #[test]
    fn random_parsers(m in model()) {
        check(&m)?;
    }

    /// The call names the parser's first function, so the whole writer
    /// runs more often.
    #[test]
    fn called_functions(mut m in model(), args in prop::collection::vec(name(), 0..3)) {
        if let Some(f) = m.funcs.first() {
            let mut call = vec![(f.name.clone(), 0), (b"(".to_vec(), 0)];
            for a in args {
                call.push((a, 0));
                call.push((b",".to_vec(), 0));
            }
            call.push((b")".to_vec(), 0));
            m.call = Some(call);
        }
        check(&m)?;
    }
}
