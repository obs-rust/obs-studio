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

#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <stdarg.h>
#include <string.h>
#include <wchar.h>
#include <wctype.h>

#include "c99defs.h"
#include "dstr.h"
#include "bmem.h"
#include "utf8.h"
#include "platform.h"

static const wchar_t *wstrblank = L"";

int wstrcmpi(const wchar_t *str1, const wchar_t *str2)
{
	if (!str1)
		str1 = wstrblank;
	if (!str2)
		str2 = wstrblank;

	do {
		wchar_t ch1 = (wchar_t)towupper(*str1);
		wchar_t ch2 = (wchar_t)towupper(*str2);

		if (ch1 < ch2)
			return -1;
		else if (ch1 > ch2)
			return 1;
	} while (*str1++ && *str2++);

	return 0;
}

int wstrcmp_n(const wchar_t *str1, const wchar_t *str2, size_t n)
{
	if (!n)
		return 0;
	if (!str1)
		str1 = wstrblank;
	if (!str2)
		str2 = wstrblank;

	do {
		wchar_t ch1 = *str1;
		wchar_t ch2 = *str2;

		if (ch1 < ch2)
			return -1;
		else if (ch1 > ch2)
			return 1;
	} while (*str1++ && *str2++ && --n);

	return 0;
}

int wstrcmpi_n(const wchar_t *str1, const wchar_t *str2, size_t n)
{
	if (!n)
		return 0;
	if (!str1)
		str1 = wstrblank;
	if (!str2)
		str2 = wstrblank;

	do {
		wchar_t ch1 = (wchar_t)towupper(*str1);
		wchar_t ch2 = (wchar_t)towupper(*str2);

		if (ch1 < ch2)
			return -1;
		else if (ch1 > ch2)
			return 1;
	} while (*str1++ && *str2++ && --n);

	return 0;
}

wchar_t *wstrstri(const wchar_t *str, const wchar_t *find)
{
	size_t len;

	if (!str || !find)
		return NULL;

	len = wcslen(find);

	do {
		if (wstrcmpi_n(str, find, len) == 0)
			return (wchar_t *)str;
	} while (*str++);

	return NULL;
}

static inline bool is_padding(int ch)
{
	return ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r';
}

wchar_t *wcsdepad(wchar_t *str)
{
	wchar_t *temp;
	size_t len;

	if (!str)
		return str;
	if (!*str)
		return str;

	temp = str;

	/* remove preceding spaces/tabs */
	while (is_padding(*temp))
		++temp;

	len = wcslen(temp);
	if (temp != str)
		memmove(str, temp, (len + 1) * sizeof(wchar_t));

	if (len) {
		temp = str + (len - 1);
		while (is_padding(*temp))
			*(temp--) = 0;
	}

	return str;
}

void dstr_printf(struct dstr *dst, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	dstr_vprintf(dst, format, args);
	va_end(args);
}

void dstr_catf(struct dstr *dst, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	dstr_vcatf(dst, format, args);
	va_end(args);
}

void dstr_vprintf(struct dstr *dst, const char *format, va_list args)
{
	va_list args_cp;
	va_copy(args_cp, args);

	int len = vsnprintf(NULL, 0, format, args_cp);
	va_end(args_cp);

	if (len < 0)
		len = 4095;

	dstr_ensure_capacity(dst, ((size_t)len) + 1);
	len = vsnprintf(dst->array, ((size_t)len) + 1, format, args);

	if (!*dst->array) {
		dstr_free(dst);
		return;
	}

	dst->len = len < 0 ? strlen(dst->array) : (size_t)len;
}

void dstr_vcatf(struct dstr *dst, const char *format, va_list args)
{
	va_list args_cp;
	va_copy(args_cp, args);

	int len = vsnprintf(NULL, 0, format, args_cp);
	va_end(args_cp);

	if (len < 0)
		len = 4095;

	dstr_ensure_capacity(dst, dst->len + ((size_t)len) + 1);
	len = vsnprintf(dst->array + dst->len, ((size_t)len) + 1, format, args);

	if (!*dst->array) {
		dstr_free(dst);
		return;
	}

	dst->len += len < 0 ? strlen(dst->array + dst->len) : (size_t)len;
}

void dstr_from_mbs(struct dstr *dst, const char *mbstr)
{
	dstr_free(dst);
	dst->len = os_mbs_to_utf8_ptr(mbstr, 0, &dst->array);
}

char *dstr_to_mbs(const struct dstr *str)
{
	char *dst;
	os_mbs_to_utf8_ptr(str->array, str->len, &dst);
	return dst;
}

wchar_t *dstr_to_wcs(const struct dstr *str)
{
	wchar_t *dst;
	os_utf8_to_wcs_ptr(str->array, str->len, &dst);
	return dst;
}

void dstr_from_wcs(struct dstr *dst, const wchar_t *wstr)
{
	size_t len = wchar_to_utf8(wstr, 0, NULL, 0, 0);

	if (len) {
		dstr_resize(dst, len);
		wchar_to_utf8(wstr, 0, dst->array, len + 1, 0);
	} else {
		dstr_free(dst);
	}
}

void dstr_to_upper(struct dstr *str)
{
	wchar_t *wstr;
	wchar_t *temp;

	if (dstr_is_empty(str))
		return;

	wstr = dstr_to_wcs(str);
	temp = wstr;

	if (!wstr)
		return;

	while (*temp) {
		*temp = (wchar_t)towupper(*temp);
		temp++;
	}

	dstr_from_wcs(str, wstr);
	bfree(wstr);
}

void dstr_to_lower(struct dstr *str)
{
	wchar_t *wstr;
	wchar_t *temp;

	if (dstr_is_empty(str))
		return;

	wstr = dstr_to_wcs(str);
	temp = wstr;

	if (!wstr)
		return;

	while (*temp) {
		*temp = (wchar_t)towlower(*temp);
		temp++;
	}

	dstr_from_wcs(str, wstr);
	bfree(wstr);
}
