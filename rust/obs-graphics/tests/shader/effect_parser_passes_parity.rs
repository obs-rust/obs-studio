//! Tier 2 layouts and Tier 3 original-C parity for the effect parser's
//! passes and techniques (`libobs/graphics/effect-parser-passes.c.h`,
//! `obs_graphics::ffi::effect_parser`).
//!
//! Both sides get the same tokens: the oracle's C lexer and preprocessor
//! (`cf_parser_parse()`) make them twice from the same text, and each side
//! runs `ep_parse_technique` from the same token. The oracle is the
//! original C with the C `cf_adderror`, `cf_pass_pair` and `strref_cmp`;
//! the shim reaches libobs's `cf_adderror` and `cf_pass_pair`, which are
//! obs-util's Rust here as in the hybrid libobs. Every technique and pass
//! they push (each field, the names, darray counts and capacities), every
//! program token, the other collections, every error (text, file, row,
//! column, level) and where the cursor stopped must match. A program token
//! is compared as what it is a copy of: the index of the token in its own
//! parser's token array whose every field it equals (the pointers into the
//! lexer's text included), or the zeroed end token; so each side's copies
//! are exact copies of the same tokens. No intentional differences: the C frees a dropped
//! technique's name on every path since #3598, and so does the port. Texts
//! hold no `#`: the preprocessor's directives are not the techniques' (each
//! side gets the same tokens either way).

use core::ffi::{CStr, c_char};
use core::mem::{align_of, offset_of, size_of};
use std::ffi::CString;

use obs_c_oracle::effect_parser_passes as c;
use obs_graphics::ffi::effect_parser::{self as rs, ep_pass, ep_technique};
use obs_graphics::ffi::effect_parser_lifecycle::effect_parser;
use obs_util::ffi::cf_tokenizer::cf_token;
use obs_util::ffi::darray::darray;
use obs_util::ffi::lexer::error_item;
use proptest::prelude::*;

#[test]
fn layouts() {
    let rust = [
        size_of::<ep_pass>(),
        align_of::<ep_pass>(),
        offset_of!(ep_pass, name),
        offset_of!(ep_pass, vertex_program),
        offset_of!(ep_pass, fragment_program),
        offset_of!(ep_pass, pass),
        size_of::<ep_technique>(),
        align_of::<ep_technique>(),
        offset_of!(ep_technique, name),
        offset_of!(ep_technique, passes),
        size_of::<cf_token>(),
        align_of::<cf_token>(),
        offset_of!(cf_token, lex),
        offset_of!(cf_token, str),
        offset_of!(cf_token, unmerged_str),
        offset_of!(cf_token, r#type),
        size_of::<core::ffi::c_int>(),
    ];
    let mut oracle = [0usize; 32];
    // SAFETY: the oracle writes fewer than 32 values.
    let n = unsafe { c::oracle_effect_parser_passes_layout(oracle.as_mut_ptr()) };
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
            c::oracle_effect_parser_passes_prepare((&raw mut *ep).cast(), text.as_ptr(), start)
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
            let first = c::oracle_effect_parser_passes_tokens(self.ptr().cast());
            core::slice::from_raw_parts(first.cast::<cf_token>(), self.count + 1)
        }
    }
}

impl Drop for Prepared {
    fn drop(&mut self) {
        // SAFETY: an initialized parser; the oracle frees what either side
        // pushed (all bmalloc'd) as ep_free() does.
        unsafe { c::oracle_effect_parser_passes_free(self.ptr().cast()) };
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

/// Every field of the token: what `da_push_back` copies (its padding
/// aside).
fn fields(token: &cf_token) -> (usize, usize, usize, usize, usize, i32) {
    (
        token.lex as usize,
        token.str.array as usize,
        token.str.len,
        token.unmerged_str.array as usize,
        token.unmerged_str.len,
        token.r#type,
    )
}

/// A program token as what it is a copy of: `#<index>` of the token whose
/// every field it equals, `end` for the zeroed end token, or `?`.
fn token_name(tokens: &[cf_token], token: &cf_token) -> String {
    if fields(token) == (0, 0, 0, 0, 0, 0) {
        return "end".into();
    }
    match tokens.iter().position(|t| fields(t) == fields(token)) {
        Some(i) => format!("#{i}"),
        None => "?".into(),
    }
}

/// Everything either side can leave in the parser, one line each.
fn snapshot(p: &mut Prepared) -> String {
    let position = {
        let first = p.tokens().as_ptr();
        // SAFETY: cur_token points into the same token array.
        unsafe { p.ep.cfp.cur_token.cast_const().offset_from(first) }
    };
    let tokens: Vec<cf_token> = p.tokens().to_vec();
    let ep = &*p.ep;
    let mut out = format!(
        "at {position}\nparams {} structs {} funcs {} samplers {} files {} tokens {}\ntechniques {}\neffect {:?} pass {:?}\n",
        shape(&ep.params),
        shape(&ep.structs),
        shape(&ep.funcs),
        shape(&ep.samplers),
        shape(&ep.files),
        shape(&ep.tokens),
        shape(&ep.techniques),
        ep.effect,
        ep.cur_pass,
    );
    // SAFETY: each darray holds `num` items of its C type, every string a
    // bmalloc'd C string or null.
    unsafe {
        for ept in items::<ep_technique>(&ep.techniques) {
            out += &format!(
                "technique {} passes {}\n",
                text(ept.name),
                shape(&ept.passes)
            );
            for pass in items::<ep_pass>(&ept.passes) {
                out += &format!(
                    "  pass {} vs {} ps {} pass {:?}\n",
                    text(pass.name),
                    shape(&pass.vertex_program),
                    shape(&pass.fragment_program),
                    pass.pass
                );
                for (label, program) in
                    [("vs", &pass.vertex_program), ("ps", &pass.fragment_program)]
                {
                    let names: Vec<String> = items::<cf_token>(program)
                        .iter()
                        .map(|token| token_name(&tokens, token))
                        .collect();
                    out += &format!("    {label} {}\n", names.join(" "));
                }
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

/// Both sides from token `start` of `text`. `None` when the C parse fails.
fn run(text: &str, start: usize) -> Option<(String, String)> {
    let text = CString::new(text).ok()?;
    let mut oracle = Prepared::new(&text, start)?;
    let mut shim = Prepared::new(&text, start)?;
    // SAFETY: both are initialized parsers at a token of their stream.
    unsafe {
        c::oracle_ep_parse_technique(oracle.ptr().cast());
        rs::ep_parse_technique(shim.ptr());
    }
    Some((snapshot(&mut oracle), snapshot(&mut shim)))
}

fn check(text: &str, start: usize) -> Result<(), TestCaseError> {
    if let Some((oracle, shim)) = run(text, start) {
        prop_assert!(
            !oracle.contains(" ?"),
            "a token copy matches no token: {}",
            oracle
        );
        prop_assert_eq!(oracle, shim, "{:?} from token {}", text, start);
    }
    Ok(())
}

/// The texts of `test_effect_parser_passes` and of the Tier 1 tests, with
/// techniques of libobs's own effects.
const FIXED: &[&str] = &[
    "technique Draw { pass { vertex_shader = VSDefault(vert_in); pixel_shader = PSDrawBare(vert_in); } }",
    "technique T { pass P0 { vertex_program = VS(v); pixel_program = PS(v); } }",
    "technique T { pass P0 { vertex_shader = VS(v); } pass P1 { pixel_shader = PS(v); } }\ntechnique U { pass { pixel_program = PS2(v, 1.0); vertex_program = VS2(v); } }\nuniform float c;",
    "technique T { pass { vertex_shader = VS(v); vertex_shader = VS2(v); } }",
    "technique T { pass { } }",
    "technique T { pass {} }",
    "technique T { }",
    "technique T {}\nuniform float c;",
    "technique Draw\n{\n\tpass\n\t{\n\t\tvertex_shader = VSDefault(vert_in);\n\t\tpixel_shader  = PSDrawBare(vert_in);\n\t}\n}",
    "technique T { pass { vertex_shader = compile VS(v); pixel_shader = compile PS(v); } }",
    "technique T { pass 1 { vertex_shader = VS(v); } }",
    "technique T { pass { vertex_shader = 1 + { x ); } }",
    "technique T { pass { vertex_shader = ; } }",
    "technique U { pass { pixel_program = PS2(v, 1.0); vertex_program = VS2(v); } }\nuniform float c;",
    "technique T { pass { vertex_shader = VS(v) } }\nuniform float c;",
    "technique T { pass { vertex_shader = VS(v) } pass Q { pixel_shader = PS(v); } }\nuniform float c;",
    "technique T { pass { geometry_shader = GS(v); vertex_shader = VS(v); } }",
    "technique T { pass { geometry_shader = GS(v) } }\nuniform float c;",
    "technique T { pass { vertex_shader VS(v); pixel_shader = PS(v); } }",
    "technique T { pass { vertex_shader VS(v) } }\nuniform float c;",
    "technique T { pass { ; vertex_shader = VS(v); } }",
    "technique T { pass { vertex_shader = compile ; } }",
    "technique T { pass vertex_shader = VS(v); } }\nuniform float c;",
    "technique T { pass P Q { vertex_shader = VS(v); } }\nuniform float c;",
    "technique T { pass { vertex_shader = VS(v); }; }\nuniform float c;",
    "technique T { foo { } pass { vertex_shader = VS(v); } }\nuniform float c;",
    "technique T { foo; pass { vertex_shader = VS(v); } }\nuniform float c;",
    "technique T { pass { vertex_shader = VS(v); } foo }\nuniform float c;",
    "technique T { ; pass { vertex_shader = VS(v); } }\nuniform float c;",
    "technique T pass { vertex_shader = VS(v); } };\nuniform float c;",
    "technique T;\nuniform float c;",
    "technique T pass { vertex_shader = VS(v); } }\nuniform float c",
    "technique { pass { vertex_shader = VS(v); } }\nuniform float c;",
    "technique 1 { pass { vertex_shader = VS(v); } }\nuniform float c;",
    "technique T { pass { vertex_shader = VS(v); } }; uniform float c;",
    "technique T { pass { vertex_shader = VS(v); } } uniform float c;",
    "technique",
    "technique T",
    "technique T pass",
    "technique T {",
    "technique T { pass",
    "technique T { pass P",
    "technique T { pass {",
    "technique T { pass P {",
    "technique T { pass { vertex_shader",
    "technique T { pass { vertex_shader =",
    "technique T { pass { vertex_shader = compile",
    "technique T { pass { vertex_shader = VS(v)",
    "technique T { pass { vertex_shader = VS(v);",
    "technique T { pass { vertex_shader = VS(v); }",
    "technique T { pass { geometry_shader",
    "technique T { pass { geometry_shader = GS(v)",
    "technique T { foo",
    "technique T { foo {",
    "technique Draw { pass { vertex_shader = VSDefault(vert_in); pixel_shader = PSDraw(vert_in); } }\ntechnique Other { pass { vertex_shader = VSDefault(vert_in); } }",
    "technique Other { pass { vertex_shader = VSDefault(vert_in); } }",
    "technique T { pass { vertex_shader = VS(v) } }\ntechnique U { pass { pixel_shader = PS(v); } }",
    "technique T\ntechnique U { pass { pixel_shader = PS(v); } }",
    "technique T { pass { vertex_shader = VS(v) } } }",
    "technique T { pass { pixel_shader = A; vertex_program = B; pixel_program = C; } }",
    "technique T { pass { vertex_shader = VS(\"x;\", 'y', 0.5); } }",
    "technique T { pass a {} pass {} pass \"b\" {} }",
    "technique T { pass { vertex_shader = compile compile VS(v); } }",
    "technique T { pass { vertex_shader = VS( // c\n v /* d */ ); } }",
    "technique T { pass { vertex_shader VS(v) } pass { pixel_shader = PS(v); } }",
    "technique T { pass { geometry_shader = { x; ",
    "technique T { foo { pass { } ",
    "technique T { pass } { vertex_shader = VS(v); } }",
    "technique 1",
    "technique 1 { pass {} } x",
    "}\ntechnique T { pass { vertex_shader = VS(v); } }",
    "struct VertInOut { float4 pos : POSITION; };\nuniform float4x4 ViewProj;\nVertInOut VSDefault(VertInOut vert_in) { return vert_in; }\ntechnique Draw { pass { vertex_shader = VSDefault(vert_in); pixel_shader = PSDraw(vert_in); } }\ntechnique Other { pass { vertex_shader = VSDefault(vert_in); } }",
    "technique Draw\n{\n\tpass\n\t{\n\t\tvertex_shader = VSDefault(vert_in);\n\t\tpixel_shader  = PSDrawBare(vert_in);\n\t}\n}\n\ntechnique DrawAlphaDivide\n{\n\tpass\n\t{\n\t\tvertex_shader = VSDefault(vert_in);\n\t\tpixel_shader  = PSDrawAlphaDivide(vert_in);\n\t}\n}",
    "technique Draw\n{\n\tpass p0\n\t{\n\t\tvertex_shader = VSDefault(v_in);\n\t\tpixel_shader  = PSDrawLowresBilinearRGBA(v_in);\n\t}\n\tpass p1\n\t{\n\t\tvertex_shader = VSDefault(v_in);\n\t\tpixel_shader  = PSDrawLowresBilinearRGBA(v_in);\n\t}\n}",
];

#[test]
fn fixed_texts() {
    for text in FIXED {
        check(text, 0).unwrap();
    }
}

#[test]
fn fixed_texts_cut_short() {
    // Every prefix of every text: the end of the text at each step of the
    // technique.
    for text in FIXED {
        for end in 1..text.len() {
            if text.is_char_boundary(end) {
                check(&text[..end], 0).unwrap();
            }
        }
    }
}

#[test]
fn fixed_texts_from_every_token() {
    // The technique from any token, as both sides take it: the keyword
    // need not be where it starts.
    for text in FIXED {
        for start in 0..48 {
            check(text, start).unwrap();
        }
    }
}

#[test]
fn null_parser() {
    // SAFETY: a null parser does nothing.
    unsafe { rs::ep_parse_technique(core::ptr::null_mut()) };
}

fn soup() -> impl Strategy<Value = String> {
    let word = prop::sample::select(vec![
        "technique",
        "pass",
        "vertex_shader",
        "vertex_program",
        "pixel_shader",
        "pixel_program",
        "compile",
        "T",
        "VS",
        "v",
        "1",
        "0.5",
        "\"x\"",
        "'y'",
        "\";\"",
        "=",
        ";",
        "{",
        "}",
        "(",
        ")",
        ",",
        " ",
        "\n",
        "\t",
        "\r\n",
        "//c\n",
        "/*c*/",
    ]);
    prop::collection::vec(word, 0..32).prop_map(|words| words.concat())
}

/// A well-formed technique with up to three passes, its tokens dropped,
/// replaced or joined by others at random: the shapes the soup rarely
/// reaches.
fn technique() -> impl Strategy<Value = String> {
    let pass = (
        prop::option::of(prop::sample::select(vec!["P", "1", "\"p\""])),
        prop::collection::vec(
            (
                prop::sample::select(vec![
                    "vertex_shader",
                    "vertex_program",
                    "pixel_shader",
                    "pixel_program",
                    "geometry_shader",
                ]),
                any::<bool>(),
                prop::sample::select(vec!["VS(v)", "PS(v, 1.0)", "", "{ x }", "a b c"]),
            ),
            0..3,
        ),
    )
        .prop_map(|(name, commands)| {
            let mut s = format!("pass {} {{ ", name.unwrap_or(""));
            for (command, compile, call) in commands {
                let compile = if compile { "compile " } else { "" };
                s += &format!("{command} = {compile}{call}; ");
            }
            s + "} "
        });
    (
        prop::collection::vec(pass, 0..3),
        prop::collection::vec(
            (
                any::<prop::sample::Index>(),
                prop::sample::select(vec!["", ";", "}", "{", "=", "pass", "x"]),
            ),
            0..3,
        ),
    )
        .prop_map(|(passes, edits)| {
            let mut words: Vec<String> = format!("technique T {{ {}}}", passes.concat())
                .split(' ')
                .map(String::from)
                .collect();
            for (at, word) in edits {
                let i = at.index(words.len());
                words[i] = word.into();
            }
            words.join(" ")
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    #[test]
    fn random_techniques(sep in prop::sample::select(vec!["", " ", "\n"]), body in soup()) {
        check(&format!("technique{sep}{body}"), 0)?;
    }

    #[test]
    fn random_starts(body in soup(), start in 0usize..40) {
        check(&body, start)?;
    }

    #[test]
    fn edited_techniques(text in technique()) {
        check(&text, 0)?;
    }
}
