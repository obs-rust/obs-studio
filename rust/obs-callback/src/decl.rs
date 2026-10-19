//! Safe `parse_decl_string`: the procedure declaration grammar of
//! `libobs/callback/decl.c.h`, such as `"int get(in ptr src, out string s)"`.
//!
//! The text goes through the ported tokenizer ([`obs_util::cf_tokenizer`]).
//! The parse steps are the `cf-parser.h` inlines that `decl.c.h` calls
//! (`cf_next_token`, `cf_go_to_token`, `cf_pass_pair`, ...), in the same
//! order, so the errors, their positions, and the recovery after an error
//! are the C ones. Odd cases pinned by `test/cmocka/test_decl.c` (a non-void
//! return becomes a last `"return"` OUT parameter; every failure discards
//! the whole result) are characterized, not endorsed.
//!
//! Intentional differences, all where C has undefined behavior or runs the
//! preprocessor:
//! - A first token that is not a name (leading whitespace, `"5 f()"`): C
//!   reads an uninitialized return type. Here the return type is missing,
//!   which adds a second `Expected 'return type'`.
//! - A function name that is not a name (`"void 5()"`): C calls `strcmp`
//!   on a null name. Here the reserved-name check is skipped.
//! - A `#` that starts a line: C runs it as a preprocessor directive
//!   (`#include` reads a file). Here it is an error,
//!   [`PREPROCESSOR_MESSAGE`], and the parse fails.

use obs_util::cf_tokenizer::{CfLexed, CfToken, CfTokenType, cf_lex};
use obs_util::lexer::{ErrorItem, build_error_string, str_offset_in};

/// `enum call_param_type` in `libobs/callback/calldata.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum ParamType {
    Void = 0,
    Int = 1,
    Float = 2,
    Bool = 3,
    Ptr = 4,
    String = 5,
}

/// `CALL_PARAM_IN`.
pub const PARAM_IN: u32 = 1 << 0;
/// `CALL_PARAM_OUT`.
pub const PARAM_OUT: u32 = 1 << 1;

/// The `file` that `parse_decl_string` gives the parser. It is the first
/// field of each error line.
pub const ERROR_FILE: &str = "declaration";

/// Error for a `#` that starts a line (see the module docs).
pub const PREPROCESSOR_MESSAGE: &str = "Preprocessor directives are not supported";

/// `struct decl_param`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclParam {
    pub name: Vec<u8>,
    pub kind: ParamType,
    pub flags: u32,
}

/// `struct decl_info` after a successful parse, without `decl_string`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decl {
    pub name: Vec<u8>,
    pub params: Vec<DeclParam>,
}

/// One `error_data` item. `row` and `column` are 1-based, into the input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclError {
    pub row: u32,
    pub column: u32,
    pub message: Vec<u8>,
}

/// Result of [`parse_decl`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclParse {
    /// `Some` when C returns true.
    pub decl: Option<Decl>,
    /// The parser's errors in order. A failure can have none (empty text, an
    /// unterminated block comment).
    pub errors: Vec<DeclError>,
}

impl DeclParse {
    /// `error_data_buildstring`: one `"declaration (row, col): msg\n"` line
    /// per error, `None` without errors.
    pub fn error_text(&self) -> Option<Vec<u8>> {
        let items: Vec<ErrorItem<'_>> = self
            .errors
            .iter()
            .map(|e| ErrorItem {
                file: Some(ERROR_FILE.as_bytes()),
                row: e.row,
                column: e.column,
                error: Some(&e.message),
            })
            .collect();
        build_error_string(&items)
    }

    /// The text C logs at `LOG_WARNING` after the parse,
    /// `"Errors/warnings for '<decl_string>':\n\n<errors>"`. `None` when
    /// there are no errors; C logs nothing then.
    pub fn warning(&self, decl_string: &[u8]) -> Option<Vec<u8>> {
        let errors = self.error_text()?;
        let mut out = b"Errors/warnings for '".to_vec();
        out.extend_from_slice(decl_string);
        out.extend_from_slice(b"':\n\n");
        out.extend_from_slice(&errors);
        Some(out)
    }
}

// cf-parser.h return codes.
const PARSE_SUCCESS: i32 = 0;
const PARSE_CONTINUE: i32 = -1;
const PARSE_EOF: i32 = -5;

const TYPE_OR_STORAGE: &[u8] = b"type or storage specifier";

/// `parse_decl_string` on `text`, the declaration up to its NUL.
pub fn parse_decl(text: &[u8]) -> DeclParse {
    let failed = |errors| DeclParse { decl: None, errors };

    // cf_parser_parse: an empty text does not lex, and an unterminated
    // block comment fails the preprocessor. Neither adds an error.
    if text.is_empty() {
        return failed(Vec::new());
    }
    let lexed = cf_lex(text);
    let mut p = Parser {
        lexed: &lexed,
        input: text,
        cur: 0,
        errors: Vec::new(),
    };
    if let Some(at) = p.directive() {
        p.cur = at;
        p.add_error(PREPROCESSOR_MESSAGE.as_bytes());
        return failed(p.errors);
    }
    if lexed.unexpected_eof {
        return failed(p.errors);
    }

    let mut decl = Decl {
        name: Vec::new(),
        params: Vec::new(),
    };
    let ret_type = match p.parse_head(&mut decl) {
        Some(ret_type) => ret_type,
        None => return failed(p.errors),
    };
    p.parse_params(&mut decl);

    if !p.errors.is_empty() {
        return failed(p.errors);
    }
    if ret_type != ParamType::Void {
        decl.params.push(DeclParam {
            name: b"return".to_vec(),
            kind: ret_type,
            flags: PARAM_OUT,
        });
    }
    DeclParse {
        decl: Some(decl),
        errors: p.errors,
    }
}

/// `get_type`.
fn get_type(name: &[u8], is_return: bool) -> Option<ParamType> {
    match name {
        b"int" => Some(ParamType::Int),
        b"float" => Some(ParamType::Float),
        b"bool" => Some(ParamType::Bool),
        b"ptr" => Some(ParamType::Ptr),
        b"string" => Some(ParamType::String),
        b"void" if is_return => Some(ParamType::Void),
        _ => None,
    }
}

/// `is_reserved_name`.
fn is_reserved_name(name: &[u8]) -> bool {
    matches!(
        name,
        b"int" | b"float" | b"bool" | b"ptr" | b"string" | b"void" | b"return"
    )
}

/// `$1` in a `cf_adderror` format, as `dstr_safe_printf` fills it.
fn format1(format: &[u8], val: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(format.len() + val.len());
    let mut i = 0;
    while i < format.len() {
        if format[i..].starts_with(b"$1") {
            out.extend_from_slice(val);
            i += 2;
        } else {
            out.push(format[i]);
            i += 1;
        }
    }
    out
}

/// A `struct cf_parser` over the lexed tokens: `cur` is `cur_token`.
struct Parser<'a> {
    lexed: &'a CfLexed,
    input: &'a [u8],
    cur: usize,
    errors: Vec<DeclError>,
}

impl Parser<'_> {
    fn token(&self) -> &CfToken {
        &self.lexed.tokens[self.cur]
    }

    fn kind(&self) -> CfTokenType {
        self.token().kind
    }

    fn text_of(&self, token: &CfToken) -> &[u8] {
        &self.lexed.reformatted[token.str_start..token.str_start + token.str_len]
    }

    fn text(&self) -> &[u8] {
        self.text_of(self.token())
    }

    /// `*cur_token->str.array`: the terminator token reads the NUL.
    fn first_byte(&self) -> u8 {
        self.text().first().copied().unwrap_or(0)
    }

    /// The first token that `cf_preprocess_tokens` would read as a
    /// directive: a `#` first on a line.
    fn directive(&self) -> Option<usize> {
        let mut newline = true;
        for (i, token) in self.lexed.tokens.iter().enumerate() {
            match token.kind {
                CfTokenType::None => return None,
                CfTokenType::Newline => newline = true,
                CfTokenType::SpaceTab => {}
                _ => {
                    if newline && self.text_of(token).first() == Some(&b'#') {
                        return Some(i);
                    }
                    newline = false;
                }
            }
        }
        None
    }

    /// `cf_adderror`, at the current token.
    fn add_error(&mut self, message: &[u8]) {
        let (row, column) = str_offset_in(self.input, self.token().unmerged_start);
        self.errors.push(DeclError {
            row,
            column,
            message: message.to_vec(),
        });
    }

    fn add_error1(&mut self, format: &[u8], val: &[u8]) {
        self.add_error(&format1(format, val));
    }

    fn add_error_expecting(&mut self, expected: &[u8]) {
        self.add_error1(b"Expected '$1'", expected);
    }

    fn add_error_unexpected_eof(&mut self) {
        self.add_error(b"Unexpected EOF");
    }

    fn next_token(&mut self) -> bool {
        if !matches!(
            self.kind(),
            CfTokenType::SpaceTab | CfTokenType::Newline | CfTokenType::None
        ) {
            self.cur += 1;
        }
        while matches!(self.kind(), CfTokenType::SpaceTab | CfTokenType::Newline) {
            self.cur += 1;
        }
        self.kind() != CfTokenType::None
    }

    fn next_valid_token(&mut self) -> bool {
        if !self.next_token() {
            self.add_error_unexpected_eof();
            return false;
        }
        true
    }

    /// `cf_pass_pair`.
    fn pass_pair(&mut self, open: u8, close: u8) -> bool {
        if self.kind() != CfTokenType::Other || self.first_byte() != open {
            return self.kind() != CfTokenType::None;
        }
        self.cur += 1;
        while self.kind() != CfTokenType::None {
            if self.first_byte() == open {
                if !self.pass_pair(open, close) {
                    break;
                }
                continue;
            } else if self.first_byte() == close {
                self.cur += 1;
                return true;
            }
            self.cur += 1;
        }
        false
    }

    fn go_to_token(&mut self, str1: &[u8], str2: Option<&[u8]>) -> bool {
        while self.next_token() {
            if self.text() == str1 || str2.is_some_and(|s| self.text() == s) {
                return true;
            } else if self.first_byte() == b'{' && !self.pass_pair(b'{', b'}') {
                break;
            }
        }
        false
    }

    fn go_to_valid_token(&mut self, str1: &[u8]) -> bool {
        if !self.go_to_token(str1, None) {
            self.add_error_unexpected_eof();
            return false;
        }
        true
    }

    fn token_should_be(&mut self, s: &[u8], goto1: &[u8]) -> i32 {
        if self.text() == s {
            return PARSE_SUCCESS;
        }
        if !self.go_to_token(goto1, None) {
            return PARSE_EOF;
        }
        self.add_error_expecting(s);
        PARSE_CONTINUE
    }

    fn next_token_should_be(&mut self, s: &[u8], goto1: Option<&[u8]>) -> i32 {
        if !self.next_token() {
            self.add_error_unexpected_eof();
            return PARSE_EOF;
        } else if self.text() == s {
            return PARSE_SUCCESS;
        }
        if let Some(goto1) = goto1
            && !self.go_to_token(goto1, None)
        {
            return PARSE_EOF;
        }
        self.add_error_expecting(s);
        PARSE_CONTINUE
    }

    /// `cf_peek_valid_token`: the next token's kind, `None` at the end.
    fn peek_valid_token(&mut self) -> Option<CfTokenType> {
        let saved = self.cur;
        let success = self.next_token();
        let kind = self.kind();
        self.cur = saved;
        if !success {
            self.add_error_unexpected_eof();
            return None;
        }
        Some(kind)
    }

    /// `cf_get_name_ref` / `cf_get_name`: the current name, or the code.
    fn get_name(&mut self, expected: &[u8], goto_token: Option<&[u8]>) -> Result<Vec<u8>, i32> {
        if self.kind() != CfTokenType::Name {
            self.add_error_expecting(expected);
            if let Some(goto_token) = goto_token
                && !self.go_to_valid_token(goto_token)
            {
                return Err(PARSE_EOF);
            }
            return Err(PARSE_CONTINUE);
        }
        Ok(self.text().to_vec())
    }

    fn next_name(&mut self, expected: &[u8], goto_token: Option<&[u8]>) -> Result<Vec<u8>, i32> {
        if !self.next_valid_token() {
            return Err(PARSE_EOF);
        }
        self.get_name(expected, goto_token)
    }

    /// From the return type to the `(`. `None` is C's `goto fail`; the
    /// return type is that of a parse that went on.
    fn parse_head(&mut self, decl: &mut Decl) -> Option<ParamType> {
        let ret_name = match self.get_name(b"return type", None) {
            Ok(name) => Some(name),
            Err(PARSE_EOF) => return None,
            Err(_) => None,
        };
        let ret_type = match ret_name.as_deref().and_then(|n| get_type(n, true)) {
            Some(kind) => kind,
            None => {
                self.add_error_expecting(b"return type");
                ParamType::Void
            }
        };

        match self.next_name(b"function name", Some(b"(")) {
            Ok(name) => {
                if is_reserved_name(&name) {
                    self.add_error1(b"'$1' is a reserved name", &name);
                }
                decl.name = name;
            }
            Err(PARSE_EOF) => return None,
            Err(_) => {}
        }

        if self.next_token_should_be(b"(", Some(b"(")) == PARSE_EOF {
            return None;
        }
        Some(ret_type)
    }

    /// `is_in_out_specifier`.
    fn in_out_specifier(&mut self, name: &[u8], flags: &mut u32) -> bool {
        let (flag, word): (u32, &[u8]) = match name {
            b"in" => (PARAM_IN, b"in"),
            b"out" => (PARAM_OUT, b"out"),
            _ => return false,
        };
        if *flags & flag != 0 {
            self.add_error1(b"'$1' specifier already exists", word);
        }
        *flags |= flag;
        true
    }

    /// `parse_param`.
    fn parse_param(&mut self, decl: &mut Decl) -> i32 {
        let mut flags = 0;
        let mut name = match self.next_name(TYPE_OR_STORAGE, Some(b",")) {
            Ok(name) => name,
            Err(code) => return code,
        };
        while self.in_out_specifier(&name, &mut flags) {
            name = match self.next_name(TYPE_OR_STORAGE, Some(b",")) {
                Ok(name) => name,
                Err(code) => return code,
            };
        }
        if flags == 0 {
            flags = PARAM_IN;
        }

        let Some(kind) = get_type(&name, false) else {
            self.add_error_expecting(b"type");
            self.go_to_token(b",", Some(b")"));
            return PARSE_CONTINUE;
        };

        let name = match self.next_name(b"parameter name", Some(b",")) {
            Ok(name) => name,
            Err(code) => return code,
        };
        if decl.params.iter().any(|p| p.name == name) {
            self.add_error1(b"'$1' already exists", &name);
        }
        if is_reserved_name(&name) {
            self.add_error1(b"'$1' is a reserved name", &name);
        }
        decl.params.push(DeclParam { name, kind, flags });
        PARSE_SUCCESS
    }

    /// `parse_params`.
    fn parse_params(&mut self, decl: &mut Decl) {
        let Some(mut peek) = self.peek_valid_token() else {
            return;
        };
        while peek == CfTokenType::Name {
            let code = self.parse_param(decl);
            if code == PARSE_EOF {
                return;
            }
            if code != PARSE_CONTINUE && !self.next_valid_token() {
                return;
            }
            if self.text() == b")" {
                break;
            } else if self.token_should_be(b",", b",") == PARSE_EOF {
                return;
            }
            match self.peek_valid_token() {
                Some(kind) => peek = kind,
                None => return,
            }
        }
        if self.text() != b")" {
            self.next_token_should_be(b")", None);
        }
    }
}
