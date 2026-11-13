/* Actual-library characterization of the legacy preferred-size wrapper.
 * Keep unchanged through the Rust swap (Refs #4816). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

struct preferred_size_log {
	unsigned calls;
	int level;
	char message[192];
};

#if defined(__GNUC__) || defined(__clang__)
__attribute__((format(printf, 2, 0)))
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	struct preferred_size_log *log = param;
	log->calls++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

static struct preferred_size_log set_size(obs_output_t *output, uint32_t width, uint32_t height)
{
	log_handler_t old_handler;
	void *old_param;
	struct preferred_size_log log = {0};
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, &log);
	obs_output_set_preferred_size(output, width, height);
	base_set_log_handler(old_handler, old_param);
	return log;
}

static void null_output_uses_legacy_diagnostic(void **state)
{
	(void)state;
	struct preferred_size_log log = set_size(NULL, 0, UINT32_MAX);
	assert_int_equal(log.calls, 1);
	assert_int_equal(log.level, LOG_DEBUG);
	assert_string_equal(log.message, "obs_output_set_preferred_size: Null 'output' parameter");
}

static void audio_only_rejects_before_delegation(void **state)
{
	(void)state;
	struct obs_output output = {0};
	unsigned char before[sizeof(output)];
	output.context.name = "audio-only";
	output.info.flags = OBS_OUTPUT_AUDIO;
	output.scaled_width = 37;
	output.scaled_height = 53;
	/* Even an active output must report the wrapper's flag diagnostic first. */
	for (unsigned active = 0; active < 2; active++) {
		output.active = active != 0;
		memcpy(before, &output, sizeof(output));
		struct preferred_size_log log = set_size(&output, 0, UINT32_MAX);
		assert_int_equal(log.calls, 1);
		assert_int_equal(log.level, LOG_WARNING);
		assert_string_equal(
			log.message,
			"Output 'audio-only': Tried to use obs_output_set_preferred_size on a non-video output");
		assert_memory_equal(&output, before, sizeof(output));
	}
}

static void delegates_unchanged_dimensions_to_slot_zero(void **state)
{
	(void)state;
	struct obs_output output = {0};
	struct obs_output expected;
	const uint32_t sizes[][2] = {{1, 3}, {1919, 1079}, {UINT32_MAX, UINT32_MAX - 1}, {0, 701}, {905, 0}, {0, 0}};
	output.context.name = "raw-video";
	output.info.flags = OBS_OUTPUT_VIDEO;
	output.scaled_width = 37;
	output.scaled_height = 53;
	for (size_t i = 0; i < sizeof(sizes) / sizeof(sizes[0]); i++) {
		memcpy(&expected, &output, sizeof(output));
		expected.scaled_width = sizes[i][0];
		expected.scaled_height = sizes[i][1];
		struct preferred_size_log log = set_size(&output, sizes[i][0], sizes[i][1]);
		assert_int_equal(log.calls, 0);
		/* Only index 0 updates a raw output's stored preferred size. */
		assert_int_equal(obs_output_get_width(&output), sizes[i][0]);
		assert_int_equal(obs_output_get_height(&output), sizes[i][1]);
		assert_int_equal(obs_output_get_width2(&output, 0), sizes[i][0]);
		assert_int_equal(obs_output_get_height2(&output, 0), sizes[i][1]);
		assert_memory_equal(&output, &expected, sizeof(output));
	}
}

static void active_video_keeps_indexed_setter_diagnostic(void **state)
{
	(void)state;
	struct obs_output output = {0};
	unsigned char before[sizeof(output)];
	output.context.name = "active-video";
	output.info.flags = OBS_OUTPUT_VIDEO;
	output.active = true;
	output.scaled_width = 37;
	output.scaled_height = 53;
	memcpy(before, &output, sizeof(output));
	struct preferred_size_log log = set_size(&output, 1919, 1079);
	assert_int_equal(log.calls, 1);
	assert_int_equal(log.level, LOG_WARNING);
	assert_string_equal(log.message,
			    "output 'active-video': Cannot set the preferred resolution while the output is active");
	assert_memory_equal(&output, before, sizeof(output));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_output_uses_legacy_diagnostic),
		cmocka_unit_test(audio_only_rejects_before_delegation),
		cmocka_unit_test(delegates_unchanged_dimensions_to_slot_zero),
		cmocka_unit_test(active_video_keeps_indexed_setter_diagnostic),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
