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
        todo!()
    }

    /// The text C logs at `LOG_WARNING` after the parse,
    /// `"Errors/warnings for '<decl_string>':\n\n<errors>"`. `None` when
    /// there are no errors; C logs nothing then.
    pub fn warning(&self, _decl_string: &[u8]) -> Option<Vec<u8>> {
        todo!()
    }
}

/// `parse_decl_string` on `text`, the declaration up to its NUL.
pub fn parse_decl(_text: &[u8]) -> DeclParse {
    todo!()
}
