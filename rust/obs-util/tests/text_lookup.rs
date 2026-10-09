//! Tier 1: safe-core tests. The `test_text_lookup_*` tests mirror
//! `test/cmocka/test_text_lookup.c` case for case (`create_and_getstr_test`,
//! `add_overrides_test`, `missing_file_test`, `null_lookup_test`); the rest are
//! edge cases of the INI-like format.
//!
//! Intentional difference from C: a token whose opening quote is the last
//! character on its line (`Key="` directly followed by a newline or the end of
//! the file) makes the C length wrap to `SIZE_MAX` and crash. Rust does not
//! crash: the token is treated as an empty string, which (exactly like
//! `Key=""` in C) ends the parse of the rest of the file; entries parsed
//! before it are kept and the entry itself is not added.

use obs_c_oracle as _; // links the test allocator
use obs_util::text_lookup::TextLookup;
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A temp file that is removed on drop.
struct TempFile(PathBuf);

impl TempFile {
    fn new(data: &[u8]) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "obs_rust_text_lookup_{}_{}.ini",
            std::process::id(),
            n
        ));
        std::fs::write(&path, data).unwrap();
        TempFile(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn missing_path() -> PathBuf {
    std::env::temp_dir().join("obs_rust_text_lookup_does_not_exist.ini")
}

/// Creates a lookup from `data` like `text_lookup_create`.
fn create(data: &[u8]) -> Option<TextLookup> {
    let file = TempFile::new(data);
    let mut lookup = TextLookup::new();
    lookup.add_file(file.path()).then_some(lookup)
}

fn getstr(lookup: &TextLookup, key: &str) -> Option<Vec<u8>> {
    let key = CString::new(key).unwrap();
    lookup.get(&key).map(|v| v.to_bytes().to_vec())
}

#[track_caller]
fn assert_lookup(lookup: &TextLookup, key: &str, expected: &str) {
    assert_eq!(
        getstr(lookup, key).as_deref(),
        Some(expected.as_bytes()),
        "key {key:?}"
    );
}

#[track_caller]
fn assert_missing(lookup: &TextLookup, key: &str) {
    assert_eq!(getstr(lookup, key), None, "key {key:?}");
}

const INI1: &str = "# a comment line\n\
                    Key=\"Value\"\n\
                    Plain=hello\n\
                    Esc=\"a\\nb\\tc\"\n\
                    Quote=\"say \\\"hi\\\"\"\n\
                    Dup=\"first\"\n\
                    Dup=\"second\"\n\
                    Last=\"end\"";

const INI2: &str = "Key=\"Override\"\n\
                    Extra=\"more\"";

#[test]
fn test_text_lookup_create_and_getstr() {
    let lookup = create(INI1.as_bytes()).expect("create");

    assert_lookup(&lookup, "Key", "Value");
    assert_lookup(&lookup, "Plain", "hello");
    assert_lookup(&lookup, "Esc", "a\nb\tc");
    assert_lookup(&lookup, "Quote", "say \"hi\"");
    // later duplicate replaces the earlier one
    assert_lookup(&lookup, "Dup", "second");
    // final line without trailing newline is still read
    assert_lookup(&lookup, "Last", "end");

    assert_missing(&lookup, "Missing");
    assert_missing(&lookup, "key"); // lookup is case sensitive
}

#[test]
fn test_text_lookup_add_overrides() {
    let f1 = TempFile::new(INI1.as_bytes());
    let f2 = TempFile::new(INI2.as_bytes());

    let mut lookup = TextLookup::new();
    assert!(lookup.add_file(f1.path()));
    assert_lookup(&lookup, "Key", "Value");
    assert_missing(&lookup, "Extra");

    assert!(lookup.add_file(f2.path()));

    assert_lookup(&lookup, "Key", "Override");
    assert_lookup(&lookup, "Extra", "more");
    // keys absent from the second file survive
    assert_lookup(&lookup, "Plain", "hello");

    // adding a missing file fails and leaves existing entries intact
    assert!(!lookup.add_file(&missing_path()));
    assert_lookup(&lookup, "Key", "Override");
}

#[test]
fn test_text_lookup_missing_file() {
    let mut lookup = TextLookup::new();
    assert!(!lookup.add_file(&missing_path()));
}

#[test]
fn test_text_lookup_empty_lookup() {
    // stands in for the NULL-lookup case of the C test at the safe layer
    let lookup = TextLookup::new();
    assert_missing(&lookup, "Key");
}

// ---- edge cases ----------------------------------------------------------

#[test]
fn empty_file_fails() {
    assert!(create(b"").is_none());
}

#[test]
fn bom_only_file_fails() {
    assert!(create(b"\xEF\xBB\xBF").is_none());
}

#[test]
fn bom_is_stripped() {
    let lookup = create(b"\xEF\xBB\xBFA=\"1\"\n").unwrap();
    assert_lookup(&lookup, "A", "1");
}

#[test]
fn leading_nul_succeeds_with_no_entries() {
    let lookup = create(b"\0A=\"1\"").unwrap();
    assert_missing(&lookup, "A");
}

#[test]
fn data_after_nul_is_ignored() {
    let lookup = create(b"A=\"1\"\n\0B=\"2\"\n").unwrap();
    assert_lookup(&lookup, "A", "1");
    assert_missing(&lookup, "B");
}

#[test]
fn crlf_line_endings() {
    let lookup = create(b"A=\"1\"\r\nB=\"2\"\r\n").unwrap();
    assert_lookup(&lookup, "A", "1");
    assert_lookup(&lookup, "B", "2");
}

#[test]
fn comments_are_skipped() {
    let lookup = create(b"# one\n#two\nA=\"1\" # trailing\n# three\nB=\"2\"\n").unwrap();
    assert_lookup(&lookup, "A", "1");
    assert_lookup(&lookup, "B", "2");
}

#[test]
fn hash_inside_quotes_is_kept() {
    let lookup = create(b"A=\"x#y\"\n").unwrap();
    assert_lookup(&lookup, "A", "x#y");
}

#[test]
fn hash_ends_unquoted_value() {
    let lookup = create(b"A=ab#c\n").unwrap();
    assert_lookup(&lookup, "A", "ab");
}

#[test]
fn unquoted_value_stops_at_whitespace() {
    let lookup = create(b"A=hello world\nB=1\n").unwrap();
    assert_lookup(&lookup, "A", "hello");
    assert_lookup(&lookup, "B", "1");
}

#[test]
fn unquoted_value_joins_mixed_tokens() {
    let lookup = create(b"A=12.5abc\n").unwrap();
    assert_lookup(&lookup, "A", "12.5abc");
}

#[test]
fn whitespace_before_value_becomes_the_value() {
    // C quirk: `Key = "v"` yields the single space after the key.
    let lookup = create(b"Key = \"v\"\n").unwrap();
    assert_lookup(&lookup, "Key", " ");
}

#[test]
fn line_without_value_is_skipped() {
    let lookup = create(b"Justname\n\nA=\"1\"\n").unwrap();
    assert_missing(&lookup, "Justname");
    assert_lookup(&lookup, "A", "1");
}

#[test]
fn key_without_equals_sign() {
    let lookup = create(b"A \"1\"\n").unwrap();
    assert_lookup(&lookup, "A", " ");
}

#[test]
fn text_after_value_is_ignored() {
    let lookup = create(b"A=\"1\" junk = \"x\"\nB=\"2\"\n").unwrap();
    assert_lookup(&lookup, "A", "1");
    assert_missing(&lookup, "junk");
    assert_lookup(&lookup, "B", "2");
}

#[test]
fn all_escapes() {
    let lookup = create(br#"A="1\n2\t3\r4\"5""#).unwrap();
    assert_lookup(&lookup, "A", "1\n2\t3\r4\"5");
}

#[test]
fn escape_replacement_is_sequential() {
    // `\\n` is `\`, `\n`: the first pass turns the trailing `\n` into LF.
    let lookup = create(br#"A="\\n""#).unwrap();
    assert_eq!(getstr(&lookup, "A").unwrap(), b"\\\n");
}

#[test]
fn unknown_escape_is_kept() {
    let lookup = create(br#"A="a\qb""#).unwrap();
    assert_lookup(&lookup, "A", "a\\qb");
}

#[test]
fn quoted_value_may_contain_spaces_and_equals() {
    let lookup = create(b"A=\"x = y  z\"\n").unwrap();
    assert_lookup(&lookup, "A", "x = y  z");
}

#[test]
fn non_ascii_bytes_roundtrip() {
    let lookup = create("A=\"h\u{e9}llo\"\n".as_bytes()).unwrap();
    assert_lookup(&lookup, "A", "h\u{e9}llo");
}

#[test]
fn invalid_utf8_value_roundtrip() {
    let lookup = create(b"A=\"\xff\xfe\"\n").unwrap();
    assert_eq!(getstr(&lookup, "A").unwrap(), b"\xff\xfe");
}

#[test]
fn quoted_key() {
    let lookup = create(b"\"My Key\"=\"v\"\n").unwrap();
    assert_lookup(&lookup, "My Key", "v");
}

#[test]
fn unterminated_string_runs_to_end_of_line() {
    let lookup = create(b"A=\"abc\nB=\"2\"\n").unwrap();
    assert_lookup(&lookup, "A", "abc");
    assert_lookup(&lookup, "B", "2");
}

#[test]
fn unterminated_string_at_eof() {
    let lookup = create(b"A=\"abc").unwrap();
    assert_lookup(&lookup, "A", "abc");
}

#[test]
fn empty_quoted_value_ends_parsing() {
    // C quirk: an empty token makes lookup_gettoken fail, which stops the parse.
    let lookup = create(b"A=\"1\"\nKey=\"\"\nB=\"2\"\n").unwrap();
    assert_lookup(&lookup, "A", "1");
    assert_missing(&lookup, "Key");
    assert_missing(&lookup, "B");
}

// ---- intentional difference: C crashes ----------------------------------

#[test]
fn opening_quote_last_on_line_does_not_crash() {
    let lookup = create(b"A=\"1\"\nKey=\"\nB=\"2\"\n").unwrap();
    assert_lookup(&lookup, "A", "1");
    assert_missing(&lookup, "Key");
    assert_missing(&lookup, "B");
}

#[test]
fn opening_quote_last_in_file_does_not_crash() {
    let lookup = create(b"A=\"1\"\nKey=\"").unwrap();
    assert_lookup(&lookup, "A", "1");
    assert_missing(&lookup, "Key");
}

#[test]
fn quoted_key_with_opening_quote_last_does_not_crash() {
    let lookup = create(b"A=\"1\"\n\"\nB=\"2\"\n").unwrap();
    assert_lookup(&lookup, "A", "1");
    assert_missing(&lookup, "B");
}

#[test]
fn only_an_opening_quote_does_not_crash() {
    let lookup = create(b"\"").unwrap();
    assert_missing(&lookup, "");
}
