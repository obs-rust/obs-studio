//! Safe core of `libobs/graphics/effect-parser-blocks.c.h`: the effect
//! parser's struct and sampler-state blocks.
//!
//! - [`parse_struct`] (`ep_parse_struct`, with `ep_parse_struct_var`):
//!   `struct name { type name [: MAPPING]; ... };`;
//! - [`parse_sampler_state`] (`ep_parse_sampler_state`, with
//!   `ep_parse_sampler_state_item`): `sampler_state name { state = value;
//!   ... };`.
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
//! No intentional differences. The C freed a dropped item's state name and
//! value on every path already (#3550, fixed in the C before this port).

pub use crate::shader_parser::{BlockError, Sampler, SamplerText, Struct, StructVar, Tokens};
use crate::shader_parser::{
    PARSE_EOF, PARSE_SUCCESS, first_byte, next_name, next_token_should_be, next_valid_token,
    peek_valid_token_is,
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
