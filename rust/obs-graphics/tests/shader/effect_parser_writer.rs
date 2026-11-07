//! Tier 1: the effect parser's shader writer of
//! `libobs/graphics/effect-parser-writer.c.h`, in
//! `obs_graphics::effect_parser::writer`.
//!
//! `params`, `blocks`, `funcs`, `mains`, `calls` and `preset` port
//! `test/cmocka/test_effect_parser_writer.c` one to one: the same cases, in
//! the same groups, with the same expected text. That test parses each
//! effect text with `ep_parse()` and then writes each pass's vertex and
//! pixel call; these start from the parser `ep_parse()` leaves for that
//! text (its params, structs, samplers and functions, each function's body
//! and dependency lists as the function parser keeps them, and each pass's
//! call tokens), written out below as the C keeps them, and expect the C
//! test's text without its last line: the parse errors and leaks it counts
//! are `ep_parse()`'s, which stays C. The dump is the C test's: each
//! shader line behind `| `, a tab as `\t`, the used params, and the items
//! whose `written` flag is set after the call.
//!
//! One intentional difference (#3768): where `main()` takes two or more
//! mapped params, the C writes them with nothing between them, which the C
//! test pins in `mains_cases[0]` and `[1]`; the port puts `", "` between
//! them, so those two expectations here have the comma.
//!
//! The `edge_*` tests add what the C test leaves out.

use obs_graphics::effect_parser::writer::{
    CallToken, Deps, ShaderOut, ShaderSource, ShaderString, VarType, VarsOf, WriterFunc,
    WriterParam, WriterParser, WriterSampler, WriterStruct, WriterVar, find_func, find_func_token,
    find_param, find_param_token, find_sampler, find_struct, make_shader_string,
};

use std::cell::RefCell;
use std::rc::Rc;

use VarType::{In, InOut, Out, Uniform};

/// How a param is declared.
#[derive(Clone, Copy)]
enum Decl {
    Const,
    Uni,
    Plain,
}
use Decl::{Const, Plain, Uni};

fn v(
    var_type: VarType,
    type_name: &'static str,
    name: &'static str,
    mapping: Option<&'static str>,
) -> WriterVar<'static> {
    WriterVar {
        type_name: type_name.as_bytes(),
        name: name.as_bytes(),
        mapping: mapping.map(str::as_bytes),
        var_type,
    }
}

/// A function as the function parser keeps it.
struct Func<'v> {
    ret: &'static str,
    name: &'static str,
    mapping: Option<&'static str>,
    vars: &'v [WriterVar<'static>],
    body: &'static str,
    funcs: &'static [&'static str],
    structs: &'static [&'static str],
    params: &'static [&'static str],
    samplers: &'static [&'static str],
}

fn names(list: &[&'static str]) -> Vec<&'static [u8]> {
    list.iter().map(|s| s.as_bytes()).collect()
}

/// The parser, item by item, in `ep_parse()`'s order.
#[derive(Default)]
struct Src(ShaderSource<'static>);

impl Src {
    fn param(&mut self, decl: Decl, type_name: &'static str, name: &'static str, count: i32) {
        self.0.params.push(WriterParam {
            type_name: type_name.as_bytes(),
            name: name.as_bytes(),
            is_const: matches!(decl, Const),
            is_uniform: matches!(decl, Uni),
            array_count: count,
            written: false,
        });
    }

    fn strukt(&mut self, name: &'static str, vars: &[WriterVar<'static>]) {
        self.0.structs.push(WriterStruct {
            name: name.as_bytes(),
            vars: vars.to_vec(),
            written: false,
        });
    }

    fn sampler(&mut self, name: &'static str, items: &[(&'static str, &'static str)]) {
        self.0.samplers.push(WriterSampler {
            name: name.as_bytes(),
            states: items.iter().map(|(s, _)| s.as_bytes()).collect(),
            values: items.iter().map(|(_, v)| v.as_bytes()).collect(),
            written: false,
        });
    }

    fn func(&mut self, f: Func<'_>) {
        self.0.funcs.push(WriterFunc {
            name: f.name.as_bytes(),
            ret_type: f.ret.as_bytes(),
            mapping: f.mapping.map(str::as_bytes),
            contents: f.body.as_bytes(),
            param_vars: f.vars.to_vec(),
            func_deps: names(f.funcs),
            struct_deps: names(f.structs),
            param_deps: names(f.params),
            sampler_deps: names(f.samplers),
            written: false,
        });
    }
}

type Call = Option<&'static [&'static str]>;

/// A case: the parser, and each pass's label with its two calls.
struct Case {
    src: Src,
    passes: Vec<(&'static str, Call, Call)>,
}

/// The tokens of a call as the pass parser keeps them: each token's text,
/// the first byte the one tested for whitespace.
fn tokens(call: &[&'static str]) -> Vec<CallToken<'static>> {
    call.iter()
        .map(|t| CallToken {
            text: t.as_bytes(),
            lead: t.as_bytes().first().copied().unwrap_or(0),
        })
        .collect()
}

fn dump_shader(text: &[u8], out: &mut String) {
    let mut line_start = true;
    for &c in text {
        if line_start {
            out.push_str("    | ");
            line_start = false;
        }
        match c {
            b'\n' => {
                out.push('\n');
                line_start = true;
            }
            b'\t' => out.push_str("\\t"),
            _ => out.push(char::from(c)),
        }
    }
    if !line_start {
        out.push_str("\n    (no final newline)\n");
    }
}

fn joined<'a>(items: impl Iterator<Item = &'a [u8]>) -> String {
    items
        .map(|n| String::from_utf8_lossy(n).into_owned())
        .collect::<Vec<_>>()
        .join(",")
}

fn dump_written(src: &ShaderSource<'_>, out: &mut String) {
    out.push_str(&format!(
        "    written param={} struct={} func={} sampler={}\n",
        joined(src.params.iter().filter(|p| p.written).map(|p| p.name)),
        joined(src.structs.iter().filter(|s| s.written).map(|s| s.name)),
        joined(src.funcs.iter().filter(|f| f.written).map(|f| f.name)),
        joined(src.samplers.iter().filter(|s| s.written).map(|s| s.name)),
    ));
}

fn set_all_written(src: &mut ShaderSource<'_>) {
    src.params.iter_mut().for_each(|p| p.written = true);
    src.structs.iter_mut().for_each(|s| s.written = true);
    src.funcs.iter_mut().for_each(|f| f.written = true);
    src.samplers.iter_mut().for_each(|s| s.written = true);
}

/// `write_call`: one call into an empty shader string, dumped.
fn write_call(src: &mut ShaderSource<'_>, label: &str, call: Call, preset: bool, out: &mut String) {
    if preset {
        set_all_written(src);
    }
    let call = call.map(tokens);
    let mut shader = ShaderString::default();
    make_shader_string(src, &mut shader, call.as_deref());

    out.push_str(&format!("  {label}\n"));
    dump_shader(&shader.text, out);
    out.push_str(&format!(
        "    used {}\n",
        joined(shader.used.iter().map(Vec::as_slice))
    ));
    dump_written(src, out);
}

/// `write_passes`, on one parser across every pass.
fn dump(case: Case, preset: bool) -> String {
    let mut src = case.src.0;
    let mut out = String::new();
    for (label, vertex, pixel) in case.passes {
        out.push_str(&format!("{label}\n"));
        write_call(&mut src, "vertex", vertex, preset, &mut out);
        write_call(&mut src, "pixel", pixel, preset, &mut out);
    }
    out
}

/// A case of the C test, built.
type CaseFn = fn() -> Case;

/// `check_cases`.
fn check(cases: &[(CaseFn, &str)], preset: bool) {
    for (i, (case, expected)) in cases.iter().enumerate() {
        assert_eq!(dump(case(), preset), *expected, "case {i}");
    }
}

#[test]
fn params() {
    check(
        &[
            (params_cases_0, PARAMS_0),
            (params_cases_1, PARAMS_1),
            (params_cases_2, PARAMS_2),
        ],
        false,
    );
}

#[test]
fn blocks() {
    check(
        &[(blocks_cases_0, BLOCKS_0), (blocks_cases_1, BLOCKS_1)],
        false,
    );
}

#[test]
fn funcs() {
    check(&[(funcs_cases_0, FUNCS_0), (funcs_cases_1, FUNCS_1)], false);
}

#[test]
fn mains() {
    check(
        &[
            (mains_cases_0, MAINS_0),
            (mains_cases_1, MAINS_1),
            (mains_cases_2, MAINS_2),
        ],
        false,
    );
}

#[test]
fn calls() {
    check(
        &[
            (calls_cases_0, CALLS_0),
            (calls_cases_1, CALLS_1),
            (calls_cases_2, CALLS_2),
            (calls_cases_3, CALLS_3),
        ],
        false,
    );
}

#[test]
fn preset() {
    check(&[(preset_cases_0, PRESET_0)], true);
}

/// One call on `src`, with each token's first byte as its lead.
fn write(src: &mut ShaderSource<'static>, call: Option<&[&'static str]>) -> ShaderString {
    let call = call.map(tokens);
    let mut out = ShaderString::default();
    make_shader_string(src, &mut out, call.as_deref());
    out
}

fn text(out: &ShaderString) -> String {
    String::from_utf8_lossy(&out.text).into_owned()
}

fn simple(body: &'static str, vars: &[WriterVar<'static>]) -> Src {
    let mut s = Src::default();
    s.param(Uni, "float4", "tint", 0);
    s.func(Func {
        ret: "float4",
        name: "PS",
        mapping: Some("TARGET"),
        vars,
        body,
        funcs: &[],
        structs: &[],
        params: &[],
        samplers: &[],
    });
    s
}

#[test]
fn edge_calls_write_nothing() {
    // No token array, no tokens, whitespace only, or NULL after whitespace:
    // nothing written, and the flags as they were.
    for call in [
        None,
        Some(&[][..]),
        Some(&[" ", "\t", "\n", "\r"][..]),
        Some(&[" ", "NULL", "(", ")"][..]),
    ] {
        let mut src = simple("{ return tint; }", &[]).0;
        src.params[0].written = true;
        let out = write(&mut src, call);
        assert_eq!(out, ShaderString::default());
        assert!(src.params[0].written);
    }
}

#[test]
fn edge_leading_whitespace_is_skipped() {
    // Leading whitespace tokens are not part of the call string; later
    // ones are.
    let mut src = simple("{ return tint; }", &[]).0;
    let out = write(&mut src, Some(&[" ", "\n", "PS", "(", " ", "tint", ")"]));
    assert_eq!(
        text(&out),
        "uniform float4 tint;\nfloat4 PS()\n{ return tint; }\n\n\
         float4 main() : TARGET\n{\n\treturn PS( tint)\n}\n"
    );
    assert_eq!(out.used, [b"tint".to_vec()]);
}

#[test]
fn edge_lead_is_the_byte_tested() {
    // The C tests the token's first byte as it reads it, which for an
    // empty token is the lexer's next byte.
    let mut src = simple("{ }", &[]).0;
    let call = [
        CallToken {
            text: b"",
            lead: b' ',
        },
        CallToken {
            text: b"PS",
            lead: b'P',
        },
    ];
    let mut out = ShaderString::default();
    make_shader_string(&mut src, &mut out, Some(&call[..]));
    assert_eq!(
        text(&out),
        "float4 PS()\n{ }\n\nfloat4 main() : TARGET\n{\n\treturn PS\n}\n"
    );

    // An empty first token whose next byte is not whitespace is the
    // function's name: no function has it, and NULL is not it.
    let call = [
        CallToken {
            text: b"",
            lead: b'P',
        },
        CallToken {
            text: b"PS",
            lead: b'P',
        },
    ];
    let mut out = ShaderString::default();
    make_shader_string(&mut src, &mut out, Some(&call[..]));
    assert_eq!(out, ShaderString::default());
}

#[test]
fn edge_null_is_the_whole_token() {
    // strref_cmp(token, "NULL"): "NULLX" or "NUL" is a name like any other.
    for name in ["NULLX", "NUL"] {
        let mut s = Src::default();
        s.func(Func {
            ret: "void",
            name,
            mapping: None,
            vars: &[],
            body: "{}",
            funcs: &[],
            structs: &[],
            params: &[],
            samplers: &[],
        });
        let mut src = s.0;
        let out = write(&mut src, Some(&[name]));
        assert_eq!(
            text(&out),
            format!("void {name}()\n{{}}\n\nvoid main()\n{{\n\treturn {name}\n}}\n")
        );
    }
}

#[test]
fn edge_missing_dependencies_are_skipped() {
    // The parser records only names it found, so the C never meets a
    // missing one (it would crash). The port skips it: the param's line
    // break pair stays (it counts the names), a sampler's line break goes
    // with it.
    let mut s = Src::default();
    s.func(Func {
        ret: "float4",
        name: "PS",
        mapping: None,
        vars: &[],
        body: "{}",
        funcs: &["gone_func"],
        structs: &["gone_struct"],
        params: &["gone_param"],
        samplers: &["gone_sampler"],
    });
    let mut src = s.0;
    let out = write(&mut src, Some(&["PS"]));
    assert_eq!(
        text(&out),
        "\n\nfloat4 PS()\n{}\n\nfloat4 main()\n{\n\treturn PS\n}\n"
    );
    assert!(!src.funcs[0].written);
}

#[test]
fn edge_var_types_and_counts() {
    let mut s = Src::default();
    s.param(Plain, "float", "neg", -1);
    s.param(Uni, "float", "big", i32::MAX);
    s.func(Func {
        ret: "void",
        name: "f",
        mapping: None,
        vars: &[
            v(In, "float", "a", None),
            v(InOut, "float", "b", None),
            v(Out, "float", "c", Some("COLOR")),
            v(Uniform, "float", "d", None),
        ],
        body: "{}",
        funcs: &[],
        structs: &[],
        params: &["neg", "big"],
        samplers: &[],
    });
    let mut src = s.0;
    let out = write(&mut src, Some(&["f", "(", ")"]));
    // dstr_catf("[%u]") prints the int's bits as unsigned.
    assert_eq!(
        text(&out),
        "float neg[4294967295];\nuniform float big[2147483647];\n\n\n\
         void f(float a, inout float b, out float c : COLOR, uniform float d)\n{}\n\n\
         void main(float c : COLOR)\n{\n\treturn f()\n}\n"
    );
    assert_eq!(out.used, [b"big".to_vec()]);
}

#[test]
fn edge_body_bytes_go_in_unchanged() {
    // A body is appended whole (dstr_cat_dstr), even when it starts with a
    // NUL byte; a dstr_cat piece that starts with one appends nothing.
    let mut src = simple("\0{\x01}", &[]).0;
    let out = write(&mut src, Some(&["PS"]));
    assert_eq!(
        out.text,
        b"float4 PS()\n\0{\x01}\n\nfloat4 main() : TARGET\n{\n\treturn PS\n}\n"
    );

    let mut plain = ShaderString::default();
    plain.cat(b"\0x");
    plain.cat(b"");
    plain.cat_dstr(b"");
    assert!(plain.text.is_empty());
    plain.cat_dstr(b"\0x");
    assert_eq!(plain.text, b"\0x");
}

#[test]
fn edge_lookups_take_the_first_match() {
    let mut s = Src::default();
    s.param(Uni, "float", "a", 0);
    s.param(Const, "float", "a", 0);
    s.strukt("S", &[]);
    s.strukt("S", &[v(In, "float", "x", Some("X"))]);
    s.sampler("smp", &[]);
    s.sampler("smp", &[("Filter", "Linear")]);
    for _ in 0..2 {
        s.func(Func {
            ret: "void",
            name: "f",
            mapping: None,
            vars: &[],
            body: "{}",
            funcs: &[],
            structs: &[],
            params: &[],
            samplers: &[],
        });
    }
    let src = s.0;
    assert_eq!(find_param(&src, b"a"), Some(0));
    assert_eq!(find_param(&src, b"b"), None);
    assert_eq!(find_struct(&src, b"S"), Some(0));
    assert_eq!(find_sampler(&src, b"smp"), Some(0));
    assert_eq!(find_func(&src, b"f"), Some(0));
    assert_eq!(find_func(&src, b"F"), None);
    assert_eq!(find_func_token(&src, b"f"), Some(0));
    assert_eq!(find_param_token(&src, b"a"), Some(0));
    // strref_cmp: a token that is a prefix or extension of the name, or
    // empty, does not name it.
    assert_eq!(find_param_token(&src, b"ab"), None);
    assert_eq!(find_param_token(&src, b""), None);
    assert_eq!(find_func_token(&src, b"f\0"), Some(0));
    // An empty name is named by an empty token (strref_is_empty).
    let mut e = Src::default();
    e.param(Uni, "float", "", 0);
    assert_eq!(find_param_token(&e.0, b""), Some(0));
    assert_eq!(find_param_token(&e.0, b"\0x"), Some(0));
    assert!(!WriterStruct::default().mapped());
    assert!(src.structs[1].mapped());
    assert!(!src.structs[0].mapped());
}

#[test]
fn edge_struct_params_of_main() {
    // A param whose type is a mapped struct goes into main() with no
    // mapping of its own; one whose struct isn't mapped, or that has no
    // struct and no mapping, is dropped.
    let mut s = Src::default();
    s.strukt("In", &[v(In, "float4", "pos", Some("POSITION"))]);
    s.strukt("Bare", &[v(In, "float4", "pos", None)]);
    s.func(Func {
        ret: "float4",
        name: "PS",
        mapping: Some("TARGET"),
        vars: &[
            v(In, "In", "i", None),
            v(In, "Bare", "b", None),
            v(In, "float", "k", None),
        ],
        body: "{}",
        funcs: &[],
        structs: &[],
        params: &[],
        samplers: &[],
    });
    let mut src = s.0;
    let out = write(&mut src, Some(&["PS", "(", "i", ",", "b", ",", "k", ")"]));
    assert_eq!(
        text(&out),
        "float4 PS(In i, Bare b, float k)\n{}\n\nfloat4 main(In i) : TARGET\n{\n\treturn PS(i,b,k)\n}\n"
    );
}

#[test]
fn edge_main_params_get_commas() {
    // #3768: a comma between main()'s mapped params, none before the first
    // one written (a dropped param leaves none behind) and none after the
    // last.
    let mut s = Src::default();
    s.strukt("In", &[v(In, "float4", "pos", Some("POSITION"))]);
    s.func(Func {
        ret: "float4",
        name: "PS",
        mapping: Some("TARGET"),
        vars: &[
            v(In, "float", "k", None),
            v(In, "In", "i", None),
            v(In, "float4", "c", Some("COLOR")),
            v(In, "float", "j", None),
            v(In, "float2", "uv", Some("TEXCOORD0")),
        ],
        body: "{}",
        funcs: &[],
        structs: &[],
        params: &[],
        samplers: &[],
    });
    let mut src = s.0;
    let out = write(&mut src, Some(&["PS"]));
    assert!(
        text(&out)
            .contains("\nfloat4 main(In i, float4 c : COLOR, float2 uv : TEXCOORD0) : TARGET\n"),
        "{}",
        text(&out)
    );
}

// params_cases[0]
fn params_cases_0() -> Case {
    let mut s = Src::default();
    s.param(Uni, "float4x4", "ViewProj", 0);
    s.param(Uni, "float4", "colors", 4);
    s.param(Const, "float", "scale", 0);
    s.param(Plain, "float", "plain", 0);
    s.param(Uni, "texture2d", "image", 0);
    s.func(Func {
        ret: "float4",
        name: "VSMain",
        mapping: Some("POSITION"),
        vars: &[v(In, "float4", "pos", Some("POSITION"))],
        body: "{\n\treturn mul(pos * scale, ViewProj) + colors[0] * plain;\n}",
        funcs: &[],
        structs: &[],
        params: &["scale", "ViewProj", "colors", "plain"],
        samplers: &[],
    });
    s.func(Func {
        ret: "float4",
        name: "PSMain",
        mapping: Some("TARGET"),
        vars: &[v(In, "float4", "pos", Some("POSITION"))],
        body: "{\n\treturn colors[1];\n}",
        funcs: &[],
        structs: &[],
        params: &["colors"],
        samplers: &[],
    });
    Case {
        src: s,
        passes: vec![(
            "Draw/0",
            Some(&["VSMain", "(", "pos", ")", ";"][..]),
            Some(&["PSMain", "(", "pos", ")", ";"][..]),
        )],
    }
}

// params_cases[1]
fn params_cases_1() -> Case {
    let mut s = Src::default();
    s.param(Uni, "float4", "tint", 0);
    s.param(Const, "float", "half_gain", 0);
    s.param(Uni, "float", "unused", 0);
    s.func(Func {
        ret: "float4",
        name: "PSTint",
        mapping: Some("TARGET"),
        vars: &[
            v(In, "float4", "pos", Some("POSITION")),
            v(In, "float4", "t", None),
            v(In, "float", "g", None),
        ],
        body: "{\n\treturn pos * t * g * tint;\n}",
        funcs: &[],
        structs: &[],
        params: &["tint"],
        samplers: &[],
    });
    Case {
        src: s,
        passes: vec![(
            "Draw/0",
            Some(
                &[
                    "PSTint",
                    "(",
                    "pos",
                    ",",
                    "tint",
                    ",",
                    "half_gain",
                    ")",
                    ";",
                ][..],
            ),
            Some(&["PSTint", "(", "pos", ",", "tint", ",", "tint", ")", ";"][..]),
        )],
    }
}

// params_cases[2]
fn params_cases_2() -> Case {
    let mut s = Src::default();
    s.param(Uni, "float4", "one", 1);
    s.param(Uni, "float", "a", 0);
    s.param(Uni, "float4", "a", 0);
    s.param(Const, "float", "b", 0);
    s.param(Uni, "float", "b", 0);
    s.func(Func {
        ret: "float4",
        name: "f",
        mapping: None,
        vars: &[v(In, "float4", "c", None)],
        body: "{ return c * a * b; }",
        funcs: &[],
        structs: &[],
        params: &["a", "b"],
        samplers: &[],
    });
    s.func(Func {
        ret: "float4",
        name: "f",
        mapping: None,
        vars: &[v(In, "float4", "c", None)],
        body: "{ return c; }",
        funcs: &[],
        structs: &[],
        params: &[],
        samplers: &[],
    });
    s.func(Func {
        ret: "float4",
        name: "PS",
        mapping: Some("TARGET"),
        vars: &[v(In, "float4", "c", Some("COLOR"))],
        body: "{ return f(c) + one[0]; }",
        funcs: &["f"],
        structs: &[],
        params: &["one"],
        samplers: &[],
    });
    Case {
        src: s,
        passes: vec![(
            "Draw/0",
            Some(&["f", "(", "c", ",", "b", ")", ";"][..]),
            Some(&["PS", "(", "c", ")", ";"][..]),
        )],
    }
}

// blocks_cases[0]
fn blocks_cases_0() -> Case {
    let mut s = Src::default();
    s.param(Uni, "float4x4", "ViewProj", 0);
    s.param(Uni, "texture2d", "image", 0);
    s.strukt(
        "VertInOut",
        &[
            v(In, "float4", "pos", Some("POSITION")),
            v(In, "float2", "uv", Some("TEXCOORD0")),
        ],
    );
    s.strukt(
        "Light",
        &[
            v(In, "float3", "dir", None),
            v(In, "float", "strength", None),
        ],
    );
    s.sampler(
        "def_sampler",
        &[
            ("Filter", "Linear"),
            ("AddressU", "Clamp"),
            ("AddressV", "Wrap"),
        ],
    );
    s.func(Func {
        ret: "VertInOut",
        name: "VSDefault",
        mapping: None,
        vars: &[v(In, "VertInOut", "vert_in", None)],
        body: "{\n\tVertInOut vert_out;\n\tvert_out.pos = mul(float4(vert_in.pos.xyz, 1.0), ViewProj);\n\tvert_out.uv = vert_in.uv;\n\treturn vert_out;\n}",
        funcs: &[],
        structs: &["VertInOut", "VertInOut", "VertInOut"],
        params: &["ViewProj"],
        samplers: &[],
    });
    s.func(Func {
        ret: "float4",
        name: "PSLit",
        mapping: Some("TARGET"),
        vars: &[v(In, "VertInOut", "vert_in", None)],
        body: "{\n\tLight l;\n\tl.strength = 0.5;\n\treturn image.Sample(def_sampler, vert_in.uv) * l.strength;\n}",
        funcs: &[],
        structs: &["VertInOut", "Light"],
        params: &["image"],
        samplers: &["def_sampler"],
    });
    s.func(Func {
        ret: "float4",
        name: "PSLight",
        mapping: Some("TARGET"),
        vars: &[v(In, "Light", "l", None)],
        body: "{\n\treturn float4(l.dir, l.strength);\n}",
        funcs: &[],
        structs: &["Light"],
        params: &[],
        samplers: &[],
    });
    Case {
        src: s,
        passes: vec![
            (
                "Draw/0",
                Some(&["VSDefault", "(", "vert_in", ")", ";"][..]),
                Some(&["PSLit", "(", "vert_in", ")", ";"][..]),
            ),
            (
                "Unmapped/0",
                Some(&["VSDefault", "(", "vert_in", ")", ";"][..]),
                Some(&["PSLight", "(", "l", ")", ";"][..]),
            ),
        ],
    }
}

// blocks_cases[1]
fn blocks_cases_1() -> Case {
    let mut s = Src::default();
    s.param(Uni, "texture2d", "image", 0);
    s.strukt(
        "Late",
        &[
            v(In, "float2", "uv", None),
            v(In, "float4", "pos", Some("POSITION")),
        ],
    );
    s.sampler("plain_sampler", &[]);
    s.sampler("point_sampler", &[("Filter", "Point")]);
    s.func(Func {
        ret: "float4",
        name: "PSTwo",
        mapping: Some("TARGET"),
        vars: &[v(In, "Late", "v", None)],
        body: "{\n\treturn image.Sample(plain_sampler, v.uv) + image.Sample(point_sampler, v.uv) + image.Sample(plain_sampler, v.uv);\n}",
        funcs: &[],
        structs: &["Late"],
        params: &["image", "image", "image"],
        samplers: &["plain_sampler", "point_sampler", "plain_sampler"],
    });
    Case {
        src: s,
        passes: vec![("Draw/0", None, Some(&["PSTwo", "(", "v", ")", ";"][..]))],
    }
}

// funcs_cases[0]
fn funcs_cases_0() -> Case {
    let mut s = Src::default();
    s.param(Uni, "float", "gain", 0);
    s.func(Func {
        ret: "float4",
        name: "leaf",
        mapping: None,
        vars: &[v(In, "float4", "c", None)],
        body: "{ return c * gain; }",
        funcs: &[],
        structs: &[],
        params: &["gain"],
        samplers: &[],
    });
    s.func(Func {
        ret: "float4",
        name: "mid",
        mapping: None,
        vars: &[v(In, "float4", "c", None)],
        body: "{ return leaf(c) + leaf(c); }",
        funcs: &["leaf", "leaf"],
        structs: &[],
        params: &[],
        samplers: &[],
    });
    s.func(Func {
        ret: "float4",
        name: "other",
        mapping: None,
        vars: &[v(In, "float4", "c", None)],
        body: "{ return leaf(c); }",
        funcs: &["leaf"],
        structs: &[],
        params: &[],
        samplers: &[],
    });
    s.func(Func {
        ret: "float4",
        name: "top",
        mapping: None,
        vars: &[v(In, "float4", "c", None)],
        body: "{ return mid(c) + other(c); }",
        funcs: &["mid", "other"],
        structs: &[],
        params: &[],
        samplers: &[],
    });
    s.func(Func {
        ret: "float4",
        name: "PSMain",
        mapping: Some("TARGET"),
        vars: &[v(In, "float4", "pos", Some("POSITION"))],
        body: "{ return top(pos); }",
        funcs: &["top"],
        structs: &[],
        params: &[],
        samplers: &[],
    });
    Case {
        src: s,
        passes: vec![(
            "Draw/0",
            Some(&["top", "(", "pos", ")", ";"][..]),
            Some(&["PSMain", "(", "pos", ")", ";"][..]),
        )],
    }
}

// funcs_cases[1]
fn funcs_cases_1() -> Case {
    let mut s = Src::default();
    s.func(Func {
        ret: "float4",
        name: "rec",
        mapping: None,
        vars: &[v(In, "float4", "c", None)],
        body: "{ return rec(c); }",
        funcs: &[],
        structs: &[],
        params: &[],
        samplers: &[],
    });
    s.func(Func {
        ret: "void",
        name: "adjust",
        mapping: None,
        vars: &[
            v(InOut, "float4", "c", None),
            v(Out, "float", "a", None),
            v(Uniform, "float", "k", None),
            v(In, "float", "b", None),
        ],
        body: "{ a = b * k; c *= a; }",
        funcs: &[],
        structs: &[],
        params: &[],
        samplers: &[],
    });
    s.func(Func {
        ret: "float4",
        name: "PSAdjust",
        mapping: Some("TARGET"),
        vars: &[v(In, "float4", "c", Some("COLOR"))],
        body: "{\n\tfloat a;\n\tadjust(c, a, 1.0, 2.0);\n\treturn rec(c) * a;\n}",
        funcs: &["adjust", "rec"],
        structs: &[],
        params: &[],
        samplers: &[],
    });
    Case {
        src: s,
        passes: vec![(
            "Draw/0",
            Some(&["rec", "(", "c", ")", ";"][..]),
            Some(&["PSAdjust", "(", "c", ")", ";"][..]),
        )],
    }
}

// mains_cases[0]
fn mains_cases_0() -> Case {
    let mut s = Src::default();
    s.func(Func {
        ret: "float4",
        name: "PSTwo",
        mapping: Some("TARGET"),
        vars: &[
            v(In, "float4", "pos", Some("POSITION")),
            v(In, "float2", "uv", Some("TEXCOORD0")),
        ],
        body: "{\n\treturn float4(uv, pos.zw);\n}",
        funcs: &[],
        structs: &[],
        params: &[],
        samplers: &[],
    });
    Case {
        src: s,
        passes: vec![(
            "Draw/0",
            Some(&["PSTwo", "(", "pos", ",", "uv", ")", ";"][..]),
            Some(&["PSTwo", "(", "pos", ",", "uv", ")", ";"][..]),
        )],
    }
}

// mains_cases[1]
fn mains_cases_1() -> Case {
    let mut s = Src::default();
    s.strukt(
        "VertInOut",
        &[
            v(In, "float4", "pos", Some("POSITION")),
            v(In, "float2", "uv", Some("TEXCOORD0")),
        ],
    );
    s.func(Func {
        ret: "float4",
        name: "PSMix",
        mapping: Some("TARGET"),
        vars: &[
            v(In, "float4", "pos", Some("POSITION")),
            v(In, "float", "k", None),
            v(In, "float2", "uv", Some("TEXCOORD0")),
        ],
        body: "{\n\treturn pos * k + float4(uv, 0.0, 0.0);\n}",
        funcs: &[],
        structs: &[],
        params: &[],
        samplers: &[],
    });
    s.func(Func {
        ret: "float4",
        name: "PSBoth",
        mapping: Some("TARGET"),
        vars: &[
            v(In, "VertInOut", "v", None),
            v(In, "float4", "c", Some("COLOR")),
        ],
        body: "{\n\treturn v.pos * c;\n}",
        funcs: &[],
        structs: &["VertInOut"],
        params: &[],
        samplers: &[],
    });
    Case {
        src: s,
        passes: vec![(
            "Draw/0",
            Some(&["PSMix", "(", "pos", ",", "k", ",", "uv", ")", ";"][..]),
            Some(&["PSBoth", "(", "v", ",", "c", ")", ";"][..]),
        )],
    }
}

// mains_cases[2]
fn mains_cases_2() -> Case {
    let mut s = Src::default();
    s.func(Func {
        ret: "float4",
        name: "PSNone",
        mapping: Some("TARGET"),
        vars: &[],
        body: "{\n\treturn float4(1.0, 1.0, 1.0, 1.0);\n}",
        funcs: &[],
        structs: &[],
        params: &[],
        samplers: &[],
    });
    s.func(Func {
        ret: "float4",
        name: "VSNoMap",
        mapping: None,
        vars: &[v(In, "float4", "pos", Some("POSITION"))],
        body: "{\n\treturn pos;\n}",
        funcs: &[],
        structs: &[],
        params: &[],
        samplers: &[],
    });
    Case {
        src: s,
        passes: vec![(
            "Draw/0",
            Some(&["VSNoMap", "(", "pos", ")", ";"][..]),
            Some(&["PSNone", "(", ")", ";"][..]),
        )],
    }
}

// calls_cases[0]
fn calls_cases_0() -> Case {
    let mut s = Src::default();
    s.param(Uni, "float4", "tint", 0);
    s.func(Func {
        ret: "float4",
        name: "VSMain",
        mapping: Some("POSITION"),
        vars: &[v(In, "float4", "pos", Some("POSITION"))],
        body: "{ return pos * tint; }",
        funcs: &[],
        structs: &[],
        params: &["tint"],
        samplers: &[],
    });
    Case {
        src: s,
        passes: vec![
            (
                "Draw/0",
                Some(&["VSMain", "(", "pos", ")", ";"][..]),
                Some(&["NULL", ";"][..]),
            ),
            (
                "Vertex/0",
                Some(&["VSMain", "(", "pos", ")", ";"][..]),
                None,
            ),
        ],
    }
}

// calls_cases[1]
fn calls_cases_1() -> Case {
    let mut s = Src::default();
    s.param(Uni, "float4", "tint", 0);
    s.func(Func {
        ret: "float4",
        name: "VSMain",
        mapping: Some("POSITION"),
        vars: &[v(In, "float4", "pos", Some("POSITION"))],
        body: "{ return pos * tint; }",
        funcs: &[],
        structs: &[],
        params: &["tint"],
        samplers: &[],
    });
    Case {
        src: s,
        passes: vec![(
            "Draw/0",
            Some(&["Missing", "(", "pos", ",", "tint", ")", ";"][..]),
            Some(&["tint", ";"][..]),
        )],
    }
}

// calls_cases[2]
fn calls_cases_2() -> Case {
    let mut s = Src::default();
    s.param(Uni, "float4", "tint", 0);
    s.func(Func {
        ret: "float4",
        name: "VSMain",
        mapping: Some("POSITION"),
        vars: &[
            v(In, "float4", "pos", Some("POSITION")),
            v(In, "float4", "t", None),
        ],
        body: "{ return pos * t; }",
        funcs: &[],
        structs: &[],
        params: &[],
        samplers: &[],
    });
    Case {
        src: s,
        passes: vec![(
            "Draw/0",
            Some(&["VSMain", "(", "pos", ",", "tint", ")", ";"][..]),
            Some(&["VSMain", "(", "pos", ",", "tint", ")", ";"][..]),
        )],
    }
}

// calls_cases[3]
fn calls_cases_3() -> Case {
    let mut s = Src::default();
    s.param(Uni, "float4x4", "ViewProj", 0);
    s.strukt(
        "VertInOut",
        &[
            v(In, "float4", "pos", Some("POSITION")),
            v(In, "float2", "uv", Some("TEXCOORD0")),
        ],
    );
    s.func(Func {
        ret: "VertInOut",
        name: "VSDefault",
        mapping: None,
        vars: &[v(In, "VertInOut", "v", None)],
        body: "{ v.pos = mul(v.pos, ViewProj); return v; }",
        funcs: &[],
        structs: &["VertInOut", "VertInOut"],
        params: &["ViewProj"],
        samplers: &[],
    });
    Case {
        src: s,
        passes: vec![
            (
                "Draw/0",
                Some(&["VSDefault", "(", "v", ")", ";"][..]),
                Some(&["VSDefault", "(", "v", ")", ";"][..]),
            ),
            (
                "Draw/1",
                Some(&["VSDefault", "(", "v", ")", ";"][..]),
                Some(&["NULL", ";"][..]),
            ),
        ],
    }
}

// preset_cases[0]
fn preset_cases_0() -> Case {
    let mut s = Src::default();
    s.param(Uni, "float4x4", "ViewProj", 0);
    s.param(Uni, "texture2d", "image", 0);
    s.strukt(
        "VertInOut",
        &[
            v(In, "float4", "pos", Some("POSITION")),
            v(In, "float2", "uv", Some("TEXCOORD0")),
        ],
    );
    s.sampler("def_sampler", &[("Filter", "Linear")]);
    s.func(Func {
        ret: "float4",
        name: "sample_at",
        mapping: None,
        vars: &[v(In, "float2", "uv", None)],
        body: "{ return image.Sample(def_sampler, uv); }",
        funcs: &[],
        structs: &[],
        params: &["image"],
        samplers: &["def_sampler"],
    });
    s.func(Func {
        ret: "VertInOut",
        name: "VSDefault",
        mapping: None,
        vars: &[v(In, "VertInOut", "v", None)],
        body: "{ v.pos = mul(v.pos, ViewProj); return v; }",
        funcs: &[],
        structs: &["VertInOut", "VertInOut"],
        params: &["ViewProj"],
        samplers: &[],
    });
    s.func(Func {
        ret: "float4",
        name: "PSDraw",
        mapping: Some("TARGET"),
        vars: &[v(In, "VertInOut", "v", None)],
        body: "{ return sample_at(v.uv); }",
        funcs: &["sample_at"],
        structs: &["VertInOut"],
        params: &[],
        samplers: &[],
    });
    Case {
        src: s,
        passes: vec![
            (
                "Draw/0",
                Some(&["VSDefault", "(", "v", ")", ";"][..]),
                Some(&["PSDraw", "(", "v", ")", ";"][..]),
            ),
            (
                "Null/0",
                Some(&["VSDefault", "(", "ViewProj", ")", ";"][..]),
                Some(&["NULL", ";"][..]),
            ),
            (
                "Vertex/0",
                Some(&["VSDefault", "(", "v", ")", ";"][..]),
                None,
            ),
        ],
    }
}

/// `params_cases[0]`: const, uniform, array, plain and texture params named in two functions' bodies; a const param is declared without its value
const PARAMS_0: &str = concat!(
    "Draw/0\n",
    "  vertex\n",
    "    | const float scale;\n",
    "    | uniform float4x4 ViewProj;\n",
    "    | uniform float4 colors[4];\n",
    "    | float plain;\n",
    "    | \n",
    "    | \n",
    "    | float4 VSMain(float4 pos : POSITION)\n",
    "    | {\n",
    "    | \\treturn mul(pos * scale, ViewProj) + colors[0] * plain;\n",
    "    | }\n",
    "    | \n",
    "    | float4 main(float4 pos : POSITION) : POSITION\n",
    "    | {\n",
    "    | \\treturn VSMain(pos);\n",
    "    | }\n",
    "    used ViewProj,colors\n",
    "    written param= struct= func= sampler=\n",
    "  pixel\n",
    "    | uniform float4 colors[4];\n",
    "    | \n",
    "    | \n",
    "    | float4 PSMain(float4 pos : POSITION)\n",
    "    | {\n",
    "    | \\treturn colors[1];\n",
    "    | }\n",
    "    | \n",
    "    | float4 main(float4 pos : POSITION) : TARGET\n",
    "    | {\n",
    "    | \\treturn PSMain(pos);\n",
    "    | }\n",
    "    used colors\n",
    "    written param= struct= func= sampler=\n",
);

/// `params_cases[1]`: params named by the call: written before the function, once each, and not again as its dependencies
const PARAMS_1: &str = concat!(
    "Draw/0\n",
    "  vertex\n",
    "    | uniform float4 tint;\n",
    "    | const float half_gain;\n",
    "    | \n",
    "    | \n",
    "    | float4 PSTint(float4 pos : POSITION, float4 t, float g)\n",
    "    | {\n",
    "    | \\treturn pos * t * g * tint;\n",
    "    | }\n",
    "    | \n",
    "    | float4 main(float4 pos : POSITION) : TARGET\n",
    "    | {\n",
    "    | \\treturn PSTint(pos,tint,half_gain);\n",
    "    | }\n",
    "    used tint\n",
    "    written param= struct= func= sampler=\n",
    "  pixel\n",
    "    | uniform float4 tint;\n",
    "    | \n",
    "    | \n",
    "    | float4 PSTint(float4 pos : POSITION, float4 t, float g)\n",
    "    | {\n",
    "    | \\treturn pos * t * g * tint;\n",
    "    | }\n",
    "    | \n",
    "    | float4 main(float4 pos : POSITION) : TARGET\n",
    "    | {\n",
    "    | \\treturn PSTint(pos,tint,tint);\n",
    "    | }\n",
    "    used tint\n",
    "    written param= struct= func= sampler=\n",
);

/// `params_cases[2]`: a name declared twice: the first param or function of that name is the one written; and a one-element array keeps its [1]
const PARAMS_2: &str = concat!(
    "Draw/0\n",
    "  vertex\n",
    "    | const float b;\n",
    "    | uniform float a;\n",
    "    | \n",
    "    | \n",
    "    | float4 f(float4 c)\n",
    "    | { return c * a * b; }\n",
    "    | \n",
    "    | float4 main()\n",
    "    | {\n",
    "    | \\treturn f(c,b);\n",
    "    | }\n",
    "    used a\n",
    "    written param= struct= func= sampler=\n",
    "  pixel\n",
    "    | uniform float4 one[1];\n",
    "    | \n",
    "    | \n",
    "    | uniform float a;\n",
    "    | const float b;\n",
    "    | \n",
    "    | \n",
    "    | float4 f(float4 c)\n",
    "    | { return c * a * b; }\n",
    "    | \n",
    "    | \n",
    "    | float4 PS(float4 c : COLOR)\n",
    "    | { return f(c) + one[0]; }\n",
    "    | \n",
    "    | float4 main(float4 c : COLOR) : TARGET\n",
    "    | {\n",
    "    | \\treturn PS(c);\n",
    "    | }\n",
    "    used one,a\n",
    "    written param= struct= func= sampler=\n",
);

/// `blocks_cases[0]`: a sampler, a mapped struct and an unmapped one (characterized, not endorsed: main drops the unmapped struct param, though the call still passes it)
const BLOCKS_0: &str = concat!(
    "Draw/0\n",
    "  vertex\n",
    "    | uniform float4x4 ViewProj;\n",
    "    | \n",
    "    | \n",
    "    | struct VertInOut {\n",
    "    | \\tfloat4 pos : POSITION;\n",
    "    | \\tfloat2 uv : TEXCOORD0;\n",
    "    | };\n",
    "    | \n",
    "    | VertInOut VSDefault(VertInOut vert_in)\n",
    "    | {\n",
    "    | \\tVertInOut vert_out;\n",
    "    | \\tvert_out.pos = mul(float4(vert_in.pos.xyz, 1.0), ViewProj);\n",
    "    | \\tvert_out.uv = vert_in.uv;\n",
    "    | \\treturn vert_out;\n",
    "    | }\n",
    "    | \n",
    "    | VertInOut main(VertInOut vert_in)\n",
    "    | {\n",
    "    | \\treturn VSDefault(vert_in);\n",
    "    | }\n",
    "    used ViewProj\n",
    "    written param= struct= func= sampler=\n",
    "  pixel\n",
    "    | uniform texture2d image;\n",
    "    | \n",
    "    | \n",
    "    | sampler_state def_sampler {\n",
    "    | \\tFilter = Linear;\n",
    "    | \n",
    "    | \\tAddressU = Clamp;\n",
    "    | \n",
    "    | \\tAddressV = Wrap;\n",
    "    | \n",
    "    | };\n",
    "    | \n",
    "    | struct VertInOut {\n",
    "    | \\tfloat4 pos : POSITION;\n",
    "    | \\tfloat2 uv : TEXCOORD0;\n",
    "    | };\n",
    "    | \n",
    "    | struct Light {\n",
    "    | \\tfloat3 dir;\n",
    "    | \\tfloat strength;\n",
    "    | };\n",
    "    | \n",
    "    | float4 PSLit(VertInOut vert_in)\n",
    "    | {\n",
    "    | \\tLight l;\n",
    "    | \\tl.strength = 0.5;\n",
    "    | \\treturn image.Sample(def_sampler, vert_in.uv) * l.strength;\n",
    "    | }\n",
    "    | \n",
    "    | float4 main(VertInOut vert_in) : TARGET\n",
    "    | {\n",
    "    | \\treturn PSLit(vert_in);\n",
    "    | }\n",
    "    used image\n",
    "    written param= struct= func= sampler=\n",
    "Unmapped/0\n",
    "  vertex\n",
    "    | uniform float4x4 ViewProj;\n",
    "    | \n",
    "    | \n",
    "    | struct VertInOut {\n",
    "    | \\tfloat4 pos : POSITION;\n",
    "    | \\tfloat2 uv : TEXCOORD0;\n",
    "    | };\n",
    "    | \n",
    "    | VertInOut VSDefault(VertInOut vert_in)\n",
    "    | {\n",
    "    | \\tVertInOut vert_out;\n",
    "    | \\tvert_out.pos = mul(float4(vert_in.pos.xyz, 1.0), ViewProj);\n",
    "    | \\tvert_out.uv = vert_in.uv;\n",
    "    | \\treturn vert_out;\n",
    "    | }\n",
    "    | \n",
    "    | VertInOut main(VertInOut vert_in)\n",
    "    | {\n",
    "    | \\treturn VSDefault(vert_in);\n",
    "    | }\n",
    "    used ViewProj\n",
    "    written param= struct= func= sampler=\n",
    "  pixel\n",
    "    | struct Light {\n",
    "    | \\tfloat3 dir;\n",
    "    | \\tfloat strength;\n",
    "    | };\n",
    "    | \n",
    "    | float4 PSLight(Light l)\n",
    "    | {\n",
    "    | \\treturn float4(l.dir, l.strength);\n",
    "    | }\n",
    "    | \n",
    "    | float4 main() : TARGET\n",
    "    | {\n",
    "    | \\treturn PSLight(l);\n",
    "    | }\n",
    "    used \n",
    "    written param= struct= func= sampler=\n",
);

/// `blocks_cases[1]`: no vertex call, a sampler with no states, a sampler named twice (a blank line each time), and a struct mapped only after its first member (characterized, not endorsed: main drops it)
const BLOCKS_1: &str = concat!(
    "Draw/0\n",
    "  vertex\n",
    "    used \n",
    "    written param= struct= func= sampler=\n",
    "  pixel\n",
    "    | uniform texture2d image;\n",
    "    | \n",
    "    | \n",
    "    | sampler_state plain_sampler {\n",
    "    | };\n",
    "    | \n",
    "    | sampler_state point_sampler {\n",
    "    | \\tFilter = Point;\n",
    "    | \n",
    "    | };\n",
    "    | \n",
    "    | \n",
    "    | struct Late {\n",
    "    | \\tfloat2 uv;\n",
    "    | \\tfloat4 pos : POSITION;\n",
    "    | };\n",
    "    | \n",
    "    | float4 PSTwo(Late v)\n",
    "    | {\n",
    "    | \\treturn image.Sample(plain_sampler, v.uv) + image.Sample(point_sampler, v.uv) + image.Sample(plain_sampler, v.uv);\n",
    "    | }\n",
    "    | \n",
    "    | float4 main() : TARGET\n",
    "    | {\n",
    "    | \\treturn PSTwo(v);\n",
    "    | }\n",
    "    used image\n",
    "    written param= struct= func= sampler=\n",
);

/// `funcs_cases[0]`: a chain and a diamond of calls
const FUNCS_0: &str = concat!(
    "Draw/0\n",
    "  vertex\n",
    "    | uniform float gain;\n",
    "    | \n",
    "    | \n",
    "    | float4 leaf(float4 c)\n",
    "    | { return c * gain; }\n",
    "    | \n",
    "    | \n",
    "    | float4 mid(float4 c)\n",
    "    | { return leaf(c) + leaf(c); }\n",
    "    | \n",
    "    | \n",
    "    | float4 other(float4 c)\n",
    "    | { return leaf(c); }\n",
    "    | \n",
    "    | \n",
    "    | float4 top(float4 c)\n",
    "    | { return mid(c) + other(c); }\n",
    "    | \n",
    "    | float4 main()\n",
    "    | {\n",
    "    | \\treturn top(pos);\n",
    "    | }\n",
    "    used gain\n",
    "    written param= struct= func= sampler=\n",
    "  pixel\n",
    "    | uniform float gain;\n",
    "    | \n",
    "    | \n",
    "    | float4 leaf(float4 c)\n",
    "    | { return c * gain; }\n",
    "    | \n",
    "    | \n",
    "    | float4 mid(float4 c)\n",
    "    | { return leaf(c) + leaf(c); }\n",
    "    | \n",
    "    | \n",
    "    | float4 other(float4 c)\n",
    "    | { return leaf(c); }\n",
    "    | \n",
    "    | \n",
    "    | float4 top(float4 c)\n",
    "    | { return mid(c) + other(c); }\n",
    "    | \n",
    "    | \n",
    "    | float4 PSMain(float4 pos : POSITION)\n",
    "    | { return top(pos); }\n",
    "    | \n",
    "    | float4 main(float4 pos : POSITION) : TARGET\n",
    "    | {\n",
    "    | \\treturn PSMain(pos);\n",
    "    | }\n",
    "    used gain\n",
    "    written param= struct= func= sampler=\n",
);

/// `funcs_cases[1]`: a function that calls itself, and param keywords
const FUNCS_1: &str = concat!(
    "Draw/0\n",
    "  vertex\n",
    "    | float4 rec(float4 c)\n",
    "    | { return rec(c); }\n",
    "    | \n",
    "    | float4 main()\n",
    "    | {\n",
    "    | \\treturn rec(c);\n",
    "    | }\n",
    "    used \n",
    "    written param= struct= func= sampler=\n",
    "  pixel\n",
    "    | void adjust(inout float4 c, out float a, uniform float k, float b)\n",
    "    | { a = b * k; c *= a; }\n",
    "    | \n",
    "    | \n",
    "    | float4 rec(float4 c)\n",
    "    | { return rec(c); }\n",
    "    | \n",
    "    | \n",
    "    | float4 PSAdjust(float4 c : COLOR)\n",
    "    | {\n",
    "    | \\tfloat a;\n",
    "    | \\tadjust(c, a, 1.0, 2.0);\n",
    "    | \\treturn rec(c) * a;\n",
    "    | }\n",
    "    | \n",
    "    | float4 main(float4 c : COLOR) : TARGET\n",
    "    | {\n",
    "    | \\treturn PSAdjust(c);\n",
    "    | }\n",
    "    used \n",
    "    written param= struct= func= sampler=\n",
);

/// `mains_cases[0]`: two mapped params. The C test pins the C's `main()`,
/// with no comma between them (characterized, not endorsed); the port puts
/// one there (#3768).
const MAINS_0: &str = concat!(
    "Draw/0\n",
    "  vertex\n",
    "    | float4 PSTwo(float4 pos : POSITION, float2 uv : TEXCOORD0)\n",
    "    | {\n",
    "    | \\treturn float4(uv, pos.zw);\n",
    "    | }\n",
    "    | \n",
    "    | float4 main(float4 pos : POSITION, float2 uv : TEXCOORD0) : TARGET\n",
    "    | {\n",
    "    | \\treturn PSTwo(pos,uv);\n",
    "    | }\n",
    "    used \n",
    "    written param= struct= func= sampler=\n",
    "  pixel\n",
    "    | float4 PSTwo(float4 pos : POSITION, float2 uv : TEXCOORD0)\n",
    "    | {\n",
    "    | \\treturn float4(uv, pos.zw);\n",
    "    | }\n",
    "    | \n",
    "    | float4 main(float4 pos : POSITION, float2 uv : TEXCOORD0) : TARGET\n",
    "    | {\n",
    "    | \\treturn PSTwo(pos,uv);\n",
    "    | }\n",
    "    used \n",
    "    written param= struct= func= sampler=\n",
);

/// `mains_cases[1]`: mapped and unmapped params mixed (characterized, not
/// endorsed: main drops k). The C test pins no comma between the rest; the
/// port puts one there (#3768).
const MAINS_1: &str = concat!(
    "Draw/0\n",
    "  vertex\n",
    "    | float4 PSMix(float4 pos : POSITION, float k, float2 uv : TEXCOORD0)\n",
    "    | {\n",
    "    | \\treturn pos * k + float4(uv, 0.0, 0.0);\n",
    "    | }\n",
    "    | \n",
    "    | float4 main(float4 pos : POSITION, float2 uv : TEXCOORD0) : TARGET\n",
    "    | {\n",
    "    | \\treturn PSMix(pos,k,uv);\n",
    "    | }\n",
    "    used \n",
    "    written param= struct= func= sampler=\n",
    "  pixel\n",
    "    | struct VertInOut {\n",
    "    | \\tfloat4 pos : POSITION;\n",
    "    | \\tfloat2 uv : TEXCOORD0;\n",
    "    | };\n",
    "    | \n",
    "    | float4 PSBoth(VertInOut v, float4 c : COLOR)\n",
    "    | {\n",
    "    | \\treturn v.pos * c;\n",
    "    | }\n",
    "    | \n",
    "    | float4 main(VertInOut v, float4 c : COLOR) : TARGET\n",
    "    | {\n",
    "    | \\treturn PSBoth(v,c);\n",
    "    | }\n",
    "    used \n",
    "    written param= struct= func= sampler=\n",
);

/// `mains_cases[2]`: no params, and no return mapping
const MAINS_2: &str = concat!(
    "Draw/0\n",
    "  vertex\n",
    "    | float4 VSNoMap(float4 pos : POSITION)\n",
    "    | {\n",
    "    | \\treturn pos;\n",
    "    | }\n",
    "    | \n",
    "    | float4 main(float4 pos : POSITION)\n",
    "    | {\n",
    "    | \\treturn VSNoMap(pos);\n",
    "    | }\n",
    "    used \n",
    "    written param= struct= func= sampler=\n",
    "  pixel\n",
    "    | float4 PSNone()\n",
    "    | {\n",
    "    | \\treturn float4(1.0, 1.0, 1.0, 1.0);\n",
    "    | }\n",
    "    | \n",
    "    | float4 main() : TARGET\n",
    "    | {\n",
    "    | \\treturn PSNone();\n",
    "    | }\n",
    "    used \n",
    "    written param= struct= func= sampler=\n",
);

/// `calls_cases[0]`: NULL, and a pass with no pixel shader
const CALLS_0: &str = concat!(
    "Draw/0\n",
    "  vertex\n",
    "    | uniform float4 tint;\n",
    "    | \n",
    "    | \n",
    "    | float4 VSMain(float4 pos : POSITION)\n",
    "    | { return pos * tint; }\n",
    "    | \n",
    "    | float4 main(float4 pos : POSITION) : POSITION\n",
    "    | {\n",
    "    | \\treturn VSMain(pos);\n",
    "    | }\n",
    "    used tint\n",
    "    written param= struct= func= sampler=\n",
    "  pixel\n",
    "    used \n",
    "    written param= struct= func= sampler=\n",
    "Vertex/0\n",
    "  vertex\n",
    "    | uniform float4 tint;\n",
    "    | \n",
    "    | \n",
    "    | float4 VSMain(float4 pos : POSITION)\n",
    "    | { return pos * tint; }\n",
    "    | \n",
    "    | float4 main(float4 pos : POSITION) : POSITION\n",
    "    | {\n",
    "    | \\treturn VSMain(pos);\n",
    "    | }\n",
    "    used tint\n",
    "    written param= struct= func= sampler=\n",
    "  pixel\n",
    "    used \n",
    "    written param= struct= func= sampler=\n",
);

/// `calls_cases[1]`: an unknown function, and a param in the function's place: only the params the call names
const CALLS_1: &str = concat!(
    "Draw/0\n",
    "  vertex\n",
    "    | uniform float4 tint;\n",
    "    used tint\n",
    "    written param= struct= func= sampler=\n",
    "  pixel\n",
    "    | uniform float4 tint;\n",
    "    used tint\n",
    "    written param= struct= func= sampler=\n",
);

/// `calls_cases[2]`: spaces inside the call, which the pass parser drops
const CALLS_2: &str = concat!(
    "Draw/0\n",
    "  vertex\n",
    "    | uniform float4 tint;\n",
    "    | float4 VSMain(float4 pos : POSITION, float4 t)\n",
    "    | { return pos * t; }\n",
    "    | \n",
    "    | float4 main(float4 pos : POSITION) : POSITION\n",
    "    | {\n",
    "    | \\treturn VSMain(pos,tint);\n",
    "    | }\n",
    "    used tint\n",
    "    written param= struct= func= sampler=\n",
    "  pixel\n",
    "    | uniform float4 tint;\n",
    "    | float4 VSMain(float4 pos : POSITION, float4 t)\n",
    "    | { return pos * t; }\n",
    "    | \n",
    "    | float4 main(float4 pos : POSITION) : POSITION\n",
    "    | {\n",
    "    | \\treturn VSMain(pos,tint);\n",
    "    | }\n",
    "    used tint\n",
    "    written param= struct= func= sampler=\n",
);

/// `calls_cases[3]`: two passes calling the same function
const CALLS_3: &str = concat!(
    "Draw/0\n",
    "  vertex\n",
    "    | uniform float4x4 ViewProj;\n",
    "    | \n",
    "    | \n",
    "    | struct VertInOut {\n",
    "    | \\tfloat4 pos : POSITION;\n",
    "    | \\tfloat2 uv : TEXCOORD0;\n",
    "    | };\n",
    "    | \n",
    "    | VertInOut VSDefault(VertInOut v)\n",
    "    | { v.pos = mul(v.pos, ViewProj); return v; }\n",
    "    | \n",
    "    | VertInOut main(VertInOut v)\n",
    "    | {\n",
    "    | \\treturn VSDefault(v);\n",
    "    | }\n",
    "    used ViewProj\n",
    "    written param= struct= func= sampler=\n",
    "  pixel\n",
    "    | uniform float4x4 ViewProj;\n",
    "    | \n",
    "    | \n",
    "    | struct VertInOut {\n",
    "    | \\tfloat4 pos : POSITION;\n",
    "    | \\tfloat2 uv : TEXCOORD0;\n",
    "    | };\n",
    "    | \n",
    "    | VertInOut VSDefault(VertInOut v)\n",
    "    | { v.pos = mul(v.pos, ViewProj); return v; }\n",
    "    | \n",
    "    | VertInOut main(VertInOut v)\n",
    "    | {\n",
    "    | \\treturn VSDefault(v);\n",
    "    | }\n",
    "    used ViewProj\n",
    "    written param= struct= func= sampler=\n",
    "Draw/1\n",
    "  vertex\n",
    "    | uniform float4x4 ViewProj;\n",
    "    | \n",
    "    | \n",
    "    | struct VertInOut {\n",
    "    | \\tfloat4 pos : POSITION;\n",
    "    | \\tfloat2 uv : TEXCOORD0;\n",
    "    | };\n",
    "    | \n",
    "    | VertInOut VSDefault(VertInOut v)\n",
    "    | { v.pos = mul(v.pos, ViewProj); return v; }\n",
    "    | \n",
    "    | VertInOut main(VertInOut v)\n",
    "    | {\n",
    "    | \\treturn VSDefault(v);\n",
    "    | }\n",
    "    used ViewProj\n",
    "    written param= struct= func= sampler=\n",
    "  pixel\n",
    "    used \n",
    "    written param= struct= func= sampler=\n",
);

/// `preset_cases[0]`: a sampler, a struct, params and a function dependency all marked written; a uniform the call names is then not pushed into used_params
const PRESET_0: &str = concat!(
    "Draw/0\n",
    "  vertex\n",
    "    | \n",
    "    | \n",
    "    | VertInOut VSDefault(VertInOut v)\n",
    "    | { v.pos = mul(v.pos, ViewProj); return v; }\n",
    "    | \n",
    "    | VertInOut main(VertInOut v)\n",
    "    | {\n",
    "    | \\treturn VSDefault(v);\n",
    "    | }\n",
    "    used \n",
    "    written param= struct= func= sampler=\n",
    "  pixel\n",
    "    | float4 PSDraw(VertInOut v)\n",
    "    | { return sample_at(v.uv); }\n",
    "    | \n",
    "    | float4 main(VertInOut v) : TARGET\n",
    "    | {\n",
    "    | \\treturn PSDraw(v);\n",
    "    | }\n",
    "    used \n",
    "    written param= struct= func= sampler=\n",
    "Null/0\n",
    "  vertex\n",
    "    | \n",
    "    | \n",
    "    | VertInOut VSDefault(VertInOut v)\n",
    "    | { v.pos = mul(v.pos, ViewProj); return v; }\n",
    "    | \n",
    "    | VertInOut main(VertInOut v)\n",
    "    | {\n",
    "    | \\treturn VSDefault(ViewProj);\n",
    "    | }\n",
    "    used \n",
    "    written param= struct= func= sampler=\n",
    "  pixel\n",
    "    used \n",
    "    written param=ViewProj,image struct=VertInOut func=sample_at,VSDefault,PSDraw sampler=def_sampler\n",
    "Vertex/0\n",
    "  vertex\n",
    "    | \n",
    "    | \n",
    "    | VertInOut VSDefault(VertInOut v)\n",
    "    | { v.pos = mul(v.pos, ViewProj); return v; }\n",
    "    | \n",
    "    | VertInOut main(VertInOut v)\n",
    "    | {\n",
    "    | \\treturn VSDefault(v);\n",
    "    | }\n",
    "    used \n",
    "    written param= struct= func= sampler=\n",
    "  pixel\n",
    "    used \n",
    "    written param=ViewProj,image struct=VertInOut func=sample_at,VSDefault,PSDraw sampler=def_sampler\n",
);

/// The list a recorder and its output log into, in order.
type Log = Rc<RefCell<Vec<String>>>;

/// A parser that logs every read and write of the writer, with the text it
/// appends, in one list, so the order of the C's reads and flag writes is
/// checked, not only the result.
struct Recorder<'a> {
    src: ShaderSource<'a>,
    log: Log,
}

impl Recorder<'_> {
    fn note(&self, what: String) {
        self.log.borrow_mut().push(what);
    }
}

/// The recorder's appends, into the same log.
struct RecordedOut(Log);

impl ShaderOut for RecordedOut {
    fn cat(&mut self, piece: &[u8]) {
        self.0
            .borrow_mut()
            .push(format!("cat {:?}", String::from_utf8_lossy(piece)));
    }
    fn cat_dstr(&mut self, piece: &[u8]) {
        self.0
            .borrow_mut()
            .push(format!("cat_dstr {:?}", String::from_utf8_lossy(piece)));
    }
    fn push_used(&mut self, name: &[u8]) {
        self.0
            .borrow_mut()
            .push(format!("used {}", String::from_utf8_lossy(name)));
    }
}
macro_rules! logged {
    ($($name:ident($($arg:ident: $ty:ty),*) -> $ret:ty;)*) => {
        $(fn $name(&self, $($arg: $ty),*) -> $ret {
            self.note(format!(concat!(stringify!($name), "{:?}"), ($($arg,)*)));
            self.src.$name($($arg),*)
        })*
    };
}

impl WriterParser for Recorder<'_> {
    logged! {
        param_count() -> usize;
        param_name(i: usize) -> &[u8];
        param_type(i: usize) -> &[u8];
        param_is_const(i: usize) -> bool;
        param_is_uniform(i: usize) -> bool;
        param_array_count(i: usize) -> i32;
        param_written(i: usize) -> bool;
        struct_count() -> usize;
        struct_name(i: usize) -> &[u8];
        struct_written(i: usize) -> bool;
        sampler_count() -> usize;
        sampler_name(i: usize) -> &[u8];
        sampler_value_count(i: usize) -> usize;
        sampler_state(i: usize, j: usize) -> &[u8];
        sampler_value(i: usize, j: usize) -> &[u8];
        sampler_written(i: usize) -> bool;
        func_count() -> usize;
        func_name(i: usize) -> &[u8];
        func_ret_type(i: usize) -> &[u8];
        func_mapping(i: usize) -> Option<&[u8]>;
        func_contents(i: usize) -> &[u8];
        dep_count(i: usize, deps: Deps) -> usize;
        dep(i: usize, deps: Deps, k: usize) -> &[u8];
        func_written(i: usize) -> bool;
        var_count(of: VarsOf) -> usize;
        var_type(of: VarsOf, j: usize) -> VarType;
        var_type_name(of: VarsOf, j: usize) -> &[u8];
        var_name(of: VarsOf, j: usize) -> &[u8];
        var_mapping(of: VarsOf, j: usize) -> Option<&[u8]>;
    }
    fn set_param_written(&mut self, i: usize, written: bool) {
        self.note(format!("set_param_written{:?}", (i, written)));
        self.src.set_param_written(i, written);
    }
    fn set_struct_written(&mut self, i: usize, written: bool) {
        self.note(format!("set_struct_written{:?}", (i, written)));
        self.src.set_struct_written(i, written);
    }
    fn set_sampler_written(&mut self, i: usize, written: bool) {
        self.note(format!("set_sampler_written{:?}", (i, written)));
        self.src.set_sampler_written(i, written);
    }
    fn set_func_written(&mut self, i: usize, written: bool) {
        self.note(format!("set_func_written{:?}", (i, written)));
        self.src.set_func_written(i, written);
    }
}

/// One call through the recorder, its log.
fn record(src: Src, call: Option<&[&'static str]>) -> Vec<String> {
    let call = call.map(tokens);
    let log = Log::default();
    let mut rec = Recorder {
        src: src.0,
        log: Rc::clone(&log),
    };
    let mut out = RecordedOut(Rc::clone(&log));
    make_shader_string(&mut rec, &mut out, call.as_deref());
    drop(rec);
    log.take()
}

fn traced() -> Src {
    let mut s = Src::default();
    s.param(Uni, "float4", "tint", 0);
    s.strukt("V", &[v(In, "float4", "pos", Some("POSITION"))]);
    s.sampler("smp", &[("Filter", "Linear")]);
    s.func(Func {
        ret: "float4",
        name: "PS",
        mapping: Some("TARGET"),
        vars: &[v(In, "V", "v", None)],
        body: "{}",
        funcs: &[],
        structs: &["V"],
        params: &["tint"],
        samplers: &["smp"],
    });
    s
}

#[test]
fn edge_calls_read_nothing() {
    // ep_makeshaderstring() returns before it reads anything of the parser
    // for a call with no token array, no tokens, only whitespace or NULL.
    for call in [
        None,
        Some(&[][..]),
        Some(&[" ", "\t"][..]),
        Some(&["\n", "NULL", "(", ")"][..]),
    ] {
        assert_eq!(record(traced(), call), Vec::<String>::new(), "{call:?}");
    }
}

/// What the writer reads and writes for `traced()` and `PS(v)`.
const TRACE: &[&str] = &[
    "param_count()",
    "param_name(0,)",
    "param_count()",
    "param_name(0,)",
    "param_count()",
    "param_name(0,)",
    "param_count()",
    "param_name(0,)",
    "func_count()",
    "func_name(0,)",
    "set_func_written(0, true)",
    "dep_count(0, Params)",
    "dep(0, Params, 0)",
    "param_count()",
    "param_name(0,)",
    "param_written(0,)",
    "param_is_const(0,)",
    "param_is_uniform(0,)",
    "param_name(0,)",
    "used tint",
    "cat \"uniform \"",
    "param_type(0,)",
    "cat \"float4\"",
    "cat \" \"",
    "param_name(0,)",
    "cat \"tint\"",
    "param_array_count(0,)",
    "cat \";\\n\"",
    "set_param_written(0, true)",
    "dep_count(0, Params)",
    "dep_count(0, Params)",
    "cat \"\\n\\n\"",
    "dep_count(0, Samplers)",
    "dep(0, Samplers, 0)",
    "sampler_count()",
    "sampler_name(0,)",
    "sampler_written(0,)",
    "cat \"sampler_state \"",
    "sampler_name(0,)",
    "cat \"smp\"",
    "cat \" {\"",
    "sampler_value_count(0,)",
    "cat \"\\n\\t\"",
    "sampler_state(0, 0)",
    "cat \"Filter\"",
    "cat \" = \"",
    "sampler_value(0, 0)",
    "cat \"Linear\"",
    "cat \";\\n\"",
    "sampler_value_count(0,)",
    "cat \"\\n};\\n\"",
    "set_sampler_written(0, true)",
    "cat \"\\n\"",
    "dep_count(0, Samplers)",
    "dep_count(0, Structs)",
    "dep(0, Structs, 0)",
    "struct_count()",
    "struct_name(0,)",
    "struct_written(0,)",
    "struct_written(0,)",
    "cat \"struct \"",
    "struct_name(0,)",
    "cat \"V\"",
    "cat \" {\"",
    "var_count(Struct(0),)",
    "cat \"\\n\\t\"",
    "var_type(Struct(0), 0)",
    "var_type_name(Struct(0), 0)",
    "cat \"float4\"",
    "cat \" \"",
    "var_name(Struct(0), 0)",
    "cat \"pos\"",
    "var_mapping(Struct(0), 0)",
    "cat \" : \"",
    "cat \"POSITION\"",
    "cat \";\"",
    "var_count(Struct(0),)",
    "cat \"\\n};\\n\"",
    "set_struct_written(0, true)",
    "cat \"\\n\"",
    "set_struct_written(0, true)",
    "dep_count(0, Structs)",
    "dep_count(0, Funcs)",
    "func_ret_type(0,)",
    "cat \"float4\"",
    "cat \" \"",
    "func_name(0,)",
    "cat \"PS\"",
    "cat \"(\"",
    "var_count(Func(0),)",
    "var_type(Func(0), 0)",
    "var_type_name(Func(0), 0)",
    "cat \"V\"",
    "cat \" \"",
    "var_name(Func(0), 0)",
    "cat \"v\"",
    "var_mapping(Func(0), 0)",
    "var_count(Func(0),)",
    "cat \")\\n\"",
    "func_contents(0,)",
    "cat_dstr \"{}\"",
    "cat \"\\n\"",
    "cat \"\\n\"",
    "func_ret_type(0,)",
    "cat \"float4\"",
    "cat \" main(\"",
    "var_count(Func(0),)",
    "var_mapping(Func(0), 0)",
    "var_type_name(Func(0), 0)",
    "struct_count()",
    "struct_name(0,)",
    "var_count(Struct(0),)",
    "var_mapping(Struct(0), 0)",
    "var_type_name(Func(0), 0)",
    "cat \"V\"",
    "cat \" \"",
    "var_name(Func(0), 0)",
    "cat \"v\"",
    "var_count(Func(0),)",
    "cat \")\"",
    "func_mapping(0,)",
    "cat \" : \"",
    "cat \"TARGET\"",
    "cat \"\\n{\\n\\treturn \"",
    "cat_dstr \"PS(v)\"",
    "cat \"\\n}\\n\"",
    "param_count()",
    "set_param_written(0, false)",
    "param_count()",
    "struct_count()",
    "set_struct_written(0, false)",
    "struct_count()",
    "func_count()",
    "set_func_written(0, false)",
    "func_count()",
    "sampler_count()",
    "set_sampler_written(0, false)",
    "sampler_count()",
];

#[test]
fn edge_reads_in_c_order() {
    // Every read of the parser and every written flag set, with the text,
    // in the C's order: each loop bound read on each pass, a flag tested
    // before its item is read and set right after it is written, a
    // function marked written before its dependencies, the struct
    // dependency's written flag tested twice and set twice, and the
    // flags cleared one collection at a time at the end.
    assert_eq!(record(traced(), Some(&["PS", "(", "v", ")"])), TRACE);
}
