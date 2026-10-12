#include <stdio.h>
#include <string.h>
#include <sys/stat.h>

#include <util/bmem.h>
#include <util/platform.h>

#include "text-file.h"

static void remove_cr(wchar_t *source)
{
	int j = 0;
	for (int i = 0; source[i] != '\0'; ++i) {
		if (source[i] != L'\r') {
			source[j++] = source[i];
		}
	}
	source[j] = '\0';
}

/* UTF-16LE text as a NUL-terminated wide string, in a new bmalloc buffer.
 * A stray last byte and NUL units are dropped. Where wchar_t is UTF-32, surrogate pairs are
 * joined and unpaired surrogates become U+FFFD. */
static wchar_t *utf16le_to_wcs(const uint8_t *data, size_t size)
{
	const size_t units = size / 2;
	wchar_t *text = bmalloc((units + 1) * sizeof(wchar_t));
	size_t len = 0;

	for (size_t i = 0; i < units; i++) {
		uint32_t c = data[i * 2] | (uint32_t)data[i * 2 + 1] << 8;
#if WCHAR_MAX > 0xFFFF
		if (c >= 0xD800 && c < 0xDC00 && i + 1 < units) {
			const uint32_t low = data[i * 2 + 2] | (uint32_t)data[i * 2 + 3] << 8;
			if (low >= 0xDC00 && low < 0xE000) {
				c = 0x10000 + ((c - 0xD800) << 10) + (low - 0xDC00);
				i++;
			}
		}
		if (c >= 0xD800 && c < 0xE000)
			c = 0xFFFD;
#endif
		if (c != 0)
			text[len++] = (wchar_t)c;
	}

	text[len] = 0;
	return text;
}

/* `size` bytes of UTF-8 text from the current position of `file`, as a
 * wide string. NUL bytes cannot be shown and would end the string early
 * (obsproject/obs-studio#7595), so they are dropped. */
static wchar_t *read_utf8(FILE *file, size_t size)
{
	char *data = bmalloc(size + 1);
	size = fread(data, 1, size, file);

	size_t len = 0;
	for (size_t i = 0; i < size; i++) {
		if (data[i] != 0)
			data[len++] = data[i];
	}
	data[len] = 0;

	wchar_t *text = bzalloc((len + 1) * sizeof(wchar_t));
	os_utf8_to_wcs(data, len, text, len + 1);
	bfree(data);
	remove_cr(text);
	return text;
}

/* `size` bytes of UTF-16LE text from the current position of `file`. */
static wchar_t *read_utf16(FILE *file, size_t size)
{
	uint8_t *data = bmalloc(size + 1);
	size = fread(data, 1, size, file);
	wchar_t *text = utf16le_to_wcs(data, size);
	bfree(data);
	remove_cr(text);
	return text;
}

wchar_t *ft2_read_text_file(const char *filename)
{
	wchar_t *text = NULL;
	FILE *tmp_file = NULL;
	uint32_t filesize = 0;
	char *tmp_read = NULL;
	uint16_t header = 0;
	size_t bytes_read;

	tmp_file = os_fopen(filename, "rb");
	if (tmp_file == NULL)
		return NULL;
	fseek(tmp_file, 0, SEEK_END);
	filesize = (uint32_t)ftell(tmp_file);
	fseek(tmp_file, 0, SEEK_SET);
	bytes_read = fread(&header, 1, 2, tmp_file);

	if (bytes_read == 2 && header == 0xFEFF) {
		// File is already in UTF-16 format
		text = read_utf16(tmp_file, filesize - 2);

		bfree(tmp_read);
		fclose(tmp_file);

		return text;
	}

	fseek(tmp_file, 0, SEEK_SET);

	text = read_utf8(tmp_file, filesize);
	fclose(tmp_file);
	return text;
}

wchar_t *ft2_read_text_file_end(const char *filename, uint32_t log_lines)
{
	wchar_t *text = NULL;
	FILE *tmp_file = NULL;
	uint32_t filesize = 0, cur_pos = 0;
	char *tmp_read = NULL;
	uint16_t value = 0, line_breaks = 0;
	size_t bytes_read;
	char bvalue;

	bool utf16 = false;

	tmp_file = fopen(filename, "rb");
	if (tmp_file == NULL)
		return NULL;
	bytes_read = fread(&value, 1, 2, tmp_file);

	if (bytes_read == 2 && value == 0xFEFF)
		utf16 = true;

	fseek(tmp_file, 0, SEEK_END);
	filesize = (uint32_t)ftell(tmp_file);

	/* UTF-16 text is whole 2-byte units after the BOM */
	const uint32_t start = utf16 ? 2 : 0;
	if (utf16)
		filesize = start + (filesize - start) / 2 * 2;
	cur_pos = filesize;

	while (line_breaks <= log_lines && cur_pos > start) {
		if (!utf16)
			cur_pos--;
		else
			cur_pos -= 2;
		fseek(tmp_file, cur_pos, SEEK_SET);

		if (!utf16) {
			bytes_read = fread(&bvalue, 1, 1, tmp_file);
			if (bytes_read == 1 && bvalue == '\n')
				line_breaks++;
		} else {
			bytes_read = fread(&value, 1, 2, tmp_file);
			if (bytes_read == 2 && value == L'\n')
				line_breaks++;
		}
	}

	if (cur_pos != start)
		cur_pos += (utf16) ? 2 : 1;

	fseek(tmp_file, cur_pos, SEEK_SET);

	if (utf16) {
		text = read_utf16(tmp_file, filesize - cur_pos);

		bfree(tmp_read);
		fclose(tmp_file);

		return text;
	}

	text = read_utf8(tmp_file, filesize - cur_pos);
	fclose(tmp_file);
	return text;
}

static time_t get_modified_timestamp(const char *filename)
{
	struct stat stats;

	// stat is apparently terrifying and horrible, but we only call it once
	// every second at most.
	if (os_stat(filename, &stats) != 0)
		return -1;

	return stats.st_mtime;
}

enum ft2_text_file_action ft2_text_file_watch_set(struct ft2_text_file_watch *watch, const char *path, bool force,
						  uint64_t now)
{
	if (!path || !*path) {
		ft2_text_file_watch_free(watch);
		return FT2_TEXT_FILE_MISSING;
	}

	/* Keep polling a file that does not exist yet (another program may
	 * write it later), and stop polling the previous one. */
	if (!os_file_exists(path)) {
		if (!watch->path || strcmp(watch->path, path) != 0) {
			bfree(watch->path);
			watch->path = bstrdup(path);
		}
		watch->timestamp = -1;
		watch->changed = false;
		watch->last_checked = now;
		return FT2_TEXT_FILE_MISSING;
	}

	if (watch->path != NULL && strcmp(watch->path, path) == 0 && !force)
		return FT2_TEXT_FILE_UNCHANGED;

	bfree(watch->path);
	watch->path = bstrdup(path);
	watch->last_checked = now;
	return FT2_TEXT_FILE_LOAD;
}

bool ft2_text_file_watch_tick(struct ft2_text_file_watch *watch, uint64_t now)
{
	if (!watch->path || now - watch->last_checked < 1000000000)
		return false;

	time_t t = get_modified_timestamp(watch->path);
	watch->last_checked = now;

	bool reload = watch->changed;
	watch->changed = false;

	if (watch->timestamp != t) {
		watch->timestamp = t;
		watch->changed = true;
	}

	return reload;
}

void ft2_text_file_watch_free(struct ft2_text_file_watch *watch)
{
	bfree(watch->path);
	watch->path = NULL;
}
