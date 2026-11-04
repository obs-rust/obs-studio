//! Tier 1: the effect parser's struct and sampler-state blocks of
//! `libobs/graphics/effect-parser-blocks.c.h`, in
//! `obs_graphics::effect_parser`.
//!
//! `struct_*` and `sampler_*` port `test/cmocka/test_effect_parser_blocks.c`
//! one to one (same texts, in the same groups) at the block: that test runs
//! the whole `ep_parse()` behind a guard line, these run the block from its
//! keyword token, so each expectation is the block's own part of the C one
//! (the struct or sampler it keeps, the errors it adds, one row up) and the
//! token it stops at, where `ep_parse()` goes on. The C test's two texts
//! that start with a stray `}` never reach a block (`ep_parse_other()`
//! skips them), so they are left to that test and Tier 3. `edge_*` add what
//! the C test leaves out. The tokens, the cursor and the error format are
//! the shader parser's Tier 1 ones (`super::shader_parser`): obs-util's port
//! of the C lexer, and `cf_adderror`'s text at the token's place.

use obs_graphics::effect_parser::{parse_sampler_state, parse_struct};
use proptest::prelude::*;

use super::shader_parser::{LexTokens, check, dump, sampler_line, struct_line};

fn run_struct(text: &str) -> String {
    let mut t = LexTokens::new(text);
    let kept = parse_struct(&mut t);
    dump(&t, kept.as_ref().map(struct_line))
}

fn run_sampler(text: &str) -> String {
    let mut t = LexTokens::new(text);
    let kept = parse_sampler_state(&mut t);
    assert_eq!(
        kept.as_ref().map(|eps| eps.states.len()),
        kept.as_ref().map(|eps| eps.values.len())
    );
    dump(&t, kept.as_ref().map(sampler_line))
}

#[test]
fn structs() {
    check(
        run_struct,
        &[
            (
                "struct VertInOut { float4 pos : POSITION; float2 uv : TEXCOORD0; };\nuniform float4 c;",
                "struct VertInOut { float4 pos : POSITION; float2 uv : TEXCOORD0; }\nat ;\n",
            ),
            (
                "struct A { float a; int b; };\nstruct B { float4 c : COLOR; };",
                "struct A { float a; int b; }\nat ;\n",
            ),
            ("struct E { };", "struct E { }\nat ;\n"),
            ("struct E {};", "struct E { }\nat ;\n"),
            (
                "struct S { float a; float a; };",
                "struct S { float a; float a; }\nat ;\n",
            ),
            (
                "struct S\n{\n\tfloat4 pos : POSITION;\n\tfloat2 uv  : TEXCOORD0;\n};",
                "struct S { float4 pos : POSITION; float2 uv : TEXCOORD0; }\nat ;\n",
            ),
        ],
    );
}

#[test]
fn structs_second_block() {
    // The second blocks of the C test's two-struct and mixed texts.
    check(
        run_struct,
        &[
            (
                "struct B { float4 c : COLOR; };",
                "struct B { float4 c : COLOR; }\nat ;\n",
            ),
            ("struct S { float a; };", "struct S { float a; }\nat ;\n"),
        ],
    );
}

#[test]
fn struct_errors() {
    check(
        run_struct,
        &[
            (
                "struct S { float a;; float b; };",
                "struct S { float a; float b; }\nat ;\n",
            ),
            // A missing name skips to the ';' inside the body, and
            // ep_parse() reads the '}' after it at the top level
            // (characterized, not endorsed).
            (
                "struct { float a; };\nuniform float c;",
                "dropped\nerror 1:8 0 Expected 'name'\nat ;\n",
            ),
            // The skip after a bad name passes the braced body whole and
            // stops at the ';' after it (characterized, not endorsed).
            (
                "struct 1 { float a; };\nuniform float c;",
                "dropped\nerror 1:8 0 Expected 'name'\nat ;\n",
            ),
            (
                "struct S float a; };\nuniform float c;",
                "dropped\nerror 1:17 0 Expected '{'\nat ;\n",
            ),
            (
                "struct S { 1 a; float b; };",
                "struct S { float b; }\nerror 1:12 0 Expected 'type name'\nat ;\n",
            ),
            (
                "struct S { float ; float b; };",
                "struct S { float b; }\nerror 1:18 0 Syntax error\nat ;\n",
            ),
            (
                "struct S { float }; uniform float c;",
                "struct S { }\nerror 1:18 0 Syntax error\nat ;\n",
            ),
            (
                "struct S { float 1; float b; };",
                "struct S { float b; }\nerror 1:18 0 Expected 'variable name'\nat ;\n",
            ),
            (
                "struct S { float a : ; float b; };",
                "struct S { float b; }\nerror 1:22 0 Syntax error\nat ;\n",
            ),
            (
                "struct S { float a : }; uniform float c;",
                "struct S { }\nerror 1:22 0 Syntax error\nat ;\n",
            ),
            (
                "struct S { float a : 1; float b; };",
                "struct S { float b; }\nerror 1:22 0 Expected 'mapping name'\nat ;\n",
            ),
            (
                "struct S { float : POSITION; float b; };",
                "struct S { float b; }\nerror 1:18 0 Expected 'variable name'\nat ;\n",
            ),
            // A stray token before ';' drops the variable with no error
            // (characterized, not endorsed).
            (
                "struct S { float a b; float c; };",
                "struct S { float c; }\nat ;\n",
            ),
            // The same after a mapping (characterized, not endorsed).
            (
                "struct S { float a : POSITION b; float c; };",
                "struct S { float c; }\nat ;\n",
            ),
            // A skip that stops at '}' is not taken as the end of the
            // struct: it reads on to the end of the text (characterized,
            // not endorsed).
            (
                "struct S { float a b }; uniform float c;",
                "dropped\nerror 1:41 0 Unexpected EOF\nat EOF\n",
            ),
            // No ';' after the block: the struct is dropped, and ep_parse()
            // goes on from the token that is there instead.
            (
                "struct S { float a; }\nuniform float c;",
                "dropped\nerror 2:1 0 Expected ';'\nat uniform\n",
            ),
            (
                "struct S { float a; } x;\nuniform float c;",
                "dropped\nerror 1:23 0 Expected ';'\nat x\n",
            ),
        ],
    );
}

#[test]
fn struct_cut_short() {
    check(
        run_struct,
        &[
            ("struct", "dropped\nerror 1:7 0 Unexpected EOF\nat EOF\n"),
            ("struct S", "dropped\nerror 1:9 0 Unexpected EOF\nat EOF\n"),
            (
                "struct S {",
                "dropped\nerror 1:11 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "struct S { float",
                "dropped\nerror 1:17 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "struct S { float a",
                "dropped\nerror 1:19 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "struct S { float a :",
                "dropped\nerror 1:21 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "struct S { float a : POSITION",
                "dropped\nerror 1:30 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "struct S { float a : POSITION b",
                "dropped\nerror 1:32 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "struct S { float a;",
                "dropped\nerror 1:20 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "struct S { float a; }",
                "dropped\nerror 1:22 0 Unexpected EOF\nat EOF\n",
            ),
        ],
    );
}

#[test]
fn samplers() {
    check(
        run_sampler,
        &[
            (
                "sampler_state def_sampler { Filter = Linear; AddressU = Clamp; AddressV = Wrap; };\nuniform float4 c;",
                "sampler def_sampler { Filter = Linear; AddressU = Clamp; AddressV = Wrap; }\nat ;\n",
            ),
            (
                "sampler_state a { Filter = Point; };\nsampler_state b { MaxAnisotropy = 4; BorderColor = 00ff00ff; };",
                "sampler a { Filter = Point; }\nat ;\n",
            ),
            ("sampler_state s { };", "sampler s { }\nat ;\n"),
            ("sampler_state s {};", "sampler s { }\nat ;\n"),
            (
                "sampler_state s { Filter = Linear; Filter = Point; };",
                "sampler s { Filter = Linear; Filter = Point; }\nat ;\n",
            ),
            // A value's tokens are joined without their spaces.
            (
                "sampler_state s { BorderColor = 0 0 0 1; };",
                "sampler s { BorderColor = 0001; }\nat ;\n",
            ),
            (
                "sampler_state s\n{\n\tFilter   = Linear;\n\tAddressU = Clamp;\n};",
                "sampler s { Filter = Linear; AddressU = Clamp; }\nat ;\n",
            ),
        ],
    );
}

#[test]
fn samplers_second_block() {
    // The second blocks of the C test's two-sampler and mixed texts.
    check(
        run_sampler,
        &[
            (
                "sampler_state b { MaxAnisotropy = 4; BorderColor = 00ff00ff; };",
                "sampler b { MaxAnisotropy = 4; BorderColor = 00ff00ff; }\nat ;\n",
            ),
            (
                "sampler_state def_sampler { Filter = Linear; };\nuniform float4x4 ViewProj;",
                "sampler def_sampler { Filter = Linear; }\nat ;\n",
            ),
            (
                "sampler_state t { AddressU = Wrap; };",
                "sampler t { AddressU = Wrap; }\nat ;\n",
            ),
        ],
    );
}

#[test]
fn sampler_errors() {
    check(
        run_sampler,
        &[
            (
                "sampler_state s { Filter Linear; AddressU = Wrap; };",
                "sampler s { AddressU = Wrap; }\nerror 1:32 0 Expected '='\nat ;\n",
            ),
            // A missing ';' joins the next item into the value, with no
            // error (characterized, not endorsed).
            (
                "sampler_state s { Filter = Linear AddressU = Wrap; };",
                "sampler s { Filter = LinearAddressU=Wrap; }\nat ;\n",
            ),
            // An empty value drops the item with no error (characterized,
            // not endorsed).
            (
                "sampler_state s { Filter = ; AddressU = Wrap; };",
                "sampler s { AddressU = Wrap; }\nat ;\n",
            ),
            ("sampler_state s { Filter = ; };", "sampler s { }\nat ;\n"),
            (
                "sampler_state s { 1 = Linear; AddressU = Wrap; };",
                "sampler s { AddressU = Wrap; }\nerror 1:19 0 Expected 'state name'\nat ;\n",
            ),
            // The skip after a missing state name steps over the ';' it
            // starts on, so the next item goes too (characterized, not
            // endorsed).
            (
                "sampler_state s { ; AddressU = Wrap; };",
                "sampler s { }\nerror 1:19 0 Expected 'state name'\nat ;\n",
            ),
            (
                "sampler_state { Filter = Linear; };\nuniform float c;",
                "dropped\nerror 1:15 0 Expected 'name'\nat ;\n",
            ),
            (
                "sampler_state 1 { Filter = Linear; };\nuniform float c;",
                "dropped\nerror 1:15 0 Expected 'name'\nat ;\n",
            ),
            (
                "sampler_state s Filter = Linear; };\nuniform float c;",
                "dropped\nerror 1:32 0 Expected '{'\nat ;\n",
            ),
            (
                "sampler_state s { Filter = Linear; }\nuniform float c;",
                "dropped\nerror 2:1 0 Expected ';'\nat uniform\n",
            ),
            (
                "sampler_state s { Filter = Linear; } x;\nuniform float c;",
                "dropped\nerror 1:38 0 Expected ';'\nat x\n",
            ),
            // A missing ';' reads the value on past the '}', so the block
            // runs to the end of the text; the peek puts the cursor back on
            // the last ';' (characterized, not endorsed).
            (
                "sampler_state s { Filter = Linear }; uniform float c;",
                "dropped\nerror 1:53 0 Expected '='\nerror 1:53 0 Unexpected EOF\nat ;\n",
            ),
        ],
    );
}

#[test]
fn sampler_cut_short() {
    check(
        run_sampler,
        &[
            (
                "sampler_state",
                "dropped\nerror 1:14 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "sampler_state s",
                "dropped\nerror 1:16 0 Unexpected EOF\nat EOF\n",
            ),
            // The peek puts the cursor back on the '{', where ep_parse()
            // then meets it itself (characterized, not endorsed).
            (
                "sampler_state s {",
                "dropped\nerror 1:17 0 Unexpected EOF\nat {\n",
            ),
            (
                "sampler_state s { Filter",
                "dropped\nerror 1:25 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "sampler_state s { Filter =",
                "dropped\nerror 1:27 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "sampler_state s { Filter = Linear",
                "dropped\nerror 1:34 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "sampler_state s { Filter = Linear Point",
                "dropped\nerror 1:40 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "sampler_state s { Filter = Linear;",
                "dropped\nerror 1:34 0 Unexpected EOF\nat ;\n",
            ),
            (
                "sampler_state s { Filter = Linear; }",
                "dropped\nerror 1:37 0 Unexpected EOF\nat EOF\n",
            ),
        ],
    );
}

#[test]
fn edge_sampler_values() {
    check(
        run_sampler,
        &[
            // Any token is part of a value: strings, numbers, operators.
            (
                "sampler_state s { BorderColor = \"x\"; MaxLOD = -1.5; };",
                "sampler s { BorderColor = \"x\"; MaxLOD = -1.5; }\nat ;\n",
            ),
            // A string holding a ';' does not end the value: only a token
            // that starts with one does.
            (
                "sampler_state s { A = \";\" b; };",
                "sampler s { A = \";\"b; }\nat ;\n",
            ),
            // Newlines and tabs between the value's tokens are dropped too.
            (
                "sampler_state s { A =\n\t0\n\t1\t;\n};",
                "sampler s { A = 01; }\nat ;\n",
            ),
            // A '{' in a value is joined in, not skipped as a pair, and the
            // '}' after the item then ends the block (characterized, not
            // endorsed).
            (
                "sampler_state s { A = { b; }; };",
                "sampler s { A = {b; }\nat ;\n",
            ),
            // A '=' where the value goes is part of it.
            (
                "sampler_state s { A = = b; };",
                "sampler s { A = =b; }\nat ;\n",
            ),
            // Two empty items, then a kept one.
            (
                "sampler_state s { A = ; B = ; C = c; };",
                "sampler s { C = c; }\nat ;\n",
            ),
            // A value cut short after several tokens.
            (
                "sampler_state s { A = 0 0 0",
                "dropped\nerror 1:28 0 Unexpected EOF\nat EOF\n",
            ),
            // A state name cut short inside the item after an empty one.
            (
                "sampler_state s { A = ; B",
                "dropped\nerror 1:26 0 Unexpected EOF\nat EOF\n",
            ),
            // A bad name cut short inside the braces it skips.
            (
                "sampler_state 1 { Filter",
                "dropped\nerror 1:15 0 Expected 'name'\nerror 1:25 0 Unexpected EOF\nat EOF\n",
            ),
        ],
    );
}

#[test]
fn edge_struct_vars() {
    check(
        run_struct,
        &[
            // A mapping then '}' with no ';': the skip to ';' or '}' steps
            // past the '}' first, so the struct runs on to the end.
            (
                "struct S { float a : P };",
                "dropped\nerror 1:26 0 Unexpected EOF\nat EOF\n",
            ),
            // A stray token then braces: the skip passes over them, and the
            // ';' right after the '}' goes too, so the next variable is lost.
            (
                "struct S { float a b {c;}; float d; };",
                "struct S { }\nat ;\n",
            ),
            // Two mappings: the second is the stray token.
            (
                "struct S { float a : P : Q; float b; };",
                "struct S { float b; }\nat ;\n",
            ),
            // A var type of a number, cut short in its skip.
            (
                "struct S { 1",
                "dropped\nerror 1:12 0 Expected 'type name'\nerror 1:13 0 Unexpected EOF\nat EOF\n",
            ),
        ],
    );
}

/// A token soup over the blocks' vocabulary: every keyword, name, number,
/// string, delimiter and space the blocks compare with or skip.
fn soup() -> impl Strategy<Value = String> {
    let word = prop::sample::select(vec![
        "s", "Filter", "Linear", "float", "a", "POSITION", "1", "\"x\"", "=", ";", ":", "{", "}",
        "(", ")", " ", "\n", "\t",
    ]);
    prop::collection::vec(word, 0..24).prop_map(|words| words.join(" "))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]

    /// The block never steps past the terminator (the cursor's assert), a
    /// kept sampler holds one value per state, and no kept value is empty.
    #[test]
    fn walks_stay_in_the_tokens(keyword in prop::bool::ANY, text in soup()) {
        if keyword {
            let mut t = LexTokens::new(&format!("sampler_state {text}"));
            if let Some(eps) = parse_sampler_state(&mut t) {
                prop_assert_eq!(eps.states.len(), eps.values.len());
                prop_assert!(eps.values.iter().all(|v| !v.is_empty()));
            }
        } else {
            run_struct(&format!("struct {text}"));
        }
    }
}
