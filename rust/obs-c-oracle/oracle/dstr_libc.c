/* Test-only oracle: the original libobs/util/dstr-libc.c with its own
 * functions renamed to oracle_*. It also calls functions that the Rust port
 * replaces (the dstr.c ones) and the utf8 conversions, so those are renamed
 * to the oracle_* versions compiled by oracle/dstr.c and oracle/utf8.c.
 * os_mbs_to_utf8_ptr and os_utf8_to_wcs_ptr stay unrenamed (platform.c copies in
 * oracle/platform_conv_host.c). */

/* functions defined by dstr-libc.c */
#define wstrcmpi oracle_wstrcmpi
#define wstrcmp_n oracle_wstrcmp_n
#define wstrcmpi_n oracle_wstrcmpi_n
#define wstrstri oracle_wstrstri
#define wcsdepad oracle_wcsdepad
#define dstr_printf oracle_dstr_printf
#define dstr_catf oracle_dstr_catf
#define dstr_vprintf oracle_dstr_vprintf
#define dstr_vcatf oracle_dstr_vcatf
#define dstr_from_mbs oracle_dstr_from_mbs
#define dstr_to_mbs oracle_dstr_to_mbs
#define dstr_to_wcs oracle_dstr_to_wcs
#define dstr_from_wcs oracle_dstr_from_wcs
#define dstr_to_upper oracle_dstr_to_upper
#define dstr_to_lower oracle_dstr_to_lower

/* functions defined by dstr.c (Rust side) that dstr-libc.c may call */
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

/* utf8.c conversions (oracle/utf8.c) */
#define wchar_to_utf8 oracle_wchar_to_utf8
#define utf8_to_wchar oracle_utf8_to_wchar

#include "util/dstr-libc.c"
