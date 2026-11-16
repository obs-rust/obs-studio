//! Tier 1: the effect parser's entry point of
//! `libobs/graphics/effect-parser-entry.c.h`, in
//! `obs_graphics::effect_parser::entry`.
//!
//! `empty_and_compile_result`, `backend_preprocessor_definition`,
//! `dispatch_all_top_level_kinds`,
//! `parse_errors_gate_compile_but_warnings_do_not` and
//! `unexpected_braces_recover_to_next_declaration` port
//! `test/cmocka/test_effect_parser_entry.c` one to one: the same texts,
//! backend names and compile results, and the same expectations (the
//! result, the compile calls, how many params, structs, samplers,
//! functions and techniques were kept, the errors, the first param's name
//! and the first error). That test runs the real C lexer, preprocessor and
//! child parsers; [`Fake`] runs the core over obs-util's port of the lexer
//! (`cf_lex`) and its `cf_pass_pair`, with stand-ins for the rest: a
//! preprocessor that knows only the `#ifdef`, `#else`, `#endif` and
//! `#error` lines these texts use, `ep_check_for_keyword` as the C writes
//! it, and block, parameter and function parsers that keep their item and
//! stop where the C ones stop for these texts. Every model call is traced,
//! so the `edge_*` tests also pin the order of the calls.

use core::ffi::CStr;

use obs_graphics::effect_parser::entry::{
    EntryModel, Specifiers, get_var_specifiers, parse, parse_other,
};
use obs_graphics::shader_parser::{BlockError, Tokens};
use obs_util::cf_parser::{Cursor, pass_pair};
use obs_util::cf_tokenizer::{CfLexed, CfTokenType, cf_lex};

const PARSE_SUCCESS: i32 = 0;
const PARSE_CONTINUE: i32 = -1;
const PARSE_EOF: i32 = -5;
const LEX_ERROR: i32 = 0;
const LEX_WARNING: i32 = 1;

/// The stand-in parser: the text's tokens, what the child parsers kept,
/// the errors (message, level) and every call.
struct Fake {
    text: &'static str,
    backend: Option<&'static str>,
    compile_result: bool,
    /// Make `has_token` answer `false` (a `NULL` `cur_token`).
    no_token: bool,
    defines: Vec<String>,
    lexed: Option<CfLexed>,
    cur: usize,
    effect_set: bool,
    params: Vec<(String, Specifiers)>,
    structs: usize,
    samplers: usize,
    funcs: Vec<String>,
    techniques: usize,
    compile_calls: usize,
    errors: Vec<(String, i32)>,
    trace: Vec<String>,
}

impl Fake {
    fn new(text: &'static str, backend: Option<&'static str>, compile_result: bool) -> Self {
        Self {
            text,
            backend,
            compile_result,
            no_token: false,
            defines: Vec::new(),
            lexed: None,
            cur: 0,
            effect_set: false,
            params: Vec::new(),
            structs: 0,
            samplers: 0,
            funcs: Vec::new(),
            techniques: 0,
            compile_calls: 0,
            errors: Vec::new(),
            trace: Vec::new(),
        }
    }

    fn lexed(&self) -> &CfLexed {
        self.lexed
            .as_ref()
            .expect("tokens read before the text parsed")
    }

    fn at(&self) -> String {
        String::from_utf8_lossy(self.text()).into_owned()
    }

    fn error(&mut self, message: &str, level: i32) {
        self.errors.push((message.to_owned(), level));
    }

    /// Steps until the token is `s` (or the end).
    fn skip_to(&mut self, s: &str) {
        while self.kind() != CfTokenType::None as i32 && self.at() != s {
            self.advance();
        }
    }

    /// Steps to the next `{` and past its `}`.
    fn skip_block(&mut self) {
        self.skip_to("{");
        self.pass_pair();
    }

    /// `cf_next_token`'s skip of whitespace and newlines.
    fn skip_space(&mut self) {
        while self.kind() == CfTokenType::SpaceTab as i32
            || self.kind() == CfTokenType::Newline as i32
        {
            self.advance();
        }
    }
}

struct PairCursor<'a>(&'a mut Fake);

impl Cursor for PairCursor<'_> {
    fn token(&self) -> (i32, u8) {
        (self.0.kind(), self.0.text().first().copied().unwrap_or(0))
    }
    fn advance(&mut self) {
        self.0.cur += 1;
    }
}

impl Tokens for Fake {
    type Mark = usize;

    fn kind(&self) -> i32 {
        self.lexed().tokens[self.cur].kind as i32
    }

    fn text(&self) -> &[u8] {
        let lexed = self.lexed();
        let t = &lexed.tokens[self.cur];
        &lexed.reformatted[t.str_start..t.str_start + t.str_len]
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
        let mut text = message.to_string_lossy().into_owned();
        if let Some(val1) = val1 {
            text = text.replace("$1", &val1.to_string_lossy());
        }
        self.trace.push(format!("error {text}"));
        self.error(&text, error.level());
    }

    fn pass_pair(&mut self) -> bool {
        self.trace.push(format!("pass_pair {}", self.at()));
        pass_pair(&mut PairCursor(self), b'{', b'}')
    }
}

impl EntryModel for Fake {
    type Name = &'static str;

    fn preprocessor_name(&mut self) -> Option<&'static str> {
        self.trace.push("preprocessor_name".into());
        self.backend
    }

    fn add_def(&mut self, name: &'static str) {
        self.trace.push(format!("add_def {name}"));
        self.defines.push(name.into());
    }

    fn set_effect(&mut self) {
        self.trace.push("set_effect".into());
        self.effect_set = true;
    }

    /// `cf_lexer_lex` fails on an empty text; the preprocessor drops the
    /// lines `#ifdef`, `#else` and `#endif` leave out and adds an error for
    /// `#error`.
    fn parse_text(&mut self) -> bool {
        self.trace.push("parse_text".into());
        if self.text.is_empty() {
            return false;
        }
        let mut kept = String::new();
        let mut skipping: Vec<bool> = Vec::new();
        for line in self.text.split_inclusive('\n') {
            let directive = line.trim_end();
            if let Some(name) = directive.strip_prefix("#ifdef ") {
                skipping.push(!self.defines.iter().any(|d| d == name.trim()));
            } else if directive == "#else" {
                let top = skipping.last_mut().expect("#else without #ifdef");
                *top = !*top;
            } else if directive == "#endif" {
                skipping.pop();
            } else if directive.starts_with("#error") {
                if !skipping.iter().any(|s| *s) {
                    self.error(directive, LEX_ERROR);
                }
            } else if !skipping.iter().any(|s| *s) {
                kept += line;
                continue;
            }
            kept += "\n";
        }
        self.lexed = Some(cf_lex(kept.as_bytes()));
        self.cur = 0;
        true
    }

    fn has_token(&self) -> bool {
        !self.no_token
    }

    fn parse_struct(&mut self) {
        self.trace.push("parse_struct".into());
        self.skip_block();
        self.skip_to(";");
        self.structs += 1;
    }

    fn parse_technique(&mut self) {
        self.trace.push("parse_technique".into());
        self.skip_block();
        self.techniques += 1;
    }

    fn parse_sampler_state(&mut self) {
        self.trace.push("parse_sampler_state".into());
        self.skip_block();
        self.skip_to(";");
        self.samplers += 1;
    }

    /// `ep_check_for_keyword` as the C writes it.
    fn check_for_keyword(&mut self, keyword: &'static CStr, val: &mut bool) -> i32 {
        let keyword_text = keyword.to_string_lossy();
        self.trace
            .push(format!("check_for_keyword {keyword_text} at {}", self.at()));
        if self.text() != keyword.to_bytes() {
            return PARSE_SUCCESS;
        }
        // cf_next_valid_token
        if self.kind() != CfTokenType::None as i32 {
            self.advance();
        }
        self.skip_space();
        if self.kind() == CfTokenType::None as i32 {
            self.error("Unexpected EOF", LEX_ERROR);
            return PARSE_EOF;
        }
        if *val {
            self.error(
                &format!("'{keyword_text}' keyword already specified"),
                LEX_WARNING,
            );
        }
        *val = true;
        PARSE_CONTINUE
    }

    fn parse_function(&mut self, type_name: Vec<u8>, name: Vec<u8>) {
        let name = String::from_utf8(name).unwrap();
        self.trace.push(format!(
            "parse_function {} {name} at {}",
            String::from_utf8_lossy(&type_name),
            self.at()
        ));
        self.skip_block();
        self.funcs.push(name);
    }

    fn parse_param(&mut self, type_name: Vec<u8>, name: Vec<u8>, specifiers: Specifiers) {
        let name = String::from_utf8(name).unwrap();
        self.trace.push(format!(
            "parse_param {} {name} {specifiers:?} at {}",
            String::from_utf8_lossy(&type_name),
            self.at()
        ));
        self.skip_to(";");
        self.params.push((name, specifiers));
    }

    fn has_errors(&self) -> bool {
        self.errors.iter().any(|(_, level)| *level == LEX_ERROR)
    }

    fn compile(&mut self) -> bool {
        self.trace.push("compile".into());
        self.compile_calls += 1;
        self.compile_result
    }
}

/// The C test's `check_parse()`: parse `text` and check what it checks.
#[allow(clippy::too_many_arguments)]
fn check_parse(
    text: &'static str,
    backend: Option<&'static str>,
    compiler_ok: bool,
    expected_ok: bool,
    expected_calls: usize,
    params: usize,
    structs: usize,
    samplers: usize,
    funcs: usize,
    techniques: usize,
    errors: usize,
    param_name: Option<&str>,
    first_error: Option<&str>,
) -> Fake {
    let mut fake = Fake::new(text, backend, compiler_ok);
    let ok = parse(&mut fake);
    assert_eq!(ok, expected_ok, "{text:?}");
    assert!(fake.effect_set, "{text:?}");
    assert_eq!(fake.compile_calls, expected_calls, "{text:?}");
    assert_eq!(fake.params.len(), params, "{text:?}");
    assert_eq!(fake.structs, structs, "{text:?}");
    assert_eq!(fake.samplers, samplers, "{text:?}");
    assert_eq!(fake.funcs.len(), funcs, "{text:?}");
    assert_eq!(fake.techniques, techniques, "{text:?}");
    assert_eq!(fake.errors.len(), errors, "{text:?}: {:?}", fake.errors);
    if let Some(param_name) = param_name {
        assert_eq!(fake.params[0].0, param_name, "{text:?}");
    }
    if let Some(first_error) = first_error {
        assert_eq!(fake.errors[0].0, first_error, "{text:?}");
    }
    fake
}

#[test]
fn empty_and_compile_result() {
    // Characterized, not endorsed: an empty source fails before compile.
    check_parse("", None, true, false, 0, 0, 0, 0, 0, 0, 0, None, None);
    check_parse(
        " \t\r\n;;; // comment\n",
        None,
        true,
        true,
        1,
        0,
        0,
        0,
        0,
        0,
        0,
        None,
        None,
    );
    check_parse(
        "float value;",
        None,
        false,
        false,
        1,
        1,
        0,
        0,
        0,
        0,
        0,
        Some("value"),
        None,
    );
}

#[test]
fn backend_preprocessor_definition() {
    let text = "#ifdef ENTRY_BACKEND\nfloat selected;\n#else\nfloat fallback;\n#endif\n";
    let fake = check_parse(
        text,
        Some("ENTRY_BACKEND"),
        true,
        true,
        1,
        1,
        0,
        0,
        0,
        0,
        0,
        Some("selected"),
        None,
    );
    assert_eq!(fake.defines, ["ENTRY_BACKEND"]);
    let fake = check_parse(
        text,
        None,
        true,
        true,
        1,
        1,
        0,
        0,
        0,
        0,
        0,
        Some("fallback"),
        None,
    );
    assert!(fake.defines.is_empty());
    check_parse(
        text,
        Some("OTHER_BACKEND"),
        true,
        true,
        1,
        1,
        0,
        0,
        0,
        0,
        0,
        Some("fallback"),
        None,
    );
}

#[test]
fn dispatch_all_top_level_kinds() {
    check_parse(
        "; struct S { float x; }; sampler_state sample { Filter = Linear; }; \
         float value; float fun() { return value; } technique Draw { pass {} } ;",
        None,
        true,
        true,
        1,
        1,
        1,
        1,
        1,
        1,
        0,
        Some("value"),
        None,
    );
}

#[test]
fn parse_errors_gate_compile_but_warnings_do_not() {
    check_parse(
        "uniform uniform float value;",
        None,
        true,
        true,
        1,
        1,
        0,
        0,
        0,
        0,
        1,
        Some("value"),
        Some("'uniform' keyword already specified"),
    );
    check_parse(
        "uniform float fun() {}",
        None,
        true,
        false,
        0,
        0,
        0,
        0,
        1,
        0,
        1,
        None,
        Some("'uniform' keyword cannot be used with a function"),
    );
    // Preprocessor errors gate compilation but do not stop declaration
    // dispatch.
    check_parse(
        "#error stop_here\nfloat ignored;",
        None,
        true,
        false,
        0,
        1,
        0,
        0,
        0,
        0,
        1,
        Some("ignored"),
        None,
    );
}

#[test]
fn unexpected_braces_recover_to_next_declaration() {
    // Characterized, not endorsed: parsing keeps declarations after the
    // error, even though the accumulated error prevents the compile step.
    check_parse(
        "{{}} ; float recovered;",
        None,
        true,
        false,
        0,
        1,
        0,
        0,
        0,
        0,
        1,
        Some("recovered"),
        Some("Unexpected code segment"),
    );
}

fn trace_of(fake: &Fake) -> Vec<&str> {
    fake.trace.iter().map(String::as_str).collect()
}

#[test]
fn edge_call_order_of_a_declaration() {
    let mut fake = Fake::new("const property uniform float4 x;", Some("GL"), true);
    assert!(parse(&mut fake));
    assert_eq!(
        trace_of(&fake),
        [
            "preprocessor_name",
            "add_def GL",
            "set_effect",
            "parse_text",
            "check_for_keyword property at const",
            "check_for_keyword const at const",
            "check_for_keyword property at property",
            "check_for_keyword property at uniform",
            "check_for_keyword const at uniform",
            "check_for_keyword uniform at uniform",
            "check_for_keyword property at float4",
            "check_for_keyword const at float4",
            "check_for_keyword uniform at float4",
            "parse_param float4 x Specifiers { is_property: true, is_const: true, is_uniform: true } at ;",
            "compile",
        ]
    );
}

#[test]
fn edge_function_with_every_specifier() {
    let mut fake = Fake::new("property const uniform float f() {}", None, true);
    assert!(!parse(&mut fake));
    let errors: Vec<&str> = fake.errors.iter().map(|(e, _)| e.as_str()).collect();
    assert_eq!(
        errors,
        [
            "'property' keyword cannot be used with a function",
            "'const' keyword cannot be used with a function",
            "'uniform' keyword cannot be used with a function",
        ]
    );
    assert_eq!(fake.funcs, ["f"]);
    assert_eq!(fake.compile_calls, 0);
    // The errors go at the "(" token, before the function parser runs.
    let at_paren = fake
        .trace
        .iter()
        .position(|t| t.starts_with("parse_function float f at ("))
        .unwrap();
    assert!(
        fake.trace[..at_paren]
            .iter()
            .filter(|t| t.starts_with("error"))
            .count()
            == 3
    );
}

#[test]
fn edge_failed_text_sets_the_effect_first_and_stops() {
    let mut fake = Fake::new("", Some("D3D"), true);
    assert!(!parse(&mut fake));
    assert_eq!(
        trace_of(&fake),
        [
            "preprocessor_name",
            "add_def D3D",
            "set_effect",
            "parse_text"
        ]
    );
}

#[test]
fn edge_null_token_skips_the_walk() {
    let mut fake = Fake::new("float x;", None, false);
    fake.no_token = true;
    assert!(!parse(&mut fake));
    assert_eq!(
        trace_of(&fake),
        ["preprocessor_name", "set_effect", "parse_text", "compile"]
    );
    assert!(fake.params.is_empty());
}

#[test]
fn edge_declaration_cut_short() {
    // The end of the text after a specifier, a type or a name: the
    // declaration is dropped, each with its "Unexpected EOF".
    for (text, errors) in [
        ("uniform", 1),
        ("float", 1),
        ("float x", 1),
        ("uniform float", 1),
    ] {
        let mut fake = Fake::new(text, None, true);
        assert!(!parse(&mut fake), "{text:?}");
        assert!(fake.params.is_empty() && fake.funcs.is_empty(), "{text:?}");
        assert_eq!(fake.errors.len(), errors, "{text:?}: {:?}", fake.errors);
        assert_eq!(fake.errors[0].0, "Unexpected EOF", "{text:?}");
        assert_eq!(fake.compile_calls, 0, "{text:?}");
    }
}

#[test]
fn edge_type_and_name_must_be_names() {
    // A type that is not a name skips to the ";" with its error; so does a
    // name that is not one, and the walk goes on after it.
    let mut fake = Fake::new("1 x; float 2; float ok;", None, true);
    assert!(!parse(&mut fake));
    let errors: Vec<&str> = fake.errors.iter().map(|(e, _)| e.as_str()).collect();
    assert_eq!(errors, ["Expected 'type'", "Expected 'name'"]);
    assert_eq!(fake.params.len(), 1);
    assert_eq!(fake.params[0].0, "ok");
}

#[test]
fn edge_specifiers_alone() {
    let mut fake = Fake::new("const", None, true);
    fake.lexed = Some(cf_lex(b"const x"));
    let mut specifiers = Specifiers::default();
    assert!(get_var_specifiers(&mut fake, &mut specifiers));
    assert!(specifiers.is_const && !specifiers.is_property && !specifiers.is_uniform);
    assert_eq!(fake.at(), "x");

    fake.lexed = Some(cf_lex(b"uniform"));
    fake.cur = 0;
    let mut specifiers = Specifiers::default();
    assert!(!get_var_specifiers(&mut fake, &mut specifiers));
}

#[test]
fn edge_parse_other_leaves_the_walk_at_the_param_end() {
    let mut fake = Fake::new("", None, true);
    fake.lexed = Some(cf_lex(b"float a; float b;"));
    parse_other(&mut fake);
    assert_eq!(fake.params.len(), 1);
    assert_eq!(fake.at(), ";");
}
