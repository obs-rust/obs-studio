//! Safe core of `libobs/graphics/effect-parser-blocks.c.h`,
//! `libobs/graphics/effect-parser-passes.c.h` and
//! `libobs/graphics/effect-parser-params.c.h`: the effect parser's struct
//! and sampler-state blocks, its passes and techniques, and its parameter
//! declarations.
//!
//! - [`parse_struct`] (`ep_parse_struct`, with `ep_parse_struct_var`):
//!   `struct name { type name [: MAPPING]; ... };`;
//! - [`parse_sampler_state`] (`ep_parse_sampler_state`, with
//!   `ep_parse_sampler_state_item`): `sampler_state name { state = value;
//!   ... };`;
//! - [`parse_technique`] (`ep_parse_technique`, with [`parse_pass`],
//!   [`parse_pass_command`] and `ep_parse_pass_command_call`): `technique
//!   name { pass [name] { vertex_shader = <call>; pixel_shader = <call>; }
//!   ... }`, with `vertex_program` and `pixel_program` for the commands and
//!   an optional `compile` before a call, skipped with a warning;
//! - [`parse_param_array`] (`ep_parse_param_array`): `[count]`, and
//!   [`parse_param_assign`] (`ep_parse_param_assign`, with
//!   [`parse_param_assignment_val`] and its value parsers): `= value`, by
//!   the parser `obs_effect_param_route()` picks for the type (a texture
//!   path or a string, an `int`, a `float`, a `bool`, or the `{ ... }` and
//!   `type( ... )` forms of the vector and matrix types).
//!
//! `ep_parse()` calls each on the keyword token that starts it, and goes on
//! from the token where it stopped. Both walk the parser's tokens in place
//! through [`Tokens`], as the shader parser's blocks do
//! ([`crate::shader_parser`]), with the same `util/cf-parser.h` steps, each
//! called where the C calls it, and every error added at the token the C
//! adds it at, in the same order.
//!
//! The struct block's C is the shader parser's, line for line (`struct
//! ep_var` and `struct ep_struct` in place of `struct shader_var` and
//! `struct shader_struct`), so [`parse_struct`] is
//! [`crate::shader_parser::parse_struct`]. The sampler-state item differs:
//! its value is every token up to the next `;`, joined with no spaces
//! between them (`dstr_ncat`), and an item with an empty value is dropped
//! with no error.
//!
//! A block that parses is returned whole; one that hits the end of the text
//! is `None`, as the C frees it and pushes nothing. Characterized, not
//! endorsed, and kept (`test_effect_parser_blocks`): a missing `;` in a
//! sampler state joins the next item into the value; an empty value drops
//! the item with no error; a stray token before a struct variable's `;`
//! drops the variable with no error; a bad block name skips to a `;` past
//! the braces.
//!
//! A technique's programs are not text: each is the list of the tokens the
//! C copies into it ([`ProgramToken::Token`], a [`Tokens::Mark`]), each
//! `;`-ended call followed by the end token ([`ProgramToken::End`]); the
//! shim copies the C tokens at those marks. Characterized, not endorsed,
//! and kept (`test_effect_parser_passes`): a missing `;` before a pass's
//! `}` ends the call with no end token, and that `}` is not taken as the
//! end of the pass, so the technique reads on and is dropped with what
//! follows it; an entry other than a pass skips to the next `}`, which
//! ends the technique, with no error; a second command for a program is
//! appended after the first one's end token; a pass's name is any token;
//! end of text after a command's name is reported twice.
//!
//! A parameter is a [`Param`], `struct ep_param` as the C fills it, with
//! its default value as bytes: a string's and its NUL, or each `int` and
//! `float` in the order the C pushes them. Beyond [`Tokens`], the value
//! parsers read through [`ParamTokens`] what the C reads: the number
//! token's text as the C string `strtol` and `os_strtod` read, running on
//! past the token; libobs's `os_strtod`; `cf_literal_to_str` of the token.
//! The type decisions and the `int` and count reads are
//! [`crate::effect_param_types`]'s rules, the `float` read
//! [`crate::shader_param_values`]'s, each called where the C calls it.
//! Characterized, not endorsed, and kept (`test_effect_parser_params`): an
//! `int` reads the leading digits of a number token; the `-` is stepped over
//! with `cf_next_token`, so `- 1` reads as `-1` and the end of the text after
//! it adds no error; a `bool` other than `true` or `false` and a type with
//! no value parser do not skip to the `;`; a vector or matrix with a bad row
//! or column digit adds its error and reads as many values as the digits
//! say; a count that is not a number token fails with no error.
//!
//! No intentional differences. The C freed a dropped item's state name and
//! value on every path already (#3550), and a dropped technique's name
//! (#3598), each fixed in the C before its port.

use crate::effect_param_types::{self, Route};
use crate::shader_param_values;
pub use crate::shader_parser::{BlockError, Sampler, SamplerText, Struct, StructVar, Tokens};
use crate::shader_parser::{
    CFTOKEN_NUM, CFTOKEN_STRING, PARSE_CONTINUE, PARSE_EOF, PARSE_SUCCESS,
    PARSE_UNEXPECTED_CONTINUE, first_byte, go_to_token, go_to_valid_token, next_name, next_token,
    next_token_should_be, next_valid_token, peek_valid_token_is, token_is, token_is_type,
};

/// `ep_parse_struct`, from the `struct` token: the struct `ep_parse()`
/// pushes into `ep->structs`, or `None` where the C frees it.
pub fn parse_struct(t: &mut impl Tokens) -> Option<Struct> {
    crate::shader_parser::parse_struct(t)
}

/// `ep_parse_sampler_state_item`: one `state = value;`, pushed into `eps`
/// only when it parsed with a value.
fn parse_sampler_state_item(t: &mut impl Tokens, eps: &mut Sampler) -> i32 {
    let state = match next_name(t, c"state name", Some(c";")) {
        Ok(state) => state,
        Err(code) => return code,
    };
    let ret = next_token_should_be(t, c"=", Some(c";"), None);
    if ret != PARSE_SUCCESS {
        return ret;
    }

    let mut value = Vec::new();
    loop {
        if !next_valid_token(t) {
            return PARSE_EOF;
        }
        if first_byte(t) == b';' {
            break;
        }
        // dstr_ncat: a token's text is never empty.
        value.extend_from_slice(t.text());
    }

    if !value.is_empty() {
        eps.states.push(SamplerText(state));
        eps.values.push(SamplerText(value));
    }
    ret
}

/// `ep_parse_sampler_state`, from the `sampler_state` token: the sampler
/// `ep_parse()` pushes into `ep->samplers`, or `None` where the C frees it
/// (`goto error`).
pub fn parse_sampler_state(t: &mut impl Tokens) -> Option<Sampler> {
    let mut eps = Sampler {
        name: next_name(t, c"name", Some(c";")).ok()?,
        ..Sampler::default()
    };
    if next_token_should_be(t, c"{", Some(c";"), None) != PARSE_SUCCESS {
        return None;
    }

    while !peek_valid_token_is(t, c"}")? {
        if parse_sampler_state_item(t, &mut eps) == PARSE_EOF {
            return None;
        }
    }

    if next_token_should_be(t, c"}", Some(c";"), None) != PARSE_SUCCESS {
        return None;
    }
    if next_token_should_be(t, c";", None, None) != PARSE_SUCCESS {
        return None;
    }
    Some(eps)
}

/// One entry of a pass's `vertex_program` or `fragment_program`: a copy of
/// the parser's token at a [`Tokens::Mark`] (`da_push_back(*call,
/// ep->cfp.cur_token)`), or the zeroed end token (`cf_token_clear`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramToken<M> {
    /// The token at this mark, copied whole.
    Token(M),
    /// The end token: all zero, so `CFTOKEN_NONE` with no text.
    End,
}

/// `struct ep_pass` as the C fills it: its name (`NULL` when unnamed) and
/// its two programs. `pass` stays `NULL`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pass<M> {
    pub name: Option<Vec<u8>>,
    pub vertex_program: Vec<ProgramToken<M>>,
    pub fragment_program: Vec<ProgramToken<M>>,
}

impl<M> Default for Pass<M> {
    /// `ep_pass_init`: all zero.
    fn default() -> Self {
        Self {
            name: None,
            vertex_program: Vec::new(),
            fragment_program: Vec::new(),
        }
    }
}

/// `struct ep_technique`: its name and its passes, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Technique<M> {
    pub name: Vec<u8>,
    pub passes: Vec<Pass<M>>,
}

type Program<M> = Vec<ProgramToken<M>>;

/// `ep_parse_pass_command_call`: the call's tokens up to and with the
/// `;`, then the end token. A `}` first stops it with no end token.
fn parse_pass_command_call<T: Tokens>(t: &mut T, call: &mut Program<T::Mark>) -> i32 {
    while !token_is(t, c";") {
        if token_is(t, c"}") {
            t.add_error(BlockError::Expecting(c";"));
            return PARSE_CONTINUE;
        }

        call.push(ProgramToken::Token(t.mark()));
        if !next_valid_token(t) {
            return PARSE_EOF;
        }
    }

    call.push(ProgramToken::Token(t.mark()));
    call.push(ProgramToken::End);
    PARSE_SUCCESS
}

/// `ep_parse_pass_command`, from the token before the command:
/// `vertex_shader = <call>;` or `pixel_shader = <call>;` (or
/// `vertex_program`, `pixel_program`), an optional `compile` skipped with a
/// warning, the call's tokens appended to that program of `pass`. Returns
/// the C's code: `PARSE_SUCCESS`, `PARSE_CONTINUE` or `PARSE_EOF`.
pub fn parse_pass_command<T: Tokens>(t: &mut T, pass: &mut Pass<T::Mark>) -> i32 {
    if !next_valid_token(t) {
        return PARSE_EOF;
    }

    let call = if token_is(t, c"vertex_shader") || token_is(t, c"vertex_program") {
        &mut pass.vertex_program
    } else if token_is(t, c"pixel_shader") || token_is(t, c"pixel_program") {
        &mut pass.fragment_program
    } else {
        t.add_error(BlockError::SyntaxError);
        if !go_to_valid_token(t, c";", Some(c"}")) {
            return PARSE_EOF;
        }
        return PARSE_CONTINUE;
    };

    if next_token_should_be(t, c"=", Some(c";"), Some(c"}")) != PARSE_SUCCESS {
        return PARSE_CONTINUE;
    }

    if !next_valid_token(t) {
        return PARSE_EOF;
    }
    if token_is(t, c"compile") {
        t.add_error(BlockError::CompileNotNecessary);
        if !next_valid_token(t) {
            return PARSE_EOF;
        }
    }

    parse_pass_command_call(t, call)
}

/// `ep_parse_pass`, from the technique's entry token: `pass [name] {
/// <commands> }`, filling `pass`. Returns the C's code:
/// `PARSE_UNEXPECTED_CONTINUE` when the entry is not `pass`, `PARSE_EOF`,
/// or `PARSE_SUCCESS` past the `}`.
pub fn parse_pass<T: Tokens>(t: &mut T, pass: &mut Pass<T::Mark>) -> i32 {
    if !token_is(t, c"pass") {
        return PARSE_UNEXPECTED_CONTINUE;
    }

    if !next_valid_token(t) {
        return PARSE_EOF;
    }

    if !token_is(t, c"{") {
        // bstrdup_n of the token: any token can name a pass.
        pass.name = Some(t.text().to_vec());
        if !next_valid_token(t) {
            return PARSE_EOF;
        }
    }

    let Some(mut peek_is_close) = peek_valid_token_is(t, c"}") else {
        return PARSE_EOF;
    };

    while !peek_is_close {
        let ret = parse_pass_command(t, pass);
        if ret < 0 && ret != PARSE_CONTINUE {
            return ret;
        }

        let Some(is_close) = peek_valid_token_is(t, c"}") else {
            return PARSE_EOF;
        };
        peek_is_close = is_close;
    }

    // token is '}'
    next_token(t);

    PARSE_SUCCESS
}

/// The C's `error:` label: one `cf_next_token`, and the technique freed.
fn drop_technique<T: Tokens>(t: &mut T) -> Option<Technique<T::Mark>> {
    next_token(t);
    None
}

/// `ep_parse_technique`, from the `technique` token: `technique name {
/// pass ... }`, the technique `ep_parse()` pushes into `ep->techniques`,
/// or `None` where the C frees it.
pub fn parse_technique<T: Tokens>(t: &mut T) -> Option<Technique<T::Mark>> {
    let Ok(name) = next_name(t, c"name", Some(c";")) else {
        return drop_technique(t);
    };
    let mut ept = Technique {
        name,
        passes: Vec::new(),
    };

    if !next_valid_token(t) {
        return None;
    }

    if !token_is(t, c"{") {
        if !go_to_token(t, c";", None) {
            t.add_error(BlockError::Expecting(c";"));
            return None;
        }

        t.add_error(BlockError::Expecting(c"{"));
        return drop_technique(t);
    }
    if !next_valid_token(t) {
        return drop_technique(t);
    }

    while !token_is(t, c"}") {
        let mut pass = Pass::default();

        match parse_pass(t, &mut pass) {
            PARSE_UNEXPECTED_CONTINUE => {
                if !go_to_token(t, c"}", None) {
                    return drop_technique(t);
                }
                continue;
            }
            PARSE_EOF => return drop_technique(t),
            _ => {}
        }

        ept.passes.push(pass);

        if !next_valid_token(t) {
            return drop_technique(t);
        }
    }

    // pass the current token (which is '}') if we reached here
    next_token(t);

    Some(ept)
}

/// What the parameter parsers read of the `struct cf_parser` beyond
/// [`Tokens`]: the number token's text as the C string `strtol` and
/// `os_strtod` read, which runs on past the token into the lexer's text,
/// libobs's `os_strtod`, `cf_literal_to_str` of the token, and the one
/// error that names the parameter's type.
pub trait ParamTokens: Tokens {
    /// Byte `i` of `cur_token->str.array` read as a C string: the token's
    /// text, then the lexer's text after it, 0 at its end. Asked only in
    /// order, and never past a 0.
    fn c_str_byte(&self, i: usize) -> u8;
    /// `os_strtod` of `copy`, a NUL-terminated copy of at most 64 bytes.
    fn strtod(&self, copy: &[u8]) -> f64;
    /// `cf_literal_to_str(cur_token->str.array, cur_token->str.len)`: the
    /// string's bytes up to its first NUL (what `strlen` sees), or `None`
    /// where it returns `NULL`.
    fn literal_to_str(&self) -> Option<Vec<u8>>;
    /// `cf_adderror(p, INVALID_TYPE, LEX_ERROR, type, NULL, NULL)` at
    /// `cur_token`.
    fn add_invalid_type_error(&mut self, type_name: &[u8]);
}

/// The error `ep_parse_param_assignment_val` adds for a type with no value
/// parser, with the type as `$1`.
pub const INVALID_TYPE: &core::ffi::CStr = c"Invalid type '$1' used for assignment";

/// `struct ep_param` as the parameter parsers fill it: a parameter of
/// `ep->params`, or one of its annotations. `properties` stays empty,
/// `written` false and `writeorder` 0, as `ep_param_init` leaves them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Param {
    pub type_name: Vec<u8>,
    pub name: Vec<u8>,
    pub is_property: bool,
    pub is_const: bool,
    pub is_uniform: bool,
    /// Set once, from the type `ep_param_init` was given.
    pub is_texture: bool,
    pub array_count: i32,
    /// The bytes of `default_val`: a string's bytes and its NUL, or each
    /// `int` or `float` value's bytes in the order they were pushed.
    pub default_val: Vec<u8>,
    pub annotations: Vec<Param>,
}

impl Param {
    /// `ep_param_init`: `is_texture` when the type starts with `texture`
    /// (`astrcmp_n(type, "texture", 7)`), everything else zero.
    #[must_use]
    pub fn new(
        type_name: Vec<u8>,
        name: Vec<u8>,
        is_property: bool,
        is_const: bool,
        is_uniform: bool,
    ) -> Self {
        Self {
            is_texture: type_name.starts_with(b"texture"),
            type_name,
            name,
            is_property,
            is_const,
            is_uniform,
            array_count: 0,
            default_val: Vec::new(),
            annotations: Vec::new(),
        }
    }
}

/// `ep_parse_param_array`, from the `[` token: `[count]` into
/// `param.array_count`, then past the `]`. `false` where the parameter is
/// dropped.
pub fn parse_param_array(t: &mut impl ParamTokens, param: &mut Param) -> bool {
    if !next_valid_token(t) {
        return false;
    }

    if t.kind() != CFTOKEN_NUM {
        return false;
    }
    let len = t.text().len();
    let Some(count) = effect_param_types::array_count_by(|i| t.c_str_byte(i), len) else {
        return false;
    };
    param.array_count = count;

    if next_token_should_be(t, c"]", Some(c";"), None) == PARSE_EOF {
        return false;
    }
    next_valid_token(t)
}

/// `ep_parse_param_assign_texture` and `ep_parse_param_assign_string`: a
/// string token, its `cf_literal_to_str` copied into `default_val` with its
/// NUL (`da_copy_array`). `expected` names it in the error.
fn parse_param_assign_text(
    t: &mut impl ParamTokens,
    param: &mut Param,
    expected: &'static core::ffi::CStr,
) -> i32 {
    if !next_valid_token(t) {
        return PARSE_EOF;
    }

    let code = token_is_type(t, CFTOKEN_STRING, expected, Some(c";"));
    if code != PARSE_SUCCESS {
        return code;
    }

    if let Some(mut text) = t.literal_to_str() {
        text.push(0);
        param.default_val = text;
    }
    PARSE_SUCCESS
}

/// `ep_parse_param_assign_intfloat`: `[-] number`, its `float`
/// (`is_float`) or `int` pushed onto `default_val`. The `-` is stepped
/// over with `cf_next_token`, so a space after it is allowed and the end
/// of the text adds no error.
fn parse_param_assign_intfloat(t: &mut impl ParamTokens, param: &mut Param, is_float: bool) -> i32 {
    let mut is_negative = false;

    if !next_valid_token(t) {
        return PARSE_EOF;
    }

    if token_is(t, c"-") {
        is_negative = true;

        if !next_token(t) {
            return PARSE_EOF;
        }
    }

    let code = token_is_type(t, CFTOKEN_NUM, c"numeric value", Some(c";"));
    if code != PARSE_SUCCESS {
        return code;
    }

    if is_float {
        let f = shader_param_values::float_value_by(
            |i| t.c_str_byte(i),
            is_negative,
            |copy| t.strtod(copy),
        );
        param.default_val.extend_from_slice(&f.to_ne_bytes());
    } else {
        let i = effect_param_types::int_value_by(|i| t.c_str_byte(i), is_negative);
        param.default_val.extend_from_slice(&i.to_ne_bytes());
    }
    PARSE_SUCCESS
}

/// `ep_parse_param_assign_bool`: `true` or `false`, pushed as an `int`.
/// Anything else is an error, and `PARSE_EOF` with no skip.
fn parse_param_assign_bool(t: &mut impl ParamTokens, param: &mut Param) -> i32 {
    if !next_valid_token(t) {
        return PARSE_EOF;
    }

    if token_is(t, c"true") {
        param.default_val.extend_from_slice(&1_i32.to_ne_bytes());
        return PARSE_SUCCESS;
    } else if token_is(t, c"false") {
        param.default_val.extend_from_slice(&0_i32.to_ne_bytes());
        return PARSE_SUCCESS;
    }

    t.add_error(BlockError::Expecting(c"true or false"));
    PARSE_EOF
}

/// `ep_parse_param_assign_intfloat_array`: the values of a `floatN`,
/// `intN`, `floatNxM` or `intNxM`, as `{ v, ... }` or `type( v, ... )`,
/// their count from the type's digits.
fn parse_param_assign_intfloat_array(
    t: &mut impl ParamTokens,
    param: &mut Param,
    is_float: bool,
) -> i32 {
    if effect_param_types::rows_bad(&param.type_name, is_float) {
        t.add_error(BlockError::InvalidRowCount);
    }

    let mut intfloat_count = effect_param_types::rows(&param.type_name, is_float);

    if effect_param_types::has_cols(&param.type_name, is_float) {
        if effect_param_types::cols_bad(&param.type_name, is_float) {
            t.add_error(BlockError::InvalidColumnCount);
        }

        intfloat_count = effect_param_types::cols(&param.type_name, is_float, intfloat_count);
    }

    if !next_token(t) {
        t.add_error(BlockError::UnexpectedEof);
        return PARSE_EOF;
    }
    let end_char = if token_is(t, c"{") {
        c"}"
    } else if t.text() == param.type_name.as_slice() {
        let code = next_token_should_be(t, c"(", Some(c";"), None);
        if code != PARSE_SUCCESS {
            return code;
        }
        c")"
    } else if !go_to_token(t, c";", None) {
        return PARSE_EOF;
    } else {
        t.add_error(BlockError::Expecting(c"{"));
        return PARSE_CONTINUE;
    };

    let mut i = 0_i32;
    while i < intfloat_count {
        // i < intfloat_count <= i32::MAX, so i + 1 does not overflow.
        let next_i = i.saturating_add(1);
        let next = if next_i < intfloat_count {
            c","
        } else {
            end_char
        };

        let code = parse_param_assign_intfloat(t, param, is_float);
        if code != PARSE_SUCCESS {
            return code;
        }

        let code = next_token_should_be(t, next, Some(c";"), None);
        if code != PARSE_SUCCESS {
            return code;
        }
        i = next_i;
    }
    PARSE_SUCCESS
}

/// `ep_parse_param_assignment_val`, from the `=` token: the value of the
/// parser `obs_effect_param_route()` picks for the parameter's type, into
/// `param.default_val`. Returns that parser's code, or `PARSE_CONTINUE`
/// after the invalid type error (no skip).
pub fn parse_param_assignment_val(t: &mut impl ParamTokens, param: &mut Param) -> i32 {
    match effect_param_types::route(&param.type_name, param.is_texture) {
        Route::Texture => return parse_param_assign_text(t, param, c"texture path string"),
        Route::Int => return parse_param_assign_intfloat(t, param, false),
        Route::Float => return parse_param_assign_intfloat(t, param, true),
        Route::IntArray => return parse_param_assign_intfloat_array(t, param, false),
        Route::FloatArray => return parse_param_assign_intfloat_array(t, param, true),
        Route::String => return parse_param_assign_text(t, param, c"string"),
        Route::Bool => return parse_param_assign_bool(t, param),
        Route::Invalid => {}
    }

    t.add_invalid_type_error(&param.type_name);
    PARSE_CONTINUE
}

/// `ep_parse_param_assign`, from the `=` token: the value, then the next
/// valid token. `false` where the parameter is dropped.
pub fn parse_param_assign(t: &mut impl ParamTokens, param: &mut Param) -> bool {
    if parse_param_assignment_val(t, param) != PARSE_SUCCESS {
        return false;
    }
    next_valid_token(t)
}
