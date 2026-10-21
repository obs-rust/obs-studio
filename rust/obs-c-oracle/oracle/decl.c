/* Test-only oracle: the original libobs/callback/decl.c.h, unmodified,
 * with parse_decl_string renamed to oracle_parse_decl_string so it can link
 * next to the Rust implementation. The C parser it runs on
 * (util/cf-parser.c.h, and the preprocessor in util/cf-lexer.c.h) is not
 * ported and is compiled here too, under oracle_* names. The tokenizer,
 * lexer, and dstr functions they call are renamed to their oracles
 * (oracle/cf_tokenizer.c, oracle/lexer.c, oracle/dstr.c), so the whole
 * parse is the original C. bmalloc and blog stay the real symbols: the Rust
 * port and this oracle share them. */
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>

#define parse_decl_string oracle_parse_decl_string

#define cf_adderror oracle_cf_adderror
#define cf_pass_pair oracle_cf_pass_pair
#define cf_preprocess oracle_cf_preprocess
#define cf_preprocessor_init oracle_cf_preprocessor_init
#define cf_preprocessor_free oracle_cf_preprocessor_free
#define cf_preprocessor_add_def oracle_cf_preprocessor_add_def
#define cf_preprocessor_remove_def oracle_cf_preprocessor_remove_def
#define cf_preprocess_get_def_value oracle_cf_preprocess_get_def_value

#define cf_literal_to_str oracle_cf_literal_to_str
#define cf_lexer_init oracle_cf_lexer_init
#define cf_lexer_free oracle_cf_lexer_free
#define cf_lexer_lex oracle_cf_lexer_lex

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

#define dstr_init_copy_strref oracle_dstr_init_copy_strref
#define dstr_copy oracle_dstr_copy
#define dstr_copy_strref oracle_dstr_copy_strref
#define dstr_ncopy oracle_dstr_ncopy
#define dstr_ncopy_dstr oracle_dstr_ncopy_dstr
#define dstr_cat_dstr oracle_dstr_cat_dstr
#define dstr_cat_strref oracle_dstr_cat_strref
#define dstr_ncat oracle_dstr_ncat
#define dstr_ncat_dstr oracle_dstr_ncat_dstr
#define dstr_insert oracle_dstr_insert
#define dstr_insert_dstr oracle_dstr_insert_dstr
#define dstr_insert_ch oracle_dstr_insert_ch
#define dstr_remove oracle_dstr_remove
#define dstr_safe_printf oracle_dstr_safe_printf
#define dstr_replace oracle_dstr_replace
#define dstr_depad oracle_dstr_depad
#define dstr_left oracle_dstr_left
#define dstr_mid oracle_dstr_mid
#define dstr_right oracle_dstr_right
#define dstr_printf oracle_dstr_printf
#define dstr_catf oracle_dstr_catf
#define dstr_vprintf oracle_dstr_vprintf

/* Pointer subtraction is a ptrdiff_t. cf-lexer.c assigns it to size_t.
 * The oracle builds with -Wsign-conversion and libobs does not. */
#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic ignored "-Wsign-conversion"
#endif

/* Not static in cf-lexer.c, with no prototype in a header. */
struct cf_preprocessor;
struct strref;
bool cf_preprocess_get_def_value(struct cf_preprocessor *pp, const struct strref *def_name, int *value);

#include "callback/decl.c.h"
#include "util/cf-lexer.c.h"
#include "util/cf-parser.c.h"
#include "util/threading.h"

/* The declarations are part of the macros: the exports have no header. */
#define LAYOUT(type)                          \
	size_t oracle_##type##_size(void);    \
	size_t oracle_##type##_align(void);   \
	size_t oracle_##type##_size(void)     \
	{                                     \
		return sizeof(struct type);   \
	}                                     \
	size_t oracle_##type##_align(void)    \
	{                                     \
		return _Alignof(struct type); \
	}
#define OFFSET(type, field)                          \
	size_t oracle_##type##_offset_##field(void); \
	size_t oracle_##type##_offset_##field(void)  \
	{                                            \
		return offsetof(struct type, field); \
	}

LAYOUT(decl_param)
OFFSET(decl_param, name)
OFFSET(decl_param, type)
OFFSET(decl_param, flags)

LAYOUT(decl_info)
OFFSET(decl_info, name)
OFFSET(decl_info, decl_string)
OFFSET(decl_info, params)

/* Test-only exports called from Rust over FFI: the warning that
 * parse_decl_string logs, captured through the log handler so the Tier 3
 * test can compare the C and Rust text. The handler is process-wide, and
 * other tests in the same binary log on their own threads meanwhile: only
 * lines from the thread that called oracle_decl_log_begin, and only lines
 * that start with the decl prefix, are kept. Lines from other threads go
 * on to the previous handler. */
size_t oracle_call_param_type_size(void);
void oracle_decl_log_begin(void);
void oracle_decl_log_begin_all(void);
size_t oracle_decl_log_end(char *out, size_t capacity, int *level);

size_t oracle_call_param_type_size(void)
{
	return sizeof(enum call_param_type);
}

#define DECL_LOG_PREFIX "Errors/warnings for '"

static char decl_log[16384];
static size_t decl_log_len;
static int decl_log_level;
static log_handler_t decl_log_prev;
static void *decl_log_prev_param;
static THREAD_LOCAL bool decl_log_owner;
static bool decl_log_all;

static void decl_log_capture(int level, const char *format, va_list args, void *param)
{
	char text[sizeof(decl_log)];
	int len;
	(void)param;

	if (!decl_log_owner) {
		if (decl_log_prev)
			decl_log_prev(level, format, args, decl_log_prev_param);
		return;
	}

#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wformat-nonliteral"
#endif
	len = vsnprintf(text, sizeof(text), format, args);
#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic pop
#endif

	if (len < 0 || (!decl_log_all && strncmp(text, DECL_LOG_PREFIX, strlen(DECL_LOG_PREFIX)) != 0))
		return;
	if ((size_t)len >= sizeof(text))
		len = (int)(sizeof(text) - 1);
	if (decl_log_len + (size_t)len >= sizeof(decl_log))
		return;

	memcpy(decl_log + decl_log_len, text, (size_t)len);
	decl_log_len += (size_t)len;
	if (decl_log_all && decl_log_len + 1 < sizeof(decl_log))
		decl_log[decl_log_len++] = '\n';
	decl_log_level = level;
}

void oracle_decl_log_begin(void)
{
	base_get_log_handler(&decl_log_prev, &decl_log_prev_param);
	decl_log_len = 0;
	decl_log_level = 0;
	decl_log_owner = true;
	decl_log_all = false;
	base_set_log_handler(decl_log_capture, NULL);
}

/* As oracle_decl_log_begin, but keeps every line of this thread, each
 * followed by a newline. */
void oracle_decl_log_begin_all(void)
{
	oracle_decl_log_begin();
	decl_log_all = true;
}

size_t oracle_decl_log_end(char *out, size_t capacity, int *level)
{
	size_t len = decl_log_len < capacity ? decl_log_len : capacity;

	base_set_log_handler(decl_log_prev, decl_log_prev_param);
	decl_log_owner = false;
	memcpy(out, decl_log, len);
	*level = decl_log_level;
	return decl_log_len;
}

#undef DECL_LOG_PREFIX
