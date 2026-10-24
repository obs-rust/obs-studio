/* Test-only oracle: the original libobs/util/dstr-libc.c with its own
 * functions renamed to oracle_*. It also calls functions that the Rust port
 * replaces (the dstr.c ones) and the utf8 conversions, so those are renamed
 * to the oracle_* versions compiled by oracle/dstr.c, oracle/utf8.c and
 * oracle/platform.c.
 *
 * The two allocating conversions are the exception: dstr-libc.c frees what
 * os_utf8_to_wcs_ptr returns with bfree and keeps what os_mbs_to_utf8_ptr
 * returns as a dstr's array, both the libobs allocator that the dstr oracles
 * share. oracle/platform.c's copies allocate with oracle_bmalloc, so they
 * are defined below as their util/platform-conversions.c.h bodies around
 * oracle/platform.c's conversions, allocating with bmalloc (#2154). */

/* allocating conversions, defined at the end of this file */
#define os_mbs_to_utf8_ptr oracle_dstr_libc_mbs_to_utf8_ptr
#define os_utf8_to_wcs_ptr oracle_dstr_libc_utf8_to_wcs_ptr

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

#include "util/dstr-libc.c.h"

/* oracle/platform.c, which allocates with oracle_bmalloc */
size_t oracle_os_utf8_to_wcs(const char *str, size_t len, wchar_t *dst, size_t dst_size);
size_t oracle_os_mbs_to_wcs_ptr(const char *str, size_t len, wchar_t **pstr);
size_t oracle_os_wcs_to_utf8(const wchar_t *str, size_t len, char *dst, size_t dst_size);
void oracle_bfree(void *ptr);

size_t os_utf8_to_wcs_ptr(const char *str, size_t len, wchar_t **pstr)
{
	if (str) {
		size_t out_len = oracle_os_utf8_to_wcs(str, len, NULL, 0);

		*pstr = bmalloc((out_len + 1) * sizeof(wchar_t));
		return oracle_os_utf8_to_wcs(str, len, *pstr, out_len + 1);
	} else {
		*pstr = NULL;
		return 0;
	}
}

size_t os_mbs_to_utf8_ptr(const char *str, size_t len, char **pstr)
{
	char *dst = NULL;
	size_t out_len = 0;

	if (str) {
		wchar_t *wstr = NULL;
		size_t wlen = oracle_os_mbs_to_wcs_ptr(str, len, &wstr);

		/* os_wcs_to_utf8_ptr, allocating with bmalloc */
		if (wstr) {
			out_len = oracle_os_wcs_to_utf8(wstr, wlen, NULL, 0);
			dst = bmalloc((out_len + 1) * sizeof(char));
			out_len = oracle_os_wcs_to_utf8(wstr, wlen, dst, out_len + 1);
		}
		oracle_bfree(wstr);
	}

	*pstr = dst;
	return out_len;
}
