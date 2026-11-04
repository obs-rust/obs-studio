//! obs-graphics: the effect parser's passes and techniques
//! (`libobs/graphics/effect-parser-passes.c.h`). Each side runs a typical
//! technique (two named passes, each with a vertex and a pixel program)
//! from its keyword token, on tokens the C lexer made once, and folds what
//! it pushed into a checksum both must match. Every iteration frees what it
//! pushed, and the fixtures are freed at the end.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::effect_parser_passes as c;
use obs_graphics::ffi::effect_parser::{self as rs, ep_pass, ep_technique};
use obs_graphics::ffi::effect_parser_lifecycle::effect_parser;
use std::ffi::CStr;
use std::hint::black_box;

const TEXT: &CStr = c"technique Draw\n{\n\tpass p0\n\t{\n\t\tvertex_shader = VSDefault(vert_in);\n\t\tpixel_shader  = PSDrawBare(vert_in);\n\t}\n\tpass p1\n\t{\n\t\tvertex_shader = VSDefault(vert_in);\n\t\tpixel_shader  = PSDrawAlphaDivide(vert_in, 1.0);\n\t}\n}";

/// The parser over `TEXT`, at its `technique` token.
struct Fixture {
    ep: Box<effect_parser>,
}

impl Fixture {
    fn new() -> Self {
        // SAFETY: every field of effect_parser permits zero; the oracle
        // initializes it and lexes TEXT into it, at the first token.
        let mut ep: Box<effect_parser> = Box::new(unsafe { core::mem::zeroed() });
        // SAFETY: a boxed parser that stays put; TEXT is a C string.
        let count = unsafe {
            c::oracle_effect_parser_passes_prepare((&raw mut *ep).cast(), TEXT.as_ptr(), 0)
        };
        assert!(count > 0);
        Self { ep }
    }

    fn at_start(&mut self) -> *mut effect_parser {
        // SAFETY: the parser is initialized; its token array starts with
        // the technique keyword.
        self.ep.cfp.cur_token =
            unsafe { c::oracle_effect_parser_passes_tokens((&raw mut *self.ep).cast()) }.cast();
        &raw mut *self.ep
    }

    /// Folds what the technique pushed, then frees it: the C ep_free of
    /// everything but the tokens, done by hand.
    fn take(&mut self) -> u64 {
        let mut acc = self.ep.techniques.num as u64;
        // SAFETY: each darray holds `num` items of its C type.
        unsafe {
            for i in 0..self.ep.techniques.num {
                let ept = &*self.ep.techniques.array.cast::<ep_technique>().add(i);
                for j in 0..ept.passes.num {
                    let pass = &*ept.passes.array.cast::<ep_pass>().add(j);
                    acc = acc * 31 + pass.vertex_program.num as u64;
                    acc = acc * 31 + pass.fragment_program.num as u64;
                }
            }
            c::oracle_effect_parser_passes_clear((&raw mut *self.ep).cast());
        }
        acc
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: an initialized parser.
        unsafe { c::oracle_effect_parser_passes_free((&raw mut *self.ep).cast()) };
    }
}

fn run(f: &mut Fixture, technique: unsafe fn(*mut effect_parser)) -> u64 {
    // SAFETY: the fixture's parser, at the keyword token.
    unsafe { technique(black_box(f.at_start())) };
    f.take()
}

unsafe fn oracle_technique(ep: *mut effect_parser) {
    // SAFETY: the caller's.
    unsafe { c::oracle_ep_parse_technique(ep.cast()) };
}

unsafe fn rust_technique(ep: *mut effect_parser) {
    // SAFETY: the caller's.
    unsafe { rs::ep_parse_technique(ep) };
}

pub fn benchmarks(criterion: &mut Criterion) {
    let mut rust = Fixture::new();
    let mut oracle = Fixture::new();
    pair(
        criterion,
        "effect_parser/passes/technique",
        || run(&mut rust, rust_technique),
        || run(&mut oracle, oracle_technique),
    );
}
