//! Tier 1: the effect parser's parameter declarations, annotations and
//! value assignments of `libobs/graphics/effect-parser-params.c.h`, in
//! `obs_graphics::effect_parser`.
//!
//! `scalars`, `vectors`, `arrays`, `eof`, `annotations`, `annotation_eof`
//! and `mixed` port `test/cmocka/test_effect_parser_params.c` one to one
//! (same texts, in the same groups, with its comments) at the declaration:
//! that test runs the whole `ep_parse()` behind a guard line, these step
//! over the specifiers, the type and the name as `ep_parse_other()` does and
//! run `ep_parse_param` from the token after the name, so each expectation
//! is the declaration's own part of the C one (the param it keeps, written
//! as the C test writes it, the errors it adds, one row up) and the token it
//! stops at, where `ep_parse()` goes on. `mixed` runs every declaration of
//! its text in turn and steps over the function as `ep_parse()` would leave
//! it. `edge_params` adds what the C test leaves out, and
//! `annotation_var_returns`, `annotations_returns`, `array_returns`,
//! `assignment_returns` and `assign_returns` pin the codes and results of the
//! inner functions. A proptest walks token soups after a declaration's name.
//! The tokens, the cursor and the
//! error format are the shader parser's Tier 1 ones
//! (`super::shader_parser`): obs-util's port of the C lexer, and
//! `cf_adderror`'s text at the token's place. The number token's text runs on
//! into the lexed text, `os_strtod` is obs-util's and `cf_literal_to_str` is
//! obs-util's with this target's `unsigned long`.

use core::ffi::c_ulong;

use obs_graphics::effect_parser::{
    INVALID_TYPE, Param, ParamTokens, parse_annotations, parse_param, parse_param_annotation_var,
    parse_param_array, parse_param_assign, parse_param_assignment_val,
};
use obs_graphics::shader_parser::{CFTOKEN_NEWLINE, CFTOKEN_NONE, CFTOKEN_SPACETAB, Tokens};
use obs_util::cf_parser::diagnostic;
use obs_util::cf_tokenizer::cf_literal_to_str;
use proptest::prelude::*;

use super::shader_parser::{LexTokens, check};

impl ParamTokens for LexTokens {
    fn c_str_byte(&self, i: usize) -> u8 {
        let t = &self.lexed.tokens[self.cur];
        self.lexed
            .reformatted
            .get(t.str_start + i)
            .copied()
            .unwrap_or(0)
    }

    fn strtod(&self, copy: &[u8]) -> f64 {
        assert_eq!(copy.last(), Some(&0), "the copy ends with its NUL");
        assert!(copy.len() <= 64, "os_strtod's buffer");
        obs_util::platform_number::strtod(copy)
    }

    fn literal_to_str(&self) -> Option<Vec<u8>> {
        let text = self.text();
        let ulong_bits = c_ulong::BITS;
        let mut out = cf_literal_to_str(text, text.len(), ulong_bits)?;
        // strlen(): the C copies up to the first NUL.
        if let Some(end) = out.iter().position(|&b| b == 0) {
            out.truncate(end);
        }
        Some(out)
    }

    fn add_invalid_type_error(&mut self, type_name: &[u8]) {
        let (row, column, text) = diagnostic(
            &self.input,
            self.lexed.tokens[self.cur].unmerged_start,
            INVALID_TYPE.to_bytes(),
            [Some(type_name), None, None],
        );
        self.errors.push(format!(
            "error {row}:{column} 0 {}",
            String::from_utf8_lossy(&text)
        ));
    }
}

fn s(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// `dump_param` of the C test.
fn param_text(param: &Param) -> String {
    let mut out = String::new();
    if param.is_const {
        out += " const";
    }
    if param.is_uniform {
        out += " uniform";
    }
    if param.is_property {
        out += " property";
    }
    out += &format!(" {} {}", s(&param.type_name), s(&param.name));
    if param.array_count != 0 {
        out += &format!(" [{}]", param.array_count);
    }
    if !param.default_val.is_empty() {
        let bytes: Vec<String> = param
            .default_val
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        out += &format!(" = {{{}}}", bytes.join(" "));
    }
    if !param.annotations.is_empty() {
        out += " <";
        for annotation in &param.annotations {
            out += &param_text(annotation);
            out += ";";
        }
        out += " >";
    }
    out
}

/// The errors, then where the cursor stopped.
fn tail(t: &LexTokens) -> String {
    let mut out = String::new();
    for error in &t.errors {
        out += error;
        out += "\n";
    }
    out + "at " + &t.at() + "\n"
}

fn run_array(text: &str) -> String {
    let mut t = LexTokens::new(text);
    assert_eq!(t.text(), b"[", "{text:?}");
    let mut param = Param::new(b"float".to_vec(), b"a".to_vec(), false, false, false);
    let ok = parse_param_array(&mut t, &mut param);
    format!("{ok}{}\n", param_text(&param)) + &tail(&t)
}

/// A value from the `=` token, for a param of `type_name`, with its code
/// and the param's default bytes.
fn run_value(type_name: &str, text: &str) -> String {
    let mut t = LexTokens::new(text);
    assert_eq!(t.text(), b"=", "{text:?}");
    let mut param = Param::new(
        type_name.as_bytes().to_vec(),
        b"v".to_vec(),
        false,
        false,
        true,
    );
    let code = parse_param_assignment_val(&mut t, &mut param);
    format!("code {code}{}\n", param_text(&param)) + &tail(&t)
}

fn run_assign(type_name: &str, text: &str) -> String {
    let mut t = LexTokens::new(text);
    assert_eq!(t.text(), b"=", "{text:?}");
    let mut param = Param::new(
        type_name.as_bytes().to_vec(),
        b"v".to_vec(),
        false,
        false,
        true,
    );
    let ok = parse_param_assign(&mut t, &mut param);
    format!("{ok}{}\n", param_text(&param)) + &tail(&t)
}

/// `run_value` on `"<type>|<text>"`.
fn run_value_case(case: &str) -> String {
    let (type_name, text) = case.split_once('|').unwrap();
    run_value(type_name, text)
}

/// `run_assign` on `"<type>|<text>"`.
fn run_assign_case(case: &str) -> String {
    let (type_name, text) = case.split_once('|').unwrap();
    run_assign(type_name, text)
}

const ARRAY_RETURNS: &[(&str, &str)] = &[
    ("[3];", "true float a [3]\nat ;\n"),
    ("[3] = 1;", "true float a [3]\nat =\n"),
    ("[0];", "true float a\nat ;\n"),
    ("[-1];", "false float a\nat -\n"),
    ("[x];", "false float a\nat x\n"),
    ("[1.5];", "false float a\nat 1.5\n"),
    ("[3;", "false float a [3]\nat EOF\n"),
    (
        "[3",
        "false float a [3]\nerror 1:3 0 Unexpected EOF\nat EOF\n",
    ),
    ("[", "false float a\nerror 1:2 0 Unexpected EOF\nat EOF\n"),
    (
        "[3]",
        "false float a [3]\nerror 1:4 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "[3 4];",
        "false float a [3]\nerror 1:6 0 Expected ']'\nerror 1:7 0 Unexpected EOF\nat EOF\n",
    ),
    ("[]", "false float a\nat ]\n"),
    ("[2147483648];", "true float a [2147483647]\nat ;\n"),
];

#[test]
fn array_returns() {
    check(run_array, ARRAY_RETURNS);
}

const ASSIGNMENT_RETURNS: &[(&str, &str)] = &[
    (
        "float|= 1.5;",
        "code 0 uniform float v = {00 00 c0 3f}\nat 1.5\n",
    ),
    (
        "float|= -1.5;",
        "code 0 uniform float v = {00 00 c0 bf}\nat 1.5\n",
    ),
    (
        "float|= - 1.5;",
        "code 0 uniform float v = {00 00 c0 bf}\nat 1.5\n",
    ),
    (
        "float|= x;",
        "code -1 uniform float v\nerror 1:3 0 Expected 'numeric value'\nat ;\n",
    ),
    ("float|= -", "code -5 uniform float v\nat EOF\n"),
    (
        "float|=",
        "code -5 uniform float v\nerror 1:2 0 Unexpected EOF\nat EOF\n",
    ),
    ("int|= 3;", "code 0 uniform int v = {03 00 00 00}\nat 3\n"),
    (
        "int|= --3;",
        "code -1 uniform int v\nerror 1:4 0 Expected 'numeric value'\nat ;\n",
    ),
    (
        "bool|= true;",
        "code 0 uniform bool v = {01 00 00 00}\nat true\n",
    ),
    (
        "bool|= 1;",
        "code -5 uniform bool v\nerror 1:3 0 Expected 'true or false'\nat 1\n",
    ),
    (
        "bool|=",
        "code -5 uniform bool v\nerror 1:2 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "string|= \"s\";",
        "code 0 uniform string v = {73 00}\nat \"s\"\n",
    ),
    (
        "string|= s;",
        "code -1 uniform string v\nerror 1:3 0 Expected 'string'\nat ;\n",
    ),
    (
        "string|=",
        "code -5 uniform string v\nerror 1:2 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "texture2d|= \"t\";",
        "code 0 uniform texture2d v = {74 00}\nat \"t\"\n",
    ),
    (
        "texture2d|= t;",
        "code -1 uniform texture2d v\nerror 1:3 0 Expected 'texture path string'\nat ;\n",
    ),
    (
        "float2|= {1, 2};",
        "code 0 uniform float2 v = {00 00 80 3f 00 00 00 40}\nat }\n",
    ),
    (
        "float2|= float2(1, 2);",
        "code 0 uniform float2 v = {00 00 80 3f 00 00 00 40}\nat )\n",
    ),
    (
        "float2|= float3(1, 2);",
        "code -1 uniform float2 v\nerror 1:15 0 Expected '{'\nat ;\n",
    ),
    (
        "float2|= 1;",
        "code -1 uniform float2 v\nerror 1:4 0 Expected '{'\nat ;\n",
    ),
    ("float2|= 1", "code -5 uniform float2 v\nat EOF\n"),
    (
        "float2|= {1, 2",
        "code -5 uniform float2 v = {00 00 80 3f 00 00 00 40}\nerror 1:8 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "float2|= {1 2};",
        "code -1 uniform float2 v = {00 00 80 3f}\nerror 1:8 0 Expected ','\nat ;\n",
    ),
    (
        "float2|= float2 1, 2);",
        "code -1 uniform float2 v\nerror 1:15 0 Expected '('\nat ;\n",
    ),
    (
        "float2|=",
        "code -5 uniform float2 v\nerror 1:2 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "float0|= {};",
        "code 0 uniform float0 v\nerror 1:1 0 Invalid row count\nat {\n",
    ),
    (
        "float5|= {1, 2, 3, 4, 5};",
        "code 0 uniform float5 v = {00 00 80 3f 00 00 00 40 00 00 40 40 00 00 80 40 00 00 a0 40}\nerror 1:1 0 Invalid row count\nat }\n",
    ),
    (
        "float4x5|= {1, 2};",
        "code -1 uniform float4x5 v = {00 00 80 3f 00 00 00 40}\nerror 1:1 0 Invalid column count\nerror 1:9 0 Expected ','\nat ;\n",
    ),
    (
        "int2x2|= int2x2(1, 2, 3, 4);",
        "code 0 uniform int2x2 v = {01 00 00 00 02 00 00 00 03 00 00 00 04 00 00 00}\nat )\n",
    ),
    (
        "foo|= 1;",
        "code -1 uniform foo v\nerror 1:1 0 Invalid type 'foo' used for assignment\nat =\n",
    ),
    (
        "|= 1;",
        "code -1 uniform  v\nerror 1:1 0 Invalid type '' used for assignment\nat =\n",
    ),
];

#[test]
fn assignment_returns() {
    check(run_value_case, ASSIGNMENT_RETURNS);
}

const ASSIGN_RETURNS: &[(&str, &str)] = &[
    (
        "float|= 1.5;",
        "true uniform float v = {00 00 c0 3f}\nat ;\n",
    ),
    (
        "float|= 1.5",
        "false uniform float v = {00 00 c0 3f}\nerror 1:6 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "float|= x;",
        "false uniform float v\nerror 1:3 0 Expected 'numeric value'\nat ;\n",
    ),
    (
        "foo|= 1;",
        "false uniform foo v\nerror 1:1 0 Invalid type 'foo' used for assignment\nat =\n",
    ),
    (
        "bool|= 1;",
        "false uniform bool v\nerror 1:3 0 Expected 'true or false'\nat 1\n",
    ),
    (
        "float2|= {1, 2};",
        "true uniform float2 v = {00 00 80 3f 00 00 00 40}\nat ;\n",
    ),
    (
        "float2|= {1, 2}",
        "false uniform float2 v = {00 00 80 3f 00 00 00 40}\nerror 1:9 0 Unexpected EOF\nat EOF\n",
    ),
];

#[test]
fn assign_returns() {
    check(run_assign_case, ASSIGN_RETURNS);
}

/// What `ep_parse_other()` does before `ep_parse_param`: the specifiers
/// (`ep_get_var_specifiers`, each keyword then the next valid token), the
/// type, the name and the next valid token. `None` when the text ends after
/// the name, where `ep_parse_other()` stops (its own "Unexpected EOF") and
/// never calls `ep_parse_param`. The texts here always have the rest.
fn declaration(t: &mut LexTokens) -> Option<(Vec<u8>, Vec<u8>, [bool; 3])> {
    let mut flags = [false; 3];
    loop {
        let flag = match t.text() {
            b"property" => 0,
            b"const" => 1,
            b"uniform" => 2,
            _ => break,
        };
        flags[flag] = true;
        step(t);
    }
    let type_name = t.text().to_vec();
    step(t);
    let name = t.text().to_vec();
    if !next(t) {
        return None;
    }
    Some((type_name, name, flags))
}

/// `cf_next_token`: whether a token follows.
fn next(t: &mut LexTokens) -> bool {
    t.advance();
    while t.kind() == CFTOKEN_SPACETAB || t.kind() == CFTOKEN_NEWLINE {
        t.advance();
    }
    t.kind() != CFTOKEN_NONE
}

/// [`next`] where the texts here always have a token.
fn step(t: &mut LexTokens) {
    assert!(next(t), "the declaration's prefix ran out");
}

fn run_param(t: &mut LexTokens) -> String {
    let Some((type_name, name, [is_property, is_const, is_uniform])) = declaration(t) else {
        return "not reached".into();
    };
    let kept = parse_param(t, type_name, name, is_property, is_const, is_uniform);
    kept.map_or_else(|| "dropped".into(), |p| format!("param{}", param_text(&p)))
}

/// One declaration from its first token.
fn run(text: &str) -> String {
    let mut t = LexTokens::new(text);
    let line = run_param(&mut t);
    line + "\n" + &tail(&t)
}

/// Every declaration of `text` in turn, as `ep_parse()` reaches them: `;`
/// and white space are stepped over, and a function (a `(` after the name)
/// is written as the C test writes it and stepped over to its `}`.
fn run_decls(text: &str) -> String {
    let mut t = LexTokens::new(text);
    let mut lines = String::new();
    let mut funcs = String::new();
    while t.kind() != CFTOKEN_NONE {
        if t.text() == b";" || t.kind() == CFTOKEN_SPACETAB || t.kind() == CFTOKEN_NEWLINE {
            t.advance();
            continue;
        }
        let start = t.mark();
        let Some((type_name, name, _)) = declaration(&mut t) else {
            break;
        };
        if t.text() == b"(" {
            funcs += &format!("func {} {}\n", s(&type_name), s(&name));
            while t.text() != b"{" {
                t.advance();
            }
            assert!(t.pass_pair());
            continue;
        }
        t.reset(start);
        lines += &run_param(&mut t);
        lines += "\n";
    }
    lines + &funcs + &tail(&t)
}

fn run_annotation_var(text: &str) -> String {
    let mut t = LexTokens::new(text);
    let mut var = Param::new(Vec::new(), Vec::new(), false, false, false);
    let code = parse_param_annotation_var(&mut t, &mut var);
    format!("code {code}{}\n", param_text(&var)) + &tail(&t)
}

fn run_annotations(text: &str) -> String {
    let mut t = LexTokens::new(text);
    let mut annotations = Vec::new();
    let ok = parse_annotations(&mut t, &mut annotations);
    let mut line = format!("{ok} <");
    for annotation in &annotations {
        line += &param_text(annotation);
        line += ";";
    }
    line + " >\n" + &tail(&t)
}

const SCALARS: &[(&str, &str)] = &[
    ("uniform float f;", "param uniform float f\nat ;\n"),
    (
        "const float f; property float g; float h;",
        "param const float f\nat ;\n",
    ),
    (
        "uniform int i = 3;",
        "param uniform int i = {03 00 00 00}\nat ;\n",
    ),
    (
        "uniform int i = -3;",
        "param uniform int i = {fd ff ff ff}\nat ;\n",
    ),
    (
        "uniform int i = - 3;",
        "param uniform int i = {fd ff ff ff}\nat ;\n",
    ),
    // An int reads the leading digits of the number token.
    (
        "uniform int i = 1.5;",
        "param uniform int i = {01 00 00 00}\nat ;\n",
    ),
    (
        "uniform float f = 1.5;",
        "param uniform float f = {00 00 c0 3f}\nat ;\n",
    ),
    (
        "uniform float f = -1.5;",
        "param uniform float f = {00 00 c0 bf}\nat ;\n",
    ),
    (
        "uniform float f = - 1.5;",
        "param uniform float f = {00 00 c0 bf}\nat ;\n",
    ),
    (
        "uniform float f = 2;",
        "param uniform float f = {00 00 00 40}\nat ;\n",
    ),
    // A float reads as far as os_strtod() goes.
    (
        "uniform float f = 1.5f;",
        "param uniform float f = {00 00 c0 3f}\nat ;\n",
    ),
    // A value that is not a number is reported at it, and the skip to the ';' drops
    // the param.
    (
        "uniform float f = x; uniform float g;",
        "dropped\nerror 1:19 0 Expected 'numeric value'\nat ;\n",
    ),
    // Only one '-' is taken.
    (
        "uniform float f = --1.0; uniform float g;",
        "dropped\nerror 1:20 0 Expected 'numeric value'\nat ;\n",
    ),
    (
        "uniform bool b = true;",
        "param uniform bool b = {01 00 00 00}\nat ;\n",
    ),
    (
        "uniform bool b = false;",
        "param uniform bool b = {00 00 00 00}\nat ;\n",
    ),
    // A bool other than true or false does not skip to the ';': the statement after it
    // starts at the bad value and reports it again (characterized, not endorsed).
    (
        "uniform bool b = 1; uniform float g;",
        "dropped\nerror 1:18 0 Expected 'true or false'\nat 1\n",
    ),
    (
        "uniform string s = \"hi\";",
        "param uniform string s = {68 69 00}\nat ;\n",
    ),
    (
        "uniform string s = \"a\\\"b\\n\";",
        "param uniform string s = {61 22 62 0a 00}\nat ;\n",
    ),
    (
        "uniform string s = \"\";",
        "param uniform string s = {00}\nat ;\n",
    ),
    (
        "uniform string s = hi; uniform float g;",
        "dropped\nerror 1:20 0 Expected 'string'\nat ;\n",
    ),
    (
        "uniform texture2d image;",
        "param uniform texture2d image\nat ;\n",
    ),
    (
        "uniform texture2d image = \"a.png\";",
        "param uniform texture2d image = {61 2e 70 6e 67 00}\nat ;\n",
    ),
    (
        "uniform texture_rect image = \"a.png\";",
        "param uniform texture_rect image = {61 2e 70 6e 67 00}\nat ;\n",
    ),
    (
        "uniform texture2d image = a; uniform float g;",
        "dropped\nerror 1:27 0 Expected 'texture path string'\nat ;\n",
    ),
    // An invalid type is reported at the '=', with no skip, so the statement after it
    // starts at the '=' (characterized, not endorsed).
    (
        "uniform foo v = 1; uniform float g;",
        "dropped\nerror 1:15 0 Invalid type 'foo' used for assignment\nat =\n",
    ),
    // No ';' after the value drops the param with no error, and the parser goes on
    // from the token after the value (characterized, not endorsed).
    (
        "uniform float f = 1.0 uniform float g;",
        "dropped\nat uniform\n",
    ),
    // Anything else after the name drops the param with no error of its own
    // (characterized, not endorsed).
    ("uniform float f 1.0; uniform float g;", "dropped\nat 1.0\n"),
];

#[test]
fn scalars() {
    check(run, SCALARS);
}

const VECTORS: &[(&str, &str)] = &[
    (
        "uniform float4 c = {1.0, 0.5, 0.25, 1.0};",
        "param uniform float4 c = {00 00 80 3f 00 00 00 3f 00 00 80 3e 00 00 80 3f}\nat ;\n",
    ),
    (
        "uniform float4 c = float4(1.0, 0.5, 0.25, 1.0);",
        "param uniform float4 c = {00 00 80 3f 00 00 00 3f 00 00 80 3e 00 00 80 3f}\nat ;\n",
    ),
    (
        "uniform float2 c = {-1.0, - 2.0};",
        "param uniform float2 c = {00 00 80 bf 00 00 00 c0}\nat ;\n",
    ),
    (
        "uniform int2 v = {1, -2};",
        "param uniform int2 v = {01 00 00 00 fe ff ff ff}\nat ;\n",
    ),
    (
        "uniform int3 v = int3(1, 2, 3);",
        "param uniform int3 v = {01 00 00 00 02 00 00 00 03 00 00 00}\nat ;\n",
    ),
    (
        "uniform float3x3 m = {1, 2, 3, 4, 5, 6, 7, 8, 9};",
        "param uniform float3x3 m = {00 00 80 3f 00 00 00 40 00 00 40 40 00 00 80 40 00 00 a0 40 00 00 c0 40 00 00 e0 40 00 00 00 41 00 00 10 41}\nat ;\n",
    ),
    (
        "uniform float4x4 m = {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1};",
        "param uniform float4x4 m = {00 00 80 3f 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 80 3f 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 80 3f 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 80 3f}\nat ;\n",
    ),
    (
        "uniform int2x2 m = {1, 2, 3, 4};",
        "param uniform int2x2 m = {01 00 00 00 02 00 00 00 03 00 00 00 04 00 00 00}\nat ;\n",
    ),
    (
        "uniform float1 f = {1.0};",
        "param uniform float1 f = {00 00 80 3f}\nat ;\n",
    ),
    // A bad row count is reported at the '=', and the values are read anyway, by the
    // row digit (characterized, not endorsed).
    (
        "uniform float5 v = {1, 2, 3, 4, 5};",
        "param uniform float5 v = {00 00 80 3f 00 00 00 40 00 00 40 40 00 00 80 40 00 00 a0 40}\nerror 1:18 0 Invalid row count\nat ;\n",
    ),
    // No values are read for a row count of 0, so the '}' is taken where the ';'
    // should be (characterized, not endorsed).
    (
        "uniform float0 v = {}; uniform float g;",
        "dropped\nerror 1:18 0 Invalid row count\nat }\n",
    ),
    // A missing ',' or end is reported at the ';' the skip goes to.
    (
        "uniform float4x5 m = {1, 2}; uniform float g;",
        "dropped\nerror 1:20 0 Invalid column count\nerror 1:28 0 Expected ','\nat ;\n",
    ),
    (
        "uniform float4 c = {1.0, 0.5}; uniform float g;",
        "dropped\nerror 1:30 0 Expected ','\nat ;\n",
    ),
    (
        "uniform float2 c = {1.0 0.5}; uniform float g;",
        "dropped\nerror 1:29 0 Expected ','\nat ;\n",
    ),
    (
        "uniform float2 c = {1.0, 0.5, 0.25}; uniform float g;",
        "dropped\nerror 1:36 0 Expected '}'\nat ;\n",
    ),
    (
        "uniform float2 c = {1.0, x}; uniform float g;",
        "dropped\nerror 1:26 0 Expected 'numeric value'\nat ;\n",
    ),
    // A vector without '{' or its own type skips to the ';', and reports the '{'
    // there.
    (
        "uniform float2 c = 1.0; uniform float g;",
        "dropped\nerror 1:23 0 Expected '{'\nat ;\n",
    ),
    (
        "uniform float2 c = float3(1.0, 2.0); uniform float g;",
        "dropped\nerror 1:36 0 Expected '{'\nat ;\n",
    ),
    (
        "uniform float2 c = float2 1.0, 2.0); uniform float g;",
        "dropped\nerror 1:36 0 Expected '('\nat ;\n",
    ),
    (
        "uniform float2 c = float2(1.0, 2.0}; uniform float g;",
        "dropped\nerror 1:36 0 Expected ')'\nat ;\n",
    ),
    (
        "uniform float2 c = {1.0, 2.0); uniform float g;",
        "dropped\nerror 1:30 0 Expected '}'\nat ;\n",
    ),
];

#[test]
fn vectors() {
    check(run, VECTORS);
}

const ARRAYS: &[(&str, &str)] = &[
    ("uniform float a[3];", "param uniform float a [3]\nat ;\n"),
    (
        "uniform float a[3] = 1.0;",
        "param uniform float a [3] = {00 00 80 3f}\nat ;\n",
    ),
    // A count of 0 is no array.
    ("uniform float a[0];", "param uniform float a\nat ;\n"),
    // A count that is not a number token drops the param with no error of its own; the
    // statement after it starts at the bad token (characterized, not endorsed).
    ("uniform float a[-1]; uniform float g;", "dropped\nat -\n"),
    ("uniform float a[x]; uniform float g;", "dropped\nat x\n"),
    (
        "uniform float a[1.5]; uniform float g;",
        "dropped\nat 1.5\n",
    ),
    // A missing ']' skips from the ';' it found to the next one, so the statement
    // after it goes too (characterized, not endorsed).
    (
        "uniform float a[3; uniform float g;",
        "dropped\nerror 1:35 0 Expected ']'\nerror 1:36 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "uniform float a[3] uniform float g;",
        "dropped\nat uniform\n",
    ),
    ("uniform float a[]; uniform float g;", "dropped\nat ]\n"),
    (
        "uniform float4 a[2] = {1, 2, 3, 4};",
        "param uniform float4 a [2] = {00 00 80 3f 00 00 00 40 00 00 40 40 00 00 80 40}\nat ;\n",
    ),
];

#[test]
fn arrays() {
    check(run, ARRAYS);
}

const EOF: &[(&str, &str)] = &[
    ("uniform float f", "not reached\nat EOF\n"),
    (
        "uniform float f =",
        "dropped\nerror 1:18 0 Unexpected EOF\nat EOF\n",
    ),
    // End of text after a '-' adds no error (characterized, not endorsed).
    ("uniform float f = -", "dropped\nat EOF\n"),
    (
        "uniform float f = 1.0",
        "dropped\nerror 1:22 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "uniform bool b =",
        "dropped\nerror 1:17 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "uniform string s =",
        "dropped\nerror 1:19 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "uniform texture2d image =",
        "dropped\nerror 1:26 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "uniform float2 c =",
        "dropped\nerror 1:19 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "uniform float2 c = {",
        "dropped\nerror 1:21 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "uniform float2 c = {1.0",
        "dropped\nerror 1:24 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "uniform float2 c = {1.0,",
        "dropped\nerror 1:25 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "uniform float2 c = {1.0, 2.0",
        "dropped\nerror 1:29 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "uniform float2 c = {1.0, 2.0}",
        "dropped\nerror 1:30 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "uniform float2 c = float2",
        "dropped\nerror 1:26 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "uniform float2 c = float2(",
        "dropped\nerror 1:27 0 Unexpected EOF\nat EOF\n",
    ),
    // End of text in the skip after a vector without '{' adds no error (characterized,
    // not endorsed).
    ("uniform float2 c = 1.0", "dropped\nat EOF\n"),
    (
        "uniform float a[",
        "dropped\nerror 1:17 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "uniform float a[3",
        "dropped\nerror 1:18 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "uniform float a[3]",
        "dropped\nerror 1:19 0 Unexpected EOF\nat EOF\n",
    ),
];

#[test]
fn eof() {
    check(run, EOF);
}

const ANNOTATIONS: &[(&str, &str)] = &[
    (
        "uniform float f<string name = \"x\"; float v = 1.0;> = 2.0;",
        "param uniform float f = {00 00 00 40} < string name = {78 00}; float v = {00 00 80 3f}; >\nat ;\n",
    ),
    (
        "uniform float4 c<int a = 1; bool b = false; float2 p = {1, 2};>;",
        "param uniform float4 c < int a = {01 00 00 00}; bool b = {00 00 00 00}; float2 p = {00 00 80 3f 00 00 00 40}; >\nat ;\n",
    ),
    ("uniform float f< >;", "param uniform float f\nat ;\n"),
    ("uniform float f<>;", "param uniform float f\nat ;\n"),
    (
        "uniform float f<;; float v;>;",
        "param uniform float f < float v; >\nat ;\n",
    ),
    (
        "uniform float f<int a = 1; int a = 2;>;",
        "param uniform float f < int a = {01 00 00 00}; int a = {02 00 00 00}; >\nat ;\n",
    ),
    (
        "uniform float f[2]<int a = 1;> = 1.0;",
        "param uniform float f [2] = {00 00 80 3f} < int a = {01 00 00 00}; >\nat ;\n",
    ),
    (
        "uniform texture2d image<string name = \"Image\";> = \"a.png\";",
        "param uniform texture2d image = {61 2e 70 6e 67 00} < string name = {49 6d 61 67 65 00}; >\nat ;\n",
    ),
    // An entry with no name is dropped, and the list goes on.
    (
        "uniform float f<string = \"x\";>; uniform float g;",
        "param uniform float f\nerror 1:24 0 Expected 'variable name'\nat ;\n",
    ),
    (
        "uniform float f<string>; uniform float g;",
        "param uniform float f\nerror 1:23 0 Expected 'variable name'\nerror 1:23 0 Syntax error\nat ;\n",
    ),
    // A ':' mapping ends the list where it is, so the '>' check fails and the param is
    // dropped; the parser goes on from the ':', inside the list (characterized, not
    // endorsed).
    (
        "uniform float f<string name : POS;>; uniform float g;",
        "dropped\nerror 1:29 0 Expected '= or ;'\nerror 1:29 0 Syntax error\nerror 1:29 0 Expected '>'\nat :\n",
    ),
    // An entry that ends at '>' is dropped with a syntax error, and the list ends
    // there.
    (
        "uniform float f<string name>; uniform float g;",
        "param uniform float f\nerror 1:28 0 Expected '= or ;'\nerror 1:28 0 Syntax error\nat ;\n",
    ),
    // A failed value is reported once by the value parser and again by the entry, then
    // the list ends at the ';' it skipped to, which is not '>': the param is dropped
    // (characterized, not endorsed).
    (
        "uniform float f<float v = x;>; uniform float g;",
        "dropped\nerror 1:27 0 Expected 'numeric value'\nerror 1:28 0 Expected 'assignment value'\nerror 1:28 0 Syntax error\nerror 1:28 0 Expected '>'\nat ;\n",
    ),
    // No ';' after an entry's value drops it with no error, and the skip to the next
    // ';' takes the next entry too (characterized, not endorsed).
    (
        "uniform float f<float v = 1.0 int w = 2;>; uniform float g;",
        "param uniform float f\nat ;\n",
    ),
    (
        "uniform float f<float v 1.0; int w = 2;>; uniform float g;",
        "param uniform float f < int w = {02 00 00 00}; >\nat ;\n",
    ),
    (
        "uniform float f<1 v; int w = 2;>; uniform float g;",
        "param uniform float f < int w = {02 00 00 00}; >\nerror 1:17 0 Expected 'type name'\nat ;\n",
    ),
    (
        "uniform float f<float 1; int w = 2;>; uniform float g;",
        "param uniform float f < int w = {02 00 00 00}; >\nerror 1:23 0 Expected 'variable name'\nat ;\n",
    ),
    // An annotation has no annotations: the skip stops at the inner ';', the inner '>'
    // ends the list, and the outer list's last entry is read as a param of its own
    // (characterized, not endorsed).
    (
        "uniform float f<float v<int a = 1;>; int w = 2;>; uniform float g;",
        "param uniform float f\nat ;\n",
    ),
    // An invalid type is reported at the '=', with no skip, so every error lands there
    // and the parser goes on from it (characterized, not endorsed).
    (
        "uniform float f<foo v = 1; int w = 2;>; uniform float g;",
        "dropped\nerror 1:23 0 Invalid type 'foo' used for assignment\nerror 1:23 0 Expected 'assignment value'\nerror 1:23 0 Syntax error\nerror 1:23 0 Expected '>'\nat =\n",
    ),
    // No ';' after the list drops the param with no error.
    (
        "uniform float f<int w = 2;> uniform float g;",
        "dropped\nat uniform\n",
    ),
    // Annotations come before the value: after it, they are the token where the ';'
    // should be.
    (
        "uniform float f = 1.0<int a = 1;>; uniform float g;",
        "dropped\nat <\n",
    ),
    (
        "uniform float f<int a = 1;><int b = 2;>; uniform float g;",
        "dropped\nat <\n",
    ),
];

#[test]
fn annotations() {
    check(run, ANNOTATIONS);
}

const ANNOTATION_EOF: &[(&str, &str)] = &[
    (
        "uniform float f<",
        "dropped\nerror 1:17 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "uniform float f<float",
        "dropped\nerror 1:22 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "uniform float f<float v",
        "dropped\nerror 1:24 0 Unexpected EOF\nat EOF\n",
    ),
    // End of text in an entry's value is reported four times (characterized, not
    // endorsed).
    (
        "uniform float f<float v =",
        "dropped\nerror 1:26 0 Unexpected EOF\nerror 1:26 0 Expected 'assignment value'\nerror 1:26 0 Syntax error\nerror 1:26 0 Expected '>'\nat EOF\n",
    ),
    (
        "uniform float f<float v = 1.0",
        "dropped\nerror 1:30 0 Unexpected EOF\nerror 1:30 0 Expected 'assignment value'\nerror 1:30 0 Syntax error\nerror 1:30 0 Expected '>'\nat EOF\n",
    ),
    (
        "uniform float f<float v = 1.0;",
        "dropped\nerror 1:31 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "uniform float f<float v = 1.0;>",
        "dropped\nerror 1:32 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "uniform float f<float v x",
        "dropped\nerror 1:26 0 Unexpected EOF\nerror 1:26 0 Expected '; or >'\nat EOF\n",
    ),
];

#[test]
fn annotation_eof() {
    check(run, ANNOTATION_EOF);
}

const MIXED: &[(&str, &str)] = &[(
    "uniform float4x4 ViewProj;\nuniform texture2d image;\nuniform float4 color = {1.0, 1.0, 1.0, 1.0};\nfloat4 PSDraw(float4 v) { return v * color; }\nuniform float alpha = 0.5;",
    "param uniform float4x4 ViewProj\nparam uniform texture2d image\nparam uniform float4 color = {00 00 80 3f 00 00 80 3f 00 00 80 3f 00 00 80 3f}\nparam uniform float alpha = {00 00 00 3f}\nfunc float4 PSDraw\nat EOF\n",
)];

#[test]
fn mixed() {
    check(run_decls, MIXED);
}

const EDGE_PARAMS: &[(&str, &str)] = &[
    // Every specifier, in any order.
    (
        "const uniform property float f;",
        "param const uniform property float f\nat ;\n",
    ),
    (
        "property const float f = 1.0;",
        "param const property float f = {00 00 80 3f}\nat ;\n",
    ),
    // No specifier.
    ("float f = 1.0;", "param float f = {00 00 80 3f}\nat ;\n"),
    // The int rule saturates past INT_MAX (#3464).
    ("int i = 2147483648;", "param int i = {ff ff ff 7f}\nat ;\n"),
    (
        "int i = -2147483649;",
        "param int i = {00 00 00 80}\nat ;\n",
    ),
    ("int i = 0x10;", "param int i = {00 00 00 00}\nat ;\n"),
    // A float past FLT_MAX is an infinity.
    ("float f = 1e39;", "param float f = {00 00 80 7f}\nat ;\n"),
    ("float f = .5;", "param float f = {00 00 00 3f}\nat ;\n"),
    // os_strtod reads at most 63 bytes.
    (
        "float f = 1.000000000000000000000000000000000000000000000000000000000000002;",
        "param float f = {00 00 80 3f}\nat ;\n",
    ),
    // A string ends at its first NUL (strlen), escapes and all.
    ("string s = \"a\\0b\";", "param string s = {61 00}\nat ;\n"),
    ("string s = 'x';", "param string s = {78 00}\nat ;\n"),
    (
        "string s = \"tab\\there\";",
        "param string s = {74 61 62 09 68 65 72 65 00}\nat ;\n",
    ),
    // Any type starting with string takes the string parser.
    ("strings s = \"x\";", "param strings s = {78 00}\nat ;\n"),
    // Any type starting with texture is a texture.
    (
        "texture3d t = \"v.dds\";",
        "param texture3d t = {76 2e 64 64 73 00}\nat ;\n",
    ),
    (
        "textures t = \"v.dds\";",
        "param textures t = {76 2e 64 64 73 00}\nat ;\n",
    ),
    // bool2 has no parser.
    (
        "bool2 b = true;",
        "dropped\nerror 1:9 0 Invalid type 'bool2' used for assignment\nat =\n",
    ),
    // int1 and float1 take the array parser.
    ("int1 v = {3};", "param int1 v = {03 00 00 00}\nat ;\n"),
    ("int1 v = 3;", "dropped\nerror 1:11 0 Expected '{'\nat ;\n"),
    (
        "float4x4 m = float4x4(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1);",
        "param float4x4 m = {00 00 80 3f 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 80 3f 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 80 3f 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 80 3f}\nat ;\n",
    ),
    (
        "float3 c = float3(-1, - 2, 3);",
        "param float3 c = {00 00 80 bf 00 00 00 c0 00 00 40 40}\nat ;\n",
    ),
    (
        "float2 c = {\n\t1.0,\n\t2.0\n};",
        "param float2 c = {00 00 80 3f 00 00 00 40}\nat ;\n",
    ),
    (
        "float2 c = float2 (1.0, 2.0);",
        "param float2 c = {00 00 80 3f 00 00 00 40}\nat ;\n",
    ),
    // A missing value after a '-' in a vector.
    (
        "float2 c = {-, 1.0};",
        "dropped\nerror 1:14 0 Expected 'numeric value'\nat ;\n",
    ),
    (
        "float2 c = {1.0, -};",
        "dropped\nerror 1:19 0 Expected 'numeric value'\nat ;\n",
    ),
    // Two counts: the second '[' is where the ';' should be.
    ("float a[2][3];", "dropped\nat [\n"),
    (
        "float a[3] = {1.0};",
        "dropped\nerror 1:14 0 Expected 'numeric value'\nat ;\n",
    ),
    // An array count that is a float or hex number reads its leading digits.
    ("float a[0x10];", "dropped\nat 0x10\n"),
    ("float a[007];", "param float a [7]\nat ;\n"),
    ("float a[2147483648];", "param float a [2147483647]\nat ;\n"),
    // Annotations on an array, and a value after both.
    (
        "float4 a[2]<string name = \"n\";> = {1, 2, 3, 4};",
        "param float4 a [2] = {00 00 80 3f 00 00 00 40 00 00 40 40 00 00 80 40} < string name = {6e 00}; >\nat ;\n",
    ),
    // An annotation's texture type has no texture flag (set from the empty type), so
    // its value has no parser.
    (
        "float f<texture2d t = \"x\";>;",
        "dropped\nerror 1:21 0 Invalid type 'texture2d' used for assignment\nerror 1:21 0 Expected 'assignment value'\nerror 1:21 0 Syntax error\nerror 1:21 0 Expected '>'\nat =\n",
    ),
    // An annotation's string, vector and matrix values.
    (
        "float f<string s = \"a\"; float4 c = float4(1, 2, 3, 4); int2x2 m = {1, 2, 3, 4};>;",
        "param float f < string s = {61 00}; float4 c = {00 00 80 3f 00 00 00 40 00 00 40 40 00 00 80 40}; int2x2 m = {01 00 00 00 02 00 00 00 03 00 00 00 04 00 00 00}; >\nat ;\n",
    ),
    // Annotation entries keep no specifiers.
    ("float f<uniform float v;>;", "param float f\nat ;\n"),
    (
        "float f<float v = 1.0;;;>;",
        "param float f < float v = {00 00 80 3f}; >\nat ;\n",
    ),
    // Nothing after the name.
    ("float f", "not reached\nat EOF\n"),
    ("float f<", "dropped\nerror 1:9 0 Unexpected EOF\nat EOF\n"),
    ("float f[", "dropped\nerror 1:9 0 Unexpected EOF\nat EOF\n"),
    // A ';' straight after the name.
    (
        "uniform float4x4 ViewProj;",
        "param uniform float4x4 ViewProj\nat ;\n",
    ),
];

#[test]
fn edge_params() {
    check(run, EDGE_PARAMS);
}

const ANNOTATION_VAR_RETURNS: &[(&str, &str)] = &[
    ("< ;", "code -1  \nat ;\n"),
    ("< >", "code -2  \nat >\n"),
    ("< float v;", "code 0 float v\nat ;\n"),
    ("< float v = 1.0;", "code 0 float v = {00 00 80 3f}\nat ;\n"),
    (
        "< float ;",
        "code -3 float \nerror 1:9 0 Expected 'variable name'\nat ;\n",
    ),
    (
        "< float >",
        "code -4 float \nerror 1:9 0 Expected 'variable name'\nat >\n",
    ),
    (
        "< float v : X;",
        "code -4 float v\nerror 1:11 0 Expected '= or ;'\nat :\n",
    ),
    (
        "< float v >",
        "code -4 float v\nerror 1:11 0 Expected '= or ;'\nat >\n",
    ),
    ("<", "code -5  \nerror 1:2 0 Unexpected EOF\nat EOF\n"),
    (
        "< float",
        "code -5 float \nerror 1:8 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "< float v",
        "code -5 float v\nerror 1:10 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "< 1 v;",
        "code -1  \nerror 1:3 0 Expected 'type name'\nat ;\n",
    ),
    (
        "< float 1;",
        "code -1 float \nerror 1:9 0 Expected 'variable name'\nat ;\n",
    ),
    ("< float v x;", "code -1 float v\nat ;\n"),
    ("< float v x >", "code -1 float v\nat >\n"),
    (
        "< float v x",
        "code -5 float v\nerror 1:12 0 Unexpected EOF\nerror 1:12 0 Expected '; or >'\nat EOF\n",
    ),
    (
        "< float v = x;",
        "code -4 float v\nerror 1:13 0 Expected 'numeric value'\nerror 1:14 0 Expected 'assignment value'\nat ;\n",
    ),
    (
        "< float v = 1.0 x;",
        "code -1 float v = {00 00 80 3f}\nat ;\n",
    ),
    (
        "< foo v = 1;",
        "code -4 foo v\nerror 1:9 0 Invalid type 'foo' used for assignment\nerror 1:9 0 Expected 'assignment value'\nat =\n",
    ),
];

#[test]
fn annotation_var_returns() {
    check(run_annotation_var, ANNOTATION_VAR_RETURNS);
}

const ANNOTATIONS_RETURNS: &[(&str, &str)] = &[
    // Not at '<'.
    ("= 1.0;", "false < >\nerror 1:1 0 Expected '<'\nat =\n"),
    ("<>;", "true < >\nat ;\n"),
    (
        "< float v = 1.0; int w = 2; >;",
        "true < float v = {00 00 80 3f}; int w = {02 00 00 00}; >\nat ;\n",
    ),
    ("< float v; > x", "true < float v; >\nat x\n"),
    (
        "<float v;>",
        "false < float v; >\nerror 1:11 0 Unexpected EOF\nat EOF\n",
    ),
    (
        "< float v : X; >",
        "false < >\nerror 1:11 0 Expected '= or ;'\nerror 1:11 0 Syntax error\nerror 1:11 0 Expected '>'\nat :\n",
    ),
    (
        "< float v = x; >",
        "false < >\nerror 1:13 0 Expected 'numeric value'\nerror 1:14 0 Expected 'assignment value'\nerror 1:14 0 Syntax error\nerror 1:14 0 Expected '>'\nat ;\n",
    ),
];

#[test]
fn annotations_returns() {
    check(run_annotations, ANNOTATIONS_RETURNS);
}

const SOUP: &[&str] = &[
    "uniform",
    "float",
    "float2",
    "float4x4",
    "int",
    "int2",
    "bool",
    "string",
    "texture2d",
    "foo",
    "f",
    "v",
    "=",
    "{",
    "}",
    "(",
    ")",
    "[",
    "]",
    "<",
    ">",
    ";",
    ",",
    ":",
    "-",
    "1",
    "2.5",
    "0",
    "true",
    "false",
    "\"s\"",
    "x",
    "\n",
];

proptest! {
    /// Any token soup after a declaration's name: the cursor never steps
    /// past the terminator (`LexTokens` asserts it), a kept param stops at
    /// its `;`, its value is a whole number of `int`s or `float`s or a
    /// string with its NUL, and every annotation has a type and a name.
    #[test]
    fn soups_keep_whole_params(
        type_name in proptest::sample::select(&SOUP[1..10]),
        tokens in proptest::collection::vec(proptest::sample::select(SOUP), 0..24),
    ) {
        let text = format!("{type_name} p {}", tokens.join(" "));
        let mut t = LexTokens::new(&text);
        let Some((type_name, name, [is_property, is_const, is_uniform])) = declaration(&mut t) else {
            return Ok(());
        };
        let kept = parse_param(&mut t, type_name, name, is_property, is_const, is_uniform);
        if let Some(param) = kept {
            prop_assert_eq!(t.text(), b";");
            check_value(&param)?;
            for annotation in &param.annotations {
                prop_assert!(!annotation.type_name.is_empty() && !annotation.name.is_empty());
                prop_assert!(annotation.annotations.is_empty());
                prop_assert!(!annotation.is_texture);
                check_value(annotation)?;
            }
        }
    }
}

fn check_value(param: &Param) -> Result<(), TestCaseError> {
    let text = param.is_texture || param.type_name.starts_with(b"string");
    if text {
        prop_assert!(param.default_val.is_empty() || param.default_val.ends_with(&[0]));
    } else {
        prop_assert_eq!(param.default_val.len() % 4, 0);
    }
    Ok(())
}
