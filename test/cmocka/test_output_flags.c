/* Actual-library characterization; unchanged through the Rust swap. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

struct flags_log {
	unsigned calls;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
__attribute__((format(printf, 2, 0)))
#endif
static void capture_flags_log(int level, const char *format, va_list args, void *param)
{
	struct flags_log *log = param;
	log->calls++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

static void flags(void **state)
{
	(void)state;
	struct obs_output output = {0};
	unsigned char before[sizeof(output)];
	const uint32_t special[] = {0, 1, 3, 7, 0x80000000u, 0xaaaaaaaau, UINT32_MAX};
	output.owns_info_id = true;
	output.mixer_mask = 65;
	for (size_t i = 0; i < sizeof(special) / sizeof(special[0]) + 32; i++) {
		uint32_t value = i < sizeof(special) / sizeof(special[0])
					 ? special[i]
					 : UINT32_C(1) << (i - sizeof(special) / sizeof(special[0]));
		output.info.flags = value;
		memcpy(before, &output, sizeof(output));
		for (unsigned repeat = 0; repeat < 2; repeat++) {
			assert_int_equal(obs_output_get_flags(&output), value);
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
	struct flags_log log = {0};
	base_set_log_handler(capture_flags_log, &log);
	uint32_t result = obs_output_get_flags(NULL);
	base_set_log_handler(old, param);
	assert_int_equal(result, 0);
	assert_int_equal(log.calls, 1);
	assert_int_equal(log.level, LOG_DEBUG);
	assert_string_equal(log.message, "obs_output_get_flags: Null 'output' parameter");
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(flags), cmocka_unit_test(null_output)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
