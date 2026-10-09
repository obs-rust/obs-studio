/* Test-only: the string conversions libobs/util/dstr.c calls, copied
 * verbatim from libobs/util/platform.c, on top of the real util/utf8.c that
 * oracle/utf8.c compiles under oracle_* names (the Rust port owns the plain
 * utf8_to_wchar/wchar_to_utf8 names in test binaries). They used to be stubs that returned 0 for
 * every input (#84), so dstr_to_wcs, dstr_from_mbs and dstr_from_wcs in the
 * oracle disagreed with libobs.
 *
 * Deliberately includes no libobs header: platform.h marks these EXPORT. */
#include <stdlib.h>
#include <string.h>
#include <wchar.h>

#define UNUSED_PARAMETER(param) (void)param

void *bmalloc(size_t size);
void bfree(void *ptr);
#define utf8_to_wchar oracle_utf8_to_wchar
#define wchar_to_utf8 oracle_wchar_to_utf8

size_t utf8_to_wchar(const char *in, size_t insize, wchar_t *out, size_t outsize, int flags);
size_t wchar_to_utf8(const wchar_t *in, size_t insize, char *out, size_t outsize, int flags);

size_t os_mbs_to_wcs(const char *str, size_t len, wchar_t *dst, size_t dst_size)
{
	UNUSED_PARAMETER(len);

	size_t out_len;

	if (!str)
		return 0;

	out_len = dst ? (dst_size - 1) : mbstowcs(NULL, str, 0);

	if (dst) {
		if (!dst_size)
			return 0;

		if (out_len)
			out_len = mbstowcs(dst, str, out_len + 1);

		dst[out_len] = 0;
	}

	return out_len;
}

size_t os_utf8_to_wcs(const char *str, size_t len, wchar_t *dst, size_t dst_size)
{
	size_t in_len;
	size_t out_len;

	if (!str)
		return 0;

	in_len = len ? len : strlen(str);
	out_len = dst ? (dst_size - 1) : utf8_to_wchar(str, in_len, NULL, 0, 0);

	if (dst) {
		if (!dst_size)
			return 0;

		if (out_len)
			out_len = utf8_to_wchar(str, in_len, dst, out_len + 1, 0);

		dst[out_len] = 0;
	}

	return out_len;
}

size_t os_wcs_to_utf8(const wchar_t *str, size_t len, char *dst, size_t dst_size)
{
	size_t in_len;
	size_t out_len;

	if (!str)
		return 0;

	in_len = (len != 0) ? len : wcslen(str);
	out_len = dst ? (dst_size - 1) : wchar_to_utf8(str, in_len, NULL, 0, 0);

	if (dst) {
		if (!dst_size)
			return 0;

		if (out_len)
			out_len = wchar_to_utf8(str, in_len, dst, out_len, 0);

		dst[out_len] = 0;
	}

	return out_len;
}

size_t os_mbs_to_wcs_ptr(const char *str, size_t len, wchar_t **pstr)
{
	if (str) {
		size_t out_len = os_mbs_to_wcs(str, len, NULL, 0);

		*pstr = bmalloc((out_len + 1) * sizeof(wchar_t));
		return os_mbs_to_wcs(str, len, *pstr, out_len + 1);
	} else {
		*pstr = NULL;
		return 0;
	}
}

size_t os_utf8_to_wcs_ptr(const char *str, size_t len, wchar_t **pstr)
{
	if (str) {
		size_t out_len = os_utf8_to_wcs(str, len, NULL, 0);

		*pstr = bmalloc((out_len + 1) * sizeof(wchar_t));
		return os_utf8_to_wcs(str, len, *pstr, out_len + 1);
	} else {
		*pstr = NULL;
		return 0;
	}
}

size_t os_wcs_to_utf8_ptr(const wchar_t *str, size_t len, char **pstr)
{
	if (str) {
		size_t out_len = os_wcs_to_utf8(str, len, NULL, 0);

		*pstr = bmalloc((out_len + 1) * sizeof(char));
		return os_wcs_to_utf8(str, len, *pstr, out_len + 1);
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
		size_t wlen = os_mbs_to_wcs_ptr(str, len, &wstr);
		out_len = os_wcs_to_utf8_ptr(wstr, wlen, &dst);
		bfree(wstr);
	}

	*pstr = dst;
	return out_len;
}
