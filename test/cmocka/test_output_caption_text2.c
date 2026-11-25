/* Characterize text2 directly through the exported API and real caption queue.
 * Like test_output_caption_append, only the fields used by the API are needed;
 * no running core, encoder, video backend or replacement API is involved. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include "obs-internal.h"
#include "util/bmem.h"
#include "util/threading.h"

static unsigned log_count;
static int log_level;
static char log_message[256];
static log_handler_t saved_handler;
static void *saved_param;

#if defined(__GNUC__) || defined(__clang__)
__attribute__((format(printf, 2, 0)))
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(log_message, sizeof(log_message), format, args);
}

struct fixture {
	struct obs_output output;
	struct caption_track_data tracks[2];
	long allocations;
};

static int setup(void **state)
{
	long allocations = bnum_allocs();
	struct fixture *f = bzalloc(sizeof(*f));
	f->allocations = allocations;
	for (size_t i = 0; i < 2; i++)
		assert_int_equal(pthread_mutex_init(&f->tracks[i].caption_mutex, NULL), 0);
	f->output.caption_tracks[0] = &f->tracks[0];
	f->output.caption_tracks[MAX_OUTPUT_VIDEO_ENCODERS - 1] = &f->tracks[1];
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, NULL);
	log_count = 0;
	*state = f;
	return 0;
}

static int teardown(void **state)
{
	struct fixture *f = *state;
	base_set_log_handler(saved_handler, saved_param);
	for (size_t i = 0; i < 2; i++) {
		struct caption_text *node = f->tracks[i].caption_head;
		while (node) {
			struct caption_text *next = node->next;
			bfree(node);
			node = next;
		}
		assert_int_equal(pthread_mutex_destroy(&f->tracks[i].caption_mutex), 0);
	}
	long allocations = f->allocations;
	bfree(f);
	assert_int_equal(bnum_allocs(), allocations);
	return 0;
}

static void null_output_stops_before_text(void **state)
{
	(void)state;
	obs_output_output_caption_text2(NULL, NULL, -0.0);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_output_output_caption_text2: Null 'output' parameter");
}

static void inactive_output_is_silent(void **state)
{
	struct fixture *f = *state;
	/* NULL text is never read while inactive, even when reconnecting. */
	os_atomic_set_bool(&f->output.reconnecting, true);
	obs_output_output_caption_text2(&f->output, NULL, 3.25);
	assert_int_equal(log_count, 0);
	for (size_t i = 0; i < 2; i++) {
		assert_null(f->tracks[i].caption_head);
		assert_null(f->tracks[i].caption_tail);
	}
}

static void sparse_tracks_append_copied_text(void **state)
{
	struct fixture *f = *state;
	os_atomic_set_bool(&f->output.active, true);
	char text[] = "hello caption";
	obs_output_output_caption_text2(&f->output, text, 3.25);
	memset(text, 'x', sizeof(text) - 1);
	obs_output_output_caption_text2(&f->output, "second", 3.25);
	/* Characterized, not endorsed: empty input still queues an empty page. */
	obs_output_output_caption_text2(&f->output, "", 3.25);
	assert_int_equal(log_count, 3);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "Caption text: ");
	for (size_t i = 0; i < 2; i++) {
		struct caption_text *node = f->tracks[i].caption_head;
		const char *expected[] = {"hello caption", "second", ""};
		for (size_t j = 0; j < 3; j++) {
			assert_non_null(node);
			assert_string_equal(node->text, expected[j]);
			assert_true(node->display_duration == 3.25);
			if (j == 2)
				assert_ptr_equal(node, f->tracks[i].caption_tail);
			node = node->next;
		}
		assert_null(node);
	}
	assert_ptr_not_equal(f->tracks[0].caption_head, f->tracks[1].caption_head);
}

static void every_page_gets_caller_duration(void **state)
{
	struct fixture *f = *state;
	os_atomic_set_bool(&f->output.active, true);
	char text[482];
	memset(text, 'a', 480);
	text[480] = 'b';
	text[481] = '\0';
	obs_output_output_caption_text2(&f->output, text, 3.25);
	for (size_t i = 0; i < 2; i++) {
		struct caption_text *first = f->tracks[i].caption_head;
		assert_non_null(first);
		/* The page contains fifteen 32-column rows, separated by newlines. */
		assert_int_equal(strlen(first->text), 480 + 14);
		for (size_t row = 0; row < 15; row++) {
			assert_memory_equal(first->text + row * 33, text, 32);
			if (row < 14)
				assert_int_equal(first->text[row * 33 + 32], '\n');
		}
		assert_true(first->display_duration == 3.25);
		struct caption_text *last = first->next;
		assert_non_null(last);
		assert_string_equal(last->text, "b");
		assert_true(last->display_duration == 3.25);
		assert_ptr_equal(last, f->tracks[i].caption_tail);
		assert_null(last->next);
	}
}

static void active_without_tracks(void **state)
{
	struct fixture *f = *state;
	os_atomic_set_bool(&f->output.active, true);
	f->output.caption_tracks[0] = NULL;
	f->output.caption_tracks[MAX_OUTPUT_VIDEO_ENCODERS - 1] = NULL;
	obs_output_output_caption_text2(&f->output, "no tracks", 3.25);
	assert_int_equal(log_count, 1);
	assert_string_equal(log_message, "Caption text: no tracks");
	assert_null(f->tracks[0].caption_head);
	assert_null(f->tracks[1].caption_head);
}

/* Characterized, not endorsed: duration is stored without validation, including
 * signed zero, negative values, infinities and a quiet NaN payload. */
static void duration_bits_survive_repeated_appends(void **state)
{
	struct fixture *f = *state;
	os_atomic_set_bool(&f->output.active, true);
	const uint64_t bits[] = {
		UINT64_C(0x0000000000000000), UINT64_C(0x8000000000000000), UINT64_C(0xbff4000000000000),
		UINT64_C(0x3fb999999999999a), UINT64_C(0x7ff0000000000000), UINT64_C(0xfff0000000000000),
		UINT64_C(0x7ff8000000000123),
	};
	_Static_assert(sizeof(double) == sizeof(uint64_t), "IEEE double storage");
	for (size_t j = 0; j < sizeof(bits) / sizeof(bits[0]); j++) {
		double duration;
		memcpy(&duration, &bits[j], sizeof(duration));
		obs_output_output_caption_text2(&f->output, "duration", duration);
	}
	for (size_t i = 0; i < 2; i++) {
		struct caption_text *node = f->tracks[i].caption_head;
		for (size_t j = 0; j < sizeof(bits) / sizeof(bits[0]); j++) {
			assert_non_null(node);
			assert_string_equal(node->text, "duration");
			assert_memory_equal(&node->display_duration, &bits[j], sizeof(double));
			if (j + 1 == sizeof(bits) / sizeof(bits[0]))
				assert_ptr_equal(node, f->tracks[i].caption_tail);
			node = node->next;
		}
		assert_null(node);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(null_output_stops_before_text, setup, teardown),
		cmocka_unit_test_setup_teardown(inactive_output_is_silent, setup, teardown),
		cmocka_unit_test_setup_teardown(sparse_tracks_append_copied_text, setup, teardown),
		cmocka_unit_test_setup_teardown(every_page_gets_caller_duration, setup, teardown),
		cmocka_unit_test_setup_teardown(active_without_tracks, setup, teardown),
		cmocka_unit_test_setup_teardown(duration_bits_survive_repeated_appends, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
