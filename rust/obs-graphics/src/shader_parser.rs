//! Safe core of `libobs/graphics/shader-parser-blocks.c.h`: the shader
//! parser's sampler-state and struct blocks.
//!
//! - [`parse_sampler_state`] (`sp_parse_sampler_state`, with
//!   `sp_parse_sampler_state_item`): `sampler_state name { state = value;
//!   ... };`;
//! - [`parse_struct`] (`sp_parse_struct`, with `sp_parse_struct_var`):
//!   `struct name { type name [: MAPPING]; ... };`.
//!
//! `shader_parse()` calls each on the keyword token that starts it, and goes
//! on from the token where it stopped. Both walk the parser's tokens in place
//! through [`Tokens`]: the shim's is the C `cf_parser`'s own `cur_token`, so
//! `shader_parse()` sees the cursor move exactly as before. The steps are the
//! `util/cf-parser.h` inline helpers the C calls (`cf_next_token`,
//! `cf_next_name`, `cf_next_token_should_be`, `cf_peek_valid_token`,
//! `cf_token_is_type`, `cf_go_to_valid_token`, ...), ported 1:1 under the
//! header-inline exemption (testing-policy.md) and kept here, each called
//! where the C calls it. Every error goes through [`Tokens::add_error`] at
//! the token the C adds it at and in the same order (the shim's is the
//! exported `cf_adderror`), and `{ ... }` is skipped by
//! [`Tokens::pass_pair`] (the exported `cf_pass_pair`).
//!
//! A block that parses is returned whole; one that hits the end of the text
//! is `None`, as the C frees it and pushes nothing. Characterized, not
//! endorsed, and kept (`test_shader_parser_blocks`): a missing `;` in a
//! sampler state skips to the next `;`, losing the item after it; a stray
//! token before a struct variable's `;` drops the variable with no error;
//! a bad block name skips to a `;` past the braces.
//!
//! No intentional differences.

use core::ffi::CStr;

/// `PARSE_SUCCESS` (`util/lexer.h`).
pub const PARSE_SUCCESS: i32 = 0;
/// `PARSE_CONTINUE`.
pub const PARSE_CONTINUE: i32 = -1;
/// `PARSE_BREAK`.
pub const PARSE_BREAK: i32 = -2;
/// `PARSE_UNEXPECTED_CONTINUE`.
pub const PARSE_UNEXPECTED_CONTINUE: i32 = -3;
/// `PARSE_UNEXPECTED_BREAK`.
pub const PARSE_UNEXPECTED_BREAK: i32 = -4;
/// `PARSE_EOF`.
pub const PARSE_EOF: i32 = -5;

/// `CFTOKEN_NONE`, the terminator (`util/cf-lexer.h`).
pub const CFTOKEN_NONE: i32 = 0;
/// `CFTOKEN_NAME`.
pub const CFTOKEN_NAME: i32 = 1;
/// `CFTOKEN_SPACETAB`.
pub const CFTOKEN_SPACETAB: i32 = 3;
/// `CFTOKEN_NEWLINE`.
pub const CFTOKEN_NEWLINE: i32 = 4;

/// The errors the blocks add, each a `cf_adderror(p, ..., LEX_ERROR, ...)`
/// of `util/cf-parser.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockError {
    /// `cf_adderror_expecting`: `"Expected '$1'"` with the expected text.
    Expecting(&'static CStr),
    /// `cf_adderror_unexpected_eof`: `"Unexpected EOF"`.
    UnexpectedEof,
    /// `cf_adderror_syntax_error`: `"Syntax error"`.
    SyntaxError,
}

impl BlockError {
    /// The `error` and `val1` arguments the C passes `cf_adderror`.
    #[must_use]
    pub fn message(self) -> (&'static CStr, Option<&'static CStr>) {
        match self {
            Self::Expecting(expected) => (c"Expected '$1'", Some(expected)),
            Self::UnexpectedEof => (c"Unexpected EOF", None),
            Self::SyntaxError => (c"Syntax error", None),
        }
    }
}

/// The `struct cf_parser` the blocks walk: its `cur_token` and what the
/// C reads of it.
pub trait Tokens {
    /// A saved `cur_token`.
    type Mark: Copy;
    /// `cur_token->type`.
    fn kind(&self) -> i32;
    /// `cur_token->str`, empty when its `array` is null (the terminator).
    fn text(&self) -> &[u8];
    /// `cur_token++`.
    fn advance(&mut self);
    /// `cur_token`, to put back.
    fn mark(&self) -> Self::Mark;
    /// `cur_token = mark`.
    fn reset(&mut self, mark: Self::Mark);
    /// `cf_adderror()` at `cur_token`, level `LEX_ERROR`.
    fn add_error(&mut self, error: BlockError);
    /// `cf_pass_pair(p, '{', '}')`.
    fn pass_pair(&mut self) -> bool;
}

/// `struct shader_sampler`: its name and its states and values, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sampler {
    pub name: Vec<u8>,
    pub states: Vec<Vec<u8>>,
    pub values: Vec<Vec<u8>>,
}

/// A `struct shader_var` of a struct: `shader_var_init` (all zero) with its
/// type, name and mapping.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StructVar {
    pub type_name: Vec<u8>,
    pub name: Vec<u8>,
    pub mapping: Option<Vec<u8>>,
}

/// `struct shader_struct`: its name and its variables, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Struct {
    pub name: Vec<u8>,
    pub vars: Vec<StructVar>,
}

/// `sp_parse_sampler_state`, from the `sampler_state` token: the sampler
/// `shader_parse()` pushes into `sp->samplers`, or `None` where the C
/// frees it (`goto error`).
pub fn parse_sampler_state(t: &mut impl Tokens) -> Option<Sampler> {
    let _ = t;
    todo!("sp_parse_sampler_state")
}

/// `sp_parse_struct`, from the `struct` token: the struct `shader_parse()`
/// pushes into `sp->structs`, or `None` where the C frees it.
pub fn parse_struct(t: &mut impl Tokens) -> Option<Struct> {
    let _ = t;
    todo!("sp_parse_struct")
}
