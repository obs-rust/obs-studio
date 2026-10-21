/*
 * Copyright (c) 2023 Lain Bailey <lain@obsproject.com>
 *
 * Permission to use, copy, modify, and distribute this software for any
 * purpose with or without fee is hereby granted, provided that the above
 * copyright notice and this permission notice appear in all copies.
 *
 * THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
 * WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
 * ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
 * WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
 * ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
 * OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
 */

#include "build/libobs-platform.pch.h"

size_t os_mbs_to_wcs(const char *str, size_t len, wchar_t *dst, size_t dst_size)
{
	UNUSED_PARAMETER(len);

	size_t out_len;

	if (!str)
		return 0;

	out_len = dst ? (dst_size - 1) : mbstowcs(NULL, str, 0);

	/* (size_t)-1: text the current locale cannot convert */
	if (out_len == (size_t)-1 && !dst)
		return 0;

	if (dst) {
		if (!dst_size)
			return 0;

		/* at most dst_size - 1, so the NUL below stays in the buffer */
		if (out_len)
			out_len = mbstowcs(dst, str, out_len);
		if (out_len == (size_t)-1)
			out_len = 0;

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

size_t os_wcs_to_mbs(const wchar_t *str, size_t len, char *dst, size_t dst_size)
{
	UNUSED_PARAMETER(len);

	size_t out_len;

	if (!str)
		return 0;

	out_len = dst ? (dst_size - 1) : wcstombs(NULL, str, 0);

	/* (size_t)-1: text the current locale cannot convert */
	if (out_len == (size_t)-1 && !dst)
		return 0;

	if (dst) {
		if (!dst_size)
			return 0;

		/* at most dst_size - 1, so the NUL below stays in the buffer */
		if (out_len)
			out_len = wcstombs(dst, str, out_len);
		if (out_len == (size_t)-1)
			out_len = 0;

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

size_t os_wcs_to_mbs_ptr(const wchar_t *str, size_t len, char **pstr)
{
	if (str) {
		size_t out_len = os_wcs_to_mbs(str, len, NULL, 0);

		*pstr = bmalloc((out_len + 1) * sizeof(char));
		return os_wcs_to_mbs(str, len, *pstr, out_len + 1);
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

size_t os_utf8_to_mbs_ptr(const char *str, size_t len, char **pstr)
{
	char *dst = NULL;
	size_t out_len = 0;

	if (str) {
		wchar_t *wstr = NULL;
		size_t wlen = os_utf8_to_wcs_ptr(str, len, &wstr);
		out_len = os_wcs_to_mbs_ptr(wstr, wlen, &dst);
		bfree(wstr);
	}
	*pstr = dst;

	return out_len;
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
