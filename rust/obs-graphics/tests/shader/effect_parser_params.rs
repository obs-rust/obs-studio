//! Tier 1: the effect parser's parameter value assignments of
//! `libobs/graphics/effect-parser-params.c.h`, in
//! `obs_graphics::effect_parser`.
//!
//! `array_returns`, `assignment_returns` and `assign_returns` run
//! `ep_parse_param_array`, `ep_parse_param_assignment_val` and
//! `ep_parse_param_assign` from their `[` or `=` token on a param of a given
//! type, and pin the code or result each returns, the param's array count
//! and default bytes (written as `test/cmocka/test_effect_parser_params.c`
//! writes them), the errors added and the token each stops at. The tokens,
//! the cursor and the error format are the shader parser's Tier 1 ones
//! (`super::shader_parser`): obs-util's port of the C lexer, and
//! `cf_adderror`'s text at the token's place. The number token's text runs on
//! into the lexed text, `os_strtod` is obs-util's and `cf_literal_to_str` is
//! obs-util's with this target's `unsigned long`.

use core::ffi::c_ulong;

use obs_graphics::effect_parser::{
    INVALID_TYPE, Param, ParamTokens, parse_param_array, parse_param_assign,
    parse_param_assignment_val,
};
use obs_graphics::shader_parser::Tokens;
use obs_util::cf_parser::diagnostic;
use obs_util::cf_tokenizer::cf_literal_to_str;

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
