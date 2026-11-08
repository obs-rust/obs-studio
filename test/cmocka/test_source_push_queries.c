/* Actual-library characterization of the synchronized push-to-mute/talk getters. */
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
	const uint64_t mute = obs_source_push_to_mute_enabled(NULL);
	char messages[4][128];
	memcpy(messages[0], log_message, sizeof(log_message));
	const uint64_t mute_delay = obs_source_get_push_to_mute_delay(NULL);
	memcpy(messages[1], log_message, sizeof(log_message));
	const uint64_t talk = obs_source_push_to_talk_enabled(NULL);
	memcpy(messages[2], log_message, sizeof(log_message));
	const uint64_t talk_delay = obs_source_get_push_to_talk_delay(NULL);
	memcpy(messages[3], log_message, sizeof(log_message));
	base_set_log_handler(old_handler, old_param);
	const uint64_t values[] = {mute, mute_delay, talk, talk_delay};
	for (size_t i = 0; i < 4; i++)
		assert_int_equal(values[i], 0);
	assert_int_equal(log_count, 4);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(messages[0], "obs_source_push_to_mute_enabled: Null 'source' parameter");
	assert_string_equal(messages[1], "obs_source_get_push_to_mute_delay: Null 'source' parameter");
	assert_string_equal(messages[2], "obs_source_push_to_talk_enabled: Null 'source' parameter");
	assert_string_equal(messages[3], "obs_source_get_push_to_talk_delay: Null 'source' parameter");
}

static void independent_fields(void **state)
{
	(void)state;
	const uint64_t edges[] = {0, 1, UINT64_C(0x8000000000000000), UINT64_MAX, UINT64_C(0xa5a55a5af00f1234)};
	struct obs_source source = {0};
	assert_int_equal(pthread_mutex_init(&source.audio_mutex, NULL), 0);
	for (size_t m = 0; m < sizeof(edges) / sizeof(edges[0]); m++) {
		for (size_t t = 0; t < sizeof(edges) / sizeof(edges[0]); t++) {
			for (unsigned flags = 0; flags < 4; flags++) {
				source.push_to_mute_enabled = (flags & 1) != 0;
				source.push_to_talk_enabled = (flags & 2) != 0;
				source.push_to_mute_delay = edges[m];
				source.push_to_talk_delay = edges[t];
				source.push_to_mute_pressed = !source.push_to_mute_enabled;
				source.push_to_talk_pressed = !source.push_to_talk_enabled;
				for (unsigned repeat = 0; repeat < 2; repeat++) {
					assert_int_equal(obs_source_push_to_mute_enabled(&source), flags & 1);
					assert_int_equal(obs_source_push_to_talk_enabled(&source), (flags & 2) != 0);
					assert_true(obs_source_get_push_to_mute_delay(&source) == edges[m]);
					assert_true(obs_source_get_push_to_talk_delay(&source) == edges[t]);
					assert_int_equal(source.push_to_mute_enabled, flags & 1);
					assert_int_equal(source.push_to_talk_enabled, (flags & 2) != 0);
					assert_true(source.push_to_mute_delay == edges[m]);
					assert_true(source.push_to_talk_delay == edges[t]);
					assert_int_equal(source.push_to_mute_pressed, !(flags & 1));
					assert_int_equal(source.push_to_talk_pressed, !(flags & 2));
				}
			}
		}
	}
	assert_int_equal(pthread_mutex_destroy(&source.audio_mutex), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_defaults), cmocka_unit_test(independent_fields)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
