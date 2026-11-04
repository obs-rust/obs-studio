//! Safe core of `libobs/graphics/effect-parser-blocks.c.h` and
//! `libobs/graphics/effect-parser-passes.c.h`: the effect parser's struct
//! and sampler-state blocks, and its passes and techniques.
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
//!   an optional `compile` before a call, skipped with a warning.
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
//! No intentional differences. The C freed a dropped item's state name and
//! value on every path already (#3550), and a dropped technique's name
//! (#3598), each fixed in the C before its port.

pub use crate::shader_parser::{BlockError, Sampler, SamplerText, Struct, StructVar, Tokens};
use crate::shader_parser::{
    PARSE_CONTINUE, PARSE_EOF, PARSE_SUCCESS, PARSE_UNEXPECTED_CONTINUE, first_byte, go_to_token,
    go_to_valid_token, next_name, next_token, next_token_should_be, next_valid_token,
    peek_valid_token_is, token_is,
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
