/* Characterize the exported setter against real libobs, without starting OBS. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

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

static void null_source(void **state)
{
	(void)state;
	struct log_capture capture = {0};
	log_handler_t saved_handler;
	void *saved_param;
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, &capture);
	obs_source_set_default_flags(NULL, UINT32_MAX);
	base_set_log_handler(saved_handler, saved_param);

	assert_int_equal(capture.count, 1);
	assert_int_equal(capture.level, LOG_DEBUG);
	assert_string_equal(capture.message, "obs_source_set_default_flags: Null 'source' parameter");
}

static void exact_storage_and_replacement(void **state)
{
	(void)state;
	struct obs_source source = {0};
	source.flags = UINT32_C(0xa55aa55a);
	source.default_flags = UINT32_MAX;
	source.last_obs_ver = UINT32_C(0x12345678);
	unsigned char expected[sizeof(source)];
	memcpy(expected, &source, sizeof(source));
	const uint32_t patterns[] = {0, UINT32_MAX, UINT32_C(0xaaaaaaaa), UINT32_C(0x55555555), 0};

	/* Unknown bits are stored too: characterized, not endorsed. */
	for (size_t i = 0; i < sizeof(patterns) / sizeof(patterns[0]); i++) {
		memcpy(expected + offsetof(struct obs_source, default_flags), &patterns[i], sizeof(patterns[i]));
		obs_source_set_default_flags(&source, patterns[i]);
		assert_int_equal(source.default_flags, patterns[i]);
		assert_memory_equal(&source, expected, sizeof(source));
	}
	for (unsigned bit = 0; bit < 32; bit++) {
		const uint32_t flag = UINT32_C(1) << bit;
		memcpy(expected + offsetof(struct obs_source, default_flags), &flag, sizeof(flag));
		obs_source_set_default_flags(&source, flag);
		assert_int_equal(source.default_flags, flag);
		assert_memory_equal(&source, expected, sizeof(source));
	}
}

static void independent_sources_and_current_flags(void **state)
{
	(void)state;
	struct obs_source first = {0}, second = {0};
	first.flags = OBS_SOURCE_FLAG_FORCE_MONO;
	second.flags = UINT32_C(0x40000000);
	second.default_flags = UINT32_C(0x2468ace0);

	obs_source_set_default_flags(&first, UINT32_C(0x80000001));
	assert_int_equal(first.default_flags, UINT32_C(0x80000001));
	assert_int_equal(first.flags, OBS_SOURCE_FLAG_FORCE_MONO);
	assert_int_equal(second.default_flags, UINT32_C(0x2468ace0));
	assert_int_equal(second.flags, UINT32_C(0x40000000));

	first.flags = UINT32_MAX;
	obs_source_set_default_flags(&second, 0);
	assert_int_equal(first.default_flags, UINT32_C(0x80000001));
	assert_int_equal(first.flags, UINT32_MAX);
	assert_int_equal(second.default_flags, 0);
	assert_int_equal(second.flags, UINT32_C(0x40000000));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_source),
		cmocka_unit_test(exact_storage_and_replacement),
		cmocka_unit_test(independent_sources_and_current_flags),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
