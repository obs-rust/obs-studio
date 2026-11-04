//! Tier 2 layouts and Tier 3 original-C parity for the effect parser's
//! struct and sampler-state blocks
//! (`libobs/graphics/effect-parser-blocks.c.h`,
//! `obs_graphics::ffi::effect_parser`).
//!
//! Both sides get the same tokens: the oracle's C lexer and preprocessor
//! (`cf_parser_parse()`) make them twice from the same text, and each side
//! runs its block from the same token. The oracle is the original C with
//! the C `cf_adderror`, `cf_pass_pair`, `strref_cmp` and `dstr_ncat`; the
//! shim reaches libobs's `cf_adderror` and `cf_pass_pair`, which are
//! obs-util's Rust here as in the hybrid libobs. Every struct, variable and
//! sampler they push (each field, strings, darray counts and capacities),
//! the other collections, every error (text, file, row, column, level) and
//! where the cursor stopped must match. A kept sampler value's own
//! allocation size is not compared: the C grows it as a `dstr`, the shim
//! copies it once, and `ep_free()` frees either. No intentional
//! differences: the C frees a dropped item's state name and value on every
//! path since #3550, and so does the port. Texts hold no `#`: the
//! preprocessor's directives are not these blocks' (each side gets the same
//! tokens either way).

use core::ffi::{CStr, c_char};
use core::mem::{align_of, offset_of, size_of};
use std::ffi::CString;

use obs_c_oracle::effect_parser_blocks as c;
use obs_graphics::ffi::effect_parser::{self as rs, ep_sampler, ep_struct, ep_var};
use obs_graphics::ffi::effect_parser_lifecycle::effect_parser;
use obs_util::ffi::darray::darray;
use obs_util::ffi::lexer::error_item;
use proptest::prelude::*;

#[test]
fn layouts() {
    let rust = [
        size_of::<ep_var>(),
        align_of::<ep_var>(),
        offset_of!(ep_var, r#type),
        offset_of!(ep_var, name),
        offset_of!(ep_var, mapping),
        offset_of!(ep_var, var_type),
        size_of::<core::ffi::c_int>(),
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
    ];
    let mut oracle = [0usize; 64];
    // SAFETY: the oracle writes fewer than 64 values.
    let n = unsafe { c::oracle_effect_parser_blocks_layout(oracle.as_mut_ptr()) };
    assert_eq!(&rust[..], &oracle[..n]);
}

/// An `effect_parser` the oracle's C lexed and preprocessed, at one token.
struct Prepared(Box<effect_parser>);

impl Prepared {
    /// `None` when the C parse fails.
    fn new(text: &CStr, start: usize) -> Option<Self> {
        // SAFETY: every field of effect_parser permits zero; the oracle
        // initializes it before anything reads it.
        let mut ep: Box<effect_parser> = Box::new(unsafe { core::mem::zeroed() });
        // SAFETY: a boxed parser that stays put, and a C string the lexer
        // copies.
        let count = unsafe {
            c::oracle_effect_parser_blocks_prepare((&raw mut *ep).cast(), text.as_ptr(), start)
        };
        let prepared = Self(ep);
        (count != 0).then_some(prepared)
    }

    fn ptr(&mut self) -> *mut effect_parser {
        &raw mut *self.0
    }
}

impl Drop for Prepared {
    fn drop(&mut self) {
        // SAFETY: an initialized parser; the oracle frees what either side
        // pushed (all bmalloc'd) as ep_free() does.
        unsafe { c::oracle_effect_parser_blocks_free(self.ptr().cast()) };
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
    let position = unsafe { c::oracle_effect_parser_blocks_position(p.ptr().cast()) };
    let ep = &*p.0;
    let mut out = format!(
        "at {position}\nparams {} funcs {} techniques {} files {} tokens {}\nstructs {}\nsamplers {}\neffect {:?} pass {:?}\n",
        shape(&ep.params),
        shape(&ep.funcs),
        shape(&ep.techniques),
        shape(&ep.files),
        shape(&ep.tokens),
        shape(&ep.structs),
        shape(&ep.samplers),
        ep.effect,
        ep.cur_pass,
    );
    // SAFETY: each darray holds `num` items of its C type, every string a
    // bmalloc'd C string or null.
    unsafe {
        for st in items::<ep_struct>(&ep.structs) {
            out += &format!(
                "struct {} vars {} written {}\n",
                text(st.name),
                shape(&st.vars),
                st.written
            );
            for var in items::<ep_var>(&st.vars) {
                out += &format!(
                    "  {} {} : {} kind {}\n",
                    text(var.r#type),
                    text(var.name),
                    text(var.mapping),
                    var.var_type
                );
            }
        }
        for es in items::<ep_sampler>(&ep.samplers) {
            out += &format!(
                "sampler {} states {} values {} written {}\n",
                text(es.name),
                shape(&es.states),
                shape(&es.values),
                es.written
            );
            for (state, value) in items::<*mut c_char>(&es.states)
                .iter()
                .zip(items::<*mut c_char>(&es.values))
            {
                out += &format!("  {} = {}\n", text(*state), text(*value));
            }
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

#[derive(Clone, Copy, Debug)]
enum Block {
    Struct,
    Sampler,
}

/// Both sides from token `start` of `text`. `None` when the C parse fails.
fn run(text: &str, start: usize, block: Block) -> Option<(String, String)> {
    let text = CString::new(text).ok()?;
    let mut oracle = Prepared::new(&text, start)?;
    let mut shim = Prepared::new(&text, start)?;
    // SAFETY: both are initialized parsers at a token of their stream.
    unsafe {
        match block {
            Block::Struct => {
                c::oracle_ep_parse_struct(oracle.ptr().cast());
                rs::ep_parse_struct(shim.ptr());
            }
            Block::Sampler => {
                c::oracle_ep_parse_sampler_state(oracle.ptr().cast());
                rs::ep_parse_sampler_state(shim.ptr());
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

/// The texts of `test_effect_parser_blocks` and of the Tier 1 tests, with
/// the blocks of libobs's own effects.
const FIXED: &[&str] = &[
    "struct VertInOut { float4 pos : POSITION; float2 uv : TEXCOORD0; };\nuniform float4 c;",
    "struct A { float a; int b; };\nstruct B { float4 c : COLOR; };",
    "struct E { };",
    "struct E {};",
    "struct S { float a; float a; };",
    "struct S\n{\n\tfloat4 pos : POSITION;\n\tfloat2 uv  : TEXCOORD0;\n};",
    "struct S { float a;; float b; };",
    "struct { float a; };\nuniform float c;",
    "struct 1 { float a; };\nuniform float c;",
    "struct S float a; };\nuniform float c;",
    "struct S { 1 a; float b; };",
    "struct S { float ; float b; };",
    "struct S { float }; uniform float c;",
    "struct S { float 1; float b; };",
    "struct S { float a : ; float b; };",
    "struct S { float a : }; uniform float c;",
    "struct S { float a : 1; float b; };",
    "struct S { float : POSITION; float b; };",
    "struct S { float a b; float c; };",
    "struct S { float a : POSITION b; float c; };",
    "struct S { float a b }; uniform float c;",
    "struct S { float a; }\nuniform float c;",
    "struct S { float a; } x;\nuniform float c;",
    "}\nstruct S { float a; };",
    "struct S { float a : P };",
    "struct S { float a b {c;}; float d; };",
    "struct S { float a b {c;} ; float d; };",
    "struct S { float a : P : Q; float b; };",
    "struct S { float a b { { c; } }; float d; };",
    "struct S { float a b { c; ",
    "struct VertData {\n\tfloat4 pos : POSITION; // comment\n\t/* block */ float2 uv : TEXCOORD0;\n};",
    "sampler_state def_sampler { Filter = Linear; AddressU = Clamp; AddressV = Wrap; };\nuniform float4 c;",
    "sampler_state a { Filter = Point; };\nsampler_state b { MaxAnisotropy = 4; BorderColor = 00ff00ff; };",
    "sampler_state s { };",
    "sampler_state s {};",
    "sampler_state s { Filter = Linear; Filter = Point; };",
    "sampler_state s { BorderColor = 0 0 0 1; };",
    "sampler_state s\n{\n\tFilter   = Linear;\n\tAddressU = Clamp;\n};",
    "sampler_state s { Filter Linear; AddressU = Wrap; };",
    "sampler_state s { Filter = Linear AddressU = Wrap; };",
    "sampler_state s { Filter = ; AddressU = Wrap; };",
    "sampler_state s { Filter = ; };",
    "sampler_state s { 1 = Linear; AddressU = Wrap; };",
    "sampler_state s { ; AddressU = Wrap; };",
    "sampler_state { Filter = Linear; };\nuniform float c;",
    "sampler_state 1 { Filter = Linear; };\nuniform float c;",
    "sampler_state s Filter = Linear; };\nuniform float c;",
    "sampler_state s { Filter = Linear; }\nuniform float c;",
    "sampler_state s { Filter = Linear; } x;\nuniform float c;",
    "sampler_state s { Filter = Linear }; uniform float c;",
    "}\nsampler_state s { Filter = Linear; };",
    "sampler_state s { BorderColor = \"x\"; MaxLOD = -1.5; };",
    "sampler_state s { A = \";\" b; };",
    "sampler_state s { A =\n\t0\n\t1\t;\n};",
    "sampler_state s { A = { b; }; };",
    "sampler_state s { A = = b; };",
    "sampler_state s { A = ; B = ; C = c; };",
    "sampler_state s { A = 0 0 0 1 2 3 4 5 6 7 8 9 a b c d e f g h; };",
    "sampler_state s { A = /* c */ x // d\n y; };",
    "sampler_state def_sampler {\n\tFilter   = Linear;\n\tAddressU = Clamp;\n\tAddressV = Clamp;\n};",
    "struct VertInOut {\n\tfloat4 pos : POSITION;\n\tfloat2 uv  : TEXCOORD0;\n};\n\nsampler_state def_sampler {\n\tFilter   = Linear;\n\tAddressU = Clamp;\n\tAddressV = Clamp;\n};",
];

fn block_of(text: &str) -> Block {
    if text.trim_start_matches("}\n").starts_with("struct") {
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
            check(text, start, Block::Struct).unwrap();
            check(text, start, Block::Sampler).unwrap();
        }
    }
}

#[test]
fn null_parser() {
    // SAFETY: a null parser does nothing.
    unsafe {
        rs::ep_parse_struct(core::ptr::null_mut());
        rs::ep_parse_sampler_state(core::ptr::null_mut());
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
        "\";\"",
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
