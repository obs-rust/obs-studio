/* Actual-library characterization: keep unchanged through the Rust swap. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

struct pause_log {
	unsigned calls;
	int level;
	char message[128];
};

#if defined(__GNUC__) || defined(__clang__)
__attribute__((format(printf, 2, 0)))
#endif
static void capture_pause_log(int level, const char *format, va_list args, void *param)
{
	struct pause_log *log = param;
	log->calls++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

static void test_flags(void **state)
{
	(void)state;
	struct obs_output output = {0};
	unsigned char before[sizeof(output)];
	const uint32_t flags[] = {0, OBS_OUTPUT_CAN_PAUSE, UINT32_MAX, UINT32_MAX ^ OBS_OUTPUT_CAN_PAUSE,
				  OBS_OUTPUT_VIDEO | OBS_OUTPUT_AUDIO | OBS_OUTPUT_ENCODED};
	output.owns_info_id = true; /* Unrelated state must remain untouched. */
	for (size_t i = 0; i < sizeof(flags) / sizeof(flags[0]); i++) {
		output.info.flags = flags[i];
		memcpy(before, &output, sizeof(output));
		assert_int_equal(obs_output_can_pause(&output), (flags[i] & OBS_OUTPUT_CAN_PAUSE) != 0);
		assert_memory_equal(&output, before, sizeof(output));
	}
	for (unsigned bit = 0; bit < 32; bit++) {
		output.info.flags = UINT32_C(1) << bit;
		assert_int_equal(obs_output_can_pause(&output), output.info.flags == OBS_OUTPUT_CAN_PAUSE);
		output.info.flags |= OBS_OUTPUT_CAN_PAUSE;
		assert_true(obs_output_can_pause(&output));
	}
}

static void test_null(void **state)
{
	(void)state;
	log_handler_t old;
	void *param;
	struct pause_log log = {0};
	base_get_log_handler(&old, &param);
	base_set_log_handler(capture_pause_log, &log);
	bool result = obs_output_can_pause(NULL);
	base_set_log_handler(old, param);
	assert_false(result);
	assert_int_equal(log.calls, 1);
	assert_int_equal(log.level, LOG_DEBUG);
	assert_string_equal(log.message, "obs_output_can_pause: Null 'output' parameter");
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(test_flags), cmocka_unit_test(test_null)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
