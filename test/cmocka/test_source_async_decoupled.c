/* Actual-library characterization of the async decoupling reset. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static unsigned log_count;
static int log_level;
static char log_message[128];
#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(log_message, sizeof(log_message), format, args);
}

static void null_source(void **state)
{
	(void)state;
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	obs_source_set_async_decoupled(NULL, false);
	obs_source_set_async_decoupled(NULL, true);
	base_set_log_handler(old_handler, old_param);
	assert_int_equal(log_count, 2);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_set_async_decoupled: Null 'source' parameter");
}

static int setup_source(void **state)
{
	struct obs_source *source = bzalloc(sizeof(*source));
	if (pthread_mutex_init(&source->audio_buf_mutex, NULL) != 0) {
		bfree(source);
		return -1;
	}
	*state = source;
	return 0;
}

static int teardown_source(void **state)
{
	struct obs_source *source = *state;
	for (size_t i = 0; i < MAX_AUDIO_CHANNELS; i++)
		deque_free(&source->audio_input_buf[i]);
	pthread_mutex_destroy(&source->audio_buf_mutex);
	bfree(source);
	return 0;
}

static void seed_audio(struct obs_source *source)
{
	const unsigned char bytes[] = {11, 22, 33, 44, 55};
	for (size_t i = 0; i < MAX_AUDIO_CHANNELS; i++) {
		/* Leave one channel empty; make the others nonempty and unequal. */
		if (i != 1)
			deque_push_back(&source->audio_input_buf[i], bytes, 1 + i % sizeof(bytes));
	}
	source->timing_set = true;
	source->timing_adjust = 123456;
	source->last_audio_input_buf_size = 87;
	source->audio_ts = 987654;
	source->next_audio_sys_ts_min = 456789;
	source->async_unbuffered = true;
	source->async_width = 37;
	source->async_height = 53;
}

static void enabling_resets_even_when_already_enabled(void **state)
{
	struct obs_source *source = *state;
	for (unsigned call = 0; call < 2; call++) {
		seed_audio(source);
		assert_int_equal(source->async_decoupled, call != 0);
		obs_source_set_async_decoupled(source, true);
		assert_true(source->async_decoupled);
		assert_false(source->timing_set);
		assert_int_equal(source->last_audio_input_buf_size, 0);
		assert_int_equal(source->audio_ts, 0);
		assert_int_equal(source->next_audio_sys_ts_min, 0);
		for (size_t i = 0; i < MAX_AUDIO_CHANNELS; i++)
			assert_int_equal(source->audio_input_buf[i].size, 0);
		assert_int_equal(source->timing_adjust, 123456);
		assert_true(source->async_unbuffered);
		assert_int_equal(source->async_width, 37);
		assert_int_equal(source->async_height, 53);
		/* The reset releases the mutex before returning. */
		int lock_result = pthread_mutex_trylock(&source->audio_buf_mutex);
		if (lock_result == 0)
			pthread_mutex_unlock(&source->audio_buf_mutex);
		assert_int_equal(lock_result, 0);
	}
}

static void disabling_preserves_audio(void **state)
{
	struct obs_source *source = *state;
	seed_audio(source);
	for (unsigned previous = 0; previous < 2; previous++) {
		source->async_decoupled = previous != 0;
		unsigned char expected[sizeof(*source)];
		source->async_decoupled = false;
		memcpy(expected, source, sizeof(*source));
		source->async_decoupled = previous != 0;
		obs_source_set_async_decoupled(source, false);
		assert_memory_equal(source, expected, sizeof(*source));
		const unsigned char bytes[] = {11, 22, 33, 44, 55};
		for (size_t i = 0; i < MAX_AUDIO_CHANNELS; i++) {
			if (i == 1)
				continue;
			unsigned char actual[sizeof(bytes)] = {0};
			deque_peek_front(&source->audio_input_buf[i], actual, 1 + i % sizeof(bytes));
			assert_memory_equal(actual, bytes, 1 + i % sizeof(bytes));
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_source),
		cmocka_unit_test_setup_teardown(enabling_resets_even_when_already_enabled, setup_source,
						teardown_source),
		cmocka_unit_test_setup_teardown(disabling_preserves_audio, setup_source, teardown_source),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
