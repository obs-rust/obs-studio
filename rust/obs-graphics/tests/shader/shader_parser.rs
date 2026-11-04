//! Tier 1: the shader parser's sampler-state and struct blocks of
//! `libobs/graphics/shader-parser-blocks.c.h`, in
//! `obs_graphics::shader_parser`.
//!
//! `sampler_*` and `struct_*` port `test/cmocka/test_shader_parser_blocks.c`
//! one to one (same texts, in the same groups) at the block: that test runs
//! the whole `shader_parse()`, these run the block from its keyword token,
//! so each expectation is the block's own part of the C one (the sampler or
//! struct it keeps, the errors it adds) and the token it stops at, where
//! `shader_parse()` goes on. `edge_*` add what the C test leaves out. The
//! tokens are obs-util's port of the C lexer (`cf_lex`); errors are
//! formatted as obs-util's `cf_adderror` formats them, at the token's place
//! in the text.

use core::ffi::CStr;

use obs_graphics::shader_parser::{
    BlockError, Sampler, Struct, Tokens, parse_sampler_state, parse_struct,
};
use obs_util::cf_parser::{Cursor, diagnostic, pass_pair};
use obs_util::cf_tokenizer::{CfLexed, CfTokenType, cf_lex};
use proptest::prelude::*;

/// The lexed text as the C `cf_parser`, at its first token.
pub(crate) struct LexTokens {
    input: Vec<u8>,
    lexed: CfLexed,
    cur: usize,
    errors: Vec<String>,
}

impl LexTokens {
    pub(crate) fn new(text: &str) -> Self {
        Self {
            input: text.as_bytes().to_vec(),
            lexed: cf_lex(text.as_bytes()),
            cur: 0,
            errors: Vec::new(),
        }
    }

    /// The text of the token at `mark`.
    pub(crate) fn text_at(&self, mark: usize) -> String {
        let t = &self.lexed.tokens[mark];
        String::from_utf8_lossy(&self.lexed.reformatted[t.str_start..t.str_start + t.str_len])
            .into_owned()
    }

    /// Where the block stopped: the token's text, or `EOF` at the end.
    pub(crate) fn at(&self) -> String {
        if self.kind() == CfTokenType::None as i32 {
            "EOF".into()
        } else {
            String::from_utf8_lossy(self.text()).into_owned()
        }
    }
}

/// `cf_pass_pair`'s cursor, over the same tokens.
struct PairCursor<'a>(&'a mut LexTokens);

impl Cursor for PairCursor<'_> {
    fn token(&self) -> (i32, u8) {
        (self.0.kind(), self.0.text().first().copied().unwrap_or(0))
    }
    fn advance(&mut self) {
        self.0.cur += 1;
    }
}

impl Tokens for LexTokens {
    type Mark = usize;

    fn kind(&self) -> i32 {
        self.lexed.tokens[self.cur].kind as i32
    }

    fn text(&self) -> &[u8] {
        let t = &self.lexed.tokens[self.cur];
        &self.lexed.reformatted[t.str_start..t.str_start + t.str_len]
    }

    fn advance(&mut self) {
        assert_ne!(
            self.kind(),
            CfTokenType::None as i32,
            "advanced past the end"
        );
        self.cur += 1;
    }

    fn mark(&self) -> usize {
        self.cur
    }

    fn reset(&mut self, mark: usize) {
        self.cur = mark;
    }

    fn add_error(&mut self, error: BlockError) {
        let (message, val1) = error.message();
        let (row, column, text) = diagnostic(
            &self.input,
            self.lexed.tokens[self.cur].unmerged_start,
            message.to_bytes(),
            [val1.map(CStr::to_bytes), None, None],
        );
        self.errors.push(format!(
            "error {row}:{column} {} {}",
            error.level(),
            String::from_utf8_lossy(&text)
        ));
    }

    fn pass_pair(&mut self) -> bool {
        pass_pair(&mut PairCursor(self), b'{', b'}')
    }
}

fn s(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

pub(crate) fn sampler_line(ss: &Sampler) -> String {
    let mut line = format!("sampler {} {{", s(&ss.name));
    for (state, value) in ss.states.iter().zip(&ss.values) {
        line += &format!(" {} = {};", s(state), s(value));
    }
    line + " }"
}

pub(crate) fn struct_line(st: &Struct) -> String {
    let mut line = format!("struct {} {{", s(&st.name));
    for var in &st.vars {
        line += &format!(" {} {}", s(&var.type_name), s(&var.name));
        if let Some(mapping) = &var.mapping {
            line += &format!(" : {}", s(mapping));
        }
        line += ";";
    }
    line + " }"
}

/// The block's result, its errors and where it stopped, one per line.
pub(crate) fn dump(t: &LexTokens, kept: Option<String>) -> String {
    let mut out = kept.unwrap_or_else(|| "dropped".into()) + "\n";
    for error in &t.errors {
        out += error;
        out += "\n";
    }
    out + "at " + &t.at() + "\n"
}

fn run_sampler(text: &str) -> String {
    let mut t = LexTokens::new(text);
    assert_eq!(t.text(), b"sampler_state", "{text:?}");
    let kept = parse_sampler_state(&mut t);
    assert_eq!(
        kept.as_ref().map(|ss| ss.states.len()),
        kept.as_ref().map(|ss| ss.values.len())
    );
    dump(&t, kept.as_ref().map(sampler_line))
}

fn run_struct(text: &str) -> String {
    let mut t = LexTokens::new(text);
    assert_eq!(t.text(), b"struct", "{text:?}");
    let kept = parse_struct(&mut t);
    dump(&t, kept.as_ref().map(struct_line))
}

pub(crate) fn check(run: fn(&str) -> String, cases: &[(&str, &str)]) {
    let mut failed = 0;
    for (i, (text, expected)) in cases.iter().enumerate() {
        let got = run(text);
        if got != *expected {
            eprintln!("case {i}: {text:?}\n--- expected\n{expected}--- got\n{got}");
            failed += 1;
        }
    }
    assert_eq!(failed, 0, "{failed} of {} cases differ", cases.len());
}

#[test]
fn sampler_states() {
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
            (
                "sampler_state s { Filter = Linear; };",
                "sampler s { Filter = Linear; }\nat ;\n",
            ),
        ],
    );
}

#[test]
fn sampler_states_second_block() {
    // The second block of the second case, from its own keyword.
    let text = "sampler_state b { MaxAnisotropy = 4; BorderColor = 00ff00ff; };";
    assert_eq!(
        run_sampler(text),
        "sampler b { MaxAnisotropy = 4; BorderColor = 00ff00ff; }\nat ;\n"
    );
}

#[test]
fn sampler_state_errors() {
    check(
        run_sampler,
        &[
            (
                "sampler_state s { Filter Linear; AddressU = Wrap; };",
                "sampler s { AddressU = Wrap; }\nerror 1:32 0 Expected '='\nat ;\n",
            ),
            // A missing ';' skips to the next one, so the item after it goes
            // too (characterized, not endorsed).
            (
                "sampler_state s { Filter = Linear AddressU = Wrap; };",
                "sampler s { }\nerror 1:50 0 Expected ';'\nat ;\n",
            ),
            // The ';' is read as the value, so the item's own ';' is missing
            // (characterized, not endorsed).
            (
                "sampler_state s { Filter = ; AddressU = Wrap; };",
                "sampler s { }\nerror 1:45 0 Expected ';'\nat ;\n",
            ),
            (
                "sampler_state s { 1 = Linear; AddressU = Wrap; };",
                "sampler s { AddressU = Wrap; }\nerror 1:19 0 Expected 'state name'\nat ;\n",
            ),
            // A bad name skips to a ';', passing over the braces, and the
            // declaration after the block is lost with it (characterized,
            // not endorsed).
            (
                "sampler_state 1 { Filter = Linear; };\nuniform float a;",
                "dropped\nerror 1:15 0 Expected 'name'\nat ;\n",
            ),
            (
                "sampler_state s Filter = Linear;\nuniform float a;",
                "dropped\nerror 1:32 0 Expected '{'\nat ;\n",
            ),
            // No ';' after the block: the sampler is dropped, and the parser
            // goes on from the token that is there instead.
            (
                "sampler_state s { Filter = Linear; }\nuniform float a;",
                "dropped\nerror 2:1 0 Expected ';'\nat uniform\n",
            ),
            (
                "sampler_state s { Filter = Linear; } uniform float a;",
                "dropped\nerror 1:38 0 Expected ';'\nat uniform\n",
            ),
        ],
    );
}

#[test]
fn sampler_state_cut_short() {
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
            // The peek puts the cursor back on the '{', where shader_parse()
            // then meets it itself.
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
fn structs() {
    check(
        run_struct,
        &[
            (
                "struct VertInOut {\n\tfloat4 pos : POSITION;\n\tfloat2 uv : TEXCOORD0;\n\tfloat4 color;\n};\nuniform float4x4 ViewProj;",
                "struct VertInOut { float4 pos : POSITION; float2 uv : TEXCOORD0; float4 color; }\nat ;\n",
            ),
            (
                "struct A { float a; };\nstruct B { int b : X; float4x4 m; };",
                "struct A { float a; }\nat ;\n",
            ),
            ("struct S { };", "struct S { }\nat ;\n"),
            ("struct S {};", "struct S { }\nat ;\n"),
            (
                "struct S { ; float a; ; };",
                "struct S { float a; }\nat ;\n",
            ),
            ("struct S { float a; };", "struct S { float a; }\nat ;\n"),
        ],
    );
}

#[test]
fn structs_second_block() {
    assert_eq!(
        run_struct("struct B { int b : X; float4x4 m; };"),
        "struct B { int b : X; float4x4 m; }\nat ;\n"
    );
}

#[test]
fn struct_errors() {
    check(
        run_struct,
        &[
            // A bad variable is dropped with its error; the struct keeps the
            // rest.
            (
                "struct S { 1 a; float b; };",
                "struct S { float b; }\nerror 1:12 0 Expected 'type name'\nat ;\n",
            ),
            (
                "struct S { float ; float b; };",
                "struct S { float b; }\nerror 1:18 0 Syntax error\nat ;\n",
            ),
            (
                "struct S { float }; uniform float a;",
                "struct S { }\nerror 1:18 0 Syntax error\nat ;\n",
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
            // A stray token before the ';' drops the variable with no error
            // (characterized, not endorsed).
            (
                "struct S { float a b; float c; };",
                "struct S { float c; }\nat ;\n",
            ),
            // A stray token before the '}' skips to it and then steps past
            // it, so the struct runs on to the end of the text
            // (characterized, not endorsed).
            (
                "struct S { float a b }; uniform float c;",
                "dropped\nerror 1:41 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "struct S { float a : POSITION b; float c; };",
                "struct S { float c; }\nat ;\n",
            ),
            // As for a sampler state: the declaration after the block is
            // lost (characterized, not endorsed).
            (
                "struct 1 { float a; };\nuniform float b;",
                "dropped\nerror 1:8 0 Expected 'name'\nat ;\n",
            ),
            // No '{': the block skips to the ';', and shader_parse() goes on
            // from there.
            (
                "struct S float a; };\nuniform float b;",
                "dropped\nerror 1:17 0 Expected '{'\nat ;\n",
            ),
            (
                "struct S { float a; }\nuniform float b;",
                "dropped\nerror 2:1 0 Expected ';'\nat uniform\n",
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
fn edge_sampler_items() {
    check(
        run_sampler,
        &[
            // Any token is a value, a string or a number too.
            (
                "sampler_state s { BorderColor = \"x\"; MaxAnisotropy = 16; };",
                "sampler s { BorderColor = \"x\"; MaxAnisotropy = 16; }\nat ;\n",
            ),
            // Newlines and tabs between every token.
            (
                "sampler_state\n\ts\n{\n\tFilter\t=\tLinear\t;\n}\n;",
                "sampler s { Filter = Linear; }\nat ;\n",
            ),
            // A '{' where the '=' goes: the skip to ';' steps off the '{'
            // before it looks, so it stops at the ';' inside the braces,
            // and the inner '}' ends the block (characterized, not
            // endorsed).
            (
                "sampler_state s { Filter { a; }; AddressU = Wrap; };",
                "sampler s { }\nerror 1:29 0 Expected '='\nat ;\n",
            ),
            // A bad name cut short inside the braces it skips.
            (
                "sampler_state 1 { Filter",
                "dropped\nerror 1:15 0 Expected 'name'\nerror 1:25 0 Unexpected EOF\nat EOF\n",
            ),
            // A missing '}' with no ';' after it.
            (
                "sampler_state s { Filter = Linear; } ;",
                "sampler s { Filter = Linear; }\nat ;\n",
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
            // The same with a space before the ';': it is found.
            (
                "struct S { float a b {c;} ; float d; };",
                "struct S { float d; }\nat ;\n",
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

    /// The block never steps past the terminator (the cursor's assert), and
    /// a kept block holds one value per state.
    #[test]
    fn walks_stay_in_the_tokens(keyword in prop::bool::ANY, text in soup()) {
        if keyword {
            run_sampler(&format!("sampler_state {text}"));
        } else {
            run_struct(&format!("struct {text}"));
        }
    }
}
