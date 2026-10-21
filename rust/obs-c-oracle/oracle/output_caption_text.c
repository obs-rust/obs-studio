/* Test-only oracle: the original libobs/obs-output-caption-text.c.h,
 * unmodified, with obs_output_caption_text_page renamed so it can link next
 * to the Rust shim, plus oracle_output_caption_text_* wrappers around its
 * static functions and the CAPTION_* bounds. */
#include <stdbool.h>
#include <stddef.h>

#define obs_output_caption_text_page oracle_obs_output_caption_text_page

#include "obs-output-caption-text.c.h"

/* The bounds, in the order of rust/obs-c-oracle/src/output_caption_text.rs. */
enum {
	ORACLE_OUTPUT_CAPTION_TEXT_VALUE_COUNT = 4,
};

size_t oracle_output_caption_text_value(size_t which);
bool oracle_output_caption_text_utf8_continuation(unsigned char c);
size_t oracle_output_caption_text_utf8_char_length(const char *text);
bool oracle_output_caption_text_is_newline(const char *text);
bool oracle_output_caption_text_utf8_whitespace(const char *text, size_t bytes);
size_t oracle_output_caption_text_row(const char **text, char *row);

size_t oracle_output_caption_text_value(size_t which)
{
	const size_t values[ORACLE_OUTPUT_CAPTION_TEXT_VALUE_COUNT] = {
		CAPTION_LINE_CHARS,
		CAPTION_PAGE_ROWS,
		CAPTION_LINE_BYTES,
		CAPTION_PAGE_BYTES,
	};
	return which < ORACLE_OUTPUT_CAPTION_TEXT_VALUE_COUNT ? values[which] : 0;
}

bool oracle_output_caption_text_utf8_continuation(unsigned char c)
{
	return caption_utf8_continuation(c);
}

size_t oracle_output_caption_text_utf8_char_length(const char *text)
{
	return caption_utf8_char_length(text);
}

bool oracle_output_caption_text_is_newline(const char *text)
{
	return caption_is_newline(text);
}

bool oracle_output_caption_text_utf8_whitespace(const char *text, size_t bytes)
{
	return caption_utf8_whitespace(text, bytes);
}

/* row has CAPTION_LINE_BYTES + 1 bytes. */
size_t oracle_output_caption_text_row(const char **text, char *row)
{
	return caption_text_row(text, row);
}
