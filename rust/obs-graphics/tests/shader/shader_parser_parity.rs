//! Tier 2 layouts and Tier 3 original-C parity for the shader parser's
//! sampler-state and struct blocks (`libobs/graphics/shader-parser-blocks.c.h`,
//! `obs_graphics::ffi::shader_parser`).
//!
//! Both sides get the same tokens: the oracle's C lexer and preprocessor
//! (`cf_parser_parse()`) make them twice from the same text, and each side
//! runs its block from the same token. The oracle is the original C with
//! the C `cf_adderror`, `cf_pass_pair` and `strref_cmp`; the shim reaches
//! libobs's, which are obs-util's Rust here as in the hybrid libobs. Every
//! sampler, struct and variable they push (each field, strings, darray
//! counts and capacities), the other collections, every error (text, file,
//! row, column, level) and where the cursor stopped must match. No
//! intentional differences. Texts hold no `#`: the preprocessor's
//! directives are not these blocks' (each side gets the same tokens either
//! way).

use core::ffi::{CStr, c_char};
use core::mem::{align_of, offset_of, size_of};
use std::ffi::CString;

use obs_c_oracle::shader_parser_blocks as c;
use obs_graphics::ffi::shader_parser::{self as rs, shader_parser, shader_struct, shader_var};
use obs_graphics::ffi::shader_sampler_convert::shader_sampler;
use obs_util::ffi::cf_tokenizer::cf_token;
use obs_util::ffi::darray::darray;
use obs_util::ffi::lexer::error_item;
use proptest::prelude::*;

#[test]
fn layouts() {
    let rust = [
        size_of::<shader_var>(),
        align_of::<shader_var>(),
        offset_of!(shader_var, r#type),
        offset_of!(shader_var, name),
        offset_of!(shader_var, mapping),
        offset_of!(shader_var, var_type),
        offset_of!(shader_var, array_count),
        offset_of!(shader_var, gl_sampler_id),
        offset_of!(shader_var, default_val),
        size_of::<shader_struct>(),
        align_of::<shader_struct>(),
        offset_of!(shader_struct, name),
        offset_of!(shader_struct, vars),
        size_of::<shader_sampler>(),
        align_of::<shader_sampler>(),
        offset_of!(shader_sampler, name),
        offset_of!(shader_sampler, states),
        offset_of!(shader_sampler, values),
        size_of::<shader_parser>(),
        align_of::<shader_parser>(),
        offset_of!(shader_parser, cfp),
        offset_of!(shader_parser, params),
        offset_of!(shader_parser, structs),
        offset_of!(shader_parser, samplers),
        offset_of!(shader_parser, funcs),
        size_of::<cf_token>(),
        align_of::<cf_token>(),
        offset_of!(cf_token, lex),
        offset_of!(cf_token, str),
        offset_of!(cf_token, unmerged_str),
        offset_of!(cf_token, r#type),
    ];
    let mut oracle = [0usize; 64];
    // SAFETY: the oracle writes fewer than 64 values.
    let n = unsafe { c::oracle_shader_parser_blocks_layout(oracle.as_mut_ptr()) };
    assert_eq!(&rust[..], &oracle[..n]);
}

/// A `shader_parser` the oracle's C lexed and preprocessed, at one token.
struct Prepared(Box<shader_parser>);

impl Prepared {
    /// `None` when the C parse fails.
    fn new(text: &CStr, start: usize) -> Option<(Self, usize)> {
        // SAFETY: every field of shader_parser permits zero; the oracle
        // initializes it before anything reads it.
        let mut sp: Box<shader_parser> = Box::new(unsafe { core::mem::zeroed() });
        // SAFETY: a boxed parser that stays put, and a C string the lexer
        // copies.
        let count = unsafe {
            c::oracle_shader_parser_blocks_prepare((&raw mut *sp).cast(), text.as_ptr(), start)
        };
        let prepared = Self(sp);
        (count != 0).then_some((prepared, count))
    }

    fn ptr(&mut self) -> *mut shader_parser {
        &raw mut *self.0
    }
}

impl Drop for Prepared {
    fn drop(&mut self) {
        // SAFETY: an initialized parser; shader_parser_free frees what
        // either side pushed (all bmalloc'd).
        unsafe { c::oracle_shader_parser_blocks_free(self.ptr().cast()) };
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

/// Everything either side can leave in the parser, one line each.
fn snapshot(p: &mut Prepared) -> String {
    // SAFETY: the parser is initialized; the oracle reports its position.
    let position = unsafe { c::oracle_shader_parser_blocks_position(p.ptr().cast()) };
    let sp = &*p.0;
    let mut out = format!(
        "at {position}\nparams {} funcs {}\nsamplers {}\nstructs {}\n",
        shape(&sp.params),
        shape(&sp.funcs),
        shape(&sp.samplers),
        shape(&sp.structs)
    );
    // SAFETY: each darray holds `num` items of its C type, every string a
    // bmalloc'd C string or null.
    unsafe {
        for ss in items::<shader_sampler>(&sp.samplers) {
            out += &format!(
                "sampler {} states {} values {}\n",
                text(ss.name),
                shape(&ss.states),
                shape(&ss.values)
            );
            for (state, value) in items::<*mut c_char>(&ss.states)
                .iter()
                .zip(items::<*mut c_char>(&ss.values))
            {
                out += &format!("  {} = {}\n", text(*state), text(*value));
            }
        }
        for st in items::<shader_struct>(&sp.structs) {
            out += &format!("struct {} vars {}\n", text(st.name), shape(&st.vars));
            for var in items::<shader_var>(&st.vars) {
                out += &format!(
                    "  {} {} : {} kind {} count {} gl {} default {}\n",
                    text(var.r#type),
                    text(var.name),
                    text(var.mapping),
                    var.var_type,
                    var.array_count,
                    var.gl_sampler_id,
                    shape(&var.default_val)
                );
            }
        }
        for e in items::<error_item>(&sp.cfp.error_list.errors) {
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

#[derive(Clone, Copy, Debug)]
enum Block {
    Sampler,
    Struct,
}

/// Both sides from token `start` of `text`. `None` when the C parse fails.
fn run(text: &str, start: usize, block: Block) -> Option<(String, String)> {
    let text = CString::new(text).ok()?;
    let (mut oracle, _) = Prepared::new(&text, start)?;
    let (mut shim, _) = Prepared::new(&text, start)?;
    // SAFETY: both are initialized parsers at a token of their stream.
    unsafe {
        match block {
            Block::Sampler => {
                c::oracle_sp_parse_sampler_state(oracle.ptr().cast());
                rs::sp_parse_sampler_state(shim.ptr());
            }
            Block::Struct => {
                c::oracle_sp_parse_struct(oracle.ptr().cast());
                rs::sp_parse_struct(shim.ptr());
            }
        }
    }
    Some((snapshot(&mut oracle), snapshot(&mut shim)))
}

fn check(text: &str, start: usize, block: Block) -> Result<(), TestCaseError> {
    if let Some((oracle, shim)) = run(text, start, block) {
        prop_assert_eq!(
            oracle,
            shim,
            "{:?} from token {} as {:?}",
            text,
            start,
            block
        );
    }
    Ok(())
}

/// The texts of `test_shader_parser_blocks` and of the Tier 1 tests, with
/// a few whole shaders' blocks.
const FIXED: &[&str] = &[
    "sampler_state def_sampler { Filter = Linear; AddressU = Clamp; AddressV = Wrap; };\nuniform float4 c;",
    "sampler_state a { Filter = Point; };\nsampler_state b { MaxAnisotropy = 4; BorderColor = 00ff00ff; };",
    "sampler_state s { };",
    "sampler_state s {};",
    "sampler_state s { Filter = Linear; Filter = Point; };",
    "sampler_state s { Filter Linear; AddressU = Wrap; };",
    "sampler_state s { Filter = Linear AddressU = Wrap; };",
    "sampler_state s { Filter = ; AddressU = Wrap; };",
    "sampler_state s { 1 = Linear; AddressU = Wrap; };",
    "sampler_state 1 { Filter = Linear; };\nuniform float a;",
    "sampler_state s Filter = Linear;\nuniform float a;",
    "sampler_state s { Filter = Linear; }\nuniform float a;",
    "sampler_state s { Filter = Linear; } uniform float a;",
    "sampler_state s { BorderColor = \"x\"; MaxAnisotropy = 16; };",
    "sampler_state\n\ts\n{\n\tFilter\t=\tLinear\t;\n}\n;",
    "sampler_state s { Filter { a; }; AddressU = Wrap; };",
    "sampler_state s { Filter = Linear; } ;",
    "sampler_state textureSampler {\n\tFilter    = Linear;\n\tAddressU  = Border;\n\tAddressV  = Border;\n\tBorderColor = 00000000;\n};",
    "struct VertInOut {\n\tfloat4 pos : POSITION;\n\tfloat2 uv : TEXCOORD0;\n\tfloat4 color;\n};\nuniform float4x4 ViewProj;",
    "struct A { float a; };\nstruct B { int b : X; float4x4 m; };",
    "struct S { };",
    "struct S {};",
    "struct S { ; float a; ; };",
    "struct S { 1 a; float b; };",
    "struct S { float ; float b; };",
    "struct S { float }; uniform float a;",
    "struct S { float a : ; float b; };",
    "struct S { float a : }; uniform float c;",
    "struct S { float a : 1; float b; };",
    "struct S { float a b; float c; };",
    "struct S { float a b }; uniform float c;",
    "struct S { float a : POSITION b; float c; };",
    "struct 1 { float a; };\nuniform float b;",
    "struct S float a; };\nuniform float b;",
    "struct S { float a; }\nuniform float b;",
    "struct S { float a : P };",
    "struct S { float a b {c;}; float d; };",
    "struct S { float a b {c;} ; float d; };",
    "struct S { float a : P : Q; float b; };",
    "struct S { float a b { { c; } }; float d; };",
    "struct S { float a b { c; ",
    "struct FragData {\n\tfloat4 pos : POSITION; // comment\n\t/* block */ float2 uv : TEXCOORD0;\n};",
];

fn block_of(text: &str) -> Block {
    if text.starts_with("struct") {
        Block::Struct
    } else {
        Block::Sampler
    }
}

#[test]
fn fixed_texts() {
    for text in FIXED {
        check(text, 0, block_of(text)).unwrap();
    }
}

#[test]
fn fixed_texts_cut_short() {
    // Every prefix of every text: the end of the text at each step of
    // each block.
    for text in FIXED {
        for end in 1..text.len() {
            if text.is_char_boundary(end) {
                check(&text[..end], 0, block_of(text)).unwrap();
            }
        }
    }
}

#[test]
fn fixed_texts_from_every_token() {
    // Each block from any token, as both sides take it: the keyword need
    // not be where it starts.
    for text in FIXED {
        for start in 0..64 {
            check(text, start, Block::Sampler).unwrap();
            check(text, start, Block::Struct).unwrap();
        }
    }
}

#[test]
fn null_parser() {
    // SAFETY: a null parser does nothing.
    unsafe {
        rs::sp_parse_sampler_state(core::ptr::null_mut());
        rs::sp_parse_struct(core::ptr::null_mut());
    }
}

fn soup() -> impl Strategy<Value = String> {
    let word = prop::sample::select(vec![
        "sampler_state",
        "struct",
        "s",
        "Filter",
        "Linear",
        "float",
        "a",
        "POSITION",
        "1",
        "0.5",
        "\"x\"",
        "'y'",
        "=",
        ";",
        ":",
        "{",
        "}",
        "(",
        ")",
        ",",
        "-",
        " ",
        "\n",
        "\t",
        "\r\n",
        "//c\n",
        "/*c*/",
    ]);
    prop::collection::vec(word, 0..32).prop_map(|words| words.concat())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    #[test]
    fn random_blocks(sampler in any::<bool>(), sep in prop::sample::select(vec!["", " ", "\n"]), body in soup()) {
        let (keyword, block) = if sampler { ("sampler_state", Block::Sampler) } else { ("struct", Block::Struct) };
        check(&format!("{keyword}{sep}{body}"), 0, block)?;
    }

    #[test]
    fn random_starts(sampler in any::<bool>(), body in soup(), start in 0usize..40) {
        let block = if sampler { Block::Sampler } else { Block::Struct };
        check(&body, start, block)?;
    }
}
