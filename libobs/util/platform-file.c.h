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

int64_t os_fgetsize(FILE *file)
{
	int64_t cur_offset = os_ftelli64(file);
	int64_t size;
	int errval = 0;

	if (fseek(file, 0, SEEK_END) == -1)
		return -1;

	size = os_ftelli64(file);
	if (size == -1)
		errval = errno;

	if (os_fseeki64(file, cur_offset, SEEK_SET) != 0 && errval != 0)
		errno = errval;

	return size;
}

size_t os_fread_mbs(FILE *file, char **pstr)
{
	size_t size = 0;
	size_t len = 0;

	fseek(file, 0, SEEK_END);
	int64_t end = os_ftelli64(file);
	*pstr = NULL;

	/* POSIX `fopen` accepts a directory and `ftell` then reports a bogus
	 * length with no error (2^63 here), so a positive size is not proof the
	 * file can be read. Ask for one byte before trusting it: a directory (or
	 * any unreadable stream) fails the read instead of asking `bmalloc` for
	 * 2^63 bytes (obs-rust/obs-studio#1768). */
	if (end > 0) {
		fseek(file, 0, SEEK_SET);
		if (fgetc(file) == EOF)
			return len;

		size = (size_t)end;

		char *mbstr = bmalloc(size + 1);

		fseek(file, 0, SEEK_SET);
		size = fread(mbstr, 1, size, file);
		if (size == 0) {
			bfree(mbstr);
			return 0;
		}

		mbstr[size] = 0;
		len = os_mbs_to_utf8_ptr(mbstr, size, pstr);
		bfree(mbstr);
	}

	return len;
}

size_t os_fread_utf8(FILE *file, char **pstr)
{
	size_t size = 0;
	size_t len = 0;

	*pstr = NULL;

	fseek(file, 0, SEEK_END);
	/* As in `os_fread_mbs`: a directory's `ftell` reports a bogus length with
	 * no error, so probe one byte before trusting it (#1768). */
	int64_t end = os_ftelli64(file);

	if (end > 0) {
		fseek(file, 0, SEEK_SET);
		if (fgetc(file) == EOF)
			return len;

		size = (size_t)end;

		char bom[3];
		char *utf8str;
		off_t offset;

		bom[0] = 0;
		bom[1] = 0;
		bom[2] = 0;

		/* remove the ghastly BOM if present */
		fseek(file, 0, SEEK_SET);
		size_t size_read = fread(bom, 1, 3, file);
		(void)size_read;

		offset = (astrcmp_n(bom, "\xEF\xBB\xBF", 3) == 0) ? 3 : 0;

		/* `size` was measured before the BOM read, so a file that grew past
		 * the BOM in between would underflow here (#1768). */
		if (size <= (size_t)offset)
			return 0;

		size -= (size_t)offset;

		utf8str = bmalloc(size + 1);
		fseek(file, offset, SEEK_SET);

		size = fread(utf8str, 1, size, file);
		if (size == 0) {
			bfree(utf8str);
			return 0;
		}

		utf8str[size] = 0;

		*pstr = utf8str;
	}

	return len;
}

char *os_quick_read_mbs_file(const char *path)
{
	FILE *f = os_fopen(path, "rb");
	char *file_string = NULL;

	if (!f)
		return NULL;

	os_fread_mbs(f, &file_string);
	fclose(f);

	return file_string;
}

char *os_quick_read_utf8_file(const char *path)
{
	FILE *f = os_fopen(path, "rb");
	char *file_string = NULL;

	if (!f)
		return NULL;

	os_fread_utf8(f, &file_string);
	fclose(f);

	return file_string;
}

bool os_quick_write_mbs_file(const char *path, const char *str, size_t len)
{
	FILE *f = os_fopen(path, "wb");
	char *mbs = NULL;
	size_t mbs_len = 0;
	if (!f)
		return false;

	mbs_len = os_utf8_to_mbs_ptr(str, len, &mbs);
	bool success = !mbs_len || fwrite(mbs, 1, mbs_len, f) == mbs_len;
	bfree(mbs);
	if (fclose(f) != 0)
		success = false;

	return success;
}

bool os_quick_write_utf8_file(const char *path, const char *str, size_t len, bool marker)
{
	FILE *f = os_fopen(path, "wb");
	if (!f)
		return false;

	if (marker) {
		if (fwrite("\xEF\xBB\xBF", 3, 1, f) != 1) {
			fclose(f);
			return false;
		}
	}

	if (len) {
		if (fwrite(str, len, 1, f) != 1) {
			fclose(f);
			return false;
		}
	}
	return fclose(f) == 0;
}

bool os_quick_write_utf8_file_safe(const char *path, const char *str, size_t len, bool marker, const char *temp_ext,
				   const char *backup_ext)
{
	struct dstr backup_path = {0};
	struct dstr temp_path = {0};
	bool success = false;

	if (!temp_ext || !*temp_ext) {
		blog(LOG_ERROR, "os_quick_write_utf8_file_safe: invalid "
				"temporary extension specified");
		return false;
	}

	dstr_copy(&temp_path, path);
	if (*temp_ext != '.')
		dstr_cat(&temp_path, ".");
	dstr_cat(&temp_path, temp_ext);

	if (!os_quick_write_utf8_file(temp_path.array, str, len, marker)) {
		blog(LOG_ERROR,
		     "os_quick_write_utf8_file_safe: failed to "
		     "write to %s",
		     temp_path.array);
		goto cleanup;
	}

	if (backup_ext && *backup_ext) {
		dstr_copy(&backup_path, path);
		if (*backup_ext != '.')
			dstr_cat(&backup_path, ".");
		dstr_cat(&backup_path, backup_ext);
	}

	if (os_safe_replace(path, temp_path.array, backup_path.array) == 0)
		success = true;

cleanup:
	dstr_free(&backup_path);
	dstr_free(&temp_path);
	return success;
}

int64_t os_get_file_size(const char *path)
{
	FILE *f = os_fopen(path, "rb");
	if (!f)
		return -1;

	int64_t sz = os_fgetsize(f);
	fclose(f);

	return sz;
}
