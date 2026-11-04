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

/// Owned bytes of one sampler state or value.
///
/// Keep the element type local to obs-graphics. A `Vec<Vec<u8>>` can reuse
/// obs-util's generic allocation helpers in a release staticlib, pulling its
/// allocator into executables that already import shared libobs's allocator
/// (#3549). A distinct element type keeps these instantiations here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SamplerText(pub(crate) Vec<u8>);

impl core::ops::Deref for SamplerText {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// `struct shader_sampler`: its name and its states and values, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sampler {
    pub name: Vec<u8>,
    pub states: Vec<SamplerText>,
    pub values: Vec<SamplerText>,
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

/// `strref_cmp(&cur_token->str, s) == 0` for the non-empty literals the
/// blocks compare with: the token's bytes are `s` (a token holds no NUL,
/// since the lexer's text is a C string).
fn token_is(t: &impl Tokens, s: &CStr) -> bool {
    t.text() == s.to_bytes()
}

/// `*cur_token->str.array`: the terminator's (null) string reads as 0.
pub(crate) fn first_byte(t: &impl Tokens) -> u8 {
    t.text().first().copied().unwrap_or(0)
}

/// `cf_next_token`.
pub(crate) fn next_token(t: &mut impl Tokens) -> bool {
    let kind = t.kind();
    if kind != CFTOKEN_SPACETAB && kind != CFTOKEN_NEWLINE && kind != CFTOKEN_NONE {
        t.advance();
    }
    while t.kind() == CFTOKEN_SPACETAB || t.kind() == CFTOKEN_NEWLINE {
        t.advance();
    }
    t.kind() != CFTOKEN_NONE
}

/// `cf_next_valid_token`.
pub(crate) fn next_valid_token(t: &mut impl Tokens) -> bool {
    if !next_token(t) {
        t.add_error(BlockError::UnexpectedEof);
        return false;
    }
    true
}

/// `cf_go_to_token`.
fn go_to_token(t: &mut impl Tokens, str1: &CStr, str2: Option<&CStr>) -> bool {
    while next_token(t) {
        if token_is(t, str1) || str2.is_some_and(|s| token_is(t, s)) {
            return true;
        } else if first_byte(t) == b'{' && !t.pass_pair() {
            break;
        }
    }
    false
}

/// `cf_go_to_valid_token`.
fn go_to_valid_token(t: &mut impl Tokens, str1: &CStr, str2: Option<&CStr>) -> bool {
    if !go_to_token(t, str1, str2) {
        t.add_error(BlockError::UnexpectedEof);
        return false;
    }
    true
}

/// `cf_next_token_should_be`.
pub(crate) fn next_token_should_be(
    t: &mut impl Tokens,
    s: &'static CStr,
    goto1: Option<&CStr>,
    goto2: Option<&CStr>,
) -> i32 {
    if !next_token(t) {
        t.add_error(BlockError::UnexpectedEof);
        return PARSE_EOF;
    } else if token_is(t, s) {
        return PARSE_SUCCESS;
    }
    if let Some(goto1) = goto1
        && !go_to_token(t, goto1, goto2)
    {
        return PARSE_EOF;
    }
    t.add_error(BlockError::Expecting(s));
    PARSE_CONTINUE
}

/// `cf_peek_valid_token`: whether there is a next token, and whether its
/// text is `s` (the only thing the blocks read of the peeked copy). The
/// error goes at the token the cursor was put back on, as in the C.
pub(crate) fn peek_valid_token_is(t: &mut impl Tokens, s: &CStr) -> Option<bool> {
    let saved = t.mark();
    let success = next_token(t);
    let is = token_is(t, s);
    t.reset(saved);
    if !success {
        t.add_error(BlockError::UnexpectedEof);
        return None;
    }
    Some(is)
}

/// `cf_token_is_type`.
fn token_is_type(
    t: &mut impl Tokens,
    kind: i32,
    type_expected: &'static CStr,
    goto_token: Option<&CStr>,
) -> i32 {
    if t.kind() != kind {
        t.add_error(BlockError::Expecting(type_expected));
        if let Some(goto_token) = goto_token
            && !go_to_valid_token(t, goto_token, None)
        {
            return PARSE_EOF;
        }
        return PARSE_CONTINUE;
    }
    PARSE_SUCCESS
}

/// `cf_next_name`: the name's text, or the code.
pub(crate) fn next_name(
    t: &mut impl Tokens,
    name: &'static CStr,
    goto_token: Option<&CStr>,
) -> Result<Vec<u8>, i32> {
    if !next_valid_token(t) {
        return Err(PARSE_EOF);
    }
    // cf_get_name
    match token_is_type(t, CFTOKEN_NAME, name, goto_token) {
        PARSE_SUCCESS => Ok(t.text().to_vec()),
        code => Err(code),
    }
}

/// `cf_next_token_copy`.
fn next_token_copy(t: &mut impl Tokens) -> Result<Vec<u8>, i32> {
    if !next_valid_token(t) {
        return Err(PARSE_EOF);
    }
    Ok(t.text().to_vec())
}

/// `sp_parse_sampler_state_item`: one `state = value;`, pushed into `ss`
/// only when the whole item parsed.
fn parse_sampler_state_item(t: &mut impl Tokens, ss: &mut Sampler) -> i32 {
    let state = match next_name(t, c"state name", Some(c";")) {
        Ok(state) => state,
        Err(code) => return code,
    };
    let ret = next_token_should_be(t, c"=", Some(c";"), None);
    if ret != PARSE_SUCCESS {
        return ret;
    }
    let value = match next_token_copy(t) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let ret = next_token_should_be(t, c";", Some(c";"), None);
    if ret != PARSE_SUCCESS {
        return ret;
    }
    ss.states.push(SamplerText(state));
    ss.values.push(SamplerText(value));
    ret
}

/// `sp_parse_sampler_state`, from the `sampler_state` token: the sampler
/// `shader_parse()` pushes into `sp->samplers`, or `None` where the C
/// frees it (`goto error`).
pub fn parse_sampler_state(t: &mut impl Tokens) -> Option<Sampler> {
    let mut ss = Sampler {
        name: next_name(t, c"name", Some(c";")).ok()?,
        ..Sampler::default()
    };
    if next_token_should_be(t, c"{", Some(c";"), None) != PARSE_SUCCESS {
        return None;
    }

    while !peek_valid_token_is(t, c"}")? {
        if parse_sampler_state_item(t, &mut ss) == PARSE_EOF {
            return None;
        }
    }

    if next_token_should_be(t, c"}", Some(c";"), None) != PARSE_SUCCESS {
        return None;
    }
    if next_token_should_be(t, c";", None, None) != PARSE_SUCCESS {
        return None;
    }
    Some(ss)
}

/// `sp_parse_struct_var`: the code, with `var` filled as far as it got.
fn parse_struct_var(t: &mut impl Tokens, var: &mut StructVar) -> i32 {
    // variable type
    if !next_valid_token(t) {
        return PARSE_EOF;
    }
    if token_is(t, c";") {
        return PARSE_CONTINUE;
    }
    if token_is(t, c"}") {
        return PARSE_BREAK;
    }
    let code = token_is_type(t, CFTOKEN_NAME, c"type name", Some(c";"));
    if code != PARSE_SUCCESS {
        return code;
    }
    var.type_name = t.text().to_vec();

    // variable name
    if !next_valid_token(t) {
        return PARSE_EOF;
    }
    if token_is(t, c";") {
        return PARSE_UNEXPECTED_CONTINUE;
    }
    if token_is(t, c"}") {
        return PARSE_UNEXPECTED_BREAK;
    }
    let code = token_is_type(t, CFTOKEN_NAME, c"variable name", Some(c";"));
    if code != PARSE_SUCCESS {
        return code;
    }
    var.name = t.text().to_vec();

    // variable mapping if any (POSITION, TEXCOORD, etc)
    if !next_valid_token(t) {
        return PARSE_EOF;
    }
    if token_is(t, c":") {
        if !next_valid_token(t) {
            return PARSE_EOF;
        }
        if token_is(t, c";") {
            return PARSE_UNEXPECTED_CONTINUE;
        }
        if token_is(t, c"}") {
            return PARSE_UNEXPECTED_BREAK;
        }
        let code = token_is_type(t, CFTOKEN_NAME, c"mapping name", Some(c";"));
        if code != PARSE_SUCCESS {
            return code;
        }
        var.mapping = Some(t.text().to_vec());
        if !next_valid_token(t) {
            return PARSE_EOF;
        }
    }

    if !token_is(t, c";") {
        if !go_to_valid_token(t, c";", Some(c"}")) {
            return PARSE_EOF;
        }
        return PARSE_CONTINUE;
    }
    PARSE_SUCCESS
}

/// `sp_parse_struct`, from the `struct` token: the struct `shader_parse()`
/// pushes into `sp->structs`, or `None` where the C frees it.
pub fn parse_struct(t: &mut impl Tokens) -> Option<Struct> {
    let mut ss = Struct {
        name: next_name(t, c"name", Some(c";")).ok()?,
        ..Struct::default()
    };
    if next_token_should_be(t, c"{", Some(c";"), None) != PARSE_SUCCESS {
        return None;
    }

    // get structure variables
    loop {
        let mut var = StructVar::default();
        match parse_struct_var(t, &mut var) {
            PARSE_UNEXPECTED_CONTINUE => t.add_error(BlockError::SyntaxError),
            PARSE_CONTINUE => {}
            PARSE_UNEXPECTED_BREAK => {
                t.add_error(BlockError::SyntaxError);
                break;
            }
            PARSE_BREAK => break,
            PARSE_EOF => return None,
            // PARSE_SUCCESS, the only other code it returns.
            _ => ss.vars.push(var),
        }
    }

    if next_token_should_be(t, c";", None, None) != PARSE_SUCCESS {
        return None;
    }
    Some(ss)
}
