/* obs_output_output_caption_text2() silently truncated every caption at
 * CAPTION_LINE_BYTES (128) bytes: caption_text_new() stored text in a fixed
 * 129-byte array, and the "split text into 32 character strings" comment was
 * never implemented. A caption longer than 128 bytes lost its tail, and a
 * multibyte UTF-8 character straddling byte 128 was cut in half, leaving a
 * truncated sequence in the queued text.
 *
 * The fix implements the comment: text is word-wrapped at 32 columns and
 * paginated at 15 rows, each page enqueued as its own caption_text node with
 * the call's display_duration.
 *
 * The test registers a minimal video encoder and output, sets a video
 * encoder (which creates the caption track) and marks the output active so
 * the public API enqueues text. It then walks output->caption_tracks[0]'s
 * caption_text queue through obs-internal.h and checks the pages. No video
 * backend or encoder start is needed, so the test runs on every platform
 * that can obs_startup().
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/threading.h>

#include "obs-internal.h"

static int obs_up;

/* ------------------------------------------------------------------------- */

static const char *enc_name(void *type_data)
{
	(void)type_data;
	return "test encoder";
}

static void *enc_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	return encoder;
}

static void enc_destroy(void *data)
{
	(void)data;
}

static bool enc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received_packet)
{
	(void)data;
	(void)frame;
	(void)packet;
	*received_packet = false;
	return true;
}

static const char *out_name(void *type_data)
{
	(void)type_data;
	return "test output";
}

static void *out_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	return output;
}

static void out_destroy(void *data)
{
	(void)data;
}

static bool out_start(void *data)
{
	(void)data;
	return false;
}

static void out_stop(void *data, uint64_t ts)
{
	(void)data;
	(void)ts;
}

static void out_packet(void *data, struct encoder_packet *packet)
{
	(void)data;
	(void)packet;
}

static void register_types(void)
{
	struct obs_encoder_info enc_info = {
		.id = "test_caption_encoder",
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = enc_name,
		.create = enc_create,
		.destroy = enc_destroy,
		.encode = enc_encode,
	};
	obs_register_encoder(&enc_info);

	struct obs_output_info out_info = {
		.id = "test_caption_output",
		.flags = OBS_OUTPUT_ENCODED | OBS_OUTPUT_VIDEO,
		.get_name = out_name,
		.create = out_create,
		.destroy = out_destroy,
		.start = out_start,
		.stop = out_stop,
		.encoded_packet = out_packet,
	};
	obs_register_output(&out_info);
}

/* ------------------------------------------------------------------------- */

struct caption_harness {
	obs_output_t *output;
	obs_encoder_t *encoder;
	struct caption_harness *next_live;
};

/* Harnesses not yet destroyed. A failed assertion leaves its case before
 * harness_destroy(), and an output still marked active would block
 * obs_shutdown(), so main() destroys these first. */
static struct caption_harness *live_harnesses;

static struct caption_harness *harness_create(void)
{
	struct caption_harness *h = bzalloc(sizeof(*h));
	h->output = obs_output_create("test_caption_output", "test", NULL, NULL);
	h->encoder = obs_video_encoder_create("test_caption_encoder", "test enc", NULL, NULL);
	assert_non_null(h->output);
	assert_non_null(h->encoder);

	obs_output_set_video_encoder(h->output, h->encoder);
	assert_non_null(h->output->caption_tracks[0]);

	/* begin_data_capture would need a running core video; setting active
	 * is enough for obs_output_output_caption_text2() */
	os_atomic_set_bool(&h->output->active, true);
	h->next_live = live_harnesses;
	live_harnesses = h;
	return h;
}

static void harness_destroy(struct caption_harness *h)
{
	struct caption_track_data *ctrack = h->output->caption_tracks[0];

	for (struct caption_harness **link = &live_harnesses; *link; link = &(*link)->next_live) {
		if (*link == h) {
			*link = h->next_live;
			break;
		}
	}

	pthread_mutex_lock(&ctrack->caption_mutex);
	struct caption_text *node = ctrack->caption_head;
	while (node) {
		struct caption_text *next = node->next;
		bfree(node);
		node = next;
	}
	ctrack->caption_head = NULL;
	ctrack->caption_tail = NULL;
	pthread_mutex_unlock(&ctrack->caption_mutex);

	os_atomic_set_bool(&h->output->active, false);
	obs_encoder_release(h->encoder);
	obs_output_release(h->output);
	bfree(h);
}

/* Collect the queued pages: returns the node count, fills `texts`/`durs`. */
static size_t collect_pages(struct caption_harness *h, const char **texts, double *durs, size_t max)
{
	struct caption_track_data *ctrack = h->output->caption_tracks[0];
	size_t count = 0;

	pthread_mutex_lock(&ctrack->caption_mutex);
	for (struct caption_text *node = ctrack->caption_head; node && count < max; node = node->next) {
		texts[count] = node->text;
		durs[count] = node->display_duration;
		count++;
	}
	pthread_mutex_unlock(&ctrack->caption_mutex);
	return count;
}

/* Every queued page must contain valid UTF-8: continuation bytes only after
 * a valid lead byte, and no truncated sequence at the end. */
static void assert_valid_utf8(const char *text)
{
	const unsigned char *c = (const unsigned char *)text;
	while (*c) {
		size_t len = 0;
		if (*c < 0x80)
			len = 1;
		else if ((*c & 0xE0) == 0xC0)
			len = 2;
		else if ((*c & 0xF0) == 0xE0)
			len = 3;
		else if ((*c & 0xF8) == 0xF0)
			len = 4;
		assert_true(len > 0);
		for (size_t i = 1; i < len; i++)
			assert_true((c[i] & 0xC0) == 0x80);
		c += len;
	}
}

/* Concatenate the pages, dropping the '\n' row separators, and compare
 * against the input text with its whitespace runs collapsed to single
 * spaces/newlines the same way the wrapper treats them. The call sites here
 * only need "every non-newline input byte survived, in order". */
static void assert_pages_preserve(struct caption_harness *h, const char *input)
{
	const char *texts[8];
	double durs[8];
	size_t pages = collect_pages(h, texts, durs, 8);

	size_t capacity = strlen(input) + 1;
	char *joined = bzalloc(capacity);
	size_t out = 0;
	for (size_t i = 0; i < pages; i++) {
		assert_valid_utf8(texts[i]);
		for (const char *c = texts[i]; *c && out < capacity - 1; c++) {
			if (*c != '\n' && *c != ' ' && *c != '\r' && *c != '\t')
				joined[out++] = *c;
		}
	}

	/* the inputs below use single-space-separated words, so the wrapper
	 * drops no input characters: removing whitespace from the input must
	 * reproduce the joined pages exactly */
	char *expected = bzalloc(capacity);
	size_t exp_len = 0;
	for (const char *c = input; *c; c++) {
		if (*c != '\n' && *c != ' ' && *c != '\r' && *c != '\t')
			expected[exp_len++] = *c;
	}
	assert_string_equal(expected, joined);
	bfree(expected);
	bfree(joined);
}

/* ------------------------------------------------------------------------- */

static char *repeat_char(size_t n, char c)
{
	char *s = bzalloc(n + 1);
	memset(s, c, n);
	return s;
}

static void caption_boundary_127(void **state)
{
	(void)state;
	struct caption_harness *h = harness_create();
	char *input = repeat_char(127, 'a');

	obs_output_output_caption_text2(h->output, input, 2.0);
	assert_pages_preserve(h, input);

	bfree(input);
	harness_destroy(h);
}

static void caption_boundary_128(void **state)
{
	(void)state;
	struct caption_harness *h = harness_create();
	char *input = repeat_char(128, 'a');

	obs_output_output_caption_text2(h->output, input, 2.0);
	assert_pages_preserve(h, input);

	bfree(input);
	harness_destroy(h);
}

static void caption_boundary_129(void **state)
{
	(void)state;
	struct caption_harness *h = harness_create();
	char *input = repeat_char(129, 'a');

	obs_output_output_caption_text2(h->output, input, 2.0);
	assert_pages_preserve(h, input);

	bfree(input);
	harness_destroy(h);
}

static void caption_multibyte_at_128(void **state)
{
	(void)state;
	struct caption_harness *h = harness_create();

	/* 127 'a' bytes, then U+00E9 (2 bytes) at bytes 128-129, then a tail:
	 * the old fixed 128-byte buffer cut the character in half. */
	char *input = bzalloc(256);
	memset(input, 'a', 127);
	strcat(input, "\xC3\xA9");
	strcat(input, "tail");

	obs_output_output_caption_text2(h->output, input, 2.0);
	assert_pages_preserve(h, input);

	const char *texts[8];
	double durs[8];
	size_t pages = collect_pages(h, texts, durs, 8);
	assert_true(pages > 0);
	/* the last row carries the tail that the 128-byte cap used to cut off */
	assert_non_null(strstr(texts[pages - 1], "tail"));

	bfree(input);
	harness_destroy(h);
}

static void caption_multi_page(void **state)
{
	(void)state;
	struct caption_harness *h = harness_create();

	/* 40 rows worth of text: more than the 15-row page, so the queue must
	 * hold ceil(40/15) = 3 pages, each with the call's duration */
	char input[2048] = {0};
	for (int i = 0; i < 40; i++) {
		char row[40];
		snprintf(row, sizeof(row), "row%02d-abcdefghijklmnopqrstuvwx", i);
		if (i)
			strcat(input, " ");
		strcat(input, row);
	}

	obs_output_output_caption_text2(h->output, input, 3.5);

	const char *texts[8];
	double durs[8];
	size_t pages = collect_pages(h, texts, durs, 8);
	assert_int_equal(3, pages);
	for (size_t i = 0; i < pages; i++)
		assert_true(durs[i] == 3.5);

	/* every input row must appear, in order, on the expected page:
	 * rows 0-14 on page 0, 15-29 on page 1, 30-39 on page 2 */
	for (int i = 0; i < 40; i++) {
		char marker[8];
		snprintf(marker, sizeof(marker), "row%02d", i);
		size_t expected_page = (size_t)i / 15;
		for (size_t p = 0; p < pages; p++) {
			if (p == expected_page)
				assert_non_null(strstr(texts[p], marker));
			else
				assert_null(strstr(texts[p], marker));
		}
	}

	harness_destroy(h);
}

static void caption_word_wrap(void **state)
{
	(void)state;
	struct caption_harness *h = harness_create();

	/* words wrap at column 32: "wrapped" (7) does not fit after
	 * "aaaaaaaaaaaaaaaaaaaaaaaaa" (25) + a space, so it moves to row 2 */
	const char *input = "aaaaaaaaaaaaaaaaaaaaaaaaa wrapped";
	obs_output_output_caption_text2(h->output, input, 2.0);

	const char *texts[8];
	double durs[8];
	size_t pages = collect_pages(h, texts, durs, 8);
	assert_int_equal(1, pages);
	assert_string_equal("aaaaaaaaaaaaaaaaaaaaaaaaa\nwrapped", texts[0]);

	harness_destroy(h);
}

/* 15 rows x 32 columns = 480 columns exactly fills one page. */
static void caption_page_edge_480(void **state)
{
	(void)state;
	struct caption_harness *h = harness_create();
	char *input = repeat_char(480, 'a');

	obs_output_output_caption_text2(h->output, input, 2.0);
	assert_pages_preserve(h, input);

	const char *texts[8];
	double durs[8];
	assert_int_equal(1, collect_pages(h, texts, durs, 8));

	bfree(input);
	harness_destroy(h);
}

/* 481 columns spill one character onto a second page. */
static void caption_page_edge_481(void **state)
{
	(void)state;
	struct caption_harness *h = harness_create();
	char *input = repeat_char(481, 'a');

	obs_output_output_caption_text2(h->output, input, 2.0);
	assert_pages_preserve(h, input);

	const char *texts[8];
	double durs[8];
	size_t pages = collect_pages(h, texts, durs, 8);
	assert_int_equal(2, pages);
	assert_string_equal("a", texts[1]);

	bfree(input);
	harness_destroy(h);
}

/* A 3-byte UTF-8 character filling column 32 must not split mid-sequence. */
static void caption_utf8_3byte_at_edge(void **state)
{
	(void)state;
	struct caption_harness *h = harness_create();

	/* 31 'a' then 中 (U+4E2D, 3 bytes) as character 32, then 'b' wraps. */
	char *input = bzalloc(64);
	memset(input, 'a', 31);
	strcat(input, "\xE4\xB8\xAD");
	strcat(input, "b");

	obs_output_output_caption_text2(h->output, input, 2.0);
	assert_pages_preserve(h, input);

	const char *texts[8];
	double durs[8];
	size_t pages = collect_pages(h, texts, durs, 8);
	assert_int_equal(1, pages);
	/* row 1 = 31 a's + 中, row 2 = 'b' */
	assert_non_null(strstr(texts[0], "\xE4\xB8\xAD\nb"));

	bfree(input);
	harness_destroy(h);
}

/* A 4-byte UTF-8 character filling column 32 must not split mid-sequence. */
static void caption_utf8_4byte_at_edge(void **state)
{
	(void)state;
	struct caption_harness *h = harness_create();

	char *input = bzalloc(64);
	memset(input, 'a', 31);
	strcat(input, "\xF0\x9F\xA6\x84");
	strcat(input, "b");

	obs_output_output_caption_text2(h->output, input, 2.0);
	assert_pages_preserve(h, input);

	const char *texts[8];
	double durs[8];
	size_t pages = collect_pages(h, texts, durs, 8);
	assert_int_equal(1, pages);
	assert_non_null(strstr(texts[0], "\xF0\x9F\xA6\x84\nb"));

	bfree(input);
	harness_destroy(h);
}

/* Whitespace-only input enqueues the historical single empty page. */
static void caption_whitespace_only(void **state)
{
	(void)state;
	struct caption_harness *h = harness_create();

	obs_output_output_caption_text2(h->output, "   \t\n  ", 2.0);

	const char *texts[8];
	double durs[8];
	size_t pages = collect_pages(h, texts, durs, 8);
	assert_int_equal(1, pages);
	assert_string_equal("", texts[0]);
	assert_true(durs[0] == 2.0);

	harness_destroy(h);
}

/* A tab is interior whitespace: it becomes a space in the row so the text
 * after it is not lost to an unmappable-cell row cutoff. */
static void caption_tab_becomes_space(void **state)
{
	(void)state;
	struct caption_harness *h = harness_create();

	obs_output_output_caption_text2(h->output, "hello\tworld", 2.0);

	const char *texts[8];
	double durs[8];
	size_t pages = collect_pages(h, texts, durs, 8);
	assert_int_equal(1, pages);
	assert_string_equal("hello world", texts[0]);

	harness_destroy(h);
}

/* ------------------------------------------------------------------------- */
/* Characterization of the wrap and pagination in obs-output-caption-text.c
 * before its Rust port. These pin what the C does today. Cases marked
 * "characterized, not endorsed" describe behaviour that is kept as-is but is
 * not a design statement. */

/* Queue one caption and expect exactly one page with this text. */
static void assert_single_page(const char *input, const char *expected)
{
	struct caption_harness *h = harness_create();

	obs_output_output_caption_text2(h->output, input, 2.0);

	const char *texts[8];
	double durs[8];
	assert_int_equal(1, collect_pages(h, texts, durs, 8));
	assert_string_equal(expected, texts[0]);

	harness_destroy(h);
}

/* The bounds every queued page keeps whatever the input: at most
 * CAPTION_PAGE_BYTES bytes, 1-15 rows joined by '\n', each row 1 to
 * CAPTION_LINE_BYTES bytes and at most CAPTION_LINE_CHARS characters,
 * starting with a non-whitespace character, with no CR or LF inside, and
 * well-formed UTF-8. Only a sole page may be empty. Returns the page count. */
static size_t assert_page_invariants(struct caption_harness *h)
{
	struct caption_track_data *ctrack = h->output->caption_tracks[0];
	const char **texts = NULL;
	size_t pages = 0;

	/* Collect under the lock and check after it: a failed assertion must
	 * not leave caption_mutex held for obs_shutdown(). The nodes live until
	 * harness_destroy(). */
	pthread_mutex_lock(&ctrack->caption_mutex);
	for (struct caption_text *node = ctrack->caption_head; node; node = node->next) {
		texts = brealloc(texts, (pages + 1) * sizeof(*texts));
		texts[pages++] = node->text;
	}
	pthread_mutex_unlock(&ctrack->caption_mutex);

	for (size_t p = 0; p < pages; p++) {
		const char *page = texts[p];
		size_t page_len = strlen(page);

		assert_true(page_len <= CAPTION_PAGE_BYTES);
		assert_valid_utf8(page);
		if (!page_len) {
			assert_int_equal(1, pages);
			continue;
		}

		size_t rows = 0;
		const char *row = page;
		for (;;) {
			const char *nl = strchr(row, '\n');
			size_t row_len = nl ? (size_t)(nl - row) : strlen(row);
			size_t chars = 0;

			rows++;
			assert_true(row_len >= 1);
			assert_true(row_len <= CAPTION_LINE_BYTES);
			assert_true((unsigned char)row[0] > ' ' && (unsigned char)row[0] != 0x7F);
			assert_false(row_len >= 2 && (unsigned char)row[0] == 0xC2 && (unsigned char)row[1] == 0xA0);
			for (size_t i = 0; i < row_len; i++) {
				assert_true(row[i] != '\r');
				if (((unsigned char)row[i] & 0xC0) != 0x80)
					chars++;
			}
			assert_true(chars <= CAPTION_LINE_CHARS);
			if (!nl)
				break;
			row = nl + 1;
		}
		assert_true(rows <= CAPTION_PAGE_ROWS);
	}
	bfree(texts);
	return pages;
}

/* Each invalid UTF-8 form is dropped one byte at a time, and the bytes after
 * the bad lead are judged on their own. Valid characters on either side are
 * kept, and a dropped byte takes no column. */
static void caption_invalid_utf8_dropped(void **state)
{
	(void)state;
	/* a lone continuation byte */
	assert_single_page("a\x80"
			   "b",
			   "ab");
	/* C0 and C1 overlong leads */
	assert_single_page("a\xC0\xAF"
			   "b\xC1\xBF"
			   "c",
			   "abc");
	/* E0 80-9F: overlong 3-byte forms */
	assert_single_page("a\xE0\x80\xAF"
			   "b\xE0\x9F\xBF"
			   "c",
			   "abc");
	/* ED A0-BF: UTF-16 surrogates */
	assert_single_page("a\xED\xA0\x80"
			   "b\xED\xBF\xBF"
			   "c",
			   "abc");
	/* F0 80-8F: overlong 4-byte forms */
	assert_single_page("a\xF0\x8F\xBF\xBF"
			   "b",
			   "ab");
	/* F4 90 and above: past U+10FFFF */
	assert_single_page("a\xF4\x90\x80\x80"
			   "b",
			   "ab");
	/* F5-FF never start a character */
	assert_single_page("a\xF5\x80\x80\x80"
			   "b\xF8\x80"
			   "c\xFE\xFF"
			   "d",
			   "abcd");
	/* sequences truncated by the end of the string */
	assert_single_page("ab\xE4\xB8", "ab");
	assert_single_page("ab\xF0\x9F\xA6", "ab");
	assert_single_page("ab\xC3", "ab");
	/* a sequence truncated by an ASCII byte */
	assert_single_page("a\xE4\xB8"
			   "b",
			   "ab");
	/* the valid edges of each range are kept: U+D7FF, U+10FFFF, U+0800,
	 * U+10000, U+0080 */
	assert_single_page("\xED\x9F\xBF|\xF4\x8F\xBF\xBF|\xE0\xA0\x80|\xF0\x90\x80\x80|\xC2\x80",
			   "\xED\x9F\xBF|\xF4\x8F\xBF\xBF|\xE0\xA0\x80|\xF0\x90\x80\x80|\xC2\x80");
}

/* Characterized, not endorsed: an invalid byte is dropped, not replaced, so
 * the words on either side of it join. */
static void caption_invalid_byte_joins_words(void **state)
{
	(void)state;
	assert_single_page("one\xFFtwo", "onetwo");
}

/* Invalid bytes take no column: 32 4-byte characters, each followed by a
 * lone continuation byte, still fill one 128-byte row. */
static void caption_invalid_bytes_take_no_column(void **state)
{
	(void)state;
	char input[512] = {0};
	char expected[256] = {0};
	for (int i = 0; i < 32; i++) {
		strcat(input, "\xF0\x9F\xA6\x84\x80");
		strcat(expected, "\xF0\x9F\xA6\x84");
	}
	strcat(input, "z");
	strcat(expected, "\nz");
	assert_single_page(input, expected);
}

/* A string of only invalid bytes queues the single empty page, like "". */
static void caption_only_invalid_bytes(void **state)
{
	(void)state;
	assert_single_page("\xFF\xFE\x80\xC0", "");
	assert_single_page("\xE4\xB8", "");
}

/* The empty string queues one empty page. */
static void caption_empty_string(void **state)
{
	(void)state;
	assert_single_page("", "");
}

/* Only spaces, short or longer than a row, queue the single empty page. */
static void caption_all_spaces(void **state)
{
	(void)state;
	char *spaces = repeat_char(100, ' ');

	assert_single_page(" ", "");
	assert_single_page("        ", "");
	assert_single_page(spaces, "");

	bfree(spaces);
}

/* CR, LF and CRLF each end a row. */
static void caption_newlines_end_rows(void **state)
{
	(void)state;
	assert_single_page("one\rtwo\nthree\r\nfour", "one\ntwo\nthree\nfour");
}

/* Characterized, not endorsed: a newline is skipped as leading whitespace, so
 * blank lines (and leading and trailing newlines) give no empty rows. */
static void caption_blank_lines_collapse(void **state)
{
	(void)state;
	assert_single_page("four\n\n\nfive\r\n\r\nsix", "four\nfive\nsix");
	assert_single_page("\r\n\nhello\n\r", "hello");
}

/* Characterized, not endorsed: whitespace before a newline or the end of the
 * text stays in the row; only a wrap drops the whitespace it splits at. */
static void caption_trailing_whitespace_kept(void **state)
{
	(void)state;
	assert_single_page("hello world \nnext ", "hello world \nnext ");
	assert_single_page("end\t", "end ");
}

/* NBSP (U+00A0) is a split point and is skipped as leading whitespace, but
 * inside a row it is kept as its two bytes, not turned into a space. */
static void caption_nbsp(void **state)
{
	(void)state;
	char input[64] = {0};
	char expected[64] = {0};

	assert_single_page("aaaa\xC2\xA0"
			   "bbbb",
			   "aaaa\xC2\xA0"
			   "bbbb");
	assert_single_page("\xC2\xA0\xC2\xA0hi", "hi");

	/* the wrap splits at the NBSP and drops it */
	memset(input, 'a', 30);
	strcat(input, "\xC2\xA0"
		      "bbbbb");
	memset(expected, 'a', 30);
	strcat(expected, "\nbbbbb");
	assert_single_page(input, expected);

	/* an NBSP as character 33 ends a full row */
	memset(input, 0, sizeof(input));
	memset(expected, 0, sizeof(expected));
	memset(input, 'a', 32);
	strcat(input, "\xC2\xA0"
		      "b");
	memset(expected, 'a', 32);
	strcat(expected, "\nb");
	assert_single_page(input, expected);
}

/* Characterized, not endorsed: every C0 control byte and DEL counts as
 * whitespace, so inside a row each one becomes a space. */
static void caption_controls_become_spaces(void **state)
{
	(void)state;
	assert_single_page("a\x7F"
			   "b\x01"
			   "c\x1F"
			   "d\x0B"
			   "e\x0C"
			   "f",
			   "a b c d e f");
	assert_single_page("\x7F\x01lead", "lead");
}

/* A word of exactly 32 characters fills one row; with a space after it, the
 * next word starts the next row. */
static void caption_word_of_32(void **state)
{
	(void)state;
	char *word = repeat_char(32, 'x');
	char input[64];
	char expected[64];

	assert_single_page(word, word);

	snprintf(input, sizeof(input), "%s next", word);
	snprintf(expected, sizeof(expected), "%s\nnext", word);
	assert_single_page(input, expected);

	bfree(word);
}

/* A word of 33 characters is hard-wrapped after character 32. After a short
 * word it moves to its own row first, then wraps. */
static void caption_word_of_33(void **state)
{
	(void)state;
	char *word32 = repeat_char(32, 'x');
	char *word33 = repeat_char(33, 'x');
	char input[64];
	char expected[64];

	snprintf(expected, sizeof(expected), "%s\nx", word32);
	assert_single_page(word33, expected);

	snprintf(input, sizeof(input), "ab %s", word33);
	snprintf(expected, sizeof(expected), "ab\n%s\nx", word32);
	assert_single_page(input, expected);

	bfree(word33);
	bfree(word32);
}

/* A space as character 32 is dropped by the wrap after it. */
static void caption_space_at_column_32(void **state)
{
	(void)state;
	char *word31 = repeat_char(31, 'a');
	char input[64];
	char expected[64];

	snprintf(input, sizeof(input), "%s b", word31);
	snprintf(expected, sizeof(expected), "%s\nb", word31);
	assert_single_page(input, expected);

	bfree(word31);
}

/* Characterized, not endorsed: with whitespace as characters 32 and 33, the
 * wrap splits at the later one, so the row ends with a space. */
static void caption_two_spaces_at_column_32(void **state)
{
	(void)state;
	char *word31 = repeat_char(31, 'a');
	char input[64];
	char expected[64];

	snprintf(input, sizeof(input), "%s  b", word31);
	snprintf(expected, sizeof(expected), "%s \nb", word31);
	assert_single_page(input, expected);

	bfree(word31);
}

/* A 2-byte character as character 32 stays on the row, and one as character
 * 33 starts the next row, whole. */
static void caption_utf8_2byte_at_wrap(void **state)
{
	(void)state;
	char input[64] = {0};
	char expected[64] = {0};

	memset(input, 'a', 31);
	strcat(input, "\xC3\xA9\xC3\xA9");
	memset(expected, 'a', 31);
	strcat(expected, "\xC3\xA9\n\xC3\xA9");
	assert_single_page(input, expected);
}

/* 32 4-byte characters make the largest row, CAPTION_LINE_BYTES bytes, and a
 * page of 15 of them is CAPTION_PAGE_BYTES exactly. One more character
 * starts a second page. */
static void caption_row_and_page_byte_bound(void **state)
{
	(void)state;
	struct caption_harness *h = harness_create();
	size_t chars = CAPTION_PAGE_ROWS * CAPTION_LINE_CHARS;
	char *input = bzalloc(chars * 4 + 2);

	for (size_t i = 0; i < chars; i++)
		memcpy(input + i * 4, "\xF0\x9F\xA6\x84", 4);
	obs_output_output_caption_text2(h->output, input, 2.0);

	const char *texts[8];
	double durs[8];
	assert_int_equal(1, collect_pages(h, texts, durs, 8));
	assert_int_equal(CAPTION_PAGE_BYTES, strlen(texts[0]));
	assert_int_equal(1, assert_page_invariants(h));
	harness_destroy(h);

	h = harness_create();
	input[chars * 4] = 'z';
	obs_output_output_caption_text2(h->output, input, 2.0);
	assert_int_equal(2, collect_pages(h, texts, durs, 8));
	assert_int_equal(CAPTION_PAGE_BYTES, strlen(texts[0]));
	assert_string_equal("z", texts[1]);
	assert_int_equal(2, assert_page_invariants(h));
	harness_destroy(h);

	bfree(input);
}

/* 16 short lines: 15 rows on the first page, the 16th on a second page. */
static void caption_more_than_15_rows(void **state)
{
	(void)state;
	struct caption_harness *h = harness_create();
	char input[256] = {0};
	char expected[256] = {0};

	for (int i = 0; i < 16; i++) {
		char line[8];
		snprintf(line, sizeof(line), "%sl%02d", i ? "\n" : "", i);
		strcat(input, line);
		if (i < 15)
			strcat(expected, line);
	}
	obs_output_output_caption_text2(h->output, input, 2.0);

	const char *texts[8];
	double durs[8];
	assert_int_equal(2, collect_pages(h, texts, durs, 8));
	assert_string_equal(expected, texts[0]);
	assert_string_equal("l15", texts[1]);

	harness_destroy(h);
}

/* Whitespace and invalid bytes after a full page queue no empty second page. */
static void caption_full_page_then_whitespace(void **state)
{
	(void)state;
	struct caption_harness *h = harness_create();
	char *input = bzalloc(1024);

	memset(input, 'a', CAPTION_PAGE_ROWS * CAPTION_LINE_CHARS);
	strcat(input, "  \r\n\t\xC2\xA0\xFF\x80 ");
	obs_output_output_caption_text2(h->output, input, 2.0);

	const char *texts[8];
	double durs[8];
	assert_int_equal(1, collect_pages(h, texts, durs, 8));
	assert_int_equal(CAPTION_PAGE_ROWS * CAPTION_LINE_CHARS + CAPTION_PAGE_ROWS - 1, strlen(texts[0]));

	bfree(input);
	harness_destroy(h);
}

/* Any input ends: the cursor always moves forward, so the queueing loop
 * finishes, and every page keeps the bounds above. The inputs are every byte
 * value, runs of each lead byte, and pseudo-random bytes weighted toward
 * whitespace, newlines, multibyte leads and continuation bytes. */
static void caption_any_input_ends(void **state)
{
	(void)state;
	static const unsigned char pool[] = {' ',  '\t', '\r', '\n', 0x7F, 0x01, 'a',  'b',  0x80, 0xBF,
					     0xA0, 0xC2, 0xC3, 0xC0, 0xC1, 0xE0, 0xE4, 0xED, 0xEF, 0xF0,
					     0xF4, 0xF5, 0xFF, 0x9F, 0x90, 0x8F, 0xB8, 0xAD, 0xA6, 0x84};
	size_t len = 4096;
	char *input = bzalloc(len + 1);
	uint32_t seed = 0x2545F491;

	/* every byte value 1-255, in order */
	for (size_t i = 0; i < 255; i++)
		input[i] = (char)(i + 1);
	input[255] = 0;
	struct caption_harness *h = harness_create();
	obs_output_output_caption_text2(h->output, input, 2.0);
	assert_true(assert_page_invariants(h) >= 1);
	harness_destroy(h);

	/* a long run of each lead byte, with and without a continuation */
	for (unsigned int lead = 0x80; lead <= 0xFF; lead++) {
		for (size_t i = 0; i < len; i++)
			input[i] = (char)((i & 1) ? 0x80 : lead);
		input[len] = 0;
		h = harness_create();
		obs_output_output_caption_text2(h->output, input, 2.0);
		assert_true(assert_page_invariants(h) >= 1);
		harness_destroy(h);
	}

	/* pseudo-random strings of several lengths */
	for (int round = 0; round < 64; round++) {
		size_t n = 1 + (size_t)round * 61;
		for (size_t i = 0; i < n; i++) {
			seed = seed * 1103515245u + 12345u;
			input[i] = (char)pool[(seed >> 16) % sizeof(pool)];
		}
		input[n] = 0;
		h = harness_create();
		obs_output_output_caption_text2(h->output, input, 2.0);
		assert_true(assert_page_invariants(h) >= 1);
		harness_destroy(h);
	}

	bfree(input);
}

int main(void)
{
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		puts("obs_startup failed, skipping");
		return 0;
	}
	register_types();

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(caption_boundary_127),
		cmocka_unit_test(caption_boundary_128),
		cmocka_unit_test(caption_boundary_129),
		cmocka_unit_test(caption_multibyte_at_128),
		cmocka_unit_test(caption_multi_page),
		cmocka_unit_test(caption_word_wrap),
		cmocka_unit_test(caption_page_edge_480),
		cmocka_unit_test(caption_page_edge_481),
		cmocka_unit_test(caption_utf8_3byte_at_edge),
		cmocka_unit_test(caption_utf8_4byte_at_edge),
		cmocka_unit_test(caption_whitespace_only),
		cmocka_unit_test(caption_tab_becomes_space),
		cmocka_unit_test(caption_invalid_utf8_dropped),
		cmocka_unit_test(caption_invalid_byte_joins_words),
		cmocka_unit_test(caption_invalid_bytes_take_no_column),
		cmocka_unit_test(caption_only_invalid_bytes),
		cmocka_unit_test(caption_empty_string),
		cmocka_unit_test(caption_all_spaces),
		cmocka_unit_test(caption_newlines_end_rows),
		cmocka_unit_test(caption_blank_lines_collapse),
		cmocka_unit_test(caption_trailing_whitespace_kept),
		cmocka_unit_test(caption_nbsp),
		cmocka_unit_test(caption_controls_become_spaces),
		cmocka_unit_test(caption_word_of_32),
		cmocka_unit_test(caption_word_of_33),
		cmocka_unit_test(caption_space_at_column_32),
		cmocka_unit_test(caption_two_spaces_at_column_32),
		cmocka_unit_test(caption_utf8_2byte_at_wrap),
		cmocka_unit_test(caption_row_and_page_byte_bound),
		cmocka_unit_test(caption_more_than_15_rows),
		cmocka_unit_test(caption_full_page_then_whitespace),
		cmocka_unit_test(caption_any_input_ends),
	};
	int rc = cmocka_run_group_tests(tests, NULL, NULL);

	while (live_harnesses)
		harness_destroy(live_harnesses);
	obs_shutdown();
	return rc;
}
