//! Tier 2: the `#[repr(C)]` lexer structs match the layout the C compiler
//! produces for `libobs/util/lexer.h`, and the enums keep their values.

use core::mem::{align_of, offset_of, size_of};

use obs_c_oracle::lexer as c;
use obs_util::ffi::lexer::{base_token, error_data, error_item, lexer, strref};
use obs_util::lexer::{BaseTokenType, IgnoreWhitespace};

#[test]
fn strref_layout_matches_c_header() {
    // SAFETY: the oracle layout functions take no arguments and only return constants.
    unsafe {
        assert_eq!(size_of::<strref>(), c::oracle_strref_size());
        assert_eq!(align_of::<strref>(), c::oracle_strref_align());
        assert_eq!(offset_of!(strref, array), c::oracle_strref_offset_array());
        assert_eq!(offset_of!(strref, len), c::oracle_strref_offset_len());
    }
}

#[test]
fn base_token_layout_matches_c_header() {
    // SAFETY: as above.
    unsafe {
        assert_eq!(size_of::<base_token>(), c::oracle_base_token_size());
        assert_eq!(align_of::<base_token>(), c::oracle_base_token_align());
        assert_eq!(
            offset_of!(base_token, text),
            c::oracle_base_token_offset_text()
        );
        assert_eq!(
            offset_of!(base_token, r#type),
            c::oracle_base_token_offset_type()
        );
        assert_eq!(
            offset_of!(base_token, passed_whitespace),
            c::oracle_base_token_offset_passed_whitespace()
        );
    }
}

#[test]
fn error_item_layout_matches_c_header() {
    // SAFETY: as above.
    unsafe {
        assert_eq!(size_of::<error_item>(), c::oracle_error_item_size());
        assert_eq!(align_of::<error_item>(), c::oracle_error_item_align());
        assert_eq!(
            offset_of!(error_item, error),
            c::oracle_error_item_offset_error()
        );
        assert_eq!(
            offset_of!(error_item, file),
            c::oracle_error_item_offset_file()
        );
        assert_eq!(
            offset_of!(error_item, row),
            c::oracle_error_item_offset_row()
        );
        assert_eq!(
            offset_of!(error_item, column),
            c::oracle_error_item_offset_column()
        );
        assert_eq!(
            offset_of!(error_item, level),
            c::oracle_error_item_offset_level()
        );
    }
}

#[test]
fn error_data_layout_matches_c_header() {
    // SAFETY: as above.
    unsafe {
        assert_eq!(size_of::<error_data>(), c::oracle_error_data_size());
        assert_eq!(align_of::<error_data>(), c::oracle_error_data_align());
        assert_eq!(
            offset_of!(error_data, errors),
            c::oracle_error_data_offset_errors()
        );
    }
}

#[test]
fn lexer_layout_matches_c_header() {
    // SAFETY: as above.
    unsafe {
        assert_eq!(size_of::<lexer>(), c::oracle_lexer_size());
        assert_eq!(align_of::<lexer>(), c::oracle_lexer_align());
        assert_eq!(offset_of!(lexer, text), c::oracle_lexer_offset_text());
        assert_eq!(offset_of!(lexer, offset), c::oracle_lexer_offset_offset());
    }
}

#[test]
fn enums_match_c_header() {
    let kinds = [
        BaseTokenType::None,
        BaseTokenType::Alpha,
        BaseTokenType::Digit,
        BaseTokenType::Whitespace,
        BaseTokenType::Other,
    ];
    // SAFETY: the oracle helpers are pure.
    unsafe {
        // `type` is declared `c_int`, so the C enum must be int-sized.
        assert_eq!(c::oracle_base_token_type_size(), size_of::<i32>());
        for (i, kind) in kinds.iter().enumerate() {
            assert_eq!(*kind as i32, c::oracle_basetoken_value(i as i32));
        }
        assert_eq!(
            IgnoreWhitespace::Parse as i32,
            c::oracle_ignore_whitespace_value(0)
        );
        assert_eq!(
            IgnoreWhitespace::Ignore as i32,
            c::oracle_ignore_whitespace_value(1)
        );
    }
}
