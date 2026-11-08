/* Actual-library characterization; unchanged through the Rust swap. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

struct mixer_log {
	unsigned calls;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
__attribute__((format(printf, 2, 0)))
#endif
static void capture_mixer_log(int level, const char *format, va_list args, void *param)
{
	struct mixer_log *log = param;
	log->calls++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

static void masks(void **state)
{
	(void)state;
	struct obs_output output = {0};
	unsigned char before[sizeof(output)];
	const size_t special[] = {0, 1, 2, 3, 32, 64, 65, 128, SIZE_MAX};
	output.owns_info_id = true;
	for (size_t i = 0; i < sizeof(special) / sizeof(special[0]) + sizeof(size_t) * 8; i++) {
		size_t mask = i < sizeof(special) / sizeof(special[0])
				      ? special[i]
				      : (size_t)1 << (i - sizeof(special) / sizeof(special[0]));
		size_t expected = 0;
		for (size_t bit = 0; bit < MAX_AUDIO_MIXES; bit++) {
			if (mask & ((size_t)1 << bit)) {
				expected = bit;
				break;
			}
		}
		output.mixer_mask = mask;
		output.info.flags = (uint32_t)i;
		memcpy(before, &output, sizeof(output));
		for (unsigned repeat = 0; repeat < 2; repeat++) {
			assert_int_equal(obs_output_get_mixer(&output), expected);
			assert_int_equal(obs_output_get_mixers(&output), mask);
			assert_memory_equal(&output, before, sizeof(output));
		}
	}
}

static void null_output(void **state)
{
	(void)state;
	log_handler_t old;
	void *param;
	base_get_log_handler(&old, &param);
	size_t (*getters[])(const obs_output_t *) = {obs_output_get_mixer, obs_output_get_mixers};
	const char *messages[] = {"obs_output_get_mixer: Null 'output' parameter",
				  "obs_output_get_mixers: Null 'output' parameter"};
	for (size_t i = 0; i < 2; i++) {
		struct mixer_log log = {0};
		base_set_log_handler(capture_mixer_log, &log);
		size_t result = getters[i](NULL);
		base_set_log_handler(old, param);
		assert_int_equal(result, 0);
		assert_int_equal(log.calls, 1);
		assert_int_equal(log.level, LOG_DEBUG);
		assert_string_equal(log.message, messages[i]);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(masks), cmocka_unit_test(null_output)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
