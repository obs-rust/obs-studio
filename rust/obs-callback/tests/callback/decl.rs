//! Tier 1 mirror of `test/cmocka/test_decl.c`, plus edge cases.
//!
//! `bnum_allocs` and the `decl_string` pointer stay with that cmocka test:
//! the safe core returns owned values and does not see the C pointer. The
//! error texts below are what the C oracle produces (Tier 3 checks them on
//! random input too).

use obs_callback::decl::{
    DeclParse, PARAM_IN, PARAM_OUT, PREPROCESSOR_MESSAGE, ParamType, parse_decl,
};

fn check_param(parsed: &DeclParse, idx: usize, name: &str, kind: ParamType, flags: u32) {
    let decl = parsed.decl.as_ref().expect("parse succeeded");
    assert!(idx < decl.params.len());
    let param = &decl.params[idx];
    assert_eq!(param.name, name.as_bytes());
    assert_eq!(param.kind, kind);
    assert_eq!(param.flags, flags);
}

/// A failed parse has no result at all.
fn check_fails(text: &str) -> DeclParse {
    let parsed = parse_decl(text.as_bytes());
    assert!(parsed.decl.is_none(), "{text:?} parsed");
    parsed
}

fn errors(text: &str) -> String {
    let parsed = check_fails(text);
    String::from_utf8(parsed.error_text().unwrap_or_default()).expect("ASCII")
}

/// `void_function_test`.
#[test]
fn void_function_test() {
    let parsed = parse_decl(b"void f(int a)");
    let decl = parsed.decl.as_ref().expect("parse succeeded");
    assert_eq!(decl.name, b"f");
    assert_eq!(decl.params.len(), 1);
    // parameters without a specifier are input parameters
    check_param(&parsed, 0, "a", ParamType::Int, PARAM_IN);
    assert!(parsed.errors.is_empty());
}

/// `return_type_test`.
#[test]
fn return_type_test() {
    // A non-void return type is appended as an output parameter "return".
    let parsed = parse_decl(b"int get(in ptr src, out string s, in out float x)");
    let decl = parsed.decl.as_ref().expect("parse succeeded");
    assert_eq!(decl.name, b"get");
    assert_eq!(decl.params.len(), 4);
    check_param(&parsed, 0, "src", ParamType::Ptr, PARAM_IN);
    check_param(&parsed, 1, "s", ParamType::String, PARAM_OUT);
    check_param(&parsed, 2, "x", ParamType::Float, PARAM_IN | PARAM_OUT);
    check_param(&parsed, 3, "return", ParamType::Int, PARAM_OUT);
}

/// `all_types_test`.
#[test]
fn all_types_test() {
    let parsed = parse_decl(b"bool f(int a, float b, bool c, ptr d, string e)");
    assert_eq!(
        parsed.decl.as_ref().expect("parse succeeded").params.len(),
        6
    );
    check_param(&parsed, 0, "a", ParamType::Int, PARAM_IN);
    check_param(&parsed, 1, "b", ParamType::Float, PARAM_IN);
    check_param(&parsed, 2, "c", ParamType::Bool, PARAM_IN);
    check_param(&parsed, 3, "d", ParamType::Ptr, PARAM_IN);
    check_param(&parsed, 4, "e", ParamType::String, PARAM_IN);
    check_param(&parsed, 5, "return", ParamType::Bool, PARAM_OUT);
}

/// `empty_params_test`.
#[test]
fn empty_params_test() {
    let parsed = parse_decl(b"void destroy()");
    let decl = parsed.decl.as_ref().expect("parse succeeded");
    assert_eq!(decl.name, b"destroy");
    assert!(decl.params.is_empty());

    // a non-void function without parameters only has "return"
    let parsed = parse_decl(b"ptr get()");
    assert_eq!(
        parsed.decl.as_ref().expect("parse succeeded").params.len(),
        1
    );
    check_param(&parsed, 0, "return", ParamType::Ptr, PARAM_OUT);
}

/// `failures_test`.
#[test]
fn failures_test() {
    // types
    check_fails("void f(foo a)");
    check_fails("void f(void a)");
    check_fails("foo f(int a)");
    // names
    check_fails("void f(int a, int a)");
    check_fails("void f(int return)");
    check_fails("void f(int int)");
    check_fails("void int(int a)");
    // specifiers
    check_fails("void f(in in int a)");
    check_fails("void f(out out int a)");
    // structure
    check_fails("void f(int a");
    check_fails("");
}

#[test]
fn failure_errors_match_c() {
    assert_eq!(
        errors("void f(foo a)"),
        "declaration (1, 8): Expected 'type'\n"
    );
    assert_eq!(
        errors("void f(void a)"),
        "declaration (1, 8): Expected 'type'\n"
    );
    assert_eq!(
        errors("foo f(int a)"),
        "declaration (1, 1): Expected 'return type'\n"
    );
    assert_eq!(
        errors("void f(int a, int a)"),
        "declaration (1, 19): 'a' already exists\n"
    );
    assert_eq!(
        errors("void f(int return)"),
        "declaration (1, 12): 'return' is a reserved name\n"
    );
    assert_eq!(
        errors("void int(int a)"),
        "declaration (1, 6): 'int' is a reserved name\n"
    );
    assert_eq!(
        errors("void f(in in int a)"),
        "declaration (1, 11): 'in' specifier already exists\n"
    );
    assert_eq!(
        errors("void f(out out int a)"),
        "declaration (1, 12): 'out' specifier already exists\n"
    );
    assert_eq!(
        errors("void f(int a"),
        "declaration (1, 13): Unexpected EOF\n"
    );
}

#[test]
fn warning_is_the_c_log_text() {
    let parsed = check_fails("void f(foo a)");
    assert_eq!(
        parsed.warning(b"void f(foo a)").expect("has errors"),
        b"Errors/warnings for 'void f(foo a)':\n\ndeclaration (1, 8): Expected 'type'\n"
    );
    assert_eq!(parse_decl(b"void f()").warning(b"void f()"), None);
}

#[test]
fn failures_without_errors_log_nothing() {
    // An empty text does not lex, and an unterminated block comment fails
    // the preprocessor. C adds no error for either.
    assert_eq!(check_fails("").error_text(), None);
    assert_eq!(check_fails("void f(int a /* open").error_text(), None);
}

#[test]
fn whitespace_and_comments_between_tokens() {
    let parsed = parse_decl(b"void\n  f(\n\tint a,\r\n string /* s */ s // end\n)");
    let decl = parsed.decl.as_ref().expect("parse succeeded");
    assert_eq!(decl.name, b"f");
    check_param(&parsed, 0, "a", ParamType::Int, PARAM_IN);
    check_param(&parsed, 1, "s", ParamType::String, PARAM_IN);
}

#[test]
fn missing_comma_ends_the_parameters() {
    // Characterized, not endorsed: after "int a" C wants ',' or ')', finds
    // "int", skips to the next ',' (there is none), and succeeds with "a".
    let parsed = parse_decl(b"void f(int a int b)");
    assert_eq!(
        parsed.decl.as_ref().expect("parse succeeded").params.len(),
        1
    );
    check_param(&parsed, 0, "a", ParamType::Int, PARAM_IN);
}

#[test]
fn recovery_skips_brace_pairs() {
    // The bad type skips to the next ',' or ')', and passes "{a, b}" whole.
    assert_eq!(
        errors("void f(foo {a, b}, int c)"),
        "declaration (1, 8): Expected 'type'\n"
    );
}

// Intentional differences (module docs of `obs_callback::decl`).

#[test]
fn leading_whitespace_is_a_missing_return_type() {
    // C reads an uninitialized return type here.
    assert_eq!(
        errors(" void f()"),
        "declaration (1, 1): Expected 'return type'\n\
         declaration (1, 1): Expected 'return type'\n\
         declaration (1, 2): 'void' is a reserved name\n\
         declaration (1, 8): Expected '('\n"
    );
}

#[test]
fn function_name_that_is_not_a_name() {
    // C calls strcmp on a null name here.
    assert_eq!(
        errors("void 5()"),
        "declaration (1, 6): Expected 'function name'\n"
    );
}

#[test]
fn preprocessor_directive_is_an_error() {
    assert_eq!(
        errors("#define x\nvoid f()"),
        format!("declaration (1, 1): {PREPROCESSOR_MESSAGE}\n")
    );
    assert_eq!(
        errors("void f()\n  #include \"x\""),
        format!("declaration (2, 3): {PREPROCESSOR_MESSAGE}\n")
    );
    // A '#' that does not start a line is an ordinary token.
    assert_eq!(errors("void f(# a)"), "declaration (1, 8): Expected ')'\n");
}
