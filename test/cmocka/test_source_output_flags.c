/* Characterize info.output_flags independently of current/default instance flags. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "libobs/build/libobs.pch.h"

struct log_capture {
	unsigned count;
	int level;
	char message[128];
};

#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	struct log_capture *capture = param;
	capture->count++;
	capture->level = level;
	vsnprintf(capture->message, sizeof(capture->message), format, args);
}

static void info_flags_are_returned_without_mutation(void **state)
{
	(void)state;
	struct obs_source source = {0};
	const uint32_t patterns[] = {0, UINT32_MAX, UINT32_C(0xaaaaaaaa), UINT32_C(0x55555555)};
	unsigned char before[sizeof(source)];

	/* Unknown output bits are returned unchanged: characterized, not endorsed. */
	for (size_t i = 0; i < sizeof(patterns) / sizeof(patterns[0]); i++) {
		source.info.output_flags = patterns[i];
		source.flags = patterns[i] ^ UINT32_C(0x13579bdf);
		source.default_flags = patterns[i] ^ UINT32_C(0x2468ace0);
		memcpy(before, &source, sizeof(source));
		assert_int_equal(obs_source_get_output_flags(&source), patterns[i]);
		assert_memory_equal(&source, before, sizeof(source));
	}
	for (unsigned bit = 0; bit < 32; bit++) {
		source.info.output_flags = UINT32_C(1) << bit;
		source.flags = 0;
		source.default_flags = UINT32_MAX;
		memcpy(before, &source, sizeof(source));
		assert_int_equal(obs_source_get_output_flags(&source), source.info.output_flags);
		assert_memory_equal(&source, before, sizeof(source));
	}
}

static void independent_sources_and_instance_flags(void **state)
{
	(void)state;
	struct obs_source first = {0}, second = {0};
	first.info.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_AUDIO;
	second.info.output_flags = OBS_SOURCE_ASYNC_VIDEO;
	first.flags = second.flags = UINT32_MAX;
	first.default_flags = second.default_flags = OBS_SOURCE_FLAG_FORCE_MONO;

	assert_int_equal(obs_source_get_output_flags(&first), OBS_SOURCE_VIDEO | OBS_SOURCE_AUDIO);
	assert_int_equal(obs_source_get_output_flags(&second), OBS_SOURCE_ASYNC_VIDEO);
	first.flags = first.default_flags = 0;
	assert_int_equal(obs_source_get_output_flags(&first), OBS_SOURCE_VIDEO | OBS_SOURCE_AUDIO);
	first.info.output_flags = UINT32_C(0x80000000);
	assert_int_equal(obs_source_get_output_flags(&first), UINT32_C(0x80000000));
	assert_int_equal(obs_source_get_output_flags(&second), OBS_SOURCE_ASYNC_VIDEO);
}

static void null_source_returns_zero_and_logs(void **state)
{
	(void)state;
	struct log_capture capture = {0};
	log_handler_t saved_handler;
	void *saved_param;
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, &capture);
	const uint32_t result = obs_source_get_output_flags(NULL);
	base_set_log_handler(saved_handler, saved_param);

	assert_int_equal(result, 0);
	assert_int_equal(capture.count, 1);
	assert_int_equal(capture.level, LOG_DEBUG);
	assert_string_equal(capture.message, "obs_source_get_output_flags: Null 'source' parameter");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(info_flags_are_returned_without_mutation),
		cmocka_unit_test(independent_sources_and_instance_flags),
		cmocka_unit_test(null_source_returns_zero_and_logs),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
