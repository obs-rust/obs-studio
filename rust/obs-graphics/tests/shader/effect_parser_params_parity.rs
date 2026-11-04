//! Tier 2 layout and Tier 3 original-C parity for the effect parser's
//! parameter declarations, annotations and value assignments
//! (`libobs/graphics/effect-parser-params.c.h`,
//! `obs_graphics::ffi::effect_parser`).
//!
//! Both sides get the same tokens: the oracle's C lexer and preprocessor
//! (`cf_parser_parse()`) make them twice from the same text, and each side
//! runs `ep_parse_param` from the same token with the same type, name and
//! flags (each its own `bstrdup` copies, which it owns). The oracle is the
//! original C with the C `cf_adderror`, `cf_pass_pair`, `cf_literal_to_str`,
//! `strref_cmp`, parameter type decisions, number reads and `os_strtod`; the
//! shim reaches libobs's `cf_adderror`, `cf_literal_to_str` and `os_strtod`,
//! which are obs-util's Rust here as in the hybrid libobs. Every param they
//! push (each field but the `gs_effect_param` pointer, which the C leaves
//! unset until `ep_compile_param`, so the shim's `NULL` is checked on its own;
//! the type, name, default bytes, darray counts and capacities, and each
//! annotation the same way), the other collections, every error (text, file,
//! row, column, level) and where the cursor stopped must match.
//!
//! Inputs: the declarations of `test_effect_parser_params` and of the Tier 1
//! tests, each from the token after its name as `ep_parse_other()` reaches
//! it, every prefix of each, each from every token with each of a set of
//! types, and random token soups after a random type or from a random token.
//! No intentional differences in this file. Left out: an `int` value or an
//! array count past `INT_MAX`, where the number reads' intentional difference
//! (#3464, `effect_param_types`) applies; `int_values_past_int_max` checks
//! the port's side of it against that contract. Texts hold no `#`: the
//! preprocessor's directives are not the declarations' (each side gets the
//! same tokens either way).

use core::ffi::{CStr, c_char};
use core::mem::{align_of, offset_of, size_of};
use std::ffi::CString;

use obs_c_oracle::effect_parser_params as c;
use obs_graphics::ffi::effect_parser::{self as rs, ep_param};
use obs_graphics::ffi::effect_parser_lifecycle::effect_parser;
use obs_util::ffi::cf_tokenizer::cf_token;
use obs_util::ffi::darray::darray;
use obs_util::ffi::lexer::error_item;
use proptest::prelude::*;

/// `CFTOKEN_NONE`, `CFTOKEN_SPACETAB` and `CFTOKEN_NEWLINE`.
const NONE: i32 = 0;
const SPACETAB: i32 = 3;
const NEWLINE: i32 = 4;

#[test]
fn layouts() {
    let rust = [
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
    ];
    let mut oracle = [0usize; 32];
    // SAFETY: the oracle writes fewer than 32 values.
    let n = unsafe { c::oracle_effect_parser_params_layout(oracle.as_mut_ptr()) };
    assert_eq!(&rust[..], &oracle[..n]);
}

/// An `effect_parser` the oracle's C lexed and preprocessed, at one token.
struct Prepared {
    ep: Box<effect_parser>,
    /// Its tokens before the terminator.
    count: usize,
}

impl Prepared {
    /// `None` when the C parse fails.
    fn new(text: &CStr, start: usize) -> Option<Self> {
        // SAFETY: every field of effect_parser permits zero; the oracle
        // initializes it before anything reads it.
        let mut ep: Box<effect_parser> = Box::new(unsafe { core::mem::zeroed() });
        // SAFETY: a boxed parser that stays put, and a C string the lexer
        // copies.
        let count = unsafe {
            c::oracle_effect_parser_params_prepare((&raw mut *ep).cast(), text.as_ptr(), start)
        };
        let prepared = Self { ep, count };
        (count != 0).then_some(prepared)
    }

    fn ptr(&mut self) -> *mut effect_parser {
        &raw mut *self.ep
    }

    /// The parser's tokens, the terminator included.
    fn tokens(&mut self) -> &[cf_token] {
        // SAFETY: the oracle's token array holds count tokens and the
        // terminator, alive as long as the parser.
        unsafe {
            let first = c::oracle_effect_parser_params_tokens(self.ptr().cast());
            core::slice::from_raw_parts(first.cast::<cf_token>(), self.count + 1)
        }
    }
}

impl Drop for Prepared {
    fn drop(&mut self) {
        // SAFETY: an initialized parser; the oracle frees what either side
        // pushed (all bmalloc'd) as ep_free() does.
        unsafe { c::oracle_effect_parser_params_free(self.ptr().cast()) };
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
        "[{} of {}{}]",
        da.num,
        da.capacity,
        if da.array.is_null() { " null" } else { "" }
    )
}

/// # Safety
///
/// `da` is a darray of `T` holding `num` items.
unsafe fn items<T>(da: &darray) -> &[T] {
    if da.num == 0 {
        return &[];
    }
    // SAFETY: the caller's contract.
    unsafe { core::slice::from_raw_parts(da.array.cast::<T>(), da.num) }
}

/// Every field of `param` but `param`, its annotations indented under it.
///
/// # Safety
///
/// `param` is a param one of the sides pushed.
unsafe fn param_lines(param: &ep_param, indent: &str) -> String {
    // SAFETY: the caller's contract: each darray holds `num` items of its C
    // type, every string a bmalloc'd C string or null.
    unsafe {
        let bytes: Vec<String> = items::<u8>(&param.default_val)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let mut out = format!(
            "{indent}param {} {} default {} {{{}}} properties {} const {} property {} uniform {} texture {} written {} order {} count {} annotations {}\n",
            text(param.r#type),
            text(param.name),
            shape(&param.default_val),
            bytes.join(" "),
            shape(&param.properties),
            param.is_const,
            param.is_property,
            param.is_uniform,
            param.is_texture,
            param.written,
            param.writeorder,
            param.array_count,
            shape(&param.annotations),
        );
        for annotation in items::<ep_param>(&param.annotations) {
            out += &param_lines(annotation, &format!("{indent}  "));
        }
        out
    }
}

/// Everything either side can leave in the parser, one line each.
fn snapshot(p: &mut Prepared) -> String {
    let position = {
        let first = p.tokens().as_ptr();
        // SAFETY: cur_token points into the same token array.
        unsafe { p.ep.cfp.cur_token.cast_const().offset_from(first) }
    };
    let ep = &*p.ep;
    let mut out = format!(
        "at {position}\nparams {} structs {} funcs {} samplers {} techniques {} files {} tokens {}\neffect {:?} pass {:?}\n",
        shape(&ep.params),
        shape(&ep.structs),
        shape(&ep.funcs),
        shape(&ep.samplers),
        shape(&ep.techniques),
        shape(&ep.files),
        shape(&ep.tokens),
        ep.effect,
        ep.cur_pass,
    );
    // SAFETY: each darray holds `num` items of its C type, every string a
    // bmalloc'd C string or null.
    unsafe {
        for param in items::<ep_param>(&ep.params) {
            out += &param_lines(param, "");
        }
        for e in items::<error_item>(&ep.cfp.error_list.errors) {
            out += &format!(
                "error {}:{}:{} {} {}\n",
                text(e.file),
                e.row,
                e.column,
                e.level,
                text(e.error)
            );
        }
    }
    out
}

/// The shim's params and annotations all have a `NULL` `param`.
///
/// # Safety
///
/// As for [`param_lines`].
unsafe fn null_effect_params(params: &darray) -> bool {
    // SAFETY: the caller's contract.
    unsafe {
        items::<ep_param>(params)
            .iter()
            .all(|p| p.param.is_null() && null_effect_params(&p.annotations))
    }
}

/// A declaration's specifiers, type and name.
#[derive(Clone, Debug)]
struct Decl {
    type_name: CString,
    name: CString,
    is_property: bool,
    is_const: bool,
    is_uniform: bool,
}

/// Both sides from token `start` of `text` with `decl`. `None` when the C
/// parse fails.
fn run(text: &str, start: usize, decl: &Decl) -> Option<(String, String)> {
    let text = CString::new(text).ok()?;
    let mut oracle = Prepared::new(&text, start)?;
    let mut shim = Prepared::new(&text, start)?;
    // SAFETY: both are initialized parsers at a token of their stream; each
    // side owns its own bmalloc'd type and name.
    unsafe {
        c::oracle_ep_parse_param(
            oracle.ptr().cast(),
            c::oracle_effect_parser_params_strdup(decl.type_name.as_ptr()),
            c::oracle_effect_parser_params_strdup(decl.name.as_ptr()),
            decl.is_property,
            decl.is_const,
            decl.is_uniform,
        );
        rs::ep_parse_param(
            shim.ptr(),
            c::oracle_effect_parser_params_strdup(decl.type_name.as_ptr()),
            c::oracle_effect_parser_params_strdup(decl.name.as_ptr()),
            decl.is_property,
            decl.is_const,
            decl.is_uniform,
        );
        assert!(null_effect_params(&shim.ep.params));
    }
    Some((snapshot(&mut oracle), snapshot(&mut shim)))
}

fn check(text: &str, start: usize, decl: &Decl) -> Result<(), TestCaseError> {
    if let Some((oracle, shim)) = run(text, start, decl) {
        prop_assert_eq!(
            oracle,
            shim,
            "{:?} from token {} as {:?}",
            text,
            start,
            decl
        );
    }
    Ok(())
}

/// The token's text.
fn token_text(token: &cf_token) -> &[u8] {
    if token.str.array.is_null() {
        return &[];
    }
    // SAFETY: a token's str is len readable bytes of the lexer's text.
    unsafe { core::slice::from_raw_parts(token.str.array.cast::<u8>(), token.str.len) }
}

/// `ep_parse_other()`'s steps before `ep_parse_param`, on the C tokens of
/// `text`: the specifiers, the type, the name and the next valid token,
/// each `cf_next_token`. The token index and the declaration, or `None` when
/// the text ends first.
fn declaration(text: &str) -> Option<(usize, Decl)> {
    let text = CString::new(text).ok()?;
    let mut prepared = Prepared::new(&text, 0)?;
    let tokens = prepared.tokens();
    let mut i = 0;
    let next = |i: &mut usize| {
        *i += 1;
        while tokens[*i].r#type == SPACETAB || tokens[*i].r#type == NEWLINE {
            *i += 1;
        }
        tokens[*i].r#type != NONE
    };
    while tokens[i].r#type == SPACETAB || tokens[i].r#type == NEWLINE {
        i += 1;
    }
    let mut flags = [false; 3];
    loop {
        let flag = match token_text(&tokens[i]) {
            b"property" => 0,
            b"const" => 1,
            b"uniform" => 2,
            _ => break,
        };
        flags[flag] = true;
        if !next(&mut i) {
            return None;
        }
    }
    let type_name = CString::new(token_text(&tokens[i])).ok()?;
    if !next(&mut i) {
        return None;
    }
    let name = CString::new(token_text(&tokens[i])).ok()?;
    if !next(&mut i) {
        return None;
    }
    let [is_property, is_const, is_uniform] = flags;
    Some((
        i,
        Decl {
            type_name,
            name,
            is_property,
            is_const,
            is_uniform,
        },
    ))
}

fn decl(type_name: &str, flags: u8) -> Decl {
    Decl {
        type_name: CString::new(type_name).unwrap(),
        name: CString::new("p").unwrap(),
        is_property: flags & 1 != 0,
        is_const: flags & 2 != 0,
        is_uniform: flags & 4 != 0,
    }
}

/// The types the declarations from every token are tried as: one of each
/// value parser, the matrix and constructor forms, and none.
const TYPES: &[&str] = &[
    "float",
    "int",
    "bool",
    "string",
    "texture2d",
    "float2",
    "int3",
    "float4x4",
    "float5",
    "foo",
];

/// The declarations of `test_effect_parser_params` and of the Tier 1 tests,
/// but the three whose value or count is past `INT_MAX`.
const FIXED: &[&str] = &[
    "uniform float f;",
    "const float f; property float g; float h;",
    "uniform int i = 3;",
    "uniform int i = -3;",
    "uniform int i = - 3;",
    "uniform int i = 1.5;",
    "uniform float f = 1.5;",
    "uniform float f = -1.5;",
    "uniform float f = - 1.5;",
    "uniform float f = 2;",
    "uniform float f = 1.5f;",
    "uniform float f = x; uniform float g;",
    "uniform float f = --1.0; uniform float g;",
    "uniform bool b = true;",
    "uniform bool b = false;",
    "uniform bool b = 1; uniform float g;",
    "uniform string s = \"hi\";",
    "uniform string s = \"a\\\"b\\n\";",
    "uniform string s = \"\";",
    "uniform string s = hi; uniform float g;",
    "uniform texture2d image;",
    "uniform texture2d image = \"a.png\";",
    "uniform texture_rect image = \"a.png\";",
    "uniform texture2d image = a; uniform float g;",
    "uniform foo v = 1; uniform float g;",
    "uniform float f = 1.0 uniform float g;",
    "uniform float f 1.0; uniform float g;",
    "uniform float4 c = {1.0, 0.5, 0.25, 1.0};",
    "uniform float4 c = float4(1.0, 0.5, 0.25, 1.0);",
    "uniform float2 c = {-1.0, - 2.0};",
    "uniform int2 v = {1, -2};",
    "uniform int3 v = int3(1, 2, 3);",
    "uniform float3x3 m = {1, 2, 3, 4, 5, 6, 7, 8, 9};",
    "uniform float4x4 m = {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1};",
    "uniform int2x2 m = {1, 2, 3, 4};",
    "uniform float1 f = {1.0};",
    "uniform float5 v = {1, 2, 3, 4, 5};",
    "uniform float0 v = {}; uniform float g;",
    "uniform float4x5 m = {1, 2}; uniform float g;",
    "uniform float4 c = {1.0, 0.5}; uniform float g;",
    "uniform float2 c = {1.0 0.5}; uniform float g;",
    "uniform float2 c = {1.0, 0.5, 0.25}; uniform float g;",
    "uniform float2 c = {1.0, x}; uniform float g;",
    "uniform float2 c = 1.0; uniform float g;",
    "uniform float2 c = float3(1.0, 2.0); uniform float g;",
    "uniform float2 c = float2 1.0, 2.0); uniform float g;",
    "uniform float2 c = float2(1.0, 2.0}; uniform float g;",
    "uniform float2 c = {1.0, 2.0); uniform float g;",
    "uniform float a[3];",
    "uniform float a[3] = 1.0;",
    "uniform float a[0];",
    "uniform float a[-1]; uniform float g;",
    "uniform float a[x]; uniform float g;",
    "uniform float a[1.5]; uniform float g;",
    "uniform float a[3; uniform float g;",
    "uniform float a[3] uniform float g;",
    "uniform float a[]; uniform float g;",
    "uniform float4 a[2] = {1, 2, 3, 4};",
    "uniform float f",
    "uniform float f =",
    "uniform float f = -",
    "uniform float f = 1.0",
    "uniform bool b =",
    "uniform string s =",
    "uniform texture2d image =",
    "uniform float2 c =",
    "uniform float2 c = {",
    "uniform float2 c = {1.0",
    "uniform float2 c = {1.0,",
    "uniform float2 c = {1.0, 2.0",
    "uniform float2 c = {1.0, 2.0}",
    "uniform float2 c = float2",
    "uniform float2 c = float2(",
    "uniform float2 c = 1.0",
    "uniform float a[",
    "uniform float a[3",
    "uniform float a[3]",
    "uniform float f<string name = \"x\"; float v = 1.0;> = 2.0;",
    "uniform float4 c<int a = 1; bool b = false; float2 p = {1, 2};>;",
    "uniform float f< >;",
    "uniform float f<>;",
    "uniform float f<;; float v;>;",
    "uniform float f<int a = 1; int a = 2;>;",
    "uniform float f[2]<int a = 1;> = 1.0;",
    "uniform texture2d image<string name = \"Image\";> = \"a.png\";",
    "uniform float f<string = \"x\";>; uniform float g;",
    "uniform float f<string>; uniform float g;",
    "uniform float f<string name : POS;>; uniform float g;",
    "uniform float f<string name>; uniform float g;",
    "uniform float f<float v = x;>; uniform float g;",
    "uniform float f<float v = 1.0 int w = 2;>; uniform float g;",
    "uniform float f<float v 1.0; int w = 2;>; uniform float g;",
    "uniform float f<1 v; int w = 2;>; uniform float g;",
    "uniform float f<float 1; int w = 2;>; uniform float g;",
    "uniform float f<float v<int a = 1;>; int w = 2;>; uniform float g;",
    "uniform float f<foo v = 1; int w = 2;>; uniform float g;",
    "uniform float f<int w = 2;> uniform float g;",
    "uniform float f = 1.0<int a = 1;>; uniform float g;",
    "uniform float f<int a = 1;><int b = 2;>; uniform float g;",
    "uniform float f<",
    "uniform float f<float",
    "uniform float f<float v",
    "uniform float f<float v =",
    "uniform float f<float v = 1.0",
    "uniform float f<float v = 1.0;",
    "uniform float f<float v = 1.0;>",
    "uniform float f<float v x",
    "uniform float4x4 ViewProj;\nuniform texture2d image;\nuniform float4 color = {1.0, 1.0, 1.0, 1.0};\nfloat4 PSDraw(float4 v) { return v * color; }\nuniform float alpha = 0.5;",
    "const uniform property float f;",
    "property const float f = 1.0;",
    "float f = 1.0;",
    "int i = 0x10;",
    "float f = 1e39;",
    "float f = .5;",
    "float f = 1.000000000000000000000000000000000000000000000000000000000000002;",
    "string s = \"a\\0b\";",
    "string s = 'x';",
    "string s = \"tab\\there\";",
    "strings s = \"x\";",
    "texture3d t = \"v.dds\";",
    "textures t = \"v.dds\";",
    "bool2 b = true;",
    "int1 v = {3};",
    "int1 v = 3;",
    "float4x4 m = float4x4(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1);",
    "float3 c = float3(-1, - 2, 3);",
    "float2 c = {\n\t1.0,\n\t2.0\n};",
    "float2 c = float2 (1.0, 2.0);",
    "float2 c = {-, 1.0};",
    "float2 c = {1.0, -};",
    "float a[2][3];",
    "float a[3] = {1.0};",
    "float a[0x10];",
    "float a[007];",
    "float4 a[2]<string name = \"n\";> = {1, 2, 3, 4};",
    "float f<texture2d t = \"x\";>;",
    "float f<string s = \"a\"; float4 c = float4(1, 2, 3, 4); int2x2 m = {1, 2, 3, 4};>;",
    "float f<uniform float v;>;",
    "float f<float v = 1.0;;;>;",
    "float f",
    "float f<",
    "float f[",
    "uniform float4x4 ViewProj;",
];

#[test]
fn fixed_texts() {
    // Every text reaches ep_parse_param but the two that end right after
    // the name ("uniform float f" and "float f").
    let reached = FIXED
        .iter()
        .filter(|text| declaration(text).is_some())
        .count();
    assert_eq!(reached, FIXED.len() - 2, "texts that reach ep_parse_param");
    for text in FIXED {
        if let Some((start, decl)) = declaration(text) {
            for flags in 0..8 {
                let decl = Decl {
                    is_property: flags & 1 != 0,
                    is_const: flags & 2 != 0,
                    is_uniform: flags & 4 != 0,
                    ..decl.clone()
                };
                check(text, start, &decl).unwrap();
            }
        }
    }
}

#[test]
fn fixed_texts_cut_short() {
    // Every prefix of every text: the end of the text at each step of the
    // declaration.
    for text in FIXED {
        for end in 1..text.len() {
            if !text.is_char_boundary(end) {
                continue;
            }
            let text = &text[..end];
            if let Some((start, decl)) = declaration(text) {
                check(text, start, &decl).unwrap();
            }
        }
    }
}

#[test]
fn fixed_texts_from_every_token() {
    // The declaration from any token and as any type, as both sides take
    // it: ep_parse_param need not start after a name.
    for text in FIXED {
        if let Some((start, _)) = declaration(text) {
            for type_name in TYPES {
                check(text, start, &decl(type_name, 4)).unwrap();
            }
        }
        for start in 0..32_usize {
            let type_name = TYPES[start % TYPES.len()];
            check(text, start, &decl(type_name, (start % 8) as u8)).unwrap();
        }
    }
}

/// The shim's own value of `text`'s declaration: its first param line.
fn shim_param(text: &str) -> String {
    let (start, decl) = declaration(text).unwrap();
    let (_, shim) = run(text, start, &decl).unwrap();
    shim.lines()
        .find(|line| line.starts_with("param "))
        .unwrap_or("dropped")
        .to_owned()
}

#[test]
fn int_values_past_int_max() {
    // The number reads' intentional difference (#3464): past INT_MAX the C
    // depends on the width of long; the port saturates to i32 everywhere.
    assert!(shim_param("int i = 2147483648;").contains("{ff ff ff 7f}"));
    assert!(shim_param("int i = -2147483649;").contains("{00 00 00 80}"));
    assert!(shim_param("float a[2147483648];").contains("count 2147483647"));
}

#[test]
fn null_parser() {
    // SAFETY: a null parser does nothing (ep_parse_other() never passes
    // one; the C dereferences it).
    unsafe {
        rs::ep_parse_param(
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            false,
            false,
            false,
        );
    }
}

fn soup() -> impl Strategy<Value = String> {
    let word = prop::sample::select(vec![
        "uniform",
        "const",
        "float",
        "float2",
        "float4x4",
        "int",
        "int2",
        "bool",
        "string",
        "texture2d",
        "foo",
        "v",
        "true",
        "false",
        "=",
        "{",
        "}",
        "(",
        ")",
        "[",
        "]",
        "<",
        ">",
        ";",
        ",",
        ":",
        "-",
        "1",
        "0",
        "2.5",
        "1e3",
        ".5",
        "007",
        "\"s\"",
        "'c'",
        "\"a\\\"b\"",
        " ",
        "\n",
        "\t",
        "//c\n",
        "/*c*/",
    ]);
    prop::collection::vec(word, 0..32).prop_map(|words| words.join(" "))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    #[test]
    fn random_declarations(
        type_name in prop::sample::select(TYPES),
        flags in 0u8..8,
        body in soup(),
    ) {
        let text = format!("{type_name} p{body}");
        if let Some((start, mut d)) = declaration(&text) {
            d.is_property = flags & 1 != 0;
            d.is_const = flags & 2 != 0;
            d.is_uniform = flags & 4 != 0;
            check(&text, start, &d)?;
        }
    }

    #[test]
    fn random_starts(
        type_name in prop::sample::select(TYPES),
        body in soup(),
        start in 0usize..40,
    ) {
        check(&body, start, &decl(type_name, 0))?;
    }
}
