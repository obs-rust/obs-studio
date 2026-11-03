//! obs-graphics: the shader parser's sampler-state and struct blocks
//! (`libobs/graphics/shader-parser-blocks.c.h`). Each side runs the blocks
//! of a typical shader (a vertex struct with mappings, a sampler state with
//! four items) from their keyword tokens, on tokens the C lexer made once,
//! and folds what it pushed into a checksum both must match. Every
//! iteration frees what it pushed, and the fixtures are freed at the end.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::shader_parser_blocks as c;
use obs_graphics::ffi::shader_parser::{self as rs, shader_parser, shader_struct};
use obs_graphics::ffi::shader_sampler_convert::shader_sampler;
use obs_util::ffi::cf_tokenizer::cf_token;
use std::ffi::CStr;
use std::hint::black_box;

const TEXT: &CStr = c"struct VertInOut {\n\tfloat4 pos : POSITION;\n\tfloat2 uv : TEXCOORD0;\n\tfloat4 color : COLOR;\n};\nsampler_state def_sampler {\n\tFilter = Linear;\n\tAddressU = Clamp;\n\tAddressV = Clamp;\n\tBorderColor = 00000000;\n};";

/// The parser over `TEXT` with the index of each block's keyword token.
struct Fixture {
    sp: Box<shader_parser>,
    start: *mut cf_token,
    blocks: [usize; 2],
}

impl Fixture {
    fn new() -> Self {
        // SAFETY: every field of shader_parser permits zero; the oracle
        // initializes it and lexes TEXT into it.
        let mut sp: Box<shader_parser> = Box::new(unsafe { core::mem::zeroed() });
        // SAFETY: a boxed parser that stays put; TEXT is a C string.
        let count = unsafe {
            c::oracle_shader_parser_blocks_prepare((&raw mut *sp).cast(), TEXT.as_ptr(), 0)
        };
        assert!(count > 0);
        let start = sp.cfp.cur_token;
        let mut blocks = [0; 2];
        for (slot, keyword) in blocks.iter_mut().zip([&b"struct"[..], b"sampler_state"]) {
            *slot = (0..count)
                .find(|&i| {
                    // SAFETY: i is below the token count.
                    let s = unsafe { &(*start.add(i)).str };
                    // SAFETY: a token's str is len bytes of the lexer's text.
                    !s.array.is_null() && unsafe { core::slice::from_raw_parts(s.array.cast::<u8>(), s.len) } == keyword
                })
                .expect("keyword");
        }
        Self { sp, start, blocks }
    }

    fn at(&mut self, index: usize) -> *mut shader_parser {
        // SAFETY: index is a keyword token inside the stream.
        self.sp.cfp.cur_token = unsafe { self.start.add(index) };
        &raw mut *self.sp
    }

    /// Folds what the blocks pushed, then frees it: the C shader_parser_free
    /// of everything but the tokens, done by hand.
    fn take(&mut self) -> u64 {
        let mut acc = self.sp.samplers.num as u64 * 31 + self.sp.structs.num as u64;
        // SAFETY: each darray holds `num` items of its C type.
        unsafe {
            for i in 0..self.sp.samplers.num {
                let ss = &*self.sp.samplers.array.cast::<shader_sampler>().add(i);
                acc = acc * 31 + ss.states.num as u64;
            }
            for i in 0..self.sp.structs.num {
                let st = &*self.sp.structs.array.cast::<shader_struct>().add(i);
                acc = acc * 31 + st.vars.num as u64;
            }
            c::oracle_shader_parser_blocks_clear((&raw mut *self.sp).cast());
        }
        acc
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: an initialized parser.
        unsafe { c::oracle_shader_parser_blocks_free((&raw mut *self.sp).cast()) };
    }
}

fn run(f: &mut Fixture, sampler: unsafe fn(*mut shader_parser), structs: unsafe fn(*mut shader_parser)) -> u64 {
    let [st, ss] = f.blocks;
    // SAFETY: the fixture's parser, at a keyword token.
    unsafe {
        structs(black_box(f.at(st)));
        sampler(black_box(f.at(ss)));
    }
    f.take()
}

unsafe fn oracle_sampler(sp: *mut shader_parser) {
    // SAFETY: the caller's.
    unsafe { c::oracle_sp_parse_sampler_state(sp.cast()) };
}

unsafe fn oracle_struct(sp: *mut shader_parser) {
    // SAFETY: the caller's.
    unsafe { c::oracle_sp_parse_struct(sp.cast()) };
}

unsafe fn rust_sampler(sp: *mut shader_parser) {
    // SAFETY: the caller's.
    unsafe { rs::sp_parse_sampler_state(sp) };
}

unsafe fn rust_struct(sp: *mut shader_parser) {
    // SAFETY: the caller's.
    unsafe { rs::sp_parse_struct(sp) };
}

pub fn benchmarks(criterion: &mut Criterion) {
    let mut rust = Fixture::new();
    let mut oracle = Fixture::new();
    pair(
        criterion,
        "shader_parser/blocks/struct_and_sampler",
        || run(&mut rust, rust_sampler, rust_struct),
        || run(&mut oracle, oracle_sampler, oracle_struct),
    );
}
