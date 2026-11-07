/* Characterize actual-library counters, including the historical log typo. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <stdio.h>
#include <cmocka.h>
#include "obs-internal.h"

static unsigned log_count;
static int log_level;
static char log_message[128];

#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
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
	uint32_t frames = obs_encoder_get_encoded_frames(NULL);
	unsigned after_frames = log_count;
	uint64_t offset = obs_encoder_get_pause_offset(NULL);
	base_set_log_handler(old_handler, old_param);
	assert_int_equal(frames, 0);
	assert_int_equal(offset, 0);
	assert_int_equal(after_frames, 1);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	/* Characterized, not endorsed: the encoder getter names the output. */
	assert_string_equal(log_message, "obs_output_get_encoded_frames: Null 'encoder' parameter");
}

static void exact_independent_counters(void **state)
{
	(void)state;
	struct obs_encoder encoder = {0};
	const uint32_t frames[] = {0, 1, UINT32_C(0x80000000), UINT32_MAX};
	const uint64_t offsets[] = {0, 1, UINT64_C(0x8000000000000000), UINT64_MAX};
	for (size_t i = 0; i < sizeof(frames) / sizeof(frames[0]); i++) {
		for (size_t j = 0; j < sizeof(offsets) / sizeof(offsets[0]); j++) {
			encoder.encoded_frames = frames[i];
			encoder.pause.ts_offset = offsets[j];
			encoder.pause.ts_start = 17;
			encoder.pause.ts_end = 29;
			assert_int_equal(obs_encoder_get_encoded_frames(&encoder), frames[i]);
			assert_true(obs_encoder_get_pause_offset(&encoder) == offsets[j]);
			assert_int_equal(encoder.encoded_frames, frames[i]);
			assert_true(encoder.pause.ts_offset == offsets[j]);
			assert_int_equal(encoder.pause.ts_start, 17);
			assert_int_equal(encoder.pause.ts_end, 29);
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_defaults),
					   cmocka_unit_test(exact_independent_counters)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
