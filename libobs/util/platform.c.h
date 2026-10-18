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

static int recursive_mkdir(char *path)
{
	char *last_slash;
	int ret;

	ret = os_mkdir(path);
	if (ret != MKDIR_ERROR)
		return ret;

	last_slash = strrchr(path, '/');
	if (!last_slash)
		return MKDIR_ERROR;

	*last_slash = 0;
	ret = recursive_mkdir(path);
	*last_slash = '/';

	if (ret == MKDIR_ERROR)
		return MKDIR_ERROR;

	ret = os_mkdir(path);
	return ret;
}

int os_mkdirs(const char *dir)
{
	struct dstr dir_str;
	int ret;

	dstr_init_copy(&dir_str, dir);
	dstr_replace(&dir_str, "\\", "/");
	ret = recursive_mkdir(dir_str.array);
	dstr_free(&dir_str);
	return ret;
}

static inline bool valid_string(const char *str)
{
	while (str && *str) {
		if (*(str++) != ' ')
			return true;
	}

	return false;
}

static void replace_text(struct dstr *str, size_t pos, size_t len, const char *new_text)
{
	struct dstr front = {0};
	struct dstr back = {0};

	dstr_left(&front, str, pos);
	dstr_right(&back, str, pos + len);
	dstr_copy_dstr(str, &front);
	dstr_cat(str, new_text);
	dstr_cat_dstr(str, &back);
	dstr_free(&front);
	dstr_free(&back);
}

static void erase_ch(struct dstr *str, size_t pos)
{
	struct dstr new_str = {0};
	dstr_left(&new_str, str, pos);
	dstr_cat(&new_str, str->array + pos + 1);
	dstr_free(str);
	*str = new_str;
}

char *os_generate_formatted_filename(const char *extension, bool space, const char *format)
{
	time_t now = time(0);
	struct tm *cur_time;
	cur_time = localtime(&now);
	struct obs_video_info ovi;
	obs_get_video_info(&ovi);

	const size_t spec_count = 23;
	static const char *spec[][2] = {
		{"%CCYY", "%Y"}, {"%YY", "%y"}, {"%MM", "%m"}, {"%DD", "%d"}, {"%hh", "%H"},
		{"%mm", "%M"},   {"%ss", "%S"}, {"%%", "%%"},

		{"%a", ""},      {"%A", ""},    {"%b", ""},    {"%B", ""},    {"%d", ""},
		{"%H", ""},      {"%I", ""},    {"%m", ""},    {"%M", ""},    {"%p", ""},
		{"%S", ""},      {"%y", ""},    {"%Y", ""},    {"%z", ""},    {"%Z", ""},
	};

	char convert[128] = {0};
	struct dstr sf;
	struct dstr c = {0};
	size_t pos = 0;

	dstr_init_copy(&sf, format);

	while (pos < sf.len) {
		const char *cmp = sf.array + pos;
		for (size_t i = 0; i < spec_count && !convert[0]; i++) {
			size_t len = strlen(spec[i][0]);

			if (astrcmp_n(cmp, spec[i][0], len) == 0) {
				/* The strftime format comes from the spec table, which is
				 * only known at run time. */
#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wformat-nonliteral"
#endif
				if (strlen(spec[i][1]))
					strftime(convert, sizeof(convert), spec[i][1], cur_time);
				else
					strftime(convert, sizeof(convert), spec[i][0], cur_time);
#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic pop
#endif

				dstr_copy(&c, convert);
				if (c.len && valid_string(c.array))
					replace_text(&sf, pos, len, convert);
			}
		}

		if (!convert[0]) {
			if (astrcmp_n(cmp, "%FPS", 4) == 0) {
				if (ovi.fps_den <= 1) {
					snprintf(convert, sizeof(convert), "%u", ovi.fps_num);
				} else {
					const double obsFPS = (double)ovi.fps_num / (double)ovi.fps_den;
					snprintf(convert, sizeof(convert), "%.2f", obsFPS);
				}
				replace_text(&sf, pos, 4, convert);

			} else if (astrcmp_n(cmp, "%CRES", 5) == 0) {
				snprintf(convert, sizeof(convert), "%ux%u", ovi.base_width, ovi.base_height);
				replace_text(&sf, pos, 5, convert);

			} else if (astrcmp_n(cmp, "%ORES", 5) == 0) {
				snprintf(convert, sizeof(convert), "%ux%u", ovi.output_width, ovi.output_height);
				replace_text(&sf, pos, 5, convert);

			} else if (astrcmp_n(cmp, "%VF", 3) == 0) {
				strcpy(convert, get_video_format_name(ovi.output_format));
				replace_text(&sf, pos, 3, convert);

			} else if (astrcmp_n(cmp, "%s", 2) == 0) {
				snprintf(convert, sizeof(convert), "%" PRId64, (int64_t)now);
				replace_text(&sf, pos, 2, convert);
			}
		}

		if (convert[0]) {
			pos += strlen(convert);
			convert[0] = 0;
		} else if (!convert[0] && sf.array[pos] == '%') {
			erase_ch(&sf, pos);
		} else {
			pos++;
		}
	}

	if (!space)
		dstr_replace(&sf, " ", "_");

	if (extension && *extension) {
		dstr_cat_ch(&sf, '.');
		dstr_cat(&sf, extension);
	}

	dstr_free(&c);

	if (sf.len > 255) {
		/* Back up to a UTF-8 character boundary so a multibyte character
		   is not split by the 255-byte limit. A character has at most
		   three continuation bytes, so invalid input stops at 252. */
		size_t len = 255;
		while (len > 252 && ((unsigned char)sf.array[len] & 0xC0) == 0x80)
			len--;
		dstr_mid(&sf, &sf, 0, len);
	}

	return sf.array;
}

static struct {
	struct timespec ts;
	bool ts_valid;
	uint64_t timestamp;
} timespec_offset = {0};

static void init_timespec_offset(void)
{
	timespec_offset.ts_valid = timespec_get(&timespec_offset.ts, TIME_UTC) == TIME_UTC;
	timespec_offset.timestamp = os_gettime_ns();
}

struct timespec *os_nstime_to_timespec(uint64_t timestamp, struct timespec *storage)
{
	static pthread_once_t once = PTHREAD_ONCE_INIT;
	pthread_once(&once, init_timespec_offset);

	if (!storage || !timespec_offset.ts_valid) {
		return NULL;
	}

	*storage = timespec_offset.ts;

	static const int64_t nsecs_per_sec = 1000000000;

	int64_t nsecs = 0;
	int64_t secs = 0;
	if (timestamp >= timespec_offset.timestamp) {
		uint64_t offset = timestamp - timespec_offset.timestamp;
		nsecs = storage->tv_nsec + offset % nsecs_per_sec;
		secs = storage->tv_sec + offset / nsecs_per_sec;
	} else {
		uint64_t offset = timespec_offset.timestamp - timestamp;
		int64_t nsec_offset = offset % nsecs_per_sec;
		int64_t sec_offset = offset / nsecs_per_sec;
		int64_t tv_nsec = storage->tv_nsec;
		if (nsec_offset > tv_nsec) {
			storage->tv_sec -= 1;
			tv_nsec += nsecs_per_sec;
		}
		nsecs = tv_nsec - nsec_offset;
		secs = storage->tv_sec - sec_offset;
	}

	if (nsecs > nsecs_per_sec) {
		nsecs -= nsecs_per_sec;
		secs += 1;
	}
	storage->tv_nsec = (long)nsecs;
	storage->tv_sec = (time_t)secs;

	return storage;
}
