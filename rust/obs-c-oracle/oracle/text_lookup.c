/* Test-only oracle: the original libobs/util/text-lookup.c, unmodified, with
 * its global symbols renamed to oracle_* so it can link next to the Rust
 * implementation. Every call into a function that is also ported to Rust
 * (lexer.c, dstr.c) is renamed too, so the oracle runs the original C all the
 * way down; the oracle_lexer_* / oracle_dstr_* definitions come from those
 * ports' oracle wrappers. os_fopen comes from file_serializer_host.c,
 * os_fread_utf8 from test_stubs.c. */
#define text_lookup_create oracle_text_lookup_create
#define text_lookup_add oracle_text_lookup_add
#define text_lookup_destroy oracle_text_lookup_destroy
#define text_lookup_getstr oracle_text_lookup_getstr

/* lexer.c */
#define lexer_getbasetoken oracle_lexer_getbasetoken
#define strref_cmp oracle_strref_cmp
#define strref_cmpi oracle_strref_cmpi
#define strref_cmp_strref oracle_strref_cmp_strref
#define strref_cmpi_strref oracle_strref_cmpi_strref
#define valid_int_str oracle_valid_int_str
#define valid_float_str oracle_valid_float_str
#define lexer_getstroffset oracle_lexer_getstroffset
#define error_data_add oracle_error_data_add
#define error_data_buildstring oracle_error_data_buildstring

/* dstr.c */
#define astrcmpi oracle_astrcmpi
#define astrcmp_n oracle_astrcmp_n
#define astrcmpi_n oracle_astrcmpi_n
#define astrstri oracle_astrstri
#define strdepad oracle_strdepad
#define strlist_split oracle_strlist_split
#define strlist_free oracle_strlist_free
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

#include "util/text-lookup.c"
