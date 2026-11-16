/* Public-API characterization before the obs-source Rust swap (#4602).
 * Private stack fixtures allocate nothing and exercise the actual libraries. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

struct media_observation {
	obs_source_t *source;
	int64_t duration;
	int64_t time;
	enum obs_media_state state;
	unsigned calls[3];
	bool reenter;
};

static struct media_observation *expected_context;

static int64_t get_duration(void *data)
{
	assert_ptr_equal(data, expected_context);
	struct media_observation *seen = data;
	seen->calls[0]++;
	if (seen->reenter) {
		/* Queries dispatch synchronously, including same-source reentry. */
		assert_true(obs_source_media_get_time(seen->source) == seen->time);
		assert_int_equal(obs_source_media_get_state(seen->source), seen->state);
	}
	return seen->duration;
}

static int64_t get_time(void *data)
{
	assert_ptr_equal(data, expected_context);
	struct media_observation *seen = data;
	seen->calls[1]++;
	return seen->time;
}

static enum obs_media_state get_state(void *data)
{
	assert_ptr_equal(data, expected_context);
	struct media_observation *seen = data;
	seen->calls[2]++;
	return seen->state;
}

static char messages[3][128];
static unsigned log_count;

#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	assert_int_equal(level, LOG_DEBUG);
	assert_true(log_count < 3);
	vsnprintf(messages[log_count++], sizeof(messages[0]), format, args);
}

static void null_source(void **unused)
{
	(void)unused;
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	log_count = 0;
	base_set_log_handler(capture_log, NULL);
	int64_t duration = obs_source_media_get_duration(NULL);
	int64_t time = obs_source_media_get_time(NULL);
	enum obs_media_state state = obs_source_media_get_state(NULL);
	base_set_log_handler(old_handler, old_param);
	assert_true(duration == 0);
	assert_true(time == 0);
	assert_int_equal(state, OBS_MEDIA_STATE_NONE);
	assert_int_equal(log_count, 3);
	assert_string_equal(messages[0], "obs_source_media_get_duration: Null 'source' parameter");
	assert_string_equal(messages[1], "obs_source_media_get_time: Null 'source' parameter");
	assert_string_equal(messages[2], "obs_source_media_get_state: Null 'source' parameter");
}

static void admission(void **unused)
{
	(void)unused;
	/* Each callback is optional independently. Non-NULL callbacks must still
	 * be suppressed without context data or the controllable-media flag. */
	for (unsigned valid = 0; valid < 2; valid++) {
		for (unsigned controllable = 0; controllable < 2; controllable++) {
			for (unsigned mask = 0; mask < 8; mask++) {
				struct obs_source source = {0};
				struct media_observation seen = {.duration = -123,
								 .time = INT64_MAX,
								 .state = OBS_MEDIA_STATE_ERROR};
				expected_context = &seen;
				source.context.data = valid ? &seen : NULL;
				source.info.output_flags = controllable ? OBS_SOURCE_CONTROLLABLE_MEDIA : 0;
				source.info.media_get_duration = (mask & 1) ? get_duration : NULL;
				source.info.media_get_time = (mask & 2) ? get_time : NULL;
				source.info.media_get_state = (mask & 4) ? get_state : NULL;
				bool admitted = valid && controllable;
				assert_true(obs_source_media_get_duration(&source) ==
					    ((admitted && (mask & 1)) ? seen.duration : 0));
				assert_true(obs_source_media_get_time(&source) ==
					    ((admitted && (mask & 2)) ? seen.time : 0));
				assert_int_equal(obs_source_media_get_state(&source),
						 (admitted && (mask & 4)) ? seen.state : OBS_MEDIA_STATE_NONE);
				for (unsigned i = 0; i < 3; i++)
					assert_int_equal(seen.calls[i], admitted && (mask & (1U << i)) ? 1 : 0);
			}
		}
	}
}

static void results_and_context(void **unused)
{
	(void)unused;
	struct obs_source source = {0};
	source.info.output_flags = OBS_SOURCE_CONTROLLABLE_MEDIA;
	source.info.media_get_duration = get_duration;
	source.info.media_get_time = get_time;
	source.info.media_get_state = get_state;
	struct media_observation contexts[2] = {{0}, {0}};
	const int64_t values[] = {0, 1, -1, INT64_MIN, INT64_MAX, INT64_C(4294967296), -INT64_C(4294967297)};
	const enum obs_media_state states[] = {OBS_MEDIA_STATE_NONE,    OBS_MEDIA_STATE_PLAYING,
					       OBS_MEDIA_STATE_OPENING, OBS_MEDIA_STATE_BUFFERING,
					       OBS_MEDIA_STATE_PAUSED,  OBS_MEDIA_STATE_STOPPED,
					       OBS_MEDIA_STATE_ENDED,   OBS_MEDIA_STATE_ERROR};
	for (size_t c = 0; c < 2; c++) {
		expected_context = &contexts[c];
		source.context.data = expected_context;
		for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
			expected_context->duration = values[i];
			expected_context->time = values[sizeof(values) / sizeof(values[0]) - 1 - i];
			/* Characterized, not endorsed: negative durations and times pass through. */
			assert_true(obs_source_media_get_duration(&source) == expected_context->duration);
			assert_true(obs_source_media_get_time(&source) == expected_context->time);
			assert_int_equal(expected_context->calls[0], i + 1);
			assert_int_equal(expected_context->calls[1], i + 1);
		}
		for (size_t i = 0; i < sizeof(states) / sizeof(states[0]); i++) {
			expected_context->state = states[i];
			assert_int_equal(obs_source_media_get_state(&source), states[i]);
			assert_int_equal(expected_context->calls[2], i + 1);
		}
	}
}

static void synchronous_reentry(void **unused)
{
	(void)unused;
	struct obs_source source = {0};
	struct media_observation seen = {.source = &source,
					 .duration = -9,
					 .time = 42,
					 .state = OBS_MEDIA_STATE_PAUSED,
					 .reenter = true};
	expected_context = &seen;
	source.context.data = &seen;
	source.info.output_flags = OBS_SOURCE_CONTROLLABLE_MEDIA;
	source.info.media_get_duration = get_duration;
	source.info.media_get_time = get_time;
	source.info.media_get_state = get_state;
	assert_true(obs_source_media_get_duration(&source) == seen.duration);
	for (unsigned i = 0; i < 3; i++)
		assert_int_equal(seen.calls[i], 1);
	assert_int_equal(source.media_actions.num, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_source), cmocka_unit_test(admission),
					   cmocka_unit_test(results_and_context),
					   cmocka_unit_test(synchronous_reentry)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
