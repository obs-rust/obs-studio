/* Characterize the public queries against the actual loaded libobs. */
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

static void null_defaults(void **state)
{
	(void)state;
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	bool paused = obs_output_paused(NULL);
	unsigned after_paused = log_count;
	char paused_message[128];
	memcpy(paused_message, log_message, sizeof(paused_message));
	uint64_t offset = obs_output_get_pause_offset(NULL);
	base_set_log_handler(old_handler, old_param);
	assert_false(paused);
	assert_int_equal(offset, 0);
	assert_int_equal(after_paused, 1);
	assert_int_equal(log_count, 2);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(paused_message, "obs_output_paused: Null 'output' parameter");
	assert_string_equal(log_message, "obs_output_get_pause_offset: Null 'output' parameter");
}

static void independent_states(void **state)
{
	(void)state;
	struct obs_output output = {0};
	assert_int_equal(pthread_mutex_init(&output.pause.mutex, NULL), 0);
	const uint64_t offsets[] = {0, 1, UINT32_MAX, UINT64_C(0x8000000000000000), UINT64_MAX};
	for (unsigned paused = 0; paused < 2; paused++) {
		os_atomic_set_bool(&output.paused, paused != 0);
		for (size_t i = 0; i < sizeof(offsets) / sizeof(offsets[0]); i++) {
			output.pause.ts_start = 41;
			output.pause.ts_end = 73;
			output.pause.ts_offset = offsets[i];
			assert_int_equal(obs_output_paused(&output), paused);
			assert_true(obs_output_get_pause_offset(&output) == offsets[i]);
			assert_true(output.pause.ts_offset == offsets[i]);
			assert_int_equal(output.pause.ts_start, 41);
			assert_int_equal(output.pause.ts_end, 73);
			assert_int_equal(pthread_mutex_trylock(&output.pause.mutex), 0);
			assert_int_equal(pthread_mutex_unlock(&output.pause.mutex), 0);
		}
	}
	assert_int_equal(pthread_mutex_destroy(&output.pause.mutex), 0);
}

static void *write_offsets(void *param)
{
	struct obs_output *output = param;
	for (unsigned i = 0; i < 4096; i++) {
		pthread_mutex_lock(&output->pause.mutex);
		output->pause.ts_offset = (i & 1) ? UINT64_MAX : 0;
		pthread_mutex_unlock(&output->pause.mutex);
	}
	return NULL;
}

static void synchronized_offset(void **state)
{
	(void)state;
	struct obs_output output = {0};
	assert_int_equal(pthread_mutex_init(&output.pause.mutex, NULL), 0);
	pthread_t writer;
	assert_int_equal(pthread_create(&writer, NULL, write_offsets, &output), 0);
	bool valid = true;
	for (unsigned i = 0; i < 4096; i++) {
		uint64_t offset = obs_output_get_pause_offset(&output);
		valid = valid && (offset == 0 || offset == UINT64_MAX);
	}
	assert_int_equal(pthread_join(writer, NULL), 0);
	assert_int_equal(pthread_mutex_destroy(&output.pause.mutex), 0);
	assert_true(valid);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_defaults), cmocka_unit_test(independent_states),
					   cmocka_unit_test(synchronized_offset)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
