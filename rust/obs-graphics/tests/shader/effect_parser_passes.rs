//! Tier 1: the effect parser's passes and techniques of
//! `libobs/graphics/effect-parser-passes.c.h`, in
//! `obs_graphics::effect_parser`.
//!
//! `techniques`, `pass_errors`, `technique_errors`, `eof` and `mixed` port
//! `test/cmocka/test_effect_parser_passes.c` one to one (same texts, in the
//! same groups) at the technique: that test runs the whole `ep_parse()`
//! behind a guard line, these run the technique from its keyword token, so
//! each expectation is the technique's own part of the C one (the technique
//! it keeps, the errors it adds, one row up) and the token it stops at,
//! where `ep_parse()` goes on. The C test's text that starts with a stray
//! `}` never reaches a technique (`ep_parse_other()` skips it), so it is
//! left to that test and Tier 3. A pass's programs are written as the C
//! test writes them: each token's text, and the end token as `<end>`.
//! `edge_techniques` adds what the C test leaves out, and `pass_returns`
//! and `pass_command_returns` pin the codes `ep_parse_pass` and
//! `ep_parse_pass_command` return. The tokens, the cursor and the error
//! format are the shader parser's Tier 1 ones (`super::shader_parser`):
//! obs-util's port of the C lexer, and `cf_adderror`'s text at the token's
//! place.

use obs_graphics::effect_parser::{
    Pass, ProgramToken, Technique, parse_pass, parse_pass_command, parse_technique,
};
use obs_graphics::shader_parser::Tokens;
use proptest::prelude::*;

use super::shader_parser::{LexTokens, check, dump};

fn program(t: &LexTokens, label: &str, tokens: &[ProgramToken<usize>]) -> String {
    let texts: Vec<String> = tokens
        .iter()
        .map(|token| match token {
            ProgramToken::Token(mark) => t.text_at(*mark),
            ProgramToken::End => "<end>".into(),
        })
        .collect();
    format!(" {label} [{}]", texts.join(" "))
}

fn pass_line(t: &LexTokens, pass: &Pass<usize>) -> String {
    let name = pass.name.as_deref().map_or_else(
        || "(null)".into(),
        |n| String::from_utf8_lossy(n).into_owned(),
    );
    format!(
        " pass {name} {{{}{} }}",
        program(t, "vs", &pass.vertex_program),
        program(t, "ps", &pass.fragment_program)
    )
}

fn technique_line(t: &LexTokens, ept: &Technique<usize>) -> String {
    let mut line = format!("technique {} {{", String::from_utf8_lossy(&ept.name));
    for pass in &ept.passes {
        line += &pass_line(t, pass);
    }
    line + " }"
}

fn run(text: &str) -> String {
    let mut t = LexTokens::new(text);
    assert_eq!(t.text(), b"technique", "{text:?}");
    let kept = parse_technique(&mut t);
    let line = kept.as_ref().map(|ept| technique_line(&t, ept));
    dump(&t, line)
}

fn run_pass(text: &str) -> String {
    let mut t = LexTokens::new(text);
    let mut pass = Pass::default();
    let code = parse_pass(&mut t, &mut pass);
    let line = format!("code {code}{}", pass_line(&t, &pass));
    dump(&t, Some(line))
}

fn run_command(text: &str) -> String {
    let mut t = LexTokens::new(text);
    assert_eq!(t.text(), b"{", "{text:?}");
    let mut pass = Pass::default();
    let code = parse_pass_command(&mut t, &mut pass);
    let line = format!("code {code}{}", pass_line(&t, &pass));
    dump(&t, Some(line))
}

#[test]
fn techniques() {
    check(
        run,
        &[
            (
                "technique Draw { pass { vertex_shader = VSDefault(vert_in); pixel_shader = PSDrawBare(vert_in); } }",
                "technique Draw { pass (null) { vs [VSDefault ( vert_in ) ; <end>] ps [PSDrawBare ( vert_in ) ; <end>] } }\nat EOF\n",
            ),
            (
                "technique T { pass P0 { vertex_program = VS(v); pixel_program = PS(v); } }",
                "technique T { pass P0 { vs [VS ( v ) ; <end>] ps [PS ( v ) ; <end>] } }\nat EOF\n",
            ),
            (
                "technique T { pass P0 { vertex_shader = VS(v); } pass P1 { pixel_shader = PS(v); } }\ntechnique U { pass { pixel_program = PS2(v, 1.0); vertex_program = VS2(v); } }\nuniform float c;",
                "technique T { pass P0 { vs [VS ( v ) ; <end>] ps [] } pass P1 { vs [] ps [PS ( v ) ; <end>] } }\nat technique\n",
            ),
            // A second command for the same program is appended after the first
            // one's end token (characterized, not endorsed).
            (
                "technique T { pass { vertex_shader = VS(v); vertex_shader = VS2(v); } }",
                "technique T { pass (null) { vs [VS ( v ) ; <end> VS2 ( v ) ; <end>] ps [] } }\nat EOF\n",
            ),
            (
                "technique T { pass { } }",
                "technique T { pass (null) { vs [] ps [] } }\nat EOF\n",
            ),
            (
                "technique T { pass {} }",
                "technique T { pass (null) { vs [] ps [] } }\nat EOF\n",
            ),
            ("technique T { }", "technique T { }\nat EOF\n"),
            (
                "technique T {}\nuniform float c;",
                "technique T { }\nat uniform\n",
            ),
            (
                "technique Draw\n{\n\tpass\n\t{\n\t\tvertex_shader = VSDefault(vert_in);\n\t\tpixel_shader  = PSDrawBare(vert_in);\n\t}\n}",
                "technique Draw { pass (null) { vs [VSDefault ( vert_in ) ; <end>] ps [PSDrawBare ( vert_in ) ; <end>] } }\nat EOF\n",
            ),
            // "compile" is skipped with a warning.
            (
                "technique T { pass { vertex_shader = compile VS(v); pixel_shader = compile PS(v); } }",
                "technique T { pass (null) { vs [VS ( v ) ; <end>] ps [PS ( v ) ; <end>] } }\nerror 1:38 1 compile keyword not necessary\nerror 1:68 1 compile keyword not necessary\nat EOF\n",
            ),
            // A pass name is any token.
            (
                "technique T { pass 1 { vertex_shader = VS(v); } }",
                "technique T { pass 1 { vs [VS ( v ) ; <end>] ps [] } }\nat EOF\n",
            ),
            // A call is every token up to ';', whatever it is.
            (
                "technique T { pass { vertex_shader = 1 + { x ); } }",
                "technique T { pass (null) { vs [1 + { x ) ; <end>] ps [] } }\nat EOF\n",
            ),
            // An empty call keeps its ';' and the end token
            // (characterized, not endorsed).
            (
                "technique T { pass { vertex_shader = ; } }",
                "technique T { pass (null) { vs [; <end>] ps [] } }\nat EOF\n",
            ),
        ],
    );
}

/// The second technique of the C test's two-technique text.
#[test]
fn techniques_second_block() {
    check(
        run,
        &[(
            "technique U { pass { pixel_program = PS2(v, 1.0); vertex_program = VS2(v); } }\nuniform float c;",
            "technique U { pass (null) { vs [VS2 ( v ) ; <end>] ps [PS2 ( v , 1.0 ) ; <end>] } }\nat uniform\n",
        )],
    );
}

#[test]
fn pass_errors() {
    check(
        run,
        &[
            // A missing ';' before the pass's '}' stops the call there, but that
            // '}' is not taken as the end of the pass: the technique's is, so the
            // technique reads on into the param and is dropped with it, with no
            // error for the end of the text (characterized, not endorsed).
            (
                "technique T { pass { vertex_shader = VS(v) } }\nuniform float c;",
                "dropped\nerror 1:44 0 Expected ';'\nat EOF\n",
            ),
            // The same with a second pass: its tokens are read as commands of the
            // first one (characterized, not endorsed).
            (
                "technique T { pass { vertex_shader = VS(v) } pass Q { pixel_shader = PS(v); } }\nuniform float c;",
                "dropped\nerror 1:44 0 Expected ';'\nerror 1:46 0 Syntax error\nerror 2:1 0 Syntax error\nerror 2:16 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "technique T { pass { geometry_shader = GS(v); vertex_shader = VS(v); } }",
                "technique T { pass (null) { vs [VS ( v ) ; <end>] ps [] } }\nerror 1:22 0 Syntax error\nat EOF\n",
            ),
            // An unknown command skips to the next ';' or '}'; stopping on the '}'
            // is as above (characterized, not endorsed).
            (
                "technique T { pass { geometry_shader = GS(v) } }\nuniform float c;",
                "dropped\nerror 1:22 0 Syntax error\nat EOF\n",
            ),
            (
                "technique T { pass { vertex_shader VS(v); pixel_shader = PS(v); } }",
                "technique T { pass (null) { vs [] ps [PS ( v ) ; <end>] } }\nerror 1:41 0 Expected '='\nat EOF\n",
            ),
            (
                "technique T { pass { vertex_shader VS(v) } }\nuniform float c;",
                "dropped\nerror 1:42 0 Expected '='\nat EOF\n",
            ),
            // A stray ';' is a syntax error, and the skip steps over it to the
            // next ';', so the command after it goes too (characterized, not endorsed).
            (
                "technique T { pass { ; vertex_shader = VS(v); } }",
                "technique T { pass (null) { vs [] ps [] } }\nerror 1:22 0 Syntax error\nat EOF\n",
            ),
            (
                "technique T { pass { vertex_shader = compile ; } }",
                "technique T { pass (null) { vs [; <end>] ps [] } }\nerror 1:38 1 compile keyword not necessary\nat EOF\n",
            ),
            // A pass without '{' takes the next token as its name and reads the
            // rest as commands up to a '}', which ends the pass; the next '}' ends
            // the technique (characterized, not endorsed).
            (
                "technique T { pass vertex_shader = VS(v); } }\nuniform float c;",
                "technique T { pass vertex_shader { vs [] ps [] } }\nerror 1:36 0 Syntax error\nat uniform\n",
            ),
            // A second name is a syntax error at the '{', and the skip takes the
            // first command with it (characterized, not endorsed).
            (
                "technique T { pass P Q { vertex_shader = VS(v); } }\nuniform float c;",
                "technique T { pass P { vs [] ps [] } }\nerror 1:24 0 Syntax error\nat uniform\n",
            ),
            // A stray ';' after a pass skips to the technique's '}' with no error
            // (characterized, not endorsed).
            (
                "technique T { pass { vertex_shader = VS(v); }; }\nuniform float c;",
                "technique T { pass (null) { vs [VS ( v ) ; <end>] ps [] } }\nat uniform\n",
            ),
        ],
    );
}

#[test]
fn technique_errors() {
    check(
        run,
        &[
            // An entry other than a pass skips to the next '}' (past any braces),
            // and that '}' ends the technique: the passes after it go, with no
            // error (characterized, not endorsed).
            (
                "technique T { foo { } pass { vertex_shader = VS(v); } }\nuniform float c;",
                "technique T { }\nat uniform\n",
            ),
            (
                "technique T { foo; pass { vertex_shader = VS(v); } }\nuniform float c;",
                "technique T { }\nat uniform\n",
            ),
            (
                "technique T { pass { vertex_shader = VS(v); } foo }\nuniform float c;",
                "technique T { pass (null) { vs [VS ( v ) ; <end>] ps [] } }\nat uniform\n",
            ),
            (
                "technique T { ; pass { vertex_shader = VS(v); } }\nuniform float c;",
                "technique T { }\nat uniform\n",
            ),
            // A technique without '{' skips to the next ';' (past any braces) and
            // is dropped.
            (
                "technique T pass { vertex_shader = VS(v); } };\nuniform float c;",
                "dropped\nerror 1:46 0 Expected '{'\nat uniform\n",
            ),
            // The skip starts after the ';' it is on, so the next statement goes
            // too (characterized, not endorsed).
            (
                "technique T;\nuniform float c;",
                "dropped\nerror 2:16 0 Expected '{'\nat EOF\n",
            ),
            // With no ';' after it the technique is dropped, its name freed with
            // it (#3598).
            (
                "technique T pass { vertex_shader = VS(v); } }\nuniform float c",
                "dropped\nerror 2:16 0 Expected ';'\nat EOF\n",
            ),
            // A missing or bad name skips to the next ';' past the braces, so the
            // param goes too (characterized, not endorsed).
            (
                "technique { pass { vertex_shader = VS(v); } }\nuniform float c;",
                "dropped\nerror 1:11 0 Expected 'name'\nat EOF\n",
            ),
            (
                "technique 1 { pass { vertex_shader = VS(v); } }\nuniform float c;",
                "dropped\nerror 1:11 0 Expected 'name'\nat EOF\n",
            ),
            (
                "technique T { pass { vertex_shader = VS(v); } }; uniform float c;",
                "technique T { pass (null) { vs [VS ( v ) ; <end>] ps [] } }\nat ;\n",
            ),
            (
                "technique T { pass { vertex_shader = VS(v); } } uniform float c;",
                "technique T { pass (null) { vs [VS ( v ) ; <end>] ps [] } }\nat uniform\n",
            ),
        ],
    );
}

#[test]
fn eof() {
    check(
        run,
        &[
            (
                "technique",
                "dropped\nerror 1:10 0 Unexpected EOF\nat EOF\n",
            ),
            // End of text after the name frees the name (#3598).
            (
                "technique T",
                "dropped\nerror 1:12 0 Unexpected EOF\nat EOF\n",
            ),
            // As does a technique without '{' that runs to the end of the text
            // (#3598).
            (
                "technique T pass",
                "dropped\nerror 1:17 0 Expected ';'\nat EOF\n",
            ),
            (
                "technique T {",
                "dropped\nerror 1:14 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "technique T { pass",
                "dropped\nerror 1:19 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "technique T { pass P",
                "dropped\nerror 1:21 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "technique T { pass {",
                "dropped\nerror 1:20 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "technique T { pass P {",
                "dropped\nerror 1:22 0 Unexpected EOF\nat EOF\n",
            ),
            // End of text after a command's name is reported twice: once by the
            // '=' check, then by the pass loop (characterized, not endorsed).
            (
                "technique T { pass { vertex_shader",
                "dropped\nerror 1:35 0 Unexpected EOF\nerror 1:35 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "technique T { pass { vertex_shader =",
                "dropped\nerror 1:37 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "technique T { pass { vertex_shader = compile",
                "dropped\nerror 1:38 1 compile keyword not necessary\nerror 1:45 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "technique T { pass { vertex_shader = VS(v)",
                "dropped\nerror 1:43 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "technique T { pass { vertex_shader = VS(v);",
                "dropped\nerror 1:43 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "technique T { pass { vertex_shader = VS(v); }",
                "dropped\nerror 1:46 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "technique T { pass { geometry_shader",
                "dropped\nerror 1:22 0 Syntax error\nerror 1:37 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "technique T { pass { geometry_shader = GS(v)",
                "dropped\nerror 1:22 0 Syntax error\nerror 1:45 0 Unexpected EOF\nat EOF\n",
            ),
            // End of text in the skip after an entry other than a pass adds no
            // error (characterized, not endorsed).
            ("technique T { foo", "dropped\nat EOF\n"),
            ("technique T { foo {", "dropped\nat EOF\n"),
        ],
    );
}

/// The techniques of the C test's mixed texts, from their keyword (the
/// struct, param and function before them are the blocks' and
/// `ep_parse()`'s).
#[test]
fn mixed() {
    check(
        run,
        &[
            (
                "technique Draw { pass { vertex_shader = VSDefault(vert_in); pixel_shader = PSDraw(vert_in); } }\ntechnique Other { pass { vertex_shader = VSDefault(vert_in); } }",
                "technique Draw { pass (null) { vs [VSDefault ( vert_in ) ; <end>] ps [PSDraw ( vert_in ) ; <end>] } }\nat technique\n",
            ),
            (
                "technique Other { pass { vertex_shader = VSDefault(vert_in); } }",
                "technique Other { pass (null) { vs [VSDefault ( vert_in ) ; <end>] ps [] } }\nat EOF\n",
            ),
            // A missing ';' before a pass's '}' takes the next technique with it
            // (characterized, not endorsed).
            (
                "technique T { pass { vertex_shader = VS(v) } }\ntechnique U { pass { pixel_shader = PS(v); } }",
                "dropped\nerror 1:44 0 Expected ';'\nat EOF\n",
            ),
            // A technique without '{' skips past the next technique's braces to
            // the end of the text: both go (characterized, not endorsed). The first
            // one's name is freed (#3598).
            (
                "technique T\ntechnique U { pass { pixel_shader = PS(v); } }",
                "dropped\nerror 2:47 0 Expected ';'\nat EOF\n",
            ),
        ],
    );
}

#[test]
fn edge_techniques() {
    check(
        run,
        &[
            // A '}' inside a command keeps the call's tokens with no end token; its
            // error keeps ep_compile() from walking them (characterized, not endorsed).
            (
                "technique T { pass { vertex_shader = VS(v) } } }",
                "technique T { pass (null) { vs [VS ( v )] ps [] } }\nerror 1:44 0 Expected ';'\nat EOF\n",
            ),
            // Both programs, in any order and spelling, the pixel one twice.
            (
                "technique T { pass { pixel_shader = A; vertex_program = B; pixel_program = C; } }",
                "technique T { pass (null) { vs [B ; <end>] ps [A ; <end> C ; <end>] } }\nat EOF\n",
            ),
            // A string holding a ';', a character and a number in a call.
            (
                "technique T { pass { vertex_shader = VS(\"x;\", 'y', 0.5); } }",
                "technique T { pass (null) { vs [VS ( \"x;\" , 'y' , 0.5 ) ; <end>] ps [] } }\nat EOF\n",
            ),
            // Three passes: named, unnamed, named by a string.
            (
                "technique T { pass a {} pass {} pass \"b\" {} }",
                "technique T { pass a { vs [] ps [] } pass (null) { vs [] ps [] } pass \"b\" { vs [] ps [] } }\nat EOF\n",
            ),
            // Only the first "compile" is skipped; the second is a token of the
            // call.
            (
                "technique T { pass { vertex_shader = compile compile VS(v); } }",
                "technique T { pass (null) { vs [compile VS ( v ) ; <end>] ps [] } }\nerror 1:38 1 compile keyword not necessary\nat EOF\n",
            ),
            // Comments inside a call are not tokens.
            (
                "technique T { pass { vertex_shader = VS( // c\n v /* d */ ); } }",
                "technique T { pass (null) { vs [VS ( v ) ; <end>] ps [] } }\nat EOF\n",
            ),
            // A missing '=' with no ';' after it skips to the pass's '}', which is
            // then not the end of the pass (characterized, not endorsed).
            (
                "technique T { pass { vertex_shader VS(v) } pass { pixel_shader = PS(v); } }",
                "dropped\nerror 1:42 0 Expected '='\nerror 1:44 0 Syntax error\nerror 1:75 0 Unexpected EOF\nat EOF\n",
            ),
            // An unknown command's skip passes braces, here to the end of the text.
            (
                "technique T { pass { geometry_shader = { x; ",
                "dropped\nerror 1:22 0 Syntax error\nerror 1:45 0 Unexpected EOF\nat EOF\n",
            ),
            // An entry other than a pass whose skip reaches the end of the text.
            ("technique T { foo { pass { } ", "dropped\nat EOF\n"),
            // A pass named '}': the name is any token (characterized, not endorsed).
            (
                "technique T { pass } { vertex_shader = VS(v); } }",
                "technique T { pass } { vs [VS ( v ) ; <end>] ps [] } }\nat EOF\n",
            ),
            // A bad name with nothing after it.
            (
                "technique 1",
                "dropped\nerror 1:11 0 Expected 'name'\nerror 1:12 0 Unexpected EOF\nat EOF\n",
            ),
            // A bad name whose skip to ';' passes the braces to the end of the
            // text.
            (
                "technique 1 { pass {} } x",
                "dropped\nerror 1:11 0 Expected 'name'\nerror 1:26 0 Unexpected EOF\nat EOF\n",
            ),
        ],
    );
}

/// `ep_parse_pass`'s codes, from the technique's entry token.
#[test]
fn pass_returns() {
    check(
        run_pass,
        &[
            (
                "pass { vertex_shader = VS(v); } }",
                "code 0 pass (null) { vs [VS ( v ) ; <end>] ps [] }\nat }\n",
            ),
            (
                "pass P { pixel_shader = PS(v); } }",
                "code 0 pass P { vs [] ps [PS ( v ) ; <end>] }\nat }\n",
            ),
            ("pass { } }", "code 0 pass (null) { vs [] ps [] }\nat }\n"),
            // Not a pass: nothing read, nothing added.
            ("foo { } }", "code -3 pass (null) { vs [] ps [] }\nat foo\n"),
            ("} }", "code -3 pass (null) { vs [] ps [] }\nat }\n"),
            (
                "pass",
                "code -5 pass (null) { vs [] ps [] }\nerror 1:5 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "pass P",
                "code -5 pass P { vs [] ps [] }\nerror 1:7 0 Unexpected EOF\nat EOF\n",
            ),
            // The peek's error is at the token it was put back on.
            (
                "pass {",
                "code -5 pass (null) { vs [] ps [] }\nerror 1:6 0 Unexpected EOF\nat {\n",
            ),
            (
                "pass { vertex_shader = VS(v);",
                "code -5 pass (null) { vs [VS ( v ) ; <end>] ps [] }\nerror 1:29 0 Unexpected EOF\nat ;\n",
            ),
            (
                "pass { vertex_shader = VS(v) } }",
                "code 0 pass (null) { vs [VS ( v )] ps [] }\nerror 1:30 0 Expected ';'\nat }\n",
            ),
            // The command stops at the pass's '}', and the peek past it finds the
            // end of the text (characterized, not endorsed).
            (
                "pass { vertex_shader = VS(v) }",
                "code -5 pass (null) { vs [VS ( v )] ps [] }\nerror 1:30 0 Expected ';'\nerror 1:30 0 Unexpected EOF\nat }\n",
            ),
            (
                "pass { geometry_shader",
                "code -5 pass (null) { vs [] ps [] }\nerror 1:8 0 Syntax error\nerror 1:23 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "pass { geometry_shader = GS(v); } }",
                "code 0 pass (null) { vs [] ps [] }\nerror 1:8 0 Syntax error\nat }\n",
            ),
        ],
    );
}

/// `ep_parse_pass_command`'s codes, from the token before the command.
#[test]
fn pass_command_returns() {
    check(
        run_command,
        &[
            (
                "{ vertex_shader = VS(v); }",
                "code 0 pass (null) { vs [VS ( v ) ; <end>] ps [] }\nat ;\n",
            ),
            (
                "{ vertex_program = VS(v); }",
                "code 0 pass (null) { vs [VS ( v ) ; <end>] ps [] }\nat ;\n",
            ),
            (
                "{ pixel_shader = PS(v); }",
                "code 0 pass (null) { vs [] ps [PS ( v ) ; <end>] }\nat ;\n",
            ),
            (
                "{ pixel_program = compile PS(v); }",
                "code 0 pass (null) { vs [] ps [PS ( v ) ; <end>] }\nerror 1:19 1 compile keyword not necessary\nat ;\n",
            ),
            (
                "{ geometry_shader = GS(v); }",
                "code -1 pass (null) { vs [] ps [] }\nerror 1:3 0 Syntax error\nat ;\n",
            ),
            (
                "{ geometry_shader = GS(v) }",
                "code -1 pass (null) { vs [] ps [] }\nerror 1:3 0 Syntax error\nat }\n",
            ),
            (
                "{ vertex_shader VS(v); }",
                "code -1 pass (null) { vs [] ps [] }\nerror 1:22 0 Expected '='\nat ;\n",
            ),
            (
                "{ vertex_shader VS(v) }",
                "code -1 pass (null) { vs [] ps [] }\nerror 1:23 0 Expected '='\nat }\n",
            ),
            (
                "{ vertex_shader = VS(v) }",
                "code -1 pass (null) { vs [VS ( v )] ps [] }\nerror 1:25 0 Expected ';'\nat }\n",
            ),
            (
                "{ vertex_shader = ; }",
                "code 0 pass (null) { vs [; <end>] ps [] }\nat ;\n",
            ),
            (
                "{",
                "code -5 pass (null) { vs [] ps [] }\nerror 1:2 0 Unexpected EOF\nat EOF\n",
            ),
            // The '=' check's end of text is PARSE_CONTINUE, not PARSE_EOF
            // (characterized, not endorsed).
            (
                "{ vertex_shader",
                "code -1 pass (null) { vs [] ps [] }\nerror 1:16 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "{ vertex_shader =",
                "code -5 pass (null) { vs [] ps [] }\nerror 1:18 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "{ vertex_shader = compile",
                "code -5 pass (null) { vs [] ps [] }\nerror 1:19 1 compile keyword not necessary\nerror 1:26 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "{ vertex_shader = VS(v)",
                "code -5 pass (null) { vs [VS ( v )] ps [] }\nerror 1:24 0 Unexpected EOF\nat EOF\n",
            ),
            (
                "{ geometry_shader",
                "code -5 pass (null) { vs [] ps [] }\nerror 1:3 0 Syntax error\nerror 1:18 0 Unexpected EOF\nat EOF\n",
            ),
        ],
    );
}

/// A token soup over the techniques' vocabulary: every keyword, name,
/// number, string, delimiter and space they compare with or skip.
fn soup() -> impl Strategy<Value = String> {
    let word = prop::sample::select(vec![
        "pass",
        "vertex_shader",
        "pixel_program",
        "compile",
        "T",
        "VS",
        "1",
        "\"x\"",
        "=",
        ";",
        "{",
        "}",
        "(",
        ")",
        " ",
        "\n",
        "\t",
    ]);
    prop::collection::vec(word, 0..32).prop_map(|words| words.join(" "))
}

/// A program as the C builds it: a call's tokens, its `;` and the end
/// token, or a call cut at a `}` with no end token.
fn well_formed(t: &LexTokens, tokens: &[ProgramToken<usize>]) -> bool {
    tokens.iter().enumerate().all(|(i, token)| match token {
        ProgramToken::End => {
            i > 0 && tokens[i - 1] != ProgramToken::End && {
                let ProgramToken::Token(mark) = tokens[i - 1] else {
                    return false;
                };
                t.text_at(mark) == ";"
            }
        }
        ProgramToken::Token(mark) => t.text_at(*mark) != "}",
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]

    /// The technique never steps past the terminator (the cursor's
    /// assert), every program token is a `;`-ended call with its end token
    /// or a call cut at a `}`, and no program holds a `}`.
    #[test]
    fn walks_stay_in_the_tokens(text in soup()) {
        let mut t = LexTokens::new(&format!("technique {text}"));
        if let Some(ept) = parse_technique(&mut t) {
            for pass in &ept.passes {
                prop_assert!(well_formed(&t, &pass.vertex_program));
                prop_assert!(well_formed(&t, &pass.fragment_program));
            }
        }
    }
}
