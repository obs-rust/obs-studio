//! obs-graphics: the effect parser's parameter declarations, annotations and
//! value assignments (`libobs/graphics/effect-parser-params.c.h`). Each side
//! runs a typical declaration (a `float4` with a `string` annotation and a
//! `{ }` default) from the token after its name, on tokens the C lexer made
//! once, with the type and name `ep_parse_other()` would hand over, and
//! folds what it pushed into a checksum both must match. Every iteration
//! frees what it pushed, and the fixtures are freed at the end.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::effect_parser_params as c;
use obs_graphics::ffi::effect_parser::{self as rs, ep_param};
use obs_graphics::ffi::effect_parser_lifecycle::effect_parser;
use obs_util::ffi::cf_tokenizer::cf_token;
use std::ffi::CStr;
use std::hint::black_box;

const TEXT: &CStr =
    c"uniform float4 color<string name = \"Color\"; float minimum = 0.0;> = {1.0, 0.5, 0.25, 1.0};";

/// The parser over `TEXT`, and the index of its `<` token.
struct Fixture {
    ep: Box<effect_parser>,
    start: usize,
}

impl Fixture {
    fn new() -> Self {
        // SAFETY: every field of effect_parser permits zero; the oracle
        // initializes it and lexes TEXT into it, at the first token.
        let mut ep: Box<effect_parser> = Box::new(unsafe { core::mem::zeroed() });
        // SAFETY: a boxed parser that stays put; TEXT is a C string.
        let count = unsafe {
            c::oracle_effect_parser_params_prepare((&raw mut *ep).cast(), TEXT.as_ptr(), 0)
        };
        assert!(count > 0);
        // SAFETY: the token array holds count tokens before the terminator.
        let tokens = unsafe {
            let first = c::oracle_effect_parser_params_tokens((&raw mut *ep).cast());
            core::slice::from_raw_parts(first.cast::<cf_token>(), count)
        };
        let start = tokens
            .iter()
            // SAFETY: a token's str is len readable bytes of the lexer's text.
            .position(|t| t.str.len == 1 && unsafe { *t.str.array.cast::<u8>() } == b'<')
            .expect("the annotation list");
        Self { ep, start }
    }

    fn at_start(&mut self) -> *mut effect_parser {
        // SAFETY: the parser is initialized; start is one of its tokens.
        self.ep.cfp.cur_token = unsafe {
            c::oracle_effect_parser_params_tokens((&raw mut *self.ep).cast())
                .cast::<cf_token>()
                .add(self.start)
        };
        &raw mut *self.ep
    }

    /// Folds what the declaration pushed, then frees it: the C ep_free of
    /// everything but the tokens, done by hand.
    fn take(&mut self) -> u64 {
        let mut acc = self.ep.params.num as u64;
        // SAFETY: each darray holds `num` items of its C type.
        unsafe {
            for i in 0..self.ep.params.num {
                let param = &*self.ep.params.array.cast::<ep_param>().add(i);
                acc = acc * 31 + param.default_val.num as u64;
                acc = acc * 31 + param.annotations.num as u64;
            }
            c::oracle_effect_parser_params_clear((&raw mut *self.ep).cast());
        }
        acc
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: an initialized parser.
        unsafe { c::oracle_effect_parser_params_free((&raw mut *self.ep).cast()) };
    }
}

type Declaration = unsafe fn(*mut effect_parser, *mut core::ffi::c_char, *mut core::ffi::c_char);

fn run(f: &mut Fixture, declaration: Declaration) -> u64 {
    // SAFETY: the fixture's parser, at the token after the name; the type
    // and name are bmalloc'd for the declaration to own.
    unsafe {
        let r#type = c::oracle_effect_parser_params_strdup(c"float4".as_ptr());
        let name = c::oracle_effect_parser_params_strdup(c"color".as_ptr());
        declaration(black_box(f.at_start()), r#type, name);
    }
    f.take()
}

unsafe fn oracle_declaration(
    ep: *mut effect_parser,
    r#type: *mut core::ffi::c_char,
    name: *mut core::ffi::c_char,
) {
    // SAFETY: the caller's.
    unsafe { c::oracle_ep_parse_param(ep.cast(), r#type, name, false, false, true) };
}

unsafe fn rust_declaration(
    ep: *mut effect_parser,
    r#type: *mut core::ffi::c_char,
    name: *mut core::ffi::c_char,
) {
    // SAFETY: the caller's.
    unsafe { rs::ep_parse_param(ep, r#type, name, false, false, true) };
}

pub fn benchmarks(criterion: &mut Criterion) {
    let mut rust = Fixture::new();
    let mut oracle = Fixture::new();
    pair(
        criterion,
        "effect_parser/params/declaration",
        || run(&mut rust, rust_declaration),
        || run(&mut oracle, oracle_declaration),
    );
}
