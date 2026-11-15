//! Tier 2 layout and Tier 3 original-C parity for the effect parser's
//! entry point (`libobs/graphics/effect-parser-entry.c.h`,
//! `obs_graphics::ffi::effect_parser_entry::ep_parse`).
//!
//! Each side gets its own `ep_init()` parser and `effect_init()` effect,
//! the same text, file name and graphics backend name, and parses it: the
//! oracle with the original C `ep_parse()` (on the C block, technique and
//! parameter parsers, cf-parser inlines, lexer and preprocessor), the shim
//! with the Rust one (on the Rust block, technique and parameter shims and
//! obs-util's lexer). Both reach the same C specifier check, function
//! parser and preprocessor, the same backend name stand-in, and the
//! production compile step over the same recording graphics stand-in
//! (`oracle/effect_parser_compile_device.c`), which fails create calls and
//! hands back error texts as the case says. The result, the backend name
//! calls, every device call and log line in order, and everything left in
//! the parser and the effect (where `cur_token` stopped, every param,
//! struct, sampler, function and technique with what they hold, every
//! error with its file, row, column and level, the preprocessor's
//! definitions, and the effect's params, techniques and passes) must
//! match; the fixed cases, run in their own process, compare the
//! allocations left after the parse too.
//!
//! The texts are the C test's, edge cases, and random sequences of
//! declarations, functions, structs, sampler states, techniques, `#ifdef`
//! blocks with the backend's name, `#error` lines, stray `{ ... }` blocks,
//! stray `}`, comments and loose tokens. Every `{` in a text is closed, and
//! no text ends in a `\` line continuation.
//!
//! No intentional differences: the C's `_DEBUG_SHADERS` lines, the one
//! difference of the port, are in no build here.

use core::ffi::CStr;
use core::mem::{align_of, offset_of, size_of};
use core::ptr;
use std::ffi::CString;

use obs_c_oracle::effect_parser_compile as dev;
use obs_c_oracle::effect_parser_entry as c;
use obs_graphics::ffi::effect_lookups::gs_effect;
use obs_graphics::ffi::effect_parser_entry::{self as rs, cf_def};
use obs_graphics::ffi::effect_parser_lifecycle::effect_parser;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use obs_util::ffi::bmem::bnum_allocs;
use proptest::prelude::*;

#[test]
fn layouts() {
    let rust = [
        size_of::<cf_def>(),
        align_of::<cf_def>(),
        offset_of!(cf_def, name),
        offset_of!(cf_def, params),
        offset_of!(cf_def, tokens),
        offset_of!(cf_def, r#macro),
    ];
    let mut oracle = [0usize; 16];
    // SAFETY: the oracle writes fewer than 16 values.
    let n = unsafe { c::oracle_effect_parser_entry_layout(oracle.as_mut_ptr()) };
    assert_eq!(&rust[..], &oracle[..n]);
}

#[test]
fn null_parser() {
    // SAFETY: a null parser returns false (the C dereferences it).
    assert!(!unsafe {
        rs::ep_parse(
            ptr::null_mut(),
            ptr::null_mut(),
            c"x;".as_ptr(),
            ptr::null(),
        )
    });
}

/// The backend names a case is parsed with.
const BACKENDS: &[Option<&str>] = &[None, Some("GL"), Some("D3D11")];
/// The file names.
const FILES: &[Option<&str>] = &[Some("entry.effect"), None, Some("")];

#[derive(Clone, Debug)]
struct Case {
    text: String,
    backend: usize,
    file: usize,
    null_mask: u32,
    error_mask: u32,
}

impl Case {
    fn new(text: &str) -> Self {
        Self {
            text: text.into(),
            backend: 0,
            file: 0,
            null_mask: 0,
            error_mask: 0,
        }
    }
}

/// Runs one side on the case: its result, the backend name calls, the
/// allocations left after the parse (only when `allocs`), the device's
/// calls and log lines, and the parser and effect after.
fn run(case: &Case, rust: bool, allocs: bool) -> String {
    // SAFETY: every field of both permits zero; the oracle initializes
    // them before anything reads them.
    let (mut ep, mut effect): (Box<effect_parser>, Box<gs_effect>) =
        unsafe { (Box::new(core::mem::zeroed()), Box::new(core::mem::zeroed())) };
    let ep: *mut effect_parser = &raw mut *ep;
    let effect: *mut gs_effect = &raw mut *effect;
    let text = CString::new(case.text.as_str()).unwrap();
    let file = FILES[case.file].map(|f| CString::new(f).unwrap());
    let backend = BACKENDS[case.backend].map(|b| CString::new(b).unwrap());
    let missing = c"missing";
    let (mut h, mut hp) = (ptr::null_mut(), ptr::null_mut());
    // SAFETY: a fresh parser and effect, the strings outlive the parse; the
    // handler records only this thread's lines and is restored before the
    // case ends.
    let (ok, left, names) = unsafe {
        let before = bnum_allocs();
        c::oracle_effect_parser_entry_init(ep.cast(), effect.cast());
        c::oracle_effect_parser_entry_set_backend(
            backend.as_ref().map_or(ptr::null(), |b| b.as_ptr()),
        );
        base_get_log_handler(&mut h, &mut hp);
        dev::oracle_compile_device_reset(case.null_mask, case.error_mask, 0, 0, missing.as_ptr());
        base_set_log_handler(dev::oracle_compile_log_handler_ptr(), ptr::null_mut());
        let file = file.as_ref().map_or(ptr::null(), |f| f.as_ptr());
        let ok = if rust {
            rs::ep_parse(ep, effect.cast(), text.as_ptr(), file)
        } else {
            c::oracle_ep_parse(ep.cast(), effect.cast(), text.as_ptr(), file)
        };
        let left = bnum_allocs() - before;
        dev::oracle_compile_device_stop();
        base_set_log_handler(h, hp);
        (ok, left, c::oracle_effect_parser_entry_name_calls())
    };
    let mut len = 0;
    // SAFETY: the device's log, len bytes; the dump is a C string, freed
    // once copied; then the parser and effect are freed.
    let (events, dump) = unsafe {
        let p = dev::oracle_compile_device_events(&mut len);
        let events =
            String::from_utf8_lossy(core::slice::from_raw_parts(p.cast::<u8>(), len)).into_owned();
        let d = c::oracle_effect_parser_entry_dump(ep.cast(), effect.cast());
        let dump = CStr::from_ptr(d).to_string_lossy().into_owned();
        c::oracle_effect_parser_entry_dump_free(d);
        c::oracle_effect_parser_entry_free(ep.cast(), effect.cast());
        (events, dump)
    };
    let left = if allocs { left.to_string() } else { "-".into() };
    format!("result {ok}\nnames {names}\nallocs {left}\n{events}{dump}")
}

fn check(case: &Case, allocs: bool) -> Result<(), TestCaseError> {
    let _guard = super::LOG_HANDLER.lock().unwrap_or_else(|e| e.into_inner());
    let oracle = run(case, false, allocs);
    let shim = run(case, true, allocs);
    prop_assert_eq!(oracle, shim, "{:?}", case);
    Ok(())
}

/// The allocation counter is process-wide, and the other tests in this
/// executable allocate too: in the parent, re-exec the test `name` of this
/// module alone, in this same test executable, and return false; in that
/// child, return true.
fn in_own_process(name: &str) -> bool {
    const CHILD: &str = "OBS_EFFECT_PARSER_ENTRY_PARITY_CHILD";
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

/// The C test's texts, and edge cases.
const FIXED: &[&str] = &[
    "",
    " \t\r\n;;; // comment\n",
    "float value;",
    "#ifdef GL\nfloat selected;\n#else\nfloat fallback;\n#endif\n",
    "#ifdef D3D11\nfloat selected;\n#endif\nfloat always;",
    "; struct S { float x; }; sampler_state sample { Filter = Linear; }; \
     float value; float fun() { return value; } technique Draw { pass {} } ;",
    "uniform uniform float value;",
    "uniform float fun() {}",
    "property const uniform float f() {}",
    "const const property property uniform uniform int x = 1;",
    "#error stop_here\nfloat ignored;",
    "{{}} ; float recovered;",
    "{ float a; } float b;",
    "}",
    "} float c;",
    "uniform",
    "const property",
    "float",
    "float x",
    "float x =",
    "1 x; float 2; float ok;",
    "float (x);",
    "struct",
    "technique",
    "sampler_state",
    "float4x4 ViewProj; texture2d image; sampler_state def { Filter = Linear; };\n\
     struct VertInOut { float4 pos : POSITION; float2 uv : TEXCOORD0; };\n\
     VertInOut VSDefault(VertInOut vert_in) { VertInOut vert_out; \
     vert_out.pos = mul(float4(vert_in.pos.xyz, 1.0), ViewProj); \
     vert_out.uv = vert_in.uv; return vert_out; }\n\
     float4 PSDraw(VertInOut vert_in) : TARGET { return image.Sample(def, vert_in.uv); }\n\
     technique Draw { pass { vertex_shader = VSDefault(vert_in); \
     pixel_shader = PSDraw(vert_in); } }\n",
    "float4 color = {1.0, 0.5, 0.25, 1.0}; int count[2]; string name = \"a\"; bool on = true;",
    "\"loose\" ; 42 ; ( ) , = < > [ ] :",
];

#[test]
fn fixed_cases() {
    if !in_own_process("fixed_cases") {
        return;
    }
    for text in FIXED {
        for backend in 0..BACKENDS.len() {
            for file in 0..FILES.len() {
                for (null_mask, error_mask) in [(0, 0), (0b01, 0b10), (!0, !0)] {
                    let case = Case {
                        backend,
                        file,
                        null_mask,
                        error_mask,
                        ..Case::new(text)
                    };
                    check(&case, true).unwrap();
                }
            }
        }
    }
}

const NAMES: &[&str] = &[
    "ViewProj",
    "image",
    "def",
    "color",
    "VertInOut",
    "VSDefault",
    "PSDraw",
    "Draw",
    "x",
    "uv",
];
const TYPES: &[&str] = &[
    "float",
    "float2",
    "float4",
    "float4x4",
    "int",
    "bool",
    "texture2d",
    "string",
    "VertInOut",
    "half3",
];
const SPECIFIERS: &[&str] = &[
    "",
    "uniform ",
    "const ",
    "property ",
    "uniform const ",
    "const const ",
];
const VALUES: &[&str] = &[
    "",
    " = 1",
    " = 1.5",
    " = -2",
    " = true",
    " = maybe",
    " = \"s\"",
    " = {1, 2, 3, 4}",
    " = float4(1, 2, 3, 4)",
    " =",
];
/// Loose tokens: no `{` and no `\`.
const LOOSE: &[&str] = &[
    "x",
    "42",
    "(",
    ")",
    ",",
    "=",
    "<",
    ">",
    "[",
    "]",
    ":",
    ";",
    "\"str\"",
    "uniform",
    "const",
    "struct",
    "technique",
    "pass",
    "return",
    "}",
    "1.0",
    "-",
];

/// A body or block content: balanced, from names, loose tokens and nested
/// blocks.
fn body() -> impl Strategy<Value = String> {
    let leaf = prop::collection::vec(
        prop_oneof![
            (0..NAMES.len()).prop_map(|i| NAMES[i].to_string()),
            (0..LOOSE.len()).prop_map(|i| LOOSE[i].to_string()),
        ],
        0..6,
    )
    .prop_map(|v| v.join(" "));
    leaf.prop_recursive(2, 12, 3, |inner| {
        prop::collection::vec(
            prop_oneof![inner.clone(), inner.prop_map(|b| format!("{{ {b} }}"))],
            0..3,
        )
        .prop_map(|v| v.join(" "))
    })
}

/// One top-level item. Every `{` it holds is closed.
fn item() -> BoxedStrategy<String> {
    let name = || (0..NAMES.len()).prop_map(|i| NAMES[i]);
    let ty = || (0..TYPES.len()).prop_map(|i| TYPES[i]);
    let spec = || (0..SPECIFIERS.len()).prop_map(|i| SPECIFIERS[i]);
    prop_oneof![
        Just(";".to_string()),
        Just(" ".to_string()),
        Just("\n".to_string()),
        Just("// comment\n".to_string()),
        (
            spec(),
            ty(),
            name(),
            0..VALUES.len(),
            any::<bool>(),
            any::<bool>()
        )
            .prop_map(|(s, t, n, v, array, semi)| format!(
                "{s}{t} {n}{}{}{}",
                if array { "[2]" } else { "" },
                VALUES[v],
                if semi { ";" } else { "" }
            )),
        (
            spec(),
            ty(),
            name(),
            prop::collection::vec((ty(), name()), 0..3),
            any::<bool>(),
            body()
        )
            .prop_map(|(s, t, n, params, mapped, b)| {
                let params: Vec<String> = params.iter().map(|(t, n)| format!("{t} {n}")).collect();
                format!(
                    "{s}{t} {n}({}){} {{ {b} }}",
                    params.join(", "),
                    if mapped { " : TARGET" } else { "" }
                )
            }),
        (
            name(),
            prop::collection::vec((ty(), name(), any::<bool>()), 0..3)
        )
            .prop_map(|(n, vars)| {
                let vars: String = vars
                    .iter()
                    .map(|(t, v, m)| format!("{t} {v}{}; ", if *m { " : POSITION" } else { "" }))
                    .collect();
                format!("struct {n} {{ {vars}}};")
            }),
        (name(), prop::collection::vec((name(), name()), 0..3)).prop_map(|(n, items)| {
            let items: String = items.iter().map(|(s, v)| format!("{s} = {v}; ")).collect();
            format!("sampler_state {n} {{ {items}}};")
        }),
        (name(), prop::collection::vec((name(), name()), 0..3)).prop_map(|(n, passes)| {
            let passes: String = passes
                .iter()
                .map(|(v, p)| format!("pass {{ vertex_shader = {v}(x); pixel_shader = {p}(x); }} "))
                .collect();
            format!("technique {n} {{ {passes}}}")
        }),
        body().prop_map(|b| format!("{{ {b} }}")),
        Just("#error stop\n".to_string()),
        (0..LOOSE.len()).prop_map(|i| LOOSE[i].to_string()),
    ]
    .boxed()
}

/// A text: items, some in an `#ifdef` of a backend name with its `#else`.
fn text() -> impl Strategy<Value = String> {
    let items = || prop::collection::vec(item(), 0..6).prop_map(|v| v.join(" "));
    prop::collection::vec(
        prop_oneof![
            3 => items(),
            1 => (0..BACKENDS.len(), items(), items()).prop_map(|(b, a, e)| format!(
                "\n#ifdef {}\n{a}\n#else\n{e}\n#endif\n",
                BACKENDS[b].unwrap_or("NONE")
            )),
        ],
        0..4,
    )
    .prop_map(|v| v.join(" "))
}

fn case() -> impl Strategy<Value = Case> {
    (
        text(),
        0..BACKENDS.len(),
        0..FILES.len(),
        any::<u8>(),
        any::<u8>(),
    )
        .prop_map(|(text, backend, file, null_mask, error_mask)| Case {
            text,
            backend,
            file,
            null_mask: u32::from(null_mask),
            error_mask: u32::from(error_mask),
        })
}

#[test]
fn random_texts() {
    let config = ProptestConfig::with_cases(obs_c_oracle::parity::cases(400));
    let mut runner = proptest::test_runner::TestRunner::new(config);
    if let Err(e) = runner.run(&case(), |case| check(&case, false)) {
        panic!("{e}");
    }
}
