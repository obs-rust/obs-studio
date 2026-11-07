//! Tier 2 layouts and Tier 3 original-C parity for the effect parser's
//! compile step (`libobs/graphics/effect-parser-compile.c.h`,
//! `obs_graphics::ffi::effect_parser::ep_compile`).
//!
//! Each side gets its own copy of the same parser: the oracle's C lexer
//! parses a short text into it (so `cf_adderror()` has a token to point at,
//! the end token, where `ep_parse()` leaves `cur_token`), with the file
//! name the case picks, and the test fills its params (types, names,
//! uniform and const flags, default bytes with spare capacity, annotations)
//! and techniques (names or `NULL`, passes with names or `NULL` and vertex
//! and pixel calls: no token array, or names, params, `NULL`, whitespace
//! and punctuation tokens before the end token), all on libobs's
//! allocator. Then the oracle (the original C, with the C writer, param
//! types, effect param lookup and `cf_adderror`) and the shim each compile
//! it over the same recording graphics stand-in
//! (`oracle/effect_parser_compile_device.c`), which fails the create calls,
//! hands back error texts, and leaves a name out of the shaders, as the
//! case says. The result, every device call and log line in order, the
//! allocations left (each such test re-execs itself alone, as the counter
//! is process-wide), every field of the effect (each param, annotation,
//! technique, pass and pass shader param, with the darray counts and
//! capacities), what is left in the parser (each default value, each
//! param's link), and every error (text, file, row, column, level) must
//! match.
//!
//! The parser has no functions, so the writer writes only the params a
//! call names (its own parity is `effect_parser_writer_parity`), and the C
//! and Rust writers write the same text.
//!
//! No intentional differences: the C's `_DEBUG_SHADERS` lines, the one
//! difference of the port, are in no build here.

use core::ffi::{CStr, c_char};
use core::mem::{align_of, offset_of, size_of};
use core::ptr;
use std::ffi::CString;

use obs_c_oracle::effect_parser_compile as c;
use obs_graphics::ffi::effect_lookups::{
    gs_effect, gs_effect_param, gs_effect_pass, gs_effect_technique,
};
use obs_graphics::ffi::effect_parser::{self as rs, ep_param, ep_pass, ep_technique};
use obs_graphics::ffi::effect_parser_lifecycle::effect_parser;
use obs_graphics::ffi::effect_passes::pass_shaderparam;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use obs_util::ffi::bmem::bnum_allocs;
use obs_util::ffi::cf_tokenizer::cf_token;
use obs_util::ffi::darray::{bmalloc, darray};
use obs_util::ffi::lexer::{error_item, strref};
use proptest::prelude::*;

#[test]
fn layouts() {
    let rust = [
        size_of::<pass_shaderparam>(),
        align_of::<pass_shaderparam>(),
        offset_of!(pass_shaderparam, eparam),
        offset_of!(pass_shaderparam, sparam),
        size_of::<ep_param>(),
        align_of::<ep_param>(),
        offset_of!(ep_param, r#type),
        offset_of!(ep_param, name),
        offset_of!(ep_param, default_val),
        offset_of!(ep_param, properties),
        offset_of!(ep_param, param),
        offset_of!(ep_param, is_const),
        offset_of!(ep_param, is_property),
        offset_of!(ep_param, is_uniform),
        offset_of!(ep_param, is_texture),
        offset_of!(ep_param, written),
        offset_of!(ep_param, writeorder),
        offset_of!(ep_param, array_count),
        offset_of!(ep_param, annotations),
        size_of::<effect_parser>(),
        align_of::<effect_parser>(),
        offset_of!(effect_parser, effect),
        offset_of!(effect_parser, params),
        offset_of!(effect_parser, structs),
        offset_of!(effect_parser, funcs),
        offset_of!(effect_parser, samplers),
        offset_of!(effect_parser, techniques),
        offset_of!(effect_parser, files),
        offset_of!(effect_parser, tokens),
        offset_of!(effect_parser, cur_pass),
        offset_of!(effect_parser, cfp),
        size_of::<i32>(),
    ];
    let mut oracle = [0usize; 48];
    // SAFETY: the oracle writes fewer than 48 values.
    let n = unsafe { c::oracle_effect_parser_compile_layout(oracle.as_mut_ptr()) };
    assert_eq!(&rust[..], &oracle[..n]);
}

/// The names params, annotations, techniques and passes get, and calls
/// name.
const NAMES: &[&str] = &[
    "ViewProj", "World", "viewproj", "world", "color", "image", "a", "b",
];
/// Param types: every kind of `obs_effect_param_type()` answer.
const TYPES: &[&str] = &[
    "float4x4",
    "float4",
    "float",
    "int",
    "bool",
    "texture2d",
    "string",
    "half",
    "int3",
    "float2",
];
/// The file names the parser is lexed with.
const FILES: &[Option<&str>] = &[Some("parity"), Some(""), None, Some("dir/x.effect")];

const CFTOKEN_NAME: i32 = 1;
const CFTOKEN_SPACETAB: i32 = 3;
const CFTOKEN_OTHER: i32 = 6;

/// Call tokens past the names: `NULL`, whitespace, punctuation.
const EXTRA_TOKENS: &[(&str, i32)] = &[
    ("NULL", CFTOKEN_NAME),
    (" ", CFTOKEN_SPACETAB),
    ("(", CFTOKEN_OTHER),
    (")", CFTOKEN_OTHER),
    (",", CFTOKEN_OTHER),
    ("main", CFTOKEN_NAME),
];

fn token(i: usize) -> (&'static str, i32) {
    if i < NAMES.len() {
        (NAMES[i], CFTOKEN_NAME)
    } else {
        EXTRA_TOKENS[(i - NAMES.len()) % EXTRA_TOKENS.len()]
    }
}

#[derive(Clone, Debug)]
struct MParam {
    ty: usize,
    name: usize,
    default: Vec<u8>,
    extra_cap: usize,
    uniform: bool,
    is_const: bool,
    annotations: Vec<MParam>,
}

#[derive(Clone, Debug)]
struct MPass {
    name: Option<usize>,
    vertex: Option<Vec<usize>>,
    pixel: Option<Vec<usize>>,
}

#[derive(Clone, Debug)]
struct MTech {
    name: Option<usize>,
    passes: Vec<MPass>,
}

#[derive(Clone, Debug)]
struct Case {
    file: usize,
    params: Vec<MParam>,
    techs: Vec<MTech>,
    null_mask: u32,
    error_mask: u32,
    empty_mask: u32,
    missing_mask: u32,
    missing: usize,
}

/// A `bmalloc` copy of `s` with its NUL.
fn cdup(s: &str) -> *mut c_char {
    // SAFETY: a plain allocation holding the bytes and the NUL.
    unsafe {
        let p = bmalloc(s.len() + 1).cast::<u8>();
        ptr::copy_nonoverlapping(s.as_ptr(), p, s.len());
        *p.add(s.len()) = 0;
        p.cast()
    }
}

/// A darray of `items` on libobs's allocator, with `extra` spare slots.
fn da_of<T>(items: Vec<T>, extra: usize) -> darray {
    let cap = items.len() + extra;
    if cap == 0 {
        return darray {
            array: ptr::null_mut(),
            num: 0,
            capacity: 0,
        };
    }
    // SAFETY: room for cap items; the items move in.
    unsafe {
        let p = bmalloc(size_of::<T>() * cap).cast::<T>();
        let num = items.len();
        for (i, item) in items.into_iter().enumerate() {
            p.add(i).write(item);
        }
        darray {
            array: p.cast(),
            num,
            capacity: cap,
        }
    }
}

fn empty() -> darray {
    da_of::<u8>(Vec::new(), 0)
}

fn param_to_c(p: &MParam) -> ep_param {
    let ty = TYPES[p.ty];
    ep_param {
        r#type: cdup(ty),
        name: cdup(NAMES[p.name]),
        default_val: da_of(p.default.clone(), p.extra_cap),
        properties: empty(),
        param: ptr::null_mut(),
        is_const: p.is_const,
        is_property: false,
        is_uniform: p.uniform,
        is_texture: ty.starts_with("texture"),
        written: false,
        writeorder: 0,
        array_count: 0,
        annotations: da_of(p.annotations.iter().map(param_to_c).collect(), 0),
    }
}

fn program_to_c(call: Option<&Vec<usize>>) -> darray {
    let Some(call) = call else {
        return empty();
    };
    let mut tokens: Vec<cf_token> = call
        .iter()
        .map(|&i| {
            let (text, r#type) = token(i);
            let s = strref {
                array: text.as_ptr().cast(),
                len: text.len(),
            };
            cf_token {
                lex: ptr::null(),
                str: s,
                unmerged_str: strref {
                    array: s.array,
                    len: s.len,
                },
                r#type,
            }
        })
        .collect();
    // SAFETY: every field of cf_token permits zero: cf_token_clear().
    tokens.push(unsafe { core::mem::zeroed() });
    da_of(tokens, 0)
}

fn tech_to_c(t: &MTech) -> ep_technique {
    ep_technique {
        name: t.name.map_or(ptr::null_mut(), |n| cdup(NAMES[n])),
        passes: da_of(
            t.passes
                .iter()
                .map(|p| ep_pass {
                    name: p.name.map_or(ptr::null_mut(), |n| cdup(NAMES[n])),
                    vertex_program: program_to_c(p.vertex.as_ref()),
                    fragment_program: program_to_c(p.pixel.as_ref()),
                    pass: ptr::null_mut(),
                })
                .collect(),
            0,
        ),
    }
}

/// One side's parser and effect.
struct Side {
    ep: Box<effect_parser>,
    effect: Box<gs_effect>,
}

impl Side {
    fn new(case: &Case) -> Self {
        // SAFETY: every field of both permits zero; the oracle initializes
        // them before anything reads them.
        let mut side = unsafe {
            Self {
                ep: Box::new(core::mem::zeroed()),
                effect: Box::new(core::mem::zeroed()),
            }
        };
        let file = FILES[case.file].map(|f| CString::new(f).unwrap());
        // SAFETY: boxed structs that stay put, and a C string or NULL the
        // lexer copies.
        let ok = unsafe {
            c::oracle_effect_parser_compile_prepare(
                (&raw mut *side.ep).cast(),
                (&raw mut *side.effect).cast(),
                file.as_ref().map_or(ptr::null(), |f| f.as_ptr()),
            )
        };
        assert!(ok);
        side.ep.params = da_of(case.params.iter().map(param_to_c).collect(), 0);
        side.ep.techniques = da_of(case.techs.iter().map(tech_to_c).collect(), 0);
        side
    }
}

impl Drop for Side {
    fn drop(&mut self) {
        // SAFETY: a prepared parser and its effect; the oracle frees both
        // as ep_free() and effect_free() do.
        unsafe {
            c::oracle_effect_parser_compile_free(
                (&raw mut *self.ep).cast(),
                (&raw mut *self.effect).cast(),
            );
        }
    }
}

fn text(p: *const c_char) -> String {
    if p.is_null() {
        "(null)".into()
    } else {
        // SAFETY: every non-null string here is NUL-terminated.
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

fn shape(da: &darray) -> String {
    format!(
        "{}/{}{}",
        da.num,
        da.capacity,
        if da.array.is_null() { " null" } else { "" }
    )
}

/// # Safety
///
/// `da` holds `num` items of `T`.
unsafe fn items<T>(da: &darray) -> &[T] {
    if da.num == 0 {
        return &[];
    }
    // SAFETY: the caller's contract.
    unsafe { core::slice::from_raw_parts(da.array.cast::<T>(), da.num) }
}

/// An effect param pointer as its index in the effect's params.
fn eparam_index(effect: &gs_effect, p: *const gs_effect_param) -> String {
    if p.is_null() {
        return "-".into();
    }
    // SAFETY: the effect's params.
    let params: &[gs_effect_param] = unsafe { items(&effect.params) };
    params
        .iter()
        .position(|q| ptr::eq(q, p))
        .map_or_else(|| "foreign".into(), |i| i.to_string())
}

fn dump_param(out: &mut String, effect: &gs_effect, p: &gs_effect_param, indent: &str) {
    // SAFETY: the default value's bytes.
    let default: &[u8] = unsafe { items(&p.default_val) };
    out.push_str(&format!(
        "{indent}{} section={} type={} changed={} cur={} default={:02x?} {} effect={} sampler={} annotations={}\n",
        text(p.name),
        p.section,
        p.type_,
        p.changed,
        shape(&p.cur_val),
        default,
        shape(&p.default_val),
        if ptr::eq(p.effect, effect) {
            "self"
        } else if p.effect.is_null() {
            "null"
        } else {
            "other"
        },
        !p.next_sampler.is_null(),
        shape(&p.annotations)
    ));
    // SAFETY: the param's annotations.
    for a in unsafe { items::<gs_effect_param>(&p.annotations) } {
        dump_param(out, effect, a, "    ");
    }
}

fn snapshot(side: &Side) -> String {
    let effect = &*side.effect;
    let mut out = String::new();
    out.push_str(&format!(
        "params {} techniques {}\n",
        shape(&effect.params),
        shape(&effect.techniques)
    ));
    // SAFETY: the effect's arrays as the compile step sized them.
    unsafe {
        for p in items::<gs_effect_param>(&effect.params) {
            dump_param(&mut out, effect, p, "  ");
        }
        out.push_str(&format!(
            "view_proj={} world={} scale={}\n",
            eparam_index(effect, effect.view_proj),
            eparam_index(effect, effect.world),
            eparam_index(effect, effect.scale)
        ));
        for t in items::<gs_effect_technique>(&effect.techniques) {
            out.push_str(&format!(
                "technique {} section={} effect={} passes={}\n",
                text(t.name),
                t.section,
                ptr::eq(t.effect, effect),
                shape(&t.passes)
            ));
            for p in items::<gs_effect_pass>(&t.passes) {
                out.push_str(&format!(
                    "  pass {} section={} vert={:?} pixel={:?}\n",
                    text(p.name),
                    p.section,
                    p.vertshader,
                    p.pixelshader
                ));
                for (label, params) in [("v", &p.vertshader_params), ("p", &p.pixelshader_params)] {
                    out.push_str(&format!("    {label} {}", shape(params)));
                    for sp in items::<pass_shaderparam>(params) {
                        out.push_str(&format!(
                            " {}/{:?}",
                            eparam_index(effect, sp.eparam),
                            sp.sparam
                        ));
                    }
                    out.push('\n');
                }
            }
        }
        for p in items::<ep_param>(&side.ep.params) {
            out.push_str(&format!(
                "parser {} default={} param={}\n",
                text(p.name),
                shape(&p.default_val),
                eparam_index(effect, p.param.cast())
            ));
            for a in items::<ep_param>(&p.annotations) {
                out.push_str(&format!(
                    "  annotation {} default={} param={:?}\n",
                    text(a.name),
                    shape(&a.default_val),
                    a.param
                ));
            }
        }
        for e in items::<error_item>(&side.ep.cfp.error_list.errors) {
            out.push_str(&format!(
                "error {} {} {}:{} {}\n",
                text(e.error),
                text(e.file),
                e.row,
                e.column,
                e.level
            ));
        }
    }
    out
}

/// Runs one side on the case: its result, the device's calls and log
/// lines, the allocations left, and the effect, parser and errors after.
fn run(case: &Case, rust: bool) -> String {
    let side = Side::new(case);
    let missing = CString::new(NAMES[case.missing]).unwrap();
    let ep: *mut effect_parser = (&raw const *side.ep).cast_mut();
    let (mut h, mut hp) = (ptr::null_mut(), ptr::null_mut());
    // SAFETY: a prepared parser; the handler records only this thread's
    // lines and is restored before the case ends.
    let (ok, allocs) = unsafe {
        base_get_log_handler(&mut h, &mut hp);
        c::oracle_compile_device_reset(
            case.null_mask,
            case.error_mask,
            case.empty_mask,
            case.missing_mask,
            missing.as_ptr(),
        );
        base_set_log_handler(c::oracle_compile_log_handler_ptr(), ptr::null_mut());
        let before = bnum_allocs();
        let ok = if rust {
            rs::ep_compile(ep)
        } else {
            c::oracle_ep_compile(ep.cast())
        };
        let allocs = bnum_allocs() - before;
        c::oracle_compile_device_stop();
        base_set_log_handler(h, hp);
        (ok, allocs)
    };
    let mut len = 0;
    // SAFETY: the device's log, len bytes.
    let events = unsafe {
        let p = c::oracle_compile_device_events(&mut len);
        String::from_utf8_lossy(core::slice::from_raw_parts(p.cast::<u8>(), len)).into_owned()
    };
    format!("result {ok}\nallocs {allocs}\n{events}{}", snapshot(&side))
}

fn check(case: &Case) -> Result<(), TestCaseError> {
    let _guard = super::LOG_HANDLER.lock().unwrap_or_else(|e| e.into_inner());
    let oracle = run(case, false);
    let shim = run(case, true);
    prop_assert_eq!(oracle, shim, "{:?}", case);
    Ok(())
}

/// The allocation counter, log handler and graphics stand-in are
/// process-wide, and the other tests in this executable allocate too: in
/// the parent, re-exec the test `name` of this module alone, in this same
/// test executable, and return false; in that child, return true.
fn in_own_process(name: &str) -> bool {
    const CHILD: &str = "OBS_EFFECT_PARSER_COMPILE_PARITY_CHILD";
    if std::env::var_os(CHILD).is_some() {
        return true;
    }
    let name = format!("{}::{name}", module_path!());
    let name = name.split_once("::").unwrap().1;
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("running 1 test"), "{stdout}");
    assert!(
        output.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    false
}

#[test]
fn null_parser() {
    // SAFETY: a null parser does nothing (the C asserts on it).
    assert!(!unsafe { rs::ep_compile(ptr::null_mut()) });
}

#[test]
fn empty_and_fixed_cases() {
    if !in_own_process("empty_and_fixed_cases") {
        return;
    }
    let solid = |name: usize| MParam {
        ty: 1,
        name,
        default: Vec::new(),
        extra_cap: 0,
        uniform: true,
        is_const: false,
        annotations: Vec::new(),
    };
    let pass = MPass {
        name: None,
        vertex: Some(vec![5, NAMES.len() + 2, 0, NAMES.len() + 3]),
        pixel: Some(vec![3]),
    };
    let base = Case {
        file: 0,
        params: vec![solid(0), solid(1), solid(3)],
        techs: vec![MTech {
            name: Some(3),
            passes: vec![
                pass.clone(),
                MPass {
                    name: Some(6),
                    ..pass
                },
            ],
        }],
        null_mask: 0,
        error_mask: 0,
        empty_mask: 0,
        missing_mask: 0,
        missing: 0,
    };
    for case in [
        Case {
            params: Vec::new(),
            techs: Vec::new(),
            ..base.clone()
        },
        base.clone(),
        Case {
            null_mask: 0b1001,
            error_mask: 0b0110,
            empty_mask: 0b1000,
            ..base.clone()
        },
        Case {
            missing_mask: 0b0101,
            ..base.clone()
        },
        Case {
            null_mask: !0,
            error_mask: !0,
            file: 2,
            ..base.clone()
        },
    ] {
        check(&case).unwrap();
    }
}

fn param(annotations: bool) -> impl Strategy<Value = MParam> {
    (
        0..TYPES.len(),
        0..NAMES.len(),
        prop::collection::vec(any::<u8>(), 0..6),
        0usize..3,
        any::<bool>(),
        any::<bool>(),
        prop::collection::vec(
            (
                0..TYPES.len(),
                0..NAMES.len(),
                prop::collection::vec(any::<u8>(), 0..4),
            ),
            0..if annotations { 3 } else { 1 },
        ),
    )
        .prop_map(
            |(ty, name, default, extra_cap, uniform, is_const, anns)| MParam {
                ty,
                name,
                default,
                extra_cap,
                uniform,
                is_const,
                annotations: anns
                    .into_iter()
                    .map(|(ty, name, default)| MParam {
                        ty,
                        name,
                        default,
                        extra_cap: 0,
                        uniform: false,
                        is_const: false,
                        annotations: Vec::new(),
                    })
                    .collect(),
            },
        )
}

fn call() -> impl Strategy<Value = Option<Vec<usize>>> {
    prop::option::weighted(
        0.85,
        prop::collection::vec(0..NAMES.len() + EXTRA_TOKENS.len(), 0..6),
    )
}

fn case() -> impl Strategy<Value = Case> {
    (
        0..FILES.len(),
        prop::collection::vec(param(true), 0..6),
        prop::collection::vec(
            (
                prop::option::weighted(0.9, 0..NAMES.len()),
                prop::collection::vec((prop::option::of(0..NAMES.len()), call(), call()), 0..3),
            ),
            0..3,
        ),
        (any::<u32>(), any::<u32>(), any::<u32>(), any::<u32>()),
        0..NAMES.len(),
        any::<u8>(),
    )
        .prop_map(|(file, params, techs, masks, missing, sparse)| {
            // Sparse failures most of the time, so whole passes compile.
            let thin = |m: u32| {
                if sparse % 4 == 0 {
                    m
                } else {
                    m & m.rotate_left(7) & m.rotate_left(13)
                }
            };
            Case {
                file,
                params,
                techs: techs
                    .into_iter()
                    .map(|(name, passes)| MTech {
                        name,
                        passes: passes
                            .into_iter()
                            .map(|(name, vertex, pixel)| MPass {
                                name,
                                vertex,
                                pixel,
                            })
                            .collect(),
                    })
                    .collect(),
                null_mask: thin(masks.0),
                error_mask: thin(masks.1),
                empty_mask: thin(masks.2),
                missing_mask: thin(masks.3),
                missing,
            }
        })
}

#[test]
fn random_parsers() {
    if !in_own_process("random_parsers") {
        return;
    }
    let config = ProptestConfig::with_cases(obs_c_oracle::parity::cases(400));
    let mut runner = proptest::test_runner::TestRunner::new(config);
    if let Err(e) = runner.run(&case(), |case| check(&case)) {
        panic!("{e}");
    }
}
