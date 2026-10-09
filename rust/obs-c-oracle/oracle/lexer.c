/* Test-only oracle: the original libobs/util/lexer.c, unmodified, with every
 * global symbol renamed to oracle_* so it can link next to the Rust
 * implementation. cf-lexer.c / cf-parser.c are not part of this oracle. */
#include <stddef.h>
#include <string.h>

#define strref_cmp oracle_strref_cmp
#define strref_cmpi oracle_strref_cmpi
#define strref_cmp_strref oracle_strref_cmp_strref
#define strref_cmpi_strref oracle_strref_cmpi_strref
#define valid_int_str oracle_valid_int_str
#define valid_float_str oracle_valid_float_str
#define error_data_add oracle_error_data_add
#define error_data_buildstring oracle_error_data_buildstring
#define lexer_getbasetoken oracle_lexer_getbasetoken
#define lexer_getstroffset oracle_lexer_getstroffset

/* dstr_catf comes from the dstr oracle (oracle/dstr_libc.c), so the oracle
 * runs the original C formatting. */
#define dstr_catf oracle_dstr_catf

#include "util/lexer.c"

/* Layout of the structs as the C compiler sees the real header. */
size_t oracle_strref_size(void)
{
	return sizeof(struct strref);
}
size_t oracle_strref_align(void)
{
	return _Alignof(struct strref);
}
size_t oracle_strref_offset_array(void)
{
	return offsetof(struct strref, array);
}
size_t oracle_strref_offset_len(void)
{
	return offsetof(struct strref, len);
}

size_t oracle_base_token_size(void)
{
	return sizeof(struct base_token);
}
size_t oracle_base_token_align(void)
{
	return _Alignof(struct base_token);
}
size_t oracle_base_token_offset_text(void)
{
	return offsetof(struct base_token, text);
}
size_t oracle_base_token_offset_type(void)
{
	return offsetof(struct base_token, type);
}
size_t oracle_base_token_offset_passed_whitespace(void)
{
	return offsetof(struct base_token, passed_whitespace);
}

size_t oracle_error_item_size(void)
{
	return sizeof(struct error_item);
}
size_t oracle_error_item_align(void)
{
	return _Alignof(struct error_item);
}
size_t oracle_error_item_offset_error(void)
{
	return offsetof(struct error_item, error);
}
size_t oracle_error_item_offset_file(void)
{
	return offsetof(struct error_item, file);
}
size_t oracle_error_item_offset_row(void)
{
	return offsetof(struct error_item, row);
}
size_t oracle_error_item_offset_column(void)
{
	return offsetof(struct error_item, column);
}
size_t oracle_error_item_offset_level(void)
{
	return offsetof(struct error_item, level);
}

size_t oracle_error_data_size(void)
{
	return sizeof(struct error_data);
}
size_t oracle_error_data_align(void)
{
	return _Alignof(struct error_data);
}
size_t oracle_error_data_offset_errors(void)
{
	return offsetof(struct error_data, errors);
}

size_t oracle_lexer_size(void)
{
	return sizeof(struct lexer);
}
size_t oracle_lexer_align(void)
{
	return _Alignof(struct lexer);
}
size_t oracle_lexer_offset_text(void)
{
	return offsetof(struct lexer, text);
}
size_t oracle_lexer_offset_offset(void)
{
	return offsetof(struct lexer, offset);
}

/* Enum values and sizes, for the c_int mapping. */
size_t oracle_base_token_type_size(void)
{
	return sizeof(enum base_token_type);
}
int oracle_basetoken_value(int which)
{
	switch (which) {
	case 0:
		return BASETOKEN_NONE;
	case 1:
		return BASETOKEN_ALPHA;
	case 2:
		return BASETOKEN_DIGIT;
	case 3:
		return BASETOKEN_WHITESPACE;
	default:
		return BASETOKEN_OTHER;
	}
}
int oracle_ignore_whitespace_value(int ignore)
{
	return ignore ? IGNORE_WHITESPACE : PARSE_WHITESPACE;
}
