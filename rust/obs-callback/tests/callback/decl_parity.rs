//! Tier 3: `parse_decl_string` against the C oracle (oracle/decl.c, the
//! original decl.c.h on the original C parser).
//!
//! Each case runs both, each on a zeroed `decl_info`, and compares the
//! return value, `name`, `decl_string`, every parameter, the darray's
//! `num` and `capacity`, and the logged warning (text and level).
//!
//! Excluded inputs, exactly the intentional differences in
//! `obs_callback::decl`:
//! - a first token that is not a name (C reads an uninitialized return
//!   type),
//! - a function-name token that is not a name (C calls `strcmp` on null),
//! - a `#` first on a line (C runs the preprocessor),
//! - a terminal line splice within a comment or string, the existing
//!   tokenizer exclusion documented in `cf_tokenizer_parity.rs`.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::ptr;

use obs_c_oracle::decl as oracle;
use obs_callback::ffi::decl::{decl_info, decl_param, parse_decl_string};
use obs_util::cf_tokenizer::{CfTokenType, cf_lex};
use obs_util::ffi::darray::darray;
use proptest::prelude::*;

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    ok: bool,
    name: Option<Vec<u8>>,
    /// `decl_string` is the input pointer (true) or null (false).
    keeps_string: bool,
    params: Vec<(Vec<u8>, c_int, u32)>,
    num: usize,
    capacity: usize,
    log: String,
    level: c_int,
}

fn zeroed() -> decl_info {
    decl_info {
        name: ptr::null_mut(),
        decl_string: ptr::null(),
        params: darray {
            array: ptr::null_mut(),
            num: 0,
            capacity: 0,
        },
    }
}

fn c_bytes(p: *const c_char) -> Option<Vec<u8>> {
    // SAFETY: the parser's names are null or NUL-terminated `bmalloc` copies.
    (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_bytes().to_vec())
}

unsafe extern "C" {
    fn bfree(ptr: *mut c_void);
}

/// `decl_info_free`, which is inline in decl.h.
fn free(info: &mut decl_info) {
    let items = info.params.array.cast::<decl_param>();
    // SAFETY: `num` items from the parser, each name `bmalloc` or null.
    unsafe {
        for i in 0..info.params.num {
            bfree((*items.add(i)).name.cast());
        }
        bfree(info.params.array);
        bfree(info.name.cast());
    }
    *info = zeroed();
}

fn run(parse: unsafe extern "C" fn(*mut c_void, *const c_char) -> bool, text: &CStr) -> Outcome {
    let _guard = super::LOG_HANDLER.lock().unwrap_or_else(|e| e.into_inner());
    let mut info = zeroed();
    let mut log = vec![0u8; 16384];
    let mut level = 0;
    // SAFETY: `info` is a zeroed decl_info and `text` is NUL-terminated.
    // The capture buffer is `log.len()` bytes.
    let (ok, len) = unsafe {
        oracle::oracle_decl_log_begin();
        let ok = parse((&raw mut info).cast(), text.as_ptr());
        let len = oracle::oracle_decl_log_end(log.as_mut_ptr().cast(), log.len(), &mut level);
        (ok, len)
    };
    log.truncate(len);
    let items = info.params.array.cast::<decl_param>();
    let params = (0..info.params.num)
        .map(|i| {
            // SAFETY: the first `num` items are valid decl_params.
            let p = unsafe { &*items.add(i) };
            (c_bytes(p.name).unwrap_or_default(), p.r#type, p.flags)
        })
        .collect();
    let out = Outcome {
        ok,
        name: c_bytes(info.name),
        keeps_string: info.decl_string == text.as_ptr(),
        params,
        num: info.params.num,
        capacity: info.params.capacity,
        log: String::from_utf8_lossy(&log).into_owned(),
        level,
    };
    free(&mut info);
    out
}

unsafe extern "C" fn rust_parse(decl: *mut c_void, text: *const c_char) -> bool {
    // SAFETY: forwarded from `run`.
    unsafe { parse_decl_string(decl.cast(), text) }
}

unsafe extern "C" fn c_parse(decl: *mut c_void, text: *const c_char) -> bool {
    // SAFETY: forwarded from `run`.
    unsafe { oracle::oracle_parse_decl_string(decl, text) }
}

/// The inputs the intentional differences cover (module docs).
fn excluded(text: &[u8]) -> bool {
    if text.is_empty() {
        return false;
    }
    let lexed = cf_lex(text);
    let terminal_splice = [b"\\\n".as_slice(), b"\\\r\n", b"\\\r", b"\\\n\r"]
        .iter()
        .any(|suffix| text.ends_with(suffix));
    if terminal_splice {
        let last = lexed
            .tokens
            .iter()
            .rev()
            .find(|t| t.kind != CfTokenType::None);
        if let Some(last) = last {
            // The lexer merges leading spaces and tabs into the comment token.
            let comment = last.kind == CfTokenType::SpaceTab
                && text[last.unmerged_start..]
                    .iter()
                    .find(|b| !matches!(b, b' ' | b'\t'))
                    == Some(&b'/');
            // Only an unterminated token consumes the trailing splice.
            // A closed comment or string followed by a splice stays covered.
            if (comment || last.kind == CfTokenType::String)
                && last.unmerged_start + last.unmerged_len >= text.len()
            {
                return true;
            }
        }
    }
    let tok = |t: &obs_util::cf_tokenizer::CfToken| {
        &lexed.reformatted[t.str_start..t.str_start + t.str_len]
    };
    let mut newline = true;
    for t in &lexed.tokens {
        match t.kind {
            CfTokenType::None => break,
            CfTokenType::Newline => newline = true,
            CfTokenType::SpaceTab => {}
            _ => {
                if newline && tok(t).first() == Some(&b'#') {
                    return true;
                }
                newline = false;
            }
        }
    }
    if lexed.unexpected_eof {
        return false;
    }
    if lexed.tokens[0].kind != CfTokenType::Name {
        return true;
    }
    let next = lexed.tokens[1..]
        .iter()
        .find(|t| !matches!(t.kind, CfTokenType::SpaceTab | CfTokenType::Newline))
        .expect("the terminator ends the tokens");
    !matches!(next.kind, CfTokenType::Name | CfTokenType::None)
}

fn assert_parity(text: &[u8]) {
    let c_text = CString::new(text).expect("no NUL");
    let c_side = run(c_parse, &c_text);
    let rust_side = run(rust_parse, &c_text);
    assert_eq!(
        rust_side,
        c_side,
        "input {:?}",
        String::from_utf8_lossy(text)
    );
}

const CASES: &[&str] = &[
    "void f(int a)",
    "int get(in ptr src, out string s, in out float x)",
    "bool f(int a, float b, bool c, ptr d, string e)",
    "void destroy()",
    "ptr get()",
    "void f(foo a)",
    "void f(void a)",
    "foo f(int a)",
    "void f(int a, int a)",
    "void f(int return)",
    "void f(int int)",
    "void int(int a)",
    "void f(in in int a)",
    "void f(out out int a)",
    "void f(int a",
    "",
    "void",
    "void f",
    "void f(",
    "void f(int",
    "void f(int a,",
    "void f(int a int b)",
    "void f(int a,, int b)",
    "void f(int a) trailing",
    "void f x(int a)",
    "void f{ a } (int a)",
    "void f(foo {a, b}, int c)",
    "void f(foo {a {b, c} d}, int e)",
    "void f{ {x} y }(int a)",
    "void f(foo {a {b}",
    "void f(in out in int a, out bool b)",
    "void\n  f(\n\tint a,\r\n string s\n)",
    "void f(int a /* note */, string // line\n b)",
    "void f(int a /* open",
    "void f(int \\\n a)",
    "int f(int a) ",
    "void f(int a)\n",
    "void f(\"s\" a)",
    "void f(5)",
    "string return(int void)",
    "void f(int a, float a, bool a)",
    "void f(# a)",
    "void f(int a int b)",
    "void\n  f(\n\tint a,\r\n string /* s */ s // end\n)",
];

#[test]
fn decl_cases_match_oracle() {
    for case in CASES {
        assert_parity(case.as_bytes());
    }
}

unsafe extern "C" {
    fn blog(level: c_int, fmt: *const c_char, ...);
}

#[test]
fn decl_log_capture_keeps_only_its_own_thread() {
    // Other tests log on their own threads while a capture is open, even
    // with the decl prefix. Only the capturing thread's lines may land in it.
    let _guard = super::LOG_HANDLER.lock().unwrap_or_else(|e| e.into_inner());
    let mut info = zeroed();
    let mut log = vec![0u8; 4096];
    let mut level = 0;
    // SAFETY: no arguments; the shared lock above is held until the end.
    unsafe { oracle::oracle_decl_log_begin() };
    std::thread::spawn(|| {
        // SAFETY: "%s" with one NUL-terminated string.
        unsafe {
            blog(
                200,
                c"%s".as_ptr(),
                c"Errors/warnings for 'other thread':\n\n".as_ptr(),
            )
        };
    })
    .join()
    .expect("logging thread");
    // SAFETY: `info` is a zeroed decl_info, the text is NUL-terminated,
    // and the capture buffer is `log.len()` bytes.
    let len = unsafe {
        oracle::oracle_parse_decl_string((&raw mut info).cast(), c"void f(foo a)".as_ptr());
        oracle::oracle_decl_log_end(log.as_mut_ptr().cast(), log.len(), &mut level)
    };
    free(&mut info);
    log.truncate(len);
    assert_eq!(
        String::from_utf8_lossy(&log),
        "Errors/warnings for 'void f(foo a)':\n\ndeclaration (1, 8): Expected 'type'\n"
    );
    assert_eq!(level, 200);
}

#[test]
fn decl_exclusions_are_the_documented_ones() {
    assert!(excluded(b" void f()"));
    assert!(excluded(b"5 f()"));
    assert!(excluded(b"void 5()"));
    assert!(excluded(b"void (int a)"));
    assert!(excluded(b"#define x\nvoid f()"));
    assert!(excluded(b"void f()\n  # x"));
    assert!(!excluded(b"void f(# a)"));
    assert!(!excluded(b"void"));
    assert!(!excluded(b""));
    for case in CASES {
        assert!(!excluded(case.as_bytes()), "{case:?}");
    }
}

fn piece() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec![
        "void", "int", "float", "bool", "ptr", "string", "return", "in", "out", "f", "a", "b",
        "name2", "foo", "(", ")", ",", "{", "}", " ", "  ", "\t", "\n", "\r\n", "5", "1.5",
        "\"s\"", "'c'", "/* c */", "/*", "// c\n", "\\\n", "#", ";", "*",
    ])
}

/// A declaration-shaped prefix keeps most cases past the excluded heads.
fn declish() -> impl Strategy<Value = Vec<u8>> {
    (
        prop::sample::select(vec![
            "void", "int", "float", "bool", "ptr", "string", "foo", "in",
        ]),
        prop::sample::select(vec![" f", " get", " int", " return", "\n\tg", " f /* x */"]),
        prop::collection::vec(piece(), 0..16),
    )
        .prop_map(|(ret, name, rest)| {
            let mut s = format!("{ret}{name}").into_bytes();
            for p in rest {
                s.extend_from_slice(p.as_bytes());
            }
            s
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    #[test]
    fn random_declarations_match_oracle(text in declish()) {
        prop_assume!(!excluded(&text));
        assert_parity(&text);
    }

    #[test]
    fn random_pieces_match_oracle(pieces in prop::collection::vec(piece(), 0..24)) {
        // A fixed head: most random heads are the excluded non-name cases.
        let mut text = b"void f".to_vec();
        text.extend_from_slice(pieces.concat().as_bytes());
        prop_assume!(!excluded(&text));
        assert_parity(&text);
    }

    #[test]
    fn random_bytes_match_oracle(bytes in prop::collection::vec(1u8..=255, 0..48)) {
        let mut text = b"void f(".to_vec();
        text.extend_from_slice(&bytes);
        prop_assume!(!excluded(&text));
        assert_parity(&text);
    }
}

#[test]
fn terminal_splices_exclude_only_comment_and_string_tokens() {
    for suffix in [b"\\\n".as_slice(), b"\\\r\n", b"\\\r", b"\\\n\r"] {
        for prefix in [
            b"void f/*open".as_slice(),
            b"void f//open",
            b"void f(\"open",
        ] {
            let mut text = prefix.to_vec();
            text.extend_from_slice(suffix);
            assert!(excluded(&text), "{text:?}");
        }
        let mut text = b"void f()".to_vec();
        text.extend_from_slice(suffix);
        assert!(!excluded(&text), "ordinary terminal splice: {text:?}");
    }
    assert!(!excluded(b"void f/* open"));
    assert!(!excluded(b"void f(\"open"));
    assert!(!excluded(b"void f/* closed */\\\n"));
    assert!(!excluded(b"void f(\"closed\"\\\n"));
}

#[test]
fn issue_1549_inputs_are_excluded() {
    for case in [
        "foo get5out/*/*abreturn\\\n",
        "void f/*stringfptr)b{b{b\r\n*float}voidfbool{float\\\n",
        "void f \"s\"*#}1.5b}}// c\nfloat  foob*floatf\t\t/**'c'\\\n",
    ] {
        assert!(excluded(case.as_bytes()), "{case:?}");
    }
}
