/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "libobs/build/libobs.pch.h"

/* Caption text wrap and pagination (moved verbatim from obs-output.c.h).
 * obs_output_output_caption_text2() in obs-output.c.h, later in the same
 * unity group, calls caption_text_page(). */

static bool caption_utf8_continuation(unsigned char c)
{
	return (c & 0xC0) == 0x80;
}

/* Unlike libcaption's utf8_char_length(), this validates continuation bytes
 * and always lets the caller consume an invalid byte. */
static size_t caption_utf8_char_length(const char *text)
{
	const unsigned char *c = (const unsigned char *)text;

	if (c[0] < 0x80)
		return c[0] != 0;
	if (c[0] >= 0xC2 && c[0] <= 0xDF)
		return caption_utf8_continuation(c[1]) ? 2 : 0;
	if (c[0] >= 0xE0 && c[0] <= 0xEF) {
		if (!caption_utf8_continuation(c[1]))
			return 0;
		if ((c[0] == 0xE0 && c[1] < 0xA0) || (c[0] == 0xED && c[1] >= 0xA0))
			return 0;
		return caption_utf8_continuation(c[2]) ? 3 : 0;
	}
	if (c[0] >= 0xF0 && c[0] <= 0xF4) {
		if (!caption_utf8_continuation(c[1]))
			return 0;
		if ((c[0] == 0xF0 && c[1] < 0x90) || (c[0] == 0xF4 && c[1] >= 0x90))
			return 0;
		if (!caption_utf8_continuation(c[2]))
			return 0;
		return caption_utf8_continuation(c[3]) ? 4 : 0;
	}
	return 0;
}

static bool caption_is_newline(const char *text)
{
	return text[0] == '\r' || text[0] == '\n';
}

static bool caption_utf8_whitespace(const char *text, size_t bytes)
{
	const unsigned char *c = (const unsigned char *)text;
	return (bytes == 1 && (c[0] <= ' ' || c[0] == 0x7F)) || (bytes == 2 && c[0] == 0xC2 && c[1] == 0xA0);
}

/* Match caption_frame_from_text()/utf8_wrap_length(): leading whitespace and
 * blank lines are skipped, words wrap at the last whitespace at or before
 * column 32, and a word longer than 32 characters is hard-wrapped. */
static size_t caption_text_row(const char **text, char row[CAPTION_LINE_BYTES + 1])
{
	const char *start;
	const char *scan;
	const char *split = NULL;
	const char *hard_end = NULL;
	const char *end;
	size_t chars = 0;
	size_t bytes = 0;

	while (**text) {
		size_t len = caption_utf8_char_length(*text);
		if (!len) {
			(*text)++;
			continue;
		}
		if (!caption_utf8_whitespace(*text, len))
			break;
		*text += len;
	}

	start = scan = *text;
	if (!*scan)
		return 0;

	for (;;) {
		size_t len;

		if (!*scan) {
			end = scan;
			break;
		}
		len = caption_utf8_char_length(scan);
		if (!len) {
			scan++;
			continue;
		}
		if (caption_is_newline(scan)) {
			end = scan;
			break;
		}
		if (caption_utf8_whitespace(scan, len))
			split = scan;
		if (chars == CAPTION_LINE_CHARS) {
			end = split ? split : hard_end;
			break;
		}
		scan += len;
		chars++;
		if (chars == CAPTION_LINE_CHARS)
			hard_end = scan;
	}

	for (scan = start; scan < end;) {
		size_t len = caption_utf8_char_length(scan);
		if (!len) {
			scan++;
			continue;
		}
		/* Interior whitespace that is not a newline (tab, VT, FF) has no
		 * EIA-608 mapping; emitting a space keeps it from becoming an
		 * empty cell that ends the row early. */
		if (len == 1 && caption_utf8_whitespace(scan, 1)) {
			row[bytes++] = ' ';
			scan++;
			continue;
		}
		memcpy(row + bytes, scan, len);
		bytes += len;
		scan += len;
	}
	row[bytes] = 0;
	*text = end;
	return bytes;
}

static size_t caption_text_page(const char **text, char page[CAPTION_PAGE_BYTES + 1])
{
	size_t page_bytes = 0;

	for (size_t rows = 0; rows < CAPTION_PAGE_ROWS; rows++) {
		char *row = page + page_bytes + (rows ? 1 : 0);
		size_t row_bytes = caption_text_row(text, row);
		if (!row_bytes)
			break;
		if (rows)
			page[page_bytes++] = '\n';
		page_bytes += row_bytes;
	}
	page[page_bytes] = 0;
	return page_bytes;
}
